//! The collectors against a made-up file tree and a stand-in for the programs they start. No real machine state is read.
#![cfg(unix)] // the collection and the program read a Unix machine (see `src/main.rs`)

use std::cell::RefCell;
use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::time::Duration;

use formate::kette::sha256_hex;
use werkbank_pruefen::bewerte::{bewerte, kuerzen};
use werkbank_pruefen::lage::{Abweichung, Lage};
use werkbank_pruefen::manifest::{PFLICHT, PROFIL_A, PROFIL_B, STARTER};
use werkbank_pruefen::sammeln::{
    Ausgabe, System, Umgebung, anhang_trifft, aufrufer, nutzer_gut, reale_uid,
};

/// One answer: the program, a part of its command line, and what it returns (`None` = did not run).
type Antwort = (&'static str, &'static str, Option<(i32, String)>);

/// Answers the programs the collectors start; remembers each call.
struct Attrappe {
    antworten: Vec<Antwort>,
    aufrufe: RefCell<Vec<String>>,
}

impl System for Attrappe {
    fn befehl(&self, programm: &str, args: &[&str], _frist: Duration) -> Option<Ausgabe> {
        let zeile = format!("{programm} {}", args.join(" "));
        self.aufrufe.borrow_mut().push(zeile.clone());
        self.antworten
            .iter()
            .find(|(p, teil, _)| *p == programm && zeile.contains(teil))
            .and_then(|(_, _, a)| a.clone())
            .map(|(rc, stdout)| Ausgabe {
                rc: Some(rc),
                // A failing stand-in answers on standard error, as the real programs do.
                stderr: if rc == 0 {
                    String::new()
                } else {
                    stdout.clone()
                },
                stdout,
            })
    }
}

fn eigener_ordner(name: &str) -> PathBuf {
    let o = std::env::temp_dir().join(format!("wb-pruefen-sammeln-{}-{name}", std::process::id()));
    fs::remove_dir_all(&o).ok();
    fs::create_dir_all(&o).expect("folder");
    o
}

fn schreiben(wurzel: &Path, rel: &str, inhalt: &[u8], modus: u32) {
    let p = wurzel.join(rel);
    fs::create_dir_all(p.parent().expect("parent")).expect("folders");
    fs::write(&p, inhalt).expect("write");
    fs::set_permissions(&p, fs::Permissions::from_mode(modus)).expect("mode");
}

