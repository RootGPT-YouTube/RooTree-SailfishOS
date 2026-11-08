//! L'helper della modalità root di RooTree. Lo avvia l'app con
//! `devel-su rootree-helper --fd 3` (la password la scrive l'utente) e gli
//! parla solo sul socket privato che riceve come fd 3, una riga JSON per
//! richiesta. Fa soltanto operazioni sul filesystem; quando l'app chiude il
//! socket (modalità spenta, app chiusa o morta) annulla i lavori aperti ed esce.

use std::os::fd::FromRawFd;
use std::os::unix::net::UnixStream;

use rootree_core::{fsops, helper};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let Some(fd) = args.iter().position(|a| a == "--fd").and_then(|i| args.get(i + 1)).and_then(|s| s.parse::<i32>().ok()) else {
        eprintln!("uso: rootree-helper --fd N (lo avvia RooTree, non va lanciato a mano)");
        std::process::exit(2);
    };
    if unsafe { libc::geteuid() } != 0 {
        eprintln!("rootree-helper: serve root (devel-su)");
        std::process::exit(1);
    }
    // deve essere un socket: niente scritture a caso su un fd qualunque
    let mut st: libc::stat = unsafe { std::mem::zeroed() };
    if unsafe { libc::fstat(fd, &mut st) } != 0 || st.st_mode & libc::S_IFMT != libc::S_IFSOCK {
        eprintln!("rootree-helper: fd {fd} non è un socket");
        std::process::exit(2);
    }
    unsafe {
        // il terminale di devel-su non lo legge nessuno: niente uscita lì
        // (riempito, bloccherebbe l'helper)
        let null = libc::open(c"/dev/null".as_ptr(), libc::O_RDWR);
        if null >= 0 {
            for t in 0..3 {
                libc::dup2(null, t);
            }
        }
        // l'app che chiude il terminale manda SIGHUP: si esce dalla fine del
        // socket, dopo aver annullato i lavori (niente .rootree-part lasciati)
        libc::signal(libc::SIGHUP, libc::SIG_IGN);
        libc::umask(0o022);
    }
    fsops::INHERIT_OWNER.store(true, std::sync::atomic::Ordering::Relaxed);

    helper::serve(unsafe { UnixStream::from_raw_fd(fd) });
}
