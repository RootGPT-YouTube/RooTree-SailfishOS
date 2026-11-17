//! Nucleo di RooTree: il filesystem locale (elenco, memorie, operazioni
//! immediate) e il motore delle operazioni lunghe. Il C++ lo usa attraverso
//! il ponte `ffi` qui sotto.

pub mod archive;
pub mod cloud;
pub mod http;
pub mod oauth;
pub mod fsops;
pub mod helper;
pub mod jobs;
pub mod proto;
pub mod remote;
pub mod root;
pub mod search;
pub mod thumbs;
pub mod trash;
pub mod vault;
pub mod vfs;
pub mod webdav;
pub mod ftp;
pub mod smb;
pub mod s3;
pub mod storage;
pub mod kdrive;
pub mod sync;

/// Per gli esempi a mano (prova-copia): lo stato di un lavoro senza i tipi del ponte.
pub mod ffi_status {
    pub struct Status {
        pub state: u8,
        pub error: String,
        pub bytes: u64,
        pub ask_dst: String,
    }
}
#[cfg(test)]
mod tests;

// i derive di serde dentro il ponte cxx usano questi nomi
#[allow(unused_imports)]
use serde::{Deserialize, Serialize};

#[cxx::bridge(namespace = "rootree")]
mod ffi {
    /// Un elemento di una cartella. `kind`: 0 file, 1 cartella, 2 altro
    /// (fifo, socket, dispositivo). Per un link, i dati sono quelli di ciò a
    /// cui punta (se non è rotto).
    #[derive(Serialize, Deserialize)]
    struct Entry {
        name: String,
        path: String,
        kind: u8,
        is_link: bool,
        link_broken: bool,
        link_target: String,
        size: u64,
        mtime: i64,
        mode: u32,
        uid: u32,
        gid: u32,
        hidden: bool,
        category: String,
    }

    /// Una memoria. `kind`: 0 interna, 1 Android, 2 scheda SD, 3 USB, 4 radice.
    struct Place {
        kind: u8,
        label: String,
        path: String,
        total: u64,
        free: u64,
    }

    /// Lo stato di un'operazione (vedi le costanti in jobs.rs).
    #[derive(Clone, Serialize, Deserialize)]
    struct JobStatus {
        id: u64,
        kind: u8,
        state: u8,
        total_bytes: u64,
        done_bytes: u64,
        total_items: u64,
        done_items: u64,
        dirs: u64,
        current: String,
        error: String,
        ask_src: String,
        ask_dst: String,
        ask_dst_is_dir: bool,
        /// conflitto fra due cartelle (la domanda è «unire?», come in Dolphin)
        ask_src_is_dir: bool,
        dest: String,
        first_source: String,
        sources: u64,
        /// i dati in più di un lavoro, in JSON (per tipo di file, esito del controllo)
        detail: String,
    }

    /// Un risultato della ricerca; `snippet`: la riga trovata (ricerca nel contenuto).
    struct SearchHit {
        entry: Entry,
        snippet: String,
    }

    /// I risultati arrivati dall'ultima chiamata e lo stato della ricerca.
    #[derive(Default)]
    struct SearchBatch {
        hits: Vec<SearchHit>,
        found: u64,
        scanned: u64,
        done: bool,
        limited: bool,
        current: String,
        error: String,
    }

    /// Un'immagine decodificata e rimpicciolita, RGBA a 8 bit.
    struct Picture {
        width: u32,
        height: u32,
        rgba: Vec<u8>,
    }

