//! Nucleo di RooTree: il filesystem locale (elenco, memorie, operazioni
//! immediate) e il motore delle operazioni lunghe. Il C++ lo usa attraverso
//! il ponte `ffi` qui sotto.

mod fsops;
mod jobs;
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

    extern "Rust" {
        fn version() -> String;

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
        fn job_statuses() -> Vec<JobStatus>;
        fn job_cancel(id: u64);
        fn job_resolve(id: u64, action: u8, apply_all: bool);
        fn job_forget(id: u64);
    }
}

use fsops::*;

fn version() -> String {
    format!("{} ({})", env!("CARGO_PKG_VERSION"), std::env::consts::ARCH)
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
