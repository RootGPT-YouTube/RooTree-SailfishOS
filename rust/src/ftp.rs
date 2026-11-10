//! FTP e FTPS (Fase 5, step 5). Implementa `Drive`, quindi copia, coda,
//! Annulla e conflitti valgono come per gli altri remoti.
//!
//! - Un collegamento di controllo per sfogliare, e un collegamento NUOVO per
//!   ogni trasferimento: su FTP un collegamento fa una cosa alla volta, e
//!   l'utente deve poter sfogliare il server mentre una copia è in corso.
//! - FTPS esplicito (AUTH TLS sulla porta 21) o implicito (porta 990), con un
//!   connettore OpenSSL nostro invece di quello di suppaftp per due motivi:
//!   1. il canale dati riusa la sessione TLS del controllo: vsftpd (comune
//!      sui NAS) la pretende di serie (`require_ssl_reuse=YES`) e senza
//!      riuso rifiuta ogni trasferimento («522 SSL connection failed»);
//!   2. i NAS hanno spesso un certificato fatto in casa: come per le chiavi
//!      SSH, la prima volta si mostra l'impronta e si chiede se fidarsi
//!      (`HOSTKEY_NEW:TLS:impronta`), e se poi cambia si rifiuta
//!      (`HOSTKEY_CHANGED`). Un certificato valido per una CA non chiede nulla.
//!   Nessuna credenziale parte prima della verifica.
//! - Accesso anonimo: utente «anonymous», nessuna password da chiedere.

use std::collections::BTreeMap;
use std::fmt;
use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, UNIX_EPOCH};

use openssl::hash::MessageDigest;
use openssl::ex_data::Index;
use openssl::ssl::{Ssl, SslConnector, SslMethod, SslSession, SslSessionCacheMode, SslStream, SslVerifyMode};
use serde::Deserialize;
use suppaftp::list::{File as LFile, ListParser};
use suppaftp::types::FileType;
use suppaftp::{FtpError, ImplFtpStream, Status, TransferStream};

use crate::cloud::Drive;
use crate::remote::RStat;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
/// un server piccolo può metterci parecchio a chiudere un file grosso
const IO_TIMEOUT: Duration = Duration::from_secs(120);

// ------------------------------------------------------------------ TLS

/// Lo stream TLS (controllo e dati) per suppaftp.
pub struct Ossl {
    stream: SslStream<TcpStream>,
    shutdown: bool,
}

impl fmt::Debug for Ossl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Ossl")
    }
}

impl suppaftp::TlsStream for Ossl {
    type InnerStream = SslStream<TcpStream>;

    fn tcp_stream(mut self) -> suppaftp::FtpResult<TcpStream> {
        let s = self.stream.get_ref().try_clone().map_err(FtpError::ConnectionError)?;
        self.shutdown = false;
        Ok(s)
    }

    fn get_ref(&self) -> &TcpStream {
        self.stream.get_ref()
    }

    fn mut_ref(&mut self) -> &mut Self::InnerStream {
        &mut self.stream
    }
}

impl Drop for Ossl {
    fn drop(&mut self) {
        if self.shutdown {
            let _ = self.stream.shutdown();
        }
    }
}

/// L'esito della verifica del certificato, letto dopo il collegamento.
#[derive(Clone, Debug, PartialEq)]
enum Verdict {
    Ok,
    New(String),
    Changed(String),
}

struct TlsInner {
    connector: SslConnector,
    /// l'ultima sessione TLS: il canale dati la riusa
    session: Arc<Mutex<Option<SslSession>>>,
    /// l'impronta già accettata dall'utente per questo server, se c'è
    pinned: Option<String>,
    /// l'impronta vista al primo collegamento (controllo): i dati devono avere la stessa
    seen: Mutex<Option<String>>,
    verdict: Mutex<Verdict>,
}

#[derive(Clone)]
pub struct Tls(Arc<TlsInner>);

impl fmt::Debug for Tls {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Tls")
    }
}

/// L'etichetta del collegamento di CONTROLLO: solo le sue sessioni si tengono.
fn control_tag() -> Index<Ssl, bool> {
    static I: OnceLock<Index<Ssl, bool>> = OnceLock::new();
    *I.get_or_init(|| Ssl::new_ex_index::<bool>().expect("indice ex_data"))
}

