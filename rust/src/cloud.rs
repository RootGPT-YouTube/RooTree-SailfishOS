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
}

pub fn is_cloud_type(kind: &str) -> bool {
    matches!(kind, "dropbox" | "onedrive")
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
    #[serde(rename = "clientSecret")]
    client_secret: String,
}

/// Collega un profilo cloud col suo token di rinnovo (`secret`).
pub fn connect(profile_json: &str, secret: &str) -> Result<Arc<dyn Drive>, String> {
    let p: Profile = serde_json::from_str(profile_json).map_err(|e| format!("profilo non valido: {e}"))?;
    if secret.is_empty() {
        return Err("LOGIN:serve l'accesso dal browser".into());
    }
    let auth = Auth::new(&p.kind, &p.client_id, &p.client_secret, secret);
    match p.kind.as_str() {
        "dropbox" => {
            let d = Dropbox { auth };
            // prova subito: token valido e account raggiungibile
            d.rpc("users/get_current_account", Value::Null).map_err(|e| e.to_string())?;
            let _ = p.id;
            Ok(Arc::new(d))
        }
        "onedrive" => {
            let d = OneDrive { auth };
            d.get_json(&format!("{GRAPH}/me/drive")).map_err(|e| e.to_string())?;
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
    client_secret: String,
    refresh: String,
    access: Mutex<Option<(String, Instant)>>,
}

impl Auth {
    fn new(kind: &str, client_id: &str, client_secret: &str, refresh: &str) -> Self {
        Auth { kind: kind.into(), client_id: client_id.into(), client_secret: client_secret.into(), refresh: refresh.into(), access: Mutex::new(None) }
    }

    fn token(&self, force: bool) -> io::Result<String> {
        let mut g = self.access.lock().unwrap();
        if !force
            && let Some((t, until)) = g.as_ref()
            && Instant::now() < *until
        {
            return Ok(t.clone());
        }
        let t = oauth::refresh(&self.kind, &self.client_id, &self.client_secret, &self.refresh).map_err(io::Error::other)?;
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

// ------------------------------------------------------------------ OneDrive
// Microsoft Graph v1.0, indirizzi per percorso:
// https://learn.microsoft.com/graph/onedrive-addressing-driveitems
// Caricamento: fino a 4 MB un PUT …:/content; sopra, una sessione
// (createUploadSession) e pezzi multipli di 320 KiB sull'uploadUrl, che è già
// autorizzato (senza Bearer), con Content-Range che vuole il TOTALE.

const GRAPH: &str = "https://graph.microsoft.com/v1.0";
const OD_SIMPLE_MAX: u64 = 4 << 20;
/// 32 × 320 KiB = 10 MiB: multiplo di 320 KiB, sotto i 60 MiB massimi
const OD_CHUNK: usize = 32 * 320 * 1024;

pub struct OneDrive {
    auth: Auth,
}

/// L'elemento per percorso: «/me/drive/root» o «/me/drive/root:/a/b:» (con
/// i «:» per poterci attaccare /children, /content…).
fn od_item(path: &str) -> String {
    let p = path.trim_end_matches('/');
    if p.is_empty() { format!("{GRAPH}/me/drive/root") } else { format!("{GRAPH}/me/drive/root:{}:", http::pct(p, true)) }
}

/// Per i PATCH di spostamento: il genitore come «/drive/root:/a» (radice «/drive/root:»).
fn od_parent_ref(parent: &str) -> String {
    let p = parent.trim_end_matches('/');
    format!("/drive/root:{p}")
}

fn od_stat(e: &Value) -> RStat {
    let dir = !e["folder"].is_null();
    RStat {
        is_dir: dir,
        is_file: !dir,
        is_link: false,
        size: if dir { 0 } else { e["size"].as_u64().unwrap_or(0) },
        mode: if dir { 0o755 } else { 0o644 },
        mtime: e["lastModifiedDateTime"].as_str().map(http::rfc3339).unwrap_or(0),
    }
}

fn split_parent(path: &str) -> (String, String) {
    let p = path.trim_end_matches('/');
    match p.rsplit_once('/') {
        Some((dir, name)) => (if dir.is_empty() { "/".into() } else { dir.into() }, name.into()),
        None => ("/".into(), p.into()),
    }
}

impl OneDrive {
    fn get_json(&self, url: &str) -> io::Result<Value> {
        let r = self.auth.send(&|bearer| http::agent().get(url).header("Authorization", bearer).call())?;
        ok_json(r)
    }

    fn send_json(&self, method: &str, url: &str, body: Value) -> io::Result<Value> {
        let b = body.to_string();
        let r = self.auth.send(&|bearer| {
            let req = match method {
                "PATCH" => http::agent().patch(url),
                _ => http::agent().post(url),
            };
            req.header("Authorization", bearer).header("Content-Type", "application/json").send(b.as_str())
        })?;
        ok_json(r)
    }
}

impl Drive for OneDrive {
    fn stat(&self, path: &str) -> io::Result<RStat> {
        let p = path.trim_end_matches('/');
        let url = if p.is_empty() { format!("{GRAPH}/me/drive/root") } else { format!("{GRAPH}/me/drive/root:{}", http::pct(p, true)) };
        Ok(od_stat(&self.get_json(&url)?))
    }

    fn list(&self, path: &str) -> io::Result<Vec<(String, RStat)>> {
        let mut out = Vec::new();
        let mut url = format!("{}/children?$top=200&$select=name,size,folder,file,lastModifiedDateTime", od_item(path));
        loop {
            let v = self.get_json(&url)?;
            for e in v["value"].as_array().into_iter().flatten() {
                out.push((e["name"].as_str().unwrap_or("").to_string(), od_stat(e)));
            }
            match v["@odata.nextLink"].as_str() {
                Some(next) => url = next.to_string(),
                None => break,
            }
        }
        Ok(out)
    }

    fn read(&self, path: &str) -> io::Result<Box<dyn Read + Send>> {
        // …/content risponde con un rinvio a un indirizzo già autorizzato; ureq lo segue
        let url = format!("{}/content", od_item(path));
        let mut r = self.auth.send(&|bearer| http::agent().get(&url).header("Authorization", bearer).call())?;
        let status = r.status().as_u16();
        if status != 200 {
            let body = body_string(&mut r);
            return Err(http::status_error(status, &body));
        }
        Ok(Box::new(r.into_body().into_reader()))
    }

    fn writer(self: Arc<Self>, path: &str, size: Option<u64>) -> io::Result<Box<dyn crate::vfs::Sink>> {
        Ok(Box::new(OneDriveSink { drive: self, path: path.trim_end_matches('/').to_string(), size, buf: Vec::new(), upload_url: None, offset: 0, done: false }))
    }

    fn mkdir(&self, path: &str) -> io::Result<()> {
        let (parent, name) = split_parent(path);
        let body = json!({ "name": name, "folder": {}, "@microsoft.graph.conflictBehavior": "fail" });
        self.send_json("POST", &format!("{}/children", od_item(&parent)), body).map(|_| ())
    }

    fn delete(&self, path: &str) -> io::Result<()> {
        let p = path.trim_end_matches('/');
        let url = format!("{GRAPH}/me/drive/root:{}", http::pct(p, true));
        let r = self.auth.send(&|bearer| http::agent().delete(&url).header("Authorization", bearer).call())?;
        ok_json(r).map(|_| ())
    }

    fn rename(&self, from: &str, to: &str) -> io::Result<()> {
        let f = from.trim_end_matches('/');
        let (parent, name) = split_parent(to);
        let body = json!({ "name": name, "parentReference": { "path": od_parent_ref(&parent) }, "@microsoft.graph.conflictBehavior": "fail" });
        self.send_json("PATCH", &format!("{GRAPH}/me/drive/root:{}", http::pct(f, true)), body).map(|_| ())
    }
}

/// Fino a 4 MB tutto in memoria e un PUT a flush; sopra (dimensione nota) una
/// sessione, con i pezzi che partono mentre si scrive.
struct OneDriveSink {
    drive: Arc<OneDrive>,
    path: String,
    size: Option<u64>,
    buf: Vec<u8>,
    upload_url: Option<String>,
    offset: u64,
    done: bool,
}

impl OneDriveSink {
    fn big(&self) -> bool {
        self.size.is_some_and(|s| s > OD_SIMPLE_MAX)
    }

    fn session(&mut self) -> io::Result<String> {
        if let Some(u) = &self.upload_url {
            return Ok(u.clone());
        }
        let url = format!("{GRAPH}/me/drive/root:{}:/createUploadSession", http::pct(&self.path, true));
        let v = self.drive.send_json("POST", &url, json!({ "item": { "@microsoft.graph.conflictBehavior": "replace" } }))?;
        let u = v["uploadUrl"].as_str().ok_or_else(|| io::Error::other("OneDrive: sessione di caricamento senza indirizzo"))?.to_string();
        self.upload_url = Some(u.clone());
        Ok(u)
    }

    fn push(&mut self, data: &[u8]) -> io::Result<()> {
        let total = self.size.unwrap_or(0);
        let url = self.session()?;
        let end = self.offset + data.len() as u64 - 1;
        // ⛔ l'uploadUrl è già autorizzato: niente intestazione Authorization
        let mut r = http::agent()
            .put(&url)
            .header("Content-Range", format!("bytes {}-{end}/{total}", self.offset))
            .send(data)
            .map_err(http::net)?;
        let status = r.status().as_u16();
        if !(200..300).contains(&status) {
            let body = body_string(&mut r);
            return Err(http::status_error(status, &body));
        }
        self.offset += data.len() as u64;
        Ok(())
    }
}

impl Write for OneDriveSink {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        self.buf.extend_from_slice(data);
        if self.big() && self.buf.len() >= OD_CHUNK {
            let chunk: Vec<u8> = self.buf.drain(..OD_CHUNK).collect();
            self.push(&chunk)?;
        }
        Ok(data.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        if self.done {
            return Ok(());
        }
        if self.big() {
            let rest = std::mem::take(&mut self.buf);
            if !rest.is_empty() {
                self.push(&rest)?;
            }
            if Some(self.offset) != self.size {
                return Err(io::Error::other(format!("OneDrive: caricati {} byte su {:?}", self.offset, self.size)));
            }
        } else {
            let data = std::mem::take(&mut self.buf);
            let url = format!("{GRAPH}/me/drive/root:{}:/content", http::pct(&self.path, true));
            let r = self.drive.auth.send(&|bearer| {
                http::agent().put(&url).header("Authorization", bearer).header("Content-Type", "application/octet-stream").send(&data[..])
            })?;
            ok_json(r)?;
        }
        self.done = true;
        Ok(())
    }
}

impl crate::vfs::Sink for OneDriveSink {
    fn sync(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod prove {
    #[test]
    fn indirizzi_onedrive() {
        assert_eq!(super::od_item("/"), "https://graph.microsoft.com/v1.0/me/drive/root");
        assert_eq!(super::od_item("/Foto mare/è"), "https://graph.microsoft.com/v1.0/me/drive/root:/Foto%20mare/%C3%A8:");
        assert_eq!(super::od_parent_ref("/"), "/drive/root:");
        assert_eq!(super::od_parent_ref("/Documenti"), "/drive/root:/Documenti");
        assert_eq!(super::split_parent("/a/b.txt"), ("/a".to_string(), "b.txt".to_string()));
        assert_eq!(super::split_parent("/b.txt"), ("/".to_string(), "b.txt".to_string()));
        assert_eq!(super::dbx_path("/"), "");
        assert_eq!(super::dbx_path("/x/"), "/x");
    }
}