    extern "Rust" {
        fn version() -> String;

        /// Decodifica `path` (WebP, TIFF, BMP) e la riduce entro `max` px di lato.
        fn decode_picture(path: &str, max: u32) -> Result<Picture>;
        /// Il testo di un file, al più `max_bytes`; errore se sembra binario.
        fn read_text(path: &str, max_bytes: u64) -> Result<String>;

        fn list_dir(path: &str, show_hidden: bool, sort_key: u8, descending: bool, dirs_first: bool) -> Result<Vec<Entry>>;
        fn stat_path(path: &str) -> Result<Entry>;
        fn places(home: &str) -> Vec<Place>;
        fn make_dir(parent: &str, name: &str) -> Result<String>;
        /// Un file vuoto nuovo (mai sopra uno che c'è).
        fn make_file(parent: &str, name: &str) -> Result<String>;
        fn rename_entry(path: &str, new_name: &str) -> Result<String>;
        fn set_mode(path: &str, mode: u32) -> Result<()>;
        /// Proprietario e gruppo (nome o numero; vuoto = non cambiare): serve root.
        fn set_owner(path: &str, user: &str, group: &str) -> Result<()>;
        /// Salva un file di testo (atomico); da root con copia di sicurezza,
        /// di cui rende il percorso.
        fn write_text(path: &str, text: &str) -> Result<String>;
        /// Modalità root: avvia l'helper con devel-su (errori: vedi root.rs).
        fn root_start(password: &str) -> Result<()>;
        fn root_stop();
        fn root_active() -> bool;

        /// Connessioni remote (Fase 5): collega un profilo (JSON di Remotes)
        /// e rende il percorso iniziale; errori con codice (vedi remote.rs).
        fn remote_connect(profile_json: &str, secret: &str) -> Result<String>;
        /// Fidarsi del server appena visto (dopo HOSTKEY_NEW).
        fn remote_trust(id: &str) -> Result<()>;
        fn remote_disconnect(id: &str);
        /// Accesso dal browser (cloud): l'URL da aprire; poi `oauth_complete`
        /// aspetta il ritorno (bloccante, su un thread) e rende il token di rinnovo.
        fn oauth_begin(id: &str, kind: &str, client_id: &str) -> Result<String>;
        fn oauth_complete(id: &str, kind: &str, client_id: &str, timeout_secs: u32) -> Result<String>;
        fn oauth_cancel(id: &str);
        fn remote_connected(id: &str) -> bool;

        /// La cassaforte delle password (vault.rs).
        fn vault_set_path(path: &str);
        fn vault_exists() -> bool;
        fn vault_unlocked() -> bool;
        fn vault_create(master: &str) -> Result<()>;
        fn vault_unlock(master: &str) -> Result<()>;
        fn vault_lock();
        fn vault_get(id: &str) -> String;
        fn vault_set(id: &str, secret: &str) -> Result<()>;
        fn vault_remove(id: &str) -> Result<()>;
        fn mega_revoke(id: &str) -> Result<()>;
        fn remote_trash(id: &str, action: &str, item: &str) -> Result<String>;
        fn user_name(uid: u32) -> String;
        fn group_name(gid: u32) -> String;

        fn job_copy(sources: Vec<String>, dest: &str) -> u64;
        fn job_move(sources: Vec<String>, dest: &str) -> u64;
        fn job_delete(paths: Vec<String>) -> u64;
        /// Nel cestino (trash.rs).
        fn job_trash(paths: Vec<String>) -> u64;
        /// Prima di eliminare: "trash", "service:<tipo>" o "none:<perché>".
        fn trash_hint(paths: Vec<String>) -> String;
        /// Il cestino della memoria dove sta `scope` (una cartella locale):
        /// list | restore | delete | empty (anche quello di root, se accesa).
        fn trash_local(scope: &str, action: &str, item: &str) -> Result<String>;
        /// Giorni dopo cui le voci si tolgono da sole (0 = mai).
        fn trash_set_days(days: u32);
        /// All'avvio, in un filo: le voci vecchie dei cestini locali.
        fn trash_purge_local();
        fn job_size(paths: Vec<String>) -> u64;
        /// Estrae tutto l'archivio in `dest` («Estrai qui»).
        /// la pagina «i» di una memoria: l'occupato per tipo di file (lavoro),
        /// i dati del file system (JSON), il controllo di una scheda (lavoro)
        fn job_types(paths: Vec<String>) -> u64;
        fn storage_info(path: &str) -> String;
        fn job_fs_check(mount: &str, repair: bool) -> u64;
        fn job_extract(archive: &str, dest: &str) -> u64;
        /// Crea l'archivio `dest` (formato dal nome: .zip .tar .tar.gz .tar.bz2 .tar.xz .tar.zst .7z).
        fn job_compress(sources: Vec<String>, dest: &str) -> u64;
        /// Una sincronizzazione (sync.rs): `config_json` = la sincronizzazione,
        /// `state_dir` = dove sta lo stato dei giri; `confirmed` = l'utente ha
        /// confermato le eliminazioni che le salvaguardie avevano fermato.
        fn job_sync(config_json: &str, state_dir: &str, confirmed: bool) -> u64;
        fn sync_forget(state_dir: &str, id: &str);
        fn job_statuses() -> Vec<JobStatus>;

        /// Se `path` è (o sta dentro) un archivio: il file dell'archivio, altrimenti "".
        fn archive_of(path: &str) -> String;
        /// Si può sfogliare come una cartella (dal nome).
        fn is_archive(name: &str) -> bool;
        /// Si può creare con questo nome.
        fn can_create_archive(name: &str) -> bool;
        /// Estrae una voce dell'archivio in `dir` (per aprirla) e ne rende il percorso.
        fn archive_extract_one(path: &str, dir: &str) -> Result<String>;
        /// Il nome di un archivio senza le sue estensioni.
        fn archive_stem(name: &str) -> String;

        /// mode: 0 nome (anche * e ?), 1 regex sul nome, 2 testo nel contenuto, 3 regex nel contenuto.
        fn search_start(root: &str, query: &str, mode: u8, case_sensitive: bool, hidden: bool) -> Result<u64>;
        fn search_take(id: u64) -> SearchBatch;
        fn search_stop(id: u64);
        fn job_cancel(id: u64);
        fn job_resolve(id: u64, action: u8, apply_all: bool);
        fn job_forget(id: u64);
    }
}

