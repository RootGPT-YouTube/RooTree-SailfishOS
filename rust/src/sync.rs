//! Le sincronizzazioni (0.9.5): una cartella del telefono con una cartella di
//! una connessione (server o cloud), nella direzione e con le eliminazioni
//! scelte dall'utente per ognuna. Gira solo con RooTree aperta (all'apertura,
//! quando un file cambia, o a mano): nessun servizio in background (decisione
//! dell'utente del 15/11).
//!
//! Come decide: per ogni file confronta lo stato di adesso, dalle due parti,
//! con quello dell'ultima sincronizzazione riuscita (`<stato>/<id>.json`).
//! - un file nuovo da una parte si copia dall'altra, se la direzione lo vuole;
//! - un file cambiato da una parte sola si ricopia nella stessa direzione;
//! - cambiato dalle due parti: vince il più recente, e la copia perdente non
//!   si perde (sul telefono nel cestino, sul remoto rinominata «(conflitto …)»);
//! - un file sparito da una parte che c'era all'ultimo giro è un'eliminazione:
//!   si ripete dall'altra solo se l'utente l'ha chiesto, altrimenti (se la
//!   direzione lo permette) si ricopia.
//! Due file che non si sono mai visti insieme sono uguali se hanno la stessa
//! dimensione (i cloud non tengono la data originale: senza questa regola il
//! primo giro ricopierebbe tutto); dopo, contano dimensione e data (2 s di
//! tolleranza, quanto la FAT delle schede).
//!
//! Salvaguardie (approvate dall'utente il 15/11):
//! - ciò che si elimina dal telefono va nel cestino di RooTree, dal remoto nel
//!   cestino del servizio (o in quello di RooTree sul server): recuperabile;
//! - più di `MANY` eliminazioni in un giro ⇒ non si fa niente e si chiede;
//! - una parte che si presenta vuota tutta insieme (mentre l'ultima volta
//!   aveva dei file) non ne svuota l'altra senza conferma: un NAS montato
//!   male o una scheda tolta non sono un «archiviato».

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::jobs::Handle;
use crate::vfs;

/// sopra questo numero di eliminazioni in un giro si chiede conferma
pub const MANY: usize = 50;
const CHUNK: usize = 1 << 20;
const SYNC_EVERY: usize = 8 << 20;
const MTIME_SLACK: i64 = 2;

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Config {
    pub id: String,
    pub name: String,
    /// la cartella del telefono
    pub local: String,
    /// la connessione; vuota = `remote_path` è un percorso qualunque (prove)
    pub remote: String,
    pub remote_path: String,
    pub subdirs: bool,
    pub hidden: bool,
    /// "up" telefono → remoto, "down" remoto → telefono, "both"
    pub direction: String,
    /// le eliminazioni che si ripetono: "none", "up" (dal telefono al remoto),
    /// "down" (dal remoto al telefono), "both"
    pub deletions: String,
}

impl Config {
    pub fn remote_root(&self) -> PathBuf {
        let p = if self.remote_path.is_empty() { "/" } else { &self.remote_path };
        if self.remote.is_empty() {
            PathBuf::from(p)
        } else {
            let p = if p.starts_with('/') { p.to_string() } else { format!("/{p}") };
            PathBuf::from(format!("{}{}{}", crate::remote::PREFIX, self.remote, p.trim_end_matches('/')))
        }
    }
    fn copy_up(&self) -> bool {
        self.direction == "up" || self.direction == "both"
    }
    fn copy_down(&self) -> bool {
        self.direction == "down" || self.direction == "both"
    }
    /// le eliminazioni fatte sul telefono si ripetono sul remoto
    fn del_up(&self) -> bool {
        self.deletions == "up" || self.deletions == "both"
    }
    fn del_down(&self) -> bool {
        self.deletions == "down" || self.deletions == "both"
    }
}

/// Un file come l'ha lasciato l'ultima sincronizzazione: dimensione e data
/// dalle due parti.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct Seen {
    pub ls: u64,
    pub lm: i64,
    pub rs: u64,
    pub rm: i64,
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct State {
    pub files: BTreeMap<String, Seen>,
    pub dirs: BTreeSet<String>,
}

