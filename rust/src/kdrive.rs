//! kDrive di Infomaniak (0.9.4), con l'API REST e un token personale.
//!
//! Perché l'API e non WebDAV: con kSuite gratuita WebDAV non c'è (FAQ
//! Infomaniak, utente 14/11/2026). Il token si crea nel Manager con il
//! permesso «drive» e si incolla nella procedura: niente app da registrare.
//! Documentazione: developer.infomaniak.com/docs/api (letta il 15/11/2026;
//! le schede delle operazioni stanno nell'attributo data-page delle pagine).
//!
//! kDrive lavora per identificatore, non per percorso: RooTree ricorda la
//! corrispondenza percorso → id (dagli elenchi e dalle ricerche per nome) e la
//! aggiorna a ogni modifica. Eliminare = cestino di kDrive.
//!
//! ⛔ 60 richieste al minuto, non aumentabili (testo di Infomaniak incollato
//! dall'utente il 14/11): un limitatore tiene al più 55 richieste in 60 s, e
//! un «troppe richieste» (429) si aspetta e si riprova da solo.

use std::collections::{HashMap, VecDeque};
use std::io::{self, Read, Write};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Deserialize;
use serde_json::{Value, json};

use crate::cloud::Drive;
use crate::http;
use crate::remote::RStat;

const API: &str = "https://api.infomaniak.com";
/// la radice di un drive
const ROOT_ID: u64 = 1;
/// fino a qui una sola richiesta; sopra, sessione a pezzi
const CHUNK: usize = 8 << 20;
const WINDOW: Duration = Duration::from_secs(60);
const MAX_IN_WINDOW: usize = 55;
/// per quanto un elenco ricordato vale senza richiederlo
const LIST_TTL: Duration = Duration::from_secs(20);

#[derive(Deserialize, Default)]
#[serde(default)]
struct Profile {
    /// l'id del drive; vuoto = il primo drive dell'account
    drive: String,
}

/// Il limitatore: al più MAX_IN_WINDOW richieste in WINDOW (finestra mobile).
#[derive(Default)]
pub struct Pace {
    sent: Mutex<VecDeque<Instant>>,
}

impl Pace {
    /// Aspetta il proprio turno. Rende quanto ha aspettato (per i test).
    pub fn wait(&self) -> Duration {
        let mut waited = Duration::ZERO;
        loop {
            let mut q = self.sent.lock().unwrap();
            let now = Instant::now();
            while q.front().is_some_and(|t| now.duration_since(*t) >= WINDOW) {
                q.pop_front();
            }
            if q.len() < MAX_IN_WINDOW {
                q.push_back(now);
                return waited;
            }
            let until = *q.front().unwrap() + WINDOW;
            drop(q);
            let d = until.saturating_duration_since(now) + Duration::from_millis(20);
            std::thread::sleep(d);
            waited += d;
        }
    }
}

#[derive(Clone)]
struct Node {
    id: u64,
    st: RStat,
}

pub struct KDrive {
    token: String,
    drive: u64,
    pace: Pace,
    /// percorso → (id, stato)
    nodes: Mutex<HashMap<String, Node>>,
    /// cartella → (quando, elenco)
    lists: Mutex<HashMap<String, (Instant, Vec<(String, RStat)>)>>,
}

fn st_of(e: &Value) -> RStat {
    let dir = e["type"] == "dir";
    RStat {
        is_dir: dir,
        is_file: !dir,
        is_link: false,
        size: e["size"].as_u64().unwrap_or(0),
        mode: if dir { 0o755 } else { 0o644 },
        mtime: e["last_modified_at"].as_i64().or(e["updated_at"].as_i64()).unwrap_or(0),
    }
}

pub(crate) fn norm(p: &str) -> String {
    let t = p.trim_end_matches('/');
    if t.is_empty() { "/".into() } else if t.starts_with('/') { t.into() } else { format!("/{t}") }
}

