//! Il cestino, in ogni memoria (richiesta dell'utente del 12/11/2026).
//!
//! Formato: quello di freedesktop.org (lo usano Dolphin, Nautilus, GIO):
//! `<cestino>/files/<nome>` e `<cestino>/info/<nome>.trashinfo` con
//! `Path=` (percorso originale, codificato) e `DeletionDate=` (ora locale).
//! Spostare nel cestino è un rinomina: istantaneo anche per una cartella enorme.
//!
//! Dove sta il cestino:
//! - home dell'utente: `~/.local/share/Trash` (lo stesso di Dolphin);
//! - un altro filesystem (memoria Android, che è un montaggio a parte anche se
//!   sta sullo stesso disco, scheda SD, USB): `<cima del montaggio>/.Trash-<uid>`;
//! - modalità root (helper, uid 0): `<cima del montaggio>/.Trash-0`, di root
//!   e 700 (scelta dell'utente del 12/11);
//! - server senza cestino proprio (SFTP, FTP/FTPS, SMB, WebDAV generico):
//!   `.Trash-rootree` nella cartella iniziale della connessione, per SMB in
//!   cima a ogni condivisione (scelta dell'utente del 12/11);
//! - Nextcloud, Mega e Dropbox: il cestino del servizio (`Drive::trash`).
//!
//! Le voci più vecchie di `days()` giorni (30, regolabili; 0 = mai) si tolgono
//! da sole, solo nei cestini che gestisce RooTree: quelli dei servizi hanno le
//! loro regole.

use std::collections::HashMap;
use std::fs;
use std::io::{self, Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Mutex, OnceLock};

use serde_json::{Value, json};

use crate::remote;
use crate::vfs;

/// Il nome del cestino sui server: l'uid dell'utente sul server non si sa
/// (FTP, SMB, WebDAV non lo dicono), quindi un nome solo per tutti.
pub const REMOTE_DIR: &str = ".Trash-rootree";

/// Errore «qui il cestino non si può fare»: l'interfaccia propone il definitivo.
pub const NO_TRASH: &str = "NOTRASH:";

static DAYS: AtomicU32 = AtomicU32::new(30);

pub fn set_days(d: u32) {
    DAYS.store(d, Ordering::Relaxed);
}

pub fn days() -> u32 {
    DAYS.load(Ordering::Relaxed)
}

fn no_trash(why: &str) -> io::Error {
    io::Error::other(format!("{NO_TRASH}{why}"))
}

// ------------------------------------------------------------------ connessioni

/// Tipo e cartella iniziale di ogni connessione aperta (da remote_connect).
fn conns() -> &'static Mutex<HashMap<String, (String, String)>> {
    static C: OnceLock<Mutex<HashMap<String, (String, String)>>> = OnceLock::new();
    C.get_or_init(Default::default)
}

pub fn remember(id: &str, kind: &str, start: &str) {
    conns().lock().unwrap().insert(id.into(), (kind.into(), start.into()));
}

fn conn_of(id: &str) -> Option<(String, String)> {
    conns().lock().unwrap().get(id).cloned()
}

/// Nextcloud, Mega, Dropbox e kDrive hanno un cestino loro: eliminare = cestino del servizio.
pub fn service_kind(kind: &str) -> bool {
    matches!(kind, "nextcloud" | "mega" | "dropbox" | "kdrive")
}

/// Il tipo della connessione `id` ("" se non è aperta).
pub fn kind_of(id: &str) -> String {
    conn_of(id).map(|c| c.0).unwrap_or_default()
}

// ------------------------------------------------------------------ dove

fn uid() -> u32 {
    unsafe { libc::geteuid() }
}

fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/home/defaultuser"))
}

fn home_trash() -> PathBuf {
    match std::env::var_os("XDG_DATA_HOME") {
        Some(d) if !d.is_empty() => PathBuf::from(d).join("Trash"),
        _ => home().join(".local/share/Trash"),
    }
}

