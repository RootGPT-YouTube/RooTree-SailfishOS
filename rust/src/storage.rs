//! La pagina «i» di una memoria (proposta di POCO Agent del 15/11/2026, 0.9.3):
//! - `types`: l'occupato diviso per tipo di file, un lavoro come la dimensione
//!   (spazio vero su disco, hardlink una volta sola, senza entrare in altri
//!   montaggi), con i file più grandi di ogni tipo;
//! - `info`: i dati del file system che il QML non vede (statvfs, catena dei
//!   dispositivi, TRIM, superblocco ext4);
//! - `check`: il controllo (fsck) di una scheda o chiavetta, solo smontata:
//!   l'app la smonta e la rimonta con udisks, il controllo lo fa l'helper root.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashSet};
use std::ffi::CString;
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{Map, Value, json};

use crate::ffi::JobStatus;
use crate::jobs::{self, Handle};

/// Le categorie, nell'ordine in cui le mostra la pagina.
pub const CATEGORIES: [&str; 10] = ["images", "video", "audio", "documents", "archives", "packages", "apps", "cache", "system", "other"];
const TOP: usize = 10;
const SAMPLE: usize = 20;

fn cat_index(c: &str) -> usize {
    CATEGORIES.iter().position(|x| *x == c).unwrap_or(CATEGORIES.len() - 1)
}

/// La categoria di un file dal nome (l'estensione).
pub fn ext_category(name: &str) -> &'static str {
    let ext = match name.rsplit_once('.') {
        Some((stem, e)) if !stem.is_empty() => e.to_ascii_lowercase(),
        _ => return "other",
    };
    match ext.as_str() {
        "jpg" | "jpeg" | "png" | "gif" | "webp" | "heic" | "heif" | "bmp" | "tif" | "tiff" | "svg" | "dng" | "avif" | "ico" | "jxl" => "images",
        "mp4" | "mkv" | "webm" | "avi" | "mov" | "m4v" | "3gp" | "mpg" | "mpeg" | "wmv" | "flv" | "ogv" | "m2ts" => "video",
        "mp3" | "flac" | "ogg" | "oga" | "opus" | "m4a" | "aac" | "wav" | "wma" | "amr" | "mka" | "ape" | "m4b" => "audio",
        "pdf" | "odt" | "ods" | "odp" | "odg" | "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx" | "txt" | "md" | "epub" | "rtf" | "csv"
        | "djvu" | "mobi" | "azw3" | "fb2" | "tex" => "documents",
        "zip" | "7z" | "rar" | "tar" | "gz" | "tgz" | "xz" | "txz" | "zst" | "bz2" | "tbz2" | "lz" | "lzma" | "cab" | "iso" => "archives",
        "apk" | "rpm" | "deb" | "xapk" | "apks" | "aab" => "packages",
        _ => "other",
    }
}

