//! Test del nucleo su alberi sintetici (mai file veri dell'utente).

use std::fs;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use crate::fsops::{self, natural_cmp};
use crate::jobs::{self, *};

static N: AtomicU32 = AtomicU32::new(0);

/// Una cartella di prova nuova; `altro_fs` la mette in target/ (nel motore
/// /tmp e il repo stanno su filesystem diversi: serve allo spostamento con copia).
fn tmp(altro_fs: bool) -> PathBuf {
    let base = if altro_fs {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("target/test-tmp")
    } else {
        std::env::temp_dir()
    };
    let d = base.join(format!("rootree-test-{}-{}", std::process::id(), N.fetch_add(1, Ordering::Relaxed)));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

/// Un albero con file piccoli, uno da 3 MiB (più blocchi di copia), una
/// cartella vuota, un file nascosto, un link e un file in sola lettura.
fn albero(root: &Path) -> PathBuf {
    let a = root.join("albero");
    fs::create_dir_all(a.join("sotto/profondo")).unwrap();
    fs::create_dir_all(a.join("vuota")).unwrap();
    fs::write(a.join("uno.txt"), b"uno").unwrap();
    fs::write(a.join(".nascosto"), b"segreto").unwrap();
    fs::write(a.join("sotto/due.bin"), (0..3 * 1024 * 1024u32).map(|i| (i % 251) as u8).collect::<Vec<_>>()).unwrap();
    fs::write(a.join("sotto/profondo/tre.md"), b"# tre").unwrap();
    fs::write(a.join("solalettura.txt"), b"ro").unwrap();
    fs::set_permissions(a.join("solalettura.txt"), fs::Permissions::from_mode(0o444)).unwrap();
    symlink("uno.txt", a.join("link-a-uno")).unwrap();
    a
}

/// Contenuto dell'albero come elenco ordinato (percorso relativo, cosa c'è).
fn impronta(root: &Path) -> Vec<(String, String)> {
    fn giro(base: &Path, p: &Path, out: &mut Vec<(String, String)>) {
        let rel = p.strip_prefix(base).unwrap().to_string_lossy().into_owned();
        let m = fs::symlink_metadata(p).unwrap();
        if m.file_type().is_symlink() {
            out.push((rel, format!("link:{}", fs::read_link(p).unwrap().display())));
        } else if m.is_dir() {
            out.push((rel, "dir".into()));
            for e in fs::read_dir(p).unwrap() {
                giro(base, &e.unwrap().path(), out);
            }
        } else {
            let data = fs::read(p).unwrap();
            let sum: u64 = data.iter().enumerate().map(|(i, b)| (i as u64 + 1) * *b as u64).sum();
            out.push((rel, format!("file:{}:{}:{:o}", data.len(), sum, m.permissions().mode() & 0o777)));
        }
    }
    let mut out = Vec::new();
    giro(root, root, &mut out);
    out.sort();
    out
}

fn s(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

#[test]
fn ordinamento_naturale() {
    let mut v = vec!["file10", "File2", "file1", "a", "B"];
    v.sort_by(|a, b| natural_cmp(a, b));
    assert_eq!(v, ["a", "B", "file1", "File2", "file10"]);
}

#[test]
fn elenco_cartelle_prima_e_nascosti() {
    let root = tmp(false);
    let a = albero(&root);
    let nomi = |hidden| -> Vec<String> {
        fsops::list_dir(&s(&a), hidden, fsops::SORT_NAME, false, true).unwrap().into_iter().map(|e| e.name).collect()
    };
    assert_eq!(nomi(false), ["sotto", "vuota", "link-a-uno", "solalettura.txt", "uno.txt"]);
    assert!(nomi(true).contains(&".nascosto".to_string()));
    let e = fsops::stat_path(&s(&a.join("link-a-uno"))).unwrap();
    assert!(e.is_link && !e.link_broken && e.size == 3 && e.link_target == "uno.txt");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn copia_albero_identico() {
    let root = tmp(false);
    let a = albero(&root);
    let dest = root.join("dest");
    fs::create_dir(&dest).unwrap();
    let st = wait(jobs::copy(vec![s(&a)], &s(&dest)));
    assert_eq!(st.state, ST_DONE, "{}", st.error);
    assert_eq!(st.done_bytes, st.total_bytes);
    assert_eq!(st.done_items, st.total_items);
    assert_eq!(impronta(&a), impronta(&dest.join("albero")));
    // nessun file temporaneo rimasto
    assert!(impronta(&dest).iter().all(|(p, _)| !p.contains("rootree-part")));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn copia_nella_stessa_cartella_fa_una_copia_accanto() {
    let root = tmp(false);
    fs::write(root.join("foto.jpg"), b"x").unwrap();
    let st = wait(jobs::copy(vec![s(&root.join("foto.jpg"))], &s(&root)));
    assert_eq!(st.state, ST_DONE);
    assert_eq!(fs::read(root.join("foto (1).jpg")).unwrap(), b"x");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn conflitto_salta_sovrascrivi_tieni_entrambi() {
    let root = tmp(false);
    let (src, dst) = (root.join("src"), root.join("dst"));
    fs::create_dir_all(&src).unwrap();
    fs::create_dir_all(&dst).unwrap();
    fs::write(src.join("a.txt"), b"nuovo").unwrap();

    for (risposta, atteso_a, atteso_copia) in [
        (ANS_SKIP, &b"vecchio"[..], None),
        (ANS_OVERWRITE, &b"nuovo"[..], None),
        (ANS_KEEP_BOTH, &b"vecchio"[..], Some(&b"nuovo"[..])),
    ] {
        fs::write(dst.join("a.txt"), b"vecchio").unwrap();
        let _ = fs::remove_file(dst.join("a (1).txt"));
        let id = jobs::copy(vec![s(&src.join("a.txt"))], &s(&dst));
        let st = wait(id);
        assert_eq!(st.state, ST_ASK_CONFLICT);
        assert_eq!(st.ask_dst, s(&dst.join("a.txt")));
        jobs::resolve(id, risposta, false);
        assert_eq!(wait(id).state, ST_DONE);
        assert_eq!(fs::read(dst.join("a.txt")).unwrap(), atteso_a);
        assert_eq!(fs::read(dst.join("a (1).txt")).ok().as_deref(), atteso_copia);
        jobs::forget(id);
    }
    let _ = fs::remove_dir_all(root);
}

#[test]
fn conflitto_annulla_lascia_tutto_come_prima() {
    let root = tmp(false);
    fs::create_dir_all(root.join("dst")).unwrap();
    fs::write(root.join("a"), b"1").unwrap();
    fs::write(root.join("dst/a"), b"2").unwrap();
    let id = jobs::copy(vec![s(&root.join("a"))], &s(&root.join("dst")));
    assert_eq!(wait(id).state, ST_ASK_CONFLICT);
    jobs::resolve(id, ANS_CANCEL, false);
    assert_eq!(wait(id).state, ST_CANCELLED);
    assert_eq!(fs::read(root.join("dst/a")).unwrap(), b"2");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn cartella_dentro_se_stessa_rifiutata() {
    let root = tmp(false);
    let a = albero(&root);
    let st = wait(jobs::copy(vec![s(&a)], &s(&a.join("sotto"))));
    assert_eq!(st.state, ST_FAILED);
    assert!(!a.join("sotto/albero").exists());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn sposta_stesso_filesystem() {
    let root = tmp(false);
    let a = albero(&root);
    let prima = impronta(&a);
    fs::create_dir(root.join("dest")).unwrap();
    let st = wait(jobs::move_to(vec![s(&a)], &s(&root.join("dest"))));
    assert_eq!(st.state, ST_DONE, "{}", st.error);
    assert!(!a.exists());
    assert_eq!(impronta(&root.join("dest/albero")), prima);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn sposta_fra_filesystem_diversi() {
    let (r1, r2) = (tmp(false), tmp(true));
    let a = albero(&r1);
    let prima = impronta(&a);
    let st = wait(jobs::move_to(vec![s(&a)], &s(&r2)));
    assert_eq!(st.state, ST_DONE, "{}", st.error);
    assert!(!a.exists(), "la sorgente doveva sparire");
    assert_eq!(impronta(&r2.join("albero")), prima);
    let _ = fs::remove_dir_all(r1);
    let _ = fs::remove_dir_all(r2);
}

#[test]
fn spostamento_con_conflitto_saltato_tiene_la_sorgente() {
    let root = tmp(false);
    fs::create_dir_all(root.join("src")).unwrap();
    fs::create_dir_all(root.join("dst/src")).unwrap();
    fs::write(root.join("src/a"), b"nuovo").unwrap();
    fs::write(root.join("src/b"), b"bi").unwrap();
    fs::write(root.join("dst/src/a"), b"vecchio").unwrap();
    let id = jobs::move_to(vec![s(&root.join("src"))], &s(&root.join("dst")));
    assert_eq!(wait(id).state, ST_ASK_CONFLICT);
    jobs::resolve(id, ANS_SKIP, false);
    assert_eq!(wait(id).state, ST_DONE);
    // b spostato, a saltato: la sorgente di a non si tocca
    assert_eq!(fs::read(root.join("dst/src/b")).unwrap(), b"bi");
    assert_eq!(fs::read(root.join("dst/src/a")).unwrap(), b"vecchio");
    assert_eq!(fs::read(root.join("src/a")).unwrap(), b"nuovo");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn annullare_una_copia_non_lascia_file_a_meta() {
    let root = tmp(false);
    let big = root.join("grande.bin");
    fs::write(&big, vec![7u8; 64 * 1024 * 1024]).unwrap();
    fs::create_dir(root.join("dest")).unwrap();
    // un conflitto prima davanti, per avere il tempo di annullare a metà del grande
    let id = jobs::copy(vec![s(&big)], &s(&root.join("dest")));
    loop {
        let st = jobs::statuses().into_iter().find(|s| s.id == id).unwrap();
        if st.done_bytes > 0 || st.state >= ST_DONE {
            break;
        }
    }
    jobs::cancel(id);
    let st = wait(id);
    assert!(st.state == ST_CANCELLED || st.state == ST_DONE);
    let resti: Vec<_> = fs::read_dir(root.join("dest")).unwrap().flatten().map(|e| e.file_name()).collect();
    if st.state == ST_CANCELLED {
        assert!(resti.is_empty(), "rimasti: {resti:?}");
    }
    let _ = fs::remove_dir_all(root);
}

#[test]
fn elimina_albero() {
    let root = tmp(false);
    let a = albero(&root);
    let st = wait(jobs::delete(vec![s(&a)]));
    assert_eq!(st.state, ST_DONE, "{}", st.error);
    assert_eq!(st.done_items, st.total_items);
    assert!(!a.exists());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn dimensione() {
    let root = tmp(false);
    let a = albero(&root);
    let st = wait(jobs::size(vec![s(&a)]));
    assert_eq!(st.state, ST_DONE);
    assert_eq!(st.total_bytes, 3 + 7 + 3 * 1024 * 1024 + 5 + 2);
    assert_eq!(st.total_items, 6); // 5 file + il link
    assert_eq!(st.dirs, 4); // albero, sotto, profondo, vuota
    let _ = fs::remove_dir_all(root);
}

#[test]
fn rinomina_non_sovrascrive() {
    let root = tmp(false);
    fs::write(root.join("a"), b"a").unwrap();
    fs::write(root.join("b"), b"b").unwrap();
    assert!(fsops::rename_entry(&s(&root.join("a")), "b").is_err());
    assert!(fsops::rename_entry(&s(&root.join("a")), "x/y").is_err());
    assert_eq!(fsops::rename_entry(&s(&root.join("a")), "c").unwrap(), s(&root.join("c")));
    assert_eq!(fs::read(root.join("b")).unwrap(), b"b");
    assert!(fsops::make_dir(&s(&root), "..").is_err());
    assert!(fsops::make_dir(&s(&root), "nuova").is_ok());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn testo_e_binario() {
    let root = tmp(false);
    fs::write(root.join("t.txt"), "città è già\n".repeat(10)).unwrap();
    fs::write(root.join("b.bin"), [1u8, 0, 2, 3]).unwrap();
    let t = crate::thumbs::read_text(&s(&root.join("t.txt")), 13).unwrap();
    // il limite cade dentro una «à»: niente carattere spezzato
    assert_eq!(t, "città è gi");
    assert!(crate::thumbs::read_text(&s(&root.join("b.bin")), 100).is_err());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn immagine_bmp_ridotta() {
    let root = tmp(false);
    let img = image::RgbaImage::from_pixel(800, 400, image::Rgba([10, 20, 30, 255]));
    img.save(root.join("x.bmp")).unwrap();
    let p = crate::thumbs::decode_picture(&s(&root.join("x.bmp")), 256).unwrap();
    assert_eq!((p.width, p.height), (256, 128));
    assert_eq!(&p.rgba[..4], &[10, 20, 30, 255]);
    let _ = fs::remove_dir_all(root);
}