fn unescape_mount(s: &str) -> String {
    // mountinfo scrive spazi e simili come \040
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'\\' && i + 4 <= b.len() {
            if let Some(v) = std::str::from_utf8(&b[i + 1..i + 4]).ok().and_then(|x| u8::from_str_radix(x, 8).ok()) {
                out.push(v);
                i += 4;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn mount_points() -> Vec<PathBuf> {
    fs::read_to_string("/proc/self/mountinfo")
        .unwrap_or_default()
        .lines()
        .filter_map(|l| l.split(' ').nth(4).map(|m| PathBuf::from(unescape_mount(m))))
        .collect()
}

/// La cima del montaggio che contiene `p` (il percorso più lungo che lo
/// precede). ⛔ La memoria Android è un montaggio «bind» della stessa /home:
/// stesso st_dev, ma rename() fra i due rende EXDEV, quindi si guarda il
/// montaggio, non il dispositivo.
pub fn mount_of(p: &Path) -> PathBuf {
    let real = real_path(p);
    mount_points()
        .into_iter()
        .filter(|m| real.starts_with(m))
        .max_by_key(|m| m.as_os_str().len())
        .unwrap_or_else(|| PathBuf::from("/"))
}

/// Il percorso vero (link risolti) della cartella che contiene `p`, più il nome:
/// un link si mette nel cestino come link, non ciò a cui punta.
fn real_path(p: &Path) -> PathBuf {
    match (p.parent(), p.file_name()) {
        (Some(dir), Some(name)) => fs::canonicalize(dir).map(|d| d.join(name)).unwrap_or_else(|_| p.to_path_buf()),
        _ => fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf()),
    }
}

fn is_trash_name(n: &str) -> bool {
    n.starts_with(".Trash-") || n == ".Trash"
}

/// `p` è un cestino o ci sta dentro.
pub fn inside_trash(p: &Path) -> bool {
    let s = p.to_string_lossy();
    if remote::split(&s).is_none() {
        let ht = home_trash();
        if p.starts_with(&ht) || real_path(p).starts_with(&ht) {
            return true;
        }
    }
    p.components().any(|c| is_trash_name(&c.as_os_str().to_string_lossy()))
}

/// Il cestino locale per `p` (cartella di un montaggio, o quello della home).
fn local_trash_for(p: &Path) -> PathBuf {
    let top = mount_of(p);
    // come dice la specifica: lo stesso montaggio della home va nel cestino
    // della home, anche fuori dalla home (es. /home/debugging)
    if uid() != 0 && top == mount_of(&home()) {
        return home_trash();
    }
    top.join(format!(".Trash-{}", uid()))
}

/// Il cestino di un server senza cestino proprio, per `inner` (percorso sul server).
fn remote_trash_for(id: &str, inner: &str) -> io::Result<String> {
    let (kind, start) = conn_of(id).ok_or_else(|| no_trash("la connessione non è aperta"))?;
    if service_kind(&kind) {
        return Err(no_trash("cestino del servizio"));
    }
    let base = if kind == "smb" || kind == "s3-buckets" {
        let share = inner.trim_start_matches('/').split('/').next().unwrap_or("");
        if share.is_empty() || inner.trim_matches('/') == share {
            return Err(no_trash(if kind == "smb" { "una condivisione non si mette nel cestino" } else { "un bucket non si mette nel cestino" }));
        }
        format!("/{share}")
    } else {
        let s = start.trim_end_matches('/');
        if s.is_empty() { "/".to_string() } else { s.to_string() }
    };
    let under = if base == "/" { inner != "/" } else { inner.starts_with(&format!("{base}/")) };
    if !under {
        return Err(no_trash("fuori dalla cartella iniziale della connessione"));
    }
    Ok(format!("{}/{REMOTE_DIR}", base.trim_end_matches('/')))
}

/// Il cestino dove andrebbe `p` (percorso completo, anche `/@remote/…`).
pub fn trash_for(p: &Path) -> io::Result<PathBuf> {
    if inside_trash(p) {
        return Err(no_trash("è già nel cestino"));
    }
    match remote::split(&p.to_string_lossy()) {
        Some((id, inner)) => Ok(PathBuf::from(format!("{}{id}{}", remote::PREFIX, remote_trash_for(&id, &inner)?))),
        None => Ok(local_trash_for(p)),
    }
}

/// Quello che l'interfaccia dice prima di eliminare: "trash" (va nel
/// cestino), "service:<tipo>" (cestino del servizio) o "none:<perché>".
pub fn hint(paths: &[String]) -> String {
    for p in paths {
        let path = Path::new(p);
        if let Some((id, _)) = remote::split(p) {
            let k = kind_of(&id);
            if service_kind(&k) {
                return format!("service:{k}");
            }
        }
        if let Err(e) = trash_for(path) {
            return format!("none:{}", e.to_string().trim_start_matches(NO_TRASH));
        }
        if remote::split(p).is_none() {
            // il cestino si potrà creare? (scheda in sola lettura, cartella di root)
            let t = local_trash_for(path);
            let probe = if t.exists() { t.clone() } else { t.parent().map(Path::to_path_buf).unwrap_or_default() };
            if !writable(&probe) || !path.parent().map(writable).unwrap_or(false) {
                return "none:serve la modalità root o la memoria è in sola lettura".into();
            }
        }
    }
    "trash".into()
}

pub fn writable(p: &Path) -> bool {
    use std::os::unix::ffi::OsStrExt;
    let Ok(c) = std::ffi::CString::new(p.as_os_str().as_bytes()) else { return false };
    unsafe { libc::access(c.as_ptr(), libc::W_OK) == 0 }
}

// ------------------------------------------------------------------ .trashinfo

fn pct(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"/-_.~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn unpct(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 3 <= b.len() {
            if let Some(v) = std::str::from_utf8(&b[i + 1..i + 3]).ok().and_then(|x| u8::from_str_radix(x, 16).ok()) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// «AAAA-MM-GGThh:mm:ss» in ora locale, come vuole la specifica.
pub fn local_date(t: i64) -> String {
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    let tt = t as libc::time_t;
    unsafe { libc::localtime_r(&tt, &mut tm) };
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}", tm.tm_year + 1900, tm.tm_mon + 1, tm.tm_mday, tm.tm_hour, tm.tm_min, tm.tm_sec)
}

pub fn parse_date(s: &str) -> i64 {
    let n: Vec<i32> = s.trim().split(|c: char| !c.is_ascii_digit()).filter(|x| !x.is_empty()).filter_map(|x| x.parse().ok()).collect();
    if n.len() < 6 {
        return 0;
    }
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    tm.tm_year = n[0] - 1900;
    tm.tm_mon = n[1] - 1;
    tm.tm_mday = n[2];
    tm.tm_hour = n[3];
    tm.tm_min = n[4];
    tm.tm_sec = n[5];
    tm.tm_isdst = -1;
    unsafe { libc::mktime(&mut tm) as i64 }
}

pub fn info_text(original: &str, deleted: i64) -> String {
    format!("[Trash Info]\nPath={}\nDeletionDate={}\n", pct(original), local_date(deleted))
}

/// (percorso originale, data) da un .trashinfo; un Path relativo è relativo
/// alla cima del montaggio (la cartella che contiene il cestino).
pub fn parse_info(text: &str, trash: &Path) -> (String, i64) {
    let (mut path, mut date) = (String::new(), 0);
    for l in text.lines() {
        if let Some(v) = l.strip_prefix("Path=") {
            path = unpct(v.trim());
        } else if let Some(v) = l.strip_prefix("DeletionDate=") {
            date = parse_date(v);
        }
    }
    if !path.starts_with('/') && !path.is_empty() {
        let top = trash.parent().map(Path::to_path_buf).unwrap_or_default();
        let top = match remote::split(&top.to_string_lossy()) {
            Some((_, inner)) => PathBuf::from(inner),
            None => top,
        };
        path = top.join(path).to_string_lossy().into_owned();
    }
    (path, date)
}

fn read_small(p: &Path) -> io::Result<String> {
    let mut s = String::new();
    vfs::open_read(p)?.take(64 * 1024).read_to_string(&mut s)?;
    Ok(s)
}

fn write_new(p: &Path, text: &str) -> io::Result<()> {
    if remote::split(&p.to_string_lossy()).is_none() {
        // create_new: se due eliminazioni scelgono lo stesso nome, una ritenta
        let mut f = fs::OpenOptions::new().write(true).create_new(true).open(p)?;
        f.write_all(text.as_bytes())?;
        return f.sync_data();
    }
    let mut s = vfs::create(p, 0o600, Some(text.len() as u64))?;
    s.write_all(text.as_bytes())?;
    s.flush()
}

fn ensure_dirs(t: &Path) -> io::Result<()> {
    let local = remote::split(&t.to_string_lossy()).is_none();
    if local {
        let fresh = !t.exists();
        fs::create_dir_all(t.join("files"))?;
        fs::create_dir_all(t.join("info"))?;
        if fresh {
            // la specifica vuole il cestino di montaggio solo per il suo utente
            let _ = fs::set_permissions(t, fs::Permissions::from_mode(0o700));
        }
        return Ok(());
    }
    for d in [t.to_path_buf(), t.join("files"), t.join("info")] {
        if !vfs::is_dir(&d) {
            match vfs::create_dir(&d) {
                Ok(()) => {}
                Err(_) if vfs::is_dir(&d) => {}
                Err(e) => return Err(no_trash(&format!("il cestino non si può creare sul server ({e})"))),
            }
        }
    }
    Ok(())
}

// ------------------------------------------------------------------ azioni

/// Sposta `p` nel cestino. Errori `NOTRASH:` = qui il cestino non si può fare.
pub fn move_to_trash(p: &Path) -> io::Result<()> {
    let s = p.to_string_lossy().into_owned();
    if let Some((id, _)) = remote::split(&s)
        && service_kind(&kind_of(&id))
    {
        // il servizio sposta da sé nel suo cestino
        return vfs::remove_file(p);
    }
    let t = trash_for(p)?;
    move_into(p, &t)
}

/// Sposta `p` nel cestino `t` (che si crea se manca).
pub fn move_into(p: &Path, t: &Path) -> io::Result<()> {
    let s = p.to_string_lossy().into_owned();
    let t = t.to_path_buf();
    ensure_dirs(&t).map_err(|e| if e.to_string().starts_with(NO_TRASH) { e } else { no_trash(&format!("il cestino non si può creare ({e})")) })?;
    let original = match remote::split(&s) {
        Some((_, inner)) => inner,
        None => s.clone(),
    };
    let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "senza-nome".into());
    let text = info_text(&original, now());
    for n in 1..10_000u32 {
        let cand = if n == 1 { name.clone() } else { format!("{name}.{n}") };
        let info = t.join("info").join(format!("{cand}.trashinfo"));
        let dest = t.join("files").join(&cand);
        if vfs::exists(&dest) || vfs::exists(&info) {
            continue;
        }
        match write_new(&info, &text) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(no_trash(&format!("il cestino non è scrivibile ({e})"))),
        }
        return match vfs::rename(p, &dest) {
            Ok(()) => Ok(()),
            Err(e) => {
                let _ = vfs::remove_file(&info);
                if e.raw_os_error() == Some(vfs::EXDEV) {
                    Err(no_trash("il cestino sta su un altro filesystem"))
                } else {
                    Err(e)
                }
            }
        };
    }
    Err(io::Error::other("troppi elementi con lo stesso nome nel cestino"))
}