#[derive(Clone, Copy, Debug)]
struct Item {
    is_dir: bool,
    size: u64,
    mtime: i64,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub sync_id: String,
    pub uploaded: u64,
    pub downloaded: u64,
    pub deleted_local: u64,
    pub deleted_remote: u64,
    pub conflicts: u64,
    pub bytes: u64,
    pub skipped: Vec<String>,
    /// "" fatto; "many" troppe eliminazioni; "empty-remote" / "empty-local":
    /// una parte vuota tutta insieme. Con una conferma non si è fatto niente.
    pub confirm: String,
    /// quante eliminazioni aspettano la conferma
    pub pending: u64,
}

#[derive(Debug, PartialEq)]
enum Act {
    Up(String),
    Down(String),
    DelLocal(String),
    DelRemote(String),
}

/// Il lavoro, con o senza la scheda dell'interfaccia (le prove non ce l'hanno).
pub struct Ctl<'a> {
    pub handle: Option<&'a Handle>,
}

impl Ctl<'_> {
    fn cancelled(&self) -> bool {
        self.handle.is_some_and(|h| h.cancelled())
    }
    fn set(&self, f: impl FnOnce(&mut crate::ffi::JobStatus)) {
        if let Some(h) = self.handle {
            h.set(f);
        }
    }
    fn check(&self) -> Result<(), String> {
        if self.cancelled() { Err("annullata".into()) } else { Ok(()) }
    }
}

fn skip_name(name: &str, hidden: bool) -> bool {
    name.ends_with(".rootree-part")
        || name.starts_with(".Trash")
        || name == ".rootree-trash"
        || (!hidden && name.starts_with('.'))
}

/// Tutti i file (e le cartelle) sotto `root`, coi percorsi relativi.
fn walk(root: &Path, recursive: bool, hidden: bool, ctl: &Ctl) -> Result<HashMap<String, Item>, String> {
    let mut out = HashMap::new();
    let mut todo = vec![(root.to_path_buf(), String::new())];
    while let Some((dir, rel)) = todo.pop() {
        ctl.check()?;
        let list = vfs::read_dir_meta(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        for (p, m) in list {
            let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            if name.is_empty() || m.is_symlink || skip_name(&name, hidden) {
                continue;
            }
            let r = if rel.is_empty() { name.clone() } else { format!("{rel}/{name}") };
            if m.is_dir {
                if recursive {
                    out.insert(r.clone(), Item { is_dir: true, size: 0, mtime: m.mtime });
                    todo.push((p, r));
                }
            } else if m.is_file {
                out.insert(r, Item { is_dir: false, size: m.len, mtime: m.mtime });
            }
        }
    }
    Ok(out)
}

fn same_time(a: i64, b: i64) -> bool {
    (a - b).abs() <= MTIME_SLACK
}

fn load_state(file: &Path) -> State {
    std::fs::read(file).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

fn save_state(file: &Path, st: &State) -> io::Result<()> {
    if let Some(d) = file.parent() {
        std::fs::create_dir_all(d)?;
    }
    let tmp = file.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec(st).map_err(io::Error::other)?)?;
    std::fs::rename(&tmp, file)
}

fn stamp() -> String {
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as libc::time_t).unwrap_or(0);
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    unsafe { libc::localtime_r(&t, &mut tm) };
    format!("{:04}-{:02}-{:02} {:02}.{:02}", tm.tm_year + 1900, tm.tm_mon + 1, tm.tm_mday, tm.tm_hour, tm.tm_min)
}

/// «nome (conflitto 2026-11-16 10.30).ext»
fn conflict_name(rel: &str) -> String {
    let (dir, name) = match rel.rsplit_once('/') {
        Some((d, n)) => (format!("{d}/"), n.to_string()),
        None => (String::new(), rel.to_string()),
    };
    let (stem, ext) = match name.rsplit_once('.') {
        Some((s, e)) if !s.is_empty() => (s.to_string(), format!(".{e}")),
        _ => (name.clone(), String::new()),
    };
    format!("{dir}{stem} (conflitto {}){ext}", stamp())
}

/// Le cartelle che portano a `rel` (escluso il file): «a», «a/b».
fn parents(rel: &str) -> Vec<String> {
    let mut v = Vec::new();
    let mut acc = String::new();
    let parts: Vec<&str> = rel.split('/').collect();
    for p in &parts[..parts.len().saturating_sub(1)] {
        if !acc.is_empty() {
            acc.push('/');
        }
        acc.push_str(p);
        v.push(acc.clone());
    }
    v
}