fn hex_fp(der: &[u8]) -> String {
    let d = openssl::hash::hash(MessageDigest::sha256(), der).map(|d| d.to_vec()).unwrap_or_default();
    d.iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(":")
}

impl Tls {
    fn new(pinned: Option<String>) -> io::Result<Self> {
        let session: Arc<Mutex<Option<SslSession>>> = Arc::default();
        let mut b = SslConnector::builder(SslMethod::tls_client()).map_err(io::Error::other)?;
        // ⛔ TLS 1.2 al massimo: in TLS 1.3 OpenSSL usa ogni biglietto di
        //    sessione UNA volta sola, e il secondo canale dati sullo stesso
        //    controllo non riusa più la sessione: ProFTPD lo rifiuta («client
        //    did not reuse TLS session», visto il 10/11). In TLS 1.2 la sessione
        //    si riusa quante volte serve, come si aspettano vsftpd e ProFTPD.
        b.set_max_proto_version(Some(openssl::ssl::SslVersion::TLS1_2)).map_err(io::Error::other)?;
        // le sessioni nuove (anche i ticket di TLS 1.3, che arrivano dopo
        // la stretta di mano) vanno da parte per il canale dati. ⛔ Solo quelle
        // del CONTROLLO: anche i collegamenti dati ricevono ticket, e tenendo
        // quelli il trasferimento seguente riusava la sessione di un canale
        // dati, che ProFTPD rifiuta («425 Unable to build data connection», 10/11)
        b.set_session_cache_mode(SslSessionCacheMode::CLIENT);
        let keep = session.clone();
        b.set_new_session_callback(move |ssl, s| {
            if ssl.ex_data(control_tag()).copied().unwrap_or(false) {
                *keep.lock().unwrap() = Some(s);
            }
        });
        Ok(Tls(Arc::new(TlsInner {
            connector: b.build(),
            session,
            pinned,
            seen: Mutex::new(None),
            verdict: Mutex::new(Verdict::Ok),
        })))
    }

    fn verdict(&self) -> Verdict {
        self.0.verdict.lock().unwrap().clone()
    }
}

fn secure_err(m: impl fmt::Display) -> FtpError {
    FtpError::SecureError(m.to_string())
}

impl suppaftp::TlsConnector for Tls {
    type Stream = Ossl;

    fn connect(&self, domain: &str, stream: TcpStream) -> suppaftp::FtpResult<Ossl> {
        let mut cfg = self.0.connector.configure().map_err(secure_err)?;
        // la catena e il nome li controlla OpenSSL; qui si annota soltanto se
        // qualcosa non torna, e la decisione si prende dopo, sull'impronta
        let failed = Arc::new(AtomicBool::new(false));
        let f = failed.clone();
        cfg.set_verify_callback(SslVerifyMode::PEER, move |ok, _| {
            if !ok {
                f.store(true, Ordering::SeqCst);
            }
            true
        });
        let mut ssl = cfg.into_ssl(domain).map_err(secure_err)?;
        // il primo collegamento di questo contesto è il controllo
        if self.0.seen.lock().unwrap().is_none() {
            ssl.set_ex_data(control_tag(), true);
        }
        if let Some(s) = self.0.session.lock().unwrap().as_ref() {
            // SAFETY: la sessione viene da questo stesso SslConnector
            unsafe { ssl.set_session(s).map_err(secure_err)? };
        }
        let s = ssl.connect(stream).map_err(secure_err)?;
        let fp = s.ssl().peer_certificate().and_then(|c| c.to_der().ok()).map(|d| hex_fp(&d)).unwrap_or_default();
        if s.ssl().session_reused() {
            // niente stretta di mano completa: nessun certificato nuovo da giudicare
        } else {
            let mut seen = self.0.seen.lock().unwrap();
            match seen.as_ref() {
                // il canale dati deve parlare con lo stesso server del controllo
                Some(first) if *first != fp => return Err(secure_err("il canale dati ha un certificato diverso dal controllo")),
                Some(_) => {}
                None => {
                    *seen = Some(fp.clone());
                    if failed.load(Ordering::SeqCst) {
                        let v = match &self.0.pinned {
                            Some(p) if *p == fp => Verdict::Ok,
                            Some(_) => Verdict::Changed(fp.clone()),
                            None => Verdict::New(fp.clone()),
                        };
                        if v != Verdict::Ok {
                            *self.0.verdict.lock().unwrap() = v;
                            return Err(secure_err("certificato da confermare"));
                        }
                    }
                }
            }
        }
        if self.0.session.lock().unwrap().is_none()
            && s.ssl().ex_data(control_tag()).copied().unwrap_or(false)
            && let Some(sess) = s.ssl().session()
        {
            *self.0.session.lock().unwrap() = Some(sess.to_owned());
        }
        Ok(Ossl { stream: s, shutdown: true })
    }
}