pub(crate) fn split(p: &str) -> (String, String) {
    let p = norm(p);
    match p.rfind('/') {
        Some(0) => ("/".into(), p[1..].into()),
        Some(i) => (p[..i].into(), p[i + 1..].into()),
        None => ("/".into(), p),
    }
}

pub(crate) fn join(dir: &str, name: &str) -> String {
    if dir == "/" { format!("/{name}") } else { format!("{dir}/{name}") }
}

fn not_found() -> io::Error {
    io::Error::new(io::ErrorKind::NotFound, "non esiste")
}

/// Un errore dell'API in chiaro: {"result":"error","error":{"code":…,"description":…}}.
fn api_error(status: u16, body: &str) -> io::Error {
    let v: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    let code = v["error"]["code"].as_str().unwrap_or("");
    let desc = v["error"]["description"].as_str().unwrap_or("");
    match (status, code) {
        (_, c) if c.contains("not_found") => not_found(),
        (_, c) if c.contains("already_exists") || c.contains("conflict") => io::Error::new(io::ErrorKind::AlreadyExists, "esiste già"),
        (401, _) => io::Error::new(io::ErrorKind::PermissionDenied, "AUTH:token non valido, scaduto o revocato (si crea di nuovo nel Manager di Infomaniak)"),
        (403, _) if !code.is_empty() => io::Error::new(io::ErrorKind::PermissionDenied, format!("kDrive: permesso negato ({code}{})", if desc.is_empty() { String::new() } else { format!(": {desc}") })),
        _ if !code.is_empty() => io::Error::other(format!("kDrive: {code}{}", if desc.is_empty() { String::new() } else { format!(" — {desc}") })),
        _ => http::status_error(status, body),
    }
}

/// L'ID del drive da quello che l'utente ha scritto: il numero, o l'indirizzo
/// intero di kDrive copiato dal browser (…/kdrive/app/drive/1234567/files/5).
pub fn drive_id(s: &str) -> Option<u64> {
    let s = s.trim();
    if let Ok(n) = s.parse() {
        return Some(n);
    }
    let rest = &s[s.find("/drive/")? + "/drive/".len()..];
    rest.split(|c: char| !c.is_ascii_digit()).next()?.parse().ok()
}

pub fn connect(profile_json: &str, secret: &str) -> Result<Arc<dyn Drive>, String> {
    let p: Profile = serde_json::from_str(profile_json).map_err(|e| format!("profilo non valido: {e}"))?;
    let token = secret.trim().to_string();
    if token.is_empty() {
        return Err("AUTH:manca il token di kDrive".into());
    }
    let mut k = KDrive { token, drive: 0, pace: Pace::default(), nodes: Mutex::new(HashMap::new()), lists: Mutex::new(HashMap::new()) };
    k.drive = match p.drive.trim() {
        "" => k.first_drive().map_err(|e| e.to_string())?,
        d => drive_id(d).ok_or_else(|| format!("ID del drive non valido: «{d}» (è il numero dopo «/drive/» nell'indirizzo di kDrive, es. …/kdrive/app/drive/1234567/files)"))?,
    };
    // prova subito: token valido e drive raggiungibile
    let root = k.get(&format!("/3/drive/{}/files/{ROOT_ID}", k.drive)).map_err(|e| e.to_string())?;
    k.nodes.lock().unwrap().insert("/".into(), Node { id: ROOT_ID, st: st_of(&root["data"]) });
    Ok(Arc::new(k))
}

impl KDrive {
    /// Una richiesta col token, al ritmo permesso; un 429 si aspetta e si riprova.
    fn send(&self, f: &dyn Fn(&str) -> Result<http::Resp, ureq::Error>) -> io::Result<http::Resp> {
        let bearer = format!("Bearer {}", self.token);
        for attempt in 0..6 {
            self.pace.wait();
            let r = f(&bearer).map_err(http::net)?;
            if r.status().as_u16() != 429 {
                return Ok(r);
            }
            let after = r.headers().get("retry-after").and_then(|v| v.to_str().ok()).and_then(|v| v.parse::<u64>().ok()).unwrap_or(10 + attempt * 10);
            std::thread::sleep(Duration::from_secs(after.min(90)));
        }
        Err(io::Error::other("kDrive: troppe richieste, riprova fra un minuto"))
    }

