//! Le connessioni remote (Fase 5). Step 2: SFTP con libssh2 (crate ssh2).
//!
//! Una cartella remota ha un percorso locale finto, `/@remote/<id>/<percorso
//! sul server>`, così il resto dell'app (barra del percorso, cronologia,
//! «su») la tratta come le altre; `split()` lo riconosce.
//!
//! Sicurezza: l'impronta del server si controlla con ~/.ssh/known_hosts,
//! come fa ssh. Server nuovo → errore `HOSTKEY_NEW:` con l'impronta, e
//! l'interfaccia chiede se fidarsi (`trust()` la aggiunge); impronta cambiata
//! → `HOSTKEY_CHANGED:`, senza scorciatoie: va sistemato known_hosts.

use std::collections::HashMap;
use std::io;
use std::net::{TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use serde::Deserialize;
use ssh2::{CheckResult, HashType, KnownHostFileKind, KnownHostKeyFormat, Session, Sftp};

use crate::ffi::Entry;
use crate::fsops;

pub const PREFIX: &str = "/@remote/";

/// Il profilo, come lo salva Remotes (C++): si usa solo ciò che serve qui.
#[derive(Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct Profile {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub host: String,
    pub port: u16,
    pub user: String,
    pub auth: String,
    #[serde(rename = "keyPath")]
    pub key_path: String,
    pub path: String,
}

struct Conn {
    profile: Profile,
    secret: String,
    session: Session,
    sftp: Sftp,
}

fn conns() -> &'static Mutex<HashMap<String, Arc<Mutex<Conn>>>> {
    static C: OnceLock<Mutex<HashMap<String, Arc<Mutex<Conn>>>>> = OnceLock::new();
    C.get_or_init(Default::default)
}

/// L'impronta dell'ultimo server sconosciuto, per `trust()`: (id, host per
/// known_hosts, chiave, tipo).
fn pending() -> &'static Mutex<Option<(String, String, Vec<u8>, KnownHostKeyFormat)>> {
    static P: OnceLock<Mutex<Option<(String, String, Vec<u8>, KnownHostKeyFormat)>>> = OnceLock::new();
    P.get_or_init(|| Mutex::new(None))
}

fn known_hosts_file() -> PathBuf {
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/home/defaultuser"));
    home.join(".ssh/known_hosts")
}

fn expand(p: &str) -> PathBuf {
    match p.strip_prefix("~/") {
        Some(rest) => std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default().join(rest),
        None => PathBuf::from(p),
    }
}

/// Il nome con cui known_hosts conosce il server: «host» o «[host]:porta».
fn kh_name(host: &str, port: u16) -> String {
    if port == 22 { host.to_string() } else { format!("[{host}]:{port}") }
}

fn fingerprint(s: &Session) -> String {
    use base64::Engine;
    s.host_key_hash(HashType::Sha256)
        .map(|h| format!("SHA256:{}", base64::engine::general_purpose::STANDARD_NO_PAD.encode(h)))
        .unwrap_or_default()
}

fn key_type_name(t: ssh2::HostKeyType) -> &'static str {
    use ssh2::HostKeyType::*;
    match t {
        Rsa => "RSA",
        Dss => "DSA",
        Ecdsa256 | Ecdsa384 | Ecdsa521 => "ECDSA",
        Ed25519 => "ED25519",
        _ => "?",
    }
}

fn kh_format(t: ssh2::HostKeyType) -> KnownHostKeyFormat {
    KnownHostKeyFormat::from(t)
}