use fsops::*;

fn version() -> String {
    format!("{} ({})", env!("CARGO_PKG_VERSION"), std::env::consts::ARCH)
}

fn decode_picture(path: &str, max: u32) -> Result<ffi::Picture, String> {
    thumbs::decode_picture(path, max)
}












fn archive_of(path: &str) -> String {
    archive::split(path).map(|(a, _)| a.to_string_lossy().into_owned()).unwrap_or_default()
}

fn is_archive(name: &str) -> bool {
    archive::format_of(name).is_some()
}

fn can_create_archive(name: &str) -> bool {
    archive::creatable(name).is_some()
}

fn archive_extract_one(path: &str, dir: &str) -> Result<String, String> {
    let (a, inner) = archive::split(path).ok_or("non è dentro un archivio")?;
    archive::extract_one(&a, &inner, std::path::Path::new(dir))
        .map(|p| p.to_string_lossy().into_owned())
        .map_err(|e| e.to_string())
}

fn archive_stem(name: &str) -> String {
    jobs::archive_stem(name)
}

fn search_start(root: &str, query: &str, mode: u8, case_sensitive: bool, hidden: bool) -> Result<u64, String> {
    search::start(root, query, mode, case_sensitive, hidden)
}

fn search_take(id: u64) -> ffi::SearchBatch {
    search::take(id)
}

fn search_stop(id: u64) {
    search::stop(id)
}

// ------------------------------------------------------------------ instradamento
// Con la modalità root accesa le operazioni sul filesystem le fa l'helper;
// i lavori dell'helper hanno l'id col bit REMOTE, così si distinguono da
// quelli locali nello stesso elenco.

use proto::Request;

// ⛔ dentro i 31 bit: il C++ passa gli id al QML come int (con 1 << 40 il bit
//    si perdeva e Annulla su un lavoro root finiva su quello locale, 08/11)
const REMOTE: u64 = 1 << 30;