// ------------------------------------------------------------------ certificati fidati

/// Le impronte accettate, `host:porta` → SHA-256, accanto alla cassaforte.
fn pins_file() -> Option<PathBuf> {
    crate::vault::dir().map(|d| d.join("certificati-ftps.json"))
}

fn pins() -> BTreeMap<String, String> {
    pins_file()
        .and_then(|f| std::fs::read_to_string(f).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn pending() -> &'static Mutex<Option<(String, String, String)>> {
    static P: OnceLock<Mutex<Option<(String, String, String)>>> = OnceLock::new();
    P.get_or_init(Default::default)
}

/// Fidarsi del certificato appena visto (risposta a HOSTKEY_NEW). `None`:
/// non c'era un certificato FTPS in attesa per questa connessione.
pub fn trust(id: &str) -> Option<Result<(), String>> {
    let mut g = pending().lock().unwrap();
    match g.as_ref() {
        Some((pid, _, _)) if pid == id => {}
        _ => return None,
    }
    let (_, key, fp) = g.take()?;
    Some((|| {
        let file = pins_file().ok_or("cartella di configurazione sconosciuta")?;
        let mut all = pins();
        all.insert(key, fp);
        let json = serde_json::to_string_pretty(&all).map_err(|e| e.to_string())?;
        let tmp = file.with_extension("json.tmp");
        std::fs::write(&tmp, json).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, &file).map_err(|e| e.to_string())
    })())
}

// ------------------------------------------------------------------ profilo e collegamento

#[derive(Deserialize, Default)]
#[serde(default)]
struct Profile {
    id: String,
    host: String,
    port: u16,
    user: String,
    auth: String,
    path: String,
    secure: Option<bool>,
}

type Ftp = ImplFtpStream<Ossl>;

#[derive(Clone)]
struct Cfg {
    host: String,
    port: u16,
    addr: SocketAddr,
    user: String,
    pass: String,
    /// 0 in chiaro, 1 esplicito (AUTH TLS), 2 implicito
    tls_mode: u8,
    /// l'impronta già accettata per questo server (`host:porta` = `key`)
    pin: Option<String>,
    id: String,
    key: String,
}

fn is_ip(h: &str) -> bool {
    h.parse::<std::net::IpAddr>().is_ok()
}

fn tcp(addr: SocketAddr) -> io::Result<TcpStream> {
    let s = TcpStream::connect_timeout(&addr, CONNECT_TIMEOUT)?;
    let _ = s.set_read_timeout(Some(IO_TIMEOUT));
    let _ = s.set_write_timeout(Some(IO_TIMEOUT));
    Ok(s)
}

/// Il motivo di un errore FTP per l'utente e per il motore (che guarda il tipo).
fn ftp_err(e: FtpError) -> io::Error {
    match e {
        FtpError::ConnectionError(e) => e,
        FtpError::UnexpectedResponse(r) => {
            let body = r.as_string().unwrap_or_default();
            let text = body.trim().to_string();
            let low = text.to_lowercase();
            let code = r.status as u32;
            let kind = match code {
                530 | 532 => io::ErrorKind::PermissionDenied,
                550 | 553 | 450 if low.contains("exist") && !low.contains("not exist") && !low.contains("n't exist") => io::ErrorKind::AlreadyExists,
                550 | 553 | 450 if low.contains("denied") || low.contains("permission") || low.contains("not allowed") => io::ErrorKind::PermissionDenied,
                550 | 450 => io::ErrorKind::NotFound,
                _ => io::ErrorKind::Other,
            };
            io::Error::new(kind, if text.is_empty() { format!("errore {code}") } else { text })
        }
        other => io::Error::other(other.to_string()),
    }
}

/// Un errore che vuol dire «il collegamento è caduto»: si ricollega e si riprova.
fn is_dropped(e: &FtpError) -> bool {
    match e {
        FtpError::ConnectionError(_) => true,
        FtpError::UnexpectedResponse(r) => r.status == Status::NotAvailable,
        _ => false,
    }
}

