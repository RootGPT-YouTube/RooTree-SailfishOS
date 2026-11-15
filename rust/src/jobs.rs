//! Le operazioni lunghe: copia, spostamento, eliminazione, calcolo della
//! dimensione. Copia, spostamento ed eliminazione vanno in coda, una alla
//! volta, su un thread di lavoro; il calcolo della dimensione parte subito sul
//! suo thread. L'interfaccia legge lo stato con `statuses()` (a intervalli) e
//! risponde alle domande (conflitto, errore) con `resolve()`.
//!
//! Garanzie:
//! - un file si copia in `.<nome>.rootree-part` accanto alla destinazione e si
//!   rinomina sul nome vero solo a copia finita: un'interruzione non lascia mai
//!   un file a metà con il nome giusto, e non rovina un file sovrascritto;
//! - nello spostamento fra filesystem diversi un elemento della sorgente si
//!   cancella solo dopo che la sua copia è riuscita;
//! - i link simbolici si copiano come link, non si seguono.

use std::collections::VecDeque;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::thread;

use crate::ffi::JobStatus;
use crate::fsops;
use crate::vfs;

pub const JOB_COPY: u8 = 0;
pub const JOB_MOVE: u8 = 1;
pub const JOB_DELETE: u8 = 2;
pub const JOB_SIZE: u8 = 3;
pub const JOB_EXTRACT: u8 = 4;
pub const JOB_COMPRESS: u8 = 5;
/// nel cestino (trash.rs); JOB_DELETE è il definitivo
pub const JOB_TRASH: u8 = 6;
/// l'occupato diviso per tipo di file (storage.rs): parte subito come la dimensione
pub const JOB_TYPES: u8 = 7;
/// il controllo di un file system smontato (fsck, storage.rs)
pub const JOB_CHECK: u8 = 8;

pub const ST_QUEUED: u8 = 0;
pub const ST_SCANNING: u8 = 1;
pub const ST_RUNNING: u8 = 2;
pub const ST_ASK_CONFLICT: u8 = 3;
pub const ST_ASK_ERROR: u8 = 4;
pub const ST_DONE: u8 = 5;
pub const ST_FAILED: u8 = 6;
pub const ST_CANCELLED: u8 = 7;

pub const ANS_OVERWRITE: u8 = 0;
pub const ANS_SKIP: u8 = 1;
pub const ANS_KEEP_BOTH: u8 = 2;
pub const ANS_CANCEL: u8 = 3;
pub const ANS_RETRY: u8 = 4;
/// cartella su cartella: il contenuto va dentro quella che c'è già
pub const ANS_MERGE: u8 = 5;

const CHUNK: usize = 1 << 20;
const SYNC_EVERY: usize = 8 << 20;
const EXDEV: i32 = 18;

struct Job {
    status: Mutex<JobStatus>,
    cancel: AtomicBool,
    answer: Mutex<Option<(u8, bool)>>,
    cv: Condvar,
}

#[derive(Default)]
struct Manager {
    jobs: Vec<Arc<Job>>,
    queue: VecDeque<(Arc<Job>, Vec<PathBuf>, PathBuf)>,
}

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

fn manager() -> &'static (Mutex<Manager>, Condvar) {
    static M: OnceLock<(Mutex<Manager>, Condvar)> = OnceLock::new();
    M.get_or_init(|| {
        thread::spawn(worker);
        (Mutex::new(Manager::default()), Condvar::new())
    })
}

fn worker() {
    let (m, cv) = manager();
    loop {
        let (job, sources, dest) = {
            let mut g = m.lock().unwrap();
            loop {
                if let Some(x) = g.queue.pop_front() {
                    break x;
                }
                g = cv.wait(g).unwrap();
            }
        };
        let kind = job.status.lock().unwrap().kind;
        let mut run = Run::new(job.clone());
        let result = match kind {
            JOB_DELETE => run.delete(&sources),
            JOB_TRASH => run.trash(&sources),
            JOB_EXTRACT => run.extract(&sources, &dest),
            JOB_COMPRESS => run.compress(&sources, &dest),
            _ => run.transfer(&sources, &dest, kind == JOB_MOVE),
        };
        run.finish(result);
    }
}

fn new_job(kind: u8, sources: &[String], dest: &str) -> Arc<Job> {
    let status = JobStatus {
        id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
        kind,
        state: ST_QUEUED,
        total_bytes: 0,
        done_bytes: 0,
        total_items: 0,
        done_items: 0,
        dirs: 0,
        current: String::new(),
        error: String::new(),
        ask_src: String::new(),
        ask_dst: String::new(),
        ask_dst_is_dir: false,
        ask_src_is_dir: false,
        dest: dest.into(),
        first_source: sources.first().cloned().unwrap_or_default(),
        sources: sources.len() as u64,
        detail: String::new(),
    };
    Arc::new(Job { status: Mutex::new(status), cancel: AtomicBool::new(false), answer: Mutex::new(None), cv: Condvar::new() })
}