/// Le voci di un cestino (JSON come quelle di Nextcloud: id, name, original,
/// deleted, size, dir). `id` = il percorso completo in `files/`.
fn items_of(t: &Path) -> Vec<Value> {
    let mut out = Vec::new();
    let Ok(infos) = vfs::read_dir_meta(&t.join("info")) else { return out };
    for (info, _) in infos {
        let file = info.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let Some(stored) = file.strip_suffix(".trashinfo") else { continue };
        let target = t.join("files").join(stored);
        let Ok(m) = vfs::lmeta(&target) else { continue };
        let (original, deleted) = read_small(&info).map(|s| parse_info(&s, t)).unwrap_or_default();
        let name = Path::new(&original).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| stored.to_string());
        out.push(json!({
            "id": target.to_string_lossy(),
            "name": name,
            "original": original,
            "deleted": deleted,
            "size": if m.is_dir { 0 } else { m.len },
            "dir": m.is_dir,
        }));
    }
    out
}

/// I cestini da mostrare per `scope`: la cartella dove si è (memoria locale)
/// o `/@remote/<id>` (server senza cestino proprio).
pub fn dirs_for(scope: &str) -> Vec<PathBuf> {
    if let Some((id, _)) = remote::split(scope) {
        let Some((kind, start)) = conn_of(&id) else { return Vec::new() };
        let pre = format!("{}{id}", remote::PREFIX);
        if kind == "smb" || kind == "s3-buckets" {
            let shares = vfs::read_dir(Path::new(&format!("{pre}/"))).unwrap_or_default();
            return shares.into_iter().map(|s| s.join(REMOTE_DIR)).filter(|t| vfs::is_dir(t)).collect();
        }
        let s = start.trim_end_matches('/');
        return vec![PathBuf::from(format!("{pre}{s}/{REMOTE_DIR}"))];
    }
    let p = Path::new(if scope.is_empty() { "/" } else { scope });
    let top = mount_of(p);
    let mut v = Vec::new();
    if uid() != 0 && top == mount_of(&home()) {
        v.push(home_trash());
    }
    let own = top.join(format!(".Trash-{}", uid()));
    if !v.contains(&own) {
        v.push(own);
    }
    v
}

