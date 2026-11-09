//! WebDAV e Nextcloud/ownCloud (Fase 5, step 5). Implementa `Drive`, quindi
//! copia, coda, Annulla e conflitti valgono come per gli altri remoti.
//!
//! Il segreto della connessione è la password (per Nextcloud meglio una
//! «password per app»), nella cassaforte. Si manda solo con Basic: sull'HTTP
//! in chiaro viaggia leggibile, per questo HTTPS è il valore predefinito.

use std::io::{self, Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
use std::sync::{Arc, OnceLock};
use std::thread::JoinHandle;
use std::time::Duration;

use base64::Engine;
use quick_xml::Reader;
use quick_xml::events::Event;
use serde::Deserialize;
use ureq::Agent;
use ureq::SendBody;
use ureq::http::{Method, Request};
use ureq::tls::{TlsConfig, TlsProvider};

use crate::cloud::Drive;
use crate::http;
use crate::remote::RStat;

fn agent() -> &'static Agent {
    static A: OnceLock<Agent> = OnceLock::new();
    A.get_or_init(|| {
        Agent::config_builder()
            .http_status_as_error(false)
            // PROPFIND, MKCOL e MOVE non sono metodi standard per ureq
            .allow_non_standard_methods(true)
            .timeout_connect(Some(Duration::from_secs(15)))
            // un server piccolo può metterci parecchio a chiudere un file grosso
            .timeout_recv_response(Some(Duration::from_secs(600)))
            .tls_config(TlsConfig::builder().provider(TlsProvider::NativeTls).build())
            .build()
            .into()
    })
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Profile {
    #[serde(rename = "type")]
    kind: String,
    host: String,
    port: u16,
    user: String,
    path: String,
    secure: Option<bool>,
}

pub struct WebDav {
    /// `http[s]://server[:porta]/percorso`, senza barra finale
    root: String,
    /// il percorso di `root`, in chiaro, per riconoscere le voci nelle risposte
    root_path: String,
    auth: String,
    /// Nextcloud: `…/remote.php/dav/trashbin/<utente>`
    trash_root: Option<String>,
}

/// Da un indirizzo scritto come capita a (server con porta, percorso).
fn split_host(host: &str, secure: bool, port: u16) -> (String, String) {
    let h = host.trim();
    let h = h.strip_prefix("https://").or_else(|| h.strip_prefix("http://")).unwrap_or(h);
    let (name, sub) = match h.split_once('/') {
        Some((n, s)) => (n, format!("/{}", s.trim_matches('/'))),
        None => (h, String::new()),
    };
    let sub = if sub == "/" { String::new() } else { sub };
    let standard = if secure { 443 } else { 80 };
    if port == 0 || port == standard || name.contains(':') {
        (name.to_string(), sub)
    } else {
        (format!("{name}:{port}"), sub)
    }
}

fn clean_path(p: &str) -> String {
    let t = p.trim().trim_matches('/');
    if t.is_empty() { String::new() } else { format!("/{t}") }
}

fn status_err(status: u16, body: &str) -> io::Error {
    match status {
        401 => io::Error::new(io::ErrorKind::PermissionDenied, "AUTH:nome utente o password non accettati"),
        // su una MOVE/MKCOL il 409 vuol dire che manca la cartella che deve contenerla
        404 | 409 => io::Error::new(io::ErrorKind::NotFound, "non esiste"),
        // MKCOL su un nome già preso e MOVE con Overwrite: F
        405 | 412 => io::Error::new(io::ErrorKind::AlreadyExists, "esiste già"),
        423 => io::Error::other("il file è bloccato sul server"),
        _ => http::status_error(status, body),
    }
}

impl WebDav {
    fn new(root: String, user: &str, secret: &str) -> Self {
        let token = base64::engine::general_purpose::STANDARD.encode(format!("{user}:{secret}"));
        let root_path = decoded_path(&root);
        WebDav { root, root_path, auth: format!("Basic {token}"), trash_root: None }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.root, http::pct(path, true))
    }

    /// Una richiesta senza corpo; `Destination` solo per MOVE.
    fn call(&self, method: &str, path: &str, destination: Option<&str>) -> io::Result<http::Resp> {
        let mut b = Request::builder()
            .method(Method::from_bytes(method.as_bytes()).map_err(io::Error::other)?)
            .uri(self.url(path))
            .header("Authorization", self.auth.as_str());
        if let Some(d) = destination {
            b = b.header("Destination", d).header("Overwrite", "F");
        }
        agent().run(b.body(()).map_err(io::Error::other)?).map_err(http::net)
    }

    fn propfind(&self, path: &str, depth: &str) -> io::Result<Vec<(String, RStat)>> {
        const BODY: &str = "<?xml version=\"1.0\"?><d:propfind xmlns:d=\"DAV:\"><d:prop>\
            <d:resourcetype/><d:getcontentlength/><d:getlastmodified/></d:prop></d:propfind>";
        let req = Request::builder()
            .method(Method::from_bytes(b"PROPFIND").map_err(io::Error::other)?)
            .uri(self.url(path))
            .header("Authorization", self.auth.as_str())
            .header("Depth", depth)
            .header("Content-Type", "application/xml")
            .body(BODY)
            .map_err(io::Error::other)?;
        let mut r = agent().run(req).map_err(http::net)?;
        let status = r.status().as_u16();
        let body = r.body_mut().read_to_string().unwrap_or_default();
        if status != 207 {
            return Err(status_err(status, &body));
        }
        parse_multistatus(&body)
    }

    /// Una richiesta sul cestino: 2xx o errore.
    fn trash_call(&self, method: &str, url: &str, destination: Option<&str>) -> io::Result<()> {
        let mut b = Request::builder()
            .method(Method::from_bytes(method.as_bytes()).map_err(io::Error::other)?)
            .uri(url)
            .header("Authorization", self.auth.as_str());
        if let Some(d) = destination {
            b = b.header("Destination", d);
        }
        let mut r = agent().run(b.body(()).map_err(io::Error::other)?).map_err(http::net)?;
        let status = r.status().as_u16();
        if (200..300).contains(&status) {
            return Ok(());
        }
        let body = r.body_mut().read_to_string().unwrap_or_default();
        Err(status_err(status, &body))
    }

    fn trash_list(&self, root: &str) -> io::Result<String> {
        const BODY: &str = "<?xml version=\"1.0\"?><d:propfind xmlns:d=\"DAV:\" xmlns:nc=\"http://nextcloud.org/ns\" xmlns:oc=\"http://owncloud.org/ns\"><d:prop>\
            <nc:trashbin-filename/><nc:trashbin-original-location/><nc:trashbin-deletion-time/>\
            <d:getcontentlength/><oc:size/><d:resourcetype/></d:prop></d:propfind>";
        let req = Request::builder()
            .method(Method::from_bytes(b"PROPFIND").map_err(io::Error::other)?)
            .uri(format!("{root}/trash"))
            .header("Authorization", self.auth.as_str())
            .header("Depth", "1")
            .header("Content-Type", "application/xml")
            .body(BODY)
            .map_err(io::Error::other)?;
        let mut r = agent().run(req).map_err(http::net)?;
        let status = r.status().as_u16();
        let body = r.body_mut().read_to_string().unwrap_or_default();
        if status != 207 {
            return Err(status_err(status, &body));
        }
        parse_trash(&body).map(|v| v.to_string())
    }

    fn own_path(&self, path: &str) -> String {
        format!("{}{}", self.root_path, path).trim_end_matches('/').to_string()
    }
}