impl Cfg {
    /// Un collegamento nuovo, autenticato, in binario. Gli errori del primo
    /// collegamento sono quelli con codice per l'interfaccia (NET:, AUTH:, HOSTKEY_*).
    fn open(&self) -> Result<Ftp, String> {
        let net = |e: &dyn fmt::Display| format!("NET:{}:{}: {e}", self.host, self.port);
        // ⛔ un contesto TLS (e una sessione) PER COLLEGAMENTO: il canale dati
        //    deve riusare la sessione del SUO controllo. Con una sessione sola
        //    per tutti, il collegamento di un trasferimento sovrascriveva quella
        //    del collegamento per sfogliare, e vsftpd rifiutava l'elenco
        //    («522 SSL connection failed: session reuse required», 10/11)
        let tls = if self.tls_mode > 0 { Some(Tls::new(self.pin.clone()).map_err(|e| e.to_string())?) } else { None };
        let mut ftp = match self.tls_mode {
            2 => {
                let t = tls.clone().ok_or("TLS non pronto")?;
                #[allow(deprecated)]
                let r = Ftp::connect_secure_implicit(self.addr, t.clone(), &self.host);
                r.map_err(|e| self.tls_failure(&t).unwrap_or_else(|| net(&e)))?
            }
            _ => Ftp::connect_with_stream(tcp(self.addr).map_err(|e| net(&e))?).map_err(|e| net(&e))?,
        };
        if self.tls_mode == 1 {
            let t = tls.clone().ok_or("TLS non pronto")?;
            ftp = ftp.into_secure(t.clone(), &self.host).map_err(|e| {
                self.tls_failure(&t).unwrap_or_else(|| match &e {
                    FtpError::UnexpectedResponse(_) => "il server non offre FTPS (AUTH TLS rifiutato): togli «Cifrata» o chiedi FTPS al server".to_string(),
                    _ => format!("TLS: {e}"),
                })
            })?;
        }
        ftp.login(self.user.as_str(), self.pass.as_str()).map_err(|e| match &e {
            FtpError::UnexpectedResponse(r) if matches!(r.status as u32, 530 | 331 | 332) => {
                format!("AUTH:{}", r.as_string().unwrap_or_default().trim())
            }
            _ => e.to_string(),
        })?;
        ftp.transfer_type(FileType::Binary).map_err(|e| e.to_string())?;
        // un server dietro NAT può annunciare per PASV il suo indirizzo
        // interno: si usa sempre quello a cui ci si è collegati
        ftp.set_passive_nat_workaround(true);
        let addr = self.addr;
        ftp = ftp.passive_stream_builder(move |data: SocketAddr| {
            let to = if data.ip().is_unspecified() { SocketAddr::new(addr.ip(), data.port()) } else { data };
            tcp(to).map_err(FtpError::ConnectionError)
        });
        Ok(ftp)
    }

    fn tls_failure(&self, t: &Tls) -> Option<String> {
        match t.verdict() {
            Verdict::Ok => None,
            Verdict::New(fp) => {
                *pending().lock().unwrap() = Some((self.id.clone(), self.key.clone(), fp.clone()));
                Some(format!("HOSTKEY_NEW:TLS:{fp}"))
            }
            Verdict::Changed(fp) => Some(format!("HOSTKEY_CHANGED:TLS:{fp}")),
        }
    }

    fn reopen(&self) -> io::Result<Ftp> {
        self.open().map_err(|e| io::Error::new(io::ErrorKind::ConnectionAborted, e))
    }
}

/// I collegamenti di trasferimento finiti bene, pronti per il prossimo file:
/// un accesso può costare mezzo secondo (ProFTPD), e copiando tanti file
/// piccoli si pagherebbe a ogni file (30 file: 13-16 s invece di pochi).
type Pool = Arc<Mutex<Vec<Ftp>>>;
const POOL_MAX: usize = 2;

fn give_back(pool: &Pool, ftp: Ftp) {
    let mut g = pool.lock().unwrap();
    if g.len() < POOL_MAX {
        g.push(ftp);
    } else {
        drop(g);
        let mut ftp = ftp;
        let _ = ftp.quit();
    }
}

pub struct FtpDrive {
    cfg: Cfg,
    ctl: Mutex<Option<Ftp>>,
    pool: Pool,
    mlsd: bool,
    mlst: bool,
    start: String,
}

