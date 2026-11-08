//! L'accesso ai cloud dal browser (OAuth 2.0 «authorization code» con PKCE,
//! ritorno su 127.0.0.1): la password non passa mai da RooTree. Il token di
//! rinnovo (refresh token) è il «segreto» della connessione: va nella
//! cassaforte come le password.
//!
//! Regole dei servizi verificate l'08/11/2026 sulle pagine ufficiali:
//! - Google: client «Desktop app», ritorno su loopback con porta qualsiasi,
//!   PKCE sempre; il client_secret delle app desktop non è segreto ed è
//!   facoltativo nello scambio (developers.google.com/identity/protocols/oauth2/native-app);
//! - Microsoft: `http://localhost` ammesso per le app desktop, porta ignorata
//!   nel confronto (learn.microsoft.com/entra/identity-platform/reply-url);
//! - Dropbox: PKCE senza segreto, `token_access_type=offline` per il rinnovo,
//!   indirizzo di ritorno registrato ESATTO (quindi porta fissa)
//!   (docs.dropboxapi.com/dropbox-api/docs/oauth).

use std::collections::HashMap;
use std::io::{self, BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use rand_core::{OsRng, RngCore};
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::http;

/// Un servizio: dove si accede, dove si cambia il codice coi token, cosa si chiede.
#[derive(Clone, Debug)]
pub struct Provider {
    pub auth_url: &'static str,
    pub token_url: &'static str,
    pub scope: &'static str,
    /// parametri in più nell'URL d'accesso
    pub extra: &'static [(&'static str, &'static str)],
    /// host del ritorno («127.0.0.1» o «localhost», come vuole il servizio)
    pub host: &'static str,
    /// porta fissa (Dropbox vuole l'indirizzo esatto), 0 = qualsiasi
    pub port: u16,
}

pub fn provider(kind: &str) -> Option<Provider> {
    Some(match kind {
        "gdrive" => Provider {
            auth_url: "https://accounts.google.com/o/oauth2/v2/auth",
            token_url: "https://oauth2.googleapis.com/token",
            scope: "https://www.googleapis.com/auth/drive",
            extra: &[("access_type", "offline"), ("prompt", "consent")],
            host: "127.0.0.1",
            port: 0,
        },
        "onedrive" => Provider {
            auth_url: "https://login.microsoftonline.com/common/oauth2/v2.0/authorize",
            token_url: "https://login.microsoftonline.com/common/oauth2/v2.0/token",
            scope: "Files.ReadWrite offline_access User.Read",
            extra: &[],
            host: "localhost",
            port: 0,
        },
        "dropbox" => Provider {
            auth_url: "https://www.dropbox.com/oauth2/authorize",
            token_url: "https://api.dropboxapi.com/oauth2/token",
            scope: "",
            extra: &[("token_access_type", "offline")],
            host: "127.0.0.1",
            // la stessa porta va registrata nella console di Dropbox
            port: 53682,
        },
        _ => return None,
    })
}

#[derive(Deserialize, Debug, Default, Clone)]
pub struct Tokens {
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: String,
    #[serde(default)]
    pub expires_in: u64,
}

struct Pending {
    verifier: String,
    redirect: String,
    state: String,
    listener: TcpListener,
    /// anche su ::1, alla stessa porta: un browser che risolve «localhost» in
    /// IPv6 prima di IPv4 troverebbe la porta chiusa
    listener6: Option<TcpListener>,
}

fn pending() -> &'static Mutex<HashMap<String, Pending>> {
    static P: OnceLock<Mutex<HashMap<String, Pending>>> = OnceLock::new();
    P.get_or_init(Default::default)
}

fn cancelled() -> &'static Mutex<std::collections::HashSet<String>> {
    static C: OnceLock<Mutex<std::collections::HashSet<String>>> = OnceLock::new();
    C.get_or_init(Default::default)
}