fn list_dir(path: &str, show_hidden: bool, sort_key: u8, descending: bool, dirs_first: bool) -> Result<Vec<ffi::Entry>, String> {
    if let Some((id, inner)) = remote::split(path) {
        let mut v = remote::list(&id, &inner, show_hidden).map_err(|e| e.to_string())?;
        fsops::sort_entries(&mut v, sort_key, descending, dirs_first);
        return Ok(v);
    }
    if root::active() {
        return root::call_as(&Request::ListDir { path: path.into(), hidden: show_hidden, key: sort_key, desc: descending, dirs_first });
    }
    fsops::list_dir(path, show_hidden, sort_key, descending, dirs_first).map_err(|e| e.to_string())
}

fn stat_path(path: &str) -> Result<ffi::Entry, String> {
    if let Some((id, inner)) = remote::split(path) {
        return remote::stat(&id, &inner).map_err(|e| e.to_string());
    }
    if root::active() {
        return root::call_as(&Request::Stat { path: path.into() });
    }
    fsops::stat_path(path).map_err(|e| e.to_string())
}

fn read_text(path: &str, max_bytes: u64) -> Result<String, String> {
    if let Some((id, inner)) = remote::split(path) {
        let data = remote::read_head(&id, &inner, max_bytes).map_err(|e| e.to_string())?;
        if data[..data.len().min(8192)].contains(&0) {
            return Err("binary".into());
        }
        return Ok(String::from_utf8_lossy(&data).into_owned());
    }
    if root::active() {
        return root::call_as(&Request::ReadText { path: path.into(), max: max_bytes });
    }
    thumbs::read_text(path, max_bytes)
}

fn write_text(path: &str, text: &str) -> Result<String, String> {
    if let Some((id, inner)) = remote::split(path) {
        return remote::write_text(&id, &inner, text).map(|_| String::new()).map_err(|e| e.to_string());
    }
    if root::active() {
        return root::call_as(&Request::WriteText { path: path.into(), text: text.into() });
    }
    fsops::write_text(path, text, false).map_err(|e| e.to_string())
}

fn make_dir(parent: &str, name: &str) -> Result<String, String> {
    if let Some((id, inner)) = remote::split(parent) {
        return remote::make_dir(&id, &inner, name).map_err(|e| e.to_string());
    }
    if root::active() {
        return root::call_as(&Request::MakeDir { parent: parent.into(), name: name.into() });
    }
    fsops::make_dir(parent, name).map_err(|e| e.to_string())
}

fn make_file(parent: &str, name: &str) -> Result<String, String> {
    if let Some((id, inner)) = remote::split(parent) {
        return remote::make_file(&id, &inner, name).map_err(|e| e.to_string());
    }
    if root::active() {
        return root::call_as(&Request::MakeFile { parent: parent.into(), name: name.into() });
    }
    fsops::make_file(parent, name).map_err(|e| e.to_string())
}

fn rename_entry(path: &str, new_name: &str) -> Result<String, String> {
    if let Some((id, inner)) = remote::split(path) {
        return remote::rename_entry(&id, &inner, new_name).map_err(|e| e.to_string());
    }
    if root::active() {
        return root::call_as(&Request::Rename { path: path.into(), name: new_name.into() });
    }
    fsops::rename_entry(path, new_name).map_err(|e| e.to_string())
}

fn set_mode(path: &str, mode: u32) -> Result<(), String> {
    if let Some((id, inner)) = remote::split(path) {
        return remote::set_stat_any(&id, &inner, Some(mode & 0o7777), None).map_err(|e| e.to_string());
    }
    if root::active() {
        return root::call_as(&Request::SetMode { path: path.into(), mode });
    }
    fsops::set_mode(path, mode).map_err(|e| e.to_string())
}

fn set_owner(path: &str, user: &str, group: &str) -> Result<(), String> {
    if root::active() {
        return root::call_as(&Request::SetOwner { path: path.into(), user: user.into(), group: group.into() });
    }
    fsops::set_owner(path, user, group).map_err(|e| e.to_string())
}

fn root_start(password: &str) -> Result<(), String> {
    root::start(password)
}

fn root_stop() {
    root::stop()
}

fn root_active() -> bool {
    root::active()
}

