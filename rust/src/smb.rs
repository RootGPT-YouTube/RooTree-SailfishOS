//! SMB 2/3: cartelle condivise di Windows, Samba e dei NAS (Fase 5, step 5).
//! Implementa `Drive`, quindi copia, coda, Annulla e conflitti valgono come
//! per gli altri remoti.
//!
//! - Il crate `smb2` (MIT/Apache-2.0), Rust puro: firma e cifratura SMB3,
//!   NTLM, elenco delle condivisioni. È asincrono: gira su un runtime tokio
//!   della connessione, come Mega.
//! - Un collegamento solo: SMB fa viaggiare più richieste insieme, e letture
//!   e scritture hanno la loro copia della connessione, quindi si sfoglia
//!   mentre una copia è in corso senza aprirne altri.
//! - Percorsi: `/` elenca le condivisioni del server, `/Condivisione/…` è
//!   dentro una condivisione. Se il profilo ne nomina una, si parte da lì.
//! - Ospite: utente vuoto, nessuna password. Una password sbagliata su un
//!   Samba con «map to guest = bad user» diventerebbe un ospite muto: la
//!   libreria la rifiuta, e qui diventa «password rifiutata».

use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::sync::{Arc, Mutex};
use std::time::{Duration, UNIX_EPOCH};

use serde::Deserialize;
use smb2::{ClientConfig, ErrorKind as SK, FileDownload, FileWriter, SmbClient, Tree};

use crate::cloud::Drive;
use crate::remote::RStat;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
/// quanto si accumula prima di mandare un pezzo al server
const CHUNK: usize = 1 << 20;

#[derive(Deserialize, Default)]
#[serde(default)]
struct Profile {
    host: String,
    port: u16,
    user: String,
    auth: String,
    share: String,
    domain: String,
}

struct State {
    client: SmbClient,
    /// le condivisioni già collegate
    trees: HashMap<String, Tree>,
}

pub struct SmbDrive {
    rt: Arc<tokio::runtime::Runtime>,
    st: Mutex<State>,
    /// la condivisione del profilo (vuota: si parte dall'elenco)
    share: String,
}

/// Il motivo di un errore SMB per l'utente e per il motore (che guarda il tipo).
fn smb_err(e: smb2::Error) -> io::Error {
    let (kind, text) = match e.kind() {
        SK::NotFound => (io::ErrorKind::NotFound, "non trovato".to_string()),
        SK::AlreadyExists => (io::ErrorKind::AlreadyExists, "esiste già".to_string()),
        SK::AccessDenied | SK::AuthRequired => (io::ErrorKind::PermissionDenied, "accesso negato".to_string()),
        SK::SharingViolation => (io::ErrorKind::ResourceBusy, "il file è aperto da un altro programma".to_string()),
        SK::IsADirectory => (io::ErrorKind::IsADirectory, "è una cartella".to_string()),
        SK::NotADirectory => (io::ErrorKind::NotADirectory, "non è una cartella".to_string()),
        SK::DiskFull => (io::ErrorKind::StorageFull, "spazio esaurito sul server".to_string()),
        SK::ConnectionLost | SK::SessionExpired => (io::ErrorKind::ConnectionAborted, format!("collegamento perso ({e})")),
        SK::TimedOut => (io::ErrorKind::TimedOut, "il server non risponde".to_string()),
        SK::InvalidName => (io::ErrorKind::InvalidFilename, "nome non accettato dal server".to_string()),
        _ => (io::ErrorKind::Other, e.to_string()),
    };
    io::Error::new(kind, text)
}

/// `/Condivisione/a/b` → ("Condivisione", "a/b"); `/` → ("", "").
fn split(path: &str) -> (String, String) {
    let t = path.trim_matches('/');
    match t.split_once('/') {
        Some((s, r)) => (s.to_string(), r.trim_matches('/').to_string()),
        None => (t.to_string(), String::new()),
    }
}

fn unix(t: smb2::pack::FileTime) -> i64 {
    t.to_system_time().and_then(|s| s.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_secs() as i64).unwrap_or(0)
}

fn dir_stat() -> RStat {
    RStat { is_dir: true, is_file: false, is_link: false, size: 0, mode: 0o755, mtime: 0 }
}

