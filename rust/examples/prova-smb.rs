//! Prova a mano di SMB (non va nel pacchetto), col motore vero.
//! Argomenti: host porta utente condivisione cartella-remota cartella-locale [cifrata] [ospite].
//! La password arriva da stdin. Scrive SOLO nella cartella remota indicata
//! (dentro la condivisione), che alla fine cancella, e nella cartella locale.
use std::io::{Read, Write};
use rootree_core::{jobs, remote, vault};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let mut pw = String::new();
    std::io::stdin().read_to_string(&mut pw).unwrap();
    let (host, port, user, share, d, local) = (&a[1], a[2].parse::<u16>().unwrap(), &a[3], &a[4], a[5].clone(), a[6].clone());
    let cifrata = a.get(7).cloned().unwrap_or_default();
    let ospite = a.get(8).cloned().unwrap_or_default();
    std::fs::create_dir_all(&local).unwrap();
    vault::set_path(&format!("{local}/cassaforte.json"));
    let profile = |id: &str, auth: &str, sh: &str| serde_json::json!({"id": id, "type": "smb", "host": host, "port": port,
        "user": user, "auth": auth, "share": sh}).to_string();

    if std::env::var("RICOLLEGA").is_ok() {
        // il server si riavvia (o il Wi-Fi cambia) mentre si è collegati
        println!("collegato: {:?}", remote::connect(&profile("r", "password", share), pw.trim()));
        println!("prima: {:?}", remote::readdir("r", &format!("/{share}")).map(|v| v.len()).map_err(|e| e.to_string()));
        println!("ora riavvia il server (20 s)…");
        std::thread::sleep(std::time::Duration::from_secs(20));
        let t = std::time::Instant::now();
        println!("dopo: {:?} in {:?}", remote::readdir("r", &format!("/{share}")).map(|v| v.len()).map_err(|e| e.to_string()), t.elapsed());
        let mut s = String::new();
        let r = remote::open_any("r", &format!("/{share}/benvenuto.txt")).and_then(|mut x| x.read_to_string(&mut s));
        println!("lettura dopo: {:?} {:?}", r.map_err(|e| e.to_string()), s.trim());
        let r = remote::mkdir_any("r", &format!("/{share}/dopo-riavvio")).and_then(|_| remote::rmdir_any("r", &format!("/{share}/dopo-riavvio")));
        println!("scrittura dopo: {:?}", r.map_err(|e| e.to_string()));
        return;
    }
    println!("password sbagliata: {:?}", remote::connect(&profile("p", "password", share), "sbagliata").err());
    println!("condivisione assente: {:?}", remote::connect(&profile("p", "password", "NonCe"), pw.trim()).err());
    // senza condivisione: si parte dall'elenco
    let c = remote::connect(&profile("e", "password", ""), pw.trim());
    println!("senza condivisione: {c:?}");
    if c.is_ok() {
        println!("condivisioni: {:?}", remote::readdir("e", "/").map(|v| v.into_iter().map(|x| x.0).collect::<Vec<_>>()));
        remote::disconnect("e");
    }
    let t = std::time::Instant::now();
    let c = remote::connect(&profile("p", "password", share), pw.trim());
    println!("collegato in {:?}: {c:?}", t.elapsed());

    // da qui la cartella di prova dentro la condivisione: /Condivisione/<d>
    let d = format!("/{share}{d}");
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
    // sfogliare MENTRE un file viene letto
    let mut lento = remote::open_any("p", &r(f)).unwrap();
    let mut pezzo = vec![0u8; 1 << 16];
    lento.read_exact(&mut pezzo).unwrap();
    println!("elenco durante una lettura: {:?}", remote::readdir("p", &d).map(|v| v.len()).map_err(|e| e.to_string()));
    drop(lento);
    let mut w = remote::create_any("p", &r("/piccolo.txt"), 0o644, None).unwrap();
    w.write_all(b"ciao").unwrap(); w.flush().unwrap(); drop(w);
    // annullare a meta': il file non deve restare
    let mut w = remote::create_any("p", &r("/annullato.bin"), 0o644, Some(5 << 20)).unwrap();
    w.write_all(&data[..3 << 20]).unwrap(); drop(w);
    println!("annullato esiste: {:?}", remote::lstat("p", &r("/annullato.bin")).map(|_| ()).map_err(|e| e.kind()));
    remote::mkdir_any("p", &r("/sotto")).unwrap();
    remote::rename_any("p", &r("/piccolo.txt"), &r("/sotto/rinominato.txt")).unwrap();
    let mut w = remote::create_any("p", &r("/.nascosto"), 0o644, Some(2)).unwrap();
    w.write_all(b"ok").unwrap(); w.flush().unwrap(); drop(w);
    let mut names: Vec<_> = remote::readdir("p", &d).unwrap().into_iter().map(|(n, s)| (n, s.is_dir, s.size)).collect();
    names.sort();
    println!("elenco: {names:?}");
    println!("sotto: {:?}", remote::readdir("p", &r("/sotto")).unwrap().into_iter().map(|x| x.0).collect::<Vec<_>>());
    println!("manca: {:?}", remote::lstat("p", &r("/nonce")).map(|_| ()).map_err(|e| e.kind()));

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

    // condivisione con cifratura SMB3 obbligatoria
    if !cifrata.is_empty() {
        let c = remote::connect(&profile("c", "password", &cifrata), pw.trim());
        println!("cifrata: {c:?}");
        if c.is_ok() {
            let p = format!("/{cifrata}/segreto.bin");
            let mut w = remote::create_any("c", &p, 0o644, None).unwrap();
            w.write_all(&data[..4 << 20]).unwrap(); w.flush().unwrap(); drop(w);
            let mut b = Vec::new();
            remote::open_any("c", &p).unwrap().read_to_end(&mut b).unwrap();
            println!("cifrata andata e ritorno identica: {}", b == data[..4 << 20]);
            remote::rmdir_any("c", &p).or_else(|_| remote::unlink_any("c", &p)).ok();
            println!("cifrata pulita: {:?}", remote::lstat("c", &p).map(|_| ()).map_err(|e| e.kind()));
            remote::disconnect("c");
        }
    }
    // ospite, in sola lettura
    if !ospite.is_empty() {
        let g = serde_json::json!({"id": "g", "type": "smb", "host": host, "port": port, "user": "", "auth": "guest", "share": ospite}).to_string();
        let c = remote::connect(&g, "");
        println!("ospite: {c:?}");
        if c.is_ok() {
            println!("ospite elenco: {:?}", remote::readdir("g", &format!("/{ospite}")).map(|v| v.into_iter().map(|x| x.0).collect::<Vec<_>>()));
            let mut s = String::new();
            remote::open_any("g", &format!("/{ospite}/leggimi.txt")).unwrap().read_to_string(&mut s).unwrap();
            println!("ospite legge: {:?}", s.trim());
            println!("ospite scrive: {:?}", remote::mkdir_any("g", &format!("/{ospite}/vietato")).map_err(|e| e.kind()));
        }
    }
}
