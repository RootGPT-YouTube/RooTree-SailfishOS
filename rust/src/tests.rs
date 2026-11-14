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
        let m = fs::symlink_metadata(p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
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
    // prima la cartella (dall'08/11 si chiede, come in Dolphin): unire
    let st = wait(id);
    assert_eq!(st.state, ST_ASK_CONFLICT);
    assert!(st.ask_src_is_dir && st.ask_dst_is_dir);
    jobs::resolve(id, ANS_MERGE, false);
    // poi il file dentro
    let st = wait(id);
    assert_eq!(st.state, ST_ASK_CONFLICT);
    assert!(!st.ask_src_is_dir);
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

// ------------------------------------------------------------------ Fase 3: archivi e ricerca

/// L'impronta senza i link (7z non li salva).
fn senza_link(v: Vec<(String, String)>) -> Vec<(String, String)> {
    v.into_iter().filter(|(_, k)| !k.starts_with("link:")).collect()
}

#[test]
fn archivi_crea_sfoglia_estrai_in_ogni_formato() {
    for ext in ["zip", "tar", "tar.gz", "tar.bz2", "tar.xz", "tar.zst", "7z"] {
        let root = tmp(false);
        let a = albero(&root);
        let arch = root.join(format!("pacco.{ext}"));
        let st = wait(jobs::compress(vec![s(&a)], &s(&arch)));
        assert_eq!(st.state, ST_DONE, "{ext}: {}", st.error);
        assert!(arch.is_file(), "{ext}");
        assert!(!root.join(format!(".pacco.{ext}.rootree-part")).exists(), "{ext}");

        // sfogliare: una cartella dell'archivio come una cartella vera
        let dentro = fsops::list_dir(&format!("{}/albero/sotto", s(&arch)), false, 0, false, true).unwrap();
        let nomi: Vec<_> = dentro.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(nomi, ["profondo", "due.bin"], "{ext}");
        assert_eq!(dentro[1].size, 3 * 1024 * 1024, "{ext}");

        // «Estrai qui»: una sola cartella in cima, quindi niente cartella in più
        let dest = root.join("dest");
        fs::create_dir(&dest).unwrap();
        let st = wait(jobs::extract(&s(&arch), &s(&dest)));
        assert_eq!(st.state, ST_DONE, "{ext}: {}", st.error);
        let (atteso, avuto) = (impronta(&a), impronta(&dest.join("albero")));
        if ext == "7z" {
            assert_eq!(senza_link(atteso), senza_link(avuto), "{ext}");
        } else {
            assert_eq!(atteso, avuto, "{ext}");
        }
        assert!(impronta(&dest).iter().all(|(p, _)| !p.contains("rootree-part")), "{ext}");

        // copiare una voce da dentro l'archivio = estrarre solo quella
        let dest2 = root.join("dest2");
        fs::create_dir(&dest2).unwrap();
        let st = wait(jobs::copy(vec![format!("{}/albero/sotto", s(&arch))], &s(&dest2)));
        assert_eq!(st.state, ST_DONE, "{ext}: {}", st.error);
        assert_eq!(impronta(&a.join("sotto")), impronta(&dest2.join("sotto")), "{ext}");
        let _ = fs::remove_dir_all(root);
    }
}

#[test]
fn archivio_cattivo_non_esce_dalla_cartella() {
    let root = tmp(false);
    let arch = root.join("cattivo.tar");
    {
        let mut b = tar::Builder::new(fs::File::create(&arch).unwrap());
        // un nome con «..» (scritto a mano: il crate tar non lo permetterebbe)
        let mut h = tar::Header::new_old();
        h.as_old_mut().name[..12].copy_from_slice(b"../fuori.txt");
        h.set_size(5);
        h.set_mode(0o644);
        h.set_cksum();
        b.append(&h, &b"fuori"[..]).unwrap();
        // un link che porta fuori, e poi un file «attraverso» il link
        let mut h = tar::Header::new_gnu();
        h.set_entry_type(tar::EntryType::Symlink);
        h.set_size(0);
        b.append_link(&mut h, "esce", s(&root.join("bersaglio"))).unwrap();
        let mut h = tar::Header::new_gnu();
        h.set_size(5);
        h.set_mode(0o4755); // setuid: non deve restare
        b.append_data(&mut h, "esce/dentro.txt", &b"dentr"[..]).unwrap();
        b.finish().unwrap();
    }
    fs::create_dir(root.join("bersaglio")).unwrap();
    let dest = root.join("dest");
    fs::create_dir(&dest).unwrap();
    let st = wait(jobs::extract(&s(&arch), &s(&dest)));
    assert_eq!(st.state, ST_DONE, "{}", st.error);
    assert!(!root.join("fuori.txt").exists());
    assert!(fs::read_dir(root.join("bersaglio")).unwrap().next().is_none(), "scritto attraverso il link");
    // «../fuori.txt» saltato: in cima resta solo «esce», estratta senza cartella in più
    let f = dest.join("esce/dentro.txt");
    assert!(!f.parent().unwrap().is_symlink());
    assert_eq!(fs::read(&f).unwrap(), b"dentr");
    assert_eq!(fs::metadata(&f).unwrap().permissions().mode() & 0o7777, 0o755);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn file_compresso_da_solo() {
    use std::io::Write;
    let root = tmp(false);
    let gz = root.join("nota.txt.gz");
    let mut e = flate2::write::GzEncoder::new(fs::File::create(&gz).unwrap(), flate2::Compression::default());
    e.write_all(b"ciao dal gz").unwrap();
    e.finish().unwrap();
    let st = wait(jobs::extract(&s(&gz), &s(&root)));
    assert_eq!(st.state, ST_DONE, "{}", st.error);
    assert_eq!(fs::read(root.join("nota.txt")).unwrap(), b"ciao dal gz");
    // e si apre una voce da dentro (per i visori)
    let cache = root.join("cache");
    let p = crate::archive::extract_one(&gz, "nota.txt", &cache).unwrap();
    assert_eq!(fs::read(p).unwrap(), b"ciao dal gz");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn annullare_un_estrazione_non_lascia_niente() {
    let root = tmp(false);
    let a = root.join("grande");
    fs::create_dir(&a).unwrap();
    for i in 0..4 {
        fs::write(a.join(format!("f{i}.bin")), vec![i as u8; 8 << 20]).unwrap();
    }
    let arch = root.join("grande.tar");
    assert_eq!(wait(jobs::compress(vec![s(&a)], &s(&arch))).state, ST_DONE);
    let dest = root.join("dest");
    fs::create_dir(&dest).unwrap();
    let id = jobs::extract(&s(&arch), &s(&dest));
    jobs::cancel(id);
    let st = wait(id);
    assert_eq!(st.state, ST_CANCELLED);
    assert!(fs::read_dir(&dest).unwrap().next().is_none(), "resti dopo Annulla");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn ricerca_per_nome_e_contenuto() {
    let root = tmp(false);
    let a = albero(&root);
    let cerca = |q: &str, mode: u8, hidden: bool| {
        let id = crate::search::start(&s(&a), q, mode, false, hidden).unwrap();
        let mut hits = Vec::new();
        loop {
            let b = crate::search::take(id);
            hits.extend(b.hits.into_iter().map(|h| (h.entry.name, h.snippet)));
            if b.done {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        crate::search::stop(id);
        hits.sort();
        hits
    };
    let nomi = |v: Vec<(String, String)>| v.into_iter().map(|h| h.0).collect::<Vec<_>>();
    assert_eq!(nomi(cerca("*.MD", 0, false)), ["tre.md"]);
    assert_eq!(nomi(cerca("uno", 0, false)), ["link-a-uno", "uno.txt"]);
    assert_eq!(nomi(cerca("^(uno|due)\\.", 1, false)), ["due.bin", "uno.txt"]);
    assert!(cerca("segreto", 2, false).is_empty());
    assert_eq!(cerca("segreto", 2, true), [(".nascosto".to_string(), "segreto".to_string())]);
    assert_eq!(nomi(cerca("^# t", 3, false)), ["tre.md"]);
    assert!(crate::search::start(&s(&a), "(", 1, false, false).is_err());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn per_data_prima_i_piu_recenti() {
    let root = tmp(false);
    for (n, t) in [("vecchio", 1_000_000_000u64), ("nuovo", 1_700_000_000), ("medio", 1_400_000_000)] {
        let f = fs::File::create(root.join(n)).unwrap();
        f.set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(t)).unwrap();
    }
    let nomi = |desc| fsops::list_dir(&s(&root), false, fsops::SORT_DATE, desc, true).unwrap().into_iter().map(|e| e.name).collect::<Vec<_>>();
    assert_eq!(nomi(false), ["nuovo", "medio", "vecchio"]);
    assert_eq!(nomi(true), ["vecchio", "medio", "nuovo"]);
    let _ = fs::remove_dir_all(root);
}

/// Il criterio d'uscita della Fase 3: le estrazioni di RooTree uguali a
/// quelle di `bsdtar`, nei due versi. Serve bsdtar (c'è nel build engine);
/// si lancia a mano:  cargo test --release -- --ignored confronto
#[test]
#[ignore]
fn confronto_con_bsdtar() {
    use std::process::Command;
    let bsd = |args: &[&str], cwd: &Path| {
        let st = Command::new("bsdtar").args(args).current_dir(cwd).status().expect("manca bsdtar");
        assert!(st.success(), "bsdtar {args:?}");
    };
    let root = tmp(false);
    let a = albero(&root);
    fs::write(a.join("città è – ü.txt"), "nomi non ASCII").unwrap();
    let mut esito = Vec::new();

    // 1. archivi di bsdtar, estratti da bsdtar e da RooTree
    for (ext, crea) in [
        ("tar", &["-cf"][..]),
        ("tar.gz", &["-czf"][..]),
        ("tar.bz2", &["-cjf"][..]),
        ("tar.xz", &["-cJf"][..]),
        ("tar.zst", &["--zstd", "-cf"][..]),
        ("zip", &["--format", "zip", "-cf"][..]),
        ("7z", &["--format", "7zip", "-cf"][..]),
        ("cpio", &["--format", "newc", "-cf"][..]),
        ("iso", &["--format", "iso9660", "-cf"][..]),
    ] {
        let arch = root.join(format!("bsd.{ext}"));
        let mut args = crea.to_vec();
        let an = s(&arch);
        args.push(&an);
        args.push("albero");
        bsd(&args, &root);
        let (rif, nostra) = (root.join(format!("rif-{ext}")), root.join(format!("nostra-{ext}")));
        fs::create_dir(&rif).unwrap();
        fs::create_dir(&nostra).unwrap();
        bsd(&["-xf", &an], &rif);
        let st = wait(jobs::extract(&an, &s(&nostra)));
        assert_eq!(st.state, ST_DONE, "{ext}: {}", st.error);
        for d in [&rif, &nostra] {
            let kids: Vec<_> = fs::read_dir(d).unwrap().flatten().map(|e| e.file_name()).collect();
            assert!(d.join("albero").exists(), "{ext}: in {} c'è {kids:?}", d.display());
        }
        let ok = impronta(&rif.join("albero")) == impronta(&nostra.join("albero"));
        if !ok {
            let (r, n) = (impronta(&rif.join("albero")), impronta(&nostra.join("albero")));
            for x in r.iter().filter(|x| !n.contains(x)) {
                eprintln!("{ext}: solo bsdtar  {x:?}");
            }
            for x in n.iter().filter(|x| !r.contains(x)) {
                eprintln!("{ext}: solo RooTree {x:?}");
            }
        }
        esito.push((format!("bsdtar → RooTree  .{ext}"), ok));
    }

    // 2. il RAR di prova della suite di libarchive (RooTree non crea RAR)
    let rar = Path::new(env!("CARGO_MANIFEST_DIR")).join("testdata/prova.rar");
    let (rif, nostra) = (root.join("rif-rar"), root.join("nostra-rar"));
    fs::create_dir(&rif).unwrap();
    fs::create_dir(&nostra).unwrap();
    bsd(&["-xf", &s(&rar)], &rif);
    let st = wait(jobs::extract(&s(&rar), &s(&nostra)));
    assert_eq!(st.state, ST_DONE, "rar: {}", st.error);
    // RooTree mette le voci sciolte in una cartella col nome dell'archivio
    esito.push(("bsdtar → RooTree  .rar".into(), impronta(&rif) == impronta(&nostra.join("prova"))));

    // 3. archivi di RooTree, estratti da bsdtar
    for ext in ["zip", "tar", "tar.gz", "tar.bz2", "tar.xz", "tar.zst", "7z"] {
        let arch = root.join(format!("nostro.{ext}"));
        assert_eq!(wait(jobs::compress(vec![s(&a)], &s(&arch))).state, ST_DONE, "{ext}");
        let out = root.join(format!("da-bsdtar-{ext}"));
        fs::create_dir(&out).unwrap();
        bsd(&["-xf", &s(&arch)], &out);
        let (src, got) = (impronta(&a), impronta(&out.join("albero")));
        let ok = if ext == "7z" { senza_link(src) == senza_link(got) } else { src == got };
        esito.push((format!("RooTree → bsdtar  .{ext}"), ok));
    }

    for (k, ok) in &esito {
        eprintln!("{} {k}", if *ok { "✅" } else { "❌" });
    }
    assert!(esito.iter().all(|(_, ok)| *ok));
    let _ = fs::remove_dir_all(root);
}

// ------------------------------------------------------------------ Fase 4: modalità root

#[test]
fn modalita_root_inoltra_all_helper() {
    use std::os::unix::net::UnixStream;
    // l'helper vero (senza root: qui conta il protocollo e l'instradamento)
    let (app, helper) = UnixStream::pair().unwrap();
    let t = std::thread::spawn(move || crate::helper::serve_lines(helper));
    crate::root::attach_for_test(app);
    assert!(crate::root::active());

    let root = tmp(false);
    let a = albero(&root);
    *fsops::TEST_BACKUP_DIR.lock().unwrap() = Some(root.join("copie/di-sicurezza"));
    // elenco, informazioni, nuova cartella, rinomina, permessi: dall'helper
    let v = crate::list_dir(&s(&a), false, 0, false, true).unwrap();
    assert!(v.iter().any(|e| e.name == "sotto"));
    assert_eq!(crate::stat_path(&s(&a.join("uno.txt"))).unwrap().size, 3);
    let nuova = crate::make_dir(&s(&root), "nuova").unwrap();
    assert!(Path::new(&nuova).is_dir());
    assert!(crate::rename_entry(&nuova, "uno").is_ok());
    crate::set_mode(&s(&a.join("uno.txt")), 0o600).unwrap();
    assert_eq!(fs::metadata(a.join("uno.txt")).unwrap().permissions().mode() & 0o777, 0o600);
    assert!(crate::list_dir("/non/esiste", false, 0, false, true).is_err());

    // un lavoro dell'helper: id col bit REMOTE, stato e fine come i locali
    let dest = root.join("dest");
    fs::create_dir(&dest).unwrap();
    let id = crate::job_copy(vec![s(&a)], &s(&dest));
    assert!(id & super::REMOTE != 0);
    let st = loop {
        let st = crate::job_statuses().into_iter().find(|j| j.id == id).unwrap();
        if st.state >= ST_DONE {
            break st;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    };
    assert_eq!(st.state, ST_DONE, "{}", st.error);
    assert_eq!(impronta(&a), impronta(&dest.join("albero")));
    crate::job_forget(id);
    assert!(!crate::job_statuses().iter().any(|j| j.id == id));

    // salvataggio dall'helper: atomico, permessi tenuti
    crate::write_text(&s(&a.join("uno.txt")), "nuovo testo").unwrap();
    assert_eq!(fs::read_to_string(a.join("uno.txt")).unwrap(), "nuovo testo");
    assert_eq!(fs::metadata(a.join("uno.txt")).unwrap().permissions().mode() & 0o777, 0o600);
    assert!(!a.join(".uno.txt.rootree-part").exists());
    // e la copia di sicurezza: l'originale, una sola per file, cartella 700
    let copie = root.join("copie/di-sicurezza");
    let nome = fs::canonicalize(a.join("uno.txt")).unwrap().to_string_lossy().trim_start_matches('/').replace('/', "%");
    assert_eq!(fs::read_to_string(copie.join(&nome)).unwrap(), "uno");
    crate::write_text(&s(&a.join("uno.txt")), "terzo").unwrap();
    assert_eq!(fs::read_to_string(copie.join(&nome)).unwrap(), "nuovo testo");
    assert_eq!(fs::read_dir(&copie).unwrap().count(), 1);
    assert_eq!(fs::metadata(&copie).unwrap().permissions().mode() & 0o777, 0o700);

    // spenta la modalità, l'helper finisce e tutto torna locale
    crate::root::stop();
    t.join().unwrap();
    assert!(!crate::root::active());
    assert!(crate::list_dir(&s(&a), false, 0, false, true).is_ok());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn salvataggio_con_copia_di_sicurezza() {
    let root = tmp(false);
    let f = root.join("conf");
    fs::write(&f, "vecchio").unwrap();
    fs::set_permissions(&f, fs::Permissions::from_mode(0o640)).unwrap();
    // un file nuovo: niente copia
    assert_eq!(fsops::write_text(&s(&root.join("nuovo")), "x", false).unwrap(), "");
    fsops::write_text(&s(&f), "nuovo", false).unwrap();
    assert_eq!(fs::read_to_string(&f).unwrap(), "nuovo");
    assert_eq!(fs::metadata(&f).unwrap().permissions().mode() & 0o777, 0o640);
    // un link si salva sul file vero, e il link resta link
    symlink("conf", root.join("collegamento")).unwrap();
    fsops::write_text(&s(&root.join("collegamento")), "dal link", false).unwrap();
    assert_eq!(fs::read_to_string(&f).unwrap(), "dal link");
    assert!(fs::symlink_metadata(root.join("collegamento")).unwrap().file_type().is_symlink());
    let _ = fs::remove_dir_all(root);
}

// ------------------------------------------------------------------ Fase 5: remoti

#[test]
fn cassaforte_cifrata() {
    let root = tmp(false);
    let f = root.join("cassaforte.json");
    crate::vault::set_path(&s(&f));
    assert!(!crate::vault::exists());
    assert!(crate::vault::create("corta").is_err(), "almeno 6 caratteri");
    crate::vault::create("principale-di-prova").unwrap();
    assert!(crate::vault::unlocked());
    crate::vault::set("conn-1", "la-mia-password-ssh").unwrap();
    crate::vault::set("conn-2", "altra").unwrap();
    // nel file non c'è niente in chiaro, ed è solo dell'utente
    let raw = fs::read_to_string(&f).unwrap();
    assert!(!raw.contains("la-mia-password-ssh") && !raw.contains("conn-1"));
    assert_eq!(fs::metadata(&f).unwrap().permissions().mode() & 0o777, 0o600);
    crate::vault::lock();
    assert!(!crate::vault::unlocked());
    assert_eq!(crate::vault::get("conn-1"), None);
    assert!(crate::vault::set("conn-3", "x").is_err(), "chiusa non si scrive");
    let e = crate::vault::unlock("sbagliata").unwrap_err();
    assert!(e.to_string().starts_with("WRONG:"));
    crate::vault::unlock("principale-di-prova").unwrap();
    assert_eq!(crate::vault::get("conn-1").as_deref(), Some("la-mia-password-ssh"));
    crate::vault::remove("conn-1").unwrap();
    crate::vault::lock();
    crate::vault::unlock("principale-di-prova").unwrap();
    assert_eq!(crate::vault::get("conn-1"), None);
    assert_eq!(crate::vault::get("conn-2").as_deref(), Some("altra"));
    crate::vault::lock();
    let _ = fs::remove_dir_all(root);
}

#[test]
fn percorsi_remoti() {
    use crate::remote::split;
    assert_eq!(split("/@remote/abc/home/emi"), Some(("abc".into(), "/home/emi".into())));
    assert_eq!(split("/@remote/abc"), Some(("abc".into(), "/".into())));
    assert_eq!(split("/@remote/abc/"), Some(("abc".into(), "/".into())));
    assert_eq!(split("/home/emi"), None);
    assert_eq!(split("/@remote/"), None);
    // la lista di una cartella remota senza connessione: errore chiaro, niente panico
    assert!(crate::list_dir("/@remote/nessuna/x", false, 0, false, true).err().unwrap_or_default().contains("NOTCONNECTED"));
}

#[test]
fn cartella_su_cartella_si_chiede_come_dolphin() {
    let root = tmp(false);
    // due cartelle da copiare, entrambe già presenti nella destinazione; la
    // prima ha una sottocartella che c'è già anche lei
    for d in ["foto/viaggio", "musica"] {
        fs::create_dir_all(root.join("src").join(d)).unwrap();
        fs::create_dir_all(root.join("dst").join(d)).unwrap();
    }
    fs::write(root.join("src/foto/viaggio/nuova.jpg"), b"n").unwrap();
    fs::write(root.join("dst/foto/viaggio/vecchia.jpg"), b"v").unwrap();
    fs::write(root.join("src/musica/brano.mp3"), b"m").unwrap();
    let srcs = vec![s(&root.join("src/foto")), s(&root.join("src/musica"))];

    // «Unisci» per tutte le cartelle: una domanda sola, la sottocartella
    // «viaggio» (già presente) si unisce senza richiedere
    let id = jobs::copy(srcs.clone(), &s(&root.join("dst")));
    let st = wait(id);
    assert_eq!(st.state, ST_ASK_CONFLICT);
    assert!(st.ask_src_is_dir && st.ask_dst_is_dir);
    assert!(st.ask_dst.ends_with("dst/foto"));
    jobs::resolve(id, ANS_MERGE, true);
    assert_eq!(wait(id).state, ST_DONE);
    assert!(root.join("dst/foto/viaggio/nuova.jpg").exists() && root.join("dst/foto/viaggio/vecchia.jpg").exists());
    assert!(root.join("dst/musica/brano.mp3").exists());
    jobs::forget(id);

    // «Tieni entrambi»: la nuova arriva come «foto (1)», la vecchia non si tocca
    let id = jobs::copy(vec![srcs[0].clone()], &s(&root.join("dst")));
    assert_eq!(wait(id).state, ST_ASK_CONFLICT);
    jobs::resolve(id, ANS_KEEP_BOTH, false);
    assert_eq!(wait(id).state, ST_DONE);
    assert!(root.join("dst/foto (1)/viaggio/nuova.jpg").exists());
    assert!(!root.join("dst/foto (1)/viaggio/vecchia.jpg").exists());
    jobs::forget(id);

    // «Salta»: niente cambia
    let id = jobs::copy(vec![srcs[1].clone()], &s(&root.join("dst")));
    assert_eq!(wait(id).state, ST_ASK_CONFLICT);
    jobs::resolve(id, ANS_SKIP, false);
    assert_eq!(wait(id).state, ST_DONE);
    assert!(!root.join("dst/musica (1)").exists());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn cloud_aiuti_http() {
    use crate::http::{json_ascii, pct, rfc3339};
    assert_eq!(rfc3339("1970-01-01T00:00:00Z"), 0);
    assert_eq!(rfc3339("2024-03-01T12:34:56Z"), 1709296496);
    assert_eq!(rfc3339("2024-03-01T12:34:56.789Z"), 1709296496);
    assert_eq!(rfc3339("2024-03-01T13:34:56+01:00"), 1709296496);
    assert_eq!(rfc3339("boh"), 0);
    assert_eq!(json_ascii(&serde_json::json!({"path": "/città è"})), r#"{"path":"/citt\u00e0 \u00e8"}"#);
    assert_eq!(pct("/Foto mare/è.jpg", true), "/Foto%20mare/%C3%A8.jpg");
    assert_eq!(pct("a/b", false), "a%2Fb");
}

#[test]
fn accesso_dal_browser_ascolta_e_controlla_lo_state() {
    use std::io::{Read, Write};
    // il «browser» simulato: richieste al ritorno su 127.0.0.1
    let url = crate::oauth::begin("t1", "dropbox", "cliente-di-prova").unwrap();
    assert!(url.starts_with("https://www.dropbox.com/oauth2/authorize?"));
    assert!(url.contains("code_challenge_method=S256") && url.contains("token_access_type=offline") && url.contains("client_id=cliente-di-prova"));
    let redirect = url.split("redirect_uri=").nth(1).unwrap().split('&').next().unwrap().replace("%3A", ":").replace("%2F", "/");
    let port: u16 = redirect.trim_start_matches("http://127.0.0.1:").trim_end_matches('/').parse().unwrap();
    let state = url.split("state=").nth(1).unwrap().split('&').next().unwrap().to_string();
    let browser = std::thread::spawn(move || {
        let ask = |target: String| {
            let mut c = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
            write!(c, "GET {target} HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n").unwrap();
            let mut r = String::new();
            let _ = c.read_to_string(&mut r);
            r
        };
        let a = ask("/favicon.ico".into());                      // ignorata
        let b = ask("/?code=x&state=sbagliato".into());          // state falso: ignorata
        let c = ask(format!("/?error=access_denied&state={state}")); // l'utente rifiuta
        (a, b, c)
    });
    let r = crate::oauth::complete("t1", "dropbox", "cliente-di-prova", std::time::Duration::from_secs(10));
    let (a, b, c) = browser.join().unwrap();
    assert!(a.starts_with("HTTP/1.1 404") && b.starts_with("HTTP/1.1 404"));
    assert!(c.starts_with("HTTP/1.1 200") && c.contains("non riuscito"));
    assert!(r.unwrap_err().starts_with("CANCELLED:"));
    // senza Client ID non si parte
    assert!(crate::oauth::begin("t2", "dropbox", "  ").unwrap_err().starts_with("CLIENTID:"));
}


#[test]
fn webdav_multistatus() {
    // come risponde Nextcloud: prefisso d:, nomi codificati, proprietà assenti in una propstat 404
    let xml = r#"<?xml version="1.0"?><d:multistatus xmlns:d="DAV:">
<d:response><d:href>/remote.php/dav/files/u/Citt%c3%a0%20nuova/</d:href>
<d:propstat><d:prop><d:getlastmodified>Mon, 09 Nov 2026 11:01:32 GMT</d:getlastmodified><d:resourcetype><d:collection/></d:resourcetype></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat>
<d:propstat><d:prop><d:getcontentlength/></d:prop><d:status>HTTP/1.1 404 Not Found</d:status></d:propstat></d:response>
<d:response><d:href>/remote.php/dav/files/u/Citt%c3%a0%20nuova/a%26b.txt</d:href>
<d:propstat><d:prop><d:getlastmodified>Mon, 09 Nov 2026 11:01:33 GMT</d:getlastmodified><d:getcontentlength>42</d:getcontentlength><d:resourcetype/></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response>
</d:multistatus>"#;
    let v = crate::webdav::parse_multistatus(xml).unwrap();
    assert_eq!(v.len(), 2);
    assert_eq!(v[0].0, "/remote.php/dav/files/u/Città nuova");
    assert!(v[0].1.is_dir && !v[0].1.is_file);
    assert_eq!(v[0].1.mtime, 1794222092);
    assert_eq!(v[1].0, "/remote.php/dav/files/u/Città nuova/a&b.txt");
    assert!(v[1].1.is_file && !v[1].1.is_dir);
    assert_eq!(v[1].1.size, 42);
    // altro server: prefisso diverso e URL completo
    let v = crate::webdav::parse_multistatus(r#"<D:multistatus xmlns:D="DAV:"><D:response><D:href>https://s.example/dav/x.bin</D:href><D:propstat><D:prop><D:getcontentlength>7</D:getcontentlength><D:resourcetype></D:resourcetype></D:prop><D:status>HTTP/1.1 200 OK</D:status></D:propstat></D:response></D:multistatus>"#).unwrap();
    assert_eq!(v[0].0, "/dav/x.bin");
    assert_eq!(v[0].1.size, 7);
    assert_eq!(crate::webdav::http_date("non è una data"), 0);
}

#[test]
fn webdav_cestino() {
    let xml = r#"<?xml version="1.0"?><d:multistatus xmlns:d="DAV:" xmlns:nc="http://nextcloud.org/ns" xmlns:oc="http://owncloud.org/ns">
<d:response><d:href>/remote.php/dav/trashbin/u/trash/</d:href><d:propstat><d:prop><d:resourcetype><d:collection/></d:resourcetype></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response>
<d:response><d:href>/remote.php/dav/trashbin/u/trash/Citt%c3%a0.txt.d1794222092</d:href><d:propstat><d:prop><nc:trashbin-filename>Città.txt</nc:trashbin-filename><nc:trashbin-original-location>Documenti/Città.txt</nc:trashbin-original-location><nc:trashbin-deletion-time>1794222092</nc:trashbin-deletion-time><d:getcontentlength>12</d:getcontentlength><d:resourcetype/></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response>
<d:response><d:href>/remote.php/dav/trashbin/u/trash/Foto.d5</d:href><d:propstat><d:prop><nc:trashbin-filename>Foto</nc:trashbin-filename><nc:trashbin-deletion-time>5</nc:trashbin-deletion-time><oc:size>99</oc:size><d:resourcetype><d:collection/></d:resourcetype></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response>
</d:multistatus>"#;
    let v = crate::webdav::parse_trash(xml).unwrap();
    let a = v.as_array().unwrap();
    assert_eq!(a.len(), 2);
    assert_eq!(a[0]["id"], "Città.txt.d1794222092");
    assert_eq!(a[0]["name"], "Città.txt");
    assert_eq!(a[0]["original"], "Documenti/Città.txt");
    assert_eq!(a[0]["deleted"], 1794222092);
    assert_eq!(a[0]["size"], 12);
    assert_eq!(a[0]["dir"], false);
    assert_eq!(a[1]["dir"], true);
    assert_eq!(a[1]["size"], 99);
}

#[test]
fn cestino_trashinfo() {
    let t = Path::new("/run/media/u/SD/.Trash-1000");
    let txt = crate::trash::info_text("/run/media/u/SD/Foto città/a b%.jpg", 1_800_000_000);
    assert!(txt.contains("Path=/run/media/u/SD/Foto%20citt%C3%A0/a%20b%25.jpg"), "{txt}");
    let (p, d) = crate::trash::parse_info(&txt, t);
    assert_eq!(p, "/run/media/u/SD/Foto città/a b%.jpg");
    assert_eq!(d, 1_800_000_000);
    // Path relativo (permesso dalla specifica nei cestini di montaggio)
    let (p, _) = crate::trash::parse_info("[Trash Info]\nPath=Doc/x.txt\nDeletionDate=2026-11-12T10:00:00\n", t);
    assert_eq!(p, "/run/media/u/SD/Doc/x.txt");
    assert!(crate::trash::inside_trash(Path::new("/run/media/u/SD/.Trash-1000/files/x")));
    assert!(!crate::trash::inside_trash(Path::new("/run/media/u/SD/Trash/x")));
}

#[test]
fn cestino_sposta_ripristina_svuota() {
    let d = tmp(false);
    let t = d.join(".Trash-prova");
    let dirs = vec![t.clone()];
    fs::create_dir_all(d.join("cartella/sotto")).unwrap();
    fs::write(d.join("cartella/sotto/f.txt"), "ciao").unwrap();
    fs::write(d.join("uno.txt"), "1").unwrap();
    crate::trash::move_into(&d.join("cartella"), &t).unwrap();
    crate::trash::move_into(&d.join("uno.txt"), &t).unwrap();
    // stesso nome una seconda volta: un altro posto nel cestino
    fs::write(d.join("uno.txt"), "2").unwrap();
    crate::trash::move_into(&d.join("uno.txt"), &t).unwrap();
    assert!(!d.join("cartella").exists() && !d.join("uno.txt").exists());
    let list: serde_json::Value = serde_json::from_str(&crate::trash::op_in(&dirs, "list", "").unwrap()).unwrap();
    let v = list.as_array().unwrap();
    assert_eq!(v.len(), 3);
    let uni: Vec<_> = v.iter().filter(|x| x["name"] == "uno.txt").collect();
    assert_eq!(uni.len(), 2);
    let cart = v.iter().find(|x| x["name"] == "cartella").unwrap();
    assert_eq!(cart["dir"], true);
    assert_eq!(cart["original"], d.join("cartella").to_string_lossy().as_ref());
    // ripristino: torna dov'era, col contenuto
    crate::trash::op_in(&dirs, "restore", cart["id"].as_str().unwrap()).unwrap();
    assert_eq!(fs::read_to_string(d.join("cartella/sotto/f.txt")).unwrap(), "ciao");
    // due «uno.txt»: il secondo accanto, senza sovrascrivere
    for u in &uni {
        crate::trash::op_in(&dirs, "restore", u["id"].as_str().unwrap()).unwrap();
    }
    assert!(d.join("uno.txt").exists() && d.join("uno (1).txt").exists());
    // definitivo e svuota
    crate::trash::move_into(&d.join("uno.txt"), &t).unwrap();
    crate::trash::move_into(&d.join("cartella"), &t).unwrap();
    let list: serde_json::Value = serde_json::from_str(&crate::trash::op_in(&dirs, "list", "").unwrap()).unwrap();
    let primo = list[0]["id"].as_str().unwrap().to_string();
    crate::trash::op_in(&dirs, "delete", &primo).unwrap();
    assert!(!Path::new(&primo).exists());
    crate::trash::op_in(&dirs, "empty", "").unwrap();
    assert_eq!(fs::read_dir(t.join("files")).unwrap().count(), 0);
    assert_eq!(fs::read_dir(t.join("info")).unwrap().count(), 0);
    // una voce che non sta in un cestino non si tocca
    assert!(crate::trash::op_in(&dirs, "delete", &d.join("uno (1).txt").to_string_lossy()).is_err());
    assert!(d.join("uno (1).txt").exists());
    let _ = fs::remove_dir_all(&d);
}

#[test]
fn cestino_vecchie_voci() {
    let d = tmp(false);
    let t = d.join(".Trash-prova");
    fs::write(d.join("vecchio.txt"), "v").unwrap();
    fs::write(d.join("nuovo.txt"), "n").unwrap();
    crate::trash::move_into(&d.join("vecchio.txt"), &t).unwrap();
    crate::trash::move_into(&d.join("nuovo.txt"), &t).unwrap();
    // il vecchio retrodatato di 40 giorni
    let info = t.join("info/vecchio.txt.trashinfo");
    let old = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64 - 40 * 86_400;
    fs::write(&info, crate::trash::info_text(&d.join("vecchio.txt").to_string_lossy(), old)).unwrap();
    assert_eq!(crate::trash::purge(&t), 1);
    assert!(!t.join("files/vecchio.txt").exists() && t.join("files/nuovo.txt").exists());
    let _ = fs::remove_dir_all(&d);
}

// ------------------------------------------------------------------ S3

/// La firma SigV4 deve dare esattamente quella degli esempi della
/// documentazione AWS («Signature Calculations for the Authorization Header»).
#[test]
fn s3_firma_esempi_aws() {
    let (id, secret) = ("AKIAIOSFODNN7EXAMPLE", "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY");
    let empty = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    let h = |v: &[(&str, &str)]| v.iter().map(|(k, x)| (k.to_string(), x.to_string())).collect::<Vec<_>>();
    // GET Object
    let a = crate::s3::sign(id, secret, "us-east-1", "GET", "/test.txt", "",
        &h(&[("host", "examplebucket.s3.amazonaws.com"), ("range", "bytes=0-9"), ("x-amz-content-sha256", empty), ("x-amz-date", "20130524T000000Z")]),
        empty, "20130524T000000Z", "20130524");
    assert!(a.ends_with("Signature=f0e8bdb87c964420e857bd35b5d6ed310bd44f0170aba48dd91039c6036bdb41"), "{a}");
    assert!(a.contains("SignedHeaders=host;range;x-amz-content-sha256;x-amz-date"));
    // GET Bucket (elenco) con query
    let b = crate::s3::sign(id, secret, "us-east-1", "GET", "/", "max-keys=2&prefix=J",
        &h(&[("host", "examplebucket.s3.amazonaws.com"), ("x-amz-content-sha256", empty), ("x-amz-date", "20130524T000000Z")]),
        empty, "20130524T000000Z", "20130524");
    assert!(b.ends_with("Signature=34b48302e7b5fa45bde8084f4b7868a86f0a534bc59db6670ed5711ef69dc6f7"), "{b}");
}

#[test]
fn s3_data_e_endpoint() {
    assert_eq!(crate::s3::amz_date(1369353600), ("20130524T000000Z".into(), "20130524".into()));
    assert_eq!(crate::s3::amz_date(1794663226).1, "20261114");
    assert_eq!(crate::s3::split_endpoint("http://192.168.1.10:9000/"), ("http".into(), "192.168.1.10:9000".into()));
    assert_eq!(crate::s3::split_endpoint("https://S3.example.com:443"), ("https".into(), "s3.example.com".into()));
    assert_eq!(crate::s3::split_endpoint("minio.casa:9000"), ("https".into(), "minio.casa:9000".into()));
    assert_eq!(crate::s3::split_endpoint(""), ("https".into(), "".into()));
}

#[test]
fn s3_elenco_e_bucket() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<ListBucketResult xmlns="http://s3.amazonaws.com/doc/2006-03-01/"><Name>b</Name><Prefix>foto/</Prefix>
<IsTruncated>true</IsTruncated><NextContinuationToken>tok&amp;1</NextContinuationToken>
<Contents><Key>foto/</Key><Size>0</Size><LastModified>2026-11-14T10:00:00.000Z</LastModified></Contents>
<Contents><Key>foto/mare &amp; sole.jpg</Key><Size>12345</Size><LastModified>2026-11-14T10:00:01.000Z</LastModified></Contents>
<CommonPrefixes><Prefix>foto/2025/</Prefix></CommonPrefixes></ListBucketResult>"#;
    let l = crate::s3::parse_list(xml).unwrap();
    assert!(l.truncated);
    assert_eq!(l.next.as_deref(), Some("tok&1"));
    assert_eq!(l.objects.len(), 2);
    assert_eq!(l.objects[1].key, "foto/mare & sole.jpg");
    assert_eq!(l.objects[1].size, 12345);
    assert!(l.objects[1].mtime > 1_700_000_000);
    assert_eq!(l.prefixes, vec!["foto/2025/".to_string()]);
    let b = crate::s3::parse_buckets(r#"<ListAllMyBucketsResult><Buckets><Bucket><Name>uno</Name><CreationDate>2026-01-02T03:04:05.000Z</CreationDate></Bucket><Bucket><Name>due</Name><CreationDate>2026-01-02T03:04:05Z</CreationDate></Bucket></Buckets></ListAllMyBucketsResult>"#).unwrap();
    assert_eq!(b.iter().map(|x| x.0.as_str()).collect::<Vec<_>>(), vec!["uno", "due"]);
}
