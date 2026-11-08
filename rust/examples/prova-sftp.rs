//! Prova a mano del collegamento SFTP sul telefono (non va nel pacchetto).
//! Argomenti: host porta utente chiave [frase da stdin se serve]. Solo lettura.
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let profile = serde_json::json!({
        "id": "prova", "type": "sftp", "host": a[1], "port": a[2].parse::<u16>().unwrap(),
        "user": a[3], "auth": "key", "keyPath": a[4], "path": ""
    })
    .to_string();
    let t = std::time::Instant::now();
    match rootree_core::remote::connect(&profile, "") {
        Ok(start) => {
            println!("collegato in {} ms, cartella iniziale di {} caratteri", t.elapsed().as_millis(), start.len());
            match rootree_core::remote::list("prova", &start, false) {
                Ok(v) => println!("elenco: {} voci ({} cartelle)", v.len(), v.iter().filter(|e| e.kind == 1).count()),
                Err(e) => println!("elenco: errore {e}"),
            }
            if let Ok(extra) = std::env::var("PROVA_CARTELLA") {
                println!("{extra}: {}", rootree_core::remote::list("prova", &extra, false).map(|v| format!("{} voci ({} cartelle)", v.len(), v.iter().filter(|e| e.kind == 1).count())).unwrap_or_else(|e| e.to_string()));
            }
            let root = rootree_core::remote::list("prova", "/", true);
            println!("radice: {}", root.map(|v| format!("{} voci", v.len())).unwrap_or_else(|e| e.to_string()));
            println!("inesistente: {}", rootree_core::remote::list("prova", "/non/esiste/davvero", false).err().map(|e| e.to_string()).unwrap_or_default());
            rootree_core::remote::disconnect("prova");
            println!("scollegato: {}", !rootree_core::remote::connected("prova"));
        }
        Err(e) if e.starts_with("HOSTKEY_NEW:") && std::env::var("PROVA_FIDATI").is_ok() => {
            println!("server nuovo: {}", e.split(':').nth(1).unwrap_or(""));
            println!("fidati: {:?}", rootree_core::remote::trust("prova"));
            println!("di nuovo: {}", rootree_core::remote::connect(&profile, "").map(|_| "collegato".to_string()).unwrap_or_else(|e| e));
        }
        Err(e) => println!("collegamento: {}", e.split(':').take(2).collect::<Vec<_>>().join(":")),
    }
}