/// La categoria che una cartella impone a tutto quello che contiene ("" =
/// nessuna: decide l'estensione di ogni file). La cache vince su tutto, poi i
/// dati delle app, poi il sistema.
pub fn dir_category(parent: &'static str, parent_name: &str, name: &str, abs: &Path) -> &'static str {
    if parent == "cache" || matches!(name, ".cache" | ".thumbnails" | "code_cache") || name.eq_ignore_ascii_case("cache") {
        return "cache";
    }
    // la memoria vista da Android (dentro i dati di Waydroid) è dell'utente:
    // foto, video e documenti tornano a contare per tipo
    if name == "media" && abs.ends_with("waydroid/data/media") {
        return "";
    }
    if parent_name == "Android" && matches!(name, "data" | "obb" | "media") {
        return "apps";
    }
    if !parent.is_empty() {
        return parent;
    }
    if name == ".config" || (parent_name == ".local" && name == "share") {
        return "apps";
    }
    if abs.parent() == Some(Path::new("/"))
        && matches!(name, "usr" | "lib" | "lib64" | "bin" | "sbin" | "etc" | "opt" | "boot" | "vendor" | "system" | "odm" | "apex" | "var")
    {
        return "system";
    }
    ""
}

#[derive(Default)]
struct Acc {
    bytes: [u64; 10],
    count: [u64; 10],
    top: Vec<BinaryHeap<Reverse<(u64, String)>>>,
    files: u64,
    dirs: u64,
    seen: HashSet<(u64, u64)>,
    unreadable: u64,
    unreadable_sample: Vec<String>,
    mounts_skipped: Vec<String>,
}

impl Acc {
    fn new() -> Self {
        Acc { top: (0..CATEGORIES.len()).map(|_| BinaryHeap::new()).collect(), ..Default::default() }
    }

    fn add(&mut self, cat: &str, size: u64, path: Option<&Path>) {
        let i = cat_index(cat);
        self.bytes[i] += size;
        if let Some(p) = path {
            self.count[i] += 1;
            self.files += 1;
            let h = &mut self.top[i];
            if h.len() < TOP || h.peek().is_some_and(|Reverse((s, _))| size > *s) {
                h.push(Reverse((size, p.to_string_lossy().into_owned())));
                if h.len() > TOP {
                    h.pop();
                }
            }
        }
    }

    fn total(&self) -> u64 {
        self.bytes.iter().sum()
    }

    fn detail(&self, done: bool, started: Instant) -> String {
        let mut by = Map::new();
        for (i, c) in CATEGORIES.iter().enumerate() {
            by.insert((*c).into(), json!({ "bytes": self.bytes[i], "count": self.count[i] }));
        }
        let mut v = json!({
            "byType": by,
            "files": self.files,
            "dirs": self.dirs,
            "bytes": self.total(),
            "unreadable": self.unreadable,
            "unreadableSample": self.unreadable_sample,
            "mountsSkipped": self.mounts_skipped,
            "ms": started.elapsed().as_millis() as u64,
            "done": done,
        });
        if done {
            let mut largest = Map::new();
            for (i, c) in CATEGORIES.iter().enumerate() {
                let mut l: Vec<_> = self.top[i].iter().map(|Reverse((s, p))| (*s, p.clone())).collect();
                l.sort_by(|a, b| b.0.cmp(&a.0));
                largest.insert((*c).into(), l.into_iter().map(|(s, p)| json!({ "path": p, "size": s })).collect());
            }
            v["largest"] = Value::Object(largest);
        }
        v.to_string()
    }
}

/// I punti di montaggio (per non contare due volte android_storage, che è un
/// bind dentro la home, né /run/media o altri file system sotto «/»).
fn mount_points() -> HashSet<PathBuf> {
    mounts().into_iter().map(|m| PathBuf::from(m.mount)).collect()
}

/// Divide per tipo di file quello che c'è sotto `paths` (un lavoro: lo stato
/// si legge da `jobs::statuses()`, i dati in `detail`).
pub fn types(paths: Vec<String>) -> u64 {
    jobs::side_job(jobs::JOB_TYPES, paths.clone(), move |h| {
        types_run(&paths, h);
        Ok(())
    })
}

fn publish(h: &Handle, acc: &Acc, done: bool, started: Instant, current: &Path) {
    let d = acc.detail(done, started);
    h.set(|s: &mut JobStatus| {
        s.total_bytes = acc.total();
        s.total_items = acc.files;
        s.dirs = acc.dirs;
        s.current = current.to_string_lossy().into_owned();
        s.detail = d;
    });
}

fn types_run(paths: &[String], h: &Handle) {
    let started = Instant::now();
    let mut last = Instant::now();
    let mut acc = Acc::new();
    let mounts = mount_points();
    for root in paths {
        let rp = Path::new(root);
        let Ok(rm) = fs::symlink_metadata(rp) else {
            acc.unreadable += 1;
            continue;
        };
        let dev = rm.dev();
        let rname = rp.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let pname = rp.parent().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let mut stack = vec![(rp.to_path_buf(), dir_category("", &pname, &rname, rp))];
        while let Some((dir, cat)) = stack.pop() {
            if h.cancelled() {
                return;
            }
            let rd = match fs::read_dir(&dir) {
                Ok(r) => r,
                Err(_) => {
                    acc.unreadable += 1;
                    if acc.unreadable_sample.len() < SAMPLE {
                        acc.unreadable_sample.push(dir.to_string_lossy().into_owned());
                    }
                    continue;
                }
            };
            acc.dirs += 1;
            let dname = dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            for e in rd.flatten() {
                let p = e.path();
                let Ok(m) = fs::symlink_metadata(&p) else { continue };
                let ft = m.file_type();
                let on_disk = m.blocks() * 512;
                if ft.is_dir() {
                    if m.dev() != dev || mounts.contains(&p) {
                        if acc.mounts_skipped.len() < SAMPLE {
                            acc.mounts_skipped.push(p.to_string_lossy().into_owned());
                        }
                        continue;
                    }
                    let name = e.file_name().to_string_lossy().into_owned();
                    let c = dir_category(cat, &dname, &name, &p);
                    // lo spazio della cartella stessa (l'elenco dei nomi)
                    acc.add(if c.is_empty() { "other" } else { c }, on_disk, None);
                    stack.push((p, c));
                } else {
                    if m.dev() != dev {
                        continue;
                    }
                    if m.nlink() > 1 && !acc.seen.insert((m.dev(), m.ino())) {
                        continue;
                    }
                    let c = if !cat.is_empty() {
                        cat
                    } else if ft.is_file() {
                        ext_category(&e.file_name().to_string_lossy())
                    } else {
                        "other"
                    };
                    acc.add(c, on_disk, Some(&p));
                }
                if last.elapsed() > Duration::from_millis(300) {
                    publish(h, &acc, false, started, &dir);
                    last = Instant::now();
                }
            }
        }
    }
    publish(h, &acc, true, started, Path::new(""));
}

// ── i montaggi ─────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Default)]
pub struct Mnt {
    pub majmin: String,
    pub root: String,
    pub mount: String,
    pub opts: String,
    pub fstype: String,
    pub source: String,
    pub super_opts: String,
}

fn unescape(s: &str) -> String {
    s.replace("\\040", " ").replace("\\011", "\t").replace("\\012", "\n").replace("\\134", "\\")
}

pub fn parse_mountinfo(text: &str) -> Vec<Mnt> {
    text.lines()
        .filter_map(|l| {
            let (a, b) = l.split_once(" - ")?;
            let a: Vec<&str> = a.split(' ').collect();
            let b: Vec<&str> = b.split(' ').collect();
            if a.len() < 6 || b.len() < 2 {
                return None;
            }
            Some(Mnt {
                majmin: a[2].into(),
                root: unescape(a[3]),
                mount: unescape(a[4]),
                opts: a[5].into(),
                fstype: b[0].into(),
                source: unescape(b[1]),
                super_opts: b.get(2).map(|s| s.to_string()).unwrap_or_default(),
            })
        })
        .collect()
}

pub fn mounts() -> Vec<Mnt> {
    parse_mountinfo(&fs::read_to_string("/proc/self/mountinfo").unwrap_or_default())
}

/// Il montaggio che contiene `path` (il più lungo; a parità, l'ultimo).
pub fn mount_of<'a>(all: &'a [Mnt], path: &str) -> Option<&'a Mnt> {
    let mut best: Option<&Mnt> = None;
    for m in all {
        let inside = path == m.mount || m.mount == "/" || path.starts_with(&format!("{}/", m.mount));
        if inside && best.is_none_or(|b| m.mount.len() >= b.mount.len()) {
            best = Some(m);
        }
    }
    best
}

