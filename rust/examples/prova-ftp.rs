//! Prova a mano di FTP/FTPS (non va nel pacchetto), col motore vero.
//! Argomenti: host porta utente cifrata(0/1) cartella-remota cartella-locale.
//! La password arriva da stdin. Scrive SOLO nella cartella remota indicata,
//! che alla fine cancella, e nella cartella locale (anche il file dei
//! certificati fidati, al posto della configurazione dell'app).
use std::io::{Read, Write};
use rootree_core::{jobs, remote, vault};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let mut pw = String::new();
    std::io::stdin().read_to_string(&mut pw).unwrap();
    let (host, port, user, secure, d, local) = (&a[1], a[2].parse::<u16>().unwrap(), &a[3], a[4] == "1", a[5].clone(), a[6].clone());
    std::fs::create_dir_all(&local).unwrap();
    vault::set_path(&format!("{local}/cassaforte.json"));
    let profile = |auth: &str, path: &str| serde_json::json!({"id": "p", "type": "ftp", "host": host, "port": port,
        "user": user, "secure": secure, "auth": auth, "path": path}).to_string();

    if secure {
        // la prima volta: il certificato fatto in casa va confermato
        println!("primo collegamento: {:?}", remote::connect(&profile("password", ""), pw.trim()).err());
        println!("fiducia: {:?}", remote::trust("p"));
    }
    println!("password sbagliata: {:?}", remote::connect(&profile("password", ""), "sbagliata").err());
    let t = std::time::Instant::now();
    let c = remote::connect(&profile("password", ""), pw.trim());
    println!("collegato in {:?}: {c:?}", t.elapsed());

    let r = |s: &str| format!("{d}{s}");
    remote::mkdir_any("p", &d).unwrap();
    println!("mkdir doppia: {:?}", remote::mkdir_any("p", &d).map_err(|e| e.kind()));
    let n: usize = 30 << 20;
    let data: Vec<u8> = (0..n).map(|i| ((i * 31) ^ (i >> 8)) as u8).collect();
    let f = "/Città è [prova] #1 %.bin";
    let t = std::time::Instant::now();
    let mut w = remote::create_any("p", &r(f), 0o644, Some(n as u64)).unwrap();
    for c in data.chunks(1 << 18) { w.write_all(c).unwrap(); }
    w.flush().unwrap(); drop(w);
    println!("su {:?}", t.elapsed());
    let st = remote::lstat("p", &r(f)).unwrap();
    println!("dimensione {} ok={} mtime>0={}", st.size, st.size as usize == n, st.mtime > 0);
    let t = std::time::Instant::now();
    let mut back = Vec::new();
    remote::open_any("p", &r(f)).unwrap().read_to_end(&mut back).unwrap();
    println!("giu' {:?}, identico={}", t.elapsed(), back == data);
    if std::env::var("RICOLLEGA").is_ok() {
        // il server chiude i collegamenti fermi (TimeoutIdle 5 sul server di prova)
        std::thread::sleep(std::time::Duration::from_secs(8));
        println!("dopo l'attesa, elenco: {:?}", remote::readdir("p", &d).map(|v| v.len()).map_err(|e| e.to_string()));
        let mut b2 = Vec::new();
        let r2 = remote::open_any("p", &r(f)).and_then(|mut x| x.read_to_end(&mut b2));
        println!("dopo l'attesa, lettura: {:?} identico={}", r2.map_err(|e| e.to_string()), b2 == data);
    }
    // sfogliare MENTRE un file viene letto (collegamenti separati)
    let mut lento = remote::open_any("p", &r(f)).unwrap();
    let mut pezzo = vec![0u8; 1 << 16];
    lento.read_exact(&mut pezzo).unwrap();
    println!("elenco durante una lettura: {:?}", remote::readdir("p", &d).map(|v| v.len()).map_err(|e| e.to_string()));
    drop(lento);
    // dimensione ignota e sovrascrittura
    let mut w = remote::create_any("p", &r("/piccolo.txt"), 0o644, None).unwrap();
    w.write_all(b"ciao").unwrap(); w.flush().unwrap(); drop(w);
    // annullare a meta': il file non deve restare
    let mut w = remote::create_any("p", &r("/annullato.bin"), 0o644, Some(5 << 20)).unwrap();
    w.write_all(&data[..1 << 20]).unwrap(); drop(w);
    println!("annullato esiste: {:?}", remote::lstat("p", &r("/annullato.bin")).map(|_| ()).map_err(|e| e.kind()));
    remote::mkdir_any("p", &r("/sotto")).unwrap();
    remote::rename_any("p", &r("/piccolo.txt"), &r("/sotto/rinominato.txt")).unwrap();
    // file nascosto
    let mut w = remote::create_any("p", &r("/.nascosto"), 0o644, Some(2)).unwrap();
    w.write_all(b"ok").unwrap(); w.flush().unwrap(); drop(w);
    let mut names: Vec<_> = remote::readdir("p", &d).unwrap().into_iter().map(|(n, s)| (n, s.is_dir, s.size)).collect();
    names.sort();
    println!("elenco: {names:?}");
    println!("sotto: {:?}", remote::readdir("p", &r("/sotto")).unwrap().into_iter().map(|x| x.0).collect::<Vec<_>>());
    println!("manca: {:?}", remote::lstat("p", &r("/nonce")).map(|_| ()).map_err(|e| e.kind()));

    // copia ed eliminazione di una cartella col motore vero
    let wait = |id: u64| loop {
        if let Some(s) = jobs::statuses().into_iter().find(|j| j.id == id)
            && (s.state >= jobs::ST_DONE || s.state == jobs::ST_ASK_CONFLICT || s.state == jobs::ST_ASK_ERROR)
        { break (s.state, s.error.clone()); }
        std::thread::sleep(std::time::Duration::from_millis(50));
    };
    let _ = std::fs::remove_dir_all(format!("{local}/albero"));
    let _ = std::fs::remove_dir_all(format!("{local}/ritorno"));
    for i in 0..3 { let sd = format!("{local}/albero/s{i}"); std::fs::create_dir_all(&sd).unwrap();
        for j in 0..10 { std::fs::write(format!("{sd}/f{j}.txt"), format!("contenuto {i} {j}")).unwrap(); } }
    let t = std::time::Instant::now();
    println!("copia: {:?} in {:?}", wait(jobs::copy(vec![format!("{local}/albero")], &format!("/@remote/p{d}"))), t.elapsed());
    let ok = (0..3).all(|i| remote::readdir("p", &r(&format!("/albero/s{i}"))).map(|v| v.len() == 10).unwrap_or(false));
    println!("30 file arrivati: {ok}");
    std::fs::create_dir_all(format!("{local}/ritorno")).unwrap();
    let t = std::time::Instant::now();
    println!("ritorno: {:?} in {:?}", wait(jobs::copy(vec![format!("/@remote/p{d}/albero")], &format!("{local}/ritorno"))), t.elapsed());
    let ok = std::fs::read_to_string(format!("{local}/ritorno/albero/s2/f7.txt")).unwrap_or_default() == "contenuto 2 7";
    println!("contenuto riportato: {ok}");
    let t = std::time::Instant::now();
    println!("eliminazione: {:?} in {:?}", wait(jobs::delete(vec![format!("/@remote/p{d}/albero")])), t.elapsed());
    println!("albero dopo: {:?}", remote::lstat("p", &r("/albero")).map(|_| ()).map_err(|e| e.kind()));
    println!("eliminazione cartella piena: {:?}", remote::rmdir_any("p", &d).map_err(|e| e.to_string()));
    println!("dopo la pulizia: {:?}", remote::lstat("p", &d).map(|_| ()).map_err(|e| e.kind()));
    remote::disconnect("p");

    // accesso anonimo, in sola lettura
    let anon = serde_json::json!({"id": "a", "type": "ftp", "host": host, "port": port, "user": "", "secure": secure,
        "auth": "anonymous", "path": ""}).to_string();
    let c = remote::connect(&anon, "");
    if secure && matches!(&c, Err(e) if e.starts_with("HOSTKEY")) {
        println!("anonimo: certificato già fidato? NO: {c:?}");
    }
    println!("anonimo: {c:?}");
    if c.is_ok() {
        println!("anonimo elenco /pub: {:?}", remote::readdir("a", "/pub").map(|v| v.into_iter().map(|x| x.0).collect::<Vec<_>>()));
        let mut s = String::new();
        remote::open_any("a", "/pub/leggimi.txt").unwrap().read_to_string(&mut s).unwrap();
        println!("anonimo legge: {:?}", s.trim());
        println!("anonimo scrive: {:?}", remote::mkdir_any("a", "/pub/vietato").map_err(|e| e.kind()));
    }
}