fn enqueue(kind: u8, sources: Vec<String>, dest: &str) -> u64 {
    let job = new_job(kind, &sources, dest);
    let id = job.status.lock().unwrap().id;
    let (m, cv) = manager();
    let mut g = m.lock().unwrap();
    g.jobs.push(job.clone());
    g.queue.push_back((job, sources.into_iter().map(PathBuf::from).collect(), PathBuf::from(dest)));
    cv.notify_all();
    id
}

pub fn copy(sources: Vec<String>, dest: &str) -> u64 {
    // copiare da dentro un archivio = estrarre quelle voci
    if sources.first().and_then(|s| crate::archive::split(s)).is_some_and(|(_, inner)| !inner.is_empty()) {
        return enqueue(JOB_EXTRACT, sources, dest);
    }
    enqueue(JOB_COPY, sources, dest)
}

/// «Estrai qui»: tutto l'archivio in `dest`, in una cartella col suo nome
/// (o direttamente, se l'archivio contiene già una sola cartella).
pub fn extract(archive: &str, dest: &str) -> u64 {
    enqueue(JOB_EXTRACT, vec![archive.to_string()], dest)
}

/// Crea l'archivio `dest` (il formato dal nome) con `sources`.
pub fn compress(sources: Vec<String>, dest: &str) -> u64 {
    enqueue(JOB_COMPRESS, sources, dest)
}

pub fn move_to(sources: Vec<String>, dest: &str) -> u64 {
    enqueue(JOB_MOVE, sources, dest)
}

pub fn delete(paths: Vec<String>) -> u64 {
    enqueue(JOB_DELETE, paths, "")
}

/// Dimensione e numero di file e cartelle (per «Proprietà»): non aspetta la coda.
pub fn trash(paths: Vec<String>) -> u64 {
    enqueue(JOB_TRASH, paths, "")
}

pub fn size(paths: Vec<String>) -> u64 {
    let job = new_job(JOB_SIZE, &paths, "");
    let id = job.status.lock().unwrap().id;
    manager().0.lock().unwrap().jobs.push(job.clone());
    thread::spawn(move || {
        let mut run = Run::new(job);
        run.set(|s| s.state = ST_RUNNING);
        let mut t = Totals::default();
        for p in &paths {
            if run.cancelled() {
                break;
            }
            scan(Path::new(p), &mut t, &run.job.cancel);
        }
        run.set(|s| {
            s.total_bytes = t.bytes;
            s.total_items = t.files;
            s.dirs = t.dirs;
        });
        run.finish(Ok(()));
    });
    id
}

/// La maniglia di un lavoro che non va in coda (storage.rs): aggiorna lo
/// stato e dice se l'interfaccia l'ha annullato.
pub struct Handle(Arc<Job>);

impl Handle {
    pub fn set(&self, f: impl FnOnce(&mut JobStatus)) {
        f(&mut self.0.status.lock().unwrap());
    }
    pub fn cancelled(&self) -> bool {
        self.0.cancel.load(Ordering::Relaxed)
    }
    pub fn id(&self) -> u64 {
        self.0.status.lock().unwrap().id
    }
}

/// Un lavoro sul suo thread, senza aspettare la coda (come la dimensione):
/// `f` fa il lavoro; un `Err` lo chiude come fallito con quel messaggio.
pub fn side_job(kind: u8, paths: Vec<String>, f: impl FnOnce(&Handle) -> Result<(), String> + Send + 'static) -> u64 {
    let job = new_job(kind, &paths, "");
    let id = job.status.lock().unwrap().id;
    manager().0.lock().unwrap().jobs.push(job.clone());
    thread::spawn(move || {
        let h = Handle(job.clone());
        h.set(|s| s.state = ST_RUNNING);
        let r = f(&h).map_err(Stop::Failed);
        Run::new(job).finish(r);
    });
    id
}

pub fn statuses() -> Vec<JobStatus> {
    let g = manager().0.lock().unwrap();
    g.jobs.iter().map(|j| j.status.lock().unwrap().clone()).collect()
}

fn find(id: u64) -> Option<Arc<Job>> {
    let g = manager().0.lock().unwrap();
    g.jobs.iter().find(|j| j.status.lock().unwrap().id == id).cloned()
}

pub fn cancel(id: u64) {
    if let Some(j) = find(id) {
        j.cancel.store(true, Ordering::Relaxed);
        let _a = j.answer.lock().unwrap();
        j.cv.notify_all();
        // una in coda che non è ancora partita si chiude subito
        let mut s = j.status.lock().unwrap();
        if s.state == ST_QUEUED {
            s.state = ST_CANCELLED;
        }
    }
}

pub fn resolve(id: u64, action: u8, apply_all: bool) {
    if let Some(j) = find(id) {
        let mut a = j.answer.lock().unwrap();
        *a = Some((action, apply_all));
        // subito «in corso»: chi legge lo stato prima che il lavoro riparta non
        // deve rivedere la stessa domanda (l'interfaccia riaprirebbe il dialogo)
        let mut s = j.status.lock().unwrap();
        if s.state == ST_ASK_CONFLICT || s.state == ST_ASK_ERROR {
            s.state = ST_RUNNING;
        }
        drop(s);
        drop(a);
        j.cv.notify_all();
    }
}

