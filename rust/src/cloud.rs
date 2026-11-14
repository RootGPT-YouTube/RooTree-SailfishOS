//! I cloud (Fase 5, step 4), un servizio alla volta (scelta dell'utente
//! dell'08/11): per primo Dropbox, il più semplice (API per percorso, PKCE
//! senza segreto). Ogni servizio implementa `Drive`; remote.rs lo usa come
//! SFTP, quindi copia, coda, Annulla e conflitti valgono uguali.
//!
//! Il «segreto» di una connessione cloud è il token di rinnovo (refresh
//! token) dell'accesso dal browser (oauth.rs); il token d'accesso vive solo
//! in memoria e si rinnova da solo.

use std::io::{self, Read, Write};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Deserialize;
use serde_json::{Value, json};

use crate::http;
use crate::oauth;
use crate::remote::RStat;

/// Quello che RooTree chiede a un cloud. I percorsi sono «/a/b» (radice «/»).
pub trait Drive: Send + Sync {
    /// La cartella da cui si apre la connessione.
    fn start(&self) -> String {
        "/".into()
    }
    fn stat(&self, path: &str) -> io::Result<RStat>;
    fn list(&self, path: &str) -> io::Result<Vec<(String, RStat)>>;
    fn read(&self, path: &str) -> io::Result<Box<dyn Read + Send>>;
    /// Un file in scrittura: si carica mentre lo si scrive, e `flush`
    /// chiude il caricamento (senza flush il file non nasce).
    /// `size`: la dimensione finale, se si sa.
    fn writer(self: Arc<Self>, path: &str, size: Option<u64>) -> io::Result<Box<dyn crate::vfs::Sink>>;
    fn mkdir(&self, path: &str) -> io::Result<()>;
    /// file o cartella (anche piena)
    fn delete(&self, path: &str) -> io::Result<()>;
    fn rename(&self, from: &str, to: &str) -> io::Result<()>;
    /// Eliminare senza cestino (chi ce l'ha lo salta; Dropbox non lo permette
    /// alle app comuni: resta nel suo cestino).
    fn delete_forever(&self, path: &str) -> io::Result<()> {
        self.delete(path)
    }

    /// Il cestino, per chi ce l'ha come cartella consultabile (Nextcloud).
    /// `action`: list | restore | delete | empty; dà il JSON delle voci (list).
    fn trash(&self, _action: &str, _item: &str) -> io::Result<String> {
        Err(io::Error::new(io::ErrorKind::Unsupported, "questo servizio non ha un cestino consultabile"))
    }
}

pub fn is_cloud_type(kind: &str) -> bool {
    matches!(kind, "dropbox" | "mega" | "webdav" | "nextcloud" | "ftp" | "smb" | "s3")
}