/// Dove sta davvero una cartella montata con un bind: il montaggio intero
/// dello stesso file system più la radice del bind (android_storage →
/// /home/defaultuser/.local/share/waydroid/data/media/0).
pub fn bind_source(all: &[Mnt], m: &Mnt) -> Option<String> {
    if m.root == "/" {
        return None;
    }
    let whole = all.iter().find(|x| x.majmin == m.majmin && x.root == "/")?;
    let base = if whole.mount == "/" { String::new() } else { whole.mount.clone() };
    Some(format!("{base}{}", m.root))
}

// ── i dispositivi ──────────────────────────────────────────────────────────

fn rd(p: impl AsRef<Path>) -> String {
    fs::read_to_string(p).map(|s| s.trim().to_string()).unwrap_or_default()
}

fn dev_majmin(dev: u64) -> String {
    let major = ((dev >> 8) & 0xfff) | ((dev >> 32) & !0xfff);
    let minor = (dev & 0xff) | ((dev >> 12) & !0xff);
    format!("{major}:{minor}")
}

/// Come è attaccato un disco: "ufs", "emmc", "sd", "usb", "nvme", "virtual" o "".
/// Rende anche la cartella del controller (per health_descriptor della UFS).
fn transport(disk: &Path) -> (&'static str, Option<PathBuf>) {
    let name = disk.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let real = fs::canonicalize(disk).unwrap_or_else(|_| disk.to_path_buf());
    let path = real.to_string_lossy().into_owned();
    if name.starts_with("mmcblk") {
        return (if rd(real.join("device/type")) == "SD" { "sd" } else { "emmc" }, None);
    }
    if path.contains("/virtual/") {
        return ("virtual", None);
    }
    if path.contains("/usb") {
        return ("usb", None);
    }
    if name.starts_with("nvme") {
        return ("nvme", None);
    }
    let mut cur = real.as_path();
    while let Some(p) = cur.parent() {
        let drv = fs::read_link(p.join("driver")).ok().and_then(|d| d.file_name().map(|n| n.to_string_lossy().to_lowercase()));
        if p.join("health_descriptor").is_dir() || drv.is_some_and(|d| d.contains("ufs")) {
            return ("ufs", Some(p.to_path_buf()));
        }
        cur = p;
    }
    ("", None)
}

