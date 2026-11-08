//! Prova a mano dei link simbolici verso e da un server (non va nel pacchetto).
//! Argomenti: host porta utente chiave cartella-locale cartella-remota.
use std::path::Path;
use rootree_core::jobs;

fn wait(id: u64) -> u8 {
    loop {
        if let Some(s) = jobs::statuses().into_iter().find(|j| j.id == id)
            && s.state >= jobs::ST_DONE
        {
            if !s.error.is_empty() { println!("  errore: {}", s.error); }
            return s.state;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let (local, rdir) = (a[5].clone(), a[6].clone());
    let profile = serde_json::json!({"id": "p", "type": "sftp", "host": a[1], "port": a[2].parse::<u16>().unwrap(),
        "user": a[3], "auth": "key", "keyPath": a[4], "path": ""}).to_string();
    rootree_core::remote::connect(&profile, "").expect("collegamento");
    let r = |p: &str| format!("/@remote/p{rdir}{p}");
    let parent = Path::new(&rdir).parent().unwrap().to_str().unwrap().to_string();
    let name = Path::new(&rdir).file_name().unwrap().to_str().unwrap().to_string();
    println!("cartella remota: {}", rootree_core::remote::make_dir("p", &parent, &name).is_ok());
    println!("carica: stato {}", wait(jobs::copy(vec![format!("{local}/conlink")], &r(""))));
    println!("link sul server: {:?}", rootree_core::vfs::read_link(Path::new(&r("/conlink/link-a-uno"))));
    std::fs::create_dir_all(format!("{local}/ritorno")).unwrap();
    println!("scarica: stato {}", wait(jobs::copy(vec![r("/conlink")], &format!("{local}/ritorno"))));
    println!("link tornato: {:?}", std::fs::read_link(format!("{local}/ritorno/conlink/link-a-uno")));
    println!("elimina: stato {}", wait(jobs::delete(vec![r("")])));
    rootree_core::remote::disconnect("p");
}
