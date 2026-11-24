//! Step 6a in a real enclosure (`/usr/bin/bwrap`, test build): the package of the cloud harness is extracted once in the
//! small enclosure into the cache folder of the user and bound read-only into the session. The harness is the stand-in
//! `bin/copilot`, which acts like the loader of the CLI (it extracts when the four paths are missing, `app.js` with
//! 20 MiB, more than the home of the enclosure holds).
//!
//! The forwarder of the enclosure is the real `einschluss-weiter`; build it first in the same target folder
//! (`cargo build -p einschluss --bins`). The cases need a host where `bwrap` may make a user namespace.

#![cfg(all(feature = "pruefbau", target_os = "linux"))]
#![allow(clippy::print_stderr, clippy::too_many_lines)]

mod gemeinsam;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use gemeinsam::{FASSUNG, Gesperrt, Lauf, Welt, datei, kopie, plattform};

fn bwrap_geht() -> bool {
    Command::new("/usr/bin/bwrap")
        .args([
            "--unshare-user",
            "--unshare-net",
            "--ro-bind",
            "/",
            "/",
            "--",
            "/usr/bin/true",
        ])
        .env_clear()
        .gesperrt_status()
        .is_ok_and(|s| s.success())
}

fn weiter() -> PathBuf {
    let neben = PathBuf::from(gemeinsam::LAUF)
        .parent()
        .expect("target")
        .join("einschluss-weiter");
    let p = if neben.is_file() {
        neben
    } else {
        std::env::var_os("LAUF_TEST_WEITER").map_or(neben, PathBuf::from)
    };
    assert!(
        p.is_file(),
        "build einschluss-weiter first: {}",
        p.display()
    );
    p
}

/// A world with the real `bwrap` and the real forwarder; the harness `copilot` probes the package place inside.
fn echte_welt() -> Welt {
    assert!(
        bwrap_geht(),
        "bwrap cannot make a user namespace on this host"
    );
    let mut w = Welt::neu();
    w.bwrap = None;
    kopie(
        &weiter().to_string_lossy(),
        &w.bin.join("einschluss-weiter"),
    );
    w
}

/// The place of the package inside the enclosure.
fn gast_ordner(w: &Welt) -> PathBuf {
    w.heim
        .join(".cache/copilot/pkg")
        .join(plattform())
        .join(FASSUNG)
}

/// The probe of the harness: the mark of the package can be read, a new file in the package folder cannot be made.
fn probe(w: &Welt) {
    let g = gast_ordner(w);
    w.verhalten(
        "copilot",
        &format!(
            "probe MARKE-PAKET {} schreib:{} schreib:{}",
            g.join(".extraction-complete").display(),
            g.join("neu.txt").display(),
            g.join("app.js").display()
        ),
    );
}

fn kinder_beenden(w: &Welt) {
    for p in w.kinder_pids() {
        if let Some(pid) = rustix::process::Pid::from_raw(p) {
            let _ = rustix::process::kill_process(pid, rustix::process::Signal::TERM);
        }
    }
}

fn entpackungen(w: &Welt) -> usize {
    fs::read_to_string(w.paket_cache().join("attrappe-entpackt.log"))
        .map_or(0, |t| t.lines().count())
}

fn groesse(p: &Path) -> u64 {
    fs::metadata(p).map_or(0, |m| m.len())
}

/// The harness ran in the enclosure, found the package and could not write into it.
fn harness_sah_das_paket(w: &Welt, lauf: &Lauf) {
    let aus = lauf.stdout();
    assert_eq!(lauf.exit(), Some(0), "{}{aus}", lauf.stderr());
    let g = gast_ordner(w);
    assert!(
        aus.contains(&format!(
            "pfad {} OK",
            g.join(".extraction-complete").display()
        )),
        "the mark is there inside: {aus}"
    );
    for name in ["neu.txt", "app.js"] {
        assert!(
            aus.contains(&format!("schreib {} EROFS", g.join(name).display())),
            "{name}: EROFS inside: {aus}"
        );
    }
}

#[test]
fn a_complete_folder_is_bound_read_only_and_not_extracted() {
    let w = echte_welt();
    let args = w.cloud();
    probe(&w);
    let vorher = fs::read(w.paket_ordner().join("app.js")).expect("app.js");
    let lauf = w.lauf(&args, &[]);
    harness_sah_das_paket(&w, &lauf);
    assert_eq!(entpackungen(&w), 0, "no extraction");
    assert_eq!(
        fs::read(w.paket_ordner().join("app.js")).expect("app.js"),
        vorher
    );
    assert!(!w.paket_cache().join(".werkbank-beiseite").exists());
    assert!(
        !w.paket_ordner().join("neu.txt").exists(),
        "nothing written on the host"
    );
    eprintln!("vollstaendig: rc 0, 0 extractions, EROFS inside");
    kinder_beenden(&w);
}