pub fn connect(profile_json: &str, secret: &str) -> Result<Arc<dyn Drive>, String> {
    let p: Profile = serde_json::from_str(profile_json).map_err(|e| format!("profilo non valido: {e}"))?;
    if p.host.trim().is_empty() {
        return Err("manca l'indirizzo del server".into());
    }
    if secret.is_empty() {
        return Err("AUTH:serve la password".into());
    }
    let secure = p.secure.unwrap_or(true);
    let scheme = if secure { "https" } else { "http" };
    let (server, sub) = split_host(&p.host, secure, p.port);
    let base = format!("{scheme}://{server}{sub}");
    let root = if p.kind == "nextcloud" {
        format!("{base}/remote.php/dav/files/{}", http::pct(&p.user, false))
    } else {
        format!("{base}{}", http::pct(&clean_path(&p.path), true))
    };
    let trash_root = (p.kind == "nextcloud").then(|| format!("{base}/remote.php/dav/trashbin/{}", http::pct(&p.user, false)));
    let mut d = WebDav::new(root, &p.user, secret);
    d.trash_root = trash_root.clone();
    // ⚠️ Nextcloud 34: la primissima richiesta di un utente nuovo dà 500
    // (getMetadata su un nodo non ancora caricato), dalla seconda va bene
    let first = d.propfind("/", "0");
    let is_500 = matches!(&first, Err(e) if e.to_string().starts_with("errore 500"));
    match if is_500 { d.propfind("/", "0") } else { first } {
        Ok(_) => Ok(Arc::new(d)),
        // l'accesso può essere con un nome diverso dall'identificativo (es. l'email):
        // il vecchio indirizzo vale per chiunque abbia fatto l'accesso
        Err(e) if p.kind == "nextcloud" && e.kind() == io::ErrorKind::NotFound => {
            let mut d = WebDav::new(format!("{base}/remote.php/webdav"), &p.user, secret);
            d.trash_root = trash_root;
            d.propfind("/", "0").map_err(|e| e.to_string())?;
            Ok(Arc::new(d))
        }
        Err(e) => Err(e.to_string()),
    }
}