/// Toglie dall'elenco un'operazione finita (riuscita, fallita o annullata).
pub fn forget(id: u64) {
    let mut g = manager().0.lock().unwrap();
    g.jobs.retain(|j| {
        let s = j.status.lock().unwrap();
        !(s.id == id && s.state >= ST_DONE)
    });
}

#[derive(Default)]
struct Totals {
    bytes: u64,
    files: u64,
    dirs: u64,
}

fn scan(p: &Path, t: &mut Totals, cancel: &AtomicBool) {
    let Ok(m) = vfs::lmeta(p) else { return };
    scan_known(p, m, t, cancel);
}

fn scan_known(p: &Path, m: vfs::Meta, t: &mut Totals, cancel: &AtomicBool) {
    if m.is_dir {
        t.dirs += 1;
        if let Ok(rd) = vfs::read_dir_meta(p) {
            for (e, em) in rd {
                if cancel.load(Ordering::Relaxed) {
                    return;
                }
                scan_known(&e, em, t, cancel);
            }
        }
    } else {
        t.files += 1;
        if m.is_file {
            t.bytes += m.len;
        }
    }
}

/// Il nome stesso se è libero, altrimenti il primo «nome (N)» libero.
pub fn free_name(target: &Path) -> PathBuf {
    if !vfs::exists(target) { target.to_path_buf() } else { unique_name(target) }
}

/// «nome (1).ext», «nome (2).ext»… il primo che non esiste.
pub fn unique_name(target: &Path) -> PathBuf {
    let parent = target.parent().unwrap_or(Path::new("/"));
    let name = target.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let is_dir = vfs::lmeta(target).map(|m| m.is_dir).unwrap_or(false);
    let (stem, ext) = match name.rsplit_once('.') {
        Some((s, e)) if !s.is_empty() && !is_dir => (s.to_string(), format!(".{e}")),
        _ => (name.clone(), String::new()),
    };
    (1..)
        .map(|n| parent.join(format!("{stem} ({n}){ext}")))
        .find(|p| !vfs::exists(p))
        .unwrap()
}

enum Stop {
    Cancelled,
    Failed(String),
}

type Res<T> = Result<T, Stop>;

/// Lo stato di un'operazione che gira, con le risposte «per tutti» già date.
struct Run {
    job: Arc<Job>,
    conflict_all: Option<u8>,
    /// la risposta «per tutte le cartelle» (separata da quella dei file, come in Dolphin)
    dir_conflict_all: Option<u8>,
    skip_errors: bool,
}

impl Run {
    fn new(job: Arc<Job>) -> Self {
        Run { job, conflict_all: None, dir_conflict_all: None, skip_errors: false }
    }

    fn set(&self, f: impl FnOnce(&mut JobStatus)) {
        f(&mut self.job.status.lock().unwrap());
    }

    fn cancelled(&self) -> bool {
        self.job.cancel.load(Ordering::Relaxed)
    }

    fn check(&self) -> Res<()> {
        if self.cancelled() { Err(Stop::Cancelled) } else { Ok(()) }
    }

    fn finish(&mut self, r: Res<()>) {
        self.set(|s| {
            s.current.clear();
            s.state = match r {
                Ok(()) if self.job.cancel.load(Ordering::Relaxed) => ST_CANCELLED,
                Ok(()) => ST_DONE,
                Err(Stop::Cancelled) => ST_CANCELLED,
                Err(Stop::Failed(e)) => {
                    s.error = e;
                    ST_FAILED
                }
            }
        });
    }

    /// Ferma il lavoro finché l'interfaccia non risponde (o annulla).
    fn ask(&self, state: u8, f: impl FnOnce(&mut JobStatus)) -> Res<(u8, bool)> {
        let mut a = self.job.answer.lock().unwrap();
        *a = None;
        self.set(|s| {
            s.state = state;
            f(s)
        });
        let r = loop {
            if self.cancelled() {
                break Err(Stop::Cancelled);
            }
            if let Some(x) = a.take() {
                break Ok(x);
            }
            a = self.job.cv.wait(a).unwrap();
        };
        drop(a);
        self.set(|s| {
            s.state = ST_RUNNING;
            s.ask_src.clear();
            s.ask_dst.clear();
            s.error.clear();
        });
        match r {
            Ok((ANS_CANCEL, _)) => Err(Stop::Cancelled),
            other => other,
        }
    }

    /// Un errore su un elemento: si salta, si riprova o si annulla tutto.
    /// `Ok(true)` = riprova.
    fn on_error(&mut self, path: &Path, e: &io::Error) -> Res<bool> {
        if self.skip_errors {
            return Ok(false);
        }
        let (action, all) = self.ask(ST_ASK_ERROR, |s| {
            s.ask_src = path.to_string_lossy().into_owned();
            s.error = e.to_string();
        })?;
        if action == ANS_SKIP && all {
            self.skip_errors = true;
        }
        Ok(action == ANS_RETRY)
    }

