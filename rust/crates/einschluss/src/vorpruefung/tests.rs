//! Tests of the checks before the start that need no Linux kernel: the pure rules, and the folder, `bwrap` and test-run
//! checks with scripts in place of `bwrap`. The checks of a real `bwrap` and of the sockets have their cases in
//! `tests/vorpruefung.rs` and `tests/aussen.rs`.

#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use super::*;
use crate::beschreibung::{Beschreibung, System, Umgebung, Zustand};

static ZAEHLER: AtomicU32 = AtomicU32::new(0);

/// A throw-away folder with a real path (the temporary folder may be a link on a Mac). Removed when dropped.
struct Ordner(PathBuf);

impl Ordner {
    fn neu() -> Ordner {
        let n = ZAEHLER.fetch_add(1, Ordering::SeqCst);
        let basis = fs::canonicalize(std::env::temp_dir()).expect("temporary folder");
        let pfad = basis.join(format!("ei-vp-{}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&pfad);
        fs::create_dir_all(&pfad).expect("folder");
        Ordner(pfad)
    }

    fn skript(&self, name: &str, text: &str) -> PathBuf {
        let pfad = self.0.join(name);
        skript_in_kindprozess_schreiben(&pfad, text);
        pfad
    }
}

impl Drop for Ordner {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn uid_von(pfad: &Path) -> u32 {
    fs::metadata(pfad).expect("meta").uid()
}

/// Writes an executable script from a child process. A write descriptor opened in this process can leak into the fork
/// of another test thread; `exec` of the script then fails with ETXTBSY ("text file busy") until that fork has
/// executed. In the child process the only write descriptor lives and ends there, before this function returns.
fn skript_in_kindprozess_schreiben(pfad: &Path, text: &str) {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let mut kind = Command::new("/bin/sh")
        .args(["-c", "cat > \"$1\" && chmod 755 \"$1\"", "sh"])
        .arg(pfad)
        .stdin(Stdio::piped())
        .spawn()
        .expect("writer");
    kind.stdin
        .take()
        .expect("stdin")
        .write_all(text.as_bytes())
        .expect("script text");
    assert!(kind.wait().expect("writer end").success(), "writer failed");
}

fn profil_in(o: &Ordner) -> Profil {
    let heim = o.0.join("heim");
    let b = Beschreibung {
        harness: "attrappe".to_owned(),
        arbeitsordner: o.0.join("arbeit"),
        programmordner: vec![o.0.join("programm")],
        zustand: Zustand {
            host: o.0.join("zustand"),
            name: ".zustand".to_owned(),
        },
        heim: heim.clone(),
        such_socket: o.0.join("such").join("such.sock"),
        modell_socket: None,
        lokal_weiter: None,
        dateien: Vec::new(),
        cloud: None,
        paket: None,
        umgebung: Umgebung {
            home: heim.to_string_lossy().into_owned(),
            path: "/usr/bin".to_owned(),
            term: "xterm".to_owned(),
            lang: "C".to_owned(),
            tmpdir: "/tmp".to_owned(),
        },
        geschuetzt: Vec::new(),
        argv: vec!["/usr/bin/true".to_owned()],
        system: System::usr_merge(uid_von(&o.0)),
    };
    Profil::bauen(&b).expect("profile builds")
}

#[test]
fn the_kernel_release_is_compared_by_number() {
    assert!(kernel_ab("6.2.0", 6, 2));
    assert!(kernel_ab("7.2.5-3-omarchy\n", 6, 2));
    assert!(kernel_ab("6.11.0-generic", 6, 2));
    assert!(kernel_ab("10.0", 6, 2));
    assert!(!kernel_ab("6.1.99", 6, 2));
    assert!(!kernel_ab("5.15.0-100-generic", 6, 2));
    assert!(!kernel_ab("", 6, 2));
    assert!(!kernel_ab("7", 6, 2));
    assert!(!kernel_ab("abc", 6, 2));
    assert!(kernel_ab("6.2", 6, 2));
}

#[test]
fn only_a_zero_or_a_new_kernel_without_the_file_closes_the_terminal() {
    assert!(tiocsti_geschlossen(Some("0"), "5.4.0"));
    assert!(tiocsti_geschlossen(Some("0\n"), "5.4.0"));
    assert!(!tiocsti_geschlossen(Some("1"), "7.2.5"));
    assert!(!tiocsti_geschlossen(Some("1\n"), "7.2.5"));
    assert!(!tiocsti_geschlossen(Some(""), "7.2.5"));
    assert!(!tiocsti_geschlossen(Some("00"), "7.2.5"));
    assert!(tiocsti_geschlossen(None, "6.2.0"));
    assert!(tiocsti_geschlossen(None, "7.2.5-3-omarchy"));
    assert!(!tiocsti_geschlossen(None, "6.1.0"));
    assert!(!tiocsti_geschlossen(None, ""));
}

#[test]
fn the_message_of_bwrap_is_short_and_plain() {
    assert_eq!(kurz(b"bwrap: error\n"), "bwrap: error?");
    assert_eq!(kurz(&[0xff, b'a']), "?a");
    let lang = vec![b'x'; 1000];
    assert_eq!(kurz(&lang).len(), MELDUNG_MAX);
    assert_eq!(kurz(b""), "");
}

#[test]
fn a_source_must_be_a_real_path_that_exists() {
    let o = Ordner::neu();
    assert!(quelle_pruefen(&o.0).is_ok());
    assert_eq!(
        quelle_pruefen(&o.0.join("fehlt")).expect_err("missing"),
        Fehler::QuelleUnbrauchbar("mount source missing")
    );
    let verweis = o.0.join("verweis");
    std::os::unix::fs::symlink(&o.0, &verweis).expect("link");
    assert_eq!(
        quelle_pruefen(&verweis).expect_err("link"),
        Fehler::QuelleUnbrauchbar("mount source is not a real path (link)")
    );
}

#[test]
fn an_own_folder_is_a_real_folder_of_this_user() {
    let o = Ordner::neu();
    let uid = uid_von(&o.0);
    let was = "x";
    assert_eq!(eigener_ordner(&o.0, uid, was), Ok(()));
    assert_eq!(
        eigener_ordner(&o.0, uid + 1, was),
        Err(Fehler::QuelleUnbrauchbar(was))
    );
    let datei = o.0.join("datei");
    fs::write(&datei, "x").expect("file");
    assert_eq!(
        eigener_ordner(&datei, uid, was),
        Err(Fehler::QuelleUnbrauchbar(was))
    );
    assert_eq!(
        eigener_ordner(&o.0.join("fehlt"), uid, was),
        Err(Fehler::QuelleUnbrauchbar("mount source missing"))
    );
}

#[test]
fn a_folder_is_empty_when_it_has_no_entry() {
    let o = Ordner::neu();
    assert!(ordner_leer(&o.0));
    fs::write(o.0.join("x"), "x").expect("file");
    assert!(!ordner_leer(&o.0));
    assert!(!ordner_leer(&o.0.join("fehlt")));
}

#[test]
fn bwrap_must_exist_and_be_an_executable_file() {
    let o = Ordner::neu();
    let profil = profil_in(&o);
    let mit = |pfad: PathBuf| profil.clone().mit_bwrap(pfad).bwrap_pruefen();
    assert_eq!(mit(o.0.join("fehlt")), Err(Fehler::BwrapFehlt));
    assert_eq!(mit(o.0.clone()), Err(Fehler::BwrapNichtAusfuehrbar));
    let nicht = o.0.join("nicht");
    fs::write(&nicht, "#!/bin/sh\n").expect("file");
    fs::set_permissions(&nicht, fs::Permissions::from_mode(0o644)).expect("mode");
    assert_eq!(mit(nicht), Err(Fehler::BwrapNichtAusfuehrbar));
    assert_eq!(mit(o.skript("gut", "#!/bin/sh\n")), Ok(()));
}

#[test]
fn the_test_run_reports_a_failure_with_status_and_message() {
    let o = Ordner::neu();
    let profil = profil_in(&o);
    let kaputt = o.skript("kaputt", "#!/bin/sh\necho boom >&2\nexit 3\n");
    assert_eq!(
        profil.clone().mit_bwrap(kaputt).probelauf(),
        Err(Fehler::Probelauf {
            status: Some(3),
            meldung: "boom?".to_owned()
        })
    );
    let gut = o.skript("gut", "#!/bin/sh\nexit 0\n");
    assert_eq!(profil.clone().mit_bwrap(gut).probelauf(), Ok(()));
    assert_eq!(
        profil.mit_bwrap(o.0.join("fehlt")).probelauf(),
        Err(Fehler::BwrapNichtAusfuehrbar)
    );
}

#[test]
fn the_test_run_ends_a_bwrap_that_does_not_answer_in_time() {
    let o = Ordner::neu();
    let langsam = o.skript("langsam", "#!/bin/sh\nexec sleep 20\n");
    let profil = profil_in(&o).mit_bwrap(langsam);
    let start = Instant::now();
    let ergebnis = profil.probelauf_frist(Duration::from_millis(300));
    let dauer = start.elapsed();
    assert_eq!(
        ergebnis,
        Err(Fehler::Probelauf {
            status: None,
            meldung: "no answer within the time limit".to_owned()
        })
    );
    assert!(dauer >= Duration::from_millis(300), "{dauer:?}");
    assert!(dauer < Duration::from_secs(5), "{dauer:?}");
}

#[test]
fn the_test_run_gets_true_as_its_program_and_an_empty_environment() {
    let o = Ordner::neu();
    let spur = o.0.join("spur");
    let text = format!(
        "#!/bin/sh\necho \"$@\" > {}\nenv > {}.env\nexit 0\n",
        spur.display(),
        spur.display()
    );
    let profil = profil_in(&o).mit_bwrap(o.skript("spur.sh", &text));
    let ergebnis = profil.probelauf();
    assert_eq!(ergebnis, Ok(()));
    let aufruf = fs::read_to_string(&spur).expect("args");
    assert!(aufruf.trim_end().ends_with("-- /usr/bin/true"), "{aufruf}");
    assert!(aufruf.contains("--unshare-net"), "{aufruf}");
    let umgebung = fs::read_to_string(format!("{}.env", spur.display())).expect("env");
    assert!(
        !umgebung.contains("HOME=") && !umgebung.contains("USER="),
        "{umgebung}"
    );
}

#[test]
fn a_missing_working_folder_is_named_before_the_platform() {
    let o = Ordner::neu();
    let profil = profil_in(&o);
    assert_eq!(profil.vorpruefen(), Err(Fehler::ArbeitsordnerFehlt));
}

#[test]
fn the_refusal_of_user_namespaces_is_recognised_by_the_message_of_bwrap() {
    for meldung in [
        "bwrap: loopback: Failed RTM_NEWADDR: Operation not permitted",
        "bwrap: No permissions to creating new namespace, likely because the kernel does not allow non-privileged user namespaces.",
        "bwrap: setting up uid map: Permission denied",
        "bwrap: kernel.unprivileged_userns_clone is 0",
    ] {
        assert!(namensraum_gesperrt(meldung), "{meldung}");
    }
    for meldung in ["bwrap: execvp x: No such file or directory", "boom", ""] {
        assert!(!namensraum_gesperrt(meldung), "{meldung}");
    }
}

#[test]
fn a_test_run_that_names_the_namespace_is_reported_as_the_namespace_refusal() {
    let o = Ordner::neu();
    let profil = profil_in(&o);
    let ubuntu = o.skript(
        "ubuntu",
        "#!/bin/sh\necho 'bwrap: loopback: Failed RTM_NEWADDR: Operation not permitted' >&2\nexit 1\n",
    );
    assert_eq!(
        profil.mit_bwrap(ubuntu).probelauf(),
        Err(Fehler::NamensraumGesperrt)
    );
}

// ---------------------------------------------------------------------------------------------------------------
// the sources of the cloud exit

use crate::beschreibung::{Cloud, Verteilung};
use std::os::unix::net::UnixListener;

/// A profile with a cloud exit whose sources exist: the folders, two sockets, the forwarder file and the bundle file
/// with the given modes. The listeners stay alive in the returned value.
fn cloud_profil(
    o: &Ordner,
    programm_modus: u32,
    bundle_modus: u32,
    programm_ist_ordner: bool,
) -> (Profil, Vec<UnixListener>) {
    for name in ["arbeit", "programm", "zustand", "such", "relay"] {
        fs::create_dir_all(o.0.join(name)).expect("folder");
    }
    let such = UnixListener::bind(o.0.join("such").join("such.sock")).expect("socket");
    let relay = UnixListener::bind(o.0.join("relay").join("relay.sock")).expect("socket");
    let programm = o.0.join("weiter");
    if programm_ist_ordner {
        fs::create_dir_all(&programm).expect("folder");
    } else {
        fs::write(&programm, "x").expect("file");
    }
    fs::set_permissions(&programm, fs::Permissions::from_mode(programm_modus)).expect("mode");
    let bundle = o.0.join("bundle.pem");
    fs::write(&bundle, "x").expect("file");
    fs::set_permissions(&bundle, fs::Permissions::from_mode(bundle_modus)).expect("mode");
    let heim = o.0.join("heim");
    let b = Beschreibung {
        harness: "copilot".to_owned(),
        arbeitsordner: o.0.join("arbeit"),
        programmordner: vec![o.0.join("programm")],
        zustand: Zustand {
            host: o.0.join("zustand"),
            name: ".zustand".to_owned(),
        },
        heim: heim.clone(),
        such_socket: o.0.join("such").join("such.sock"),
        modell_socket: None,
        lokal_weiter: None,
        dateien: Vec::new(),
        cloud: Some(Cloud {
            weiterreicher: o.0.join("relay").join("relay.sock"),
            weiter_programm: programm,
            bundle,
            verteilung: Verteilung::Ubuntu,
        }),
        paket: None,
        umgebung: Umgebung {
            home: heim.to_string_lossy().into_owned(),
            path: "/usr/bin".to_owned(),
            term: "xterm".to_owned(),
            lang: "C".to_owned(),
            tmpdir: "/tmp".to_owned(),
        },
        geschuetzt: Vec::new(),
        argv: vec!["/usr/bin/true".to_owned()],
        system: System::usr_merge(uid_von(&o.0)),
    };
    (
        Profil::bauen(&b).expect("profile builds"),
        vec![such, relay],
    )
}

#[test]
fn the_sources_of_the_cloud_exit_must_be_an_executable_file_and_a_readable_file() {
    let fehler = |was: &'static str| Err(Fehler::QuelleUnbrauchbar(was));
    let programm = "forwarder program is not an executable file";
    let bundle = "bundle is not a readable file";
    // the usual modes pass
    let o = Ordner::neu();
    let (profil, _l) = cloud_profil(&o, 0o755, 0o644, false);
    assert_eq!(profil.quellen_pruefen(), Ok(()));
    // the least that counts: execute only, read only (a mutant of the mask turns these red)
    let o = Ordner::neu();
    let (profil, _l) = cloud_profil(&o, 0o111, 0o444, false);
    assert_eq!(profil.quellen_pruefen(), Ok(()));
    // a program without an execute bit, a bundle without a read bit
    let o = Ordner::neu();
    let (profil, _l) = cloud_profil(&o, 0o644, 0o644, false);
    assert_eq!(profil.quellen_pruefen(), fehler(programm));
    let o = Ordner::neu();
    let (profil, _l) = cloud_profil(&o, 0o755, 0o200, false);
    assert_eq!(profil.quellen_pruefen(), fehler(bundle));
    // a folder in place of the program
    let o = Ordner::neu();
    let (profil, _l) = cloud_profil(&o, 0o755, 0o644, true);
    assert_eq!(profil.quellen_pruefen(), fehler(programm));
}

/// A profile of the local route whose sources exist: the model socket, the forwarder with `weiter_modus` and one
/// configuration file with `datei_modus`; `verweis` puts a link in place of the file.
fn lokal_profil(
    o: &Ordner,
    weiter_modus: u32,
    datei_modus: u32,
    verweis: bool,
) -> (Profil, Vec<UnixListener>) {
    for name in ["arbeit", "programm", "zustand", "such", "dateien"] {
        fs::create_dir_all(o.0.join(name)).expect("folder");
    }
    let such = UnixListener::bind(o.0.join("such").join("such.sock")).expect("socket");
    let modell = UnixListener::bind(o.0.join("such").join("modell.sock")).expect("socket");
    let weiter = o.0.join("weiter");
    fs::write(&weiter, "x").expect("file");
    fs::set_permissions(&weiter, fs::Permissions::from_mode(weiter_modus)).expect("mode");
    let echt = o.0.join("dateien").join("echt.json");
    fs::write(&echt, "{}").expect("file");
    fs::set_permissions(&echt, fs::Permissions::from_mode(datei_modus)).expect("mode");
    let quelle = if verweis {
        let v = o.0.join("dateien").join("models.json");
        std::os::unix::fs::symlink(&echt, &v).expect("link");
        v
    } else {
        echt
    };
    let heim = o.0.join("heim");
    let b = Beschreibung {
        harness: "pi".to_owned(),
        arbeitsordner: o.0.join("arbeit"),
        programmordner: vec![o.0.join("programm")],
        zustand: Zustand {
            host: o.0.join("zustand"),
            name: ".pi".to_owned(),
        },
        heim: heim.clone(),
        such_socket: o.0.join("such").join("such.sock"),
        modell_socket: Some(o.0.join("such").join("modell.sock")),
        lokal_weiter: Some(weiter),
        dateien: vec![crate::Datei {
            quelle,
            ziel: heim.join(".pi/agent/models.json"),
        }],
        cloud: None,
        paket: None,
        umgebung: Umgebung {
            home: heim.to_string_lossy().into_owned(),
            path: "/usr/bin".to_owned(),
            term: "xterm".to_owned(),
            lang: "C".to_owned(),
            tmpdir: "/tmp".to_owned(),
        },
        geschuetzt: Vec::new(),
        argv: vec!["/usr/bin/true".to_owned()],
        system: System::usr_merge(uid_von(&o.0)),
    };
    (
        Profil::bauen(&b).expect("profile builds"),
        vec![such, modell],
    )
}

#[test]
fn a_configuration_file_must_be_an_own_regular_file_of_mode_0600() {
    let datei = Err(Fehler::QuelleUnbrauchbar(
        "configuration file is not an own regular file of mode 0600",
    ));
    let o = Ordner::neu();
    let (profil, _l) = lokal_profil(&o, 0o755, 0o600, false);
    assert_eq!(profil.quellen_pruefen(), Ok(()));
    for modus in [0o644, 0o640, 0o400, 0o700, 0o4600, 0o610, 0o601] {
        let o = Ordner::neu();
        let (profil, _l) = lokal_profil(&o, 0o755, modus, false);
        assert_eq!(profil.quellen_pruefen(), datei, "{modus:o}");
    }
    let o = Ordner::neu();
    let (profil, _l) = lokal_profil(&o, 0o755, 0o600, true);
    assert_eq!(
        profil.quellen_pruefen(),
        Err(Fehler::QuelleUnbrauchbar(
            "mount source is not a real path (link)"
        ))
    );
    // a folder in place of the file
    let o = Ordner::neu();
    let (mut profil, _l) = lokal_profil(&o, 0o755, 0o600, false);
    let ordner = o.0.join("dateien").join("ordner");
    fs::create_dir_all(&ordner).expect("folder");
    fs::set_permissions(&ordner, fs::Permissions::from_mode(0o600)).expect("mode");
    profil.beschreibung.dateien[0].quelle = ordner;
    assert_eq!(profil.quellen_pruefen(), datei);
    // a file of another user: a file of root that exists on every Unix system
    let o = Ordner::neu();
    let (mut profil, _l) = lokal_profil(&o, 0o755, 0o600, false);
    profil.beschreibung.dateien[0].quelle = fs::canonicalize("/etc/hosts").expect("hosts");
    assert_eq!(profil.quellen_pruefen(), datei, "a file of root");
    // the rule alone: owner, kind and mode each count
    assert!(datei_quelle_gut(true, 7, 0o100_600, 7));
    assert!(!datei_quelle_gut(true, 8, 0o100_600, 7));
    assert!(!datei_quelle_gut(false, 7, 0o100_600, 7));
    assert!(!datei_quelle_gut(true, 7, 0o100_644, 7));
}

#[test]
fn the_forwarder_of_the_local_route_must_be_an_executable_file() {
    let o = Ordner::neu();
    let (profil, _l) = lokal_profil(&o, 0o644, 0o600, false);
    assert_eq!(
        profil.quellen_pruefen(),
        Err(Fehler::QuelleUnbrauchbar(
            "forwarder program is not an executable file"
        ))
    );
    let o = Ordner::neu();
    let (profil, _l) = lokal_profil(&o, 0o111, 0o600, false);
    assert_eq!(profil.quellen_pruefen(), Ok(()));
}
