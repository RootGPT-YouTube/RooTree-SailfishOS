//! Operazioni immediate sul filesystem locale: elenco, informazioni, memorie,
//! nuova cartella, rinomina, permessi. Quelle lunghe stanno in `jobs`.

use std::cmp::Ordering;
use std::ffi::CString;
use std::fs;
use std::io;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};

use crate::ffi::{Entry, Place};

pub const KIND_FILE: u8 = 0;
pub const KIND_DIR: u8 = 1;
pub const KIND_OTHER: u8 = 2;

#[allow(dead_code)] // il valore predefinito: list_dir lo tratta nel ramo `_`
pub const SORT_NAME: u8 = 0;
pub const SORT_SIZE: u8 = 1;
pub const SORT_DATE: u8 = 2;
pub const SORT_TYPE: u8 = 3;

/// L'icona di un file dal suo nome: una categoria, non un tipo MIME (quello
/// servirà ad «Apri con», nella fase 2).
pub fn category(name: &str) -> &'static str {
    let ext = match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => ext.to_ascii_lowercase(),
        _ => return "file",
    };
    match ext.as_str() {
        "jpg" | "jpeg" | "png" | "gif" | "webp" | "bmp" | "svg" | "heic" | "heif" | "tif" | "tiff" | "ico" => "image",
        "mp3" | "ogg" | "oga" | "opus" | "flac" | "wav" | "m4a" | "aac" | "wma" | "amr" => "audio",
        "mp4" | "mkv" | "webm" | "avi" | "mov" | "m4v" | "3gp" | "wmv" | "mpg" | "mpeg" => "video",
        "pdf" => "pdf",
        "zip" | "tar" | "gz" | "tgz" | "xz" | "txz" | "bz2" | "tbz2" | "zst" | "7z" | "rar" | "lz" | "lzma" | "cpio" | "rpm" | "deb" => "archive",
        "txt" | "md" | "log" | "csv" | "ini" | "conf" | "cfg" | "nfo" | "srt" | "vtt" => "text",
        "c" | "h" | "cpp" | "hpp" | "cc" | "rs" | "py" | "js" | "ts" | "qml" | "java" | "kt" | "go" | "sh" | "json"
        | "xml" | "html" | "htm" | "css" | "yaml" | "yml" | "toml" | "spec" | "desktop" | "pro" | "cmake" => "code",
        "doc" | "docx" | "odt" | "rtf" => "document",
        "xls" | "xlsx" | "ods" => "spreadsheet",
        "ppt" | "pptx" | "odp" => "presentation",
        "apk" | "aab" => "package",
        _ => "file",
    }
}

/// Le cartelle «speciali» hanno il loro simbolo inciso, come in Dolphin.
pub fn folder_category(name: &str) -> &'static str {
    match name.to_lowercase().as_str() {
        "documents" | "documenti" => "folder-documents",
        "downloads" | "download" => "folder-download",
        "pictures" | "immagini" | "foto" | "photos" => "folder-pictures",
        "dcim" | "camera" => "folder-camera",
        "music" | "musica" => "folder-music",
        "videos" | "movies" | "video" => "folder-videos",
        "desktop" | "scrivania" => "folder-desktop",
        "android_storage" | "android" => "folder-android",
        _ => "folder",
    }
}

pub fn entry_for(path: &Path, name: String) -> Entry {
    // symlink_metadata: il link in sé; metadata: quello a cui punta. Un link a
    // una cartella si apre come una cartella; un link rotto resta un file.
    let lmeta = fs::symlink_metadata(path);
    let is_link = lmeta.as_ref().map(|m| m.file_type().is_symlink()).unwrap_or(false);
    let meta = if is_link { fs::metadata(path) } else { fs::symlink_metadata(path) };
    let link_broken = is_link && meta.is_err();
    let link_target = if is_link {
        fs::read_link(path).map(|p| p.to_string_lossy().into_owned()).unwrap_or_default()
    } else {
        String::new()
    };
    let m = meta.ok().or_else(|| lmeta.ok());
    let (kind, size, mtime, mode, uid, gid) = match &m {
        Some(m) => {
            let kind = if m.is_dir() {
                KIND_DIR
            } else if m.is_file() || m.file_type().is_symlink() {
                KIND_FILE
            } else {
                KIND_OTHER
            };
            (kind, if m.is_dir() { 0 } else { m.len() }, m.mtime(), m.mode(), m.uid(), m.gid())
        }
        None => (KIND_FILE, 0, 0, 0, 0, 0),
    };
    let category = if kind == KIND_DIR { folder_category(&name) } else { category(&name) };
    Entry {
        hidden: name.starts_with('.'),
        path: path.to_string_lossy().into_owned(),
        name,
        kind,
        is_link,
        link_broken,
        link_target,
        size,
        mtime,
        mode,
        uid,
        gid,
        category: category.to_string(),
    }
}