    fn json(&self, r: http::Resp) -> io::Result<Value> {
        let mut r = r;
        let status = r.status().as_u16();
        let body = r.body_mut().read_to_string().unwrap_or_default();
        if !(200..300).contains(&status) {
            return Err(api_error(status, &body));
        }
        let v: Value = if body.trim().is_empty() { Value::Null } else { serde_json::from_str(&body).map_err(io::Error::other)? };
        if v["result"] == "error" {
            return Err(api_error(status, &body));
        }
        Ok(v)
    }

    fn get(&self, path: &str) -> io::Result<Value> {
        let url = format!("{API}{path}");
        let r = self.send(&|b| http::agent().get(&url).header("Authorization", b).call())?;
        self.json(r)
    }

    fn post(&self, path: &str, body: Value) -> io::Result<Value> {
        let url = format!("{API}{path}");
        let s = body.to_string();
        let r = self.send(&|b| http::agent().post(&url).header("Authorization", b).header("Content-Type", "application/json").send(s.as_str()))?;
        self.json(r)
    }

    fn delete_req(&self, path: &str) -> io::Result<Value> {
        let url = format!("{API}{path}");
        let r = self.send(&|b| http::agent().delete(&url).header("Authorization", b).call())?;
        self.json(r)
    }

    fn post_bytes(&self, path: &str, data: &[u8]) -> io::Result<Value> {
        let url = format!("{API}{path}");
        let r = self.send(&|b| http::agent().post(&url).header("Authorization", b).header("Content-Type", "application/octet-stream").send(data))?;
        self.json(r)
    }

    /// Il primo drive del token. Con il solo permesso «drive» l'elenco degli
    /// account (/1/accounts, permesso «accounts») è vietato: utente 15/11,
    /// «This method require this specific scope: "accounts"». Si prova prima
    /// /2/drive senza account (la documentazione dà account_id obbligatorio),
    /// poi con gli account; se niente va, l'ID del drive lo scrive l'utente.
    fn first_drive(&self) -> io::Result<u64> {
        let pick = |v: &Value| v["data"].as_array().and_then(|l| l.iter().find_map(|d| d["id"].as_u64()));
        if let Ok(v) = self.get("/2/drive")
            && let Some(id) = pick(&v)
        {
            return Ok(id);
        }
        if let Ok(acc) = self.get("/1/accounts") {
            for a in acc["data"].as_array().into_iter().flatten().filter_map(|a| a["id"].as_u64()) {
                if let Ok(v) = self.get(&format!("/2/drive?account_id={a}"))
                    && let Some(id) = pick(&v)
                {
                    return Ok(id);
                }
            }
        }
        Err(io::Error::other(
            "DRIVEID:con il solo permesso «drive» RooTree non trova il drive da sola: apri kDrive nel browser, copia l'indirizzo dalla barra e incollalo nel campo «Indirizzo del tuo kDrive» di questa connessione.",
        ))
    }

    fn d(&self) -> u64 {
        self.drive
    }

    fn remember(&self, path: &str, e: &Value) {
        if let Some(id) = e["id"].as_u64() {
            self.nodes.lock().unwrap().insert(norm(path), Node { id, st: st_of(e) });
        }
    }

