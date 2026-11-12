//! Le operazioni di base sui file, locali o remote (Fase 5, step 3): ognuna
//! guarda il percorso; `/@remote/<id>/…` va via SFTP (remote.rs), il resto
//! è il filesystem locale, esattamente come prima. Il motore delle operazioni
//! (jobs.rs) le usa al posto di std::fs, così copia, spostamento ed
//! eliminazione funzionano fra telefono e server con la stessa coda.

use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::os::unix::fs::{MetadataExt, PermissionsExt, symlink as local_symlink};
use std::path::{Path, PathBuf};
use std::time::{Duration, UNIX_EPOCH};

use crate::remote;

pub const EXDEV: i32 = 18;

/// I dati di una voce che servono al motore (al posto di fs::Metadata).
#[derive(Clone, Copy, Debug)]
pub struct Meta {
    pub is_dir: bool,
    pub is_file: bool,
    pub is_symlink: bool,
    pub len: u64,
    pub mode: u32,
    pub mtime: i64,
    /// per capire se due percorsi stanno sullo stesso filesystem
    pub dev: u64,
}

fn rsplit(p: &Path) -> Option<(String, String)> {
    remote::split(&p.to_string_lossy())
}

fn dev_of(id: &str) -> u64 {
    // un numero per connessione, lontano dai dev locali
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    id.hash(&mut h);
    h.finish() | (1 << 63)
}

fn from_local(m: &fs::Metadata) -> Meta {
    Meta {
        is_dir: m.is_dir(),
        is_file: m.is_file(),
        is_symlink: m.file_type().is_symlink(),
        len: m.len(),
        mode: m.mode(),
        mtime: m.mtime(),
        dev: m.dev(),
    }
}

/// Come symlink_metadata: un link resta un link.
pub fn lmeta(p: &Path) -> io::Result<Meta> {
    match rsplit(p) {
        Some((id, inner)) => {
            let st = remote::lstat(&id, &inner)?;
            Ok(Meta { is_dir: st.is_dir, is_file: st.is_file, is_symlink: st.is_link, len: st.size, mode: st.mode, mtime: st.mtime, dev: dev_of(&id) })
        }
        None => fs::symlink_metadata(p).map(|m| from_local(&m)),
    }
}

/// Come metadata: segue i link.
pub fn meta(p: &Path) -> io::Result<Meta> {
    match rsplit(p) {
        Some((id, inner)) => {
            let st = remote::stat_follow(&id, &inner)?;
            Ok(Meta { is_dir: st.is_dir, is_file: st.is_file, is_symlink: false, len: st.size, mode: st.mode, mtime: st.mtime, dev: dev_of(&id) })
        }
        None => fs::metadata(p).map(|m| from_local(&m)),
    }
}

pub fn exists(p: &Path) -> bool {
    lmeta(p).is_ok()
}

pub fn is_dir(p: &Path) -> bool {
    meta(p).map(|m| m.is_dir).unwrap_or(false)
}

/// I figli di una cartella (percorsi completi, anche remoti).
pub fn read_dir(p: &Path) -> io::Result<Vec<PathBuf>> {
    match rsplit(p) {
        Some((id, inner)) => {
            Ok(remote::readdir(&id, &inner)?.into_iter().map(|(name, _)| p.join(name)).collect())
        }
        None => Ok(fs::read_dir(p)?.flatten().map(|e| e.path()).collect()),
    }
}

/// I figli con i loro dati: su un server basta una richiesta per cartella,
/// invece di una per ogni voce.
pub fn read_dir_meta(p: &Path) -> io::Result<Vec<(PathBuf, Meta)>> {
    match rsplit(p) {
        Some((id, inner)) => {
            let dev = dev_of(&id);
            Ok(remote::readdir(&id, &inner)?
                .into_iter()
                .map(|(name, st)| {
                    (p.join(name), Meta { is_dir: st.is_dir, is_file: st.is_file, is_symlink: st.is_link, len: st.size, mode: st.mode, mtime: st.mtime, dev })
                })
                .collect())
        }
        None => Ok(fs::read_dir(p)?
            .flatten()
            .filter_map(|e| {
                let path = e.path();
                let m = fs::symlink_metadata(&path).ok()?;
                Some((path, from_local(&m)))
            })
            .collect()),
    }
}

/// Un percorso su WebDAV o su un cloud: una cartella si elimina con una sola richiesta.
pub fn is_cloud(p: &Path) -> bool {
    rsplit(p).map(|(id, _)| remote::is_cloud(&id)).unwrap_or(false)
}

pub fn open_read(p: &Path) -> io::Result<Box<dyn Read + Send>> {
    match rsplit(p) {
        Some((id, inner)) => remote::open_any(&id, &inner),
        None => Ok(Box::new(File::open(p)?)),
    }
}

/// Un file in scrittura, con il «scrivi davvero» per il motore.
pub trait Sink: Write + Send {
    /// i dati sul supporto (locale: fsync; remoto: fsync@openssh se c'è)
    fn sync(&mut self) -> io::Result<()>;
}

