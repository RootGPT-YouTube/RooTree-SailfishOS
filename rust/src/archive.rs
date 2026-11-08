//! Gli archivi: riconoscerli dal nome, elencarne il contenuto (per sfogliarli
//! come cartelle), estrarli e crearli.
//!
//! - zip, tar (anche .gz .bz2 .xz .zst) e 7z: crate Rust, si leggono e si creano;
//! - un file compresso da solo (`nota.txt.gz`): un archivio con una voce sola;
//! - RAR e i formati rari (iso, cpio, cab, lha, deb, rpm...): solo lettura, con
//!   la libarchive del sistema caricata a runtime (nel target di build non ci
//!   sono gli header, e così RooTree parte anche dove manca: lo dice e basta).
//!
//! Sicurezza dell'estrazione (un archivio può essere costruito apposta):
//! - i nomi con `..` si saltano, quelli assoluti diventano relativi;
//! - tutto si scrive in una cartella temporanea nascosta accanto alla meta;
//! - i link simbolici si creano per ULTIMI: nessun file viene mai scritto
//!   attraverso un link estratto dallo stesso archivio;
//! - niente setuid/setgid/sticky dai permessi salvati.

use std::collections::{BTreeMap, HashMap};
use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::fs::{self, File};
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::os::unix::fs::{MetadataExt, PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::ffi::Entry;
use crate::fsops;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Comp {
    None,
    Gz,
    Bz2,
    Xz,
    Zst,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Format {
    Zip,
    Tar(Comp),
    SevenZ,
    /// un solo file compresso, senza tar
    Single(Comp),
    /// letto con libarchive
    Lib,
}

/// Il formato dal nome del file (non dal contenuto: costa niente e basta per
/// decidere cosa fare al tocco).
pub fn format_of(name: &str) -> Option<Format> {
    let n = name.to_ascii_lowercase();
    let ends = |s: &str| n.len() > s.len() && n.ends_with(s);
    let f = if ends(".tar") {
        Format::Tar(Comp::None)
    } else if ends(".tar.gz") || ends(".tgz") {
        Format::Tar(Comp::Gz)
    } else if ends(".tar.bz2") || ends(".tbz2") || ends(".tbz") {
        Format::Tar(Comp::Bz2)
    } else if ends(".tar.xz") || ends(".txz") {
        Format::Tar(Comp::Xz)
    } else if ends(".tar.zst") || ends(".tzst") {
        Format::Tar(Comp::Zst)
    } else if ends(".zip") || ends(".cbz") {
        Format::Zip
    } else if ends(".7z") {
        Format::SevenZ
    } else if ends(".gz") {
        Format::Single(Comp::Gz)
    } else if ends(".bz2") {
        Format::Single(Comp::Bz2)
    } else if ends(".xz") {
        Format::Single(Comp::Xz)
    } else if ends(".zst") {
        Format::Single(Comp::Zst)
    } else if [".rar", ".cbr", ".iso", ".cpio", ".cab", ".lzh", ".lha", ".xar", ".deb", ".rpm", ".ar"]
        .iter()
        .any(|e| ends(e))
    {
        Format::Lib
    } else {
        return None;
    };
    Some(f)
}

/// I formati che RooTree sa anche creare.
pub fn creatable(name: &str) -> Option<Format> {
    match format_of(name)? {
        f @ (Format::Zip | Format::Tar(_) | Format::SevenZ) => Some(f),
        _ => None,
    }
}

/// Una voce dell'archivio. `path` è già ripulito (vedi `clean`).
#[derive(Clone, Debug)]
pub struct AEntry {
    pub path: String,
    pub is_dir: bool,
    pub size: u64,
    pub mtime: i64,
    pub mode: u32,
    /// destinazione di un link simbolico
    pub link: Option<String>,
    /// voce dello stesso archivio di cui questa è un link fisico (tar)
    pub hardlink: Option<String>,
}

/// Il nome di una voce, reso sicuro: niente `/` iniziali, niente `.`, e
/// `None` se prova a uscire con `..` (o se resta vuoto).
pub fn clean(name: &str) -> Option<String> {
    let mut parts = Vec::new();
    for p in name.split(['/', '\\']) {
        match p {
            "" | "." => {}
            ".." => return None,
            p => parts.push(p),
        }
    }
    (!parts.is_empty()).then(|| parts.join("/"))
}

fn other(e: impl std::fmt::Display) -> io::Error {
    io::Error::other(e.to_string())
}

fn password_error() -> io::Error {
    io::Error::new(io::ErrorKind::Unsupported, "archivio protetto da password: non ancora supportato")
}

/// L'errore che il chiamante usa per dire «annullato».
pub fn cancelled() -> io::Error {
    io::Error::new(io::ErrorKind::Interrupted, "annullato")
}

// ------------------------------------------------------------------ date

/// Ora locale «da orologio» (come la scrivono zip) → secondi Unix.
fn local_to_unix(y: i32, mo: u32, d: u32, h: u32, mi: u32, s: u32) -> i64 {
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    tm.tm_year = y - 1900;
    tm.tm_mon = mo as i32 - 1;
    tm.tm_mday = d as i32;
    tm.tm_hour = h as i32;
    tm.tm_min = mi as i32;
    tm.tm_sec = s as i32;
    tm.tm_isdst = -1;
    unsafe { libc::mktime(&mut tm) as i64 }
}

fn unix_to_zip(t: i64) -> zip::DateTime {
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    let tt = t as libc::time_t;
    unsafe { libc::localtime_r(&tt, &mut tm) };
    zip::DateTime::from_date_and_time(
        (tm.tm_year + 1900).clamp(1980, 2107) as u16,
        (tm.tm_mon + 1) as u8,
        tm.tm_mday as u8,
        tm.tm_hour as u8,
        tm.tm_min as u8,
        tm.tm_sec as u8,
    )
    .unwrap_or_default()
}

fn system_to_unix(t: SystemTime) -> i64 {
    match t.duration_since(UNIX_EPOCH) {
        Ok(d) => d.as_secs() as i64,
        Err(e) => -(e.duration().as_secs() as i64),
    }
}

// ------------------------------------------------------------------ decompressori

fn decoder<'a>(r: impl Read + 'a, c: Comp) -> io::Result<Box<dyn Read + 'a>> {
    Ok(match c {
        Comp::None => Box::new(r),
        Comp::Gz => Box::new(flate2::read::MultiGzDecoder::new(r)),
        Comp::Bz2 => Box::new(bzip2::read::MultiBzDecoder::new(r)),
        Comp::Xz => Box::new(lzma_rust2::XzReader::new(r, true)),
        Comp::Zst => Box::new(zstd::stream::read::Decoder::new(r)?),
    })
}