/// Lascia perdere un accesso in attesa (l'utente è tornato senza finire).
pub fn cancel(id: &str) {
    cancelled().lock().unwrap().insert(id.to_string());
    pending().lock().unwrap().remove(id);
}

fn random(n: usize) -> String {
    let mut b = vec![0u8; n];
    OsRng.fill_bytes(&mut b);
    URL_SAFE_NO_PAD.encode(b)
}

/// Prepara l'accesso: apre l'ascolto su 127.0.0.1 e rende l'URL da aprire nel browser.
pub fn begin(id: &str, kind: &str, client_id: &str) -> Result<String, String> {
    let p = provider(kind).ok_or("servizio senza accesso dal browser")?;
    if client_id.trim().is_empty() {
        return Err("CLIENTID:manca l'ID dell'app (Client ID / App key) di questo servizio".into());
    }
    let listener = TcpListener::bind(("127.0.0.1", p.port)).map_err(|e| format!("ascolto su 127.0.0.1:{}: {e}", p.port))?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    let listener6 = if p.host == "localhost" { TcpListener::bind(("::1", port)).ok() } else { None };
    // Microsoft: registrato «http://localhost» (senza percorso), porta ignorata;
    // lo stesso indirizzo, carattere per carattere, torna nello scambio del codice
    let redirect = if p.host == "localhost" { format!("http://localhost:{port}") } else { format!("http://{}:{port}/", p.host) };
    let verifier = random(48);
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let state = random(16);
    let mut q = vec![
        ("client_id", client_id.trim().to_string()),
        ("response_type", "code".into()),
        ("redirect_uri", redirect.clone()),
        ("code_challenge", challenge),
        ("code_challenge_method", "S256".into()),
        ("state", state.clone()),
    ];
    if !p.scope.is_empty() {
        q.push(("scope", p.scope.into()));
    }
    for (k, v) in p.extra {
        q.push((k, (*v).into()));
    }
    let url = format!(
        "{}?{}",
        p.auth_url,
        q.iter().map(|(k, v)| format!("{k}={}", http::pct(v, false))).collect::<Vec<_>>().join("&")
    );
    pending().lock().unwrap().insert(id.into(), Pending { verifier, redirect, state, listener, listener6 });
    Ok(url)
}

/// Aspetta il ritorno dal browser (al più `timeout`), risponde con una pagina
/// «puoi tornare a RooTree», e scambia il codice coi token.
pub fn complete(id: &str, kind: &str, client_id: &str, client_secret: &str, timeout: Duration) -> Result<Tokens, String> {
    cancelled().lock().unwrap().remove(id);
    let pend = pending().lock().unwrap().remove(id).ok_or("nessun accesso in corso")?;
    pend.listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    if let Some(l6) = &pend.listener6 {
        let _ = l6.set_nonblocking(true);
    }
    let end = Instant::now() + timeout;
    let (code, mut stream) = loop {
        if Instant::now() > end {
            return Err("CANCELLED:tempo scaduto: l'accesso dal browser non è arrivato".into());
        }
        if cancelled().lock().unwrap().remove(id) {
            return Err("CANCELLED:accesso annullato".into());
        }
        let accepted = match pend.listener.accept() {
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => match &pend.listener6 {
                Some(l6) => l6.accept(),
                None => Err(e),
            },
            other => other,
        };
        match accepted {
            Ok((stream, _)) => {
                let _ = stream.set_nonblocking(false);
                let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
                let mut line = String::new();
                if BufReader::new(&stream).read_line(&mut line).is_err() {
                    continue;
                }
                // «GET /?code=…&state=… HTTP/1.1»
                let target = line.split_whitespace().nth(1).unwrap_or("");
                let query = target.split_once('?').map(|x| x.1).unwrap_or("");
                let params: HashMap<String, String> = query
                    .split('&')
                    .filter_map(|kv| kv.split_once('='))
                    .map(|(k, v)| (k.to_string(), unpct(v)))
                    .collect();
                if params.get("state").map(String::as_str) != Some(pend.state.as_str()) {
                    // una richiesta qualunque (il browser chiede anche la favicon): avanti
                    let _ = reply(&stream, 404, "");
                    continue;
                }
                if let Some(err) = params.get("error") {
                    let _ = reply(&stream, 200, &page(false));
                    return Err(format!("CANCELLED:accesso rifiutato ({err})"));
                }
                match params.get("code") {
                    Some(c) => break (c.clone(), stream),
                    None => {
                        let _ = reply(&stream, 400, "");
                        continue;
                    }
                }
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(150)),
            Err(e) => return Err(e.to_string()),
        }
    };
    let p = provider(kind).ok_or("servizio sconosciuto")?;
    let mut form = vec![
        ("grant_type", "authorization_code".to_string()),
        ("code", code),
        ("redirect_uri", pend.redirect.clone()),
        ("client_id", client_id.trim().to_string()),
        ("code_verifier", pend.verifier.clone()),
    ];
    if !client_secret.trim().is_empty() {
        form.push(("client_secret", client_secret.trim().to_string()));
    }
    let r = post_form(p.token_url, &form);
    let _ = reply(&stream, 200, &page(r.is_ok()));
    let _ = stream.flush();
    let t = r?;
    if t.refresh_token.is_empty() {
        return Err("il servizio non ha dato il token di rinnovo (accesso «offline»)".into());
    }
    Ok(t)
}