/// La catena dei dispositivi sotto `sys` (una cartella di /sys/class/block):
/// mapper (dm-crypt/LUKS) → loop (il file dietro) → partizione → disco.
fn node(sys: &Path, depth: u32, bottom: &mut Option<PathBuf>) -> Value {
    let name = sys.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let mut n = json!({ "name": name, "size": rd(sys.join("size")).parse::<u64>().unwrap_or(0) * 512 });
    if depth > 8 {
        return n;
    }
    let below = |n: &mut Value, b: &mut Option<PathBuf>| {
        let mut kids = vec![];
        if let Ok(rdir) = fs::read_dir(sys.join("slaves")) {
            for e in rdir.flatten() {
                let p = fs::canonicalize(e.path()).unwrap_or(e.path());
                kids.push(node(&p, depth + 1, b));
            }
        }
        n["below"] = Value::Array(kids);
    };
    if sys.join("dm").is_dir() {
        n["kind"] = "dm".into();
        n["dmName"] = rd(sys.join("dm/name")).into();
        let uuid = rd(sys.join("dm/uuid"));
        if let Some(rest) = uuid.strip_prefix("CRYPT-") {
            let kind = rest.split('-').next().unwrap_or("");
            n["crypt"] = kind.into();
            let hex = rest.split('-').nth(1).unwrap_or("");
            if hex.len() == 32 {
                n["cryptUuid"] = format!("{}-{}-{}-{}-{}", &hex[..8], &hex[8..12], &hex[12..16], &hex[16..20], &hex[20..]).into();
            }
        }
        below(&mut n, bottom);
    } else if name.starts_with("loop") {
        n["kind"] = "loop".into();
        let file = rd(sys.join("loop/backing_file"));
        n["backingFile"] = file.clone().into();
        if let Ok(m) = fs::metadata(&file)
            && let Ok(p) = fs::canonicalize(format!("/sys/dev/block/{}", dev_majmin(m.dev())))
        {
            n["below"] = Value::Array(vec![node(&p, depth + 1, bottom)]);
        } else {
            below(&mut n, bottom);
        }
    } else if sys.join("partition").exists() {
        n["kind"] = "partition".into();
        n["number"] = rd(sys.join("partition")).parse::<u64>().unwrap_or(0).into();
        if let Some(disk) = sys.parent() {
            n["below"] = Value::Array(vec![node(disk, depth + 1, bottom)]);
        }
    } else {
        n["kind"] = "disk".into();
        if bottom.is_none() {
            *bottom = Some(sys.to_path_buf());
        }
        let (t, ctrl) = transport(sys);
        n["transport"] = t.into();
        let d = sys.join("device");
        let model = format!("{} {}", rd(d.join("vendor")), rd(d.join("model"))).trim().to_string();
        if !model.is_empty() {
            n["model"] = model.into();
        }
        let rev = rd(d.join("rev"));
        if !rev.is_empty() {
            n["firmware"] = rev.into();
        }
        if matches!(t, "sd" | "emmc") {
            for (k, f) in [("mmcName", "name"), ("manfid", "manfid"), ("date", "date"), ("serial", "serial"), ("mmcType", "type")] {
                let v = rd(d.join(f));
                if !v.is_empty() {
                    n[k] = v.into();
                }
            }
        }
        if let Some(c) = ctrl {
            let h = c.join("health_descriptor");
            n["lifeA"] = rd(h.join("life_time_estimation_a")).into();
            n["lifeB"] = rd(h.join("life_time_estimation_b")).into();
            n["eol"] = rd(h.join("eol_info")).into();
            // il chip intero: tutte le unità logiche (sda, sdb…) dello stesso controller
            let mut chip = 0u64;
            if let Ok(rdir) = fs::read_dir("/sys/block") {
                for e in rdir.flatten() {
                    if let Ok(p) = fs::canonicalize(e.path())
                        && p.starts_with(&c)
                    {
                        chip += rd(p.join("size")).parse::<u64>().unwrap_or(0) * 512;
                    }
                }
            }
            n["chipSize"] = chip.into();
        }
        n["discardMax"] = rd(sys.join("queue/discard_max_bytes")).parse::<u64>().unwrap_or(0).into();
        n["rotational"] = (rd(sys.join("queue/rotational")) == "1").into();
    }
    n
}