pub fn connect(profile_json: &str, secret: &str) -> Result<Arc<dyn Drive>, String> {
    let p: Profile = serde_json::from_str(profile_json).map_err(|e| format!("profilo non valido: {e}"))?;
    let host = p.host.trim().trim_start_matches("ftps://").trim_start_matches("ftp://").trim_end_matches('/').to_string();
    if host.is_empty() {
        return Err("manca l'indirizzo del server".into());
    }
    let secure = p.secure.unwrap_or(true);
    let port = if p.port == 0 { 21 } else { p.port };
    let tls_mode = if !secure { 0 } else if port == 990 { 2 } else { 1 };
    let anonymous = p.auth == "anonymous";
    let (user, pass) = if anonymous {
        let u = if p.user.trim().is_empty() { "anonymous".to_string() } else { p.user.trim().to_string() };
        (u, "anonymous@".to_string())
    } else {
        if secret.is_empty() {
            return Err("AUTH:serve la password".into());
        }
        (p.user.trim().to_string(), secret.to_string())
    };
    let addr = (host.as_str(), port)
        .to_socket_addrs()
        .map_err(|e| format!("NET:{host}: {e}"))?
        .next()
        .ok_or_else(|| format!("NET:{host}: indirizzo sconosciuto"))?;
    let key = format!("{host}:{port}");
    let pin = pins().get(&key).cloned();
    let cfg = Cfg { host: if is_ip(&host) { host.clone() } else { host.to_lowercase() }, port, addr, user, pass, tls_mode, pin, id: p.id.clone(), key };
    let mut ftp = cfg.open()?;
    let feat = ftp.feat().unwrap_or_default();
    let has = |k: &str| feat.keys().any(|f| f.eq_ignore_ascii_case(k));
    if has("UTF8") {
        let _ = ftp.opts("UTF8", Some("ON"));
    }
    let start = if p.path.trim().is_empty() { ftp.pwd().unwrap_or_else(|_| "/".into()) } else { p.path.trim().to_string() };
    let start = if start.starts_with('/') { start } else { format!("/{start}") };
    let d = FtpDrive { mlsd: has("MLSD") || has("MLST"), mlst: has("MLST"), start, cfg, ctl: Mutex::new(Some(ftp)), pool: Pool::default() };
    // la cartella di partenza deve esistere
    d.stat(&d.start.clone()).map_err(|e| format!("cartella iniziale «{}»: {e}", d.start))?;
    Ok(Arc::new(d))
}

fn parent_and_name(path: &str) -> (String, String) {
    let t = path.trim_end_matches('/');
    match t.rfind('/') {
        Some(0) => ("/".into(), t[1..].to_string()),
        Some(i) => (t[..i].to_string(), t[i + 1..].to_string()),
        None => ("/".into(), t.to_string()),
    }
}

fn rstat(f: &LFile) -> RStat {
    let mtime = f.modified().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    // un link simbolico: l'elenco non dice dove porta, quindi resta un file
    // (scaricarlo segue il link sul server)
    let dir = f.is_directory();
    RStat { is_dir: dir, is_file: !dir, is_link: false, size: if dir { 0 } else { f.size() as u64 }, mode: if dir { 0o755 } else { 0o644 }, mtime }
}

impl FtpDrive {
    /// Un comando sul collegamento di controllo, che si ricollega una volta se è caduto
    /// (i server chiudono quelli fermi da qualche minuto).
    fn with<T>(&self, f: impl Fn(&mut Ftp) -> Result<T, FtpError>) -> io::Result<T> {
        let mut g = self.ctl.lock().unwrap();
        if g.is_none() {
            *g = Some(self.cfg.reopen()?);
        }
        match f(g.as_mut().unwrap()) {
            Ok(v) => Ok(v),
            Err(e) if is_dropped(&e) => {
                *g = Some(self.cfg.reopen()?);
                f(g.as_mut().unwrap()).map_err(ftp_err)
            }
            Err(e) => Err(ftp_err(e)),
        }
    }

    /// Un trasferimento su un collegamento suo: uno già pronto se c'è (e se
    /// nel frattempo il server l'ha chiuso, uno nuovo).
    fn transfer(&self, begin: impl Fn(&mut Ftp) -> Result<TransferStream<Ossl>, FtpError>) -> io::Result<(Ftp, TransferStream<Ossl>)> {
        let ready = self.pool.lock().unwrap().pop();
        if let Some(mut f) = ready {
            match begin(&mut f) {
                Ok(s) => return Ok((f, s)),
                Err(e) if is_dropped(&e) => {}
                Err(e) => {
                    give_back(&self.pool, f);
                    return Err(ftp_err(e));
                }
            }
        }
        let mut f = self.cfg.reopen()?;
        let s = begin(&mut f).map_err(ftp_err)?;
        Ok((f, s))
    }