struct Runner<'a> {
    lroot: PathBuf,
    rroot: PathBuf,
    ctl: &'a Ctl<'a>,
    rep: Report,
    made_local: BTreeSet<String>,
    made_remote: BTreeSet<String>,
}

impl Runner<'_> {
    fn copy(&mut self, from: &Path, to: &Path, size: u64, mtime: i64) -> Result<(), String> {
        let part = vfs::part_of(to);
        let r = (|| -> Result<(), String> {
            let e = |e: io::Error| e.to_string();
            let mut src = vfs::open_read(from).map_err(e)?;
            let mut dst = vfs::create(&part, 0o644, Some(size)).map_err(e)?;
            let mut buf = vec![0u8; CHUNK];
            let mut unsynced = 0usize;
            loop {
                self.ctl.check()?;
                let n = src.read(&mut buf).map_err(e)?;
                if n == 0 {
                    break;
                }
                dst.write_all(&buf[..n]).map_err(e)?;
                unsynced += n;
                if unsynced >= SYNC_EVERY {
                    dst.sync().map_err(e)?;
                    unsynced = 0;
                }
                self.rep.bytes += n as u64;
                self.ctl.set(|s| s.done_bytes += n as u64);
            }
            dst.sync().map_err(e)?;
            dst.flush().map_err(e)?;
            drop(dst);
            let _ = vfs::set_mtime(&part, mtime);
            vfs::rename(&part, to).map_err(e)
        })();
        if r.is_err() {
            let _ = vfs::remove_file(&part);
        }
        r
    }

    fn ensure_dirs(&mut self, root_remote: bool, rel: &str) -> Result<(), String> {
        for d in parents(rel) {
            let (root, made) = if root_remote { (&self.rroot, &mut self.made_remote) } else { (&self.lroot, &mut self.made_local) };
            if made.contains(&d) {
                continue;
            }
            let p = root.join(&d);
            if !vfs::is_dir(&p) {
                vfs::create_dir(&p).map_err(|e| format!("{}: {e}", p.display()))?;
            }
            made.insert(d);
        }
        Ok(())
    }

    fn act(&mut self, a: &Act, local: &HashMap<String, Item>, remote: &HashMap<String, Item>, base: &BTreeMap<String, Seen>) -> Result<(), String> {
        match a {
            Act::Up(rel) => {
                let l = local[rel];
                self.ctl.set(|s| s.current = self.lroot.join(rel).to_string_lossy().into_owned());
                self.ensure_dirs(true, rel)?;
                let to = self.rroot.join(rel);
                // cambiato anche sul remoto dall'ultimo giro: quella copia resta, rinominata
                if let (Some(r), Some(b)) = (remote.get(rel), base.get(rel))
                    && (r.size != b.rs || !same_time(r.mtime, b.rm))
                {
                    let c = self.rroot.join(conflict_name(rel));
                    vfs::rename(&to, &c).map_err(|e| format!("{}: {e}", to.display()))?;
                    self.rep.conflicts += 1;
                }
                self.copy(&self.lroot.join(rel), &to, l.size, l.mtime)?;
                self.rep.uploaded += 1;
            }
            Act::Down(rel) => {
                let r = remote[rel];
                self.ctl.set(|s| s.current = self.rroot.join(rel).to_string_lossy().into_owned());
                self.ensure_dirs(false, rel)?;
                let to = self.lroot.join(rel);
                if let (Some(l), Some(b)) = (local.get(rel), base.get(rel))
                    && (l.size != b.ls || !same_time(l.mtime, b.lm))
                {
                    crate::trash::move_to_trash(&to).map_err(|e| format!("{}: {e}", to.display()))?;
                    self.rep.conflicts += 1;
                }
                self.copy(&self.rroot.join(rel), &to, r.size, r.mtime)?;
                self.rep.downloaded += 1;
            }
            Act::DelLocal(rel) => {
                let p = self.lroot.join(rel);
                self.ctl.set(|s| s.current = p.to_string_lossy().into_owned());
                match crate::trash::move_to_trash(&p) {
                    Ok(()) => self.rep.deleted_local += 1,
                    Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                    Err(e) => return Err(format!("{}: {e}", p.display())),
                }
            }
            Act::DelRemote(rel) => {
                let p = self.rroot.join(rel);
                self.ctl.set(|s| s.current = p.to_string_lossy().into_owned());
                match crate::trash::move_to_trash(&p) {
                    Ok(()) => self.rep.deleted_remote += 1,
                    Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                    Err(e) => return Err(format!("{}: {e}", p.display())),
                }
            }
        }
        Ok(())
    }
}