pub fn sink(d: Arc<dyn Drive>, path: &str, size: Option<u64>) -> io::Result<Box<dyn crate::vfs::Sink>> {
    d.writer(path, size)
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Profile {
    id: String,
    #[serde(rename = "type")]
    kind: String,
    #[serde(rename = "clientId")]
    client_id: String,
    user: String,
    /// il codice a 6 cifre: solo per questo collegamento, mai nel file dei profili
    mfa: String,
    /// l'utente ha chiesto di non inserire più il codice: la sessione va nella cassaforte
    #[serde(rename = "mfaTrusted")]
    mfa_trusted: bool,
}

/// Collega un profilo cloud col suo token di rinnovo (`secret`).
pub fn connect(profile_json: &str, secret: &str) -> Result<Arc<dyn Drive>, String> {
    let p: Profile = serde_json::from_str(profile_json).map_err(|e| format!("profilo non valido: {e}"))?;
    if p.kind == "webdav" || p.kind == "nextcloud" {
        return crate::webdav::connect(profile_json, secret);
    }
    if p.kind == "ftp" {
        return crate::ftp::connect(profile_json, secret);
    }
    if p.kind == "smb" {
        return crate::smb::connect(profile_json, secret);
    }
    if p.kind == "s3" {
        return crate::s3::connect(profile_json, secret);
    }
    if p.kind == "mega" {
        return Ok(Arc::new(Mega::connect(&p, secret)?));
    }
    if secret.is_empty() {
        return Err("LOGIN:serve l'accesso dal browser".into());
    }
    let auth = Auth::new(&p.kind, &p.client_id, secret);
    match p.kind.as_str() {
        "dropbox" => {
            let d = Dropbox { auth };
            // prova subito: token valido e account raggiungibile
            d.rpc("users/get_current_account", Value::Null).map_err(|e| e.to_string())?;
            let _ = p.id;
            Ok(Arc::new(d))
        }
        k => Err(format!("UNSUPPORTED:«{k}» arriva più avanti")),
    }
}

// ------------------------------------------------------------------ token

/// Il token d'accesso, rinnovato dal token di rinnovo quando scade (o quando
/// il servizio risponde 401).
pub struct Auth {
    kind: String,
    client_id: String,
    refresh: String,
    access: Mutex<Option<(String, Instant)>>,
}

impl Auth {
    fn new(kind: &str, client_id: &str, refresh: &str) -> Self {
        Auth { kind: kind.into(), client_id: client_id.into(), refresh: refresh.into(), access: Mutex::new(None) }
    }

    fn token(&self, force: bool) -> io::Result<String> {
        let mut g = self.access.lock().unwrap();
        if !force
            && let Some((t, until)) = g.as_ref()
            && Instant::now() < *until
        {
            return Ok(t.clone());
        }
        let t = oauth::refresh(&self.kind, &self.client_id, &self.refresh).map_err(io::Error::other)?;
        // un minuto di margine sulla scadenza dichiarata
        let life = if t.expires_in > 120 { t.expires_in - 60 } else { 600 };
        *g = Some((t.access_token.clone(), Instant::now() + Duration::from_secs(life)));
        Ok(t.access_token)
    }

    /// Una richiesta autenticata; con 401 si rinnova il token e si riprova una volta.
    fn send(&self, f: &dyn Fn(&str) -> Result<http::Resp, ureq::Error>) -> io::Result<http::Resp> {
        let mut r = f(&format!("Bearer {}", self.token(false)?)).map_err(http::net)?;
        if r.status().as_u16() == 401 {
            r = f(&format!("Bearer {}", self.token(true)?)).map_err(http::net)?;
        }
        Ok(r)
    }
}

fn body_string(r: &mut http::Resp) -> String {
    r.body_mut().read_to_string().unwrap_or_default()
}

fn ok_json(mut r: http::Resp) -> io::Result<Value> {
    let status = r.status().as_u16();
    let body = body_string(&mut r);
    if !(200..300).contains(&status) {
        return Err(http::status_error(status, &body));
    }
    if body.trim().is_empty() { Ok(Value::Null) } else { serde_json::from_str(&body).map_err(io::Error::other) }
}

// ------------------------------------------------------------------ Dropbox
// API v2: https://www.dropbox.com/developers/documentation/http/documentation
// RPC su api.dropboxapi.com (JSON nel corpo), contenuti su
// content.dropboxapi.com (argomenti nell'intestazione Dropbox-API-Arg).

pub struct Dropbox {
    auth: Auth,
}

/// La radice per Dropbox è "" (non «/»).
fn dbx_path(p: &str) -> String {
    if p == "/" || p.is_empty() { String::new() } else { p.trim_end_matches('/').to_string() }
}

fn dbx_stat(e: &Value) -> RStat {
    let dir = e[".tag"] == "folder";
    RStat {
        is_dir: dir,
        is_file: !dir,
        is_link: false,
        size: e["size"].as_u64().unwrap_or(0),
        mode: if dir { 0o755 } else { 0o644 },
        mtime: e["client_modified"].as_str().or(e["server_modified"].as_str()).map(http::rfc3339).unwrap_or(0),
    }
}

/// Più di 150 MB in una volta Dropbox non li accetta: sopra, sessione a pezzi.
const DBX_CHUNK: usize = 8 << 20;

impl Dropbox {
    /// Tutte le voci eliminate (path_lower, path_display), da tutto l'account.
    fn deleted_entries(&self) -> io::Result<Vec<(String, String)>> {
        let mut out = Vec::new();
        let mut v = self.rpc("files/list_folder", json!({ "path": "", "recursive": true, "include_deleted": true, "limit": 2000 }))?;
        loop {
            for e in v["entries"].as_array().into_iter().flatten() {
                if e[".tag"] == "deleted" {
                    let shown = e["path_display"].as_str().unwrap_or("").to_string();
                    let lower = e["path_lower"].as_str().map(str::to_string).unwrap_or_else(|| shown.to_lowercase());
                    out.push((lower, shown));
                }
            }
            if v["has_more"] != true {
                break;
            }
            let cursor = v["cursor"].as_str().unwrap_or("").to_string();
            v = self.rpc("files/list_folder/continue", json!({ "cursor": cursor }))?;
        }
        Ok(out)
    }

    /// Un file: la sua ultima versione torna dov'era. Una cartella vuota si ricrea.
    fn restore_one(&self, path: &str) -> io::Result<()> {
        match self.rpc("files/list_revisions", json!({ "path": path, "mode": "path", "limit": 1 })) {
            Ok(v) => {
                let rev = v["entries"][0]["rev"].as_str().unwrap_or("").to_string();
                if rev.is_empty() {
                    return Err(io::Error::other("Dropbox: nessuna versione da ripristinare"));
                }
                self.rpc("files/restore", json!({ "path": path, "rev": rev })).map(|_| ())
            }
            Err(e) if e.to_string().contains("not_file") => {
                self.rpc("files/create_folder_v2", json!({ "path": path, "autorename": false })).map(|_| ())
            }
            Err(e) => Err(e),
        }
    }

    fn rpc(&self, endpoint: &str, arg: Value) -> io::Result<Value> {
        let url = format!("https://api.dropboxapi.com/2/{endpoint}");
        let body = arg.to_string();
        let r = self.auth.send(&|bearer| {
            http::agent().post(&url).header("Authorization", bearer).header("Content-Type", "application/json").send(body.as_str())
        })?;
        ok_json(r)
    }

    fn content(&self, endpoint: &str, arg: Value, data: &[u8]) -> io::Result<Value> {
        let url = format!("https://content.dropboxapi.com/2/{endpoint}");
        let a = http::json_ascii(&arg);
        let r = self.auth.send(&|bearer| {
            http::agent()
                .post(&url)
                .header("Authorization", bearer)
                .header("Dropbox-API-Arg", a.as_str())
                .header("Content-Type", "application/octet-stream")
                .send(data)
        })?;
        ok_json(r)
    }
}

impl Drive for Dropbox {
    fn stat(&self, path: &str) -> io::Result<RStat> {
        let p = dbx_path(path);
        if p.is_empty() {
            return Ok(RStat { is_dir: true, mode: 0o755, ..Default::default() });
        }
        let e = self.rpc("files/get_metadata", json!({ "path": p }))?;
        if e[".tag"] == "deleted" {
            return Err(io::Error::new(io::ErrorKind::NotFound, "non esiste"));
        }
        Ok(dbx_stat(&e))
    }

    fn list(&self, path: &str) -> io::Result<Vec<(String, RStat)>> {
        let mut out = Vec::new();
        let mut v = self.rpc("files/list_folder", json!({ "path": dbx_path(path), "limit": 2000 }))?;
        loop {
            for e in v["entries"].as_array().into_iter().flatten() {
                if e[".tag"] == "deleted" {
                    continue;
                }
                out.push((e["name"].as_str().unwrap_or("").to_string(), dbx_stat(e)));
            }
            if v["has_more"] != true {
                break;
            }
            let cursor = v["cursor"].as_str().unwrap_or("").to_string();
            v = self.rpc("files/list_folder/continue", json!({ "cursor": cursor }))?;
        }
        Ok(out)
    }

    fn read(&self, path: &str) -> io::Result<Box<dyn Read + Send>> {
        let a = http::json_ascii(&json!({ "path": dbx_path(path) }));
        let mut r = self.auth.send(&|bearer| {
            http::agent()
                .post("https://content.dropboxapi.com/2/files/download")
                .header("Authorization", bearer)
                .header("Dropbox-API-Arg", a.as_str())
                .send_empty()
        })?;
        let status = r.status().as_u16();
        if status != 200 {
            let body = body_string(&mut r);
            return Err(http::status_error(status, &body));
        }
        Ok(Box::new(r.into_body().into_reader()))
    }

    fn writer(self: Arc<Self>, path: &str, _size: Option<u64>) -> io::Result<Box<dyn crate::vfs::Sink>> {
        Ok(Box::new(DropboxSink { drive: self, path: dbx_path(path), buf: Vec::new(), session: None, offset: 0, done: false }))
    }

    fn mkdir(&self, path: &str) -> io::Result<()> {
        self.rpc("files/create_folder_v2", json!({ "path": dbx_path(path), "autorename": false })).map(|_| ())
    }

    fn delete(&self, path: &str) -> io::Result<()> {
        self.rpc("files/delete_v2", json!({ "path": dbx_path(path) })).map(|_| ())
    }

    /// Il cestino di Dropbox: le voci eliminate (le cartelle intere una volta
    /// sola), da ripristinare. Il definitivo l'API lo dà solo ai team
    /// (files.permanent_delete): Dropbox le toglie da solo dopo 30 giorni.
    fn trash(&self, action: &str, item: &str) -> io::Result<String> {
        match action {
            "list" => {
                let deleted = self.deleted_entries()?;
                let lower: std::collections::HashSet<String> = deleted.iter().map(|(l, _)| l.clone()).collect();
                let mut out = Vec::new();
                for (l, shown) in &deleted {
                    // dentro una cartella eliminata: la rappresenta la cartella
                    let mut anc = l.as_str();
                    let mut covered = false;
                    while let Some(i) = anc.rfind('/') {
                        anc = &anc[..i];
                        if !anc.is_empty() && lower.contains(anc) {
                            covered = true;
                            break;
                        }
                    }
                    if covered {
                        continue;
                    }
                    let dir = deleted.iter().any(|(o, _)| o.starts_with(&format!("{l}/")));
                    let name = shown.rsplit('/').next().unwrap_or(shown);
                    out.push(json!({ "id": shown, "name": name, "original": shown, "deleted": 0, "size": 0, "dir": dir }));
                }
                Ok(Value::Array(out).to_string())
            }
            "restore" => {
                let want = item.to_lowercase();
                let deleted = self.deleted_entries()?;
                let inside: Vec<&(String, String)> = deleted.iter().filter(|(l, _)| l.starts_with(&format!("{want}/"))).collect();
                if inside.is_empty() {
                    self.restore_one(item)?;
                } else {
                    // una cartella: si rimettono i suoi file (Dropbox ricrea le cartelle)
                    for (l, shown) in &inside {
                        if !deleted.iter().any(|(o, _)| o.starts_with(&format!("{l}/"))) {
                            self.restore_one(shown)?;
                        }
                    }
                }
                Ok(String::new())
            }
            _ => Err(io::Error::other(
                "Dropbox non permette alle app di eliminare definitivamente: i file eliminati li toglie da solo dopo 30 giorni, o si tolgono dal sito",
            )),
        }
    }

    fn rename(&self, from: &str, to: &str) -> io::Result<()> {
        self.rpc("files/move_v2", json!({ "from_path": dbx_path(from), "to_path": dbx_path(to), "autorename": false })).map(|_| ())
    }
}

/// Caricamento a pezzi mentre si scrive: fino a 8 MB in memoria, poi una
/// sessione di caricamento (start/append), chiusa da flush (finish). Un file
/// piccolo parte con una sola chiamata «upload».
struct DropboxSink {
    // il servizio resta vivo finché il caricamento non è finito, anche se
    // nel frattempo la connessione viene chiusa
    drive: Arc<Dropbox>,
    path: String,
    buf: Vec<u8>,
    session: Option<String>,
    offset: u64,
    done: bool,
}

impl DropboxSink {
    fn d(&self) -> &Dropbox {
        &self.drive
    }

    fn push_chunk(&mut self) -> io::Result<()> {
        let chunk = std::mem::take(&mut self.buf);
        match &self.session {
            None => {
                let v = self.d().content("files/upload_session/start", json!({ "close": false }), &chunk)?;
                self.session = Some(v["session_id"].as_str().unwrap_or("").to_string());
            }
            Some(id) => {
                let arg = json!({ "cursor": { "session_id": id, "offset": self.offset }, "close": false });
                self.d().content("files/upload_session/append_v2", arg, &chunk)?;
            }
        }
        self.offset += chunk.len() as u64;
        Ok(())
    }
}

impl Write for DropboxSink {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        self.buf.extend_from_slice(data);
        if self.buf.len() >= DBX_CHUNK {
            self.push_chunk()?;
        }
        Ok(data.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        if self.done {
            return Ok(());
        }
        let commit = json!({ "path": self.path, "mode": "overwrite", "autorename": false, "mute": true });
        match self.session.clone() {
            None => {
                let data = std::mem::take(&mut self.buf);
                self.d().content("files/upload", commit, &data)?;
            }
            Some(id) => {
                let data = std::mem::take(&mut self.buf);
                let arg = json!({ "cursor": { "session_id": id, "offset": self.offset }, "commit": commit });
                self.d().content("files/upload_session/finish", arg, &data)?;
            }
        }
        self.done = true;
        Ok(())
    }
}

impl crate::vfs::Sink for DropboxSink {
    fn sync(&mut self) -> io::Result<()> {
        // i pezzi partono già da write(): niente da forzare
        Ok(())
    }
}

// ------------------------------------------------------------------ Mega
// Crate `mega` (MIT/Apache-2.0): la cifratura è lato client, quindi serve
// la password vera (il «segreto» della connessione, nella cassaforte) e niente
// browser né chiavi dell'app. L'API dà tutto l'albero in una volta: si ricorda
// per poco e si dimentica a ogni modifica.
// Eliminare e sovrascrivere = spostare nel cestino di Mega (si recupera dal sito).

/// Dove stavano le voci messe nel cestino di Mega da RooTree (handle →
/// percorso e data), accanto alla cassaforte.
fn mega_bin_file() -> Option<std::path::PathBuf> {
    crate::vault::dir().map(|d| d.join("cestino-mega.json"))
}

fn mega_bin_notes() -> Value {
    mega_bin_file().and_then(|f| std::fs::read_to_string(f).ok()).and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_else(|| json!({}))
}

/// `Some(percorso)` annota, `None` dimentica.
fn mega_bin_note(handle: &str, path: Option<&str>) {
    let Some(f) = mega_bin_file() else { return };
    let mut v = mega_bin_notes();
    let Some(m) = v.as_object_mut() else { return };
    match path {
        Some(p) => {
            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
            m.insert(handle.into(), json!({ "path": p, "deleted": now }));
        }
        None => {
            m.remove(handle);
        }
    }
    let tmp = f.with_extension("json.tmp");
    if std::fs::write(&tmp, v.to_string()).is_ok() {
        let _ = std::fs::rename(&tmp, &f);
    }
}

const MEGA_CACHE: Duration = Duration::from_secs(30);

fn mega_runtime() -> Result<tokio::runtime::Runtime, String> {
    tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().map_err(|e| e.to_string())
}

fn mega_client() -> Result<mega::Client, String> {
    mega::Client::builder().build(reqwest::Client::new()).map_err(|e| e.to_string())
}

fn mega_session_key(id: &str) -> String {
    format!("{id}#mega-session")
}

/// Toglie la fiducia: chiude la sessione anche sui server di Mega e la
/// cancella dalla cassaforte. Se Mega non risponde, dalla cassaforte sparisce
/// lo stesso e l'errore lo dice (la sessione resta viva finché non scade).
pub fn mega_revoke(id: &str) -> Result<(), String> {
    let key = mega_session_key(id);
    let stored = crate::vault::get(&key).unwrap_or_default();
    if stored.is_empty() {
        return Ok(());
    }
    let closed = (|| {
        let rt = mega_runtime()?;
        let mut client = mega_client()?;
        rt.block_on(async {
            client.resume_session(&stored).await?;
            client.logout().await
        })
        .map_err(|e| e.to_string())
    })();
    crate::vault::remove(&key).map_err(|e| e.to_string())?;
    closed
}

pub struct Mega {
    rt: tokio::runtime::Runtime,
    client: Arc<mega::Client>,
    nodes: Mutex<Option<(Arc<mega::Nodes>, Instant)>>,
}

fn mega_err(e: mega::Error) -> io::Error {
    let d = format!("{e:?}");
    if d.contains("ENOENT") {
        io::Error::new(io::ErrorKind::NotFound, "non esiste")
    } else if d.contains("EOVERQUOTA") {
        io::Error::other("spazio esaurito sul cloud")
    } else {
        io::Error::other(format!("Mega: {e}"))
    }
}

fn mega_login_err(e: mega::Error, with_code: bool) -> String {
    let d = format!("{e:?}");
    if d.contains("EMFAREQUIRED") {
        // «MFA:» chiede il codice; «MFA:wrong», che quello dato non andava
        if with_code { "MFA:wrong".into() } else { "MFA:".into() }
    } else if with_code && ["EFAILED", "EEXPIRED"].iter().any(|c| d.contains(c)) {
        "MFA:wrong".into()
    } else if ["ENOENT", "EACCESS", "EKEY", "EINCOMPLETE", "EARGS"].iter().any(|c| d.contains(c)) {
        "AUTH:email o password di Mega sbagliata".into()
    } else {
        format!("NET:Mega: {e}")
    }
}

fn mega_stat(n: &mega::Node) -> RStat {
    let dir = !matches!(n.kind(), mega::NodeKind::File);
    RStat {
        is_dir: dir,
        is_file: !dir,
        is_link: false,
        size: n.size(),
        mode: if dir { 0o755 } else { 0o644 },
        mtime: n.modified_at().or(n.created_at()).map(|d| d.timestamp()).unwrap_or(0),
    }
}

fn mega_parts(p: &str) -> Vec<&str> {
    p.split('/').filter(|c| !c.is_empty()).collect()
}

/// («/a/b», «c») da «/a/b/c»
fn mega_split(p: &str) -> (String, String) {
    let mut c = mega_parts(p);
    let name = c.pop().unwrap_or("").to_string();
    (format!("/{}", c.join("/")), name)
}

fn mega_child<'a>(nodes: &'a mega::Nodes, parent: &str, name: &str) -> Option<&'a mega::Node> {
    nodes.iter().find(|n| n.parent() == Some(parent) && n.name() == name)
}

