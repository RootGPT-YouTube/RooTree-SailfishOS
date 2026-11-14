//! S3 e servizi compatibili (Fase 5, step 5): Amazon S3, MinIO, Backblaze B2,
//! Wasabi, Cloudflare R2, lo S3 dei NAS. Implementa `Drive`, quindi copia,
//! coda, Annulla e conflitti valgono come per gli altri remoti.
//!
//! Niente libreria S3: la firma (AWS Signature Version 4) è scritta qui sopra
//! ureq, come WebDAV e Dropbox. Il segreto della connessione è la «secret
//! access key», nella cassaforte; l'«access key ID» sta nel profilo (user).
//!
//! S3 non ha cartelle: ci sono solo oggetti con nomi tipo «a/b/c.txt». Le
//! cartelle si ricavano dai prefissi (ListObjectsV2 con delimitatore «/»), e
//! una cartella vuota è un oggetto vuoto «nome/», come fa la console di AWS.
//! Senza bucket nel profilo, la radice è l'elenco dei bucket (come le
//! condivisioni di SMB): «/bucket/a/b».

use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::Engine;
use hmac::{Hmac, Mac};
use quick_xml::Reader;
use quick_xml::events::Event;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use ureq::Agent;
use ureq::SendBody;
use ureq::http::{Method, Request};
use ureq::tls::{TlsConfig, TlsProvider};

use crate::cloud::Drive;
use crate::http;
use crate::remote::RStat;

/// Fino a qui un file va con una PUT sola (il limite di S3 è 5 GiB).
const SINGLE_MAX: u64 = 5 * 1024 * 1024 * 1024;
/// I pezzi del caricamento a pezzi (S3: almeno 5 MiB, al più 10 000 pezzi).
const PART_MIN: u64 = 16 * 1024 * 1024;
/// La copia sul server (CopyObject) arriva a 5 GiB: oltre, il motore scarica e ricarica.
const COPY_MAX: u64 = 5 * 1024 * 1024 * 1024;
const EXDEV: i32 = 18;
const EMPTY_SHA: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

fn agent() -> &'static Agent {
    static A: OnceLock<Agent> = OnceLock::new();
    A.get_or_init(|| {
        Agent::config_builder()
            .http_status_as_error(false)
            // un rinvio cambierebbe l'indirizzo firmato: la regione giusta si legge e si riprova
            .max_redirects(0)
            .timeout_connect(Some(Duration::from_secs(15)))
            .timeout_recv_response(Some(Duration::from_secs(600)))
            .tls_config(TlsConfig::builder().provider(TlsProvider::NativeTls).build())
            .build()
            .into()
    })
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Profile {
    /// l'endpoint: vuoto per Amazon; può avere schema e porta (http://nas:9000)
    host: String,
    /// l'access key ID
    user: String,
    bucket: String,
    region: String,
}

/// Il corpo di una richiesta.
enum Body<'a> {
    Empty,
    /// firmato col suo SHA-256
    Bytes(&'a [u8]),
}