/// Un compressore che va chiuso (le code dei formati) prima di rinominare.
enum Encoder<W: Write> {
    Plain(W),
    Gz(flate2::write::GzEncoder<W>),
    Bz2(bzip2::write::BzEncoder<W>),
    Xz(lzma_rust2::XzWriter<W>),
    Zst(zstd::stream::write::Encoder<'static, W>),
}

impl<W: Write> Encoder<W> {
    fn new(w: W, c: Comp) -> io::Result<Self> {
        Ok(match c {
            Comp::None => Encoder::Plain(w),
            Comp::Gz => Encoder::Gz(flate2::write::GzEncoder::new(w, flate2::Compression::default())),
            Comp::Bz2 => Encoder::Bz2(bzip2::write::BzEncoder::new(w, bzip2::Compression::default())),
            Comp::Xz => Encoder::Xz(lzma_rust2::XzWriter::new(w, lzma_rust2::XzOptions::with_preset(6))?),
            Comp::Zst => Encoder::Zst(zstd::stream::write::Encoder::new(w, 3)?),
        })
    }

    fn finish(self) -> io::Result<W> {
        match self {
            Encoder::Plain(w) => Ok(w),
            Encoder::Gz(e) => e.finish(),
            Encoder::Bz2(e) => e.finish(),
            Encoder::Xz(e) => e.finish(),
            Encoder::Zst(e) => e.finish(),
        }
    }
}

impl<W: Write> Write for Encoder<W> {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        match self {
            Encoder::Plain(w) => w.write(b),
            Encoder::Gz(e) => e.write(b),
            Encoder::Bz2(e) => e.write(b),
            Encoder::Xz(e) => e.write(b),
            Encoder::Zst(e) => e.write(b),
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        match self {
            Encoder::Plain(w) => w.flush(),
            Encoder::Gz(e) => e.flush(),
            Encoder::Bz2(e) => e.flush(),
            Encoder::Xz(e) => e.flush(),
            Encoder::Zst(e) => e.flush(),
        }
    }
}

// ------------------------------------------------------------------ libarchive (dlopen)

#[allow(non_snake_case)]
struct LibArchive {
    read_new: unsafe extern "C" fn() -> *mut c_void,
    support_filter_all: unsafe extern "C" fn(*mut c_void) -> c_int,
    support_format_all: unsafe extern "C" fn(*mut c_void) -> c_int,
    open_filename: unsafe extern "C" fn(*mut c_void, *const c_char, usize) -> c_int,
    next_header: unsafe extern "C" fn(*mut c_void, *mut *mut c_void) -> c_int,
    read_data: unsafe extern "C" fn(*mut c_void, *mut c_void, usize) -> isize,
    read_free: unsafe extern "C" fn(*mut c_void) -> c_int,
    error_string: unsafe extern "C" fn(*mut c_void) -> *const c_char,
    entry_pathname: unsafe extern "C" fn(*mut c_void) -> *const c_char,
    entry_pathname_utf8: Option<unsafe extern "C" fn(*mut c_void) -> *const c_char>,
    entry_size: unsafe extern "C" fn(*mut c_void) -> i64,
    entry_mtime: unsafe extern "C" fn(*mut c_void) -> libc::time_t,
    entry_filetype: unsafe extern "C" fn(*mut c_void) -> libc::mode_t,
    entry_perm: unsafe extern "C" fn(*mut c_void) -> libc::mode_t,
    entry_symlink: unsafe extern "C" fn(*mut c_void) -> *const c_char,
    entry_hardlink: unsafe extern "C" fn(*mut c_void) -> *const c_char,
    entry_is_encrypted: Option<unsafe extern "C" fn(*mut c_void) -> c_int>,
}

const ARCHIVE_EOF: c_int = 1;
const ARCHIVE_WARN: c_int = -20;

fn libarchive() -> Result<&'static LibArchive, String> {
    static L: OnceLock<Result<LibArchive, String>> = OnceLock::new();
    L.get_or_init(|| unsafe {
        let mut h = std::ptr::null_mut();
        for name in [c"libarchive.so.13", c"libarchive.so"] {
            h = libc::dlopen(name.as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL);
            if !h.is_null() {
                break;
            }
        }
        if h.is_null() {
            return Err("per questo formato serve libarchive: installare il pacchetto «libarchive»".into());
        }
        let sym = |n: &CStr| {
            let p = libc::dlsym(h, n.as_ptr());
            if p.is_null() { None } else { Some(p) }
        };
        macro_rules! need {
            ($n:literal) => {
                match sym($n) {
                    Some(p) => std::mem::transmute::<*mut c_void, _>(p),
                    None => return Err(format!("libarchive senza {}", $n.to_string_lossy())),
                }
            };
        }
        macro_rules! may {
            ($n:literal) => {
                sym($n).map(|p| std::mem::transmute::<*mut c_void, _>(p))
            };
        }
        Ok(LibArchive {
            read_new: need!(c"archive_read_new"),
            support_filter_all: need!(c"archive_read_support_filter_all"),
            support_format_all: need!(c"archive_read_support_format_all"),
            open_filename: need!(c"archive_read_open_filename"),
            next_header: need!(c"archive_read_next_header"),
            read_data: need!(c"archive_read_data"),
            read_free: need!(c"archive_read_free"),
            error_string: need!(c"archive_error_string"),
            entry_pathname: need!(c"archive_entry_pathname"),
            entry_pathname_utf8: may!(c"archive_entry_pathname_utf8"),
            entry_size: need!(c"archive_entry_size"),
            entry_mtime: need!(c"archive_entry_mtime"),
            entry_filetype: need!(c"archive_entry_filetype"),
            entry_perm: need!(c"archive_entry_perm"),
            entry_symlink: need!(c"archive_entry_symlink"),
            entry_hardlink: need!(c"archive_entry_hardlink"),
            entry_is_encrypted: may!(c"archive_entry_is_encrypted"),
        })
    })
    .as_ref()
    .map_err(|e| e.clone())
}

