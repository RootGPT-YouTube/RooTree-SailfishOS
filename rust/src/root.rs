//! La modalità root, dal lato dell'app: avvia l'helper con `devel-su` (la
//! password di sviluppatore si scrive nel suo terminale, che l'app apre con
//! forkpty e non salva da nessuna parte), poi gli inoltra le richieste.
//!
//! L'interfaccia non gira mai da root. L'helper parla solo sul socket privato
//! ereditato come fd 3, e se ne va da solo quando l'app chiude il socket
//! (modalità spenta, app chiusa o morta).

use std::ffi::CString;
use std::io::{BufRead, BufReader, Write};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use crate::proto::{HELLO, Request, Response};

pub const HELPER: &str = "/usr/libexec/harbour-rootree/rootree-helper";
const DEVEL_SU: &str = "/usr/bin/devel-su";

struct Conn {
    reader: BufReader<UnixStream>,
    writer: UnixStream,
    pid: libc::pid_t,
    _pty: OwnedFd,
}

fn conn() -> &'static Mutex<Option<Conn>> {
    static C: OnceLock<Mutex<Option<Conn>>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(None))
}

pub fn active() -> bool {
    conn().lock().unwrap().is_some()
}

/// Gli errori iniziano con un codice che l'interfaccia traduce:
/// `NODEV:` modalità sviluppatore spenta, `NOHELPER:` manca il pacchetto
/// -root, `AUTH:` password rifiutata.
pub fn start(password: &str) -> Result<(), String> {
    if active() {
        return Ok(());
    }
    if !Path::new(DEVEL_SU).exists() {
        return Err("NODEV:devel-su non c'è: la modalità sviluppatore è spenta".into());
    }
    if !Path::new(HELPER).exists() {
        return Err("NOHELPER:manca il pacchetto harbour-rootree-root".into());
    }
    let (mine, theirs) = UnixStream::pair().map_err(|e| e.to_string())?;
    // tutto ciò che serve al figlio si prepara PRIMA della fork (dopo, in un
    // programma con più thread, niente allocazioni)
    let prog = CString::new(DEVEL_SU).unwrap();
    let args = [CString::new("devel-su").unwrap(), CString::new(HELPER).unwrap(), CString::new("--fd").unwrap(), CString::new("3").unwrap()];
    let mut argv: Vec<*const libc::c_char> = args.iter().map(|a| a.as_ptr()).collect();
    argv.push(std::ptr::null());
    let theirs_fd = theirs.as_raw_fd();

    let mut master: libc::c_int = -1;
    let pid = unsafe { libc::forkpty(&mut master, std::ptr::null_mut(), std::ptr::null(), std::ptr::null()) };
    if pid < 0 {
        return Err(format!("forkpty: {}", std::io::Error::last_os_error()));
    }
    if pid == 0 {
        unsafe {
            libc::dup2(theirs_fd, 3); // dup2 toglie il CLOEXEC alla copia
            libc::execv(prog.as_ptr(), argv.as_ptr());
            libc::_exit(127);
        }
    }
    drop(theirs);
    let pty = unsafe { OwnedFd::from_raw_fd(master) };

    // il dialogo: aspettare «Password:», scriverla, poi il saluto sul socket.
    // ⛔ L'esito si decide sul SOCKET, non sul terminale: l'helper chiude il
    //    terminale (stdio su /dev/null) subito prima di salutare, e devel-su
    //    che rifiuta la password esce chiudendo la sua copia del socket.
    // ⛔ Niente kill+waitpid bloccante: devel-su è setuid, da qui non lo si
    //    uccide (EPERM) e l'attesa non finirebbe; esce da solo quando il
    //    socket si chiude, e lo si raccoglie su un thread.
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut out = Vec::new();
    let mut sent = false;
    let mut hello = Vec::new();
    let mut pty_open = true;
    mine.set_nonblocking(true).map_err(|e| e.to_string())?;
    let reap = move |pid: libc::pid_t| {
        std::thread::spawn(move || unsafe {
            libc::waitpid(pid, std::ptr::null_mut(), 0);
        });
    };
    let why = |out: &[u8]| -> String {
        let text = String::from_utf8_lossy(out).to_string();
        if text.contains("Auth") || text.contains("auth") {
            "AUTH:password rifiutata".into()
        } else {
            format!("l'helper non è partito: {}", text.replace("Password:", "").trim())
        }
    };
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            reap(pid);
            return Err("devel-su non risponde".into());
        }
        let mut fds = [
            libc::pollfd { fd: mine.as_raw_fd(), events: libc::POLLIN, revents: 0 },
            libc::pollfd { fd: if pty_open { master } else { -1 }, events: libc::POLLIN, revents: 0 },
        ];
        let n = unsafe { libc::poll(fds.as_mut_ptr(), 2, left.as_millis().min(1000) as i32) };
        if n < 0 {
            reap(pid);
            return Err(format!("poll: {}", std::io::Error::last_os_error()));
        }
        if pty_open && fds[1].revents & (libc::POLLIN | libc::POLLHUP | libc::POLLERR) != 0 {
            let mut buf = [0u8; 512];
            let r = unsafe { libc::read(master, buf.as_mut_ptr().cast(), buf.len()) };
            if r <= 0 {
                pty_open = false; // chiuso: l'esito lo dirà il socket
            } else {
                out.extend_from_slice(&buf[..r as usize]);
                if !sent && String::from_utf8_lossy(&out).contains("assword") {
                    let line = format!("{password}\n");
                    unsafe { libc::write(master, line.as_ptr().cast(), line.len()) };
                    sent = true;
                }
            }
        }
        if fds[0].revents & (libc::POLLIN | libc::POLLHUP | libc::POLLERR) != 0 {
            let mut buf = [0u8; 256];
            let r = unsafe { libc::read(mine.as_raw_fd(), buf.as_mut_ptr().cast(), buf.len()) };
            if r == 0 {
                // socket chiuso senza saluto: devel-su ha rifiutato, o l'helper è morto
                // (le ultime righe del terminale dicono quale)
                let mut buf = [0u8; 512];
                while pty_open {
                    let r = unsafe { libc::read(master, buf.as_mut_ptr().cast(), buf.len()) };
                    if r <= 0 { break }
                    out.extend_from_slice(&buf[..r as usize]);
                }
                reap(pid);
                return Err(why(&out));
            }
            if r > 0 {
                hello.extend_from_slice(&buf[..r as usize]);
                if hello.ends_with(b"\n") {
                    if String::from_utf8_lossy(&hello).trim() == HELLO {
                        break;
                    }
                    reap(pid);
                    return Err(format!("helper inatteso: {}", String::from_utf8_lossy(&hello).trim()));
                }
            }
        }
    }
    mine.set_nonblocking(false).map_err(|e| e.to_string())?;
    let writer = mine.try_clone().map_err(|e| e.to_string())?;
    *conn().lock().unwrap() = Some(Conn { reader: BufReader::new(mine), writer, pid, _pty: pty });
    Ok(())
}

