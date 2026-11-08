//! Prova a mano della modalità root sul telefono (non va nel pacchetto):
//! la password di sviluppatore da stdin, poi elenco di /root, salvataggio di
//! un file di prova con copia di sicurezza, e un lavoro di copia da root.
use std::io::BufRead;

use rootree_core::proto::Request;
use rootree_core::root;

fn main() {
    let mut pw = String::new();
    std::io::stdin().lock().read_line(&mut pw).unwrap();
    let t = std::time::Instant::now();
    match root::start(pw.trim_end()) {
        Ok(()) => println!("avvio: ok in {} ms", t.elapsed().as_millis()),
        Err(e) => {
            println!("avvio: {e}");
            return;
        }
    }
    let v: Vec<serde_json::Value> = root::call_as(&Request::ListDir { path: "/root".into(), hidden: true, key: 0, desc: false, dirs_first: true }).unwrap();
    println!("/root: {} voci", v.len());
    let f = std::env::args().nth(1).unwrap_or("/etc/rootree-prova.conf".into());
    let copia: String = root::call_as(&Request::WriteText { path: f.clone(), text: "modificato da RooTree\n".into() }).unwrap();
    println!("salvato {f}, copia di sicurezza: {copia}");
    let e: serde_json::Value = root::call_as(&Request::Stat { path: f.clone() }).unwrap();
    println!("dopo: uid={} mode={:o}", e["uid"], e["mode"].as_u64().unwrap() & 0o7777);
    root::stop();
    std::thread::sleep(std::time::Duration::from_millis(500));
    println!("attiva dopo stop: {}", root::active());
}