    /// La destinazione esiste: sovrascrivi, salta o tieni entrambi.
    fn on_conflict(&mut self, src: &Path, dst: &Path) -> Res<u8> {
        let dst_is_dir = vfs::lmeta(dst).map(|m| m.is_dir).unwrap_or(false);
        if let Some(a) = self.conflict_all {
            // «sovrascrivi tutti» non vale per una cartella al posto di un file
            return Ok(if a == ANS_OVERWRITE && dst_is_dir { ANS_KEEP_BOTH } else { a });
        }
        let (action, all) = self.ask(ST_ASK_CONFLICT, |s| {
            s.ask_src = src.to_string_lossy().into_owned();
            s.ask_dst = dst.to_string_lossy().into_owned();
            s.ask_dst_is_dir = dst_is_dir;
            s.ask_src_is_dir = false;
        })?;
        if all {
            self.conflict_all = Some(action);
        }
        Ok(action)
    }

    /// Una cartella dove c'è già una cartella con lo stesso nome: unire,
    /// tenere entrambe, saltare (o annullare). Chiesto come in Dolphin
    /// (richiesta dell'utente dell'08/11: prima si univa senza chiedere).
    fn on_dir_conflict(&mut self, src: &Path, dst: &Path) -> Res<u8> {
        if let Some(a) = self.dir_conflict_all {
            return Ok(a);
        }
        let (action, all) = self.ask(ST_ASK_CONFLICT, |s| {
            s.ask_src = src.to_string_lossy().into_owned();
            s.ask_dst = dst.to_string_lossy().into_owned();
            s.ask_dst_is_dir = true;
            s.ask_src_is_dir = true;
        })?;
        if all {
            self.dir_conflict_all = Some(action);
        }
        Ok(action)
    }

    fn progress(&self, items: u64, bytes: u64) {
        self.set(|s| {
            s.done_items += items;
            s.done_bytes += bytes;
        });
    }

    fn transfer(&mut self, sources: &[PathBuf], dest: &Path, is_move: bool) -> Res<()> {
        self.set(|s| s.state = ST_SCANNING);
        if !vfs::is_dir(dest) {
            return Err(Stop::Failed(format!("la destinazione non è una cartella: {}", dest.display())));
        }
        let mut t = Totals::default();
        let dest_real = fs::canonicalize(dest).unwrap_or(dest.to_path_buf());
        for src in sources {
            self.check()?;
            let real = fs::canonicalize(src).unwrap_or(src.clone());
            let is_dir = vfs::lmeta(src).map(|m| m.is_dir).unwrap_or(false);
            if is_dir && dest_real.starts_with(&real) {
                return Err(Stop::Failed(format!(
                    "impossibile copiare una cartella dentro sé stessa: {}",
                    src.display()
                )));
            }
            scan(src, &mut t, &self.job.cancel);
        }
        self.check()?;
        self.set(|s| {
            s.total_bytes = t.bytes;
            s.total_items = t.files + t.dirs;
            s.state = ST_RUNNING;
        });
        // lo spazio si controlla prima: una copia che si ferma a metà per
        // spazio esaurito lascia il lavoro peggio di come l'ha trovato
        if let Some((_, free)) = fsops::space(dest) {
            let same_fs = is_move && sources.iter().all(|s| same_device(s, dest));
            if !same_fs && t.bytes > free {
                return Err(Stop::Failed(format!(
                    "spazio insufficiente: servono {} byte, liberi {}",
                    t.bytes, free
                )));
            }
        }
        for src in sources {
            self.check()?;
            let Some(name) = src.file_name() else { continue };
            let mut target = dest.join(name);
            if src.parent() == Some(dest) || fs::canonicalize(src).ok() == fs::canonicalize(&target).ok() && vfs::exists(&target) {
                if is_move {
                    // spostare un elemento nella cartella dove già sta: niente da fare
                    let mut n = Totals::default();
                    scan(src, &mut n, &self.job.cancel);
                    self.progress(n.files + n.dirs, n.bytes);
                    continue;
                }
                // copiare nella stessa cartella: una copia accanto, come Dolphin
                target = unique_name(&target);
            }
            if is_move && self.try_rename(src, &target)? {
                continue;
            }
            let copied = self.copy_entry(src, &target, false, None, None)?;
            fsops::adopt_tree(&target);
            if is_move && copied {
                self.remove_source(src)?;
            }
        }
        Ok(())
    }

    /// Avanzamento di un archivio: una voce (byte 0) o un blocco di byte.
    fn archive_progress(&self, name: &str, n: u64) -> io::Result<()> {
        if self.cancelled() {
            return Err(crate::archive::cancelled());
        }
        self.set(|s| {
            if n == 0 {
                s.done_items += 1;
                s.current = name.to_string();
            }
            s.done_bytes += n;
        });
        Ok(())
    }

    fn archive_error(&self, e: io::Error) -> Stop {
        if e.kind() == io::ErrorKind::Interrupted || self.cancelled() { Stop::Cancelled } else { Stop::Failed(e.to_string()) }
    }