struct LaReader<'a> {
    l: &'a LibArchive,
    a: *mut c_void,
}

impl Read for LaReader<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = unsafe { (self.l.read_data)(self.a, buf.as_mut_ptr().cast(), buf.len()) };
        if n < 0 {
            return Err(other(unsafe { cstr((self.l.error_string)(self.a)) }));
        }
        Ok(n as usize)
    }
}

unsafe fn cstr(p: *const c_char) -> String {
    if p.is_null() { String::new() } else { unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned() }
}

fn walk_lib(path: &Path, f: &mut Visitor) -> io::Result<()> {
    let l = libarchive().map_err(other)?;
    let c = CString::new(path.as_os_str().as_encoded_bytes()).map_err(other)?;
    unsafe {
        let a = (l.read_new)();
        if a.is_null() {
            return Err(other("libarchive: memoria esaurita"));
        }
        struct Free<'a>(&'a LibArchive, *mut c_void);
        impl Drop for Free<'_> {
            fn drop(&mut self) {
                unsafe { (self.0.read_free)(self.1) };
            }
        }
        let _free = Free(l, a);
        (l.support_filter_all)(a);
        (l.support_format_all)(a);
        if (l.open_filename)(a, c.as_ptr(), 1 << 16) != 0 {
            return Err(other(cstr((l.error_string)(a))));
        }
        loop {
            let mut e = std::ptr::null_mut();
            let r = (l.next_header)(a, &mut e);
            if r == ARCHIVE_EOF {
                return Ok(());
            }
            if r < ARCHIVE_WARN {
                return Err(other(cstr((l.error_string)(a))));
            }
            if l.entry_is_encrypted.is_some_and(|enc| enc(e) != 0) {
                return Err(password_error());
            }
            let p = l.entry_pathname_utf8.map(|g| g(e)).filter(|p| !p.is_null()).unwrap_or_else(|| (l.entry_pathname)(e));
            let Some(name) = clean(&cstr(p)) else { continue };
            let ft = (l.entry_filetype)(e) as u32 & libc::S_IFMT;
            let link = (ft == libc::S_IFLNK).then(|| cstr((l.entry_symlink)(e)));
            let hard = cstr((l.entry_hardlink)(e));
            let hardlink = if hard.is_empty() { None } else { clean(&hard) };
            if !(ft == libc::S_IFREG || ft == libc::S_IFDIR || ft == libc::S_IFLNK || hardlink.is_some()) {
                continue; // dispositivi, fifo: non si estraggono
            }
            let ae = AEntry {
                path: name,
                is_dir: ft == libc::S_IFDIR,
                size: (l.entry_size)(e).max(0) as u64,
                mtime: (l.entry_mtime)(e) as i64,
                mode: (l.entry_perm)(e) as u32,
                link,
                hardlink,
            };
            let mut rd = LaReader { l, a };
            if !f(&ae, &mut rd)? {
                return Ok(());
            }
        }
    }
}