impl Drive for WebDav {
    fn stat(&self, path: &str) -> io::Result<RStat> {
        self.propfind(path, "0")?
            .into_iter()
            .next()
            .map(|(_, st)| st)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "non esiste"))
    }

    fn list(&self, path: &str) -> io::Result<Vec<(String, RStat)>> {
        let mut all = self.propfind(path, "1")?;
        let own = self.own_path(path);
        // di solito la cartella stessa è la prima voce, ma l'ordine non è garantito
        match all.iter().position(|(h, _)| *h == own) {
            Some(i) => {
                all.remove(i);
            }
            None if !all.is_empty() => {
                all.remove(0);
            }
            None => {}
        }
        Ok(all
            .into_iter()
            .filter_map(|(h, st)| {
                let name = h.rsplit('/').next().unwrap_or("").to_string();
                (!name.is_empty()).then_some((name, st))
            })
            .collect())
    }

    fn read(&self, path: &str) -> io::Result<Box<dyn Read + Send>> {
        let mut r = self.call("GET", path, None)?;
        let status = r.status().as_u16();
        if status != 200 {
            let body = r.body_mut().read_to_string().unwrap_or_default();
            return Err(status_err(status, &body));
        }
        Ok(Box::new(r.into_body().into_reader()))
    }

    fn writer(self: Arc<Self>, path: &str, size: Option<u64>) -> io::Result<Box<dyn crate::vfs::Sink>> {
        let url = self.url(path);
        let auth = self.auth.clone();
        let Some(total) = size else {
            // senza dimensione (per esempio il salvataggio dell'editor) si manda tutto alla fine
            return Ok(Box::new(DavSink { url, auth, buf: Some(Vec::new()), tx: None, worker: None, finished: Default::default(), done: false }));
        };
        let (tx, rx) = sync_channel::<Vec<u8>>(16);
        let finished = Arc::new(AtomicBool::new(false));
        let (u, a, f) = (url.clone(), auth.clone(), finished.clone());
        let worker = std::thread::spawn(move || {
            let mut body = Channel { rx, cur: Vec::new(), pos: 0, finished: f };
            // la lunghezza dichiarata evita il chunked, che non tutti i server reggono
            let mut r = agent()
                .put(&u)
                .header("Authorization", &a)
                .header("Content-Length", &total.to_string())
                .send(SendBody::from_reader(&mut body))
                .map_err(http::net)?;
            let status = r.status().as_u16();
            if (200..300).contains(&status) {
                Ok(())
            } else {
                let b = r.body_mut().read_to_string().unwrap_or_default();
                Err(status_err(status, &b))
            }
        });
        Ok(Box::new(DavSink { url, auth, buf: None, tx: Some(tx), worker: Some(worker), finished, done: false }))
    }

    fn mkdir(&self, path: &str) -> io::Result<()> {
        let mut r = self.call("MKCOL", path, None)?;
        let status = r.status().as_u16();
        if (200..300).contains(&status) {
            return Ok(());
        }
        let body = r.body_mut().read_to_string().unwrap_or_default();
        Err(status_err(status, &body))
    }

    fn delete(&self, path: &str) -> io::Result<()> {
        let mut r = self.call("DELETE", path, None)?;
        let status = r.status().as_u16();
        if (200..300).contains(&status) {
            return Ok(());
        }
        let body = r.body_mut().read_to_string().unwrap_or_default();
        Err(status_err(status, &body))
    }

    fn rename(&self, from: &str, to: &str) -> io::Result<()> {
        let mut r = self.call("MOVE", from, Some(&self.url(to)))?;
        let status = r.status().as_u16();
        if (200..300).contains(&status) {
            return Ok(());
        }
        let body = r.body_mut().read_to_string().unwrap_or_default();
        Err(status_err(status, &body))
    }

    fn trash(&self, action: &str, item: &str) -> io::Result<String> {
        let Some(root) = &self.trash_root else {
            return Err(io::Error::new(io::ErrorKind::Unsupported, "questo server non ha un cestino consultabile"));
        };
        let voce = format!("{root}/trash/{}", http::pct(item, false));
        match action {
            "list" => return self.trash_list(root),
            "restore" => self.trash_call("MOVE", &voce, Some(&format!("{root}/restore/{}", http::pct(item, false))))?,
            "delete" => self.trash_call("DELETE", &voce, None)?,
            "empty" => self.trash_call("DELETE", &format!("{root}/trash"), None)?,
            a => return Err(io::Error::other(format!("azione sconosciuta: {a}"))),
        }
        Ok("[]".into())
    }
}