    fn extract(&mut self, sources: &[PathBuf], dest: &Path) -> Res<()> {
        use crate::archive;
        self.set(|s| s.state = ST_SCANNING);
        if crate::remote::split(&dest.to_string_lossy()).is_some() || sources.iter().any(|p| crate::remote::split(&p.to_string_lossy()).is_some()) {
            return Err(Stop::Failed("estrarre da o verso un server arriverà più avanti: per ora copia l'archivio sul telefono".into()));
        }
        if !dest.is_dir() {
            return Err(Stop::Failed(format!("la destinazione non è una cartella: {}", dest.display())));
        }
        let first = sources.first().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
        let Some((arch, _)) = archive::split(&first) else {
            return Err(Stop::Failed(format!("non è un archivio: {first}")));
        };
        // le voci scelte (vuoto = tutto l'archivio)
        let mut picks: Vec<String> = Vec::new();
        for s in sources {
            let s = s.to_string_lossy();
            match archive::split(&s) {
                Some((a, inner)) if a == arch => {
                    if let Some(c) = archive::clean(&inner) {
                        picks.push(c);
                    }
                }
                _ => return Err(Stop::Failed("si estrae da un archivio alla volta".into())),
            }
        }
        let list = archive::list(&arch).map_err(|e| self.archive_error(e))?;
        let chosen = |p: &str| picks.is_empty() || picks.iter().any(|k| p == k || p.starts_with(&format!("{k}/")));
        let (mut bytes, mut items) = (0u64, 0u64);
        for e in list.iter().filter(|e| chosen(&e.path)) {
            bytes += e.size;
            items += 1;
        }
        self.check()?;
        self.set(|s| {
            s.total_bytes = bytes;
            s.total_items = items;
            s.state = ST_RUNNING;
        });
        if let Some((_, free)) = fsops::space(dest)
            && bytes > free
        {
            return Err(Stop::Failed(format!("spazio insufficiente: servono {bytes} byte, liberi {free}")));
        }
        let arch_name = arch.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let stem = archive_stem(&arch_name);
        let tmp = free_name(&dest.join(format!(".{stem}.rootree-part")));
        let r = (|| -> Res<()> {
            fs::create_dir(&tmp).map_err(|e| Stop::Failed(e.to_string()))?;
            let base = |k: &str| k.rsplit('/').next().unwrap_or(k).to_string();
            let map = |p: &str| -> Option<PathBuf> {
                if picks.is_empty() {
                    return Some(PathBuf::from(p));
                }
                picks.iter().find_map(|k| {
                    if p == k {
                        Some(PathBuf::from(base(k)))
                    } else {
                        p.strip_prefix(&format!("{k}/")).map(|rest| Path::new(&base(k)).join(rest))
                    }
                })
            };
            archive::extract_into(&arch, &tmp, &map, &mut |n, b| self.archive_progress(n, b))
                .map_err(|e| self.archive_error(e))?;
            self.check()?;
            // dalla cartella temporanea al posto vero
            let tops: Vec<PathBuf> = if picks.is_empty() {
                let kids: Vec<PathBuf> = fs::read_dir(&tmp).map_err(|e| Stop::Failed(e.to_string()))?.flatten().map(|e| e.path()).collect();
                if kids.len() == 1 {
                    kids // una voce sola (cartella o file): niente cartella in più
                } else {
                    let target = free_name(&dest.join(&stem));
                    fs::rename(&tmp, &target).map_err(|e| Stop::Failed(e.to_string()))?;
                    fsops::adopt_tree(&target);
                    return Ok(());
                }
            } else {
                let mut v: Vec<PathBuf> = picks.iter().map(|k| tmp.join(base(k))).collect();
                v.dedup();
                v
            };
            for src in tops {
                if fs::symlink_metadata(&src).is_err() {
                    continue;
                }
                let mut target = dest.join(src.file_name().unwrap());
                if fs::symlink_metadata(&target).is_ok() {
                    let shown = arch.join(src.strip_prefix(&tmp).unwrap_or(&src));
                    match self.on_conflict(&shown, &target)? {
                        ANS_SKIP => continue,
                        ANS_OVERWRITE if !target.is_dir() => {
                            let _ = fs::remove_file(&target);
                        }
                        _ => target = unique_name(&target),
                    }
                }
                fs::rename(&src, &target).map_err(|e| Stop::Failed(e.to_string()))?;
                fsops::adopt_tree(&target);
            }
            Ok(())
        })();
        let _ = fs::remove_dir_all(&tmp);
        r
    }