    /// Dimentica un percorso, tutto quello che c'è sotto, e l'elenco della cartella sopra.
    fn forget(&self, path: &str) {
        let p = norm(path);
        let pre = format!("{}/", p.trim_end_matches('/'));
        self.nodes.lock().unwrap().retain(|k, _| *k != p && !k.starts_with(&pre));
        let (dir, _) = split(&p);
        let mut l = self.lists.lock().unwrap();
        l.retain(|k, _| *k != p && !k.starts_with(&pre));
        l.remove(&dir);
    }

    fn node(&self, path: &str) -> io::Result<Node> {
        let p = norm(path);
        if let Some(n) = self.nodes.lock().unwrap().get(&p).cloned() {
            return Ok(n);
        }
        if p == "/" {
            return Ok(Node { id: ROOT_ID, st: RStat { is_dir: true, mode: 0o755, ..Default::default() } });
        }
        let (dir, name) = split(&p);
        let parent = self.node(&dir)?;
        if !parent.st.is_dir {
            return Err(not_found());
        }
        let v = self.get(&format!("/3/drive/{}/files/{}/name?name={}", self.d(), parent.id, http::pct(&name, false)))?;
        let e = &v["data"];
        if e["id"].as_u64().is_none() || matches!(e["status"].as_str(), Some("trashed" | "trash_inherited")) {
            return Err(not_found());
        }
        self.remember(&p, e);
        Ok(Node { id: e["id"].as_u64().unwrap(), st: st_of(e) })
    }

    fn id(&self, path: &str) -> io::Result<u64> {
        self.node(path).map(|n| n.id)
    }

    fn list_fresh(&self, path: &str) -> io::Result<Vec<(String, RStat)>> {
        let p = norm(path);
        let dir = self.id(&p)?;
        let mut out = Vec::new();
        let mut cursor = String::new();
        loop {
            let mut url = format!("/3/drive/{}/files/{dir}/files?limit=1000", self.d());
            if !cursor.is_empty() {
                url.push_str(&format!("&cursor={}", http::pct(&cursor, false)));
            }
            let v = self.get(&url)?;
            for e in v["data"].as_array().into_iter().flatten() {
                if matches!(e["status"].as_str(), Some("trashed" | "trash_inherited")) {
                    continue;
                }
                let name = e["name"].as_str().unwrap_or("").to_string();
                if name.is_empty() {
                    continue;
                }
                self.remember(&join(&p, &name), e);
                out.push((name, st_of(e)));
            }
            if v["has_more"] != true {
                break;
            }
            cursor = v["cursor"].as_str().unwrap_or("").to_string();
            if cursor.is_empty() {
                break;
            }
        }
        self.lists.lock().unwrap().insert(p, (Instant::now(), out.clone()));
        Ok(out)
    }

    /// Carica un file in una richiesta sola (fino a CHUNK). `conflict=version`:
    /// se c'è già, diventa la sua nuova versione (= sovrascrivere).
    fn upload_whole(&self, path: &str, data: &[u8]) -> io::Result<()> {
        let (dir, name) = split(path);
        let parent = self.id(&dir)?;
        let url = format!(
            "/3/drive/{}/upload?directory_id={parent}&file_name={}&total_size={}&conflict=version",
            self.d(),
            http::pct(&name, false),
            data.len()
        );
        let v = self.post_bytes(&url, data)?;
        self.forget(path);
        self.remember(path, &v["data"]);
        Ok(())
    }

    fn trash_list(&self) -> io::Result<String> {
        let mut out = Vec::new();
        let mut cursor = String::new();
        loop {
            let mut url = format!("/3/drive/{}/trash?limit=1000", self.d());
            if !cursor.is_empty() {
                url.push_str(&format!("&cursor={}", http::pct(&cursor, false)));
            }
            let v = self.get(&url)?;
            for e in v["data"].as_array().into_iter().flatten() {
                let name = e["name"].as_str().unwrap_or("");
                let path = e["path"].as_str().unwrap_or(name);
                out.push(json!({
                    "id": e["id"].as_u64().unwrap_or(0).to_string(),
                    "name": name,
                    "original": path,
                    "deleted": e["deleted_at"].as_i64().unwrap_or(0),
                    "size": e["size"].as_u64().unwrap_or(0),
                    "dir": e["type"] == "dir",
                }));
            }
            if v["has_more"] != true {
                break;
            }
            cursor = v["cursor"].as_str().unwrap_or("").to_string();
            if cursor.is_empty() {
                break;
            }
        }
        Ok(Value::Array(out).to_string())
    }
}