/// Il corpo della PUT: legge i pezzi che arrivano da `write`. Se il mittente
/// sparisce senza aver finito (Annulla) dà errore, così la PUT si interrompe
/// e sul server il file non nasce.
struct Channel {
    rx: Receiver<Vec<u8>>,
    cur: Vec<u8>,
    pos: usize,
    finished: Arc<AtomicBool>,
}

impl Read for Channel {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        while self.pos >= self.cur.len() {
            match self.rx.recv() {
                Ok(v) => {
                    self.cur = v;
                    self.pos = 0;
                }
                Err(_) if self.finished.load(Ordering::SeqCst) => return Ok(0),
                Err(_) => return Err(io::Error::new(io::ErrorKind::Interrupted, "annullato")),
            }
        }
        let n = out.len().min(self.cur.len() - self.pos);
        out[..n].copy_from_slice(&self.cur[self.pos..self.pos + n]);
        self.pos += n;
        Ok(n)
    }
}

/// Il file in scrittura; `flush` chiude il caricamento.
struct DavSink {
    url: String,
    auth: String,
    /// dimensione ignota: tutto in memoria, si manda a `flush`
    buf: Option<Vec<u8>>,
    tx: Option<SyncSender<Vec<u8>>>,
    worker: Option<JoinHandle<io::Result<()>>>,
    finished: Arc<AtomicBool>,
    done: bool,
}

impl DavSink {
    fn join(&mut self) -> io::Result<()> {
        match self.worker.take() {
            Some(w) => w.join().map_err(|_| io::Error::other("caricamento interrotto"))?,
            None => Ok(()),
        }
    }
}

impl Write for DavSink {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        if let Some(b) = self.buf.as_mut() {
            b.extend_from_slice(data);
            return Ok(data.len());
        }
        let sent = self.tx.as_ref().map(|t| t.send(data.to_vec()).is_ok()).unwrap_or(false);
        if sent {
            return Ok(data.len());
        }
        // il server ha smesso di ascoltare: la causa la sa la PUT
        self.tx = None;
        self.join()?;
        Err(io::Error::other("il server ha interrotto il caricamento"))
    }

    fn flush(&mut self) -> io::Result<()> {
        if self.done {
            return Ok(());
        }
        if let Some(data) = self.buf.take() {
            let mut r = agent()
                .put(&self.url)
                .header("Authorization", &self.auth)
                .send(&data[..])
                .map_err(http::net)?;
            let status = r.status().as_u16();
            if !(200..300).contains(&status) {
                let b = r.body_mut().read_to_string().unwrap_or_default();
                return Err(status_err(status, &b));
            }
        } else {
            self.finished.store(true, Ordering::SeqCst);
            self.tx = None;
            self.join()?;
        }
        self.done = true;
        Ok(())
    }
}

impl crate::vfs::Sink for DavSink {
    fn sync(&mut self) -> io::Result<()> {
        Ok(())
    }
}

// ------------------------------------------------------------------ risposte