/// Un lavoro dall'helper se la modalità root è accesa (con ripiego locale se
/// l'helper non risponde), altrimenti locale.
fn job_via(req: Request, local: impl FnOnce() -> u64) -> u64 {
    // ⛔ l'helper root non ha le connessioni remote: un lavoro che tocca un
    //    server si fa sempre nell'app
    let touches_remote = serde_json::to_string(&req).map(|j| j.contains(remote::PREFIX)).unwrap_or(false);
    if root::active()
        && !touches_remote
        && let Ok(id) = root::call_as::<u64>(&req)
    {
        return id | REMOTE;
    }
    local()
}

fn job_copy(sources: Vec<String>, dest: &str) -> u64 {
    job_via(Request::JobCopy { sources: sources.clone(), dest: dest.into() }, || jobs::copy(sources, dest))
}

fn job_move(sources: Vec<String>, dest: &str) -> u64 {
    job_via(Request::JobMove { sources: sources.clone(), dest: dest.into() }, || jobs::move_to(sources, dest))
}

fn job_delete(paths: Vec<String>) -> u64 {
    job_via(Request::JobDelete { paths: paths.clone() }, || jobs::delete(paths))
}

/// Nel cestino: dall'helper root solo se serve (vedi trash::needs_root: la
/// cartella che lo contiene, ma anche l'elemento stesso), così i file
/// dell'utente vanno nel SUO cestino anche con la modalità root accesa.
fn job_trash(paths: Vec<String>) -> u64 {
    let needs_root = paths.iter().any(|p| remote::split(p).is_none() && trash::needs_root(std::path::Path::new(p)));
    if root::active()
        && needs_root
        && let Ok(id) = root::call_as::<u64>(&Request::JobTrash { paths: paths.clone(), days: trash::days() })
    {
        return id | REMOTE;
    }
    jobs::trash(paths)
}

fn trash_hint(paths: Vec<String>) -> String {
    let h = trash::hint(&paths);
    // in modalità root il cestino di root c'è (sulla scheda in sola lettura no, ma lì fallisce anche il resto)
    if root::active() && h.starts_with("none:serve la modalità root") {
        return "trash".into();
    }
    h
}

fn trash_local(scope: &str, action: &str, item: &str) -> Result<String, String> {
    let via_root = |item: &str| root::call_as::<String>(&Request::TrashOp { scope: scope.into(), action: action.into(), item: item.into(), days: trash::days() });
    match action {
        "list" => {
            let mut all: Vec<serde_json::Value> = serde_json::from_str(&trash::op(scope, "list", "").map_err(|e| e.to_string())?).unwrap_or_default();
            if root::active()
                && let Ok(j) = via_root("")
            {
                let more: Vec<serde_json::Value> = serde_json::from_str(&j).unwrap_or_default();
                all.extend(more.into_iter().map(|mut v| {
                    v["root"] = true.into();
                    v
                }));
            }
            all.sort_by_key(|v| -v["deleted"].as_i64().unwrap_or(0));
            Ok(serde_json::Value::Array(all).to_string())
        }
        "empty" => {
            trash::op(scope, "empty", "").map_err(|e| e.to_string())?;
            if root::active() {
                via_root("")?;
            }
            Ok(String::new())
        }
        _ if item.contains("/.Trash-0/") && root::active() => via_root(item),
        _ => trash::op(scope, action, item).map_err(|e| e.to_string()),
    }
}

fn trash_set_days(days: u32) {
    trash::set_days(days)
}

fn trash_purge_local() {
    std::thread::spawn(trash::purge_local);
}

fn job_size(paths: Vec<String>) -> u64 {
    job_via(Request::JobSize { paths: paths.clone() }, || jobs::size(paths))
}

fn job_sync(config_json: &str, state_dir: &str, confirmed: bool) -> u64 {
    sync::start(config_json, state_dir, confirmed)
}

fn sync_forget(state_dir: &str, id: &str) {
    sync::forget(state_dir, id)
}