    fn compress(&mut self, sources: &[PathBuf], dest: &Path) -> Res<()> {
        use crate::archive;
        self.set(|s| s.state = ST_SCANNING);
        if crate::remote::split(&dest.to_string_lossy()).is_some() || sources.iter().any(|p| crate::remote::split(&p.to_string_lossy()).is_some()) {
            return Err(Stop::Failed("comprimere sul server arriverà più avanti".into()));
        }
        let name = dest.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let Some(fmt) = archive::creatable(&name) else {
            return Err(Stop::Failed(format!("formato non supportato per creare: {name}")));
        };
        let mut t = Totals::default();
        for s in sources {
            self.check()?;
            scan(s, &mut t, &self.job.cancel);
        }
        self.check()?;
        self.set(|s| {
            s.total_bytes = t.bytes;
            s.total_items = t.files + t.dirs;
            s.state = ST_RUNNING;
        });
        let dest = if fs::symlink_metadata(dest).is_ok() { unique_name(dest) } else { dest.to_path_buf() };
        let final_name = dest.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let part = dest.with_file_name(format!(".{final_name}.rootree-part"));
        let r = archive::create(&part, fmt, sources, &mut |n, b| self.archive_progress(n, b))
            .map_err(|e| self.archive_error(e))
            .and_then(|_| {
                if let Ok(f) = File::open(&part) {
                    let _ = f.sync_all();
                }
                fs::rename(&part, &dest).map_err(|e| Stop::Failed(e.to_string()))?;
                fsops::adopt_tree(&dest);
                Ok(())
            });
        if r.is_err() {
            let _ = fs::remove_file(&part);
        }
        r
    }

    /// Spostamento veloce con rename() se la destinazione è libera e sullo
    /// stesso filesystem. `Ok(false)` = va copiato e poi cancellato.
    fn try_rename(&mut self, src: &Path, target: &Path) -> Res<bool> {
        if vfs::exists(target) {
            return Ok(false);
        }
        let mut n = Totals::default();
        scan(src, &mut n, &self.job.cancel);
        loop {
            self.set(|s| s.current = src.to_string_lossy().into_owned());
            match vfs::rename(src, target) {
                Ok(()) => {
                    self.progress(n.files + n.dirs, n.bytes);
                    return Ok(true);
                }
                Err(e) if e.raw_os_error() == Some(EXDEV) => return Ok(false),
                Err(e) => {
                    if !self.on_error(src, &e)? {
                        self.progress(n.files + n.dirs, n.bytes);
                        return Ok(true); // saltato: la sorgente resta dov'è
                    }
                }
            }
        }
    }

    /// Copia ricorsiva. `Ok(true)` = copiato per intero (nello spostamento la
    /// sorgente si può cancellare); `Ok(false)` = qualcosa è stato saltato.
    /// `merging`: si è dentro una cartella che l'utente ha scelto di unire:
    /// le sottocartelle che ci sono già si uniscono senza richiedere.
    /// `known_src` e `known_target`: i dati già letti con l'elenco della
    /// cartella, per non chiederli al server voce per voce.
    fn copy_entry(
        &mut self,
        src: &Path,
        target: &Path,
        merging: bool,
        known_src: Option<vfs::Meta>,
        known_target: Option<Option<vfs::Meta>>,
    ) -> Res<bool> {
        self.check()?;
        self.set(|s| s.current = src.to_string_lossy().into_owned());
        let meta = match known_src {
            Some(m) => m,
            None => loop {
                match vfs::lmeta(src) {
                    Ok(m) => break m,
                    Err(e) => {
                        if !self.on_error(src, &e)? {
                            return Ok(false);
                        }
                    }
                }
            },
        };
        let mut target = target.to_path_buf();
        let exists = known_target.unwrap_or_else(|| vfs::lmeta(&target).ok());

        if meta.is_dir {
            match exists {
                // cartella su cartella: si chiede (dentro un'unione si unisce)
                Some(m) if m.is_dir && !merging => match self.on_dir_conflict(src, &target)? {
                    ANS_SKIP => {
                        let mut n = Totals::default();
                        scan(src, &mut n, &self.job.cancel);
                        self.progress(n.files + n.dirs, n.bytes);
                        return Ok(false);
                    }
                    ANS_KEEP_BOTH => target = unique_name(&target),
                    _ => {} // unisci
                },
                Some(m) if m.is_dir => {}
                Some(_) => match self.on_conflict(src, &target)? {
                    ANS_SKIP => {
                        let mut n = Totals::default();
                        scan(src, &mut n, &self.job.cancel);
                        self.progress(n.files + n.dirs, n.bytes);
                        return Ok(false);
                    }
                    ANS_OVERWRITE => self.remove_quiet(&target)?,
                    _ => target = unique_name(&target),
                },
                None => {}
            }
            if exists.is_none() || !vfs::exists(&target) {
                loop {
                    match vfs::create_dir(&target) {
                        Ok(()) => break,
                        Err(e) => {
                            if !self.on_error(&target, &e)? {
                                let mut n = Totals::default();
                                scan(src, &mut n, &self.job.cancel);
                                self.progress(n.files + n.dirs, n.bytes);
                                return Ok(false);
                            }
                        }
                    }
                }
            }
            let mut all = true;
            let children: Vec<(PathBuf, vfs::Meta)> = match vfs::read_dir_meta(src) {
                Ok(rd) => rd,
                Err(e) => {
                    self.on_error(src, &e)?;
                    Vec::new()
                }
            };
            // cosa c'è già nella destinazione: una richiesta sola (appena creata: niente)
            let present: Option<std::collections::HashMap<std::ffi::OsString, vfs::Meta>> = if exists.is_none() {
                Some(Default::default())
            } else {
                vfs::read_dir_meta(&target)
                    .ok()
                    .map(|rd| rd.into_iter().filter_map(|(p, m)| Some((p.file_name()?.to_owned(), m))).collect())
            };
            for (child, child_meta) in children {
                let name = child.file_name().unwrap().to_owned();
                let known = present.as_ref().map(|h| h.get(&name).copied());
                // dentro una cartella unita (o appena creata) le sottocartelle si uniscono
                all &= self.copy_entry(&child, &target.join(name), true, Some(child_meta), known)?;
            }
            // i permessi dopo i figli: una cartella in sola lettura non
            // impedisce di riempirla
            let _ = vfs::set_mode(&target, meta.mode);
            let _ = vfs::set_mtime(&target, meta.mtime);
            self.progress(1, 0);
            return Ok(all);
        }

        if exists.is_some() {
            match self.on_conflict(src, &target)? {
                ANS_SKIP => {
                    self.progress(1, if meta.is_file { meta.len } else { 0 });
                    return Ok(false);
                }
                ANS_OVERWRITE => {}
                _ => target = unique_name(&target),
            }
        }

        if meta.is_symlink {
            loop {
                let r = vfs::read_link(src).and_then(|to| {
                    if vfs::exists(&target) {
                        vfs::remove_file(&target)?;
                    }
                    vfs::symlink(&to, &target)
                });
                match r {
                    Ok(()) => break,
                    Err(e) => {
                        if !self.on_error(src, &e)? {
                            self.progress(1, 0);
                            return Ok(false);
                        }
                    }
                }
            }
            self.progress(1, 0);
            return Ok(true);
        }

        if !meta.is_file {
            // fifo, socket, dispositivi: non si copiano
            self.progress(1, 0);
            return Ok(false);
        }

        loop {
            let before = self.job.status.lock().unwrap().done_bytes;
            match self.copy_file(src, &target, &meta) {
                Ok(()) => {
                    self.progress(1, 0);
                    return Ok(true);
                }
                Err(Stop::Cancelled) => return Err(Stop::Cancelled),
                Err(Stop::Failed(msg)) => {
                    // l'avanzamento torna dov'era prima del file fallito
                    self.set(|s| s.done_bytes = before);
                    let e = io::Error::other(msg);
                    if !self.on_error(src, &e)? {
                        self.progress(1, meta.len);
                        return Ok(false);
                    }
                }
            }
        }
    }