/// Il nodo a quel percorso, dalla radice del Cloud Drive.
fn mega_at<'a>(nodes: &'a mega::Nodes, path: &str) -> io::Result<&'a mega::Node> {
    let mut cur = nodes.cloud_drive().ok_or_else(|| io::Error::other("Mega: Cloud Drive non trovato"))?;
    for c in mega_parts(path) {
        cur = mega_child(nodes, cur.handle(), c).ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "non esiste"))?;
    }
    Ok(cur)
}

impl Mega {
    fn connect(p: &Profile, password: &str) -> Result<Self, String> {
        let rt = mega_runtime()?;
        let mut client = mega_client()?;
        let key = mega_session_key(&p.id);
        let stored = crate::vault::get(&key).unwrap_or_default();
        if !stored.is_empty() {
            let resumed = rt.block_on(async {
                client.resume_session(&stored).await?;
                client.get_current_user_info().await
            });
            match resumed {
                Ok(_) => return Ok(Mega { rt, client: Arc::new(client), nodes: Mutex::new(None) }),
                Err(e) => {
                    let d = format!("{e:?}");
                    if !["ESID", "EACCESS", "ENOENT", "InvalidSession"].iter().any(|c| d.contains(c)) {
                        return Err(format!("NET:Mega: {e}"));
                    }
                    // revocata dal sito o scaduta: si riparte da password e codice
                    let _ = crate::vault::remove(&key);
                    client = mega_client()?;
                }
            }
        }
        let code = p.mfa.trim();
        let mfa = (!code.is_empty()).then_some(code);
        rt.block_on(client.login(p.user.trim(), password, mfa)).map_err(|e| mega_login_err(e, mfa.is_some()))?;
        if p.mfa_trusted && mfa.is_some() {
            let s = rt.block_on(client.serialize_session()).map_err(|e| e.to_string())?;
            crate::vault::set(&key, &s).map_err(|e| format!("cassaforte: {e}"))?;
        }
        Ok(Mega { rt, client: Arc::new(client), nodes: Mutex::new(None) })
    }