/// Il superblocco di un ext2/3/4 (serve poterlo leggere: da root).
pub fn ext4_super(dev: &str) -> Option<Value> {
    let mut f = fs::File::open(dev).ok()?;
    let mut b = [0u8; 1024];
    f.seek(SeekFrom::Start(1024)).ok()?;
    f.read_exact(&mut b).ok()?;
    parse_ext4_super(&b)
}

pub fn parse_ext4_super(b: &[u8; 1024]) -> Option<Value> {
    let u16le = |o: usize| u16::from_le_bytes([b[o], b[o + 1]]);
    let u32le = |o: usize| u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]);
    if u16le(0x38) != 0xEF53 {
        return None;
    }
    let state = u16le(0x3A);
    Some(json!({
        "clean": state & 1 != 0,
        "errorsFlag": state & 2 != 0,
        "mtime": u32le(0x2C),
        "wtime": u32le(0x30),
        "mountCount": u16le(0x34),
        "maxMountCount": u16le(0x36) as i16,
        "lastCheck": u32le(0x40),
        "checkInterval": u32le(0x44),
        "errorCount": u32le(0x194),
        "firstErrorTime": u32le(0x198),
        "lastErrorTime": u32le(0x1CC),
    }))
}

const TOOL_DIRS: [&str; 4] = ["/sbin", "/usr/sbin", "/bin", "/usr/bin"];