/// Spegne la modalità: chiudere il socket basta, l'helper annulla i lavori
/// aperti ed esce. L'attesa del processo va su un thread (non si blocca la UI).
pub fn stop() {
    if let Some(c) = conn().lock().unwrap().take() {
        let pid = c.pid;
        let _ = c.writer.shutdown(std::net::Shutdown::Both);
        drop(c);
        if pid > 0 {
            std::thread::spawn(move || unsafe {
                libc::waitpid(pid, std::ptr::null_mut(), 0);
            });
        }
    }
}

/// Una richiesta all'helper. Se l'helper è sparito, la modalità si spegne.
pub fn call(req: &Request) -> Result<serde_json::Value, String> {
    let mut g = conn().lock().unwrap();
    let Some(c) = g.as_mut() else { return Err("la modalità root è spenta".into()) };
    let line = serde_json::to_string(req).map_err(|e| e.to_string())? + "\n";
    let mut reply = String::new();
    let ok = c.writer.write_all(line.as_bytes()).is_ok() && c.reader.read_line(&mut reply).map(|n| n > 0).unwrap_or(false);
    if !ok {
        let pid = c.pid;
        *g = None;
        std::thread::spawn(move || unsafe {
            libc::waitpid(pid, std::ptr::null_mut(), 0);
        });
        return Err("la modalità root si è interrotta".into());
    }
    let r: Response = serde_json::from_str(&reply).map_err(|e| e.to_string())?;
    if r.error.is_empty() { Ok(r.value) } else { Err(r.error) }
}

/// Come `call`, convertendo il risultato.
pub fn call_as<T: serde::de::DeserializeOwned>(req: &Request) -> Result<T, String> {
    serde_json::from_value(call(req)?).map_err(|e| e.to_string())
}

/// Per i test: collega un helper già avviato (su un socket qualunque) come
/// se fosse partito con devel-su.
#[cfg(test)]
pub fn attach_for_test(sock: UnixStream) {
    let mut reader = BufReader::new(sock.try_clone().unwrap());
    let mut hello = String::new();
    reader.read_line(&mut hello).unwrap();
    assert_eq!(hello.trim(), HELLO);
    let null = std::fs::File::open("/dev/null").unwrap();
    *conn().lock().unwrap() = Some(Conn { reader, writer: sock, pid: -1, _pty: OwnedFd::from(null) });
}
