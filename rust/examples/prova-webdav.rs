//! Prova a mano di WebDAV (non va nel pacchetto), col motore vero.
//! Argomenti: tipo host porta utente https(0/1) cartella-remota. La password arriva da stdin.
//! Scrive SOLO nella cartella remota indicata, che alla fine cancella.
use std::io::{Read, Write};
use rootree_core::{jobs, remote};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let mut pw = String::new();
    std::io::stdin().read_to_string(&mut pw).unwrap();
    let profile = serde_json::json!({"id": "p", "type": a[1], "host": a[2], "port": a[3].parse::<u16>().unwrap(),
        "user": a[4], "secure": a[5] == "1", "auth": "password", "path": ""}).to_string();
    let t = std::time::Instant::now();
    println!("password sbagliata: {:?}", remote::connect(&profile, "sbagliata").err());
    println!("collegato in {:?}: {:?}", t.elapsed(), remote::connect(&profile, pw.trim()));
    let d = a[6].clone();
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
    // dimensione ignota e sovrascrittura
    let mut w = remote::create_any("p", &r("/piccolo.txt"), 0o644, None).unwrap();
    w.write_all(b"ciao").unwrap(); w.flush().unwrap(); drop(w);
    // annullare a meta': il file non deve nascere
    let mut w = remote::create_any("p", &r("/annullato.bin"), 0o644, Some(5 << 20)).unwrap();
    w.write_all(&data[..1 << 20]).unwrap(); drop(w);
    std::thread::sleep(std::time::Duration::from_secs(3));
    println!("annullato esiste: {:?}", remote::lstat("p", &r("/annullato.bin")).map(|_| ()).map_err(|e| e.kind()));
    remote::mkdir_any("p", &r("/sotto")).unwrap();
    remote::rename_any("p", &r("/piccolo.txt"), &r("/sotto/rinominato.txt")).unwrap();
    let mut names: Vec<_> = remote::readdir("p", &d).unwrap().into_iter().map(|(n, s)| (n, s.is_dir, s.size)).collect();
    names.sort();
    println!("elenco: {names:?}");
    println!("sotto: {:?}", remote::readdir("p", &r("/sotto")).unwrap().into_iter().map(|x| x.0).collect::<Vec<_>>());
    println!("manca: {:?}", remote::lstat("p", &r("/nonce")).map(|_| ()).map_err(|e| e.kind()));
    // copia ed eliminazione di una cartella col motore vero
    let local = a[7].clone();
    let wait = |id: u64| loop {
        if let Some(s) = jobs::statuses().into_iter().find(|j| j.id == id)
            && (s.state >= jobs::ST_DONE || s.state == jobs::ST_ASK_CONFLICT || s.state == jobs::ST_ASK_ERROR)
        { break (s.state, s.error.clone()); }
        std::thread::sleep(std::time::Duration::from_millis(50));
    };
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
    let items = |r: &str| serde_json::from_str::<serde_json::Value>(r).unwrap();
    let cest = items(&remote::trash("p", "list", "").unwrap());
    println!("cestino: {}", cest.as_array().map(|v| v.len()).unwrap_or(0));
    let voce = cest.as_array().unwrap().iter().find(|v| v["name"] == "albero").expect("albero nel cestino").clone();
    println!("voce: {voce}");
    println!("ripristino: {:?}", remote::trash("p", "restore", voce["id"].as_str().unwrap()));
    println!("ripristinato in {:?}: {:?}", voce["original"], remote::lstat("p", &format!("/{}", voce["original"].as_str().unwrap())).map(|s| s.is_dir).map_err(|e| e.kind()));
    println!("del definitivo: {:?}", remote::rmdir_any("p", &format!("/{}", voce["original"].as_str().unwrap())).map_err(|e| e.kind()));
    let cest = items(&remote::trash("p", "list", "").unwrap());
    let u = cest.as_array().unwrap().iter().find(|v| v["name"] == "albero").unwrap().clone();
    println!("elimina definitiva: {:?}", remote::trash("p", "delete", u["id"].as_str().unwrap()));
    println!("restano: {}", items(&remote::trash("p", "list", "").unwrap()).as_array().unwrap().len());
    println!("svuota: {:?}", remote::trash("p", "empty", ""));
    println!("dopo svuota: {}", items(&remote::trash("p", "list", "").unwrap()).as_array().unwrap().len());
    println!("albero dopo: {:?}", remote::lstat("p", &r("/albero")).map(|_| ()).map_err(|e| e.kind()));
    remote::rmdir_any("p", &d).unwrap();
    println!("dopo la pulizia: {:?}", remote::lstat("p", &d).map(|_| ()).map_err(|e| e.kind()));
}