/// Il programma che controlla quel file system, se c'è sul telefono.
pub fn check_tool(fstype: &str) -> Option<String> {
    let names: &[&str] = match fstype {
        "ext2" | "ext3" | "ext4" => &["e2fsck"],
        "vfat" | "msdos" | "fat" => &["fsck.fat", "fsck.vfat", "dosfsck"],
        "exfat" => &["fsck.exfat", "exfatfsck"],
        _ => &[],
    };
    for n in names {
        for d in TOOL_DIRS {
            let p = Path::new(d).join(n);
            if p.is_file() {
                return Some(p.to_string_lossy().into_owned());
            }
        }
    }
    None
}

/// Tutto quello che la pagina «i» non legge da sola, per la memoria che
/// contiene `path`.
pub fn info(path: &str) -> Value {
    let mut o = Map::new();
    if let Ok(c) = CString::new(path) {
        let mut s: libc::statvfs = unsafe { std::mem::zeroed() };
        if unsafe { libc::statvfs(c.as_ptr(), &mut s) } == 0 {
            let fr = s.f_frsize as u64;
            o.insert("blockSize".into(), (s.f_bsize as u64).into());
            o.insert("fragmentSize".into(), fr.into());
            o.insert("total".into(), (s.f_blocks as u64 * fr).into());
            o.insert("free".into(), (s.f_bfree as u64 * fr).into());
            o.insert("available".into(), (s.f_bavail as u64 * fr).into());
            o.insert("reserved".into(), ((s.f_bfree as u64).saturating_sub(s.f_bavail as u64) * fr).into());
            o.insert("inodesTotal".into(), (s.f_files as u64).into());
            o.insert("inodesFree".into(), (s.f_ffree as u64).into());
            o.insert("readOnly".into(), (s.f_flag & libc::ST_RDONLY != 0).into());
        }
    }
    let all = mounts();
    let Some(m) = mount_of(&all, path) else { return Value::Object(o) };
    o.insert("mount".into(), m.mount.clone().into());
    o.insert("fstype".into(), m.fstype.clone().into());
    o.insert("source".into(), m.source.clone().into());
    let opts: Vec<&str> = m.opts.split(',').chain(m.super_opts.split(',')).filter(|s| !s.is_empty()).collect();
    let mut uniq: Vec<&str> = vec![];
    for x in opts {
        if !uniq.contains(&x) {
            uniq.push(x);
        }
    }
    o.insert("options".into(), uniq.join(",").into());
    if let Some(b) = bind_source(&all, m) {
        o.insert("bindOf".into(), b.into());
    }
    let sys = fs::canonicalize(format!("/sys/dev/block/{}", m.majmin)).ok();
    let mut bottom = None;
    if let Some(sys) = &sys {
        let devname = rd(sys.join("uevent")).lines().find_map(|l| l.strip_prefix("DEVNAME=").map(String::from)).unwrap_or_default();
        o.insert("device".into(), devname.clone().into());
        o.insert("chain".into(), node(sys, 0, &mut bottom));
        // il kernel tiene il conto delle letture e scritture del dispositivo?
        // (sulle partizioni della UFS di fleur no: tutti zeri)
        let stat = rd(sys.join("stat"));
        o.insert("ioCounted".into(), stat.split_whitespace().any(|x| x != "0").into());
        if m.fstype.starts_with("ext") && !devname.is_empty() {
            match ext4_super(&format!("/dev/{devname}")) {
                Some(v) => {
                    o.insert("superblock".into(), v);
                }
                None => {
                    o.insert("superblockNeedsRoot".into(), true.into());
                }
            }
        }
    }
    // TRIM: online (opzione discard), periodico (fstrim.timer), o solo possibile
    let online = m.opts.split(',').chain(m.super_opts.split(',')).any(|x| x == "discard");
    let timer_installed = ["/usr/lib/systemd/system/fstrim.timer", "/lib/systemd/system/fstrim.timer"].iter().any(|p| Path::new(p).exists());
    let timer_on = Path::new("/etc/systemd/system/timers.target.wants/fstrim.timer").exists();
    let supported = bottom.as_ref().map(|b| rd(b.join("queue/discard_max_bytes")).parse::<u64>().unwrap_or(0) > 0).unwrap_or(false);
    o.insert("trim".into(), json!({ "online": online, "timerInstalled": timer_installed, "timerOn": timer_on, "supported": supported }));
    // il controllo: solo schede e chiavette (mai la memoria interna, in uso)
    let removable = m.mount.starts_with("/run/media/") || m.mount.starts_with("/media/");
    let tool = check_tool(&m.fstype);
    o.insert("removable".into(), removable.into());
    o.insert("checkTool".into(), tool.clone().unwrap_or_default().into());
    o.insert("checkable".into(), (removable && tool.is_some() && m.source.starts_with("/dev/")).into());
    Value::Object(o)
}

