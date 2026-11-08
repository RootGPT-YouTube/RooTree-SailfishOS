//! La cassaforte delle password delle connessioni: un file cifrato con una
//! password principale scelta dall'utente (Argon2id → chiave, poi
//! XChaCha20-Poly1305). Si sblocca una volta per sessione; la chiave resta in
//! memoria finché l'app è aperta o finché la si richiude.
//!
//! Perché non Sailfish Secrets: è una libreria Qt 5 con un D-Bus privato, e
//! RooTree è un'app Qt 6. Perché non un file in chiaro (anche se a 600): chi
//! copia la home, o un backup, avrebbe tutte le password.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use argon2::{Algorithm, Argon2, Params, Version};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct File {
    v: u32,
    /// parametri di Argon2id (memoria in KiB, passate)
    m: u32,
    t: u32,
    salt: String,
    nonce: String,
    data: String,
}

struct Open {
    key: [u8; 32],
    salt: Vec<u8>,
    m: u32,
    t: u32,
    secrets: BTreeMap<String, String>,
}

fn state() -> &'static Mutex<Option<Open>> {
    static S: OnceLock<Mutex<Option<Open>>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(None))
}

static PATH: OnceLock<Mutex<Option<PathBuf>>> = OnceLock::new();

/// Dove sta il file (lo decide l'app: la sua cartella di configurazione).
pub fn set_path(p: &str) {
    *PATH.get_or_init(Default::default).lock().unwrap() = Some(PathBuf::from(p));
}

fn path() -> io::Result<PathBuf> {
    PATH.get_or_init(Default::default)
        .lock()
        .unwrap()
        .clone()
        .ok_or_else(|| io::Error::other("cassaforte senza percorso"))
}

pub fn exists() -> bool {
    path().map(|p| p.exists()).unwrap_or(false)
}

pub fn unlocked() -> bool {
    state().lock().unwrap().is_some()
}

fn derive(master: &str, salt: &[u8], m: u32, t: u32) -> io::Result<[u8; 32]> {
    let params = Params::new(m, t, 1, Some(32)).map_err(|e| io::Error::other(e.to_string()))?;
    let mut key = [0u8; 32];
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
        .hash_password_into(master.as_bytes(), salt, &mut key)
        .map_err(|e| io::Error::other(e.to_string()))?;
    Ok(key)
}

fn save(o: &Open) -> io::Result<()> {
    let mut nonce = [0u8; 24];
    OsRng.fill_bytes(&mut nonce);
    let plain = serde_json::to_vec(&o.secrets)?;
    let data = XChaCha20Poly1305::new((&o.key).into())
        .encrypt(XNonce::from_slice(&nonce), plain.as_ref())
        .map_err(|_| io::Error::other("cifratura non riuscita"))?;
    let f = File { v: 1, m: o.m, t: o.t, salt: B64.encode(&o.salt), nonce: B64.encode(nonce), data: B64.encode(data) };
    let p = path()?;
    if let Some(d) = p.parent() {
        fs::create_dir_all(d)?;
    }
    // scritto accanto e rinominato: mai una cassaforte a metà
    let tmp = p.with_extension("tmp");
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut w = fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(&tmp)?;
        w.write_all(&serde_json::to_vec(&f)?)?;
        w.sync_all()?;
    }
    fs::rename(&tmp, &p)
}

/// Crea la cassaforte (vuota) con la password principale, e la lascia aperta.
pub fn create(master: &str) -> io::Result<()> {
    if master.chars().count() < 6 {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "la password principale deve avere almeno 6 caratteri"));
    }
    let mut salt = vec![0u8; 16];
    OsRng.fill_bytes(&mut salt);
    // 32 MiB e 3 passate: un secondo circa sul POCO, lento per chi tira a indovinare
    let (m, t) = (32 * 1024, 3);
    let key = derive(master, &salt, m, t)?;
    let o = Open { key, salt, m, t, secrets: BTreeMap::new() };
    save(&o)?;
    *state().lock().unwrap() = Some(o);
    Ok(())
}

/// Apre la cassaforte; password sbagliata → errore `WRONG:`.
pub fn unlock(master: &str) -> io::Result<()> {
    let f: File = serde_json::from_slice(&fs::read(path()?)?)?;
    let salt = B64.decode(&f.salt).map_err(io::Error::other)?;
    let nonce = B64.decode(&f.nonce).map_err(io::Error::other)?;
    let data = B64.decode(&f.data).map_err(io::Error::other)?;
    let key = derive(master, &salt, f.m, f.t)?;
    let plain = XChaCha20Poly1305::new((&key).into())
        .decrypt(XNonce::from_slice(&nonce), data.as_ref())
        .map_err(|_| io::Error::new(io::ErrorKind::PermissionDenied, "WRONG:password principale sbagliata"))?;
    let secrets = serde_json::from_slice(&plain)?;
    *state().lock().unwrap() = Some(Open { key, salt, m: f.m, t: f.t, secrets });
    Ok(())
}

pub fn lock() {
    if let Some(mut o) = state().lock().unwrap().take() {
        o.key.fill(0);
    }
}

pub fn get(id: &str) -> Option<String> {
    state().lock().unwrap().as_ref().and_then(|o| o.secrets.get(id).cloned())
}

pub fn set(id: &str, secret: &str) -> io::Result<()> {
    let mut g = state().lock().unwrap();
    let o = g.as_mut().ok_or_else(|| io::Error::new(io::ErrorKind::PermissionDenied, "LOCKED:cassaforte chiusa"))?;
    o.secrets.insert(id.to_string(), secret.to_string());
    save(o)
}

pub fn remove(id: &str) -> io::Result<()> {
    let mut g = state().lock().unwrap();
    let Some(o) = g.as_mut() else { return Ok(()) };
    if o.secrets.remove(id).is_some() {
        save(o)?;
    }
    Ok(())
}