    fn parse_list(lines: &[String], mlsd: bool) -> Vec<(String, RStat)> {
        lines
            .iter()
            .filter_map(|l| {
                let f = if mlsd { ListParser::parse_mlsd(l).ok()? } else { ListParser::parse_posix(l).or_else(|_| ListParser::parse_dos(l)).ok()? };
                let name = f.name().to_string();
                (name != "." && name != ".." && !name.is_empty()).then(|| (name, rstat(&f)))
            })
            .collect()
    }
}

impl Drive for FtpDrive {
    fn start(&self) -> String {
        self.start.clone()
    }

    fn stat(&self, path: &str) -> io::Result<RStat> {
        if path.trim_matches('/').is_empty() {
            return Ok(RStat { is_dir: true, mode: 0o755, ..Default::default() });
        }
        if self.mlst
            && let Ok(line) = self.with(|f| f.mlst(Some(path)))
            && let Ok(file) = ListParser::parse_mlst(line.trim_start_matches('\n'))
        {
            return Ok(rstat(&file));
        }
        let (parent, name) = parent_and_name(path);
        self.list(&parent)?
            .into_iter()
            .find(|(n, _)| *n == name)
            .map(|(_, st)| st)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "non esiste"))
    }

    fn list(&self, path: &str) -> io::Result<Vec<(String, RStat)>> {
        if self.mlsd {
            let lines = self.with(|f| f.mlsd(Some(path)))?;
            return Ok(Self::parse_list(&lines, true));
        }
        // LIST con -a per i file nascosti (non tutti i server lo capiscono,
        // e allora si ripiega su LIST semplice), dalla cartella stessa
        let lines = self.with(|f| {
            f.cwd(path)?;
            match f.list(Some("-a")) {
                Ok(l) => Ok(l),
                Err(e) if is_dropped(&e) => Err(e),
                Err(_) => f.list(None),
            }
        })?;
        Ok(Self::parse_list(&lines, false))
    }

    fn read(&self, path: &str) -> io::Result<Box<dyn Read + Send>> {
        let (ftp, s) = self.transfer(|f| f.retr_as_stream(path))?;
        Ok(Box::new(FtpReader { s: Some(s), ftp: Some(ftp), pool: self.pool.clone(), clean: false }))
    }

    fn writer(self: Arc<Self>, path: &str, _size: Option<u64>) -> io::Result<Box<dyn crate::vfs::Sink>> {
        let (ftp, s) = self.transfer(|f| f.put_with_stream(path))?;
        Ok(Box::new(FtpSink { s: Some(s), ftp: Some(ftp), pool: self.pool.clone(), path: path.to_string(), done: false }))
    }

    fn mkdir(&self, path: &str) -> io::Result<()> {
        self.with(|f| f.mkdir(path)).map_err(|e| {
            // «550» su una cartella che c'è già: il motore vuole AlreadyExists
            if e.kind() != io::ErrorKind::AlreadyExists && self.stat(path).map(|s| s.is_dir).unwrap_or(false) {
                io::Error::new(io::ErrorKind::AlreadyExists, "esiste già")
            } else {
                e
            }
        })
    }

    /// File o cartella, anche piena: FTP non ha un'eliminazione ricorsiva.
    fn delete(&self, path: &str) -> io::Result<()> {
        let st = self.stat(path)?;
        if st.is_dir {
            for (name, _) in self.list(path)? {
                self.delete(&format!("{}/{name}", path.trim_end_matches('/')))?;
            }
            self.with(|f| f.rmdir(path))
        } else {
            self.with(|f| f.rm(path))
        }
    }

    fn rename(&self, from: &str, to: &str) -> io::Result<()> {
        self.with(|f| f.rename(from, to))
    }
}

/// Lettura su un collegamento suo: alla fine si legge l'esito del server e,
/// se tutto è andato bene, il collegamento torna pronto per il prossimo file.
struct FtpReader {
    s: Option<TransferStream<Ossl>>,
    ftp: Option<Ftp>,
    pool: Pool,
    clean: bool,
}

