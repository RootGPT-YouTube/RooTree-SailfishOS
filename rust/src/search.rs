//! La ricerca: per nome (testo semplice o con * e ?), per espressione
//! regolare sul nome, per contenuto (testo o espressione regolare). Gira su
//! un thread suo; l'interfaccia ritira i risultati a intervalli con `take()`.

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;

use regex::bytes::{Regex, RegexBuilder};

use crate::ffi::{SearchBatch, SearchHit};
use crate::fsops;

pub const MODE_NAME: u8 = 0;
#[allow(dead_code)] // documenta il valore: build() lo tratta nel ramo `_`
pub const MODE_NAME_REGEX: u8 = 1;
pub const MODE_CONTENT: u8 = 2;
#[allow(dead_code)]
pub const MODE_CONTENT_REGEX: u8 = 3;

/// Oltre questo numero di risultati la ricerca si ferma (e lo dice).
const LIMIT: u64 = 5000;
/// I file più grandi non si leggono per cercarci dentro.
const MAX_CONTENT: u64 = 64 << 20;

#[derive(Default)]
struct State {
    pending: Vec<SearchHit>,
    found: u64,
    scanned: u64,
    done: bool,
    limited: bool,
    current: String,
    error: String,
}

struct Search {
    cancel: AtomicBool,
    state: Mutex<State>,
}

fn all() -> &'static Mutex<HashMap<u64, Arc<Search>>> {
    static S: OnceLock<Mutex<HashMap<u64, Arc<Search>>>> = OnceLock::new();
    S.get_or_init(Default::default)
}

static NEXT: AtomicU64 = AtomicU64::new(1);

/// Da «*.jpg» o «foto?» a un'espressione regolare sull'intero nome; senza
/// caratteri jolly, il testo può stare in qualunque punto del nome.
pub fn name_pattern(q: &str) -> String {
    if !q.contains(['*', '?']) {
        return regex::escape(q);
    }
    let mut r = String::from("^");
    for c in q.chars() {
        match c {
            '*' => r.push_str(".*"),
            '?' => r.push('.'),
            c => r.push_str(&regex::escape(&c.to_string())),
        }
    }
    r.push('$');
    r
}

pub fn build(query: &str, mode: u8, case_sensitive: bool) -> Result<Regex, String> {
    if query.is_empty() {
        return Err("scrivere cosa cercare".into());
    }
    let pat = match mode {
        MODE_NAME => name_pattern(query),
        MODE_CONTENT => regex::escape(query),
        _ => query.to_string(),
    };
    RegexBuilder::new(&pat)
        .case_insensitive(!case_sensitive)
        .size_limit(1 << 22)
        .build()
        .map_err(|e| format!("espressione non valida: {e}"))
}

pub fn start(root: &str, query: &str, mode: u8, case_sensitive: bool, hidden: bool) -> Result<u64, String> {
    let re = build(query, mode, case_sensitive)?;
    if !Path::new(root).is_dir() {
        return Err("la ricerca si fa in una cartella (non dentro un archivio)".into());
    }
    let id = NEXT.fetch_add(1, Ordering::Relaxed);
    let s = Arc::new(Search { cancel: AtomicBool::new(false), state: Mutex::new(State::default()) });
    all().lock().unwrap().insert(id, s.clone());
    let root = PathBuf::from(root);
    thread::spawn(move || {
        run(&s, &root, &re, mode >= MODE_CONTENT, hidden);
        s.state.lock().unwrap().done = true;
    });
    Ok(id)
}