fn stat_of(is_dir: bool, size: u64, mtime: i64, readonly: bool) -> RStat {
    let mode = match (is_dir, readonly) {
        (true, _) => 0o755,
        (false, true) => 0o444,
        (false, false) => 0o644,
    };
    RStat { is_dir, is_file: !is_dir, is_link: false, size: if is_dir { 0 } else { size }, mode, mtime }
}

/// La condivisione `name`, collegata la prima volta che serve. Si dà una
/// copia: dopo l'operazione torna nella tabella (un ricollegamento le cambia l'id).
async fn tree_for(st: &mut State, name: &str) -> smb2::Result<Tree> {
    if let Some(t) = st.trees.get(name) {
        return Ok(t.clone());
    }
    let t = st.client.connect_share(name).await?;
    st.trees.insert(name.to_string(), t.clone());
    Ok(t)
}

pub fn connect(profile_json: &str, secret: &str) -> Result<Arc<dyn Drive>, String> {
    let p: Profile = serde_json::from_str(profile_json).map_err(|e| format!("profilo non valido: {e}"))?;
    let host = p.host.trim().trim_start_matches("smb://").trim_start_matches("\\\\").trim_end_matches(['/', '\\']).to_string();
    if host.is_empty() {
        return Err("manca l'indirizzo del server".into());
    }
    let port = if p.port == 0 { 445 } else { p.port };
    let guest = p.auth == "guest";
    if !guest && secret.is_empty() {
        return Err("AUTH:serve la password".into());
    }
    let share = p.share.trim().trim_matches(['/', '\\']).to_string();
    // un IPv6 va fra parentesi quadre prima della porta
    let addr = if host.contains(':') && !host.starts_with('[') { format!("[{host}]:{port}") } else { format!("{host}:{port}") };
    let config = ClientConfig {
        addr,
        timeout: CONNECT_TIMEOUT,
        username: if guest { String::new() } else { p.user.trim().to_string() },
        password: if guest { String::new() } else { secret.to_string() },
        domain: p.domain.trim().to_string(),
        auto_reconnect: true,
        ..Default::default()
    };
    let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().map_err(|e| e.to_string())?;
    let client = rt.block_on(SmbClient::connect(config)).map_err(|e| match e.kind() {
        SK::AuthRequired | SK::GuestSessionRefused => {
            if guest { "AUTH:il server non accetta ospiti: serve un utente con la password".to_string() } else { "AUTH:password rifiutata dal server".to_string() }
        }
        SK::SigningRequired if guest => "il server vuole la firma dei messaggi, che un ospite non può dare: serve un utente con la password".to_string(),
        SK::ConnectionLost | SK::TimedOut | SK::Io => format!("NET:{host}:{port}: {e}"),
        _ => format!("NET:{host}:{port}: {e}"),
    })?;
    let mut st = State { client, trees: HashMap::new() };
    if !share.is_empty() {
        // la condivisione del profilo deve esistere e lasciarci entrare
        rt.block_on(tree_for(&mut st, &share)).map_err(|e| match e.kind() {
            SK::NotFound => format!("la condivisione «{share}» non c'è su {host}"),
            SK::AccessDenied | SK::AuthRequired => format!("la condivisione «{share}» non è accessibile a questo utente"),
            _ => format!("condivisione «{share}»: {e}"),
        })?;
    } else {
        // senza condivisione si parte dall'elenco: deve funzionare
        rt.block_on(st.client.list_shares()).map_err(|e| format!("elenco delle condivisioni di {host}: {e}. Scrivi il nome della condivisione nella connessione."))?;
    }
    Ok(Arc::new(SmbDrive { rt: Arc::new(rt), st: Mutex::new(st), share }))
}

impl SmbDrive {
    /// Un'operazione dentro la condivisione `share`.
    fn run<T>(&self, share: &str, f: impl AsyncFnOnce(&mut SmbClient, &mut Tree) -> smb2::Result<T>) -> io::Result<T> {
        let mut g = self.st.lock().unwrap();
        let st = &mut *g;
        self.rt
            .block_on(async {
                let mut tree = tree_for(st, share).await?;
                let r = f(&mut st.client, &mut tree).await;
                st.trees.insert(share.to_string(), tree);
                r
            })
            .map_err(smb_err)
    }