fn job_types(paths: Vec<String>) -> u64 {
    job_via(Request::JobTypes { paths: paths.clone() }, || storage::types(paths))
}

/// Da root si legge anche il superblocco ext4; senza, quello che si può.
fn storage_info(path: &str) -> String {
    if root::active()
        && let Ok(v) = root::call(&Request::StorageInfo { path: path.into() })
    {
        return v.to_string();
    }
    storage::info(path).to_string()
}

/// Sempre nell'app: smonta e rimonta come l'utente, il controllo lo chiede all'helper.
fn job_fs_check(mount: &str, repair: bool) -> u64 {
    storage::check(mount.into(), repair)
}

fn job_extract(archive: &str, dest: &str) -> u64 {
    job_via(Request::JobExtract { archive: archive.into(), dest: dest.into() }, || jobs::extract(archive, dest))
}

fn job_compress(sources: Vec<String>, dest: &str) -> u64 {
    job_via(Request::JobCompress { sources: sources.clone(), dest: dest.into() }, || jobs::compress(sources, dest))
}

fn job_statuses() -> Vec<ffi::JobStatus> {
    let mut v = jobs::statuses();
    if root::active()
        && let Ok(remote) = root::call_as::<Vec<ffi::JobStatus>>(&Request::JobStatuses)
    {
        v.extend(remote.into_iter().map(|mut j| {
            j.id |= REMOTE;
            j
        }));
    }
    v
}

fn job_cancel(id: u64) {
    if id & REMOTE != 0 {
        let _ = root::call(&Request::JobCancel { id: id & !REMOTE });
    } else {
        jobs::cancel(id)
    }
}

fn job_resolve(id: u64, action: u8, apply_all: bool) {
    if id & REMOTE != 0 {
        let _ = root::call(&Request::JobResolve { id: id & !REMOTE, action, all: apply_all });
    } else {
        jobs::resolve(id, action, apply_all)
    }
}

fn job_forget(id: u64) {
    if id & REMOTE != 0 {
        let _ = root::call(&Request::JobForget { id: id & !REMOTE });
    } else {
        jobs::forget(id)
    }
}

fn remote_connect(profile_json: &str, secret: &str) -> Result<String, String> {
    remote::connect(profile_json, secret)
}

fn remote_trust(id: &str) -> Result<(), String> {
    remote::trust(id)
}

fn remote_disconnect(id: &str) {
    remote::disconnect(id)
}

fn remote_connected(id: &str) -> bool {
    remote::connected(id)
}

fn vault_set_path(path: &str) {
    vault::set_path(path)
}

fn vault_exists() -> bool {
    vault::exists()
}

fn vault_unlocked() -> bool {
    vault::unlocked()
}

fn vault_create(master: &str) -> Result<(), String> {
    vault::create(master).map_err(|e| e.to_string())
}

fn vault_unlock(master: &str) -> Result<(), String> {
    vault::unlock(master).map_err(|e| e.to_string())
}

fn vault_lock() {
    vault::lock()
}

fn vault_get(id: &str) -> String {
    vault::get(id).unwrap_or_default()
}

fn vault_set(id: &str, secret: &str) -> Result<(), String> {
    vault::set(id, secret).map_err(|e| e.to_string())
}

fn vault_remove(id: &str) -> Result<(), String> {
    vault::remove(id).map_err(|e| e.to_string())
}

fn remote_trash(id: &str, action: &str, item: &str) -> Result<String, String> {
    remote::trash(id, action, item)
}

fn mega_revoke(id: &str) -> Result<(), String> {
    cloud::mega_revoke(id)
}

fn oauth_begin(id: &str, kind: &str, client_id: &str) -> Result<String, String> {
    oauth::begin(id, kind, client_id)
}

fn oauth_complete(id: &str, kind: &str, client_id: &str, timeout_secs: u32) -> Result<String, String> {
    oauth::complete(id, kind, client_id, std::time::Duration::from_secs(timeout_secs as u64)).map(|t| t.refresh_token)
}

fn oauth_cancel(id: &str) {
    oauth::cancel(id)
}