/// Collega (o ricollega) il profilo. `secret`: password, o frase della
/// chiave. Rende il percorso iniziale sul server. Errori con codice:
/// `HOSTKEY_NEW:tipo:impronta`, `HOSTKEY_CHANGED:tipo:impronta`,
/// `AUTH:` (serve una password o è sbagliata), `KEY:` (chiave illeggibile o
/// frase mancante), `NET:` (server irraggiungibile).
pub fn connect(profile_json: &str, secret: &str) -> Result<String, String> {
    let p: Profile = serde_json::from_str(profile_json).map_err(|e| format!("profilo non valido: {e}"))?;
    if p.kind != "sftp" {
        return Err(format!("UNSUPPORTED:il collegamento a «{}» arriva in uno step successivo", p.kind));
    }
    let port = if p.port == 0 { 22 } else { p.port };
    let addr = (p.host.as_str(), port)
        .to_socket_addrs()
        .map_err(|e| format!("NET:{}: {e}", p.host))?
        .next()
        .ok_or_else(|| format!("NET:{}: indirizzo sconosciuto", p.host))?;
    let tcp = TcpStream::connect_timeout(&addr, Duration::from_secs(10)).map_err(|e| format!("NET:{}:{port}: {e}", p.host))?;
    let _ = tcp.set_read_timeout(Some(Duration::from_secs(30)));
    let mut s = Session::new().map_err(|e| e.to_string())?;
    s.set_tcp_stream(tcp);
    s.set_timeout(30_000);
    s.handshake().map_err(|e| format!("NET:handshake: {e}"))?;

    // l'impronta del server, prima di mandargli qualunque credenziale
    let (key, ktype) = s.host_key().ok_or("il server non ha mostrato la sua chiave")?;
    let name = kh_name(&p.host, port);
    let mut kh = s.known_hosts().map_err(|e| e.to_string())?;
    let file = known_hosts_file();
    if file.exists() {
        let _ = kh.read_file(&file, KnownHostFileKind::OpenSSH);
    }
    let fp = fingerprint(&s);
    match kh.check_port(&p.host, port, key) {
        CheckResult::Match => {}
        CheckResult::Mismatch => return Err(format!("HOSTKEY_CHANGED:{}:{fp}", key_type_name(ktype))),
        CheckResult::NotFound | CheckResult::Failure => {
            *pending().lock().unwrap() = Some((p.id.clone(), name, key.to_vec(), kh_format(ktype)));
            return Err(format!("HOSTKEY_NEW:{}:{fp}", key_type_name(ktype)));
        }
    }

    // l'accesso
    match p.auth.as_str() {
        "key" => {
            let k = expand(&p.key_path);
            if !k.is_file() {
                return Err(format!("KEY:la chiave {} non c'è", k.display()));
            }
            let pass = if secret.is_empty() { None } else { Some(secret) };
            if let Err(e) = s.userauth_pubkey_file(&p.user, None, &k, pass) {
                let msg = e.message().to_string();
                // libssh2 dice «Unable to extract public key» o simili quando
                // la chiave è cifrata e manca la frase
                if secret.is_empty() && (msg.contains("decrypt") || msg.contains("passphrase") || msg.contains("extract") || msg.contains("parse")) {
                    return Err("KEY:la chiave è protetta da una frase".into());
                }
                return Err(format!("AUTH:{msg}"));
            }
        }
        _ => {
            if secret.is_empty() {
                return Err("AUTH:serve la password".into());
            }
            s.userauth_password(&p.user, secret).map_err(|e| format!("AUTH:{}", e.message()))?;
        }
    }
    if !s.authenticated() {
        return Err("AUTH:accesso rifiutato".into());
    }
    let sftp = s.sftp().map_err(|e| format!("il server non offre SFTP: {e}"))?;
    let start = if p.path.trim().is_empty() {
        sftp.realpath(Path::new(".")).map(|x| x.to_string_lossy().into_owned()).unwrap_or_else(|_| "/".into())
    } else {
        p.path.trim().to_string()
    };
    let id = p.id.clone();
    conns().lock().unwrap().insert(id, Arc::new(Mutex::new(Conn { profile: p, secret: secret.to_string(), session: s, sftp })));
    Ok(start)
}

/// Fidarsi del server appena visto (risposta a HOSTKEY_NEW): la sua chiave
/// va in ~/.ssh/known_hosts, come farebbe ssh.
pub fn trust(id: &str) -> Result<(), String> {
    let Some((pid, name, key, fmt)) = pending().lock().unwrap().take() else {
        return Err("nessun server in attesa di conferma".into());
    };
    if pid != id {
        return Err("la conferma non è per questo server".into());
    }
    let s = Session::new().map_err(|e| e.to_string())?;
    let mut kh = s.known_hosts().map_err(|e| e.to_string())?;
    let file = known_hosts_file();
    if file.exists() {
        kh.read_file(&file, KnownHostFileKind::OpenSSH).map_err(|e| format!("known_hosts illeggibile: {e}"))?;
    }
    kh.add(&name, &key, "aggiunto da RooTree", fmt).map_err(|e| e.to_string())?;
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700));
    }
    kh.write_file(&file, KnownHostFileKind::OpenSSH).map_err(|e| format!("known_hosts: {e}"))
}

pub fn disconnect(id: &str) {
    if let Some(c) = conns().lock().unwrap().remove(id)
        && let Ok(c) = c.lock()
    {
        let _ = c.session.disconnect(None, "RooTree", None);
    }
}

pub fn connected(id: &str) -> bool {
    conns().lock().unwrap().contains_key(id)
}