    fn need_file(&self, path: &str) -> io::Result<(String, String)> {
        let (s, r) = split(path);
        if s.is_empty() || r.is_empty() {
            return Err(io::Error::new(io::ErrorKind::Unsupported, "qui ci sono solo le condivisioni del server"));
        }
        Ok((s, r))
    }

    fn delete_tree(&self, share: &str, rel: &str) -> io::Result<()> {
        let rel_s = rel.to_string();
        let items = self.run(share, async |c, t| c.list_directory(t, &rel_s).await)?;
        for e in items {
            if e.name == "." || e.name == ".." {
                continue;
            }
            let child = format!("{rel}/{}", e.name);
            // un collegamento (junction) si toglie da solo, senza entrarci
            if e.is_directory && e.reparse_tag.is_none() {
                self.delete_tree(share, &child)?;
            } else {
                self.run(share, async |c, t| if e.is_directory { c.delete_link(t, &child).await } else { c.delete_file(t, &child).await })?;
            }
        }
        self.run(share, async |c, t| c.delete_directory(t, &rel_s).await)
    }
}

impl Drive for SmbDrive {
    fn start(&self) -> String {
        if self.share.is_empty() { "/".into() } else { format!("/{}", self.share) }
    }

    fn stat(&self, path: &str) -> io::Result<RStat> {
        let (s, r) = split(path);
        if s.is_empty() {
            return Ok(dir_stat());
        }
        if r.is_empty() {
            // la condivisione: c'è se ci si collega
            self.run(&s, async |_, _| Ok(()))?;
            return Ok(dir_stat());
        }
        let i = self.run(&s, async |c, t| c.stat(t, &r).await)?;
        let ro = i.attributes.0 & smb2::FileAttributes::READONLY != 0;
        Ok(stat_of(i.is_directory, i.size, unix(i.modified), ro))
    }

    fn list(&self, path: &str) -> io::Result<Vec<(String, RStat)>> {
        let (s, r) = split(path);
        if s.is_empty() {
            let shares = {
                let mut g = self.st.lock().unwrap();
                self.rt.block_on(g.client.list_shares())
            };
            return match shares {
                Ok(v) => Ok(v.into_iter().map(|x| (x.name, dir_stat())).collect()),
                // chi non può elencarle vede almeno la sua
                Err(_) if !self.share.is_empty() => Ok(vec![(self.share.clone(), dir_stat())]),
                Err(e) => Err(smb_err(e)),
            };
        }
        let v = self.run(&s, async |c, t| c.list_directory(t, &r).await)?;
        Ok(v.into_iter()
            .filter(|e| e.name != "." && e.name != "..")
            .map(|e| {
                let ro = e.attributes.0 & smb2::FileAttributes::READONLY != 0;
                let st = stat_of(e.is_directory, e.size, unix(e.modified), ro);
                (e.name, st)
            })
            .collect())
    }

    fn read(&self, path: &str) -> io::Result<Box<dyn Read + Send>> {
        let (s, r) = self.need_file(path)?;
        let dl = self.run(&s, async |c, t| c.open_file_download(t, &r).await)?;
        Ok(Box::new(SmbReader { rt: self.rt.clone(), dl: Some(dl), buf: Vec::new(), pos: 0 }))
    }

    fn writer(self: Arc<Self>, path: &str, _size: Option<u64>) -> io::Result<Box<dyn crate::vfs::Sink>> {
        let (s, r) = self.need_file(path)?;
        let w = self.run(&s, async |c, t| c.create_file_writer(t, &r).await)?;
        Ok(Box::new(SmbSink { rt: self.rt.clone(), w: Some(w), buf: Vec::with_capacity(CHUNK), drive: self.clone(), share: s, rel: r, done: false }))
    }

    fn mkdir(&self, path: &str) -> io::Result<()> {
        let (s, r) = self.need_file(path)?;
        self.run(&s, async |c, t| c.create_directory(t, &r).await)
    }

    fn delete(&self, path: &str) -> io::Result<()> {
        let (s, r) = self.need_file(path)?;
        let i = self.run(&s, async |c, t| c.stat(t, &r).await)?;
        if i.is_directory && i.reparse_tag.is_none() {
            self.delete_tree(&s, &r)
        } else if i.is_directory {
            self.run(&s, async |c, t| c.delete_link(t, &r).await)
        } else {
            self.run(&s, async |c, t| c.delete_file(t, &r).await)
        }
    }

