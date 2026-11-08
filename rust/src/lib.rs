//! Nucleo di RooTree: il filesystem locale (elenco, memorie, operazioni
//! immediate) e il motore delle operazioni lunghe. Il C++ lo usa attraverso
//! il ponte `ffi` qui sotto.

mod archive;
mod fsops;
mod jobs;
mod search;
mod thumbs;
#[cfg(test)]
mod tests;

#[cxx::bridge(namespace = "rootree")]
mod ffi {
    /// Un elemento di una cartella. `kind`: 0 file, 1 cartella, 2 altro
    /// (fifo, socket, dispositivo). Per un link, i dati sono quelli di ciò a
    /// cui punta (se non è rotto).
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
    #[derive(Clone)]
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
        dest: String,
        first_source: String,
        sources: u64,
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
        fn rename_entry(path: &str, new_name: &str) -> Result<String>;
        fn set_mode(path: &str, mode: u32) -> Result<()>;
        fn user_name(uid: u32) -> String;
        fn group_name(gid: u32) -> String;

        fn job_copy(sources: Vec<String>, dest: &str) -> u64;
        fn job_move(sources: Vec<String>, dest: &str) -> u64;
        fn job_delete(paths: Vec<String>) -> u64;
        fn job_size(paths: Vec<String>) -> u64;
        /// Estrae tutto l'archivio in `dest` («Estrai qui»).
        fn job_extract(archive: &str, dest: &str) -> u64;
        /// Crea l'archivio `dest` (formato dal nome: .zip .tar .tar.gz .tar.bz2 .tar.xz .tar.zst .7z).
        fn job_compress(sources: Vec<String>, dest: &str) -> u64;
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

fn read_text(path: &str, max_bytes: u64) -> Result<String, String> {
    thumbs::read_text(path, max_bytes)
}

fn job_copy(sources: Vec<String>, dest: &str) -> u64 {
    jobs::copy(sources, dest)
}

fn job_move(sources: Vec<String>, dest: &str) -> u64 {
    jobs::move_to(sources, dest)
}

fn job_delete(paths: Vec<String>) -> u64 {
    jobs::delete(paths)
}

fn job_size(paths: Vec<String>) -> u64 {
    jobs::size(paths)
}

fn job_statuses() -> Vec<ffi::JobStatus> {
    jobs::statuses()
}

fn job_cancel(id: u64) {
    jobs::cancel(id)
}

fn job_resolve(id: u64, action: u8, apply_all: bool) {
    jobs::resolve(id, action, apply_all)
}

fn job_forget(id: u64) {
    jobs::forget(id)
}

fn job_extract(archive: &str, dest: &str) -> u64 {
    jobs::extract(archive, dest)
}

fn job_compress(sources: Vec<String>, dest: &str) -> u64 {
    jobs::compress(sources, dest)
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