/// Il piano: cosa fare per ogni file.
fn plan(cfg: &Config, local: &HashMap<String, Item>, remote: &HashMap<String, Item>, base: &State) -> Vec<Act> {
    let mut names: BTreeSet<&String> = BTreeSet::new();
    names.extend(local.iter().filter(|(_, i)| !i.is_dir).map(|(k, _)| k));
    names.extend(remote.iter().filter(|(_, i)| !i.is_dir).map(|(k, _)| k));
    names.extend(base.files.keys());
    let mut acts = Vec::new();
    for rel in names {
        let l = local.get(rel).filter(|i| !i.is_dir);
        let r = remote.get(rel).filter(|i| !i.is_dir);
        let b = base.files.get(rel);
        let changed_l = |l: &Item| b.is_none_or(|b| l.size != b.ls || !same_time(l.mtime, b.lm));
        let changed_r = |r: &Item| b.is_none_or(|b| r.size != b.rs || !same_time(r.mtime, b.rm));
        match (l, r) {
            (Some(l), None) => {
                if b.is_some() && !changed_l(l) && cfg.del_down() {
                    acts.push(Act::DelLocal(rel.clone()));
                } else if cfg.copy_up() {
                    acts.push(Act::Up(rel.clone()));
                }
            }
            (None, Some(r)) => {
                if b.is_some() && !changed_r(r) && cfg.del_up() {
                    acts.push(Act::DelRemote(rel.clone()));
                } else if cfg.copy_down() {
                    acts.push(Act::Down(rel.clone()));
                }
            }
            (Some(l), Some(r)) => {
                let (cl, cr) = match b {
                    // mai visti insieme: uguali se hanno la stessa dimensione
                    None if l.size == r.size => continue,
                    None => (true, true),
                    Some(_) => (changed_l(l), changed_r(r)),
                };
                if !cl && !cr {
                    continue;
                }
                let local_wins = if cl && cr { l.mtime >= r.mtime } else { cl };
                if local_wins && cfg.copy_up() {
                    acts.push(Act::Up(rel.clone()));
                } else if !local_wins && cfg.copy_down() {
                    acts.push(Act::Down(rel.clone()));
                }
            }
            (None, None) => {}
        }
    }
    acts
}