    fn rename(&self, from: &str, to: &str) -> io::Result<()> {
        let (s1, r1) = self.need_file(from)?;
        let (s2, r2) = self.need_file(to)?;
        if s1 != s2 {
            // il motore ripiega su copia + eliminazione
            return Err(io::Error::new(io::ErrorKind::CrossesDevices, "fra due condivisioni diverse non si sposta: si copia"));
        }
        self.run(&s1, async |c, t| c.rename(t, &r1, &r2).await)
    }
}

/// Lettura a flusso: la libreria tiene in volo più richieste da sola.
struct SmbReader {
    rt: Arc<tokio::runtime::Runtime>,
    dl: Option<FileDownload<'static>>,
    buf: Vec<u8>,
    pos: usize,
}

impl Read for SmbReader {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        while self.pos >= self.buf.len() {
            let Some(dl) = self.dl.as_mut() else { return Ok(0) };
            match self.rt.block_on(dl.next_chunk()) {
                Some(Ok(b)) => {
                    self.buf = b;
                    self.pos = 0;
                }
                Some(Err(e)) => return Err(smb_err(e)),
                None => {
                    self.dl = None;
                    return Ok(0);
                }
            }
        }
        let n = out.len().min(self.buf.len() - self.pos);
        out[..n].copy_from_slice(&self.buf[self.pos..self.pos + n]);
        self.pos += n;
        Ok(n)
    }
}

impl Drop for SmbReader {
    fn drop(&mut self) {
        // la libreria chiude l'handle da sola, ma dentro il suo runtime
        if let Some(dl) = self.dl.take() {
            let _g = self.rt.enter();
            drop(dl);
        }
    }
}

/// Scrittura a pezzi da 1 MB; `flush` chiude il file sul server. Senza flush
/// (Annulla o errore) il file a metà viene tolto.
struct SmbSink {
    rt: Arc<tokio::runtime::Runtime>,
    w: Option<FileWriter>,
    buf: Vec<u8>,
    drive: Arc<SmbDrive>,
    share: String,
    rel: String,
    done: bool,
}

impl SmbSink {
    fn push(&mut self) -> io::Result<()> {
        if self.buf.is_empty() {
            return Ok(());
        }
        let w = self.w.as_mut().ok_or_else(|| io::Error::other("caricamento già chiuso"))?;
        self.rt.block_on(w.write_chunk(&self.buf)).map_err(smb_err)?;
        self.buf.clear();
        Ok(())
    }
}

impl Write for SmbSink {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        if self.w.is_none() {
            return Err(io::Error::other("caricamento già chiuso"));
        }
        self.buf.extend_from_slice(data);
        if self.buf.len() >= CHUNK {
            self.push()?;
        }
        Ok(data.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        if self.done {
            return Ok(());
        }
        self.push()?;
        if let Some(w) = self.w.take() {
            self.rt.block_on(w.finish()).map_err(smb_err)?;
        }
        self.done = true;
        Ok(())
    }
}

impl crate::vfs::Sink for SmbSink {
    fn sync(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Drop for SmbSink {
    fn drop(&mut self) {
        if self.done {
            return;
        }
        if let Some(w) = self.w.take() {
            let _ = self.rt.block_on(w.abort());
        }
        let rel = self.rel.clone();
        let _ = self.drive.run(&self.share, async |c, t| c.delete_file(t, &rel).await);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percorsi() {
        assert_eq!(split("/"), (String::new(), String::new()));
        assert_eq!(split(""), (String::new(), String::new()));
        assert_eq!(split("/Dati"), ("Dati".into(), String::new()));
        assert_eq!(split("/Dati/"), ("Dati".into(), String::new()));
        assert_eq!(split("/Dati/a b/città.txt"), ("Dati".into(), "a b/città.txt".into()));
    }

    #[test]
    fn permessi() {
        assert_eq!(stat_of(false, 5, 0, true).mode, 0o444);
        assert_eq!(stat_of(true, 5, 0, false).size, 0);
    }
}