fn eigene_uid() -> u32 {
    use std::os::unix::fs::MetadataExt;
    // The owner of a file made now, not of the temp folder (on a machine with a shared tmpfs that is root).
    let datei = std::env::temp_dir().join(format!(
        "wb-pruefen-uid-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    fs::write(&datei, b"x").expect("probe file");
    let uid = fs::metadata(&datei).expect("probe").uid();
    fs::remove_file(&datei).ok();
    uid
}

/// A manifest for the made-up tree: the required files and the profile of form A, each with size and hash.
fn baum(wurzel: &Path, version: &str, testbau: bool) {
    let mut dateien = Vec::new();
    let mut liste: Vec<(String, &str)> =
        PFLICHT.iter().map(|(p, a)| ((*p).to_owned(), *a)).collect();
    liste.push((PROFIL_A.to_owned(), "daten"));
    liste.push((STARTER.to_owned(), "programm"));
    for (pfad, art) in &liste {
        let inhalt = format!("inhalt von {pfad}").into_bytes();
        schreiben(wurzel, pfad, &inhalt, 0o644);
        dateien.push(format!(
            r#"{{"pfad":"{pfad}","groesse":{},"sha256":"{}","art":"{art}"}}"#,
            inhalt.len(),
            sha256_hex(&inhalt)
        ));
    }
    let text = format!(
        r#"{{"fassung":1,"paket":"werkbank","version":"{version}","plattform":"ubuntu-26.04-x64","bau":"{}","merkmale":[],"dateien":[{}],"bibliotheken":3}}"#,
        if testbau { "test" } else { "auslieferung" },
        dateien.join(",")
    );
    schreiben(
        wurzel,
        "usr/share/werkbank/manifest.json",
        text.as_bytes(),
        0o644,
    );
}

fn nutzerdaten(wurzel: &Path, uid_modus_ok: bool) {
    let o = wurzel.join("var/lib/werkbank/anna");
    fs::create_dir_all(o.join("vorfuehr-vault")).expect("folders");
    let modus = if uid_modus_ok { 0o700 } else { 0o755 };
    fs::set_permissions(&o, fs::Permissions::from_mode(modus)).expect("mode");
    fs::set_permissions(o.join("vorfuehr-vault"), fs::Permissions::from_mode(0o700)).expect("mode");
    schreiben(
        wurzel,
        "var/lib/werkbank/anna/einstellungen.json",
        br#"{"fassung":1}"#,
        0o600,
    );
    schreiben(
        wurzel,
        "var/lib/werkbank/anna/modellweg.schluessel",
        b"x",
        0o600,
    );
    schreiben(
        wurzel,
        "var/lib/werkbank/anna/copilot.schluessel",
        b"abc123-Zeichen\n",
        0o600,
    );
}

fn attrappe_ok(uid: u32) -> Attrappe {
    let u: &'static str = Box::leak(format!("{uid}\n").into_boxed_str());
    Attrappe {
        antworten: vec![
            ("id", "-u", Some((0, u.to_owned()))),
            ("id", "-g", Some((0, "1000\n".to_owned()))),
            ("dpkg-query", "werkbank", Some((0, "1.2.3".to_owned()))),
            (
                "systemctl",
                "werkbank",
                Some((0, "LoadState=loaded\nActiveState=active\n".to_owned())),
            ),
            ("setpriv", "bwrap", Some((0, String::new()))),
        ],
        aufrufe: RefCell::new(Vec::new()),
    }
}

fn lage_mit(wurzel: &Path, system: &Attrappe, nutzer: Option<&str>) -> Lage {
    Umgebung {
        wurzel: wurzel.to_path_buf(),
        nutzer: nutzer.map(str::to_owned),
        root: true,
        system,
    }
    .lage()
}

#[test]
fn ein_gesunder_baum_ist_gruen() {
    let w = eigener_ordner("gruen");
    baum(&w, "1.2.3", false);
    nutzerdaten(&w, true);
    schreiben(&w, "usr/bin/bwrap", b"bwrap", 0o755);
    let sys = attrappe_ok(eigene_uid());
    let lage = lage_mit(&w, &sys, Some("anna"));
    assert_eq!(lage.nutzerordner, "ok");
    assert_eq!(lage.schluessel, "ok");
    assert_eq!(lage.vault, "ok");
    assert_eq!(lage.einstellungen.zustand, "ok");
    assert_eq!(lage.paketstand.manifest, "ok");
    assert!(!lage.paketstand.testbau && !lage.paketstand.version_abweichend);
    assert!(
        lage.paketstand.dateien.is_empty(),
        "{:?}",
        lage.paketstand.dateien
    );
    assert!(lage.dienste.iter().all(|(_, z)| z == "ok"));
    // No lock file in the made-up tree: the lock is off, the probe `true` runs through the stand-in and passes.
    assert_eq!(lage.apparmor, "ok");
    let r = bewerte(&lage);
    assert_eq!(
        (r.rc, r.zeilen),
        (0, vec!["pruef.ok: Alle 8 Pruefungen bestanden.".to_owned()])
    );
    fs::remove_dir_all(&w).ok();
}

#[test]
fn ohne_nutzer_sind_dienste_ordner_und_schluessel_nicht_pruefbar() {
    let w = eigener_ordner("ohne-nutzer");
    baum(&w, "1.2.3", false);
    schreiben(&w, "usr/bin/bwrap", b"bwrap", 0o755);
    let lage = lage_mit(&w, &attrappe_ok(0), None);
    assert!(lage.dienste.iter().all(|(_, z)| z == "nicht_pruefbar"));
    for z in [&lage.nutzerordner, &lage.schluessel, &lage.vault] {
        assert_eq!(z, "nicht_pruefbar");
    }
    assert_eq!(lage.einstellungen.zustand, "nicht_pruefbar");
    let r = bewerte(&lage);
    assert_eq!(r.rc, 1);
    fs::remove_dir_all(&w).ok();
}

#[test]
fn der_schluessel_wird_nach_art_eigentuemer_modus_und_leere_beurteilt_nie_nach_inhalt() {
    let w = eigener_ordner("schluessel");
    nutzerdaten(&w, true);
    let pfad = w.join("var/lib/werkbank/anna/modellweg.schluessel");
    let uid = eigene_uid();
    let sys = attrappe_ok(uid);
    let z = |w: &Path| lage_mit(w, &sys, Some("anna")).schluessel;
    assert_eq!(z(&w), "ok");
    fs::set_permissions(&pfad, fs::Permissions::from_mode(0o644)).expect("mode");
    assert_eq!(z(&w), "modus");
    fs::set_permissions(&pfad, fs::Permissions::from_mode(0o600)).expect("mode");
    fs::write(&pfad, b"").expect("empty");
    assert_eq!(z(&w), "leer");
    fs::remove_file(&pfad).expect("remove");
    assert_eq!(z(&w), "fehlt");
    fs::create_dir(&pfad).expect("dir");
    assert_eq!(z(&w), "art");
    fs::remove_dir(&pfad).expect("dir");
    symlink("/etc/hostname", &pfad).expect("link");
    assert_eq!(z(&w), "art", "a link is not an ordinary file");
    let andere = attrappe_ok(uid + 1);
    fs::remove_file(&pfad).expect("remove");
    schreiben(
        &w,
        "var/lib/werkbank/anna/modellweg.schluessel",
        b"x",
        0o600,
    );
    assert_eq!(
        lage_mit(&w, &andere, Some("anna")).schluessel,
        "eigentuemer"
    );
    for a in sys.aufrufe.borrow().iter() {
        assert!(
            !a.contains("schluessel"),
            "the key file never appears in a command: {a}"
        );
    }
    fs::remove_dir_all(&w).ok();
}

#[test]
fn die_anmeldedatei_der_copilot_cli_wird_nach_art_eigentuemer_modus_leere_und_form_beurteilt() {
    let w = eigener_ordner("copilot");
    nutzerdaten(&w, true);
    let pfad = w.join("var/lib/werkbank/anna/copilot.schluessel");
    let uid = eigene_uid();
    let sys = attrappe_ok(uid);
    let z = |w: &Path| lage_mit(w, &sys, Some("anna")).copilot;
    assert_eq!(z(&w), "ok");
    // The form: one line of 1 to 4096 characters from `!` to `~`, with or without the final LF.
    let lang = "a".repeat(4096);
    for (inhalt, erwartet) in [
        (b"x".to_vec(), "ok"),
        (b"x\n".to_vec(), "ok"),
        (lang.clone().into_bytes(), "ok"),
        (format!("{lang}\n").into_bytes(), "ok"),
        (format!("{lang}b").into_bytes(), "form"),
        (b"zwei\nzeilen\n".to_vec(), "form"),
        (b"mit leerzeichen".to_vec(), "form"),
        (b"\n".to_vec(), "form"),
        (b"x\n\n".to_vec(), "form"),
        (b"x\r\n".to_vec(), "form"),
        ("umlaut-\u{e4}".as_bytes().to_vec(), "form"),
        (b"tab\tx".to_vec(), "form"),
    ] {
        fs::write(&pfad, &inhalt).expect("write");
        assert_eq!(z(&w), erwartet, "{:?}", String::from_utf8_lossy(&inhalt));
    }
    fs::write(&pfad, b"x").expect("write");
    fs::set_permissions(&pfad, fs::Permissions::from_mode(0o644)).expect("mode");
    assert_eq!(z(&w), "modus");
    fs::set_permissions(&pfad, fs::Permissions::from_mode(0o600)).expect("mode");
    fs::write(&pfad, b"").expect("empty");
    assert_eq!(z(&w), "leer");
    fs::remove_file(&pfad).expect("remove");
    assert_eq!(z(&w), "fehlt");
    fs::create_dir(&pfad).expect("dir");
    assert_eq!(z(&w), "art");
    fs::remove_dir(&pfad).expect("dir");
    schreiben(&w, "var/lib/werkbank/anna/copilot.schluessel", b"x", 0o600);
    assert_eq!(
        lage_mit(&w, &attrappe_ok(uid + 1), Some("anna")).copilot,
        "eigentuemer"
    );
    assert_eq!(
        lage_mit(&w, &sys, None).copilot,
        "nicht_pruefbar",
        "without a user nothing can be checked"
    );
    for a in sys.aufrufe.borrow().iter() {
        assert!(!a.contains("copilot"), "never in a command: {a}");
    }
    fs::remove_dir_all(&w).ok();
}

#[test]
fn der_nutzerordner_und_der_vault_haben_genau_den_modus_0700() {
    let w = eigener_ordner("ordner");
    nutzerdaten(&w, false);
    let sys = attrappe_ok(eigene_uid());
    let l = lage_mit(&w, &sys, Some("anna"));
    assert_eq!(l.nutzerordner, "rechte");
    fs::remove_dir_all(w.join("var/lib/werkbank/anna/vorfuehr-vault")).expect("remove");
    assert_eq!(lage_mit(&w, &sys, Some("anna")).vault, "fehlt");
    schreiben(&w, "var/lib/werkbank/anna/vorfuehr-vault", b"datei", 0o600);
    assert_eq!(lage_mit(&w, &sys, Some("anna")).vault, "kein_ordner");
    fs::remove_dir_all(&w).ok();
}

#[test]
fn die_einstellungen_haben_nur_die_ersten_beiden_stufen_von_vertrag_2() {
    let w = eigener_ordner("einst");
    nutzerdaten(&w, true);
    let sys = attrappe_ok(eigene_uid());
    let e = |inhalt: &[u8]| {
        schreiben(
            &w,
            "var/lib/werkbank/anna/einstellungen.json",
            inhalt,
            0o600,
        );
        let l = lage_mit(&w, &sys, Some("anna")).einstellungen;
        (l.zustand, l.code)
    };
    assert_eq!(e(b"{\"a\":1}").0, "ok");
    assert_eq!(
        e(b"{\"a\":1,\"a\":2}"),
        ("ungueltig".into(), Some("einst.json_ungueltig".into()))
    );
    assert_eq!(
        e(b"\xef\xbb\xbf{}").1.as_deref(),
        Some("einst.json_ungueltig")
    );
    assert_eq!(e(b"[]").1.as_deref(), Some("einst.json_ungueltig"));
    assert_eq!(e(&vec![b' '; 65_537]).1.as_deref(), Some("einst.zu_gross"));
    assert_eq!(
        e(&vec![b' '; 65_536]).1.as_deref(),
        Some("einst.json_ungueltig"),
        "65536 bytes are within the size"
    );
    fs::remove_file(w.join("var/lib/werkbank/anna/einstellungen.json")).expect("remove");
    assert_eq!(
        lage_mit(&w, &sys, Some("anna")).einstellungen.zustand,
        "fehlt"
    );
    fs::remove_dir_all(&w).ok();
}

#[test]
fn der_paketstand_sieht_fehlende_abweichende_und_fremde_dateien() {
    let w = eigener_ordner("paketstand");
    baum(&w, "1.2.3", true);
    schreiben(&w, "usr/bin/bwrap", b"bwrap", 0o755);
    fs::remove_file(w.join("usr/bin/werkbank-pruefen")).expect("remove");
    schreiben(&w, "usr/lib/werkbank/rangkern", b"anders", 0o755);
    schreiben(
        &w,
        "usr/lib/werkbank/probelauf/__pycache__/x.pyc",
        b"x",
        0o644,
    );
    schreiben(&w, "usr/share/werkbank/\u{4e2d}.txt", b"x", 0o644);
    schreiben(&w, "usr/lib/werkbank/bwrap", b"bwrap", 0o755);
    let sys = Attrappe {
        antworten: vec![("dpkg-query", "werkbank", Some((0, "9.9.9".to_owned())))],
        aufrufe: RefCell::new(Vec::new()),
    };
    let p = lage_mit(&w, &sys, None).paketstand;
    assert!(p.testbau && p.version_abweichend);
    let mut d = p.dateien.clone();
    d.sort();
    assert_eq!(
        d,
        vec![
            ("usr/bin/werkbank-pruefen".to_owned(), Abweichung::Fehlt),
            (
                "usr/lib/werkbank/probelauf/__pycache__/x.pyc".to_owned(),
                Abweichung::Fremd
            ),
            (
                "usr/lib/werkbank/rangkern".to_owned(),
                Abweichung::Abweichend
            ),
            ("usr/share/werkbank/?.txt".to_owned(), Abweichung::Fremd),
        ]
    );
    let zeilen = bewerte(&lage_mit(&w, &sys, None)).zeilen;
    assert!(zeilen.iter().all(|z| z.is_ascii() && z.len() <= 200));
    fs::remove_dir_all(&w).ok();
}

#[test]
fn ohne_dpkg_antwort_ist_der_paketstand_nicht_pruefbar() {
    let w = eigener_ordner("dpkg");
    baum(&w, "1.2.3", false);
    let sys = Attrappe {
        antworten: vec![],
        aufrufe: RefCell::new(Vec::new()),
    };
    assert_eq!(
        lage_mit(&w, &sys, None).paketstand.manifest,
        "nicht_pruefbar"
    );
    fs::remove_dir_all(&w).ok();
}

#[test]
fn ein_ungueltiges_oder_fehlendes_manifest_steht_im_paketstand() {
    let w = eigener_ordner("manifest");
    let sys = attrappe_ok(0);
    assert_eq!(lage_mit(&w, &sys, None).paketstand.manifest, "fehlt");
    schreiben(&w, "usr/share/werkbank/manifest.json", b"{}", 0o644);
    assert_eq!(lage_mit(&w, &sys, None).paketstand.manifest, "ungueltig");
    fs::remove_dir_all(&w).ok();
}

#[test]
fn die_dienste_werden_aus_loadstate_und_activestate_gelesen() {
    let w = eigener_ordner("dienste");
    let zustand = |stdout: &str, rc: i32| {
        let s = Attrappe {
            antworten: vec![("systemctl", "werkbank", Some((rc, stdout.to_owned())))],
            aufrufe: RefCell::new(Vec::new()),
        };
        let l = lage_mit(&w, &s, Some("anna"));
        let aufruf = s
            .aufrufe
            .borrow()
            .iter()
            .find(|a| a.starts_with("systemctl"))
            .cloned()
            .expect("call");
        assert!(aufruf.contains("--user --machine=anna@ show"), "{aufruf}");
        l.dienste[0].1.clone()
    };
    assert_eq!(zustand("LoadState=loaded\nActiveState=active\n", 0), "ok");
    assert_eq!(
        zustand("LoadState=loaded\nActiveState=inactive\n", 0),
        "inaktiv"
    );
    assert_eq!(
        zustand("LoadState=loaded\nActiveState=failed\n", 0),
        "fehlgeschlagen"
    );
    assert_eq!(
        zustand("LoadState=not-found\nActiveState=inactive\n", 0),
        "fehlt"
    );
    assert_eq!(zustand("", 1), "nicht_pruefbar");
    assert_eq!(zustand("Unsinn\n", 0), "nicht_pruefbar");
    fs::remove_dir_all(&w).ok();
}

fn apparmor_baum(w: &Path, form_b: bool, sperre: bool, geladen: &str) {
    schreiben(w, "usr/bin/bwrap", b"bwrap", 0o755);
    let (datei, text) = if form_b {
        (
            PROFIL_B,
            "profile werkbank-bwrap /usr/lib/werkbank/bwrap {\n  userns,\n}\n",
        )
    } else {
        (
            PROFIL_A,
            "profile werkbank-einschluss /usr/lib/werkbank/werkbank-einschluss {\n  include <abstractions/base>\n}\nprofile werkbank-einschluss-bwrap flags=(attach_disconnected,mediate_deleted) {\n  allow userns,\n}\nprofile werkbank-einschluss-kind flags=(attach_disconnected,mediate_deleted) {\n  allow userns,\n}\n",
        )
    };
    schreiben(w, datei, text.as_bytes(), 0o644);
    schreiben(
        w,
        "proc/sys/kernel/apparmor_restrict_unprivileged_userns",
        if sperre { b"1\n" } else { b"0\n" },
        0o644,
    );
    schreiben(
        w,
        "sys/kernel/security/apparmor/profiles",
        geladen.as_bytes(),
        0o644,
    );
}

fn apparmor(w: &Path, antworten: Vec<Antwort>) -> String {
    let s = Attrappe {
        antworten,
        aufrufe: RefCell::new(Vec::new()),
    };
    lage_mit(w, &s, None).apparmor
}

const EIGENE_A: &str = "werkbank-einschluss (enforce)\nwerkbank-einschluss-bwrap (enforce)\nwerkbank-einschluss-kind (enforce)\n";

#[test]
fn das_apparmor_profil_in_form_a_unter_aktiver_sperre() {
    let w = eigener_ordner("aa-a");
    apparmor_baum(&w, false, true, EIGENE_A);
    // The system `bwrap` must fail for the unprivileged user; the probe of form A is the starter with `selbsttest`.
    let scheitert = ("setpriv", "/usr/bin/bwrap", Some((1, String::new())));
    let probe_ok = (
        "setpriv",
        "/usr/lib/werkbank/werkbank-einschluss selbsttest",
        Some((0, String::new())),
    );
    let probe_rot = (
        "setpriv",
        "/usr/lib/werkbank/werkbank-einschluss selbsttest",
        Some((1, String::new())),
    );
    assert_eq!(
        apparmor(&w, vec![scheitert.clone(), probe_ok.clone()]),
        "nicht_pruefbar",
        "without the starter in the tree the probe cannot run"
    );
    schreiben(&w, STARTER, b"starter", 0o755);
    assert_eq!(
        apparmor(&w, vec![scheitert.clone(), probe_ok.clone()]),
        "ok"
    );
    assert_eq!(
        apparmor(&w, vec![scheitert.clone(), probe_rot]),
        "probelauf"
    );
    assert_eq!(
        apparmor(&w, vec![probe_ok.clone()]),
        "nicht_pruefbar",
        "a system bwrap check that did not run is not a pass"
    );
    let frei = ("setpriv", "/usr/bin/bwrap", Some((0, String::new())));
    assert_eq!(
        apparmor(&w, vec![frei, probe_ok.clone()]),
        "system_bwrap_frei"
    );
    apparmor_baum(&w, false, true, "werkbank-einschluss (enforce)\n");
    assert_eq!(
        apparmor(&w, vec![scheitert.clone(), probe_ok.clone()]),
        "nicht_geladen"
    );
    for fremd in [
        "/usr/bin/bwrap (enforce)\n",
        "/usr/bin/* (enforce)\n",
        "/usr/lib/werkbank/werkbank-einschluss (enforce)\n",
        "/usr/lib/werkbank/** (enforce)\n",
    ] {
        apparmor_baum(&w, false, true, &format!("{EIGENE_A}{fremd}"));
        assert_eq!(
            apparmor(&w, vec![scheitert.clone(), probe_ok.clone()]),
            "fremd",
            "{fremd}"
        );
    }
    // A loaded profile that is neither on `bwrap` nor on the starter is no foreign profile (a name with a path, a name with `*`).
    for harmlos in ["/usr/bin/anderes (enforce)\n", "* (enforce)\n"] {
        apparmor_baum(&w, false, true, &format!("{EIGENE_A}{harmlos}"));
        assert_eq!(
            apparmor(&w, vec![scheitert.clone(), probe_ok.clone()]),
            "ok",
            "{harmlos}"
        );
    }
    fs::remove_dir_all(&w).ok();
}

#[test]
fn das_system_bwrap_wird_nach_dem_label_des_probeprozesses_beurteilt() {
    use werkbank_pruefen::sammeln::{Label, bwrap_label};
    for (ausgabe, erwartet) in [
        ("unpriv_bwrap (enforce)\n", Label::Ubuntu),
        ("bwrap (enforce)\n", Label::Ubuntu),
        ("bwrap//&unpriv_bwrap (enforce)\n", Label::Ubuntu),
        ("unpriv_bwrap\n", Label::Ubuntu),
        ("unconfined\n", Label::Frei),
        ("", Label::Frei),
        ("\n", Label::Frei),
        ("werkbank-einschluss-bwrap (enforce)\n", Label::Frei),
        ("bwrap//&werkbank-einschluss-kind (enforce)\n", Label::Frei),
        ("firmenprofil (enforce)\n", Label::Fremd),
        ("bwrap//&firmenprofil (enforce)\n", Label::Fremd),
    ] {
        assert_eq!(bwrap_label(ausgabe), erwartet, "{ausgabe:?}");
    }
    let w = eigener_ordner("aa-ubuntu");
    apparmor_baum(&w, false, true, EIGENE_A);
    schreiben(&w, STARTER, b"starter", 0o755);
    let probe_ok = (
        "setpriv",
        "/usr/lib/werkbank/werkbank-einschluss selbsttest",
        Some((0, String::new())),
    );
    let system = |rc: i32, label: &str| {
        let s = Attrappe {
            antworten: vec![
                ("setpriv", "/usr/bin/bwrap", Some((rc, label.to_owned()))),
                probe_ok.clone(),
            ],
            aufrufe: RefCell::new(Vec::new()),
        };
        let l = lage_mit(&w, &s, None);
        (l.apparmor, l.system_bwrap)
    };
    let (zustand, hinweis) = system(0, "unpriv_bwrap (enforce)\n");
    assert_eq!((zustand.as_str(), hinweis.as_str()), ("ok", "ubuntu"));
    let (zustand, hinweis) = system(0, "unconfined\n");
    assert_eq!(
        (zustand.as_str(), hinweis.as_str()),
        ("system_bwrap_frei", "gesperrt")
    );
    let (zustand, _) = system(0, "werkbank-einschluss-bwrap (enforce)\n");
    assert_eq!(zustand, "system_bwrap_frei");
    let (zustand, _) = system(0, "firmenprofil (enforce)\n");
    assert_eq!(zustand, "nicht_pruefbar");
    let (zustand, hinweis) = system(1, "");
    assert_eq!((zustand.as_str(), hinweis.as_str()), ("ok", "gesperrt"));
    // The note shows in the result and is no finding.
    let s = Attrappe {
        antworten: vec![
            (
                "setpriv",
                "/usr/bin/bwrap",
                Some((0, "unpriv_bwrap (enforce)\n".to_owned())),
            ),
            probe_ok,
        ],
        aufrufe: RefCell::new(Vec::new()),
    };
    let lage = lage_mit(&w, &s, None);
    let r = bewerte(&lage);
    assert!(
        r.zeilen
            .iter()
            .any(|z| z.starts_with("pruef.hinweis_system_bwrap_ubuntu:")),
        "{:?}",
        r.zeilen
    );
    assert!(!r.zeilen.iter().any(|z| z.starts_with("pruef.apparmor_")));
    fs::remove_dir_all(&w).ok();
}

#[test]
fn ein_geladenes_unconfined_profil_der_werkbank_ist_rot_und_geht_allem_vor() {
    let w = eigener_ordner("aa-unconfined");
    // Even a tree without `bwrap` and without the profile file reports it first.
    schreiben(
        &w,
        "sys/kernel/security/apparmor/profiles",
        b"werkbank-test (unconfined)\n",
        0o644,
    );
    assert_eq!(apparmor(&w, vec![]), "unconfined");
    schreiben(&w, STARTER, b"starter", 0o755);
    for geladen in [
        "werkbank-einschluss-bwrap (unconfined)\n",
        "/usr/lib/werkbank/anderes (unconfined)\n",
        "unpriv_bwrap (enforce)\nwerkbank-x (enforce)\nwerkbank-y (unconfined)\n",
    ] {
        apparmor_baum(&w, false, true, geladen);
        assert_eq!(apparmor(&w, vec![]), "unconfined", "{geladen}");
    }
    // Not of the product, or not in that mode: no finding of this kind.
    for geladen in [
        "fremdes-profil (unconfined)\n",
        "werkbank-einschluss (enforce)\n",
        "werkbank-einschluss (complain)\n",
    ] {
        apparmor_baum(&w, false, true, geladen);
        assert_ne!(apparmor(&w, vec![]), "unconfined", "{geladen}");
    }
    fs::remove_dir_all(&w).ok();
}

#[test]
fn das_apparmor_profil_in_form_b() {
    let w = eigener_ordner("aa-b");
    apparmor_baum(&w, true, true, "werkbank-bwrap (enforce)\n");
    schreiben(&w, "usr/lib/werkbank/bwrap", b"bwrap", 0o755);
    let scheitert = ("setpriv", "/usr/bin/bwrap", Some((1, String::new())));
    let probe_ok = (
        "setpriv",
        "/usr/lib/werkbank/bwrap",
        Some((0, String::new())),
    );
    assert_eq!(
        apparmor(&w, vec![scheitert.clone(), probe_ok.clone()]),
        "ok"
    );
    let probe_rot = (
        "setpriv",
        "/usr/lib/werkbank/bwrap",
        Some((1, String::new())),
    );
    assert_eq!(
        apparmor(&w, vec![scheitert.clone(), probe_rot]),
        "probelauf"
    );
    schreiben(&w, "usr/lib/werkbank/bwrap", b"alt", 0o755);
    assert_eq!(
        apparmor(&w, vec![scheitert.clone(), probe_ok.clone()]),
        "kopie_abweichend"
    );
    fs::remove_file(w.join("usr/lib/werkbank/bwrap")).expect("remove");
    assert_eq!(apparmor(&w, vec![scheitert, probe_ok]), "kopie_abweichend");
    fs::remove_dir_all(&w).ok();
}

#[test]
fn ohne_sperre_genuegt_der_probelauf_und_ohne_bwrap_oder_datei_ist_es_rot() {
    let w = eigener_ordner("aa-ohne");
    assert_eq!(apparmor(&w, vec![]), "bwrap_fehlt");
    schreiben(&w, "usr/bin/bwrap", b"bwrap", 0o755);
    assert_eq!(apparmor(&w, vec![]), "datei_fehlt");
    apparmor_baum(&w, false, false, "");
    let ok = ("setpriv", "bwrap", Some((0, String::new())));
    assert_eq!(apparmor(&w, vec![ok]), "ok");
    assert_eq!(
        apparmor(&w, vec![("setpriv", "bwrap", Some((1, String::new())))]),
        "probelauf"
    );
    schreiben(&w, PROFIL_A, b"anders\n", 0o644);
    baum(&w, "1.2.3", false);
    schreiben(&w, PROFIL_A, b"anders\n", 0o644);
    assert_eq!(
        apparmor(
            &w,
            vec![("dpkg-query", "werkbank", Some((0, "1.2.3".into())))]
        ),
        "abweichend"
    );
    fs::remove_dir_all(&w).ok();
}

#[test]
fn nutzername_und_anhang_und_kuerzen() {
    assert!(nutzer_gut("anna") && nutzer_gut("_x-1") && nutzer_gut(&"a".repeat(32)));
    for schlecht in [
        "",
        "Anna",
        "a b",
        "../x",
        "-a",
        &"a".repeat(33),
        "a;b",
        "\u{e4}",
    ] {
        assert!(!nutzer_gut(schlecht), "{schlecht:?}");
    }
    assert!(anhang_trifft("/usr/bin/bwrap", "/usr/bin/bwrap"));
    assert!(anhang_trifft("/usr/bin/*", "/usr/bin/bwrap"));
    assert!(anhang_trifft("/usr/**", "/usr/bin/bwrap"));
    assert!(anhang_trifft("/usr/bin/bw*", "/usr/bin/bwrap"));
    assert!(anhang_trifft("*bwrap", "/usr/bin/bwrap"));
    assert!(!anhang_trifft("/usr/bin/other", "/usr/bin/bwrap"));
    assert!(!anhang_trifft("/usr/lib/*", "/usr/bin/bwrap"));
    assert!(!anhang_trifft("/usr/bin/bwrap*x", "/usr/bin/bwrap"));
    assert_eq!(kuerzen("a\u{e4}\tb"), "a??b");
    assert_eq!(kuerzen(&"x".repeat(300)).len(), 200);
}

#[test]
fn mehr_als_zwanzig_dateizeilen_werden_gezaehlt_und_sortiert() {
    let mut d: Vec<serde_json::Value> = (0..23)
        .map(|i| serde_json::json!({"pfad": format!("usr/lib/werkbank/f{:02}", 22 - i), "art": "fremd"}))
        .collect();
    d.push(serde_json::json!({"pfad": "usr/lib/werkbank/f00", "art": "fehlt"}));
    let lage = serde_json::json!({
        "plattform": "linux", "konten": 1, "dienste": [], "apparmor": "ok", "einstellungen": {"zustand": "ok"},
        "nutzerordner": "ok", "schluessel": "ok", "copilot": "ok", "vault": "ok", "stelle": "ok",
        "paketstand": {"manifest": "ok", "bau": "auslieferung", "version": "gleich", "dateien": d}
    });
    let r = bewerte(&Lage::aus_json(&lage).expect("lage"));
    assert_eq!(r.rc, 1);
    assert_eq!(r.zeilen.len(), 22);
    assert_eq!(
        r.zeilen[0],
        "pruef.paketstand_datei_fehlt: Die Datei usr/lib/werkbank/f00 fehlt."
    );
    assert_eq!(
        r.zeilen[1],
        "pruef.paketstand_datei_fremd: Die Datei usr/lib/werkbank/f00 steht nicht im Manifest."
    );
    assert_eq!(
        r.zeilen[20],
        "pruef.paketstand_weitere: 4 weitere Dateien weichen ab."
    );
    assert_eq!(r.zeilen[21], "pruef.rot: 21 Befunde.");
}

#[test]
fn eine_lage_mit_unbekanntem_zustand_wird_abgelehnt() {
    let gut = serde_json::json!({
        "plattform": "linux", "konten": 1, "dienste": [], "apparmor": "ok", "einstellungen": {"zustand": "ok"},
        "nutzerordner": "ok", "schluessel": "ok", "copilot": "ok", "vault": "ok", "stelle": "ok",
        "paketstand": {"manifest": "ok", "bau": "auslieferung", "version": "gleich", "dateien": []}
    });
    assert!(Lage::aus_json(&gut).is_ok());
    for (feld, wert) in [
        ("apparmor", serde_json::json!("gelb")),
        ("vault", serde_json::json!("x")),
        ("plattform", serde_json::json!("windows")),
        ("stelle", serde_json::json!("vielleicht")),
        ("konten", serde_json::json!(-1)),
        ("einstellungen", serde_json::json!({"zustand": "ungueltig"})),
        (
            "einstellungen",
            serde_json::json!({"zustand": "ok", "code": "einst.x"}),
        ),
        (
            "einstellungen",
            serde_json::json!({"zustand": "ungueltig", "code": "A B"}),
        ),
        (
            "dienste",
            serde_json::json!([{"name": "andere.service", "zustand": "ok"}]),
        ),
        (
            "dienste",
            serde_json::json!([{"name": "werkbank-kern.service", "zustand": "kaputt"}]),
        ),
        (
            "paketstand",
            serde_json::json!({"manifest": "ok", "bau": "x", "version": "gleich", "dateien": []}),
        ),
        (
            "paketstand",
            serde_json::json!({"manifest": "ok", "bau": "test", "version": "x", "dateien": []}),
        ),
        (
            "paketstand",
            serde_json::json!({"manifest": "kaputt", "bau": "test", "version": "gleich", "dateien": []}),
        ),
        (
            "paketstand",
            serde_json::json!({"manifest": "ok", "bau": "test", "version": "gleich", "dateien": [{"pfad": "", "art": "fremd"}]}),
        ),
        (
            "paketstand",
            serde_json::json!({"manifest": "ok", "bau": "test", "version": "gleich", "dateien": [{"pfad": "a", "art": "x"}]}),
        ),
    ] {
        let mut l = gut.clone();
        l[feld] = wert.clone();
        assert!(Lage::aus_json(&l).is_err(), "{feld} = {wert}");
    }
}

#[test]
fn der_zustand_von_ubuntus_profil_steht_in_beiden_faellen_da() {
    let w = eigener_ordner("aa-ubuntu-profil");
    schreiben(&w, STARTER, b"starter", 0o755);
    let probe_ok = (
        "setpriv",
        "/usr/lib/werkbank/werkbank-einschluss selbsttest",
        Some((0, String::new())),
    );
    let ubuntu = "bwrap (enforce)\nunpriv_bwrap (enforce)\n";
    // (profiles file, label of the probe child, expected state)
    let faelle: [(&str, &str, &str); 5] = [
        // S1: loaded, and the system bwrap is open through it.
        (ubuntu, "bwrap//&unpriv_bwrap (enforce)\n", "geladen"),
        // S0: not loaded, and the probe of the system bwrap fails.
        ("", "", "entladen"),
        // One of the two labels is enough.
        ("unpriv_bwrap (enforce)\n", "", "geladen"),
        ("bwrap (enforce)\n", "", "geladen"),
        // A label that only contains the name does not count.
        ("meinbwrap (enforce)\nbwrap2 (enforce)\n", "", "entladen"),
    ];
    for (n, (profile, label, zustand)) in faelle.iter().enumerate() {
        apparmor_baum(&w, false, true, &format!("{EIGENE_A}{profile}"));
        let rc = i32::from(label.is_empty());
        let s = Attrappe {
            antworten: vec![
                ("setpriv", "/usr/bin/bwrap", Some((rc, (*label).to_owned()))),
                probe_ok.clone(),
            ],
            aufrufe: RefCell::new(Vec::new()),
        };
        let lage = lage_mit(&w, &s, None);
        assert_eq!(lage.ubuntu_profil, *zustand, "case {n}");
        let r = bewerte(&lage);
        let ort = |satz: &str| r.zeilen.iter().position(|z| z.starts_with(satz));
        let gel = ort("pruef.hinweis_ubuntu_profil_geladen:");
        let ent = ort("pruef.hinweis_ubuntu_profil_entladen:");
        let sys = ort("pruef.hinweis_system_bwrap_ubuntu:");
        // Exactly one state line; the note on the system bwrap only in S1 (case 0) and after the state line.
        assert_eq!(gel.is_some(), *zustand == "geladen", "case {n}");
        assert_eq!(ent.is_some(), *zustand == "entladen", "case {n}");
        assert_eq!(sys.is_some(), n == 0, "case {n}");
        if n == 0 {
            assert!(gel < sys, "{:?}", r.zeilen);
        }
        // The state line is no finding: the count in `pruef.rot` does not include it.
        let befunde = r
            .zeilen
            .iter()
            .filter(|z| !z.starts_with("pruef.hinweis_") && !z.starts_with("pruef.rot"))
            .count();
        let rot = r
            .zeilen
            .iter()
            .find(|z| z.starts_with("pruef.rot: "))
            .cloned();
        assert_eq!(
            rot,
            Some(format!("pruef.rot: {befunde} Befunde.")),
            "case {n}"
        );
    }
    // Lock off: no line. Unreadable list: no line.
    apparmor_baum(&w, false, false, &format!("{EIGENE_A}{ubuntu}"));
    let s = Attrappe {
        antworten: vec![probe_ok.clone()],
        aufrufe: RefCell::new(Vec::new()),
    };
    assert_eq!(lage_mit(&w, &s, None).ubuntu_profil, "");
    apparmor_baum(&w, false, true, "");
    fs::remove_file(w.join("sys/kernel/security/apparmor/profiles")).expect("remove");
    assert_eq!(lage_mit(&w, &s, None).ubuntu_profil, "");
    fs::remove_dir_all(&w).ok();
}

/// The tree of a call without root: a manifest, the user folder of `anna`, the starter and the files of the profile.
fn lage_ohne_root(w: &Path, system: &Attrappe, nutzer: Option<&str>) -> Lage {
    Umgebung {
        wurzel: w.to_path_buf(),
        nutzer: nutzer.map(str::to_owned),
        root: false,
        system,
    }
    .lage()
}

#[test]
fn ohne_root_ist_der_nutzer_der_der_echten_kennung_und_sudo_user_gilt_nicht() {
    let w = eigener_ordner("aufrufer");
    schreiben(
        &w,
        "etc/passwd",
        b"root:x:0:0:root:/root:/bin/bash\nanna:x:1000:1000:Anna:/srv/beispiel/anna:/bin/bash\nBad:x:1001:1001::/h:/bin/sh\nkurz:x:7\n",
        0o644,
    );
    // Without root: the user of the real id; `SUDO_USER` is ignored, even with a good name.
    assert_eq!(
        aufrufer(Some("root"), Some(1000), &w),
        (Some("anna".to_owned()), false)
    );
    assert_eq!(
        aufrufer(None, Some(1000), &w),
        (Some("anna".to_owned()), false)
    );
    // With `sudo` (real id 0): the user that called it, if the name is good.
    assert_eq!(
        aufrufer(Some("anna"), Some(0), &w),
        (Some("anna".to_owned()), true)
    );
    assert_eq!(aufrufer(None, Some(0), &w), (None, true));
    assert_eq!(aufrufer(Some("Anna X"), Some(0), &w), (None, true));
    // An id that is not in `passwd`, a name that is no user name, a short line and an unknown id: no user, never root.
    assert_eq!(aufrufer(None, Some(4242), &w), (None, false));
    assert_eq!(aufrufer(None, Some(1001), &w), (None, false));
    assert_eq!(aufrufer(None, Some(7), &w), (None, false));
    assert_eq!(aufrufer(Some("anna"), None, &w), (None, false));
    // Without `passwd` there is no name.
    fs::remove_file(w.join("etc/passwd")).expect("remove");
    assert_eq!(aufrufer(None, Some(1000), &w), (None, false));
    fs::remove_dir_all(&w).ok();
}

#[cfg(not(target_os = "linux"))]
#[test]
fn ohne_proc_ist_die_kennung_unbekannt_und_jede_pruefung_nicht_pruefbar() {
    // macOS has no `/proc/self/status`: no id, so neither root nor a user (fail closed).
    assert_eq!(reale_uid(), None);
    assert_eq!(
        aufrufer(Some("anna"), reale_uid(), Path::new("/")),
        (None, false)
    );
}

#[cfg(target_os = "linux")]
#[test]
fn die_echte_kennung_dieses_prozesses_wird_gelesen() {
    // The test process is no root process in the contract's sense only when its id is not 0; the value must agree with the
    // owner of a file this process makes.
    assert_eq!(reale_uid(), Some(eigene_uid()));
}

fn attrappe_ohne_root(system: Vec<Antwort>) -> Attrappe {
    let mut antworten = vec![
        ("id", "-u", Some((0, format!("{}\n", eigene_uid())))),
        ("id", "-g", Some((0, "1000\n".to_owned()))),
        ("dpkg-query", "werkbank", Some((0, "1.2.3".to_owned()))),
        (
            "systemctl",
            "werkbank",
            Some((0, "LoadState=loaded\nActiveState=active\n".to_owned())),
        ),
    ];
    antworten.extend(system);
    Attrappe {
        antworten,
        aufrufe: RefCell::new(Vec::new()),
    }
}

#[test]
fn ohne_root_laufen_die_sonden_direkt_und_die_dienste_fragen_die_eigene_verwaltung() {
    // Lock off: the probe is `bwrap` itself, started directly (no `setpriv`).
    let w = eigener_ordner("ohne-root");
    baum(&w, "1.2.3", false);
    nutzerdaten(&w, true);
    schreiben(&w, "usr/bin/bwrap", b"bwrap", 0o755);
    let s = attrappe_ohne_root(vec![(
        "/usr/bin/bwrap",
        "--unshare-user",
        Some((0, String::new())),
    )]);
    let lage = lage_ohne_root(&w, &s, Some("anna"));
    let aufrufe = s.aufrufe.borrow().clone();
    assert!(
        aufrufe.iter().all(|a| !a.starts_with("setpriv")),
        "{aufrufe:?}"
    );
    assert!(
        aufrufe
            .iter()
            .any(|a| a.starts_with("/usr/bin/bwrap --unshare-user")),
        "{aufrufe:?}"
    );
    let dienst = aufrufe
        .iter()
        .find(|a| a.starts_with("systemctl"))
        .expect("systemctl call");
    assert!(dienst.starts_with("systemctl --user show "), "{dienst}");
    assert!(!dienst.contains("--machine"), "{dienst}");
    assert!(lage.ohne_root);
    assert_eq!(lage.apparmor, "ok");
    assert!(lage.dienste.iter().all(|(_, z)| z == "ok"));
    // The key file and the sign-in file are the user's own and readable: no line for them.
    assert_eq!(lage.schluessel, "ok");
    assert_eq!(lage.copilot, "ok");
    assert!(!lage.nicht_pruefbar());
    let r = bewerte(&lage);
    assert_eq!(
        (r.rc, r.zeilen),
        (0, vec!["pruef.ok: Alle 8 Pruefungen bestanden.".to_owned()])
    );
    fs::remove_dir_all(&w).ok();

    // Lock on: the probe is the starter with `selbsttest`, started directly; the system `bwrap` is tried directly too.
    let w = eigener_ordner("ohne-root-sperre");
    apparmor_baum(&w, false, true, EIGENE_A);
    schreiben(&w, STARTER, b"starter", 0o755);
    let s = attrappe_ohne_root(vec![
        ("/usr/bin/bwrap", "--unshare-user", Some((1, String::new()))),
        (
            "/usr/lib/werkbank/werkbank-einschluss",
            "selbsttest",
            Some((0, String::new())),
        ),
    ]);
    let lage = lage_ohne_root(&w, &s, Some("anna"));
    let aufrufe = s.aufrufe.borrow().clone();
    assert!(
        aufrufe.iter().all(|a| !a.starts_with("setpriv")),
        "{aufrufe:?}"
    );
    assert!(
        aufrufe
            .iter()
            .any(|a| a.starts_with("/usr/lib/werkbank/werkbank-einschluss selbsttest")),
        "{aufrufe:?}"
    );
    assert_eq!(lage.apparmor, "ok");
    fs::remove_dir_all(&w).ok();
}

#[test]
fn ohne_root_bleibt_eine_nicht_lesbare_pruefung_rot_mit_dem_hinweis_auf_sudo() {
    let w = eigener_ordner("ohne-root-rot");
    baum(&w, "1.2.3", false);
    nutzerdaten(&w, true);
    // The list of loaded profiles is not readable for a user: the lock is on and the list is missing, so the profile check
    // cannot be done.
    schreiben(&w, "usr/bin/bwrap", b"bwrap", 0o755);
    schreiben(
        &w,
        "proc/sys/kernel/apparmor_restrict_unprivileged_userns",
        b"1\n",
        0o644,
    );
    let s = Attrappe {
        antworten: vec![
            ("id", "-u", Some((0, format!("{}\n", eigene_uid())))),
            ("id", "-g", Some((0, "1000\n".to_owned()))),
            ("dpkg-query", "werkbank", Some((0, "1.2.3".to_owned()))),
            ("systemctl", "werkbank", None),
        ],
        aufrufe: RefCell::new(Vec::new()),
    };
    let lage = lage_ohne_root(&w, &s, Some("anna"));
    assert_eq!(lage.apparmor, "nicht_pruefbar");
    assert!(lage.nicht_pruefbar());
    let r = bewerte(&lage);
    assert_eq!(r.rc, 1);
    let n = r
        .zeilen
        .iter()
        .filter(|z| !z.starts_with("pruef.hinweis_") && !z.starts_with("pruef.rot"))
        .count();
    let hinweis = r
        .zeilen
        .iter()
        .position(|z| z.starts_with("pruef.hinweis_sudo: "))
        .expect("the note on sudo");
    // The note stands just before the last line and does not count in it.
    assert_eq!(hinweis + 2, r.zeilen.len(), "{:?}", r.zeilen);
    assert_eq!(r.zeilen.last(), Some(&format!("pruef.rot: {n} Befunde.")));
    // As root the same finding carries no note: `sudo` would not help.
    let mut als_root = lage.clone();
    als_root.ohne_root = false;
    assert!(
        bewerte(&als_root)
            .zeilen
            .iter()
            .all(|z| !z.starts_with("pruef.hinweis_sudo"))
    );
    fs::remove_dir_all(&w).ok();
}

#[test]
fn ohne_root_kommt_der_zustand_von_ubuntus_profil_aus_der_probe_des_system_bwrap() {
    let w = eigener_ordner("ubuntu-profil-ohne-root");
    apparmor_baum(&w, false, true, "");
    // The list of profiles is not readable for a user.
    fs::remove_file(w.join("sys/kernel/security/apparmor/profiles")).expect("remove");
    let zustand = |antwort: Option<(i32, &str)>, root: bool| {
        let s = attrappe_ohne_root(vec![(
            "/usr/bin/bwrap",
            "--unshare-user",
            antwort.map(|(rc, t)| (rc, t.to_owned())),
        )]);
        Umgebung {
            wurzel: w.clone(),
            nutzer: Some("anna".to_owned()),
            root,
            system: &s,
        }
        .lage()
        .ubuntu_profil
    };
    // Label of the child with `bwrap` or `unpriv_bwrap`: loaded (S1).
    assert_eq!(
        zustand(Some((0, "bwrap//&unpriv_bwrap (enforce)\n")), false),
        "geladen"
    );
    assert_eq!(
        zustand(Some((0, "unpriv_bwrap (enforce)\n")), false),
        "geladen"
    );
    assert_eq!(zustand(Some((0, "bwrap\n")), false), "geladen");
    // The probe fails with the uid map error: not loaded (S0).
    assert_eq!(
        zustand(
            Some((1, "bwrap: setting up uid map: Permission denied\n")),
            false
        ),
        "entladen"
    );
    // Anything else says nothing: another label, no label, another error, no answer.
    assert_eq!(zustand(Some((0, "unconfined\n")), false), "");
    assert_eq!(zustand(Some((0, "meinbwrap (enforce)\n")), false), "");
    assert_eq!(zustand(Some((0, "")), false), "");
    assert_eq!(
        zustand(Some((1, "bwrap: No such file or directory\n")), false),
        ""
    );
    assert_eq!(zustand(None, false), "");
    // As root the list of profiles is the source, never the probe: unreadable list says nothing.
    assert_eq!(zustand(Some((0, "bwrap (enforce)\n")), true), "");
    // Lock off: no line, whatever the probe says.
    apparmor_baum(&w, false, false, "");
    fs::remove_file(w.join("sys/kernel/security/apparmor/profiles")).expect("remove");
    assert_eq!(zustand(Some((0, "bwrap (enforce)\n")), false), "");
    fs::remove_dir_all(&w).ok();
}

/// The line of `lauf.json` as `postinst` writes it (one line, closed form); only `ca_zulassung` varies.
fn lauf_json_text(ca: &str) -> String {
    format!(
        concat!(
            r#"{{"fassung":1,"einstellungen":"/srv/beispiel/e.json","vault":"/srv/beispiel/v","protokoll":"/srv/beispiel/p","zustand":"/srv/beispiel/z","#,
            r#""laufzeit":"/srv/beispiel/l","sitzungen":"/srv/beispiel/s","heim":"/srv/beispiel/h","path":"/usr/bin","#,
            r#""programme":{{"rangkern":"/srv/beispiel/r"}},"harness":{{"copilot":"/opt/c","pi":"/opt/p"}},"programmordner":[],"#,
            r#""schluessel":{{"copilot":"/srv/beispiel/c","lokal":"/srv/beispiel/m"}},"ca_zulassung":"{}"}}"#,
            "\n"
        ),
        ca
    )
}

fn ca_lage(name: &str, schreibe: impl FnOnce(&Path)) -> (PathBuf, Lage) {
    let w = eigener_ordner(name);
    baum(&w, "1.2.3", false);
    nutzerdaten(&w, true);
    schreiben(&w, "usr/bin/bwrap", b"bwrap", 0o755);
    schreibe(&w);
    let lage = lage_mit(&w, &attrappe_ok(eigene_uid()), Some("anna"));
    (w, lage)
}

#[test]
fn ohne_lauf_json_gibt_es_keine_ca_zeile() {
    let (w, lage) = ca_lage("ca-keine", |_| {});
    assert_eq!(lage.ca_zulassung, "");
    let r = bewerte(&lage);
    assert_eq!(
        (r.rc, r.zeilen),
        (0, vec!["pruef.ok: Alle 8 Pruefungen bestanden.".to_owned()])
    );
    fs::remove_dir_all(&w).ok();
}

#[test]
fn jeder_der_drei_werte_gibt_eine_eigene_zeile_und_rc_bleibt_null() {
    for (wert, zeile) in [
        (
            "offen",
            "pruef.hinweis_ca_offen: Die Zulassung der lokalen CA ist offen; die Cloud-Route ist gesperrt. Das ist kein Fehler der Installation.",
        ),
        (
            "abgelehnt",
            "pruef.hinweis_ca_abgelehnt: Die Zulassung der lokalen CA ist abgelehnt; die Cloud-Route ist gesperrt.",
        ),
        (
            "zugelassen",
            "pruef.hinweis_ca_zugelassen: Die Zulassung der lokalen CA ist erteilt; die Cloud-Route ist nicht gesperrt.",
        ),
    ] {
        let (w, lage) = ca_lage(&format!("ca-{wert}"), |w| {
            schreiben(
                w,
                "etc/werkbank/lauf.json",
                lauf_json_text(wert).as_bytes(),
                0o644,
            );
        });
        assert_eq!(lage.ca_zulassung, wert);
        let r = bewerte(&lage);
        assert_eq!(
            (r.rc, r.zeilen),
            (
                0,
                vec![
                    zeile.to_owned(),
                    "pruef.ok: Alle 8 Pruefungen bestanden.".to_owned()
                ]
            ),
            "{wert}"
        );
        fs::remove_dir_all(&w).ok();
    }
}

#[test]
fn eine_lauf_json_ohne_klaren_wert_gilt_als_unbekannt_und_sperrt_die_cloud_route() {
    let gut = lauf_json_text("offen");
    let faelle: Vec<(&str, Vec<u8>)> = vec![
        ("kein-json", b"das ist kein json\n".to_vec()),
        ("wert-ja", gut.replace("\"offen\"", "\"ja\"").into_bytes()),
        (
            "wert-gross",
            gut.replace("\"offen\"", "\"Offen\"").into_bytes(),
        ),
        ("wert-zahl", gut.replace("\"offen\"", "1").into_bytes()),
        (
            "feld-fehlt",
            gut.replace("\"ca_zulassung\"", "\"ca_zulassung_x\"")
                .into_bytes(),
        ),
        ("leer", Vec::new()),
        (
            "zu-gross",
            format!("{}{}", gut.trim_end(), " ".repeat(70_000)).into_bytes(),
        ),
    ];
    for (name, inhalt) in faelle {
        let (w, lage) = ca_lage(&format!("ca-unbekannt-{name}"), |w| {
            schreiben(w, "etc/werkbank/lauf.json", &inhalt, 0o644);
        });
        assert_eq!(lage.ca_zulassung, "unbekannt", "{name}");
        let r = bewerte(&lage);
        assert_eq!(r.rc, 0, "{name}");
        assert!(
            r.zeilen
                .iter()
                .any(|z| z.starts_with("pruef.hinweis_ca_unbekannt: ")),
            "{name}"
        );
        fs::remove_dir_all(&w).ok();
    }
}

#[test]
fn ein_verweis_oder_ein_ordner_statt_der_datei_gilt_als_unbekannt() {
    let (w, lage) = ca_lage("ca-verweis", |w| {
        schreiben(
            w,
            "ziel.json",
            lauf_json_text("zugelassen").as_bytes(),
            0o644,
        );
        fs::create_dir_all(w.join("etc/werkbank")).expect("folder");
        symlink(w.join("ziel.json"), w.join("etc/werkbank/lauf.json")).expect("link");
    });
    assert_eq!(lage.ca_zulassung, "unbekannt");
    fs::remove_dir_all(&w).ok();
    let (w, lage) = ca_lage("ca-ordner", |w| {
        fs::create_dir_all(w.join("etc/werkbank/lauf.json")).expect("folder");
    });
    assert_eq!(lage.ca_zulassung, "unbekannt");
    fs::remove_dir_all(&w).ok();
}

#[test]
fn die_ca_zeile_zaehlt_nicht_als_befund_und_steht_nach_den_befunden() {
    let (w, lage) = ca_lage("ca-befund", |w| {
        schreiben(
            w,
            "etc/werkbank/lauf.json",
            lauf_json_text("offen").as_bytes(),
            0o644,
        );
        fs::remove_dir_all(w.join("var/lib/werkbank/anna/vorfuehr-vault")).expect("remove");
    });
    let r = bewerte(&lage);
    assert_eq!(r.rc, 1);
    assert_eq!(r.zeilen.len(), 3, "{:?}", r.zeilen);
    assert!(r.zeilen[0].starts_with("pruef.vault_fehlt: "));
    assert!(r.zeilen[1].starts_with("pruef.hinweis_ca_offen: "));
    assert_eq!(r.zeilen[2], "pruef.rot: 1 Befunde.");
    fs::remove_dir_all(&w).ok();
}

#[test]
fn eine_lage_mit_unbekanntem_ca_wert_wird_abgelehnt() {
    let lage = |ca: serde_json::Value| {
        serde_json::json!({
            "plattform": "linux", "konten": 1, "dienste": [], "apparmor": "ok", "einstellungen": {"zustand": "ok"},
            "nutzerordner": "ok", "schluessel": "ok", "copilot": "ok", "vault": "ok", "stelle": "ok",
            "paketstand": {"manifest": "ok", "bau": "auslieferung", "version": "gleich", "dateien": []},
            "ca_zulassung": ca
        })
    };
    for gut in ["zugelassen", "offen", "abgelehnt", "unbekannt"] {
        assert_eq!(
            Lage::aus_json(&lage(serde_json::json!(gut)))
                .expect("lage")
                .ca_zulassung,
            gut
        );
    }
    for schlecht in [
        serde_json::json!("ja"),
        serde_json::json!(""),
        serde_json::json!(1),
        serde_json::json!(null),
    ] {
        assert!(Lage::aus_json(&lage(schlecht)).is_err());
    }
}

/// `lauf.json` with the given `harness` object and `programmordner` list (JSON text); `heim` is `/srv/beispiel/h`.
fn lauf_json_mit(harness: &str, ordner: &str) -> String {
    lauf_json_text("offen")
        .replace(
            "\"harness\":{\"copilot\":\"/opt/c\",\"pi\":\"/opt/p\"}",
            &format!("\"harness\":{harness}"),
        )
        .replace(
            "\"programmordner\":[]",
            &format!("\"programmordner\":{ordner}"),
        )
}

#[test]
fn ein_doppelter_schluessel_ca_zulassung_gilt_wie_im_paket_als_offen_mit_eigener_zeile() {
    // alt (21.11. früh): ein doppelter Schlüssel war `unbekannt` (der Leser lehnte die Datei ab). Neu: `doppelt`, eigene Zeile; das Paket liest `offen`.
    let gut = lauf_json_text("offen");
    let doppelt = gut.replace(
        "\"fassung\":1",
        "\"ca_zulassung\":\"zugelassen\",\"fassung\":1",
    );
    // der Schlüsselname als Wert zählt auch (das Paket zählt den Text `"ca_zulassung"`)
    let als_wert = gut.replace("\"fassung\":1", "\"x\":\"ca_zulassung\",\"fassung\":1");
    for (name, inhalt) in [("doppelt", doppelt), ("als-wert", als_wert)] {
        let (w, lage) = ca_lage(&format!("ca-doppelt-{name}"), |w| {
            schreiben(w, "etc/werkbank/lauf.json", inhalt.as_bytes(), 0o644);
        });
        assert_eq!(lage.ca_zulassung, "doppelt", "{name}");
        let r = bewerte(&lage);
        assert_eq!(r.rc, 0, "{name}");
        assert!(
            r.zeilen
                .iter()
                .any(|z| z.starts_with("pruef.hinweis_ca_doppelt: ")),
            "{name}"
        );
        assert!(
            !r.zeilen
                .iter()
                .any(|z| z.starts_with("pruef.hinweis_ca_unbekannt")),
            "{name}"
        );
        fs::remove_dir_all(&w).ok();
    }
    // Gegenprobe: ein einfacher Schlüssel mit dem Wert `zugelassen` gibt `zugelassen`
    let (w, lage) = ca_lage("ca-doppelt-gegenprobe", |w| {
        schreiben(
            w,
            "etc/werkbank/lauf.json",
            lauf_json_text("zugelassen").as_bytes(),
            0o644,
        );
    });
    assert_eq!(lage.ca_zulassung, "zugelassen");
    fs::remove_dir_all(&w).ok();
}

#[test]
fn ein_programmordner_der_den_anmeldespeicher_beruehrt_ist_ein_befund_mit_festem_code() {
    for (name, ordner, soll) in [
        ("praefix", r#"["/srv/beispiel/h/.local"]"#, "verboten"),
        (
            "im-speicher",
            r#"["/srv/beispiel/h/.local/share/keyrings/x"]"#,
            "verboten",
        ),
        (
            "zweiter",
            r#"["/opt/pi","/srv/beispiel/h/.ssh"]"#,
            "verboten",
        ),
        (
            "paketwurzel",
            r#"["/srv/beispiel/h/.local/lib/node_modules/@scope/paket"]"#,
            "ok",
        ),
        ("leer", "[]", "ok"),
    ] {
        let text = lauf_json_mit("{}", ordner);
        let (w, lage) = ca_lage(&format!("progord-{name}"), |w| {
            schreiben(w, "etc/werkbank/lauf.json", text.as_bytes(), 0o644);
        });
        assert_eq!(lage.programmordner, soll, "{name}");
        let r = bewerte(&lage);
        if soll == "verboten" {
            assert_eq!(r.rc, 1, "{name}");
            assert!(
                r.zeilen
                    .iter()
                    .any(|z| z.starts_with("pruef.programmordner_verboten: ")),
                "{name}"
            );
        } else {
            assert!(
                !r.zeilen
                    .iter()
                    .any(|z| z.starts_with("pruef.programmordner_verboten")),
                "{name}"
            );
        }
        fs::remove_dir_all(&w).ok();
    }
}

#[test]
fn ein_harness_der_nicht_in_lauf_json_steht_nennt_seinen_grund() {
    let text = lauf_json_mit(r#"{"copilot":"/opt/c"}"#, "[]");
    let (w, lage) = ca_lage("harness-gruende", |w| {
        schreiben(w, "etc/werkbank/lauf.json", text.as_bytes(), 0o644);
    });
    // copilot steht in der Datei: keine Zeile; pi liegt nirgends: nicht gefunden
    assert_eq!(
        lage.harness,
        vec![("pi".to_owned(), "nicht_gefunden".to_owned())]
    );
    let r = bewerte(&lage);
    assert_eq!(r.rc, 0);
    assert!(
        r.zeilen
            .iter()
            .any(|z| z.starts_with("pruef.hinweis_harness_nicht_gefunden: pi nicht gefunden in "))
    );
    assert!(!r.zeilen.iter().any(|z| z.contains("copilot nicht")));
    // ein Programm an einem gesuchten Ort, dessen echter Pfad im Arbeitsordner liegt: ausserhalb
    schreiben(&w, "srv/beispiel/h/projekt/pi", b"#!/bin/sh\n", 0o755);
    fs::create_dir_all(w.join("srv/beispiel/h/.local/bin")).expect("bin");
    symlink(
        w.join("srv/beispiel/h/projekt/pi"),
        w.join("srv/beispiel/h/.local/bin/pi"),
    )
    .expect("link");
    let lage = lage_mit(&w, &attrappe_ok(eigene_uid()), Some("anna"));
    assert_eq!(
        lage.harness,
        vec![("pi".to_owned(), "ausserhalb".to_owned())]
    );
    // ein echtes Programm unter .local, das die Datei nicht nennt: nicht eingetragen
    fs::remove_file(w.join("srv/beispiel/h/.local/bin/pi")).expect("rm");
    schreiben(&w, "srv/beispiel/h/.local/bin/pi", b"#!/bin/sh\n", 0o755);
    let lage = lage_mit(&w, &attrappe_ok(eigene_uid()), Some("anna"));
    assert_eq!(
        lage.harness,
        vec![("pi".to_owned(), "nicht_eingetragen".to_owned())]
    );
    fs::remove_dir_all(&w).ok();
}

#[test]
fn programmordner_und_harness_der_lage_werden_geprueft() {
    let gut = serde_json::json!({
        "plattform": "linux", "konten": 1, "dienste": [], "apparmor": "ok", "einstellungen": {"zustand": "ok"},
        "nutzerordner": "ok", "schluessel": "ok", "copilot": "ok", "vault": "ok", "stelle": "ok",
        "paketstand": {"manifest": "ok", "bau": "auslieferung", "version": "gleich", "dateien": []}
    });
    let mit = |feld: &str, wert: serde_json::Value| {
        let mut l = gut.clone();
        l[feld] = wert;
        Lage::aus_json(&l)
    };
    for (feld, wert) in [
        ("programmordner", serde_json::json!("verboten")),
        ("programmordner", serde_json::json!("ok")),
        ("harness", serde_json::json!({"pi": "nicht_gefunden"})),
        (
            "harness",
            serde_json::json!({"copilot": "ausserhalb", "pi": "nicht_eingetragen"}),
        ),
    ] {
        assert!(mit(feld, wert.clone()).is_ok(), "{feld} {wert}");
    }
    for (feld, wert) in [
        ("programmordner", serde_json::json!("gelb")),
        ("programmordner", serde_json::json!(true)),
        ("harness", serde_json::json!({"pi": "gelb"})),
        ("harness", serde_json::json!({"pi": 1})),
        ("harness", serde_json::json!({"andere": "nicht_gefunden"})),
        ("harness", serde_json::json!(["pi"])),
    ] {
        assert!(mit(feld, wert.clone()).is_err(), "{feld} {wert}");
    }
}

/// `lauf.json` of `lauf_json_mit` padded with blanks to exactly `laenge` bytes (still valid JSON).
fn lauf_json_auf_laenge(harness: &str, ordner: &str, laenge: usize) -> Vec<u8> {
    let text = lauf_json_mit(harness, ordner);
    let kern = text.trim_end();
    assert!(kern.len() <= laenge);
    format!("{kern}{}", " ".repeat(laenge - kern.len())).into_bytes()
}

#[test]
fn eine_lauf_json_genau_an_der_grenze_wird_gelesen_und_eine_darueber_nicht() {
    use werkbank_pruefen::sammeln::LAUF_GRENZE;
    let ordner = r#"["/srv/beispiel/h/.local"]"#;
    let alle = r#"{"copilot":"/opt/c","pi":"/opt/p"}"#;
    for (name, laenge, ca, po) in [
        ("genau", LAUF_GRENZE, "offen", "verboten"),
        ("darueber", LAUF_GRENZE + 1, "unbekannt", "ok"),
    ] {
        let inhalt = lauf_json_auf_laenge(alle, ordner, laenge);
        let (w, lage) = ca_lage(&format!("grenze-{name}"), |w| {
            schreiben(w, "etc/werkbank/lauf.json", &inhalt, 0o644);
        });
        assert_eq!(lage.ca_zulassung, ca, "{name}");
        assert_eq!(lage.programmordner, po, "{name}");
        fs::remove_dir_all(&w).ok();
    }
}

#[test]
fn ein_heim_das_keine_ordnerform_hat_schaltet_die_pruefung_der_programmordner_ab() {
    let alle = r#"{"copilot":"/opt/c","pi":"/opt/p"}"#;
    // Jeder Fall nennt einen Programmordner, der unter dem eigenen Heim den Anmeldespeicher decken wuerde.
    for (name, heim, ordner, soll) in [
        (
            "gut",
            "/srv/beispiel/h",
            "/srv/beispiel/h/.local",
            "verboten",
        ),
        ("relativ", "h", "h/.local", "ok"),
        (
            "steuerzeichen",
            "/srv/beispiel/h\\u0001",
            "/srv/beispiel/h\\u0001/.local",
            "ok",
        ),
    ] {
        let text = lauf_json_mit(alle, &format!("[\"{ordner}\"]")).replace(
            "\"heim\":\"/srv/beispiel/h\"",
            &format!("\"heim\":\"{heim}\""),
        );
        let (w, lage) = ca_lage(&format!("heim-{name}"), |w| {
            schreiben(w, "etc/werkbank/lauf.json", text.as_bytes(), 0o644);
        });
        assert_eq!(lage.programmordner, soll, "{name}");
        fs::remove_dir_all(&w).ok();
    }
}

#[test]
fn die_suche_nach_dem_harness_sieht_nur_ausfuehrbare_dateien_und_erlaubte_wurzeln() {
    let text = lauf_json_mit(r#"{"copilot":"/opt/c"}"#, "[]");
    let (w, _) = ca_lage("harness-wurzeln", |w| {
        schreiben(w, "etc/werkbank/lauf.json", text.as_bytes(), 0o644);
    });
    let zustand = |w: &Path| {
        lage_mit(w, &attrappe_ok(eigene_uid()), Some("anna"))
            .harness
            .into_iter()
            .map(|(_, z)| z)
            .collect::<Vec<_>>()
    };
    // eine Datei ohne Ausfuehrrecht zaehlt nicht: nicht gefunden
    schreiben(&w, "srv/beispiel/h/.local/bin/pi", b"x", 0o644);
    assert_eq!(zustand(&w), ["nicht_gefunden"]);
    fs::remove_file(w.join("srv/beispiel/h/.local/bin/pi")).expect("rm");
    // jeder erlaubte Ort fuer den echten Pfad: nicht eingetragen (die Datei kennt das Programm nicht)
    for echt in [
        "opt/pi/pi",
        "usr/lib/pi/pi",
        "srv/beispiel/h/.volta/pi",
        "srv/beispiel/h/.npm-global/pi",
        "srv/beispiel/h/.nvm/pi",
    ] {
        schreiben(&w, echt, b"#!/bin/sh\n", 0o755);
        fs::create_dir_all(w.join("srv/beispiel/h/.local/bin")).expect("bin");
        symlink(w.join(echt), w.join("srv/beispiel/h/.local/bin/pi")).expect("link");
        assert_eq!(zustand(&w), ["nicht_eingetragen"], "{echt}");
        fs::remove_file(w.join("srv/beispiel/h/.local/bin/pi")).expect("rm");
    }
    // ein echter Pfad im Arbeitsordner und ein Ort ohne Programm
    schreiben(&w, "srv/beispiel/h/projekt/pi", b"#!/bin/sh\n", 0o755);
    symlink(
        w.join("srv/beispiel/h/projekt/pi"),
        w.join("srv/beispiel/h/.local/bin/pi"),
    )
    .expect("link");
    assert_eq!(zustand(&w), ["ausserhalb"]);
    fs::remove_file(w.join("srv/beispiel/h/.local/bin/pi")).expect("rm");
    assert_eq!(zustand(&w), ["nicht_gefunden"]);
    fs::remove_dir_all(&w).ok();
}

/// An empty list of refused folders (typed, so that `assert_eq!` needs no guess).
fn no_pairs() -> Vec<(String, String)> {
    Vec::new()
}

const ABGELEHNT_DATEI: &str = "var/lib/werkbank/programmordner-abgelehnt";
const ABHILFE: &str =
    "Abhilfe: Harness unter einem anderen Praefix installieren, dann dpkg-reconfigure werkbank.";

#[test]
fn die_zustandsdatei_der_abgelehnten_programmordner_gibt_einen_befund_je_harness() {
    // Gegenprobe: ohne die Datei gibt es weder einen Eintrag noch eine Zeile.
    let (w, lage) = ca_lage("abgelehnt-ohne", |_| {});
    assert_eq!(lage.abgelehnt, no_pairs());
    let r = bewerte(&lage);
    assert!(
        !r.zeilen
            .iter()
            .any(|z| z.starts_with("pruef.programmordner_verboten")),
        "{:?}",
        r.zeilen
    );
    fs::remove_dir_all(&w).ok();
    // Mit der Datei: ein Befund je Harness in der Reihenfolge copilot, pi; jede andere Zeile der Datei zaehlt nicht.
    for (name, inhalt, soll) in [
        (
            "pi",
            "pi /srv/beispiel/h/.local\n",
            vec![("pi", "/srv/beispiel/h/.local")],
        ),
        (
            "beide-umgekehrt",
            "pi /opt/p\ncopilot /opt/c\n",
            vec![("copilot", "/opt/c"), ("pi", "/opt/p")],
        ),
        (
            "doppelt-die-erste-gilt",
            "pi /opt/a\npi /opt/b\n",
            vec![("pi", "/opt/a")],
        ),
        ("fremdes-harness", "andere /opt/x\n", vec![]),
        ("relativ", "pi relativ\n", vec![]),
        ("blank-im-ordner", "pi /opt/a b\n", vec![]),
        ("ohne-ordner", "pi\n", vec![]),
        ("steuerzeichen", "pi /opt/a\u{7}\n", vec![]),
        ("leer", "", vec![]),
        (
            "gueltige-nach-ungueltiger",
            "x y\npi /opt/p\n",
            vec![("pi", "/opt/p")],
        ),
    ] {
        let (w, lage) = ca_lage(&format!("abgelehnt-{name}"), |w| {
            schreiben(w, ABGELEHNT_DATEI, inhalt.as_bytes(), 0o644);
        });
        let ist: Vec<(&str, &str)> = lage
            .abgelehnt
            .iter()
            .map(|(h, o)| (h.as_str(), o.as_str()))
            .collect();
        assert_eq!(ist, soll, "{name}");
        let r = bewerte(&lage);
        let zeilen: Vec<&String> = r
            .zeilen
            .iter()
            .filter(|z| z.starts_with("pruef.programmordner_verboten: Harness "))
            .collect();
        assert_eq!(zeilen.len(), soll.len(), "{name}");
        for (z, (h, o)) in zeilen.iter().zip(&soll) {
            assert_eq!(
                z.as_str(),
                format!(
                    "pruef.programmordner_verboten: Harness {h} nicht eingetragen. {ABHILFE} Ordner: {o}"
                ),
                "{name}"
            );
        }
        assert_eq!(r.rc, u8::from(!soll.is_empty()), "{name}");
        fs::remove_dir_all(&w).ok();
    }
}

#[test]
fn eine_zustandsdatei_die_kein_gewoehnliches_file_ist_oder_zu_gross_ist_gibt_nichts() {
    use werkbank_pruefen::sammeln::ABGELEHNT_GRENZE;
    // genau an der Grenze wird gelesen (Auffuellung mit Leerzeilen), eine Zeile darueber nicht
    let zeile = "pi /opt/p\n";
    for (name, laenge, soll) in [
        ("genau", ABGELEHNT_GRENZE, 1),
        ("darueber", ABGELEHNT_GRENZE + 1, 0),
    ] {
        let inhalt = format!("{zeile}{}", "\n".repeat(laenge - zeile.len()));
        let (w, lage) = ca_lage(&format!("abgelehnt-grenze-{name}"), |w| {
            schreiben(w, ABGELEHNT_DATEI, inhalt.as_bytes(), 0o644);
        });
        assert_eq!(lage.abgelehnt.len(), soll, "{name}");
        fs::remove_dir_all(&w).ok();
    }
    // ein Verweis wird nicht gefolgt
    let (w, lage) = ca_lage("abgelehnt-verweis", |w| {
        schreiben(w, "var/lib/werkbank/echt", b"pi /opt/p\n", 0o644);
        symlink(w.join("var/lib/werkbank/echt"), w.join(ABGELEHNT_DATEI)).expect("symlink");
    });
    assert_eq!(lage.abgelehnt, no_pairs());
    fs::remove_dir_all(&w).ok();
    // ein Ordner an der Stelle der Datei
    let (w, lage) = ca_lage("abgelehnt-ordner", |w| {
        fs::create_dir_all(w.join(ABGELEHNT_DATEI)).expect("dir");
    });
    assert_eq!(lage.abgelehnt, no_pairs());
    fs::remove_dir_all(&w).ok();
}

#[test]
fn ein_langer_ordner_wird_am_ende_gekuerzt_und_die_abhilfe_bleibt_ganz() {
    let ordner = format!("/srv/beispiel/{}", "a".repeat(150));
    let (w, lage) = ca_lage("abgelehnt-lang", |w| {
        schreiben(
            w,
            ABGELEHNT_DATEI,
            format!("copilot {ordner}\n").as_bytes(),
            0o644,
        );
    });
    let r = bewerte(&lage);
    let z = r
        .zeilen
        .iter()
        .find(|z| z.starts_with("pruef.programmordner_verboten: Harness copilot"))
        .expect("line");
    assert_eq!(z.len(), 200);
    assert!(z.contains(ABHILFE), "{z}");
    assert!(z.contains(" Ordner: /srv/beispiel/aaa"), "{z}");
    fs::remove_dir_all(&w).ok();
}

#[test]
fn ein_abgelehntes_harness_hat_keinen_zweiten_hinweis_das_andere_behaelt_seinen() {
    // Weder copilot noch pi liegen im Baum: beide haetten den Hinweis "nicht gefunden". pi ist abgelehnt: sein Hinweis entfaellt.
    let text = lauf_json_mit("{}", "[]");
    let (w, lage) = ca_lage("abgelehnt-hinweis", |w| {
        schreiben(w, "etc/werkbank/lauf.json", text.as_bytes(), 0o644);
        schreiben(w, ABGELEHNT_DATEI, b"pi /opt/p\n", 0o644);
    });
    let r = bewerte(&lage);
    assert!(
        r.zeilen
            .iter()
            .any(|z| z.starts_with("pruef.hinweis_harness_nicht_gefunden: copilot")),
        "{:?}",
        r.zeilen
    );
    assert!(
        !r.zeilen
            .iter()
            .any(|z| z.starts_with("pruef.hinweis_harness_nicht_gefunden: pi")),
        "{:?}",
        r.zeilen
    );
    // Gegenprobe ohne Datei: pi hat seinen Hinweis
    fs::remove_file(w.join(ABGELEHNT_DATEI)).expect("rm");
    let r = bewerte(&lage_mit(&w, &attrappe_ok(eigene_uid()), Some("anna")));
    assert!(
        r.zeilen
            .iter()
            .any(|z| z.starts_with("pruef.hinweis_harness_nicht_gefunden: pi")),
        "{:?}",
        r.zeilen
    );
    fs::remove_dir_all(&w).ok();
}

#[test]
fn die_lage_nimmt_abgelehnte_programmordner_nur_in_der_vertragsform() {
    let gut = serde_json::json!({
        "plattform": "linux", "konten": 1, "dienste": [], "apparmor": "ok", "einstellungen": {"zustand": "ok"},
        "nutzerordner": "ok", "schluessel": "ok", "copilot": "ok", "vault": "ok", "stelle": "ok",
        "paketstand": {"manifest": "ok", "bau": "auslieferung", "version": "gleich", "dateien": []}
    });
    let mit = |wert: serde_json::Value| {
        let mut l = gut.clone();
        l["programmordner_abgelehnt"] = wert;
        Lage::aus_json(&l)
    };
    let eintrag = |h: &str, o: &str| serde_json::json!({"harness": h, "ordner": o});
    // in der Reihenfolge der Ausgabe, auch wenn die Lage sie umgekehrt nennt
    let lage = mit(serde_json::json!([
        eintrag("pi", "/opt/p"),
        eintrag("copilot", "/opt/c")
    ]))
    .expect("ok");
    assert_eq!(
        lage.abgelehnt,
        vec![
            ("copilot".to_owned(), "/opt/c".to_owned()),
            ("pi".to_owned(), "/opt/p".to_owned())
        ]
    );
    assert_eq!(
        mit(serde_json::json!([])).expect("ok").abgelehnt,
        no_pairs()
    );
    for (name, schlecht) in [
        ("kein-array", serde_json::json!("pi")),
        (
            "fremdes-harness",
            serde_json::json!([eintrag("andere", "/opt/p")]),
        ),
        ("relativ", serde_json::json!([eintrag("pi", "opt/p")])),
        ("blank", serde_json::json!([eintrag("pi", "/opt/a b")])),
        (
            "doppelt",
            serde_json::json!([eintrag("pi", "/opt/a"), eintrag("pi", "/opt/b")]),
        ),
        (
            "zusatzfeld",
            serde_json::json!([{"harness": "pi", "ordner": "/opt/p", "x": 1}]),
        ),
        ("kein-objekt", serde_json::json!(["pi"])),
        (
            "zu-lang",
            serde_json::json!([eintrag("pi", &format!("/{}", "a".repeat(1024)))]),
        ),
    ] {
        assert!(mit(schlecht).is_err(), "{name}");
    }
}