/// Il cestino che contiene la voce `id` (…/<cestino>/files/<nome>).
fn trash_of_item(id: &str) -> io::Result<(PathBuf, String)> {
    let p = Path::new(id);
    let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let files = p.parent().ok_or_else(|| io::Error::other("voce non valida"))?;
    if files.file_name().map(|n| n != "files").unwrap_or(true) || name.is_empty() {
        return Err(io::Error::other("voce del cestino non valida"));
    }
    let t = files.parent().ok_or_else(|| io::Error::other("voce non valida"))?.to_path_buf();
    if !inside_trash(&t.join("files")) {
        return Err(io::Error::other("non è un cestino"));
    }
    Ok((t, name))
}

fn delete_item(t: &Path, name: &str) -> io::Result<()> {
    let target = t.join("files").join(name);
    if vfs::lmeta(&target).is_ok() {
        vfs::remove_all(&target)?;
    }
    let _ = vfs::remove_file(&t.join("info").join(format!("{name}.trashinfo")));
    Ok(())
}

/// Rimette la voce dov'era; se lì c'è già qualcosa, accanto con «(1)».
fn restore_item(t: &Path, name: &str) -> io::Result<String> {
    let info = t.join("info").join(format!("{name}.trashinfo"));
    let (original, _) = parse_info(&read_small(&info)?, t);
    if original.is_empty() {
        return Err(io::Error::other("il cestino non sa dove stava questo elemento"));
    }
    let full = match remote::split(&t.to_string_lossy()) {
        Some((id, _)) => PathBuf::from(format!("{}{id}{original}", remote::PREFIX)),
        None => PathBuf::from(&original),
    };
    if let Some(parent) = full.parent() {
        make_dirs(parent)?;
    }
    let dest = crate::jobs::free_name(&full);
    vfs::rename(&t.join("files").join(name), &dest)?;
    let _ = vfs::remove_file(&info);
    Ok(dest.to_string_lossy().into_owned())
}