/// Una sincronizzazione intera. `state_file`: lo stato dell'ultimo giro.
pub fn run(cfg: &Config, state_file: &Path, confirmed: bool, ctl: &Ctl) -> Result<Report, String> {
    let lroot = PathBuf::from(&cfg.local);
    let rroot = cfg.remote_root();
    let mut rep = Report { sync_id: cfg.id.clone(), ..Default::default() };
    if !vfs::is_dir(&lroot) {
        return Err(format!("la cartella del telefono non c'è ({}): scheda tolta o cartella spostata?", cfg.local));
    }
    ctl.set(|s| s.state = crate::jobs::ST_SCANNING);
    if !vfs::is_dir(&rroot) {
        // si crea solo se la sincronizzazione ci scrive, e solo la prima volta:
        // sparita dopo un giro riuscito vuol dire spostata o server diverso
        let st = load_state(state_file);
        if !cfg.copy_up() || !st.files.is_empty() {
            return Err(format!("la cartella remota non c'è ({})", if cfg.remote_path.is_empty() { "/" } else { &cfg.remote_path }));
        }
        let mut acc = PathBuf::new();
        for c in rroot.components() {
            acc.push(c);
            if !vfs::is_dir(&acc) {
                vfs::create_dir(&acc).map_err(|e| format!("{}: {e}", acc.display()))?;
            }
        }
    }
    let base = load_state(state_file);
    let local = walk(&lroot, cfg.subdirs, cfg.hidden, ctl)?;
    let remote = walk(&rroot, cfg.subdirs, cfg.hidden, ctl)?;
    let acts = plan(cfg, &local, &remote, &base);

    // le salvaguardie, prima di toccare qualsiasi cosa
    let del_local = acts.iter().filter(|a| matches!(a, Act::DelLocal(_))).count();
    let del_remote = acts.iter().filter(|a| matches!(a, Act::DelRemote(_))).count();
    let files = |m: &HashMap<String, Item>| m.values().filter(|i| !i.is_dir).count();
    if !confirmed {
        let confirm = if del_local > 0 && files(&remote) == 0 && !base.files.is_empty() {
            "empty-remote"
        } else if del_remote > 0 && files(&local) == 0 && !base.files.is_empty() {
            "empty-local"
        } else if del_local + del_remote > MANY {
            "many"
        } else {
            ""
        };
        if !confirm.is_empty() {
            rep.confirm = confirm.into();
            rep.pending = (del_local + del_remote) as u64;
            return Ok(rep);
        }
    }

    let total_bytes: u64 = acts
        .iter()
        .map(|a| match a {
            Act::Up(r) => local[r].size,
            Act::Down(r) => remote[r].size,
            _ => 0,
        })
        .sum();
    ctl.set(|s| {
        s.state = crate::jobs::ST_RUNNING;
        s.total_items = acts.len() as u64;
        s.total_bytes = total_bytes;
    });
    let mut ex = Runner { lroot: lroot.clone(), rroot: rroot.clone(), ctl, rep, made_local: BTreeSet::new(), made_remote: BTreeSet::new() };
    let mut failed: BTreeSet<String> = BTreeSet::new();
    for a in &acts {
        ctl.check()?;
        if let Err(e) = ex.act(a, &local, &remote, &base.files) {
            if ctl.cancelled() {
                return Err(e);
            }
            // un file che non va non ferma gli altri: resta per il prossimo giro
            let rel = match a {
                Act::Up(r) | Act::Down(r) | Act::DelLocal(r) | Act::DelRemote(r) => r.clone(),
            };
            ex.rep.skipped.push(e);
            failed.insert(rel);
        }
        ctl.set(|s| s.done_items += 1);
    }

    // le cartelle: quelle sparite da una parte (con le eliminazioni in quel
    // senso) si tolgono anche dall'altra, se sono rimaste vuote; quelle nuove
    // si creano dall'altra parte anche vuote
    let mut dirs_now: BTreeSet<String> = BTreeSet::new();
    if cfg.subdirs {
        let ldirs: BTreeSet<&String> = local.iter().filter(|(_, i)| i.is_dir).map(|(k, _)| k).collect();
        let rdirs: BTreeSet<&String> = remote.iter().filter(|(_, i)| i.is_dir).map(|(k, _)| k).collect();
        let all: BTreeSet<&String> = ldirs.iter().chain(rdirs.iter()).copied().chain(base.dirs.iter()).collect();
        // dalle più profonde, così una cartella si svuota prima della sua madre
        let mut order: Vec<&String> = all.into_iter().collect();
        order.sort_by_key(|d| std::cmp::Reverse(d.matches('/').count()));
        for d in order {
            let (inl, inr, was) = (ldirs.contains(d), rdirs.contains(d), base.dirs.contains(d));
            let lp = lroot.join(d);
            let rp = rroot.join(d);
            let empty = |p: &Path| vfs::read_dir(p).map(|v| v.is_empty()).unwrap_or(false);
            if inl && !inr && was && cfg.del_down() {
                if empty(&lp) && vfs::remove_dir(&lp).is_ok() {
                    continue;
                }
            } else if inr && !inl && was && cfg.del_up() {
                if empty(&rp) && vfs::remove_dir(&rp).is_ok() {
                    continue;
                }
            } else if inl && !inr && cfg.copy_up() && !ex.made_remote.contains(d.as_str()) {
                let _ = ex.ensure_dirs(true, &format!("{d}/x"));
            } else if inr && !inl && cfg.copy_down() && !ex.made_local.contains(d.as_str()) {
                let _ = ex.ensure_dirs(false, &format!("{d}/x"));
            }
            if vfs::is_dir(&lp) && vfs::is_dir(&rp) {
                dirs_now.insert(d.clone());
            }
        }
    }

    // il nuovo stato: i file che adesso ci sono dalle due parti e coincidono
    let mut st = State { files: BTreeMap::new(), dirs: dirs_now };
    let acted: HashMap<&String, &Act> = acts
        .iter()
        .map(|a| match a {
            Act::Up(r) | Act::Down(r) | Act::DelLocal(r) | Act::DelRemote(r) => (r, a),
        })
        .collect();
    let mut names: BTreeSet<&String> = local.keys().collect();
    names.extend(remote.keys());
    for rel in names {
        if failed.contains(rel) {
            if let Some(b) = base.files.get(rel) {
                st.files.insert(rel.clone(), *b);
            }
            continue;
        }
        let seen = match acted.get(rel) {
            Some(Act::Up(_)) => {
                let l = local[rel];
                vfs::lmeta(&rroot.join(rel)).ok().map(|m| Seen { ls: l.size, lm: l.mtime, rs: m.len, rm: m.mtime })
            }
            Some(Act::Down(_)) => {
                let r = remote[rel];
                vfs::lmeta(&lroot.join(rel)).ok().map(|m| Seen { ls: m.len, lm: m.mtime, rs: r.size, rm: r.mtime })
            }
            Some(_) => None,
            None => match (local.get(rel), remote.get(rel)) {
                (Some(l), Some(r)) if !l.is_dir && !r.is_dir => {
                    let b = base.files.get(rel);
                    let in_sync = match b {
                        None => l.size == r.size,
                        Some(b) => l.size == b.ls && same_time(l.mtime, b.lm) && r.size == b.rs && same_time(r.mtime, b.rm),
                    };
                    if in_sync {
                        Some(Seen { ls: l.size, lm: l.mtime, rs: r.size, rm: r.mtime })
                    } else {
                        // cambiato ma non copiato (la direzione non lo vuole):
                        // resta lo stato vecchio, così il cambio si vede ancora
                        b.copied()
                    }
                }
                _ => None,
            },
        };
        if let Some(s) = seen {
            st.files.insert(rel.clone(), s);
        }
    }
    save_state(state_file, &st).map_err(|e| format!("stato della sincronizzazione non salvato: {e}"))?;
    Ok(ex.rep)
}