    fn copy_file(&mut self, src: &Path, target: &Path, meta: &vfs::Meta) -> Res<()> {
        let part = vfs::part_of(target);
        let r = (|| -> Res<()> {
            let io = |e: io::Error| Stop::Failed(e.to_string());
            let mut from = vfs::open_read(src).map_err(io)?;
            let mut to = vfs::create(&part, 0o600, Some(meta.len)).map_err(io)?;
            let mut buf = vec![0u8; CHUNK];
            let mut unsynced = 0usize;
            loop {
                self.check()?;
                let n = from.read(&mut buf).map_err(io)?;
                if n == 0 {
                    break;
                }
                to.write_all(&buf[..n]).map_err(io)?;
                // ogni pochi MB si scrive davvero sul supporto: senza, i dati
                // finiscono tutti nella cache e la scrittura vera avviene nel
                // sync finale, dove la barra segna già il 100% e Annulla non
                // arriva (visto sul POCO, 400 MB verso la SD: ~20 s fermi al 100%)
                unsynced += n;
                if unsynced >= SYNC_EVERY {
                    to.sync().map_err(io)?;
                    unsynced = 0;
                }
                self.progress(0, n as u64);
            }
            // sync: su scheda SD, USB o server «copiato» deve voler dire «arrivato»
            to.sync().map_err(io)?;
            to.flush().map_err(io)?;
            drop(to);
            // FAT ed exFAT non hanno i permessi Unix e il kernel può rifiutare il
            // chmod: il contenuto conta, i permessi no, quindi non è un errore
            let _ = vfs::set_mode(&part, meta.mode);
            let _ = vfs::set_mtime(&part, meta.mtime);
            // su WebDAV e sui cloud la rinomina sopra una cartella viene rifiutata comunque
            if !vfs::is_cloud(target) && vfs::lmeta(target).map(|m| m.is_dir).unwrap_or(false) {
                return Err(Stop::Failed(format!("{} è una cartella", target.display())));
            }
            vfs::rename(&part, target).map_err(io)
        })();
        if r.is_err() {
            let _ = vfs::remove_file(&part);
        }
        r
    }

    /// Cancella la sorgente di uno spostamento, già copiata per intero.
    fn remove_source(&mut self, src: &Path) -> Res<()> {
        loop {
            match vfs::remove_all(src) {
                Ok(()) => return Ok(()),
                Err(e) => {
                    if !self.on_error(src, &e)? {
                        return Ok(());
                    }
                }
            }
        }
    }

    /// Toglie la destinazione che l'utente ha scelto di sovrascrivere.
    fn remove_quiet(&mut self, p: &Path) -> Res<()> {
        vfs::remove_all(p).map_err(|e| Stop::Failed(format!("{}: {e}", p.display())))
    }