impl Drive for KDrive {
    fn stat(&self, path: &str) -> io::Result<RStat> {
        self.node(path).map(|n| n.st)
    }

    fn list(&self, path: &str) -> io::Result<Vec<(String, RStat)>> {
        let p = norm(path);
        if let Some((t, l)) = self.lists.lock().unwrap().get(&p)
            && t.elapsed() < LIST_TTL
        {
            return Ok(l.clone());
        }
        self.list_fresh(&p)
    }

    fn read(&self, path: &str) -> io::Result<Box<dyn Read + Send>> {
        let id = self.id(path)?;
        let url = format!("{API}/2/drive/{}/files/{id}/download", self.d());
        // può rispondere 302 verso l'archivio vero: ureq segue il rinvio
        let mut r = self.send(&|b| http::agent().get(&url).header("Authorization", b).call())?;
        let status = r.status().as_u16();
        if status != 200 {
            let body = r.body_mut().read_to_string().unwrap_or_default();
            return Err(api_error(status, &body));
        }
        Ok(Box::new(r.into_body().into_reader()))
    }

    fn writer(self: Arc<Self>, path: &str, size: Option<u64>) -> io::Result<Box<dyn crate::vfs::Sink>> {
        Ok(Box::new(KSink { drive: self, path: norm(path), size, buf: Vec::new(), session: None, chunk: 0, sent: 0, done: false }))
    }

    fn mkdir(&self, path: &str) -> io::Result<()> {
        let (dir, name) = split(path);
        let parent = self.id(&dir)?;
        let v = self.post(&format!("/3/drive/{}/files/{parent}/directory", self.d()), json!({ "name": name }))?;
        self.forget(path);
        self.remember(path, &v["data"]);
        Ok(())
    }

    /// Nel cestino di kDrive (si recupera da RooTree o dalla web app).
    fn delete(&self, path: &str) -> io::Result<()> {
        let id = self.id(path)?;
        self.delete_req(&format!("/2/drive/{}/files/{id}", self.d()))?;
        self.forget(path);
        Ok(())
    }

    fn delete_forever(&self, path: &str) -> io::Result<()> {
        let id = self.id(path)?;
        self.delete_req(&format!("/2/drive/{}/files/{id}", self.d()))?;
        self.forget(path);
        self.delete_req(&format!("/2/drive/{}/trash/{id}", self.d())).map(|_| ())
    }

    fn rename(&self, from: &str, to: &str) -> io::Result<()> {
        let (fdir, _) = split(from);
        let (tdir, tname) = split(to);
        let id = self.id(from)?;
        // la destinazione occupata: come negli altri servizi, è un errore
        // (chi sovrascrive l'ha già tolta)
        if self.node(to).is_ok() {
            return Err(io::Error::new(io::ErrorKind::AlreadyExists, "esiste già"));
        }
        if fdir == tdir {
            self.post(&format!("/2/drive/{}/files/{id}/rename", self.d()), json!({ "name": tname }))?;
        } else {
            let dest = self.id(&tdir)?;
            self.post(&format!("/3/drive/{}/files/{id}/move/{dest}", self.d()), json!({ "name": tname, "conflict": "error" }))?;
        }
        self.forget(from);
        self.forget(to);
        Ok(())
    }