/// una sincronizzazione alla volta: i server (e kDrive coi suoi 60 al minuto)
/// non reggono bene più giri insieme
fn one_at_a_time() -> &'static Mutex<()> {
    static M: Mutex<()> = Mutex::new(());
    &M
}

/// Il lavoro per l'interfaccia: lo stato finale in `detail` (JSON del Report).
pub fn start(config_json: &str, state_dir: &str, confirmed: bool) -> u64 {
    let cfg: Result<Config, _> = serde_json::from_str(config_json);
    let state_dir = PathBuf::from(state_dir);
    let label = cfg.as_ref().map(|c| c.local.clone()).unwrap_or_default();
    let dest = cfg.as_ref().map(|c| c.remote_root().to_string_lossy().into_owned()).unwrap_or_default();
    crate::jobs::side_job(crate::jobs::JOB_SYNC, vec![label, dest.clone()], move |h| {
        let cfg = cfg.map_err(|e| format!("sincronizzazione non valida: {e}"))?;
        h.set(|s| {
            s.detail = serde_json::json!({ "syncId": cfg.id }).to_string();
            // la scheda d'avanzamento dice «… con <cartella remota>»
            s.dest = dest;
        });
        let _g = one_at_a_time().lock().unwrap_or_else(|e| e.into_inner());
        let file = state_dir.join(format!("{}.json", cfg.id));
        let ctl = Ctl { handle: Some(h) };
        let r = run(&cfg, &file, confirmed, &ctl);
        match r {
            Ok(rep) => {
                h.set(|s| s.detail = serde_json::to_string(&rep).unwrap_or_default());
                Ok(())
            }
            // annullata: il lavoro si chiude come annullato, non fallito
            Err(_) if h.cancelled() => Ok(()),
            Err(e) => Err(e),
        }
    })
}

/// Toglie lo stato di una sincronizzazione eliminata.
pub fn forget(state_dir: &str, id: &str) {
    if !id.is_empty() && !id.contains('/') {
        let _ = std::fs::remove_file(Path::new(state_dir).join(format!("{id}.json")));
    }
}