// ── il controllo di una scheda ─────────────────────────────────────────────

/// Nell'helper root: fsck su un dispositivo SMONTATO. Il programma lo sceglie
/// l'helper dal tipo (mai un percorso preso dall'app), e un dispositivo
/// montato si rifiuta.
pub fn fsck(dev: String, fstype: String, repair: bool) -> u64 {
    jobs::side_job(jobs::JOB_CHECK, vec![dev.clone()], move |h| {
        let meta = fs::metadata(&dev).map_err(|e| format!("{dev}: {e}"))?;
        if !dev.starts_with("/dev/") || !meta.file_type().is_block_device() {
            return Err(format!("{dev} non è un dispositivo a blocchi"));
        }
        let majmin = dev_majmin(meta.rdev());
        if mounts().iter().any(|m| m.majmin == majmin || m.source == dev) {
            return Err("il file system è ancora montato: non si controlla".into());
        }
        let tool = check_tool(&fstype).ok_or_else(|| format!("manca il programma per controllare {fstype}"))?;
        let base = Path::new(&tool).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let args: Vec<&str> = match (base.as_str(), repair) {
            ("e2fsck", false) => vec!["-f", "-n"],
            ("e2fsck", true) => vec!["-f", "-y"],
            (_, false) => vec!["-n"],
            (b, true) if b.contains("exfat") => vec!["-y"],
            (_, true) => vec!["-a"],
        };
        h.set(|s| s.current = format!("{base} {}", args.join(" ")));
        let out_path = std::env::temp_dir().join(format!("rootree-fsck-{}.log", h.id()));
        let out = fs::File::create(&out_path).map_err(|e| e.to_string())?;
        let err = out.try_clone().map_err(|e| e.to_string())?;
        let mut child = Command::new(&tool)
            .args(&args)
            .arg(&dev)
            .env("LC_ALL", "C")
            .stdin(Stdio::null())
            .stdout(out)
            .stderr(err)
            .spawn()
            .map_err(|e| format!("{tool}: {e}"))?;
        let status = loop {
            if h.cancelled() {
                let _ = child.kill();
                let _ = child.wait();
                let _ = fs::remove_file(&out_path);
                return Ok(());
            }
            match child.try_wait() {
                Ok(Some(s)) => break s,
                Ok(None) => std::thread::sleep(Duration::from_millis(200)),
                Err(e) => return Err(e.to_string()),
            }
        };
        let mut text = fs::read_to_string(&out_path).unwrap_or_default();
        let _ = fs::remove_file(&out_path);
        if text.len() > 12000 {
            let cut = text.len() - 12000;
            let cut = (cut..text.len()).find(|i| text.is_char_boundary(*i)).unwrap_or(cut);
            text = format!("…{}", &text[cut..]);
        }
        let code = status.code().unwrap_or(-1);
        h.set(|s| s.detail = json!({ "code": code, "result": verdict(&base, code, repair), "output": text, "tool": base, "repair": repair }).to_string());
        Ok(())
    })
}