impl<'a> Body<'a> {
    fn clone_of(b: &Body<'a>) -> Body<'a> {
        match b {
            Body::Empty => Body::Empty,
            Body::Bytes(x) => Body::Bytes(x),
        }
    }
}

pub struct S3 {
    /// "https" o "http"
    scheme: String,
    /// host[:porta] dell'endpoint; vuoto = Amazon (dipende dalla regione)
    endpoint: String,
    aws: bool,
    key_id: String,
    secret: String,
    /// il bucket del profilo ("" = tutti, la radice li elenca)
    bucket: String,
    /// la regione di partenza (quella del profilo o trovata al collegamento)
    region: Mutex<String>,
    /// Amazon: la regione di ogni bucket, imparata dalle risposte
    regions: Mutex<HashMap<String, String>>,
}

// ------------------------------------------------------------------ firma

type HmacSha256 = Hmac<Sha256>;

fn hmac(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut m = HmacSha256::new_from_slice(key).expect("hmac accetta ogni chiave");
    m.update(data);
    m.finalize().into_bytes().to_vec()
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn sha_hex(b: &[u8]) -> String {
    hex(&Sha256::digest(b))
}

/// (AAAAMMGGThhmmssZ, AAAAMMGG) di un istante Unix, in UTC.
pub fn amz_date(t: u64) -> (String, String) {
    let days = (t / 86400) as i64;
    let rem = t % 86400;
    // da giorni a data civile (Howard Hinnant)
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    let date = format!("{y:04}{m:02}{d:02}");
    let full = format!("{date}T{:02}{:02}{:02}Z", rem / 3600, rem % 3600 / 60, rem % 60);
    (full, date)
}

/// La firma AWS Signature Version 4 di una richiesta: il valore
/// dell'intestazione Authorization. `headers` già in minuscolo, compreso host.
#[allow(clippy::too_many_arguments)]
pub fn sign(
    key_id: &str,
    secret: &str,
    region: &str,
    method: &str,
    canonical_uri: &str,
    canonical_query: &str,
    headers: &[(String, String)],
    payload_hash: &str,
    amz_full: &str,
    amz_day: &str,
) -> String {
    let mut h: Vec<(String, String)> = headers.iter().map(|(k, v)| (k.to_lowercase(), v.trim().to_string())).collect();
    h.sort();
    let canon_headers: String = h.iter().map(|(k, v)| format!("{k}:{v}\n")).collect();
    let signed: Vec<&str> = h.iter().map(|(k, _)| k.as_str()).collect();
    let signed = signed.join(";");
    let canonical = format!("{method}\n{canonical_uri}\n{canonical_query}\n{canon_headers}\n{signed}\n{payload_hash}");
    let scope = format!("{amz_day}/{region}/s3/aws4_request");
    let to_sign = format!("AWS4-HMAC-SHA256\n{amz_full}\n{scope}\n{}", sha_hex(canonical.as_bytes()));
    let k = hmac(format!("AWS4{secret}").as_bytes(), amz_day.as_bytes());
    let k = hmac(&k, region.as_bytes());
    let k = hmac(&k, b"s3");
    let k = hmac(&k, b"aws4_request");
    let sig = hex(&hmac(&k, to_sign.as_bytes()));
    format!("AWS4-HMAC-SHA256 Credential={key_id}/{scope}, SignedHeaders={signed}, Signature={sig}")
}

/// La query canonica (e dell'indirizzo): chiavi in ordine, tutto codificato.
fn canonical_query(q: &[(&str, String)]) -> String {
    let mut v: Vec<(String, String)> = q.iter().map(|(k, val)| (http::pct(k, false), http::pct(val, false))).collect();
    v.sort();
    v.iter().map(|(k, val)| format!("{k}={val}")).collect::<Vec<_>>().join("&")
}

// ------------------------------------------------------------------ errori

/// Codice e messaggio dall'XML d'errore di S3.
fn error_code(body: &str) -> (String, String) {
    let pick = |tag: &str| {
        let open = format!("<{tag}>");
        body.find(&open)
            .and_then(|i| body[i + open.len()..].find(&format!("</{tag}>")).map(|j| body[i + open.len()..i + open.len() + j].to_string()))
            .unwrap_or_default()
    };
    (pick("Code"), pick("Message"))
}

fn s3_err(status: u16, body: &str, bucket: &str) -> io::Error {
    let (code, msg) = error_code(body);
    match code.as_str() {
        "NoSuchKey" | "NoSuchUpload" => io::Error::new(io::ErrorKind::NotFound, "non esiste"),
        "NoSuchBucket" => io::Error::new(io::ErrorKind::NotFound, format!("il bucket «{bucket}» non esiste")),
        "InvalidAccessKeyId" => io::Error::new(io::ErrorKind::PermissionDenied, "AUTH:access key ID sconosciuta al servizio"),
        "SignatureDoesNotMatch" => io::Error::new(io::ErrorKind::PermissionDenied, "AUTH:la secret access key non corrisponde"),
        // Garage (e altri) dicono AccessDenied anche per una chiave sbagliata: lo dice il messaggio
        "AccessDenied" if is_bad_key(&msg) => io::Error::new(io::ErrorKind::PermissionDenied, format!("AUTH:chiavi non accettate dal servizio ({msg})")),
        "AccessDenied" | "AllAccessDisabled" => io::Error::new(io::ErrorKind::PermissionDenied, format!("permesso negato dal servizio: {msg}")),
        "RequestTimeTooSkewed" => io::Error::other("l'ora del telefono è sbagliata di oltre 15 minuti: S3 rifiuta le richieste"),
        "BucketNotEmpty" => io::Error::other("il bucket non è vuoto"),
        "BucketAlreadyExists" | "BucketAlreadyOwnedByYou" => io::Error::new(io::ErrorKind::AlreadyExists, "esiste già un bucket con questo nome"),
        "InvalidBucketName" => io::Error::other("nome di bucket non valido: 3-63 caratteri, minuscole, cifre, punti e trattini"),
        "EntityTooLarge" => io::Error::other("file troppo grande per il servizio"),
        "AuthorizationHeaderMalformed" | "InvalidRegion" | "IllegalLocationConstraintException" => {
            io::Error::other(format!("REGION:il servizio vuole un'altra regione: scrivi nel profilo quella che indica (es. eu-central-1, auto, garage) ({msg})"))
        }
        "QuotaExceeded" | "XMinioStorageFull" => io::Error::other("spazio esaurito sul servizio"),
        _ => match status {
            404 => io::Error::new(io::ErrorKind::NotFound, "non esiste"),
            403 => io::Error::new(io::ErrorKind::PermissionDenied, if msg.is_empty() { "permesso negato dal servizio".to_string() } else { format!("permesso negato dal servizio: {msg}") }),
            _ if !code.is_empty() => io::Error::other(format!("errore {status} dal servizio: {code} {msg}")),
            _ => http::status_error(status, body),
        },
    }
}

/// Un rifiuto che riguarda le chiavi (non i permessi di una chiave buona).
fn is_bad_key(msg: &str) -> bool {
    let m = msg.to_lowercase();
    m.contains("signature") || m.contains("no such key") || m.contains("access key") || m.contains("invalid key") || m.contains("credential")
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&apos;")
}

// ------------------------------------------------------------------ indirizzi

/// Da un endpoint scritto come capita a (schema, host[:porta]): la porta
/// standard dello schema si toglie, perché la firma conta l'host com'è.
pub fn split_endpoint(e: &str) -> (String, String) {
    let t = e.trim();
    let (scheme, rest) = if let Some(r) = t.strip_prefix("https://") {
        ("https", r)
    } else if let Some(r) = t.strip_prefix("http://") {
        ("http", r)
    } else {
        ("https", t)
    };
    let host = rest.split('/').next().unwrap_or("").to_lowercase();
    let host = match (scheme, host.rsplit_once(':')) {
        ("https", Some((h, "443"))) | ("http", Some((h, "80"))) => h.to_string(),
        _ => host,
    };
    (scheme.to_string(), host)
}

impl S3 {
    fn region_for(&self, bucket: &str) -> String {
        if !bucket.is_empty()
            && let Some(r) = self.regions.lock().unwrap().get(bucket)
        {
            return r.clone();
        }
        self.region.lock().unwrap().clone()
    }

    /// (host, URI canonico) di bucket e chiave. Amazon: «bucket.s3.regione…»
    /// quando si può (lo stile a percorso è sconsigliato), gli altri a percorso.
    fn place(&self, bucket: &str, key: &str, region: &str) -> (String, String) {
        let k = http::pct(key, true);
        if self.aws {
            let ep = if !self.endpoint.is_empty() {
                self.endpoint.clone()
            } else if region == "us-east-1" {
                "s3.amazonaws.com".to_string()
            } else {
                format!("s3.{region}.amazonaws.com")
            };
            // un bucket coi punti non sta nel certificato *.s3…: a percorso
            if !bucket.is_empty() && !bucket.contains('.') && self.endpoint.is_empty() {
                return (format!("{bucket}.{ep}"), format!("/{k}"));
            }
            if bucket.is_empty() {
                return (ep, "/".into());
            }
            return (ep, if key.is_empty() { format!("/{}", http::pct(bucket, false)) } else { format!("/{}/{k}", http::pct(bucket, false)) });
        }
        if bucket.is_empty() {
            return (self.endpoint.clone(), "/".into());
        }
        let b = http::pct(bucket, false);
        (self.endpoint.clone(), if key.is_empty() { format!("/{b}") } else { format!("/{b}/{k}") })
    }

    /// Una richiesta firmata. `extra`: intestazioni in più (firmate anche loro).
    /// Amazon dice la regione giusta di un bucket in `x-amz-bucket-region`:
    /// si impara e si riprova una volta.
    fn send(&self, method: &str, bucket: &str, key: &str, query: &[(&str, String)], extra: &[(&str, String)], body: Body) -> io::Result<http::Resp> {
        let mut retried = false;
        loop {
            let region = self.region_for(bucket);
            let r = self.send_once(method, bucket, key, query, extra, &body, &region, None)?;
            let status = r.status().as_u16();
            if !retried
                && matches!(status, 301 | 307 | 400)
                && let Some(right) = r.headers().get("x-amz-bucket-region").and_then(|v| v.to_str().ok())
                && !right.is_empty()
                && right != region
            {
                if bucket.is_empty() {
                    *self.region.lock().unwrap() = right.to_string();
                } else {
                    self.regions.lock().unwrap().insert(bucket.to_string(), right.to_string());
                }
                retried = true;
                continue;
            }
            return Ok(r);
        }
    }

    /// Una richiesta firmata, una volta. `stream`: un corpo che arriva a pezzi
    /// (non firmato: UNSIGNED-PAYLOAD) con la sua lunghezza.
    #[allow(clippy::too_many_arguments)]
    fn send_once(
        &self,
        method: &str,
        bucket: &str,
        key: &str,
        query: &[(&str, String)],
        extra: &[(&str, String)],
        body: &Body,
        region: &str,
        stream: Option<(&mut dyn Read, u64)>,
    ) -> io::Result<http::Resp> {
        let (host, uri) = self.place(bucket, key, region);
        let q = canonical_query(query);
        let payload = match (body, &stream) {
            (_, Some(_)) => "UNSIGNED-PAYLOAD".to_string(),
            (Body::Empty, None) => EMPTY_SHA.to_string(),
            (Body::Bytes(b), None) => sha_hex(b),
        };
        let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let (full, day) = amz_date(now);
        let mut headers: Vec<(String, String)> = vec![
            ("host".into(), host.clone()),
            ("x-amz-content-sha256".into(), payload.clone()),
            ("x-amz-date".into(), full.clone()),
        ];
        for (k, v) in extra {
            headers.push((k.to_lowercase(), v.clone()));
        }
        let auth = sign(&self.key_id, &self.secret, region, method, &uri, &q, &headers, &payload, &full, &day);
        let url = format!("{}://{host}{uri}{}{q}", self.scheme, if q.is_empty() { "" } else { "?" });
        let mut b = Request::builder().method(Method::from_bytes(method.as_bytes()).map_err(io::Error::other)?).uri(url);
        for (k, v) in &headers {
            if k != "host" {
                b = b.header(k.as_str(), v.as_str());
            }
        }
        b = b.header("authorization", auth.as_str());
        let r = match (body, stream) {
            (_, Some((reader, len))) => {
                b = b.header("content-length", len.to_string());
                agent().run(b.body(SendBody::from_reader(reader)).map_err(io::Error::other)?)
            }
            (Body::Empty, None) => agent().run(b.body(()).map_err(io::Error::other)?),
            (Body::Bytes(data), None) => agent().run(b.body(*data).map_err(io::Error::other)?),
        };
        r.map_err(http::net)
    }

    /// Una richiesta che deve riuscire (2xx); rende il corpo.
    fn ok(&self, method: &str, bucket: &str, key: &str, query: &[(&str, String)], extra: &[(&str, String)], body: Body) -> io::Result<(http::Resp, String)> {
        let mut r = self.send(method, bucket, key, query, extra, Body::clone_of(&body))?;
        let mut status = r.status().as_u16();
        let mut text = if method == "HEAD" { String::new() } else { r.body_mut().read_to_string().unwrap_or_default() };
        // Amazon scrive la regione giusta anche nel corpo dell'errore
        if status == 400
            && let Some(right) = tag_text(&text, "Region").filter(|x| !x.is_empty() && *x != self.region_for(bucket))
        {
            if bucket.is_empty() {
                *self.region.lock().unwrap() = right;
            } else {
                self.regions.lock().unwrap().insert(bucket.to_string(), right);
            }
            r = self.send(method, bucket, key, query, extra, body)?;
            status = r.status().as_u16();
            text = if method == "HEAD" { String::new() } else { r.body_mut().read_to_string().unwrap_or_default() };
        }
        if (200..300).contains(&status) {
            // CopyObject e CompleteMultipartUpload possono fallire con 200 e un <Error> nel corpo
            if text.contains("<Error>") {
                return Err(s3_err(500, &text, bucket));
            }
            return Ok((r, text));
        }
        Err(s3_err(status, &text, bucket))
    }

    /// (bucket, chiave) di un percorso «/a/b» della connessione.
    fn loc(&self, path: &str) -> (String, String) {
        let p = path.trim_matches('/');
        if !self.bucket.is_empty() {
            return (self.bucket.clone(), p.to_string());
        }
        match p.split_once('/') {
            Some((b, k)) => (b.to_string(), k.trim_matches('/').to_string()),
            None => (p.to_string(), String::new()),
        }
    }

    /// Un oggetto: Some(dimensione, data) o None se non c'è.
    fn head(&self, bucket: &str, key: &str) -> io::Result<Option<(u64, i64)>> {
        let r = self.send("HEAD", bucket, key, &[], &[], Body::Empty)?;
        match r.status().as_u16() {
            200 => {
                let h = r.headers();
                let size = h.get("content-length").and_then(|v| v.to_str().ok()).and_then(|v| v.parse().ok()).unwrap_or(0);
                let mtime = h.get("last-modified").and_then(|v| v.to_str().ok()).map(crate::webdav::http_date).unwrap_or(0);
                Ok(Some((size, mtime)))
            }
            404 => Ok(None),
            403 => Err(io::Error::new(io::ErrorKind::PermissionDenied, "permesso negato dal servizio")),
            s => Err(io::Error::other(format!("errore {s} dal servizio"))),
        }
    }

    /// Una pagina di ListObjectsV2.
    fn list_page(&self, bucket: &str, prefix: &str, delimiter: bool, token: Option<&str>, max: Option<u32>) -> io::Result<Listing> {
        let mut q: Vec<(&str, String)> = vec![("list-type", "2".into()), ("prefix", prefix.into())];
        if delimiter {
            q.push(("delimiter", "/".into()));
        }
        if let Some(t) = token {
            q.push(("continuation-token", t.into()));
        }
        if let Some(m) = max {
            q.push(("max-keys", m.to_string()));
        }
        let (_, body) = self.ok("GET", bucket, "", &q, &[], Body::Empty)?;
        parse_list(&body)
    }

    /// Tutte le chiavi sotto un prefisso (senza delimitatore), con le dimensioni.
    fn all_keys(&self, bucket: &str, prefix: &str) -> io::Result<Vec<(String, u64)>> {
        let mut out = Vec::new();
        let mut token: Option<String> = None;
        loop {
            let page = self.list_page(bucket, prefix, false, token.as_deref(), None)?;
            out.extend(page.objects.into_iter().map(|o| (o.key, o.size)));
            match page.next {
                Some(t) if page.truncated => token = Some(t),
                _ => return Ok(out),
            }
        }
    }

    /// Elimina molte chiavi, 1000 per richiesta (DeleteObjects).
    fn delete_keys(&self, bucket: &str, keys: &[String]) -> io::Result<()> {
        for chunk in keys.chunks(1000) {
            let mut xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?><Delete><Quiet>true</Quiet>");
            for k in chunk {
                xml.push_str(&format!("<Object><Key>{}</Key></Object>", xml_escape(k)));
            }
            xml.push_str("</Delete>");
            use md5::Digest as _;
            let md5 = base64::engine::general_purpose::STANDARD.encode(md5::Md5::digest(xml.as_bytes()));
            let (_, body) = self.ok("POST", bucket, "", &[("delete", String::new())], &[("content-md5", md5), ("content-type", "application/xml".into())], Body::Bytes(xml.as_bytes()))?;
            // con Quiet il corpo elenca solo gli errori
            if body.contains("<Error>") {
                let (code, msg) = error_code(&body);
                return Err(io::Error::other(format!("eliminazione non riuscita: {code} {msg}")));
            }
        }
        Ok(())
    }

    fn copy_object(&self, from_bucket: &str, from_key: &str, to_bucket: &str, to_key: &str) -> io::Result<()> {
        let src = format!("/{}/{}", http::pct(from_bucket, false), http::pct(from_key, true));
        self.ok("PUT", to_bucket, to_key, &[], &[("x-amz-copy-source", src)], Body::Empty).map(|_| ())
    }

    /// Mette un oggetto intero (dati già in memoria).
    fn put_bytes(&self, bucket: &str, key: &str, data: &[u8]) -> io::Result<()> {
        self.ok("PUT", bucket, key, &[], &[], Body::Bytes(data)).map(|_| ())
    }
}

/// Una pagina di elenco.
#[derive(Default, Debug)]
pub struct Listing {
    pub objects: Vec<Obj>,
    pub prefixes: Vec<String>,
    pub truncated: bool,
    pub next: Option<String>,
}

#[derive(Default, Debug, Clone)]
pub struct Obj {
    pub key: String,
    pub size: u64,
    pub mtime: i64,
}

/// La risposta di ListObjectsV2.
pub fn parse_list(xml: &str) -> io::Result<Listing> {
    let mut rd = Reader::from_str(xml);
    let mut out = Listing::default();
    let mut path: Vec<Vec<u8>> = Vec::new();
    let mut cur = Obj::default();
    let mut text = String::new();
    loop {
        match rd.read_event().map_err(|e| io::Error::other(format!("risposta illeggibile: {e}")))? {
            Event::Start(e) => {
                path.push(e.local_name().as_ref().to_vec());
                text.clear();
                if e.local_name().as_ref() == b"Contents" {
                    cur = Obj::default();
                }
            }
            Event::Text(t) => text.push_str(&t.unescape().map_err(|e| io::Error::other(e.to_string()))?),
            Event::End(e) => {
                let name = e.local_name().as_ref().to_vec();
                let parent = if path.len() >= 2 { path[path.len() - 2].clone() } else { Vec::new() };
                match (parent.as_slice(), name.as_slice()) {
                    (b"Contents", b"Key") => cur.key = text.clone(),
                    (b"Contents", b"Size") => cur.size = text.trim().parse().unwrap_or(0),
                    (b"Contents", b"LastModified") => cur.mtime = http::rfc3339(text.trim()),
                    (b"CommonPrefixes", b"Prefix") => out.prefixes.push(text.clone()),
                    (b"ListBucketResult", b"IsTruncated") => out.truncated = text.trim() == "true",
                    (b"ListBucketResult", b"NextContinuationToken") => out.next = Some(text.clone()),
                    (_, b"Contents") => out.objects.push(std::mem::take(&mut cur)),
                    _ => {}
                }
                path.pop();
                text.clear();
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(out)
}

/// La risposta di ListBuckets: (nome, data di creazione).
pub fn parse_buckets(xml: &str) -> io::Result<Vec<(String, i64)>> {
    let mut rd = Reader::from_str(xml);
    let mut out = Vec::new();
    let (mut tag, mut name, mut date) = (Vec::<u8>::new(), String::new(), 0i64);
    loop {
        match rd.read_event().map_err(|e| io::Error::other(format!("risposta illeggibile: {e}")))? {
            Event::Start(e) => {
                tag = e.local_name().as_ref().to_vec();
                if tag == b"Bucket" {
                    name.clear();
                    date = 0;
                }
            }
            Event::Text(t) => {
                let v = t.unescape().map_err(|e| io::Error::other(e.to_string()))?;
                match tag.as_slice() {
                    b"Name" => name.push_str(&v),
                    b"CreationDate" => date = http::rfc3339(v.trim()),
                    _ => {}
                }
            }
            Event::End(e) => {
                tag.clear();
                if e.local_name().as_ref() == b"Bucket" && !name.is_empty() {
                    out.push((name.clone(), date));
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(out)
}

fn dir_stat(mtime: i64) -> RStat {
    RStat { is_dir: true, is_file: false, is_link: false, size: 0, mode: 0o755, mtime }
}

fn file_stat(size: u64, mtime: i64) -> RStat {
    RStat { is_dir: false, is_file: true, is_link: false, size, mode: 0o644, mtime }
}

pub fn connect(profile_json: &str, secret: &str) -> Result<Arc<dyn Drive>, String> {
    let p: Profile = serde_json::from_str(profile_json).map_err(|e| format!("profilo non valido: {e}"))?;
    if p.user.trim().is_empty() {
        return Err("AUTH:manca l'access key ID".into());
    }
    if secret.is_empty() {
        return Err("AUTH:serve la secret access key".into());
    }
    let (scheme, endpoint) = split_endpoint(&p.host);
    let aws = endpoint.is_empty() || endpoint.ends_with(".amazonaws.com");
    let bucket = p.bucket.trim().trim_matches('/').to_string();
    let region = match p.region.trim() {
        "" => "us-east-1".to_string(),
        r => r.to_string(),
    };
    let d = S3 {
        scheme,
        endpoint: if aws && endpoint.ends_with(".amazonaws.com") && endpoint.starts_with("s3.") { String::new() } else { endpoint },
        aws,
        key_id: p.user.trim().to_string(),
        secret: secret.to_string(),
        bucket: bucket.clone(),
        region: Mutex::new(region),
        regions: Mutex::new(HashMap::new()),
    };
    // con un endpoint di Amazon scritto a mano («s3.eu-west-1.amazonaws.com»)
    // la regione è nel nome: vale più di quella vuota del profilo
    if d.aws
        && p.region.trim().is_empty()
        && let Some(r) = p.host.trim().split("s3.").nth(1).and_then(|s| s.strip_suffix(".amazonaws.com")).filter(|r| !r.is_empty() && !r.contains('/'))
    {
        *d.region.lock().unwrap() = r.to_string();
    }
    // la prova: le chiavi valgono e il bucket c'è (o si possono elencare i bucket).
    // Regione vuota e un servizio che la rifiuta senza dire quale vuole: si
    // provano le predefinite dei server compatibili più comuni
    let mut tries: Vec<&str> = vec![""];
    if p.region.trim().is_empty() && !d.aws {
        tries.extend(["garage", "auto"]);
    }
    let mut last = Ok(());
    for t in tries {
        if !t.is_empty() {
            *d.region.lock().unwrap() = t.to_string();
        }
        last = probe(&d, &bucket);
        match &last {
            Err(e) if e.starts_with("REGION:") => continue,
            _ => break,
        }
    }
    if let Err(e) = last {
        return Err(e.trim_start_matches("REGION:").to_string());
    }
    Ok(Arc::new(d))
}

/// Le chiavi valgono e il bucket c'è (o si possono elencare i bucket).
fn probe(d: &S3, bucket: &str) -> Result<(), String> {
    if bucket.is_empty() {
        if let Err(e) = d.ok("GET", "", "", &[], &[], Body::Empty) {
            if e.kind() == io::ErrorKind::PermissionDenied && !e.to_string().starts_with("AUTH:") {
                return Err("questa chiave non può elencare i bucket: scrivi il nome del bucket nel profilo".into());
            }
            return Err(e.to_string());
        }
    } else {
        d.list_page(bucket, "", true, None, Some(1)).map_err(|e| e.to_string())?;
    }
    Ok(())
}

impl Drive for S3 {
    fn stat(&self, path: &str) -> io::Result<RStat> {
        let (b, k) = self.loc(path);
        if b.is_empty() {
            return Ok(dir_stat(0));
        }
        if k.is_empty() {
            if !self.bucket.is_empty() {
                return Ok(dir_stat(0));
            }
            // un bucket dell'elenco
            let r = self.send("HEAD", &b, "", &[], &[], Body::Empty)?;
            return match r.status().as_u16() {
                200 => Ok(dir_stat(0)),
                404 => Err(io::Error::new(io::ErrorKind::NotFound, "non esiste")),
                403 => Err(io::Error::new(io::ErrorKind::PermissionDenied, "permesso negato dal servizio")),
                s => Err(io::Error::other(format!("errore {s} dal servizio"))),
            };
        }
        if let Some((size, mtime)) = self.head(&b, &k)? {
            return Ok(file_stat(size, mtime));
        }
        // una cartella: qualcosa con quel prefisso (anche solo l'oggetto «k/»)
        let page = self.list_page(&b, &format!("{k}/"), true, None, Some(1))?;
        if !page.objects.is_empty() || !page.prefixes.is_empty() {
            return Ok(dir_stat(0));
        }
        Err(io::Error::new(io::ErrorKind::NotFound, "non esiste"))
    }

    fn list(&self, path: &str) -> io::Result<Vec<(String, RStat)>> {
        let (b, k) = self.loc(path);
        if b.is_empty() {
            let (_, body) = self.ok("GET", "", "", &[], &[], Body::Empty)?;
            return Ok(parse_buckets(&body)?.into_iter().map(|(n, d)| (n, dir_stat(d))).collect());
        }
        let prefix = if k.is_empty() { String::new() } else { format!("{k}/") };
        let mut out: Vec<(String, RStat)> = Vec::new();
        let mut seen_dirs = std::collections::HashSet::new();
        let mut token: Option<String> = None;
        loop {
            let page = self.list_page(&b, &prefix, true, token.as_deref(), None)?;
            for pfx in page.prefixes {
                let name = pfx.strip_prefix(&prefix).unwrap_or(&pfx).trim_end_matches('/').to_string();
                if !name.is_empty() && !name.contains('/') && seen_dirs.insert(name.clone()) {
                    out.push((name, dir_stat(0)));
                }
            }
            for o in page.objects {
                let name = o.key.strip_prefix(&prefix).unwrap_or(&o.key).to_string();
                // l'oggetto «cartella/» stesso non è una voce
                if name.is_empty() || name.contains('/') {
                    continue;
                }
                out.push((name, file_stat(o.size, o.mtime)));
            }
            match page.next {
                Some(t) if page.truncated => token = Some(t),
                _ => break,
            }
        }
        Ok(out)
    }

    fn read(&self, path: &str) -> io::Result<Box<dyn Read + Send>> {
        let (b, k) = self.loc(path);
        if k.is_empty() {
            return Err(io::Error::new(io::ErrorKind::IsADirectory, "è una cartella"));
        }
        let mut r = self.send("GET", &b, &k, &[], &[], Body::Empty)?;
        let status = r.status().as_u16();
        if status != 200 {
            let body = r.body_mut().read_to_string().unwrap_or_default();
            return Err(s3_err(status, &body, &b));
        }
        Ok(Box::new(r.into_body().into_reader()))
    }

    fn writer(self: Arc<Self>, path: &str, size: Option<u64>) -> io::Result<Box<dyn crate::vfs::Sink>> {
        let (b, k) = self.loc(path);
        if k.is_empty() {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "qui ci vanno solo bucket, non file"));
        }
        match size {
            // senza dimensione (il salvataggio dell'editor): tutto in memoria, si manda a flush
            None => Ok(Box::new(S3Sink { s3: self, bucket: b, key: k, mode: Mode::Buffer(Vec::new()), done: false })),
            // (ROOTREE_S3_SINGLE_MAX: solo per i collaudi, prova il caricamento a pezzi con file piccoli)
            Some(n) if n <= std::env::var("ROOTREE_S3_SINGLE_MAX").ok().and_then(|v| v.parse().ok()).unwrap_or(SINGLE_MAX) => {
                let (tx, rx) = sync_channel::<Vec<u8>>(16);
                let finished = Arc::new(AtomicBool::new(false));
                let (s3, bb, kk, f) = (self.clone(), b.clone(), k.clone(), finished.clone());
                let worker = std::thread::spawn(move || {
                    let mut body = Channel { rx, cur: Vec::new(), pos: 0, finished: f };
                    let region = s3.region_for(&bb);
                    let mut r = s3.send_once("PUT", &bb, &kk, &[], &[], &Body::Empty, &region, Some((&mut body, n)))?;
                    let status = r.status().as_u16();
                    let text = r.body_mut().read_to_string().unwrap_or_default();
                    if (200..300).contains(&status) { Ok(()) } else { Err(s3_err(status, &text, &bb)) }
                });
                Ok(Box::new(S3Sink { s3: self, bucket: b, key: k, mode: Mode::Stream { tx: Some(tx), worker: Some(worker), finished }, done: false }))
            }
            Some(n) => {
                // a pezzi: al più 10 000, almeno 16 MiB l'uno
                let part = PART_MIN.max(n.div_ceil(9_000));
                Ok(Box::new(S3Sink { s3: self, bucket: b, key: k, mode: Mode::Multi { upload: None, part: part as usize, buf: Vec::new(), parts: Vec::new() }, done: false }))
            }
        }
    }

    fn mkdir(&self, path: &str) -> io::Result<()> {
        let (b, k) = self.loc(path);
        if b.is_empty() {
            return Err(io::Error::new(io::ErrorKind::AlreadyExists, "esiste già"));
        }
        if k.is_empty() {
            if !self.bucket.is_empty() {
                return Err(io::Error::new(io::ErrorKind::AlreadyExists, "esiste già"));
            }
            // una cartella nella radice dell'elenco = un bucket nuovo
            let region = self.region_for("");
            let body = if self.aws && region != "us-east-1" {
                format!("<CreateBucketConfiguration xmlns=\"http://s3.amazonaws.com/doc/2006-03-01/\"><LocationConstraint>{region}</LocationConstraint></CreateBucketConfiguration>")
            } else {
                String::new()
            };
            let r = if body.is_empty() { self.ok("PUT", &b, "", &[], &[], Body::Empty) } else { self.ok("PUT", &b, "", &[], &[], Body::Bytes(body.as_bytes())) };
            return r.map(|_| ());
        }
        // S3 sovrascrive in silenzio: «esiste già» come gli altri servizi
        if self.stat(path).is_ok() {
            return Err(io::Error::new(io::ErrorKind::AlreadyExists, "esiste già"));
        }
        self.put_bytes(&b, &format!("{k}/"), &[])
    }

    fn delete(&self, path: &str) -> io::Result<()> {
        let (b, k) = self.loc(path);
        if b.is_empty() || (k.is_empty() && !self.bucket.is_empty()) {
            return Err(io::Error::new(io::ErrorKind::PermissionDenied, "la radice della connessione non si elimina"));
        }
        if k.is_empty() {
            // un bucket intero: prima il contenuto, poi il bucket
            let keys: Vec<String> = self.all_keys(&b, "")?.into_iter().map(|(k, _)| k).collect();
            self.delete_keys(&b, &keys)?;
            return self.ok("DELETE", &b, "", &[], &[], Body::Empty).map(|_| ());
        }
        let file = self.head(&b, &k)?.is_some();
        let inside: Vec<String> = self.all_keys(&b, &format!("{k}/"))?.into_iter().map(|(k, _)| k).collect();
        if !file && inside.is_empty() {
            return Err(io::Error::new(io::ErrorKind::NotFound, "non esiste"));
        }
        if file {
            self.ok("DELETE", &b, &k, &[], &[], Body::Empty)?;
        }
        self.delete_keys(&b, &inside)
    }

    fn rename(&self, from: &str, to: &str) -> io::Result<()> {
        let (fb, fk) = self.loc(from);
        let (tb, tk) = self.loc(to);
        if fk.is_empty() || tk.is_empty() || fb.is_empty() || tb.is_empty() {
            return Err(io::Error::new(io::ErrorKind::Unsupported, "un bucket non si rinomina"));
        }
        // S3 non rinomina: si copia sul server e si toglie l'originale
        if let Some((size, _)) = self.head(&fb, &fk)? {
            if size > COPY_MAX {
                return Err(io::Error::from_raw_os_error(EXDEV));
            }
            self.copy_object(&fb, &fk, &tb, &tk)?;
            return self.ok("DELETE", &fb, &fk, &[], &[], Body::Empty).map(|_| ());
        }
        let keys = self.all_keys(&fb, &format!("{fk}/"))?;
        if keys.is_empty() {
            return Err(io::Error::new(io::ErrorKind::NotFound, "non esiste"));
        }
        if keys.iter().any(|(_, s)| *s > COPY_MAX) {
            return Err(io::Error::from_raw_os_error(EXDEV));
        }
        let old_prefix = format!("{fk}/");
        for (key, _) in &keys {
            let rest = key.strip_prefix(&old_prefix).unwrap_or(key);
            self.copy_object(&fb, key, &tb, &format!("{tk}/{rest}"))?;
        }
        let old: Vec<String> = keys.into_iter().map(|(k, _)| k).collect();
        self.delete_keys(&fb, &old)
    }
}

// ------------------------------------------------------------------ scrittura

/// Il corpo della PUT a flusso: legge i pezzi che arrivano da `write`. Se il
/// mittente sparisce senza aver finito (Annulla) dà errore: la PUT si
/// interrompe, e su S3 un oggetto nasce solo intero, quindi non nasce.
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

enum Mode {
    Buffer(Vec<u8>),
    Stream { tx: Option<SyncSender<Vec<u8>>>, worker: Option<JoinHandle<io::Result<()>>>, finished: Arc<AtomicBool> },
    /// caricamento a pezzi: (numero, ETag) dei pezzi già mandati
    Multi { upload: Option<String>, part: usize, buf: Vec<u8>, parts: Vec<(u32, String)> },
}

/// Il file in scrittura; `flush` lo chiude (senza flush non nasce).
struct S3Sink {
    s3: Arc<S3>,
    bucket: String,
    key: String,
    mode: Mode,
    done: bool,
}

impl S3Sink {
    fn upload_part(&mut self, last: bool) -> io::Result<()> {
        let Mode::Multi { upload, part, buf, parts } = &mut self.mode else { return Ok(()) };
        while buf.len() >= *part || (last && !buf.is_empty()) {
            let n = if buf.len() >= *part { *part } else { buf.len() };
            if upload.is_none() {
                let (_, body) = self.s3.ok("POST", &self.bucket, &self.key, &[("uploads", String::new())], &[], Body::Empty)?;
                let id = tag_text(&body, "UploadId").ok_or_else(|| io::Error::other("il servizio non ha dato l'UploadId"))?;
                *upload = Some(id);
            }
            let id = upload.clone().unwrap_or_default();
            let number = parts.len() as u32 + 1;
            let chunk: Vec<u8> = buf.drain(..n).collect();
            let (r, _) = self.s3.ok("PUT", &self.bucket, &self.key, &[("partNumber", number.to_string()), ("uploadId", id)], &[], Body::Bytes(&chunk))?;
            let etag = r.headers().get("etag").and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
            parts.push((number, etag));
        }
        Ok(())
    }
}

/// Il testo di un tag semplice in un XML piccolo.
fn tag_text(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let i = xml.find(&open)? + open.len();
    let j = xml[i..].find(&format!("</{tag}>"))?;
    Some(xml[i..i + j].to_string())
}

impl Write for S3Sink {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        match &mut self.mode {
            Mode::Buffer(b) => {
                b.extend_from_slice(data);
                Ok(data.len())
            }
            Mode::Stream { tx, worker, .. } => {
                if tx.as_ref().map(|t| t.send(data.to_vec()).is_ok()).unwrap_or(false) {
                    return Ok(data.len());
                }
                // il servizio ha smesso di ascoltare: la causa la sa la PUT
                *tx = None;
                match worker.take().map(|w| w.join()) {
                    Some(Ok(Err(e))) => Err(e),
                    _ => Err(io::Error::other("il servizio ha interrotto il caricamento")),
                }
            }
            Mode::Multi { buf, .. } => {
                buf.extend_from_slice(data);
                self.upload_part(false)?;
                Ok(data.len())
            }
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        if self.done {
            return Ok(());
        }
        match &mut self.mode {
            Mode::Buffer(b) => {
                let data = std::mem::take(b);
                self.s3.put_bytes(&self.bucket, &self.key, &data)?;
            }
            Mode::Stream { tx, worker, finished } => {
                finished.store(true, Ordering::SeqCst);
                *tx = None;
                if let Some(w) = worker.take() {
                    w.join().map_err(|_| io::Error::other("caricamento interrotto"))??;
                }
            }
            Mode::Multi { .. } => {}
        }
        if matches!(self.mode, Mode::Multi { .. }) {
            self.upload_part(true)?;
            if let Mode::Multi { upload, parts, .. } = &mut self.mode {
                match upload.take() {
                    // un file vuoto non ha pezzi: si mette e basta
                    None => self.s3.put_bytes(&self.bucket, &self.key, &[])?,
                    Some(id) => {
                        let mut xml = String::from("<CompleteMultipartUpload>");
                        for (n, etag) in parts.iter() {
                            xml.push_str(&format!("<Part><PartNumber>{n}</PartNumber><ETag>{}</ETag></Part>", xml_escape(etag)));
                        }
                        xml.push_str("</CompleteMultipartUpload>");
                        let r = self.s3.ok("POST", &self.bucket, &self.key, &[("uploadId", id.clone())], &[], Body::Bytes(xml.as_bytes()));
                        if let Err(e) = r {
                            let _ = self.s3.send("DELETE", &self.bucket, &self.key, &[("uploadId", id)], &[], Body::Empty);
                            return Err(e);
                        }
                    }
                }
            }
        }
        self.done = true;
        Ok(())
    }
}

impl Drop for S3Sink {
    /// Annullato a metà: i pezzi già mandati di un caricamento a pezzi
    /// occupano spazio (e si pagano) finché non si annulla il caricamento.
    fn drop(&mut self) {
        if self.done {
            return;
        }
        if let Mode::Multi { upload: Some(id), .. } = &self.mode {
            let _ = self.s3.send("DELETE", &self.bucket, &self.key, &[("uploadId", id.clone())], &[], Body::Empty);
        }
    }
}

impl crate::vfs::Sink for S3Sink {
    fn sync(&mut self) -> io::Result<()> {
        Ok(())
    }
}
