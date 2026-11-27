//! The checks before the start against a real Linux host with a real `bwrap`. No stand-in harness is needed, so these
//! cases run in the default build too (and in the mutation run). The sockets are sockets of this test process.
#![cfg(target_os = "linux")]

use std::fs;
use std::os::unix::fs::MetadataExt;
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use einschluss::{Beschreibung, Fehler, Profil, System, Umgebung, Zustand};

static ZAEHLER: AtomicU32 = AtomicU32::new(0);

struct Welt {
    wurzel: PathBuf,
    b: Beschreibung,
    _such: UnixListener,
    modell: UnixListener,
    modell_pfad: PathBuf,
}

impl Welt {
    fn neu() -> Welt {
        let n = ZAEHLER.fetch_add(1, Ordering::SeqCst);
        let basis = std::env::var_os("EINSCHLUSS_TESTORT")
            .map_or_else(|| PathBuf::from("/var/tmp"), PathBuf::from);
        let wurzel = basis.join(format!("ei-vp-{}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&wurzel);
        let ordner = |name: &str| {
            let p = wurzel.join(name);
            fs::create_dir_all(&p).expect("folder");
            p
        };
        let (programm, arbeit, zustand, such_ordner) = (
            ordner("programm"),
            ordner("arbeit"),
            ordner("zustand"),
            ordner("such"),
        );
        let such_pfad = such_ordner.join("such.sock");
        let modell_pfad = such_ordner.join("modell.sock");
        let heim = wurzel.join("heim");
        let b = Beschreibung {
            harness: "attrappe".to_owned(),
            arbeitsordner: arbeit,
            programmordner: vec![programm],
            zustand: Zustand {
                host: zustand,
                name: ".zustand".to_owned(),
                fortsetzen: false,
                gespraech: None,
            },
            heim: heim.clone(),
            such_socket: such_pfad.clone(),
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
            system: System::vom_host().expect("host system"),
        };
        Welt {
            _such: UnixListener::bind(&such_pfad).expect("socket"),
            modell: UnixListener::bind(&modell_pfad).expect("socket"),
            modell_pfad,
            b,
            wurzel,
        }
    }

    fn pruefe(&self, aendern: &dyn Fn(&mut Beschreibung)) -> Result<(), Fehler> {
        let mut c = self.b.clone();
        aendern(&mut c);
        Profil::bauen(&c).expect("builds").vorpruefen()
    }
}

impl Drop for Welt {
    fn drop(&mut self) {
        let _ = &self.modell;
        let _ = fs::remove_dir_all(&self.wurzel);
    }
}

fn unbrauchbar(was: &'static str) -> Result<(), Fehler> {
    Err(Fehler::QuelleUnbrauchbar(was))
}

#[test]
fn a_good_description_passes_with_one_socket_and_with_two() {
    let welt = Welt::neu();
    assert_eq!(welt.pruefe(&|_| ()), Ok(()));
    let modell = welt.modell_pfad.clone();
    assert_eq!(
        welt.pruefe(&|c| c.modell_socket = Some(modell.clone())),
        Ok(())
    );
}

#[test]
fn a_missing_or_wrong_working_folder_is_named() {
    let welt = Welt::neu();
    let fehlt = welt.wurzel.join("gibt-es-nicht");
    assert_eq!(
        welt.pruefe(&|c| c.arbeitsordner = fehlt.clone()),
        Err(Fehler::ArbeitsordnerFehlt)
    );
    let datei = welt.wurzel.join("datei");
    fs::write(&datei, "x").expect("file");
    assert_eq!(
        welt.pruefe(&|c| c.arbeitsordner = datei.clone()),
        Err(Fehler::ArbeitsordnerFehlt)
    );
}

#[test]
fn a_state_folder_with_content_or_of_another_kind_is_refused() {
    let welt = Welt::neu();
    let leer = "state folder is not an empty own folder";
    let alt = welt.b.zustand.host.join("alt");
    fs::write(&alt, "x").expect("file");
    assert_eq!(welt.pruefe(&|_| ()), unbrauchbar(leer));
    fs::remove_file(&alt).expect("remove");
    assert_eq!(welt.pruefe(&|_| ()), Ok(()));
    let datei = welt.wurzel.join("datei");
    fs::write(&datei, "x").expect("file");
    assert_eq!(
        welt.pruefe(&|c| c.zustand.host = datei.clone()),
        unbrauchbar(leer)
    );
    let fehlt = welt.wurzel.join("fehlt");
    assert_eq!(
        welt.pruefe(&|c| c.zustand.host = fehlt.clone()),
        unbrauchbar("mount source missing")
    );
}

#[test]
fn a_socket_that_is_missing_or_no_socket_is_refused() {
    let welt = Welt::neu();
    let datei = welt.wurzel.join("datei");
    fs::write(&datei, "x").expect("file");
    let fehlt = welt.wurzel.join("fehlt.sock");
    let nicht = unbrauchbar("socket is not a socket");
    let weg = unbrauchbar("mount source missing");
    assert_eq!(welt.pruefe(&|c| c.such_socket = datei.clone()), nicht);
    assert_eq!(welt.pruefe(&|c| c.such_socket = fehlt.clone()), weg);
    assert_eq!(
        welt.pruefe(&|c| c.modell_socket = Some(datei.clone())),
        nicht
    );
    assert_eq!(welt.pruefe(&|c| c.modell_socket = Some(fehlt.clone())), weg);
}

#[test]
fn a_program_folder_that_is_missing_a_file_or_a_link_is_refused() {
    let welt = Welt::neu();
    let datei = welt.wurzel.join("datei");
    fs::write(&datei, "x").expect("file");
    let fehlt = welt.wurzel.join("fehlt");
    assert_eq!(
        welt.pruefe(&|c| c.programmordner.push(datei.clone())),
        unbrauchbar("program folder is not a folder")
    );
    assert_eq!(
        welt.pruefe(&|c| c.programmordner.push(fehlt.clone())),
        unbrauchbar("mount source missing")
    );
    let verweis = welt.wurzel.join("verweis");
    std::os::unix::fs::symlink(&welt.b.programmordner[0], &verweis).expect("link");
    assert_eq!(
        welt.pruefe(&|c| c.programmordner = vec![verweis.clone()]),
        unbrauchbar("mount source is not a real path (link)")
    );
}

#[test]
fn a_working_folder_of_another_user_is_refused() {
    let welt = Welt::neu();
    assert_eq!(
        welt.pruefe(&|c| c.system.uid += 1),
        unbrauchbar("working folder is not an own folder")
    );
}

#[test]
fn bwrap_problems_and_a_failing_test_run_are_named() {
    let welt = Welt::neu();
    let profil = Profil::bauen(&welt.b).expect("builds");
    let mit = |pfad: PathBuf| profil.clone().mit_bwrap(pfad).vorpruefen();
    assert_eq!(mit("/nonexistent/bwrap".into()), Err(Fehler::BwrapFehlt));
    assert_eq!(
        mit(welt.b.programmordner[0].clone()),
        Err(Fehler::BwrapNichtAusfuehrbar)
    );
    let datei = welt.wurzel.join("datei");
    fs::write(&datei, "x").expect("file");
    assert_eq!(mit(datei), Err(Fehler::BwrapNichtAusfuehrbar));
    let kaputt = welt.wurzel.join("kaputt.sh");
    // Written from a child process: a write descriptor of this process can leak into the fork of another test thread,
    // and `exec` of the script then fails with ETXTBSY ("text file busy").
    let mut schreiber = std::process::Command::new("/bin/sh")
        .args(["-c", "cat > \"$1\" && chmod 755 \"$1\"", "sh"])
        .arg(&kaputt)
        .stdin(std::process::Stdio::piped())
        .spawn()
        .expect("writer");
    std::io::Write::write_all(
        &mut schreiber.stdin.take().expect("stdin"),
        b"#!/bin/sh\necho boom >&2\nexit 3\n",
    )
    .expect("script text");
    assert!(schreiber.wait().expect("writer end").success());
    match mit(kaputt) {
        Err(Fehler::Probelauf {
            status: Some(3),
            meldung,
        }) => assert_eq!(meldung, "boom?"),
        anders => panic!("wanted the test run to fail with status 3, got {anders:?}"),
    }
}

#[test]
fn the_host_system_names_the_user_and_the_etc_files() {
    let welt = Welt::neu();
    let system = System::vom_host().expect("host");
    assert_eq!(system.uid, fs::metadata(&welt.wurzel).expect("meta").uid());
    for datei in ["/etc/passwd", "/etc/group"] {
        assert!(
            system.ro_binds.iter().any(|(_, ziel)| ziel == datei),
            "{datei}"
        );
    }
    let bin = system.symlinks.iter().chain(&system.ro_binds);
    assert!(bin.into_iter().any(|(_, ziel)| ziel == "/bin"));
}