/// (id, percorso sul server) da un percorso `/@remote/<id>/…`.
pub fn split(path: &str) -> Option<(String, String)> {
    let rest = path.strip_prefix(PREFIX).or_else(|| (path == PREFIX.trim_end_matches('/')).then_some(""))?;
    let (id, inner) = rest.split_once('/').unwrap_or((rest, ""));
    if id.is_empty() {
        return None;
    }
    Some((id.to_string(), format!("/{inner}")))
}

/// Una richiesta sulla connessione; se è caduta, un tentativo di ricollegarsi
/// coi dati della sessione (la rete del telefono va e viene).
fn with_conn<T>(id: &str, f: impl Fn(&Conn) -> Result<T, ssh2::Error>) -> io::Result<T> {
    let c = conns()
        .lock()
        .unwrap()
        .get(id)
        .cloned()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotConnected, "NOTCONNECTED:connessione non aperta"))?;
    let first = {
        let g = c.lock().unwrap();
        f(&g)
    };
    match first {
        Ok(v) => Ok(v),
        Err(e) if is_sftp_error(&e) => Err(sftp_io_error(e)),
        Err(_) => {
            // la sessione è caduta: si ricollega una volta
            let (profile, secret) = {
                let g = c.lock().unwrap();
                (g.profile.clone(), g.secret.clone())
            };
            let json = serde_json::json!({
                "id": profile.id, "type": profile.kind, "host": profile.host, "port": profile.port,
                "user": profile.user, "auth": profile.auth, "keyPath": profile.key_path, "path": profile.path,
            })
            .to_string();
            connect(&json, &secret).map_err(io::Error::other)?;
            let c = conns().lock().unwrap().get(id).cloned().unwrap();
            let g = c.lock().unwrap();
            f(&g).map_err(sftp_io_error)
        }
    }
}

fn is_sftp_error(e: &ssh2::Error) -> bool {
    matches!(e.code(), ssh2::ErrorCode::SFTP(_))
}

fn sftp_io_error(e: ssh2::Error) -> io::Error {
    // i codici SFTP più comuni, con un messaggio comprensibile
    let msg = match e.code() {
        ssh2::ErrorCode::SFTP(2) => "non esiste".to_string(),
        ssh2::ErrorCode::SFTP(3) => "Permission denied (sul server)".to_string(),
        _ => e.message().to_string(),
    };
    let kind = match e.code() {
        ssh2::ErrorCode::SFTP(2) => io::ErrorKind::NotFound,
        ssh2::ErrorCode::SFTP(3) => io::ErrorKind::PermissionDenied,
        _ => io::ErrorKind::Other,
    };
    io::Error::new(kind, msg)
}

fn entry(id: &str, dir: &str, name: String, st: &ssh2::FileStat, target: Option<ssh2::FileStat>) -> Entry {
    let is_link = st.file_type().is_symlink();
    let real = if is_link { target.as_ref() } else { Some(st) };
    let is_dir = real.is_some_and(|t| t.is_dir());
    let kind = if is_dir { fsops::KIND_DIR } else { fsops::KIND_FILE };
    let full = if dir == "/" { format!("/{name}") } else { format!("{dir}/{name}") };
    let category = if is_dir { fsops::folder_category(&name) } else { fsops::category(&name) };
    let s = real.unwrap_or(st);
    Entry {
        hidden: name.starts_with('.'),
        path: format!("{PREFIX}{id}{full}"),
        name,
        kind,
        is_link,
        link_broken: is_link && target.is_none(),
        link_target: String::new(),
        size: if is_dir { 0 } else { s.size.unwrap_or(0) },
        mtime: s.mtime.unwrap_or(0) as i64,
        mode: s.perm.unwrap_or(0),
        uid: s.uid.unwrap_or(0),
        gid: s.gid.unwrap_or(0),
        category: category.to_string(),
    }
}

/// Il contenuto di una cartella remota.
pub fn list(id: &str, dir: &str, show_hidden: bool) -> io::Result<Vec<Entry>> {
    let items = with_conn(id, |c| {
        let list = c.sftp.readdir(Path::new(dir))?;
        let mut out = Vec::with_capacity(list.len());
        for (p, st) in list {
            let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            if name.is_empty() || name == "." || name == ".." || (!show_hidden && name.starts_with('.')) {
                continue;
            }
            // un link: dove porta (cartella o file) lo dice stat, che lo segue
            let target = if st.file_type().is_symlink() { c.sftp.stat(&p).ok() } else { None };
            out.push((name, st, target));
        }
        Ok(out)
    })?;
    Ok(items.into_iter().map(|(n, st, t)| entry(id, dir, n, &st, t)).collect())
}