    fn nodes(&self) -> io::Result<Arc<mega::Nodes>> {
        let mut g = self.nodes.lock().unwrap();
        if let Some((n, t)) = g.as_ref()
            && t.elapsed() < MEGA_CACHE
        {
            return Ok(n.clone());
        }
        let n = Arc::new(self.rt.block_on(self.client.fetch_own_nodes()).map_err(mega_err)?);
        *g = Some((n.clone(), Instant::now()));
        Ok(n)
    }

    fn forget(&self) {
        *self.nodes.lock().unwrap() = None;
    }

    fn to_bin(&self, nodes: &mega::Nodes, node: &mega::Node) -> io::Result<()> {
        let bin = nodes.rubbish_bin().ok_or_else(|| io::Error::other("Mega: cestino non trovato"))?;
        self.rt.block_on(self.client.move_node(node, bin)).map_err(mega_err)
    }
}

impl Drive for Mega {
    fn stat(&self, path: &str) -> io::Result<RStat> {
        if mega_parts(path).is_empty() {
            return Ok(RStat { is_dir: true, mode: 0o755, ..Default::default() });
        }
        let nodes = self.nodes()?;
        Ok(mega_stat(mega_at(&nodes, path)?))
    }

    fn list(&self, path: &str) -> io::Result<Vec<(String, RStat)>> {
        let nodes = self.nodes()?;
        let dir = mega_at(&nodes, path)?;
        Ok(nodes.iter().filter(|n| n.parent() == Some(dir.handle())).map(|n| (n.name().to_string(), mega_stat(n))).collect())
    }