    fn delete(&mut self, paths: &[PathBuf]) -> Res<()> {
        self.set(|s| s.state = ST_SCANNING);
        let mut t = Totals::default();
        for p in paths {
            // su WebDAV e sui cloud una cartella se ne va con una richiesta: niente conteggio
            if vfs::is_cloud(p) {
                t.files += 1;
            } else {
                scan(p, &mut t, &self.job.cancel);
            }
        }
        self.check()?;
        self.set(|s| {
            s.total_items = t.files + t.dirs;
            s.total_bytes = t.bytes;
            s.state = ST_RUNNING;
        });
        for p in paths {
            self.delete_entry(p)?;
        }
        Ok(())
    }

    /// Nel cestino: un rinomina per elemento, niente conteggio dei file.
    fn trash(&mut self, paths: &[PathBuf]) -> Res<()> {
        self.set(|s| {
            s.total_items = paths.len() as u64;
            s.state = ST_RUNNING;
        });
        for p in paths {
            self.check()?;
            self.set(|s| s.current = p.to_string_lossy().into_owned());
            loop {
                match crate::trash::move_to_trash(p) {
                    Ok(()) => break,
                    Err(e) if e.kind() == io::ErrorKind::NotFound => break,
                    Err(e) => {
                        let msg = e.to_string();
                        let e = match msg.strip_prefix(crate::trash::NO_TRASH) {
                            Some(why) => io::Error::other(format!("non si può mettere nel cestino: {why}")),
                            None => e,
                        };
                        if !self.on_error(p, &e)? {
                            break;
                        }
                    }
                }
            }
            self.progress(1, 0);
        }
        // le voci vecchie del cestino toccato
        if let Some(t) = paths.first().and_then(|p| crate::trash::trash_for(p).ok())
            && vfs::is_dir(&t)
        {
            crate::trash::purge(&t);
        }
        Ok(())
    }

    fn delete_entry(&mut self, p: &Path) -> Res<()> {
        self.check()?;
        self.set(|s| s.current = p.to_string_lossy().into_owned());
        if vfs::is_cloud(p) {
            loop {
                // definitivo: anche Nextcloud e Mega, che da soli metterebbero nel cestino
                match vfs::remove_forever(p) {
                    Ok(()) => break,
                    Err(e) if e.kind() == io::ErrorKind::NotFound => break,
                    Err(e) => {
                        if !self.on_error(p, &e)? {
                            break;
                        }
                    }
                }
            }
            self.progress(1, 0);
            return Ok(());
        }
        let Ok(meta) = vfs::lmeta(p) else { return Ok(()) };
        if meta.is_dir {
            if let Ok(children) = vfs::read_dir(p) {
                for c in children {
                    self.delete_entry(&c)?;
                }
            }
        }
        loop {
            let r = if meta.is_dir { vfs::remove_dir(p) } else { vfs::remove_file(p) };
            match r {
                Ok(()) => break,
                Err(e) => {
                    if !self.on_error(p, &e)? {
                        break;
                    }
                }
            }
        }
        self.progress(1, if meta.is_file { meta.len } else { 0 });
        Ok(())
    }
}

fn same_device(a: &Path, b: &Path) -> bool {
    vfs::same_device(a, b)
}

/// Aspetta la fine di un'operazione (per i test).
#[cfg(test)]
pub fn wait(id: u64) -> JobStatus {
    loop {
        let s = statuses().into_iter().find(|s| s.id == id).unwrap();
        if s.state >= ST_DONE || s.state == ST_ASK_CONFLICT || s.state == ST_ASK_ERROR {
            return s;
        }
        thread::sleep(std::time::Duration::from_millis(5));
    }
}

/// Il nome di un archivio senza le estensioni dell'archivio: «foto.tar.gz» → «foto».
pub fn archive_stem(name: &str) -> String {
    let lower = name.to_ascii_lowercase();
    for ext in [".tar.gz", ".tar.bz2", ".tar.xz", ".tar.zst", ".tgz", ".tbz2", ".tbz", ".txz", ".tzst", ".tar", ".zip", ".cbz",
        ".7z", ".rar", ".cbr", ".iso", ".cpio", ".cab", ".lzh", ".lha", ".xar", ".deb", ".rpm", ".ar", ".gz", ".bz2", ".xz", ".zst"]
    {
        if lower.len() > ext.len() && lower.ends_with(ext) {
            return name[..name.len() - ext.len()].to_string();
        }
    }
    format!("{name}.estratto")
}

/// Annulla tutto e aspetta che i lavori si chiudano (ripulendo i file a
/// metà), al più `limit`: l'helper root lo fa prima di uscire.
pub fn cancel_all_and_wait(limit: std::time::Duration) {
    let ids: Vec<u64> = statuses().iter().filter(|s| s.state < ST_DONE).map(|s| s.id).collect();
    for id in ids {
        cancel(id);
    }
    let end = std::time::Instant::now() + limit;
    while std::time::Instant::now() < end && statuses().iter().any(|s| s.state < ST_DONE && s.state != ST_QUEUED) {
        thread::sleep(std::time::Duration::from_millis(50));
    }
}