    /// Il cestino di kDrive: elenco, ripristino (dov'era), eliminazione definitiva, svuotamento.
    fn trash(&self, action: &str, item: &str) -> io::Result<String> {
        let d = self.d();
        let r = match action {
            "list" => return self.trash_list(),
            "restore" => self.post(&format!("/2/drive/{d}/trash/{item}/restore"), json!({})),
            "delete" => self.delete_req(&format!("/2/drive/{d}/trash/{item}")),
            "empty" => self.delete_req(&format!("/2/drive/{d}/trash")),
            _ => return Err(io::Error::other(format!("azione sconosciuta: {action}"))),
        };
        r?;
        // un ripristino rimette file dove l'elenco ricordato non li ha
        self.lists.lock().unwrap().clear();
        Ok(String::new())
    }
}

/// Caricamento mentre si scrive: un file piccolo (o di dimensione ignota) in
/// una richiesta sola alla fine; uno grande in una sessione a pezzi da 8 MB,
/// chiusa da flush. Senza flush (annullato) la sessione si cancella.
struct KSink {
    drive: Arc<KDrive>,
    path: String,
    size: Option<u64>,
    buf: Vec<u8>,
    session: Option<String>,
    chunk: u64,
    sent: u64,
    done: bool,
}

impl KSink {
    fn start(&mut self, total: u64) -> io::Result<()> {
        let k = &self.drive;
        let (dir, name) = split(&self.path);
        let parent = k.id(&dir)?;
        let chunks = total.div_ceil(CHUNK as u64).max(1);
        let v = k.post(
            &format!("/3/drive/{}/upload/session/start", k.d()),
            json!({ "directory_id": parent, "file_name": name, "total_size": total, "total_chunks": chunks, "conflict": "version" }),
        )?;
        let token = v["data"]["token"].as_str().unwrap_or("").to_string();
        if token.is_empty() {
            return Err(io::Error::other("kDrive: la sessione di caricamento non è partita"));
        }
        self.session = Some(token);
        Ok(())
    }

    fn push(&mut self, data: &[u8]) -> io::Result<()> {
        let k = self.drive.clone();
        let token = self.session.clone().unwrap_or_default();
        self.chunk += 1;
        k.post_bytes(&format!("/3/drive/{}/upload/session/{token}/chunk?chunk_number={}&chunk_size={}", k.d(), self.chunk, data.len()), data)?;
        self.sent += data.len() as u64;
        Ok(())
    }
}

impl Write for KSink {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        self.buf.extend_from_slice(data);
        // a pezzi solo se la dimensione è nota e grande: la sessione vuole il totale
        if let Some(total) = self.size
            && total > CHUNK as u64
        {
            while self.buf.len() >= CHUNK {
                if self.session.is_none() {
                    self.start(total)?;
                }
                let rest = self.buf.split_off(CHUNK);
                let piece = std::mem::replace(&mut self.buf, rest);
                self.push(&piece)?;
            }
        }
        Ok(data.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        if self.done {
            return Ok(());
        }
        let k = self.drive.clone();
        match self.session.clone() {
            None => {
                let data = std::mem::take(&mut self.buf);
                k.upload_whole(&self.path, &data)?;
            }
            Some(token) => {
                if !self.buf.is_empty() {
                    let data = std::mem::take(&mut self.buf);
                    self.push(&data)?;
                }
                let v = k.post(&format!("/3/drive/{}/upload/session/{token}/finish", k.d()), json!({}))?;
                k.forget(&self.path);
                k.remember(&self.path, &v["data"]["file"]);
            }
        }
        self.done = true;
        Ok(())
    }
}

impl Drop for KSink {
    fn drop(&mut self) {
        // annullato a metà: la sessione aperta si cancella (se no resta 4 ore)
        if !self.done
            && let Some(token) = self.session.take()
        {
            let _ = self.drive.delete_req(&format!("/3/drive/{}/upload/session/{token}", self.drive.d()));
        }
    }
}

impl crate::vfs::Sink for KSink {
    fn sync(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
pub(crate) mod t {
    pub(crate) use super::{Pace, join, norm, split};
}
