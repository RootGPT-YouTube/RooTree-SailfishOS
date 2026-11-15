//! Il cuore dell'helper root: legge le richieste dal socket e risponde. Sta
//! nella libreria perché i test lo fanno girare su un socket qualunque, senza
//! root (il programma `rootree-helper` fa solo i controlli e lo avvia).

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::time::Duration;

use crate::proto::{HELLO, Request, Response};
use crate::{fsops, jobs, thumbs};

pub fn handle(req: Request) -> Response {
    use Request::*;
    match req {
        ListDir { path, hidden, key, desc, dirs_first } => Response::from(fsops::list_dir(&path, hidden, key, desc, dirs_first)),
        Stat { path } => Response::from(fsops::stat_path(&path)),
        ReadText { path, max } => Response::from(thumbs::read_text(&path, max)),
        WriteText { path, text } => Response::from(fsops::write_text(&path, &text, true)),
        MakeDir { parent, name } => Response::from(fsops::make_dir(&parent, &name)),
        MakeFile { parent, name } => Response::from(fsops::make_file(&parent, &name)),
        TrashOp { scope, action, item, days } => {
            crate::trash::set_days(days);
            Response::from(crate::trash::op(&scope, &action, &item))
        }
        Rename { path, name } => Response::from(fsops::rename_entry(&path, &name)),
        SetMode { path, mode } => Response::from(fsops::set_mode(&path, mode)),
        SetOwner { path, user, group } => Response::from(fsops::set_owner(&path, &user, &group)),
        JobCopy { sources, dest } => Response::ok(jobs::copy(sources, &dest)),
        JobMove { sources, dest } => Response::ok(jobs::move_to(sources, &dest)),
        JobDelete { paths } => Response::ok(jobs::delete(paths)),
        JobTrash { paths, days } => {
            crate::trash::set_days(days);
            Response::ok(jobs::trash(paths))
        }
        JobSize { paths } => Response::ok(jobs::size(paths)),
        JobExtract { archive, dest } => Response::ok(jobs::extract(&archive, &dest)),
        JobCompress { sources, dest } => Response::ok(jobs::compress(sources, &dest)),
        JobTypes { paths } => Response::ok(crate::storage::types(paths)),
        StorageInfo { path } => Response::ok(crate::storage::info(&path)),
        JobFsck { dev, fstype, repair } => Response::ok(crate::storage::fsck(dev, fstype, repair)),
        JobStatuses => Response::ok(jobs::statuses()),
        JobCancel { id } => Response::ok(jobs::cancel(id)),
        JobResolve { id, action, all } => Response::ok(jobs::resolve(id, action, all)),
        JobForget { id } => Response::ok(jobs::forget(id)),
    }
}

/// Saluta, risponde finché il socket resta aperto, poi annulla i lavori.
pub fn serve(sock: UnixStream) {
    serve_lines(sock);
    jobs::cancel_all_and_wait(Duration::from_secs(10));
}

/// Il dialogo, senza l'annullamento finale: nei test l'helper gira nello
/// stesso processo degli altri test, e annullerebbe anche i loro lavori.
pub fn serve_lines(sock: UnixStream) {
    let Ok(mut w) = sock.try_clone() else { return };
    if writeln!(w, "{HELLO}").is_err() {
        return;
    }
    for line in BufReader::new(sock).lines() {
        let Ok(line) = line else { break };
        let resp = match serde_json::from_str::<Request>(&line) {
            Ok(req) => handle(req),
            Err(e) => Response::err(format!("richiesta non valida: {e}")),
        };
        let out = serde_json::to_string(&resp).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"));
        if writeln!(w, "{out}").is_err() {
            break;
        }
    }
}