/// Confronto «naturale» senza distinguere le maiuscole: «file2» prima di «file10».
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    let (mut a, mut b) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (a.peek().copied(), b.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, _) => return Ordering::Less,
            (_, None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let mut na = String::new();
                while let Some(c) = a.peek().copied().filter(|c| c.is_ascii_digit()) {
                    na.push(c);
                    a.next();
                }
                let mut nb = String::new();
                while let Some(c) = b.peek().copied().filter(|c| c.is_ascii_digit()) {
                    nb.push(c);
                    b.next();
                }
                let (ta, tb) = (na.trim_start_matches('0'), nb.trim_start_matches('0'));
                let ord = ta.len().cmp(&tb.len()).then_with(|| ta.cmp(tb)).then_with(|| na.len().cmp(&nb.len()));
                if ord != Ordering::Equal {
                    return ord;
                }
            }
            (Some(x), Some(y)) => {
                let ord = x.to_lowercase().cmp(y.to_lowercase());
                if ord != Ordering::Equal {
                    return ord;
                }
                a.next();
                b.next();
            }
        }
    }
}

fn extension(name: &str) -> String {
    match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => ext.to_lowercase(),
        _ => String::new(),
    }
}

pub fn list_dir(path: &str, show_hidden: bool, sort_key: u8, descending: bool, dirs_first: bool) -> io::Result<Vec<Entry>> {
    let mut entries = Vec::new();
    match fs::read_dir(path) {
        Ok(rd) => {
            for item in rd {
                let item = item?;
                let name = item.file_name().to_string_lossy().into_owned();
                if !show_hidden && name.starts_with('.') {
                    continue;
                }
                entries.push(entry_for(&item.path(), name));
            }
        }
        // dentro un archivio: si sfoglia come una cartella
        Err(e) => match crate::archive::split(path) {
            Some((archive, inner)) => entries = crate::archive::children(&archive, &inner, show_hidden)?,
            None => return Err(e),
        },
    }
    sort_entries(&mut entries, sort_key, descending, dirs_first);
    Ok(entries)
}

/// L'ordine di una cartella. Per data l'ordine «normale» è dal più recente
/// (scelta dell'utente dell'08/11: «dai più vecchi ai più nuovi» non serve
/// quasi mai); «inverso» rovescia quello normale, per ogni criterio.
pub fn sort_entries(entries: &mut [Entry], sort_key: u8, descending: bool, dirs_first: bool) {
    entries.sort_by(|a, b| {
        let dirs = if dirs_first { (b.kind == KIND_DIR).cmp(&(a.kind == KIND_DIR)) } else { Ordering::Equal };
        let key = match sort_key {
            SORT_SIZE => a.size.cmp(&b.size),
            SORT_DATE => b.mtime.cmp(&a.mtime),
            SORT_TYPE => extension(&a.name).cmp(&extension(&b.name)),
            _ => Ordering::Equal,
        };
        let key = key.then_with(|| natural_cmp(&a.name, &b.name));
        // le cartelle restano in cima anche in ordine inverso, come in Dolphin
        dirs.then(if descending { key.reverse() } else { key })
    });
}

pub fn stat_path(path: &str) -> io::Result<Entry> {
    let p = Path::new(path);
    if let Err(e) = fs::symlink_metadata(p) {
        // una voce di un archivio: la si cerca fra quelle della sua cartella
        let parent = p.parent().map(|x| x.to_string_lossy().into_owned()).unwrap_or_default();
        return crate::archive::split(&parent)
            .and_then(|(a, inner)| crate::archive::children(&a, &inner, true).ok())
            .and_then(|v| v.into_iter().find(|x| x.path == path))
            .ok_or(e);
    }
    let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "/".into());
    Ok(entry_for(p, name))
}

/// (totale, libero) in byte del filesystem che contiene `path`.
pub fn space(path: &Path) -> Option<(u64, u64)> {
    let c = CString::new(path.as_os_str().as_encoded_bytes()).ok()?;
    let mut s: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(c.as_ptr(), &mut s) } != 0 {
        return None;
    }
    let f = s.f_frsize as u64;
    Some((s.f_blocks as u64 * f, s.f_bavail as u64 * f))
}