/// Cosa vuol dire il codice d'uscita: "clean", "fixed", "errors", "failed".
pub fn verdict(tool: &str, code: i32, repair: bool) -> &'static str {
    match code {
        0 => "clean",
        c if c < 0 || c & 8 != 0 => "failed",
        // e2fsck ed exfat: 1 corretti, 2 corretti (riavvio), 4 rimasti
        c if c & 4 != 0 => "errors",
        1 | 2 if repair => "fixed",
        // fsck.fat -n: 1 = trovati
        1 if tool.contains("fat") && !tool.contains("exfat") => "errors",
        1 | 2 => "errors",
        _ => "failed",
    }
}

fn udisks(args: &[&str]) -> Result<String, String> {
    let o = Command::new("udisksctl")
        .args(args)
        .arg("--no-user-interaction")
        .env("LC_ALL", "C")
        .output()
        .map_err(|e| format!("udisksctl: {e}"))?;
    let text = format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)).trim().to_string();
    if o.status.success() { Ok(text) } else { Err(text) }
}

/// Nell'app: smonta la scheda (udisks, come l'utente), la fa controllare
/// all'helper root, e la rimonta sempre, anche se il controllo fallisce.
pub fn check(mount: String, repair: bool) -> u64 {
    jobs::side_job(jobs::JOB_CHECK, vec![mount.clone()], move |h| {
        let all = mounts();
        let m = all.iter().find(|m| m.mount == mount).cloned().ok_or("la memoria non è più montata")?;
        if !(m.mount.starts_with("/run/media/") || m.mount.starts_with("/media/")) || !m.source.starts_with("/dev/") {
            return Err("si controllano solo schede e chiavette".into());
        }
        if check_tool(&m.fstype).is_none() {
            return Err(format!("manca il programma per controllare {}", m.fstype));
        }
        if !crate::root::active() {
            return Err("serve la modalità root".into());
        }
        let dev = m.source.clone();
        h.set(|s| s.current = "unmount".into());
        udisks(&["unmount", "-b", &dev]).map_err(|e| format!("non riesco a smontare la scheda (è in uso?): {e}"))?;
        let r = run_remote_check(h, &dev, &m.fstype, repair);
        h.set(|s| s.current = "mount".into());
        let mounted = udisks(&["mount", "-b", &dev]);
        let mut d: Value = match &r {
            Ok(v) => v.clone(),
            Err(e) => json!({ "result": "failed", "error": e }),
        };
        match &mounted {
            Ok(t) => d["mountedAt"] = t.rsplit(" at ").next().unwrap_or("").trim_end_matches('.').into(),
            Err(e) => d["mountError"] = e.clone().into(),
        }
        let text = d.to_string();
        h.set(|s| s.detail = text);
        if h.cancelled() {
            return Ok(());
        }
        r.map(|_| ())
    })
}

fn run_remote_check(h: &Handle, dev: &str, fstype: &str, repair: bool) -> Result<Value, String> {
    use crate::proto::Request;
    let id: u64 = crate::root::call_as(&Request::JobFsck { dev: dev.into(), fstype: fstype.into(), repair })?;
    h.set(|s| s.current = "check".into());
    let st = loop {
        std::thread::sleep(Duration::from_millis(400));
        if h.cancelled() {
            let _ = crate::root::call(&Request::JobCancel { id });
        }
        let all: Vec<JobStatus> = crate::root::call_as(&Request::JobStatuses)?;
        match all.into_iter().find(|s| s.id == id) {
            Some(s) if s.state >= jobs::ST_DONE => break s,
            Some(s) => {
                let cur = s.current.clone();
                h.set(|x| x.dest = cur);
            }
            None => return Err("il controllo è sparito dall'helper".into()),
        }
    };
    let _ = crate::root::call(&Request::JobForget { id });
    match st.state {
        jobs::ST_DONE => serde_json::from_str(&st.detail).map_err(|e| e.to_string()),
        jobs::ST_CANCELLED => Err("annullato".into()),
        _ => Err(st.error),
    }
}