// ------------------------------------------------------------------ lettura

/// Riceve ogni voce col suo contenuto; `Ok(false)` = basta così.
pub type Visitor<'a> = dyn FnMut(&AEntry, &mut dyn Read) -> io::Result<bool> + 'a;

/// Percorre l'archivio leggendo i contenuti (per estrarre).
pub fn walk(path: &Path, f: &mut Visitor) -> io::Result<()> {
    let fmt = format_of(&path.to_string_lossy()).ok_or_else(|| other("formato d'archivio sconosciuto"))?;
    match fmt {
        Format::Zip => {
            let mut ar = zip::ZipArchive::new(BufReader::new(File::open(path)?)).map_err(other)?;
            for i in 0..ar.len() {
                let mut z = match ar.by_index(i) {
                    Ok(z) => z,
                    Err(zip::result::ZipError::UnsupportedArchive(m)) if m.contains("assword") => {
                        return Err(password_error());
                    }
                    Err(e) => return Err(other(e)),
                };
                let Some(name) = z.name().ok().and_then(|n| clean(&n)) else { continue };
                let mut ae = zip_entry(&z, name);
                if z.is_symlink() {
                    let mut t = String::new();
                    z.read_to_string(&mut t)?;
                    ae.link = Some(t);
                    if !f(&ae, &mut io::empty())? {
                        return Ok(());
                    }
                    continue;
                }
                if !f(&ae, &mut z)? {
                    return Ok(());
                }
            }
            Ok(())
        }
        Format::Tar(c) => {
            let mut ar = tar::Archive::new(decoder(BufReader::new(File::open(path)?), c)?);
            for en in ar.entries()? {
                let mut en = en?;
                let Some(ae) = tar_entry(&mut en)? else { continue };
                if !f(&ae, &mut en)? {
                    return Ok(());
                }
            }
            Ok(())
        }
        Format::SevenZ => {
            let mut rd = sevenz_rust2::ArchiveReader::open(path, sevenz_rust2::Password::empty()).map_err(sz_error)?;
            let mut err = None;
            let r = rd.for_each_entries(|e, r| {
                let Some(mut ae) = sz_entry(e) else { return Ok(true) };
                let res = (|| {
                    if ae.link.is_some() {
                        let mut t = String::new();
                        r.read_to_string(&mut t)?;
                        ae.link = Some(t);
                        return f(&ae, &mut io::empty());
                    }
                    f(&ae, r)
                })();
                match res {
                    Ok(go) => Ok(go),
                    Err(e) => {
                        err = Some(e);
                        Ok(false)
                    }
                }
            });
            if let Some(e) = err {
                return Err(e);
            }
            r.map_err(sz_error)
        }
        Format::Single(c) => {
            let ae = single_entry(path);
            let mut r = decoder(BufReader::new(File::open(path)?), c)?;
            f(&ae, &mut r).map(|_| ())
        }
        Format::Lib => walk_lib(path, f),
    }
}