impl Sink for File {
    fn sync(&mut self) -> io::Result<()> {
        self.sync_data()
    }
}

impl Sink for ssh2::File {
    fn sync(&mut self) -> io::Result<()> {
        // non tutti i server hanno l'estensione: non è un errore
        let _ = self.fsync();
        Ok(())
    }
}

/// `size`: la dimensione finale se si sa (serve a chi carica a pezzi e vuole il totale prima).
pub fn create(p: &Path, mode: u32, size: Option<u64>) -> io::Result<Box<dyn Sink>> {
    match rsplit(p) {
        Some((id, inner)) => remote::create_any(&id, &inner, mode, size),
        None => {
            let f = File::create(p)?;
            Ok(Box::new(f))
        }
    }
}

pub fn create_dir(p: &Path) -> io::Result<()> {
    match rsplit(p) {
        Some((id, inner)) => remote::mkdir_any(&id, &inner),
        None => fs::create_dir(p),
    }
}

/// Rinomina sullo stesso filesystem; fra filesystem diversi (o fra telefono
/// e server, o fra due server) rende EXDEV, e il motore copia.
pub fn rename(from: &Path, to: &Path) -> io::Result<()> {
    match (rsplit(from), rsplit(to)) {
        (None, None) => fs::rename(from, to),
        // SFTP v3 (e i cloud) non sovrascrivono: rename_any toglie prima il file
        (Some((a, fi)), Some((b, ti))) if a == b => remote::rename_any(&a, &fi, &ti),
        _ => Err(io::Error::from_raw_os_error(EXDEV)),
    }
}

pub fn remove_file(p: &Path) -> io::Result<()> {
    match rsplit(p) {
        Some((id, inner)) => remote::unlink_any(&id, &inner),
        None => fs::remove_file(p),
    }
}

/// Eliminazione definitiva su un cloud (Nextcloud e Mega altrimenti mettono nel cestino).
pub fn remove_forever(p: &Path) -> io::Result<()> {
    match rsplit(p) {
        Some((id, inner)) => remote::delete_forever_any(&id, &inner),
        None => remove_all(p),
    }
}

pub fn remove_dir(p: &Path) -> io::Result<()> {
    match rsplit(p) {
        Some((id, inner)) => remote::rmdir_any(&id, &inner),
        None => fs::remove_dir(p),
    }
}

/// Tutto, anche una cartella piena (per «sovrascrivi» e dopo uno spostamento).
pub fn remove_all(p: &Path) -> io::Result<()> {
    if rsplit(p).is_none() {
        return if fs::symlink_metadata(p)?.is_dir() { fs::remove_dir_all(p) } else { fs::remove_file(p) };
    }
    if is_cloud(p) {
        return remove_file(p);
    }
    let m = lmeta(p)?;
    if m.is_dir {
        for c in read_dir(p)? {
            remove_all(&c)?;
        }
        remove_dir(p)
    } else {
        remove_file(p)
    }
}

pub fn set_mode(p: &Path, mode: u32) -> io::Result<()> {
    match rsplit(p) {
        Some((id, inner)) => remote::set_stat_any(&id, &inner, Some(mode & 0o7777), None),
        None => fs::set_permissions(p, fs::Permissions::from_mode(mode & 0o7777)),
    }
}

pub fn set_mtime(p: &Path, mtime: i64) -> io::Result<()> {
    if mtime <= 0 {
        return Ok(());
    }
    match rsplit(p) {
        Some((id, inner)) => remote::set_stat_any(&id, &inner, None, Some(mtime as u64)),
        None => File::open(p)?.set_modified(UNIX_EPOCH + Duration::from_secs(mtime as u64)),
    }
}

pub fn read_link(p: &Path) -> io::Result<PathBuf> {
    match rsplit(p) {
        Some((id, inner)) => remote::readlink_any(&id, &inner),
        None => fs::read_link(p),
    }
}

/// Un link simbolico in `link` che punta a `target`.
pub fn symlink(target: &Path, link: &Path) -> io::Result<()> {
    match rsplit(link) {
        // ⛔ OpenSSH scambia gli argomenti di SSH_FXP_SYMLINK rispetto alla
        //    bozza del protocollo. Visto sul NAS (OpenSSH) l'08/11:
        //    `s.symlink(percorso_del_link, "uno.txt")` ha creato, relativo alla
        //    home sul server, un link «uno.txt» → percorso_del_link (finito
        //    nella home dell'utente, poi tolto). Quindi con OpenSSH: prima la
        //    destinazione, poi il percorso del link.
        Some((id, inner)) => remote::symlink_any(&id, target, &inner),
        None => local_symlink(target, link),
    }
}

pub fn same_device(a: &Path, b: &Path) -> bool {
    match (lmeta(a), meta(b)) {
        (Ok(x), Ok(y)) => x.dev == y.dev,
        _ => false,
    }
}

/// Il file a metà accanto a `target` (.nome.rootree-part), anche remoto.
pub fn part_of(target: &Path) -> PathBuf {
    let name = target.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    target.with_file_name(format!(".{name}.rootree-part"))
}