pub fn stat(id: &str, path: &str) -> io::Result<Entry> {
    let p = Path::new(path);
    let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "/".into());
    let dir = p.parent().map(|d| d.to_string_lossy().into_owned()).unwrap_or_else(|| "/".into());
    let (st, target) = with_conn(id, |c| {
        let st = c.sftp.lstat(p)?;
        let t = if st.file_type().is_symlink() { c.sftp.stat(p).ok() } else { None };
        Ok((st, t))
    })?;
    Ok(entry(id, &dir, name, &st, target))
}

// ------------------------------------------------------------------ step 3: operazioni sui file

/// Un'operazione SFTP sulla connessione `id` (con la ricollegata automatica).
/// La connessione resta bloccata solo per la chiamata: un file aperto
/// (ssh2::File) vive per conto suo, e la sessione libssh2 è thread-safe.
pub fn op<T>(id: &str, f: impl Fn(&Sftp) -> Result<T, ssh2::Error>) -> io::Result<T> {
    with_conn(id, |c| f(&c.sftp))
}

pub fn open_read(id: &str, path: &str) -> io::Result<ssh2::File> {
    op(id, |s| s.open(Path::new(path)))
}

pub fn create(id: &str, path: &str, mode: i32) -> io::Result<ssh2::File> {
    use ssh2::{OpenFlags, OpenType};
    op(id, |s| s.open_mode(Path::new(path), OpenFlags::WRITE | OpenFlags::CREATE | OpenFlags::TRUNCATE, mode, OpenType::File))
}

pub fn rename(id: &str, from: &str, to: &str) -> io::Result<()> {
    op(id, |s| s.rename(Path::new(from), Path::new(to), None))
}

/// Rinomina una voce sulla stessa cartella del server (come rename_entry):
/// mai sopra una voce che c'è già.
pub fn rename_entry(id: &str, path: &str, new_name: &str) -> io::Result<String> {
    fsops::valid_name(new_name)?;
    let p = Path::new(path);
    let to = p.with_file_name(new_name);
    if op(id, |s| s.lstat(&to)).is_ok() {
        return Err(io::Error::new(io::ErrorKind::AlreadyExists, "esiste già un elemento con questo nome"));
    }
    op(id, |s| s.rename(p, &to, None))?;
    Ok(format!("{PREFIX}{id}{}", to.to_string_lossy()))
}

pub fn make_dir(id: &str, parent: &str, name: &str) -> io::Result<String> {
    fsops::valid_name(name)?;
    let p = Path::new(parent).join(name);
    op(id, |s| s.mkdir(&p, 0o755))?;
    Ok(format!("{PREFIX}{id}{}", p.to_string_lossy()))
}

pub fn set_stat(id: &str, path: &str, mode: Option<u32>, mtime: Option<u64>) -> io::Result<()> {
    let st = ssh2::FileStat { size: None, uid: None, gid: None, perm: mode, atime: mtime, mtime };
    op(id, |s| s.setstat(Path::new(path), st.clone()))
}

/// Il testo di un file remoto (per il visore), al più `max` byte.
pub fn read_head(id: &str, path: &str, max: u64) -> io::Result<Vec<u8>> {
    use std::io::Read;
    let mut f = open_read(id, path)?;
    let mut buf = Vec::new();
    (&mut f).take(max).read_to_end(&mut buf)?;
    Ok(buf)
}

/// Salva un file di testo sul server senza lasciarlo a metà: .part accanto,
/// stessi permessi dell'originale, poi al posto suo.
pub fn write_text(id: &str, path: &str, text: &str) -> io::Result<()> {
    use std::io::Write;
    let p = Path::new(path);
    let old = op(id, |s| s.stat(p)).ok();
    let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let part = p.with_file_name(format!(".{name}.rootree-part"));
    let part_s = part.to_string_lossy().into_owned();
    let r = (|| -> io::Result<()> {
        let mut f = create(id, &part_s, old.as_ref().and_then(|o| o.perm).map(|m| (m & 0o7777) as i32).unwrap_or(0o644))?;
        f.write_all(text.as_bytes())?;
        let _ = f.fsync();
        drop(f);
        if let Some(perm) = old.as_ref().and_then(|o| o.perm) {
            let _ = set_stat(id, &part_s, Some(perm & 0o7777), None);
        }
        crate::vfs::rename(Path::new(&format!("{PREFIX}{id}{part_s}")), Path::new(&format!("{PREFIX}{id}{path}")))
    })();
    if r.is_err() {
        let _ = op(id, |s| s.unlink(&part));
    }
    r
}