    fn read(&self, path: &str) -> io::Result<Box<dyn Read + Send>> {
        let nodes = self.nodes()?;
        let handle = mega_at(&nodes, path)?.handle().to_string();
        let (reader, writer) = sluice::pipe::pipe();
        let (rt, client) = (self.rt.handle().clone(), self.client.clone());
        let task = std::thread::spawn(move || {
            let node = nodes.get_node_by_handle(&handle).ok_or_else(|| io::Error::other("Mega: nodo sparito"))?;
            rt.block_on(client.download_node(node, writer)).map_err(mega_err)
        });
        Ok(Box::new(MegaReader { reader, task: Some(task) }))
    }

    fn writer(self: Arc<Self>, path: &str, size: Option<u64>) -> io::Result<Box<dyn crate::vfs::Sink>> {
        let (parent, name) = mega_split(path);
        let nodes = self.nodes()?;
        let parent = mega_at(&nodes, &parent)?.handle().to_string();
        let old = match mega_child(&nodes, &parent, &name) {
            Some(n) if !matches!(n.kind(), mega::NodeKind::File) => return Err(io::Error::new(io::ErrorKind::AlreadyExists, "esiste già una cartella con questo nome")),
            Some(n) => Some(n.handle().to_string()),
            None => None,
        };
        let mut sink = MegaSink { drive: self, parent, name, old, memory: Vec::new(), pipe: None, task: None, done: false };
        if let Some(s) = size {
            sink.start(s);
        }
        Ok(Box::new(sink))
    }