impl Read for FtpReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let Some(s) = self.s.as_mut() else { return Ok(0) };
        let n = s.read(buf)?;
        if n == 0 && !buf.is_empty() {
            // tutto arrivato: il server deve confermare (226), sennò è un errore
            let s = self.s.take().unwrap();
            s.finish().map_err(ftp_err)?;
            self.clean = true;
        }
        Ok(n)
    }
}

impl Drop for FtpReader {
    fn drop(&mut self) {
        let Some(mut ftp) = self.ftp.take() else { return };
        if self.clean {
            give_back(&self.pool, ftp);
        } else {
            // interrotta a metà (Annulla): si chiude e non si riusa
            drop(self.s.take());
            let _ = ftp.quit();
        }
    }
}

/// Scrittura su un collegamento suo; `flush` chiude il caricamento. Senza
/// flush (Annulla o errore) il file a metà sul server viene tolto.
struct FtpSink {
    s: Option<TransferStream<Ossl>>,
    ftp: Option<Ftp>,
    pool: Pool,
    path: String,
    done: bool,
}

impl Write for FtpSink {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        match self.s.as_mut() {
            Some(s) => s.write(data),
            None => Err(io::Error::other("caricamento già chiuso")),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        if self.done {
            return Ok(());
        }
        if let Some(mut s) = self.s.take() {
            s.flush()?;
            s.finish().map_err(ftp_err)?;
        }
        self.done = true;
        Ok(())
    }
}

impl crate::vfs::Sink for FtpSink {
    fn sync(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Drop for FtpSink {
    fn drop(&mut self) {
        let Some(mut ftp) = self.ftp.take() else { return };
        if self.done {
            give_back(&self.pool, ftp);
        } else {
            drop(self.s.take());
            let _ = ftp.rm(self.path.as_str());
            let _ = ftp.quit();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percorsi() {
        assert_eq!(parent_and_name("/a/b/c.txt"), ("/a/b".into(), "c.txt".into()));
        assert_eq!(parent_and_name("/c.txt"), ("/".into(), "c.txt".into()));
        assert_eq!(parent_and_name("/a/dir/"), ("/a".into(), "dir".into()));
    }

    #[test]
    fn elenchi() {
        // LIST in stile Unix (vsftpd, ProFTPD senza MLSD), con nomi difficili
        let unix = vec![
            "total 3".to_string(),
            "drwxr-xr-x    2 1000     1000         4096 Nov 10 07:58 sotto".to_string(),
            "-rw-r--r--    1 1000     1000     31457280 Nov 10 07:58 Città è [prova] #1 %.bin".to_string(),
            "-rw-r--r--    1 1000     1000            2 Nov 10 07:58 .nascosto".to_string(),
            "drwxr-xr-x    2 1000     1000         4096 Nov 10 07:58 .".to_string(),
        ];
        let mut v = FtpDrive::parse_list(&unix, false);
        v.sort_by(|a, b| a.0.cmp(&b.0));
        let n: Vec<_> = v.iter().map(|(n, s)| (n.as_str(), s.is_dir, s.size)).collect();
        assert_eq!(n, vec![(".nascosto", false, 2), ("Città è [prova] #1 %.bin", false, 31457280), ("sotto", true, 0)]);
        // MLSD (RFC 3659), con le voci della cartella stessa da saltare
        let mlsd = vec![
            "type=cdir;modify=20261110065800; .".to_string(),
            "type=dir;modify=20261110065800;UNIX.mode=0755; sotto".to_string(),
            "type=file;size=14;modify=20261110065800;UNIX.mode=0644; leggi mi.txt".to_string(),
        ];
        let v = FtpDrive::parse_list(&mlsd, true);
        assert_eq!(v.len(), 2);
        let f = v.iter().find(|x| x.0 == "leggi mi.txt").unwrap();
        assert!(f.1.is_file && f.1.size == 14 && f.1.mtime > 1_700_000_000);
        // stile Windows (IIS)
        let dos = vec!["11-10-26  07:58AM       <DIR>          Cartella".to_string(), "11-10-26  07:58AM                 1234 file.txt".to_string()];
        let v = FtpDrive::parse_list(&dos, false);
        assert_eq!(v.len(), 2);
        assert!(v.iter().any(|x| x.0 == "Cartella" && x.1.is_dir));
    }
}