pub const PLACE_INTERNAL: u8 = 0;
pub const PLACE_ANDROID: u8 = 1;
pub const PLACE_SDCARD: u8 = 2;
pub const PLACE_USB: u8 = 3;
pub const PLACE_ROOT: u8 = 4;

fn place(kind: u8, label: &str, path: &Path) -> Place {
    let (total, free) = space(path).unwrap_or((0, 0));
    Place { kind, label: label.into(), path: path.to_string_lossy().into_owned(), total, free }
}

/// Le memorie locali: interna (la home), Android, le removibili montate
/// (scheda SD, USB) e il filesystem dalla radice.
pub fn places(home: &str) -> Vec<Place> {
    let home = Path::new(home);
    let mut out = vec![place(PLACE_INTERNAL, "", home)];
    // AppSupport di Sailfish OS 4+: la memoria vista dalle app Android
    let android = home.join("android_storage");
    if android.is_dir() {
        out.push(place(PLACE_ANDROID, "", &android));
    }
    if let Ok(mounts) = fs::read_to_string("/proc/mounts") {
        for line in mounts.lines() {
            let mut f = line.split_whitespace();
            let (Some(dev), Some(dir)) = (f.next(), f.next()) else { continue };
            // /proc/mounts scrive gli spazi come \040
            let dir = dir.replace("\\040", " ");
            if !(dir.starts_with("/run/media/") || dir.starts_with("/media/")) {
                continue;
            }
            let kind = if dev.contains("mmcblk") { PLACE_SDCARD } else { PLACE_USB };
            let p = PathBuf::from(&dir);
            let label = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            out.push(place(kind, &label, &p));
        }
    }
    out.push(place(PLACE_ROOT, "", Path::new("/")));
    out
}

pub fn valid_name(name: &str) -> io::Result<()> {
    if name.is_empty() || name == "." || name == ".." || name.contains('/') || name.contains('\0') {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "nome non valido"));
    }
    Ok(())
}

pub fn make_dir(parent: &str, name: &str) -> io::Result<String> {
    valid_name(name)?;
    let p = Path::new(parent).join(name);
    fs::create_dir(&p)?;
    adopt_tree(&p);
    Ok(p.to_string_lossy().into_owned())
}

pub fn rename_entry(path: &str, new_name: &str) -> io::Result<String> {
    valid_name(new_name)?;
    let from = Path::new(path);
    let to = from.with_file_name(new_name);
    if to == from {
        return Ok(path.into());
    }
    // rename() sovrascriverebbe in silenzio un file con quel nome
    if fs::symlink_metadata(&to).is_ok() {
        return Err(io::Error::new(io::ErrorKind::AlreadyExists, "esiste già un elemento con questo nome"));
    }
    fs::rename(from, &to)?;
    Ok(to.to_string_lossy().into_owned())
}

pub fn set_mode(path: &str, mode: u32) -> io::Result<()> {
    fs::set_permissions(path, fs::Permissions::from_mode(mode & 0o7777))
}

fn name_of(id: u32, file: &str) -> String {
    // /etc/passwd e /etc/group: nome:x:id:...
    fs::read_to_string(file)
        .ok()
        .and_then(|s| {
            s.lines().find_map(|l| {
                let f: Vec<&str> = l.split(':').collect();
                (f.len() > 2 && f[2] == id.to_string()).then(|| f[0].to_string())
            })
        })
        .unwrap_or_else(|| id.to_string())
}

pub fn user_name(uid: u32) -> String {
    name_of(uid, "/etc/passwd")
}

pub fn group_name(gid: u32) -> String {
    name_of(gid, "/etc/group")
}

// ------------------------------------------------------------------ Fase 4: root

use std::sync::atomic::{AtomicBool, Ordering as AtOrd};

/// Acceso nell'helper root: quello che crea in una cartella che non è di root
/// (la home, la scheda SD) prende il proprietario di quella cartella, invece
/// di restare di root e diventare intoccabile per l'utente.
pub static INHERIT_OWNER: AtomicBool = AtomicBool::new(false);

