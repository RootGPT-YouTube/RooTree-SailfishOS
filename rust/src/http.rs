//! HTTPS per i cloud (Fase 5, step 4): un agente ureq con l'OpenSSL di
//! sistema (native-tls), errori leggibili, date RFC 3339, codifica dei percorsi.

use std::io;
use std::sync::OnceLock;
use std::time::Duration;

use ureq::Agent;
use ureq::tls::{TlsConfig, TlsProvider};

/// Una risposta HTTP di ureq.
pub type Resp = ureq::http::Response<ureq::Body>;

pub fn agent() -> &'static Agent {
    static A: OnceLock<Agent> = OnceLock::new();
    A.get_or_init(|| {
        Agent::config_builder()
            // gli stati 4xx/5xx si leggono (il corpo dice il perché), non diventano errori muti
            .http_status_as_error(false)
            .timeout_connect(Some(Duration::from_secs(15)))
            .timeout_recv_response(Some(Duration::from_secs(60)))
            .tls_config(TlsConfig::builder().provider(TlsProvider::NativeTls).build())
            .build()
            .into()
    })
}

pub fn net(e: ureq::Error) -> io::Error {
    io::Error::other(format!("NET:{e}"))
}

/// Uno stato HTTP non riuscito come errore: 401 → AUTH (token scaduto o
/// revocato), 404/409 not_found → NotFound, il resto col corpo accorciato.
pub fn status_error(status: u16, body: &str) -> io::Error {
    let short: String = body.chars().take(300).collect();
    match status {
        401 => io::Error::new(io::ErrorKind::PermissionDenied, format!("AUTH:accesso scaduto o revocato ({short})")),
        403 => io::Error::new(io::ErrorKind::PermissionDenied, format!("permesso negato dal servizio: {short}")),
        404 => io::Error::new(io::ErrorKind::NotFound, "non esiste"),
        409 if body.contains("not_found") => io::Error::new(io::ErrorKind::NotFound, "non esiste"),
        409 if body.contains("conflict") => io::Error::new(io::ErrorKind::AlreadyExists, "esiste già"),
        507 => io::Error::other("spazio esaurito sul cloud"),
        _ => io::Error::other(format!("errore {status} dal servizio: {short}")),
    }
}

/// Codifica una componente (o un percorso, lasciando le «/») per l'URL.
pub fn pct(s: &str, keep_slash: bool) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        let ok = b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') || (keep_slash && b == b'/');
        if ok {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Il JSON per l'intestazione Dropbox-API-Arg: i caratteri non ASCII vanno
/// scritti \uXXXX (un'intestazione HTTP non può contenerli).
pub fn json_ascii(v: &serde_json::Value) -> String {
    let s = v.to_string();
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if c.is_ascii() {
            out.push(c);
        } else {
            let mut buf = [0u16; 2];
            for u in c.encode_utf16(&mut buf) {
                out.push_str(&format!("\\u{u:04x}"));
            }
        }
    }
    out
}

/// «2024-03-01T12:34:56.789Z» o con «+01:00» → secondi Unix (0 se illeggibile).
pub fn rfc3339(s: &str) -> i64 {
    let b = s.as_bytes();
    if b.len() < 19 {
        return 0;
    }
    let num = |a: usize, n: usize| s.get(a..a + n).and_then(|x| x.parse::<i64>().ok());
    let (Some(y), Some(mo), Some(d), Some(h), Some(mi), Some(se)) = (num(0, 4), num(5, 2), num(8, 2), num(11, 2), num(14, 2), num(17, 2)) else {
        return 0;
    };
    // giorni dal 1970 (algoritmo di Howard Hinnant)
    let (y2, m2) = if mo <= 2 { (y - 1, mo + 9) } else { (y, mo - 3) };
    let era = y2.div_euclid(400);
    let yoe = y2 - era * 400;
    let doy = (153 * m2 + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;
    let mut t = days * 86400 + h * 3600 + mi * 60 + se;
    // fuso: «Z» o «±hh:mm» dopo l'eventuale frazione
    let rest = &s[19..];
    let rest = rest.trim_start_matches(|c: char| c == '.' || c.is_ascii_digit());
    if let Some(sign) = rest.chars().next().filter(|c| *c == '+' || *c == '-') {
        let oh = rest.get(1..3).and_then(|x| x.parse::<i64>().ok()).unwrap_or(0);
        let om = rest.get(4..6).and_then(|x| x.parse::<i64>().ok()).unwrap_or(0);
        let off = oh * 3600 + om * 60;
        t -= if sign == '+' { off } else { -off };
    }
    t
}
