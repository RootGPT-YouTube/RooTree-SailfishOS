//! Prova a mano dello step 3 sul telefono (non va nel pacchetto): copia,
//! spostamento, eliminazione fra telefono e server col motore vero.
//! Argomenti: host porta utente chiave cartella-locale-di-lavoro cartella-remota.
//! Scrive SOLO nella cartella remota indicata (che alla fine cancella).
use rootree_core::jobs;

fn wait(id: u64) -> rootree_core::ffi_status::Status {
    loop {
        if let Some(s) = jobs::statuses().into_iter().find(|j| j.id == id)
            && (s.state >= jobs::ST_DONE || s.state == jobs::ST_ASK_CONFLICT || s.state == jobs::ST_ASK_ERROR)
        {
            return rootree_core::ffi_status::Status { state: s.state, error: s.error, bytes: s.done_bytes, ask_dst: s.ask_dst };
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let (local, rdir) = (a[5].clone(), a[6].clone());
    let profile = serde_json::json!({
        "id": "p", "type": "sftp", "host": a[1], "port": a[2].parse::<u16>().unwrap(),
        "user": a[3], "auth": "key", "keyPath": a[4], "path": ""
    })
    .to_string();
    rootree_core::remote::connect(&profile, "").expect("collegamento");
    let r = |p: &str| format!("/@remote/p{rdir}{p}");
    let t = std::time::Instant::now();
    let mk = rootree_core::remote::make_dir("p", std::path::Path::new(&rdir).parent().unwrap().to_str().unwrap(), std::path::Path::new(&rdir).file_name().unwrap().to_str().unwrap());
    println!("nuova cartella remota: {}", mk.is_ok());

    // 1. caricare l'albero (con il file da 1 GB)
    let id = jobs::copy(vec![format!("{local}/albero")], &r(""));
    let s = wait(id);
    let secs = t.elapsed().as_secs_f64();
    println!("caricato: stato {} {} — {:.0} MB in {:.0} s ({:.1} MB/s)", s.state, s.error, s.bytes as f64 / 1e6, secs, s.bytes as f64 / 1e6 / secs);
    // 2. riscaricarlo
    let t = std::time::Instant::now();
    std::fs::create_dir_all(format!("{local}/ritorno")).unwrap();
    let id = jobs::copy(vec![r("/albero")], &format!("{local}/ritorno"));
    let s = wait(id);
    let secs = t.elapsed().as_secs_f64();
    println!("scaricato: stato {} {} — {:.0} MB in {:.0} s ({:.1} MB/s)", s.state, s.error, s.bytes as f64 / 1e6, secs, s.bytes as f64 / 1e6 / secs);
    // 3. conflitto: ricaricare un file che c'è già → domanda, «tieni entrambi»
    let id = jobs::copy(vec![format!("{local}/albero/uno.txt")], &r("/albero"));
    let s = wait(id);
    println!("conflitto: domanda={} su {}", s.state == jobs::ST_ASK_CONFLICT, s.ask_dst.rsplit('/').next().unwrap_or(""));
    jobs::resolve(id, jobs::ANS_KEEP_BOTH, false);
    println!("  tieni entrambi: stato {} ; «uno (1).txt» sul server: {}", wait(id).state, rootree_core::vfs::exists(std::path::Path::new(&r("/albero/uno (1).txt"))));
    // 4. rinomina e spostamento sul server, poi dal server al telefono
    println!("rinomina remota: {:?}", rootree_core::remote::rename_entry("p", &format!("{rdir}/albero/uno (1).txt"), "rinominato.txt").map(|_| "ok"));
    let id = jobs::move_to(vec![r("/albero/rinominato.txt")], &r(""));
    println!("sposta sul server (rename): stato {} ; c'è in cima: {}", wait(id).state, rootree_core::vfs::exists(std::path::Path::new(&r("/rinominato.txt"))));
    let id = jobs::move_to(vec![r("/rinominato.txt")], &local);
    println!("sposta server→telefono: stato {} ; sul telefono: {} ; sparito dal server: {}", wait(id).state,
        std::path::Path::new(&format!("{local}/rinominato.txt")).exists(), !rootree_core::vfs::exists(std::path::Path::new(&r("/rinominato.txt"))));
    // 5. Annulla a metà di un caricamento grande: niente .part sul server
    let id = jobs::copy(vec![format!("{local}/albero/grande.bin")], &r(""));
    std::thread::sleep(std::time::Duration::from_secs(3));
    jobs::cancel(id);
    let s = wait(id);
    let resti: Vec<String> = rootree_core::vfs::read_dir(std::path::Path::new(&r(""))).unwrap().iter().map(|p| p.file_name().unwrap().to_string_lossy().into_owned()).collect();
    println!("annullato: stato {} ; in cima al server: {:?}", s.state, resti);
    // 6. eliminare tutto sul server
    let id = jobs::delete(vec![r("")]);
    println!("eliminato: stato {} ; cartella remota c'è ancora: {}", wait(id).state, rootree_core::vfs::exists(std::path::Path::new(&r(""))));
    rootree_core::remote::disconnect("p");
}