fn sz_error(e: sevenz_rust2::Error) -> io::Error {
    match e {
        sevenz_rust2::Error::PasswordRequired | sevenz_rust2::Error::MaybeBadPassword(_) => password_error(),
        e => other(e),
    }
}

fn zip_entry<R: Read>(z: &zip::read::ZipFile<'_, R>, name: String) -> AEntry {
    let mtime = z
        .last_modified()
        .map(|d| local_to_unix(d.year() as i32, d.month() as u32, d.day() as u32, d.hour() as u32, d.minute() as u32, d.second() as u32))
        .unwrap_or(0);
    AEntry {
        path: name,
        is_dir: z.is_dir(),
        size: z.size(),
        mtime,
        mode: z.unix_mode().unwrap_or(0) & 0o7777,
        link: z.is_symlink().then(String::new),
        hardlink: None,
    }
}

fn tar_entry<R: Read>(en: &mut tar::Entry<'_, R>) -> io::Result<Option<AEntry>> {
    use tar::EntryType as T;
    let t = en.header().entry_type();
    if !matches!(t, T::Regular | T::Continuous | T::Directory | T::Symlink | T::Link) {
        return Ok(None);
    }
    let Some(name) = clean(&en.path()?.to_string_lossy()) else { return Ok(None) };
    let link_name = en.link_name()?.map(|l| l.to_string_lossy().into_owned());
    let h = en.header();
    Ok(Some(AEntry {
        path: name,
        is_dir: t == T::Directory,
        size: if t.is_file() { h.size().unwrap_or(0) } else { 0 },
        mtime: h.mtime().unwrap_or(0) as i64,
        mode: h.mode().unwrap_or(0o644) & 0o7777,
        link: if t == T::Symlink { link_name.clone() } else { None },
        hardlink: if t == T::Link { link_name.and_then(|l| clean(&l)) } else { None },
    }))
}

fn sz_entry(e: &sevenz_rust2::ArchiveEntry) -> Option<AEntry> {
    let name = clean(e.name())?;
    // p7zip mette i permessi Unix nei 16 bit alti degli attributi (bit 0x8000)
    let unix = (e.has_windows_attributes && e.windows_attributes & 0x8000 != 0).then(|| e.windows_attributes >> 16);
    let is_link = unix.is_some_and(|m| m & libc::S_IFMT == libc::S_IFLNK);
    Some(AEntry {
        path: name,
        is_dir: e.is_directory(),
        size: e.size(),
        mtime: if e.has_last_modified_date { system_to_unix(e.last_modified_date().into()) } else { 0 },
        mode: unix.map(|m| m & 0o7777).unwrap_or(0),
        link: is_link.then(String::new),
        hardlink: None,
    })
}

fn single_entry(path: &Path) -> AEntry {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let stem = match name.rsplit_once('.') {
        Some((s, _)) if !s.is_empty() => s.to_string(),
        _ => format!("{name}.out"),
    };
    let meta = fs::metadata(path).ok();
    AEntry {
        path: stem,
        is_dir: false,
        size: 0, // ignota senza decomprimere
        mtime: meta.as_ref().map(|m| m.mtime()).unwrap_or(0),
        mode: meta.map(|m| m.mode() & 0o777).unwrap_or(0o644),
        link: None,
        hardlink: None,
    }
}

/// L'elenco delle voci, senza leggere i contenuti dove il formato lo permette.
fn list_uncached(path: &Path) -> io::Result<Vec<AEntry>> {
    let fmt = format_of(&path.to_string_lossy()).ok_or_else(|| other("formato d'archivio sconosciuto"))?;
    let mut out = Vec::new();
    match fmt {
        Format::Zip => {
            let mut ar = zip::ZipArchive::new(BufReader::new(File::open(path)?)).map_err(other)?;
            for i in 0..ar.len() {
                let z = ar.by_index_raw(i).map_err(other)?;
                if let Some(name) = z.name().ok().and_then(|n| clean(&n)) {
                    out.push(zip_entry(&z, name));
                }
            }
        }
        Format::SevenZ => {
            let ar = sevenz_rust2::Archive::open(path).map_err(sz_error)?;
            out.extend(ar.files.iter().filter_map(sz_entry));
        }
        Format::Single(_) => out.push(single_entry(path)),
        Format::Tar(_) | Format::Lib => walk(path, &mut |e, _| {
            out.push(e.clone());
            Ok(true)
        })?,
    }
    Ok(out)
}