#[test]
fn a_missing_folder_is_extracted_in_the_small_enclosure() {
    let w = echte_welt();
    let args = w.cloud();
    probe(&w);
    w.paket_leeren();
    let lauf = w.lauf(&args, &[]);
    harness_sah_das_paket(&w, &lauf);
    assert_eq!(entpackungen(&w), 1);
    // the extraction of the stand-in: app.js of 20 MiB, more than the home tmpfs of 16 MiB of the enclosure holds
    assert_eq!(groesse(&w.paket_ordner().join("app.js")), 20 << 20);
    assert_eq!(
        einschluss::paket_pruefen(
            &w.paket_ordner(),
            &plattform(),
            rustix::process::geteuid().as_raw()
        ),
        Ok(())
    );
    let modus = fs::metadata(w.paket_cache())
        .expect("cache")
        .permissions()
        .mode()
        & 0o7777;
    assert_eq!(modus, 0o700);
    eprintln!("fehlt: rc 0, 1 extraction, 20 MiB on the host, cache 0700");
    kinder_beenden(&w);
}

#[test]
fn an_incomplete_folder_is_renamed_aside_and_extracted_again() {
    for rel in einschluss::paket_pfade(&plattform()) {
        let w = echte_welt();
        let args = w.cloud();
        probe(&w);
        fs::remove_file(w.paket_ordner().join(&rel)).expect("remove");
        let lauf = w.lauf(&args, &[]);
        harness_sah_das_paket(&w, &lauf);
        assert_eq!(entpackungen(&w), 1, "{rel}");
        let beiseite: Vec<String> = fs::read_dir(w.paket_cache().join(".werkbank-beiseite"))
            .expect("aside")
            .filter_map(Result::ok)
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(beiseite.len(), 1, "{rel}: {beiseite:?}");
        assert!(
            beiseite[0].starts_with(&format!("{}-{FASSUNG}-", plattform())),
            "{beiseite:?}"
        );
        assert_eq!(groesse(&w.paket_ordner().join("app.js")), 20 << 20, "{rel}");
        eprintln!("{rel} fehlt: rc 0, beiseite {}, neu entpackt", beiseite[0]);
        kinder_beenden(&w);
    }
}

#[test]
fn a_link_in_the_path_is_never_bound() {
    use std::os::unix::fs::symlink;
    let w = echte_welt();
    let args = w.cloud();
    probe(&w);
    let echt = w.paket_cache().join("pkg/echt");
    fs::rename(w.paket_ordner(), &echt).expect("move");
    symlink(&echt, w.paket_ordner()).expect("link");
    let lauf = w.lauf(&args, &[]);
    assert_eq!(lauf.exit(), Some(73), "{}", lauf.stderr());
    assert_eq!(
        lauf.stderr(),
        "lauf.paket_cache: Paketordner des Harness unbrauchbar\nlauf.paket_teil: - verweis\n"
    );
    assert_eq!(lauf.stdout(), "", "the harness did not run");
    assert_eq!(w.pid_von("rangkern"), None, "no child");
    assert_eq!(entpackungen(&w), 0);
    assert!(w.paket_ordner().is_symlink(), "the link stays untouched");
    // a link above the folder: the cache folder itself
    let w = echte_welt();
    let args = w.cloud();
    let woanders = w.heim.join("woanders");
    fs::rename(w.paket_cache(), &woanders).expect("move");
    symlink(&woanders, w.paket_cache()).expect("link");
    let lauf = w.lauf(&args, &[]);
    assert_eq!(lauf.exit(), Some(73), "{}", lauf.stderr());
    assert!(
        lauf.stderr().ends_with("lauf.paket_teil: - verweis\n"),
        "{}",
        lauf.stderr()
    );
    eprintln!("verweis: rc 73, lauf.paket_cache, kein Kind");
}

#[test]
fn an_unknown_form_of_the_package_starts_nothing() {
    let w = echte_welt();
    let args = w.cloud();
    fs::remove_file(w.bin.join("package.json")).expect("remove");
    let lauf = w.lauf(&args, &[]);
    assert_eq!(lauf.exit(), Some(78));
    assert_eq!(
        lauf.stderr(),
        "lauf.paket_form: Paket des Harness unbekannt oder nicht lesbar\n"
    );
    assert_eq!(w.pid_von("rangkern"), None, "no child");
    // another version in the platform package than in the main package
    let w = echte_welt();
    let args = w.cloud();
    let p = w.bin.join(format!(
        "node_modules/@github/copilot-{}/package.json",
        plattform()
    ));
    let text = fs::read_to_string(&p).expect("read");
    datei(&p, text.replace(FASSUNG, "0.0.87").as_bytes(), 0o644);
    let lauf = w.lauf(&args, &[]);
    assert_eq!(lauf.exit(), Some(78), "{}", lauf.stderr());
    assert_eq!(w.pid_von("rangkern"), None, "no child");
}