fn run(s: &Search, root: &Path, re: &Regex, content: bool, hidden: bool) {
    // dalla radice non si entra nei filesystem virtuali: sono infiniti o
    // bloccano (letture di /proc/kmsg, /sys/kernel/debug...)
    const SKIP: [&str; 4] = ["/proc", "/sys", "/dev", "/run/user"];
    let mut stack = vec![root.to_path_buf()];
    let mut buf = Vec::new();
    while let Some(dir) = stack.pop() {
        if s.cancel.load(Ordering::Relaxed) {
            return;
        }
        s.state.lock().unwrap().current = dir.to_string_lossy().into_owned();
        let Ok(rd) = fs::read_dir(&dir) else { continue };
        let mut subdirs = Vec::new();
        for item in rd.flatten() {
            if s.cancel.load(Ordering::Relaxed) {
                return;
            }
            let name = item.file_name().to_string_lossy().into_owned();
            if !hidden && name.starts_with('.') {
                continue;
            }
            let path = item.path();
            let Ok(ft) = item.file_type() else { continue };
            // i link a cartelle non si seguono: niente giri infiniti
            if ft.is_dir() && !SKIP.iter().any(|p| path == Path::new(p)) {
                subdirs.push(path.clone());
            }
            let (hit, snippet) = if content {
                if !ft.is_file() {
                    (false, String::new())
                } else {
                    match grep(&path, re, &mut buf) {
                        Some(snip) => (true, snip),
                        None => (false, String::new()),
                    }
                }
            } else {
                (re.is_match(name.as_bytes()), String::new())
            };
            let mut st = s.state.lock().unwrap();
            st.scanned += 1;
            if hit {
                st.found += 1;
                st.pending.push(SearchHit { entry: fsops::entry_for(&path, name), snippet });
                if st.found >= LIMIT {
                    st.limited = true;
                    return;
                }
            }
        }
        // in ordine alfabetico anche a ritroso: la pila li prende dal fondo
        subdirs.sort_by(|a, b| fsops::natural_cmp(&b.to_string_lossy(), &a.to_string_lossy()));
        stack.extend(subdirs);
    }
}

/// La riga che contiene la prima corrispondenza, accorciata; `None` se il
/// file non corrisponde, è binario o troppo grande.
fn grep(path: &Path, re: &Regex, buf: &mut Vec<u8>) -> Option<String> {
    let f = File::open(path).ok()?;
    if f.metadata().ok()?.len() > MAX_CONTENT {
        return None;
    }
    buf.clear();
    f.take(MAX_CONTENT).read_to_end(buf).ok()?;
    if buf[..buf.len().min(8192)].contains(&0) {
        return None; // binario
    }
    let m = re.find(buf)?;
    let start = buf[..m.start()].iter().rposition(|&b| b == b'\n').map(|i| i + 1).unwrap_or(0);
    let end = buf[m.end()..].iter().position(|&b| b == b'\n').map(|i| m.end() + i).unwrap_or(buf.len());
    let line = String::from_utf8_lossy(&buf[start..end]);
    let line = line.trim();
    // intorno alla corrispondenza, non solo l'inizio di una riga lunga
    let chars: Vec<char> = line.chars().collect();
    if chars.len() <= 120 {
        return Some(line.to_string());
    }
    let at = String::from_utf8_lossy(&buf[start..m.start()]).trim_start().chars().count();
    let from = at.saturating_sub(40).min(chars.len().saturating_sub(120));
    let s: String = chars[from..(from + 120).min(chars.len())].iter().collect();
    Some(format!("{}{}…", if from > 0 { "…" } else { "" }, s))
}

/// I risultati arrivati dall'ultima volta, e a che punto è.
pub fn take(id: u64) -> SearchBatch {
    let s = all().lock().unwrap().get(&id).cloned();
    let Some(s) = s else {
        return SearchBatch { done: true, ..Default::default() };
    };
    let mut st = s.state.lock().unwrap();
    SearchBatch {
        hits: std::mem::take(&mut st.pending),
        found: st.found,
        scanned: st.scanned,
        done: st.done,
        limited: st.limited,
        current: st.current.clone(),
        error: st.error.clone(),
    }
}

/// Ferma la ricerca e la dimentica.
pub fn stop(id: u64) {
    if let Some(s) = all().lock().unwrap().remove(&id) {
        s.cancel.store(true, Ordering::Relaxed);
    }
}