type Cached = (i64, u64, Arc<Vec<AEntry>>);

/// L'elenco, tenuto in memoria per gli ultimi archivi aperti: sfogliare un
/// tar.xz da 1 GB vuol dire decomprimerlo tutto, non va rifatto a ogni cartella.
pub fn list(path: &Path) -> io::Result<Arc<Vec<AEntry>>> {
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, Cached>>> = OnceLock::new();
    let m = fs::metadata(path)?;
    let key = (m.mtime(), m.len());
    let cache = CACHE.get_or_init(Default::default);
    if let Some((t, s, l)) = cache.lock().unwrap().get(path)
        && (*t, *s) == key
    {
        return Ok(l.clone());
    }
    let l = Arc::new(list_uncached(path)?);
    let mut c = cache.lock().unwrap();
    if c.len() >= 4 {
        c.clear();
    }
    c.insert(path.to_path_buf(), (key.0, key.1, l.clone()));
    Ok(l)
}

/// Se `path` è dentro un archivio (o è l'archivio): (file dell'archivio,
/// percorso interno, vuoto per la radice).
pub fn split(path: &str) -> Option<(PathBuf, String)> {
    let p = Path::new(path);
    if p.is_dir() {
        return None;
    }
    for anc in p.ancestors() {
        match fs::metadata(anc) {
            Ok(m) if m.is_file() => {
                let name = anc.file_name()?.to_string_lossy();
                format_of(&name)?;
                let inner = p.strip_prefix(anc).ok()?.to_string_lossy().into_owned();
                return Some((anc.to_path_buf(), inner));
            }
            Ok(_) => return None, // una cartella vera: non siamo in un archivio
            Err(_) => continue,
        }
    }
    None
}

/// Il contenuto di una cartella dell'archivio, come voci di una cartella vera.
/// Le cartelle che l'archivio non elenca da sole (capita spesso) si deducono.
pub fn children(archive: &Path, inner: &str, show_hidden: bool) -> io::Result<Vec<Entry>> {
    let all = list(archive)?;
    let prefix = if inner.is_empty() { String::new() } else { format!("{inner}/") };
    if !inner.is_empty() && !all.iter().any(|e| e.path.starts_with(&prefix) || (e.path == inner && e.is_dir)) {
        return Err(io::Error::new(io::ErrorKind::NotFound, "cartella non presente nell'archivio"));
    }
    let uid = unsafe { libc::getuid() };
    let gid = unsafe { libc::getgid() };
    let arch = archive.to_string_lossy();
    let mut found: BTreeMap<String, Entry> = BTreeMap::new();
    for e in all.iter() {
        let Some(rest) = e.path.strip_prefix(&prefix) else { continue };
        let (name, deeper) = match rest.split_once('/') {
            Some((n, _)) => (n, true),
            None => (rest, false),
        };
        if name.is_empty() || (!show_hidden && name.starts_with('.')) {
            continue;
        }
        let is_dir = deeper || e.is_dir;
        if deeper && found.contains_key(name) {
            continue;
        }
        let full = format!("{prefix}{name}");
        let kind = if is_dir { fsops::KIND_DIR } else { fsops::KIND_FILE };
        let ty = if is_dir { libc::S_IFDIR } else if e.link.is_some() { libc::S_IFLNK } else { libc::S_IFREG };
        let perm = if e.mode != 0 && !deeper { e.mode & 0o777 } else if is_dir { 0o755 } else { 0o644 };
        let category = if is_dir { fsops::folder_category(name) } else { fsops::category(name) };
        found.insert(
            name.to_string(),
            Entry {
                name: name.to_string(),
                path: format!("{arch}/{full}"),
                kind,
                is_link: e.link.is_some() && !deeper,
                link_broken: false,
                link_target: if deeper { String::new() } else { e.link.clone().unwrap_or_default() },
                size: if is_dir { 0 } else { e.size },
                mtime: if deeper { 0 } else { e.mtime },
                mode: ty | perm,
                uid,
                gid,
                hidden: name.starts_with('.'),
                category: category.to_string(),
            },
        );
    }
    Ok(found.into_values().collect())
}

