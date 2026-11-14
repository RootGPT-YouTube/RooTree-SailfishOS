//! Prova a mano di S3 (non va nel pacchetto), col motore vero.
//! Argomenti: endpoint access-key-id bucket cartella-locale [regione].
//! La secret key arriva da stdin. Scrive SOLO sotto «rootree-prova/» nel
//! bucket indicato (che alla fine cancella), in un bucket nuovo
//! «rootree-bucket-nuovo» (creato e tolto) e nella cartella locale.
use std::io::{Read, Write};
use rootree_core::{jobs, remote, vault};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let mut pw = String::new();
    std::io::stdin().read_to_string(&mut pw).unwrap();
    let pw = pw.trim().to_string();
    let (ep, key, bucket, local) = (&a[1], &a[2], &a[3], a[4].clone());
    let region = a.get(5).cloned().unwrap_or_default();
    std::fs::create_dir_all(&local).unwrap();
    vault::set_path(&format!("{local}/cassaforte.json"));
    let profile = |id: &str, k: &str, b: &str, r: &str| serde_json::json!({"id": id, "type": "s3", "host": ep, "port": 0,
        "user": k, "auth": "password", "bucket": b, "region": r}).to_string();

    println!("secret sbagliata: {:?}", remote::connect(&profile("x", key, bucket, &region), "sbagliata").err());
    println!("access key sconosciuta: {:?}", remote::connect(&profile("x", "GKnonesiste000000000000", bucket, &region), &pw).err());
    println!("bucket assente: {:?}", remote::connect(&profile("x", key, "non-esiste-davvero", &region), &pw).err());
    if region.is_empty() {
        println!("(regione vuota: deve trovarla da sola)");
    }

    // senza bucket: la radice è l'elenco dei bucket
    let c = remote::connect(&profile("t", key, "", &region), &pw);
    println!("tutti i bucket: {c:?}");
    if c.is_ok() {
        let mut v: Vec<String> = remote::readdir("t", "/").unwrap_or_default().into_iter().map(|x| x.0).collect();
        v.sort();
        println!("bucket: {v:?}");
        println!("crea bucket: {:?}", remote::mkdir_any("t", "/rootree-bucket-nuovo").map_err(|e| e.to_string()));
        let mut w = remote::create_any("t", "/rootree-bucket-nuovo/dentro.txt", 0o644, Some(5)).unwrap();
        w.write_all(b"nuovo").unwrap(); w.flush().unwrap(); drop(w);
        println!("nel bucket nuovo: {:?}", remote::readdir("t", "/rootree-bucket-nuovo").map(|v| v.into_iter().map(|x| x.0).collect::<Vec<_>>()));
        println!("stat bucket: {:?}", remote::lstat("t", "/rootree-bucket-nuovo").map(|s| s.is_dir).map_err(|e| e.to_string()));
        println!("elimina bucket pieno: {:?}", remote::rmdir_any("t", "/rootree-bucket-nuovo").map_err(|e| e.to_string()));
        println!("bucket dopo: {:?}", remote::lstat("t", "/rootree-bucket-nuovo").map(|_| ()).map_err(|e| e.kind()));
        remote::disconnect("t");
    }

    let t = std::time::Instant::now();
    let c = remote::connect(&profile("p", key, bucket, &region), &pw);
    println!("collegato in {:?}: {c:?}", t.elapsed());
    let d = "/rootree-prova".to_string();
    let r = |s: &str| format!("{d}{s}");
    let _ = remote::rmdir_any("p", &d);
    remote::mkdir_any("p", &d).unwrap();
    println!("cartella: {:?}", remote::lstat("p", &d).map(|s| s.is_dir).map_err(|e| e.to_string()));
    println!("mkdir doppia: {:?}", remote::mkdir_any("p", &d).map_err(|e| e.kind()));
    let n: usize = 30 << 20;
    let data: Vec<u8> = (0..n).map(|i| ((i * 31) ^ (i >> 8)) as u8).collect();
    let f = "/Città è [prova] #1 %+=&.bin";
    let t = std::time::Instant::now();
    let mut w = remote::create_any("p", &r(f), 0o644, Some(n as u64)).unwrap();
    for c in data.chunks(1 << 18) { w.write_all(c).unwrap(); }
    w.flush().unwrap(); drop(w);
    println!("su 30 MB {:?}", t.elapsed());
    let st = remote::lstat("p", &r(f)).unwrap();
    println!("dimensione ok={} file={} mtime>0={}", st.size as usize == n, st.is_file, st.mtime > 0);
    let t = std::time::Instant::now();
    let mut back = Vec::new();
    remote::open_any("p", &r(f)).unwrap().read_to_end(&mut back).unwrap();
    println!("giu' {:?}, identico={}", t.elapsed(), back == data);
    let mut lento = remote::open_any("p", &r(f)).unwrap();
    let mut pezzo = vec![0u8; 1 << 16];
    lento.read_exact(&mut pezzo).unwrap();
    println!("elenco durante una lettura: {:?}", remote::readdir("p", &d).map(|v| v.len()).map_err(|e| e.to_string()));
    drop(lento);

    // a pezzi: soglia abbassata a 1 MiB, 40 MB = 3 pezzi da 16 MiB
    unsafe { std::env::set_var("ROOTREE_S3_SINGLE_MAX", "1048576") };
    let m: usize = 40 << 20;
    let big: Vec<u8> = (0..m).map(|i| ((i * 7) ^ (i >> 11)) as u8).collect();
    let t = std::time::Instant::now();
    let mut w = remote::create_any("p", &r("/a-pezzi.bin"), 0o644, Some(m as u64)).unwrap();
    for c in big.chunks(1 << 18) { w.write_all(c).unwrap(); }
    w.flush().unwrap(); drop(w);
    let mut back = Vec::new();
    remote::open_any("p", &r("/a-pezzi.bin")).unwrap().read_to_end(&mut back).unwrap();
    println!("a pezzi 40 MB in {:?}, identico={}", t.elapsed(), back == big);
    // annullato a metà del caricamento a pezzi: niente oggetto
    let mut w = remote::create_any("p", &r("/a-pezzi-annullato.bin"), 0o644, Some(m as u64)).unwrap();
    for c in big[..20 << 20].chunks(1 << 18) { w.write_all(c).unwrap(); }
    drop(w);
    println!("a pezzi annullato esiste: {:?}", remote::lstat("p", &r("/a-pezzi-annullato.bin")).map(|_| ()).map_err(|e| e.kind()));
    unsafe { std::env::remove_var("ROOTREE_S3_SINGLE_MAX") };

    let mut w = remote::create_any("p", &r("/piccolo.txt"), 0o644, None).unwrap();
    w.write_all(b"ciao").unwrap(); w.flush().unwrap(); drop(w);
    // annullare a metà la PUT a flusso
    let mut w = remote::create_any("p", &r("/annullato.bin"), 0o644, Some(5 << 20)).unwrap();
    w.write_all(&data[..3 << 20]).unwrap(); drop(w);
    std::thread::sleep(std::time::Duration::from_millis(500));
    println!("annullato esiste: {:?}", remote::lstat("p", &r("/annullato.bin")).map(|_| ()).map_err(|e| e.kind()));
    remote::mkdir_any("p", &r("/sotto")).unwrap();
    remote::rename_any("p", &r("/piccolo.txt"), &r("/sotto/rinominato.txt")).unwrap();
    let mut s = String::new();
    remote::open_any("p", &r("/sotto/rinominato.txt")).unwrap().read_to_string(&mut s).unwrap();
    println!("rinominato legge: {s:?}; vecchio: {:?}", remote::lstat("p", &r("/piccolo.txt")).map(|_| ()).map_err(|e| e.kind()));
    remote::rename_any("p", &r("/sotto"), &r("/sopra")).unwrap();
    println!("cartella rinominata: {:?}; vecchia: {:?}", remote::readdir("p", &r("/sopra")).map(|v| v.into_iter().map(|x| x.0).collect::<Vec<_>>()),
        remote::lstat("p", &r("/sotto")).map(|_| ()).map_err(|e| e.kind()));
    let mut w = remote::create_any("p", &r("/.nascosto"), 0o644, Some(2)).unwrap();
    w.write_all(b"ok").unwrap(); w.flush().unwrap(); drop(w);
    let mut names: Vec<_> = remote::readdir("p", &d).unwrap().into_iter().map(|(n, s)| (n, s.is_dir, s.size)).collect();
    names.sort();
    println!("elenco: {names:?}");
    println!("manca: {:?}", remote::lstat("p", &r("/nonce")).map(|_| ()).map_err(|e| e.kind()));
    println!("file vuoto: {:?}", remote::make_file("p", &d, "vuoto.txt").map_err(|e| e.to_string())
        .and_then(|_| remote::lstat("p", &r("/vuoto.txt")).map(|s| (s.is_file, s.size)).map_err(|e| e.to_string())));

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
    std::fs::create_dir_all(format!("{local}/albero/vuota")).unwrap();
    let t = std::time::Instant::now();
    println!("copia: {:?} in {:?}", wait(jobs::copy(vec![format!("{local}/albero")], &format!("/@remote/p{d}"))), t.elapsed());
    let ok = (0..3).all(|i| remote::readdir("p", &r(&format!("/albero/s{i}"))).map(|v| v.len() == 10).unwrap_or(false));
    println!("30 file arrivati: {ok}; cartella vuota: {:?}", remote::lstat("p", &r("/albero/vuota")).map(|s| s.is_dir).map_err(|e| e.kind()));
    std::fs::create_dir_all(format!("{local}/ritorno")).unwrap();
    let t = std::time::Instant::now();
    println!("ritorno: {:?} in {:?}", wait(jobs::copy(vec![format!("/@remote/p{d}/albero")], &format!("{local}/ritorno"))), t.elapsed());
    let ok = std::fs::read_to_string(format!("{local}/ritorno/albero/s2/f7.txt")).unwrap_or_default() == "contenuto 2 7";
    println!("contenuto riportato: {ok}; vuota tornata: {}", std::path::Path::new(&format!("{local}/ritorno/albero/vuota")).is_dir());
    let t = std::time::Instant::now();
    println!("spostamento sul server: {:?} in {:?}", wait(jobs::move_to(vec![format!("/@remote/p{d}/albero/s0")], &format!("/@remote/p{d}/sopra"))), t.elapsed());
    println!("s0 spostata: {:?}", remote::readdir("p", &r("/sopra/s0")).map(|v| v.len()).map_err(|e| e.kind()));
    // nel cestino di RooTree sul bucket, poi ripristino, poi definitivo
    let t = std::time::Instant::now();
    println!("nel cestino: {:?} in {:?}", wait(jobs::trash(vec![format!("/@remote/p{d}/albero")])), t.elapsed());
    println!("albero dopo il cestino: {:?}", remote::lstat("p", &r("/albero")).map(|_| ()).map_err(|e| e.kind()));
    let voci = remote::trash("p", "list", "");
    println!("cestino: {voci:?}");
    if let Ok(j) = &voci
        && let Some(item) = serde_json::from_str::<serde_json::Value>(j).ok().and_then(|v| v.as_array().and_then(|a| a.first().cloned()))
    {
        let id = item["id"].as_str().or_else(|| item["path"].as_str()).unwrap_or("").to_string();
        println!("ripristino: {:?}", remote::trash("p", "restore", &id));
        println!("albero ripristinato: {:?}", remote::readdir("p", &r("/albero/s1")).map(|v| v.len()).map_err(|e| e.kind()));
    }
    let t = std::time::Instant::now();
    println!("eliminazione: {:?} in {:?}", wait(jobs::delete(vec![format!("/@remote/p{d}/albero")])), t.elapsed());
    println!("albero dopo: {:?}", remote::lstat("p", &r("/albero")).map(|_| ()).map_err(|e| e.kind()));
    let mut cestino: Vec<String> = remote::readdir("p", "/").unwrap_or_default().into_iter().map(|x| x.0).collect();
    cestino.sort();
    println!("radice del bucket: {cestino:?}");
    println!("eliminazione cartella piena: {:?}", remote::rmdir_any("p", &d).map_err(|e| e.to_string()));
    let _ = remote::rmdir_any("p", "/.Trash-rootree");
    println!("dopo la pulizia: {:?}", remote::lstat("p", &d).map(|_| ()).map_err(|e| e.kind()));
    remote::disconnect("p");
}