fn pct_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        let hex = |c: u8| (c as char).to_digit(16);
        if b[i] == b'%'
            && i + 2 < b.len()
            && let (Some(h), Some(l)) = (hex(b[i + 1]), hex(b[i + 2]))
        {
            out.push((h * 16 + l) as u8);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Il percorso in chiaro di un href o di un URL completo, senza barra finale.
fn decoded_path(href: &str) -> String {
    let t = href.trim();
    let path = match t.find("://") {
        Some(i) => t[i + 3..].find('/').map(|j| &t[i + 3 + j..]).unwrap_or(""),
        None => t,
    };
    pct_decode(path).trim_end_matches('/').to_string()
}

/// Le voci del cestino di Nextcloud come JSON per l'interfaccia. La cartella
/// del cestino stessa non ha il nome originale, e così resta fuori.
pub fn parse_trash(xml: &str) -> io::Result<serde_json::Value> {
    let mut rd = Reader::from_str(xml);
    let mut out = Vec::new();
    let mut tag = Vec::<u8>::new();
    let (mut href, mut name, mut original) = (String::new(), String::new(), String::new());
    let (mut deleted, mut size, mut dir, mut in_type) = (0i64, 0u64, false, false);
    loop {
        match rd.read_event().map_err(|e| io::Error::other(format!("risposta illeggibile: {e}")))? {
            Event::Start(e) => {
                let n = e.local_name().as_ref().to_vec();
                match n.as_slice() {
                    b"response" => {
                        href.clear();
                        name.clear();
                        original.clear();
                        (deleted, size, dir) = (0, 0, false);
                    }
                    b"resourcetype" => in_type = true,
                    b"collection" if in_type => dir = true,
                    _ => {}
                }
                tag = n;
            }
            Event::Empty(e) => {
                if in_type && e.local_name().as_ref() == b"collection" {
                    dir = true;
                }
            }
            Event::Text(t) => {
                let v = t.unescape().map_err(|e| io::Error::other(e.to_string()))?;
                match tag.as_slice() {
                    b"href" => href.push_str(&v),
                    b"trashbin-filename" => name.push_str(&v),
                    b"trashbin-original-location" => original.push_str(&v),
                    b"trashbin-deletion-time" => deleted = v.trim().parse().unwrap_or(0),
                    b"getcontentlength" | b"size" => size = size.max(v.trim().parse().unwrap_or(0)),
                    _ => {}
                }
            }
            Event::End(e) => {
                tag.clear();
                match e.local_name().as_ref() {
                    b"resourcetype" => in_type = false,
                    b"response" if !name.is_empty() => {
                        let path = decoded_path(&href);
                        let id = path.rsplit('/').next().unwrap_or("");
                        out.push(serde_json::json!({ "id": id, "name": name, "original": original, "deleted": deleted, "size": size, "dir": dir }));
                    }
                    _ => {}
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(serde_json::Value::Array(out))
}

/// «Mon, 09 Nov 2026 11:01:32 GMT» → secondi Unix (0 se illeggibile).
pub fn http_date(s: &str) -> i64 {
    const MESI: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    let p: Vec<&str> = s.split_whitespace().collect();
    if p.len() < 5 {
        return 0;
    }
    let Some(m) = MESI.iter().position(|x| *x == p[2]) else { return 0 };
    let Ok(d) = p[1].parse::<u32>() else { return 0 };
    http::rfc3339(&format!("{}-{:02}-{:02}T{}Z", p[3], m + 1, d, p[4]))
}

/// La risposta di PROPFIND: (percorso in chiaro, dati) per ogni voce. Conta
/// il nome locale dei tag: il prefisso del namespace cambia fra i server.
pub fn parse_multistatus(xml: &str) -> io::Result<Vec<(String, RStat)>> {
    let mut rd = Reader::from_str(xml);
    let mut out = Vec::new();
    let mut tag = Vec::<u8>::new();
    let (mut href, mut st, mut ok_prop, mut in_type) = (String::new(), RStat::default(), true, false);
    let mut pending = RStat::default();
    loop {
        let ev = rd.read_event().map_err(|e| io::Error::other(format!("risposta illeggibile: {e}")))?;
        match ev {
            Event::Start(e) => {
                let name = e.local_name().as_ref().to_vec();
                match name.as_slice() {
                    b"response" => {
                        href.clear();
                        st = RStat { is_file: true, mode: 0o644, ..Default::default() };
                    }
                    b"propstat" => {
                        pending = RStat::default();
                        ok_prop = true;
                    }
                    b"resourcetype" => in_type = true,
                    b"collection" if in_type => pending.is_dir = true,
                    _ => {}
                }
                tag = name;
            }
            Event::Empty(e) => {
                if in_type && e.local_name().as_ref() == b"collection" {
                    pending.is_dir = true;
                }
            }
            Event::Text(t) => {
                let v = t.unescape().map_err(|e| io::Error::other(e.to_string()))?;
                match tag.as_slice() {
                    b"href" => href.push_str(&v),
                    b"getcontentlength" => pending.size = v.trim().parse().unwrap_or(0),
                    b"getlastmodified" => pending.mtime = http_date(&v),
                    // una propstat «404» elenca le proprietà che il server non ha
                    b"status" => ok_prop = v.contains(" 200"),
                    _ => {}
                }
            }
            Event::End(e) => {
                tag.clear();
                match e.local_name().as_ref() {
                    b"resourcetype" => in_type = false,
                    b"propstat" if ok_prop => {
                        st.size = pending.size;
                        st.mtime = pending.mtime;
                        if pending.is_dir {
                            st.is_dir = true;
                            st.is_file = false;
                            st.mode = 0o755;
                        }
                    }
                    b"response" if !href.is_empty() => out.push((decoded_path(&href), st)),
                    _ => {}
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(out)
}