// ------------------------------------------------------------------ estrazione

/// Estrae in `root` le voci per cui `map` dà un percorso relativo (ripulito).
/// `progress(nome, byte)`: a ogni voce (byte 0) e a ogni blocco scritto; se
/// rende errore, l'estrazione si ferma con quello. Rende le voci saltate.
pub fn extract_into(
    archive: &Path,
    root: &Path,
    map: &dyn Fn(&str) -> Option<PathBuf>,
    progress: &mut dyn FnMut(&str, u64) -> io::Result<()>,
) -> io::Result<u64> {
    let mut links: Vec<(PathBuf, String)> = Vec::new();
    let mut hard: Vec<(PathBuf, PathBuf)> = Vec::new();
    let mut dirs: Vec<(PathBuf, u32, i64)> = Vec::new();
    let mut buf = vec![0u8; 1 << 20];
    walk(archive, &mut |e, r| {
        let Some(rel) = map(&e.path) else { return Ok(true) };
        let t = root.join(rel);
        progress(&e.path, 0)?;
        if e.is_dir {
            fs::create_dir_all(&t)?;
            dirs.push((t, e.mode, e.mtime));
        } else if let Some(l) = &e.link {
            links.push((t, l.clone()));
        } else if let Some(h) = &e.hardlink {
            if let Some(src) = map(h) {
                hard.push((t, root.join(src)));
            }
        } else {
            if let Some(p) = t.parent() {
                fs::create_dir_all(p)?;
            }
            let mut out = File::create(&t)?;
            loop {
                let n = r.read(&mut buf)?;
                if n == 0 {
                    break;
                }
                out.write_all(&buf[..n])?;
                progress(&e.path, n as u64)?;
            }
            let mode = if e.mode & 0o777 == 0 { 0o644 } else { e.mode & 0o777 };
            let _ = out.set_permissions(fs::Permissions::from_mode(mode));
            if e.mtime > 0 {
                let _ = out.set_modified(UNIX_EPOCH + Duration::from_secs(e.mtime as u64));
            }
        }
        Ok(true)
    })?;
    // i link fisici sono copie (l'originale può stare fuori dalla selezione)
    for (t, src) in hard {
        if src.is_file() {
            if let Some(p) = t.parent() {
                fs::create_dir_all(p)?;
            }
            fs::copy(&src, &t)?;
        }
    }
    // i link simbolici per ultimi: nessuna scrittura è passata attraverso di loro
    for (t, l) in links {
        if let Some(p) = t.parent() {
            fs::create_dir_all(p)?;
        }
        let _ = symlink(&l, &t);
    }
    // le cartelle: permessi (sempre apribili dal proprietario) e data, dal fondo
    for (d, mode, mtime) in dirs.into_iter().rev() {
        let mode = if mode & 0o777 == 0 { 0o755 } else { (mode & 0o777) | 0o700 };
        let _ = fs::set_permissions(&d, fs::Permissions::from_mode(mode));
        if mtime > 0
            && let Ok(f) = File::open(&d)
        {
            let _ = f.set_modified(UNIX_EPOCH + Duration::from_secs(mtime as u64));
        }
    }
    Ok(0)
}

/// Estrae una sola voce (per aprirla) in `dir`, col suo nome.
pub fn extract_one(archive: &Path, inner: &str, dir: &Path) -> io::Result<PathBuf> {
    let name = inner.rsplit('/').next().unwrap_or(inner).to_string();
    fs::create_dir_all(dir)?;
    let target = dir.join(&name);
    let want = clean(inner).ok_or_else(|| other("nome non valido"))?;
    let mut done = false;
    let w2 = want.clone();
    let n2 = name.clone();
    extract_into(archive, dir, &move |p| (p == w2).then(|| PathBuf::from(&n2)), &mut |p, _| {
        if p == want {
            done = true;
        }
        Ok(())
    })?;
    if !done || !target.exists() {
        return Err(io::Error::new(io::ErrorKind::NotFound, "voce non trovata nell'archivio"));
    }
    Ok(target)
}

// ------------------------------------------------------------------ creazione

struct Prog<'a, R> {
    r: R,
    f: &'a mut dyn FnMut(&str, u64) -> io::Result<()>,
    name: &'a str,
}

impl<R: Read> Read for Prog<'_, R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.r.read(buf)?;
        (self.f)(self.name, n as u64)?;
        Ok(n)
    }
}