    fn mkdir(&self, path: &str) -> io::Result<()> {
        let (parent, name) = mega_split(path);
        let nodes = self.nodes()?;
        let p = mega_at(&nodes, &parent)?;
        if mega_child(&nodes, p.handle(), &name).is_some() {
            return Err(io::Error::new(io::ErrorKind::AlreadyExists, "esiste già"));
        }
        self.rt.block_on(self.client.create_folder(p, &name)).map_err(mega_err)?;
        self.forget();
        Ok(())
    }

    fn delete(&self, path: &str) -> io::Result<()> {
        let nodes = self.nodes()?;
        let node = mega_at(&nodes, path)?;
        self.to_bin(&nodes, node)?;
        // da dove veniva: Mega non lo ricorda per le app, lo ricorda RooTree
        mega_bin_note(node.handle(), Some(path));
        self.forget();
        Ok(())
    }

    fn delete_forever(&self, path: &str) -> io::Result<()> {
        let nodes = self.nodes()?;
        self.rt.block_on(self.client.delete_node(mega_at(&nodes, path)?)).map_err(mega_err)?;
        self.forget();
        Ok(())
    }

    /// Il cestino di Mega (Rubbish Bin).
    fn trash(&self, action: &str, item: &str) -> io::Result<String> {
        let nodes = self.nodes()?;
        let bin = nodes.rubbish_bin().ok_or_else(|| io::Error::other("Mega: cestino non trovato"))?;
        let notes = mega_bin_notes();
        let in_bin = |h: &str| nodes.get_node_by_handle(h).filter(|n| n.parent() == Some(bin.handle()));
        match action {
            "list" => {
                let out: Vec<Value> = nodes
                    .iter()
                    .filter(|n| n.parent() == Some(bin.handle()))
                    .map(|n| {
                        let note = &notes[n.handle()];
                        let dir = !matches!(n.kind(), mega::NodeKind::File);
                        json!({
                            "id": n.handle(), "name": n.name(),
                            "original": note["path"].as_str().unwrap_or(""),
                            "deleted": note["deleted"].as_i64().unwrap_or(0),
                            "size": if dir { 0 } else { n.size() }, "dir": dir,
                        })
                    })
                    .collect();
                Ok(Value::Array(out).to_string())
            }
            "restore" => {
                let node = in_bin(item).ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "non è più nel cestino"))?;
                let original = notes[item]["path"].as_str().map(str::to_string).unwrap_or_else(|| format!("/{}", node.name()));
                let (parent_path, name) = mega_split(&original);
                // la cartella di prima non c'è più: nella radice
                let parent = mega_at(&nodes, &parent_path).or_else(|_| mega_at(&nodes, "/"))?;
                let (stem, ext) = match name.rsplit_once('.') {
                    Some((a, b)) if !a.is_empty() && !matches!(node.kind(), mega::NodeKind::Folder) => (a.to_string(), format!(".{b}")),
                    _ => (name.clone(), String::new()),
                };
                let mut free = name.clone();
                let mut n = 1;
                while mega_child(&nodes, parent.handle(), &free).is_some() {
                    free = format!("{stem} ({n}){ext}");
                    n += 1;
                }
                if free != node.name() {
                    self.rt.block_on(self.client.rename_node(node, &free)).map_err(mega_err)?;
                }
                self.rt.block_on(self.client.move_node(node, parent)).map_err(mega_err)?;
                mega_bin_note(item, None);
                self.forget();
                Ok(String::new())
            }
            "delete" => {
                let node = in_bin(item).ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "non è più nel cestino"))?;
                self.rt.block_on(self.client.delete_node(node)).map_err(mega_err)?;
                mega_bin_note(item, None);
                self.forget();
                Ok(String::new())
            }
            "empty" => {
                for n in nodes.iter().filter(|n| n.parent() == Some(bin.handle())) {
                    self.rt.block_on(self.client.delete_node(n)).map_err(mega_err)?;
                    mega_bin_note(n.handle(), None);
                }
                self.forget();
                Ok(String::new())
            }
            a => Err(io::Error::other(format!("azione sconosciuta: {a}"))),
        }
    }

    fn rename(&self, from: &str, to: &str) -> io::Result<()> {
        let nodes = self.nodes()?;
        let node = mega_at(&nodes, from)?;
        let (from_parent, from_name) = mega_split(from);
        let (to_parent, name) = mega_split(to);
        let target = mega_at(&nodes, &to_parent)?;
        if mega_child(&nodes, target.handle(), &name).is_some() {
            return Err(io::Error::new(io::ErrorKind::AlreadyExists, "esiste già"));
        }
        if from_name != name {
            self.rt.block_on(self.client.rename_node(node, &name)).map_err(mega_err)?;
            self.forget();
        }
        if from_parent != to_parent {
            self.rt.block_on(self.client.move_node(node, target)).map_err(mega_err)?;
            self.forget();
        }
        Ok(())
    }
}