/// Un token d'accesso nuovo dal token di rinnovo.
pub fn refresh(kind: &str, client_id: &str, client_secret: &str, refresh_token: &str) -> Result<Tokens, String> {
    let p = provider(kind).ok_or("servizio sconosciuto")?;
    let mut form = vec![
        ("grant_type", "refresh_token".to_string()),
        ("refresh_token", refresh_token.to_string()),
        ("client_id", client_id.trim().to_string()),
    ];
    if !client_secret.trim().is_empty() {
        form.push(("client_secret", client_secret.trim().to_string()));
    }
    let mut t = post_form(p.token_url, &form)?;
    // alcuni servizi non ridanno il token di rinnovo: resta quello di prima
    if t.refresh_token.is_empty() {
        t.refresh_token = refresh_token.to_string();
    }
    Ok(t)
}

fn post_form(url: &str, form: &[(&str, String)]) -> Result<Tokens, String> {
    let mut resp = http::agent()
        .post(url)
        .send_form(form.iter().map(|(k, v)| (*k, v.as_str())))
        .map_err(|e| format!("NET:{e}"))?;
    let status = resp.status().as_u16();
    let body = resp.body_mut().read_to_string().map_err(|e| e.to_string())?;
    if status != 200 {
        let short: String = body.chars().take(300).collect();
        return Err(format!("AUTH:il servizio ha rifiutato ({status}): {short}"));
    }
    serde_json::from_str(&body).map_err(|e| format!("risposta del servizio illeggibile: {e}"))
}

fn unpct(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < b.len() => {
                if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                    out.push(v);
                    i += 2;
                } else {
                    out.push(b'%');
                }
            }
            c => out.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn reply(mut stream: &std::net::TcpStream, status: u16, html: &str) -> io::Result<()> {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        _ => "Not Found",
    };
    write!(
        stream,
        "HTTP/1.1 {status} {reason}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{html}",
        html.len()
    )
}

fn page(ok: bool) -> String {
    let (title, text) = if ok {
        ("Accesso riuscito", "RooTree è collegato. Puoi chiudere questa pagina e tornare a RooTree.")
    } else {
        ("Accesso non riuscito", "Torna a RooTree e riprova.")
    };
    format!(
        "<!doctype html><meta charset=utf-8><meta name=viewport content='width=device-width'><title>{title}</title>\
         <body style='font-family:sans-serif;background:#14181d;color:#eef6ff;text-align:center;padding:3em 1em'>\
         <h2 style='color:#4aa3ff'>{title}</h2><p>{text}</p></body>"
    )
}