fn make_dirs(p: &Path) -> io::Result<()> {
    if vfs::is_dir(p) {
        return Ok(());
    }
    if let Some(parent) = p.parent() {
        make_dirs(parent)?;
    }
    match vfs::create_dir(p) {
        Err(_) if vfs::is_dir(p) => Ok(()),
        r => r,
    }
}

/// Toglie le voci più vecchie di `days()` giorni; rende quante.
pub fn purge(t: &Path) -> usize {
    let d = days();
    if d == 0 {
        return 0;
    }
    let limit = now() - i64::from(d) * 86_400;
    let mut n = 0;
    for v in items_of(t) {
        let deleted = v["deleted"].as_i64().unwrap_or(0);
        if deleted > 0 && deleted < limit {
            let name = Path::new(v["id"].as_str().unwrap_or("")).file_name().map(|x| x.to_string_lossy().into_owned()).unwrap_or_default();
            if !name.is_empty() && delete_item(t, &name).is_ok() {
                n += 1;
            }
        }
    }
    n
}

/// Il cestino di `scope`. action: list | restore | delete | empty; `item` = id
/// della voce. Rende il JSON delle voci (list) o il percorso ripristinato.
pub fn op(scope: &str, action: &str, item: &str) -> io::Result<String> {
    op_in(&dirs_for(scope), action, item)
}

/// Come `op`, sui cestini indicati.
pub fn op_in(dirs: &[PathBuf], action: &str, item: &str) -> io::Result<String> {
    match action {
        "list" => {
            let mut all = Vec::new();
            for t in dirs {
                if vfs::is_dir(&t) {
                    purge(&t);
                    all.extend(items_of(&t));
                }
            }
            all.sort_by_key(|v| -v["deleted"].as_i64().unwrap_or(0));
            Ok(Value::Array(all).to_string())
        }
        "restore" => {
            let (t, name) = trash_of_item(item)?;
            restore_item(&t, &name)
        }
        "delete" => {
            let (t, name) = trash_of_item(item)?;
            delete_item(&t, &name).map(|_| String::new())
        }
        "empty" => {
            for t in dirs {
                if !vfs::is_dir(&t) {
                    continue;
                }
                for f in vfs::read_dir(&t.join("files")).unwrap_or_default() {
                    vfs::remove_all(&f)?;
                }
                for f in vfs::read_dir(&t.join("info")).unwrap_or_default() {
                    let _ = vfs::remove_file(&f);
                }
            }
            Ok(String::new())
        }
        _ => Err(io::Error::other(format!("azione sconosciuta: {action}"))),
    }
}

/// All'avvio: le voci vecchie dei cestini locali (home e montaggi dell'utente).
pub fn purge_local() {
    let mut dirs = vec![home_trash()];
    let me = format!(".Trash-{}", uid());
    for m in mount_points() {
        let s = m.to_string_lossy();
        if s.starts_with("/home") || s.starts_with("/run/media") || s.starts_with("/media") {
            let t = m.join(&me);
            if t.is_dir() && !dirs.contains(&t) {
                dirs.push(t);
            }
        }
    }
    for t in dirs {
        if t.is_dir() {
            purge(&t);
        }
    }
}
