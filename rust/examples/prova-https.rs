//! Prova a mano: HTTPS dal telefono con l'OpenSSL di sistema (non va nel pacchetto).
fn main() {
    for url in ["https://api.dropboxapi.com/2/users/get_current_account"] {
        let r = rootree_core::http::agent().post(url).header("Content-Type", "application/json").send("null");
        match r {
            Ok(mut resp) => {
                let st = resp.status().as_u16();
                let body = resp.body_mut().read_to_string().unwrap_or_default();
                println!("{url}: HTTPS ok, stato {st} ({}…)", body.chars().take(60).collect::<String>().replace('\n', " "));
            }
            Err(e) => println!("{url}: ERRORE {e}"),
        }
    }
}