/// Le voci da mettere nell'archivio: (percorso vero, nome nell'archivio).
pub fn gather(items: &[PathBuf]) -> io::Result<Vec<(PathBuf, String)>> {
    let mut out = Vec::new();
    fn rec(p: &Path, name: String, out: &mut Vec<(PathBuf, String)>) -> io::Result<()> {
        let m = fs::symlink_metadata(p)?;
        out.push((p.to_path_buf(), name.clone()));
        if m.is_dir() {
            let mut kids: Vec<_> = fs::read_dir(p)?.flatten().collect();
            kids.sort_by_key(|k| k.file_name());
            for k in kids {
                rec(&k.path(), format!("{name}/{}", k.file_name().to_string_lossy()), out)?;
            }
        }
        Ok(())
    }
    for it in items {
        let name = it.file_name().map(|n| n.to_string_lossy().into_owned()).ok_or_else(|| other("nome non valido"))?;
        rec(it, name, &mut out)?;
    }
    Ok(out)
}

/// Crea l'archivio `dest` (formato dal nome) con `items`, ognuno col suo nome
/// in cima all'archivio. Scrive direttamente `dest`: il chiamante passa un
/// nome temporaneo e rinomina a lavoro finito.
pub fn create(
    dest: &Path,
    fmt: Format,
    items: &[PathBuf],
    progress: &mut dyn FnMut(&str, u64) -> io::Result<()>,
) -> io::Result<()> {
    let list = gather(items)?;
    match fmt {
        Format::Zip => {
            let mut z = zip::ZipWriter::new(BufWriter::new(File::create(dest)?));
            for (p, name) in &list {
                progress(name, 0)?;
                let m = fs::symlink_metadata(p)?;
                let opts = zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Deflated)
                    .unix_permissions(m.mode() & 0o7777)
                    .last_modified_time(unix_to_zip(m.mtime()))
                    .large_file(m.len() >= 0xFFFF_FFFF);
                if m.is_dir() {
                    z.add_directory(format!("{name}/"), opts).map_err(other)?;
                } else if m.file_type().is_symlink() {
                    let t = fs::read_link(p)?;
                    z.add_symlink(name.clone(), t.to_string_lossy(), opts).map_err(other)?;
                } else if m.is_file() {
                    z.start_file(name.clone(), opts).map_err(other)?;
                    io::copy(&mut Prog { r: File::open(p)?, f: &mut *progress, name }, &mut z)?;
                }
            }
            z.finish().map_err(other)?.flush()?;
        }
        Format::Tar(c) => {
            let enc = Encoder::new(BufWriter::new(File::create(dest)?), c)?;
            let mut b = tar::Builder::new(enc);
            b.follow_symlinks(false);
            for (p, name) in &list {
                progress(name, 0)?;
                let m = fs::symlink_metadata(p)?;
                let mut h = tar::Header::new_gnu();
                h.set_metadata(&m);
                if m.is_dir() {
                    h.set_size(0);
                    b.append_data(&mut h, name, io::empty())?;
                } else if m.file_type().is_symlink() {
                    h.set_size(0);
                    b.append_link(&mut h, name, fs::read_link(p)?)?;
                } else if m.is_file() {
                    b.append_data(&mut h, name, Prog { r: File::open(p)?, f: &mut *progress, name })?;
                }
            }
            b.into_inner()?.finish()?.flush()?;
        }
        Format::SevenZ => {
            let mut w = sevenz_rust2::ArchiveWriter::create(dest).map_err(sz_error)?;
            for (p, name) in &list {
                progress(name, 0)?;
                let m = fs::symlink_metadata(p)?;
                let mut e = sevenz_rust2::ArchiveEntry::from_path(p, name.clone());
                // come p7zip: i permessi Unix nei 16 bit alti degli attributi
                e.has_windows_attributes = true;
                e.windows_attributes = (e.windows_attributes & 0xFFFF) | 0x8000 | ((m.mode() & 0xFFFF) << 16);
                if m.is_dir() {
                    w.push_archive_entry::<&[u8]>(e, None).map_err(sz_error)?;
                } else if m.is_file() {
                    w.push_archive_entry(e, Some(Prog { r: File::open(p)?, f: &mut *progress, name })).map_err(sz_error)?;
                }
                // i link simbolici in 7z non sono portabili: si saltano
            }
            w.finish()?;
        }
        _ => return Err(other("questo formato si può solo estrarre")),
    }
    Ok(())
}