/// Lo scaricamento gira in un filo a parte e riempie il tubo; a fine tubo si
/// guarda com'è andata, così un guasto non sembra un file finito.
struct MegaReader {
    reader: sluice::pipe::PipeReader,
    task: Option<std::thread::JoinHandle<io::Result<()>>>,
}

impl Read for MegaReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        use futures::AsyncReadExt;
        let n = futures::executor::block_on(self.reader.read(buf))?;
        if n == 0
            && let Some(t) = self.task.take()
        {
            t.join().map_err(|_| io::Error::other("Mega: scaricamento interrotto"))??;
        }
        Ok(n)
    }
}

/// Con la dimensione nota i dati passano in un tubo verso il caricamento; senza,
/// si tengono in memoria e partono al flush (Mega vuole il totale in anticipo).
struct MegaSink {
    drive: Arc<Mega>,
    parent: String,
    name: String,
    old: Option<String>,
    memory: Vec<u8>,
    pipe: Option<sluice::pipe::PipeWriter>,
    task: Option<std::thread::JoinHandle<io::Result<()>>>,
    done: bool,
}

impl MegaSink {
    fn start(&mut self, size: u64) {
        let (reader, writer) = sluice::pipe::pipe();
        self.pipe = Some(writer);
        let (rt, client, drive) = (self.drive.rt.handle().clone(), self.drive.client.clone(), self.drive.clone());
        let (parent, name) = (self.parent.clone(), self.name.clone());
        self.task = Some(std::thread::spawn(move || {
            let nodes = drive.nodes()?;
            let p = nodes.get_node_by_handle(&parent).ok_or_else(|| io::Error::other("Mega: cartella sparita"))?;
            rt.block_on(client.upload_node(p, &name, size, reader, mega::LastModified::Now)).map_err(mega_err)
        }));
    }
}