#[test]
fn a_failed_extraction_is_the_fixed_code_and_starts_nothing() {
    let w = echte_welt();
    let args = w.cloud();
    probe(&w);
    w.paket_leeren();
    // the cache folder is there, but its folder pkg cannot be written: the extraction inside fails
    let pkg = w.paket_cache().join("pkg");
    fs::create_dir_all(&pkg).expect("pkg");
    for o in [
        w.heim.join(".cache"),
        w.heim.join(".cache/werkbank"),
        w.heim.join(".cache/werkbank/harness"),
        w.paket_cache(),
    ] {
        fs::set_permissions(&o, fs::Permissions::from_mode(0o700)).expect("mode");
    }
    fs::set_permissions(&pkg, fs::Permissions::from_mode(0o500)).expect("mode");
    let lauf = w.lauf(&args, &[]);
    fs::set_permissions(&pkg, fs::Permissions::from_mode(0o700)).expect("mode");
    assert_eq!(lauf.exit(), Some(69), "{}", lauf.stderr());
    assert_eq!(
        lauf.stderr(),
        "lauf.paket_entpacken: Paket des Harness nicht entpackt\nlauf.paket_teil: 1 rc\n"
    );
    assert_eq!(lauf.stdout(), "", "the harness did not run");
    assert_eq!(w.pid_von("rangkern"), None, "no child");
    eprintln!("entpacken scheitert: rc 69, lauf.paket_teil: 1 rc, kein Kind");
}

#[test]
fn two_starts_at_once_extract_once_and_both_start() {
    let w = echte_welt();
    let args = w.cloud();
    probe(&w);
    w.paket_leeren();
    // a second start file of its own session in a folder of its own
    let start2 = w.wurzel.join("start2");
    fs::create_dir_all(&start2).expect("folder");
    fs::set_permissions(&start2, fs::Permissions::from_mode(0o700)).expect("mode");
    let text = fs::read_to_string(w.startdatei())
        .expect("start")
        .replace("\"s-1\"", "\"s-2\"");
    datei(&start2.join("start.json"), text.as_bytes(), 0o600);
    let args2 = vec![start2.join("start.json").to_string_lossy().into_owned()];
    let (a, b) = std::thread::scope(|s| {
        let x = s.spawn(|| w.lauf(&args, &[]));
        let y = s.spawn(|| w.lauf(&args2, &[]));
        (x.join().expect("first"), y.join().expect("second"))
    });
    harness_sah_das_paket(&w, &a);
    harness_sah_das_paket(&w, &b);
    assert_eq!(entpackungen(&w), 1, "one extraction for two starts");
    eprintln!("parallel: beide rc 0, 1 Entpacken");
    kinder_beenden(&w);
    for n in ["suche.pid", "modellweg.pid"] {
        if let Some(pid) = fs::read_to_string(start2.join(n))
            .ok()
            .and_then(|t| t.trim().parse().ok())
            .and_then(rustix::process::Pid::from_raw)
        {
            let _ = rustix::process::kill_process(pid, rustix::process::Signal::TERM);
        }
    }
}

/// The counter probe: the same CLI in the enclosure without the fix (no package folder bound, no cache folder) extracts
/// into the home of 16 MiB and fails with `ENOSPC`; with the cache folder bound the same call succeeds.
#[test]
fn counter_probe_without_the_fix_the_cli_fails_with_enospc() {
    let w = echte_welt();
    w.paket_leeren();
    let cache = w.paket_cache();
    fs::create_dir_all(&cache).expect("cache");
    let v = einschluss::Vorentpacken {
        heim: w.heim.clone(),
        cache: cache.clone(),
        programmordner: vec![w.bin.clone()],
        programm: w.bin.join("copilot"),
        path: "/usr/bin:/bin".to_owned(),
        system: einschluss::System::vom_host().expect("host"),
    };
    let mit = v.argumente().expect("args");
    let stelle = mit.iter().position(|a| a == "--bind").expect("bind");
    let mut ohne = mit.clone();
    ohne.drain(stelle..stelle + 3);
    assert!(!ohne.iter().any(|a| a == "--bind"));
    let lauf = |args: &[String]| {
        Command::new("/usr/bin/bwrap")
            .args(args)
            .env_clear()
            .gesperrt_output()
            .expect("bwrap")
    };
    let o = lauf(&ohne);
    let fehler = String::from_utf8_lossy(&o.stderr).into_owned();
    assert_eq!(o.status.code(), Some(1), "{fehler}");
    assert!(fehler.contains("ENOSPC"), "{fehler}");
    assert!(
        fehler.contains("Failed to extract bundled package"),
        "{fehler}"
    );
    assert!(!w.paket_ordner().exists(), "nothing on the host");
    let m = lauf(&mit);
    assert_eq!(
        m.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&m.stderr)
    );
    assert_eq!(groesse(&w.paket_ordner().join("app.js")), 20 << 20);
    eprintln!("Gegenprobe: ohne Bind rc 1 mit ENOSPC, mit Bind rc 0");
}