fn lchown(path: &Path, uid: u32, gid: u32) -> io::Result<()> {
    let c = CString::new(path.as_os_str().as_encoded_bytes()).map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?;
    if unsafe { libc::lchown(c.as_ptr(), uid, gid) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// Vedi INHERIT_OWNER: `path` (e tutto quello che contiene) al proprietario
/// della cartella che lo contiene, se quella non è di root.
pub fn adopt_tree(path: &Path) {
    if !INHERIT_OWNER.load(AtOrd::Relaxed) {
        return;
    }
    let Some(parent) = path.parent() else { return };
    let Ok(pm) = fs::metadata(parent) else { return };
    if pm.uid() == 0 {
        return;
    }
    fn rec(p: &Path, uid: u32, gid: u32) {
        let _ = lchown(p, uid, gid);
        if fs::symlink_metadata(p).map(|m| m.is_dir()).unwrap_or(false)
            && let Ok(rd) = fs::read_dir(p)
        {
            for e in rd.flatten() {
                rec(&e.path(), uid, gid);
            }
        }
    }
    rec(path, pm.uid(), pm.gid());
}

fn id_of(name: &str, file: &str) -> io::Result<u32> {
    if let Ok(n) = name.parse::<u32>() {
        return Ok(n);
    }
    fs::read_to_string(file)?
        .lines()
        .find_map(|l| {
            let f: Vec<&str> = l.split(':').collect();
            (f.len() > 2 && f[0] == name).then(|| f[2].parse().ok()).flatten()
        })
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, format!("«{name}» non esiste")))
}

/// Proprietario e gruppo, per nome o numero; vuoto = non cambiare.
pub fn set_owner(path: &str, user: &str, group: &str) -> io::Result<()> {
    let m = fs::symlink_metadata(path)?;
    let uid = if user.is_empty() { m.uid() } else { id_of(user, "/etc/passwd")? };
    let gid = if group.is_empty() { m.gid() } else { id_of(group, "/etc/group")? };
    lchown(Path::new(path), uid, gid)
}

/// Dove l'helper root mette le copie di sicurezza dei file che salva: fuori
/// dalla home (regola dell'utente del 26/10), di root e 700 (possono essere
/// file sensibili, come /etc/shadow). Una sola copia per file, la più recente.
pub const BACKUP_DIR: &str = "/home/debugging/harbour-rootree/copie-di-sicurezza";

#[cfg(test)]
pub static TEST_BACKUP_DIR: std::sync::Mutex<Option<PathBuf>> = std::sync::Mutex::new(None);

fn backup_dir() -> PathBuf {
    #[cfg(test)]
    if let Some(d) = TEST_BACKUP_DIR.lock().unwrap().clone() {
        return d;
    }
    PathBuf::from(BACKUP_DIR)
}

/// Salva il testo in `path` senza mai lasciarlo a metà: scrive accanto e
/// rinomina, tenendo permessi e proprietario dell'originale. Con `backup`,
/// prima copia l'originale in BACKUP_DIR. Rende il percorso della copia ("" se
/// non fatta).
pub fn write_text(path: &str, text: &str, backup: bool) -> io::Result<String> {
    use std::io::Write;
    let p = Path::new(path);
    let name = p.file_name().ok_or_else(|| io::Error::from(io::ErrorKind::InvalidInput))?.to_string_lossy().into_owned();
    let old = fs::metadata(p).ok();
    if old.as_ref().is_some_and(|m| !m.is_file()) {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "non è un file normale"));
    }
    let mut copy = String::new();
    if backup && old.is_some() {
        let dir_buf = backup_dir();
        let dir = dir_buf.as_path();
        fs::create_dir_all(dir)?;
        let _ = fs::set_permissions(dir.parent().unwrap(), fs::Permissions::from_mode(0o700));
        let _ = fs::set_permissions(dir, fs::Permissions::from_mode(0o700));
        let real = fs::canonicalize(p).unwrap_or(p.to_path_buf());
        let b = dir.join(real.to_string_lossy().trim_start_matches('/').replace('/', "%"));
        fs::copy(p, &b)?;
        copy = b.to_string_lossy().into_owned();
    }
    // un link si salva sul file vero, non al suo posto
    let target = fs::canonicalize(p).unwrap_or(p.to_path_buf());
    let tmp = target.with_file_name(format!(".{name}.rootree-part"));
    let r = (|| -> io::Result<()> {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(text.as_bytes())?;
        if let Some(m) = &old {
            f.set_permissions(fs::Permissions::from_mode(m.mode() & 0o7777))?;
            let _ = lchown(&tmp, m.uid(), m.gid());
        }
        f.sync_all()?;
        fs::rename(&tmp, &target)
    })();
    if r.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    r?;
    if old.is_none() {
        adopt_tree(&target);
    }
    Ok(copy)
}