impl Write for MegaSink {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        use futures::AsyncWriteExt;
        match self.pipe.as_mut() {
            Some(w) => futures::executor::block_on(w.write_all(data))?,
            None => self.memory.extend_from_slice(data),
        }
        Ok(data.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        use futures::AsyncWriteExt;
        if self.done {
            return Ok(());
        }
        if self.pipe.is_none() {
            let data = std::mem::take(&mut self.memory);
            self.start(data.len() as u64);
            futures::executor::block_on(self.pipe.as_mut().unwrap().write_all(&data))?;
        }
        let _ = futures::executor::block_on(self.pipe.as_mut().unwrap().close());
        self.pipe = None;
        if let Some(t) = self.task.take() {
            t.join().map_err(|_| io::Error::other("Mega: caricamento interrotto"))??;
        }
        self.done = true;
        self.drive.forget();
        // il vecchio file va nel cestino solo a caricamento riuscito
        if let Some(h) = self.old.take() {
            let nodes = self.drive.nodes()?;
            if let Some(n) = nodes.get_node_by_handle(&h) {
                self.drive.to_bin(&nodes, n)?;
                self.drive.forget();
            }
        }
        Ok(())
    }
}

impl crate::vfs::Sink for MegaSink {
    fn sync(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod prove {
    #[test]
    fn indirizzi_dropbox() {
        assert_eq!(super::dbx_path("/"), "");
        assert_eq!(super::dbx_path("/x/"), "/x");
    }

    #[test]
    fn percorsi_mega() {
        assert_eq!(super::mega_split("/a/b/c.txt"), ("/a/b".to_string(), "c.txt".to_string()));
        assert_eq!(super::mega_split("/c.txt"), ("/".to_string(), "c.txt".to_string()));
        assert_eq!(super::mega_parts("//a//b/"), vec!["a", "b"]);
    }
}
