//! Nucleo di RooTree. Per ora: elencare una cartella locale.

use std::fs;

#[cxx::bridge(namespace = "rootree")]
mod ffi {
    struct Entry {
        name: String,
        is_dir: bool,
        size: u64,
    }

    extern "Rust" {
        fn version() -> String;
        fn list_dir(path: &str, show_hidden: bool) -> Result<Vec<Entry>>;
    }
}

use ffi::Entry;

fn version() -> String {
    format!("{} ({})", env!("CARGO_PKG_VERSION"), std::env::consts::ARCH)
}

/// Le cartelle prima dei file, poi in ordine alfabetico senza distinguere le maiuscole.
fn list_dir(path: &str, show_hidden: bool) -> std::io::Result<Vec<Entry>> {
    let mut entries = Vec::new();
    for item in fs::read_dir(path)? {
        let item = item?;
        let name = item.file_name().to_string_lossy().into_owned();
        if !show_hidden && name.starts_with('.') {
            continue;
        }
        // metadata() segue i link: un link a una cartella si apre come una cartella;
        // un link rotto si mostra comunque, come file
        let meta = fs::metadata(item.path()).or_else(|_| item.metadata());
        let (is_dir, size) = meta.map(|m| (m.is_dir(), m.len())).unwrap_or((false, 0));
        entries.push(Entry { name, is_dir, size });
    }
    entries.sort_by(|a, b| {
        b.is_dir.cmp(&a.is_dir).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(entries)
}
