//! The search for the key (contract 8, section 9a): every case of `faelle/suche.json` as a real start of the program
//! (`modellweg suche ...`) in a throw-away folder, and again in the process through the library with a small read buffer, so
//! that the border between two reads is hit by the cases of the sizes 4096 and 65536. The file tree of a case is made
//! after the case (unreadable files with mode 000, a FIFO with `mkfifo`, links). No test uses a real key; the marks are
//! invented and not shaped like a token. As the user root every file is readable, so the cases with an unreadable file or
//! folder are skipped then and say so on stderr.
#![cfg(unix)] // the crate is Unix only (see `src/lib.rs`)

mod gemeinsam;

use std::ffi::OsString;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use gemeinsam::{Wegwerf, fall_datei, latin1, programm, text};
use serde_json::Value;

/// The folder the cases write below; the cases name it `/srv/beispiel`.
const BEISPIEL: &str = "/srv/beispiel";

/// Whether the cases with unreadable entries cannot be tested (root reads everything).
fn ist_root() -> bool {
    rustix::process::geteuid().is_root()
}

/// The user's own cleanup: makes every folder and file readable again so that the folder can be removed.
struct Freigabe(Vec<PathBuf>);

impl Drop for Freigabe {
    fn drop(&mut self) {
        for p in self.0.iter().rev() {
            let _ = fs::set_permissions(p, fs::Permissions::from_mode(0o700));
        }
    }
}

fn abbilden(s: &str, wurzel: &Path) -> String {
    s.replace(BEISPIEL, wurzel.to_str().expect("utf8"))
}

fn modus(v: &Value) -> u32 {
    u32::from_str_radix(text(v), 8).expect("octal mode")
}

/// Makes the tree of a case below `wurzel`; returns what has to be freed again before the folder goes.
fn aufbauen(fall: &Value, wurzel: &Path) -> Freigabe {
    let eintraege = fall["baum"].as_array().expect("tree");
    let ziel = |pfad: &str| -> Option<PathBuf> {
        let abgebildet = abbilden(pfad, wurzel);
        abgebildet
            .starts_with(wurzel.to_str().expect("utf8"))
            .then(|| PathBuf::from(abgebildet))
    };
    for e in eintraege.iter().filter(|e| e["art"] == "ordner") {
        if let Some(p) = ziel(text(&e["pfad"])) {
            fs::create_dir_all(&p).expect("folder");
        }
    }
    let mut sperren = Vec::new();
    for e in eintraege.iter().filter(|e| e["art"] != "ordner") {
        let p = ziel(text(&e["pfad"])).expect("inside the test folder");
        fs::create_dir_all(p.parent().expect("parent")).expect("parent folder");
        match text(&e["art"]) {
            "datei" => {
                fs::write(&p, latin1(text(&e["inhalt"]))).expect("file");
                let m = if e["lesbar"] == false {
                    sperren.push(p.clone());
                    0o000
                } else if e.get("modus").is_some() {
                    modus(&e["modus"])
                } else {
                    0o644
                };
                fs::set_permissions(&p, fs::Permissions::from_mode(m)).expect("chmod");
            }
            "verknuepfung" => {
                let z = abbilden(text(&e["ziel"]), wurzel);
                std::os::unix::fs::symlink(z, &p).expect("link");
            }
            "fifo" => {
                assert!(
                    Command::new("mkfifo")
                        .arg(&p)
                        .status()
                        .expect("mkfifo")
                        .success()
                );
            }
            art => panic!("unknown kind {art}"),
        }
    }
    if let Some(zl) = fall.get("zielliste") {
        let p = ziel(text(&zl["pfad"])).expect("inside the test folder");
        fs::write(&p, abbilden(text(&zl["inhalt"]), wurzel)).expect("target list");
        fs::set_permissions(&p, fs::Permissions::from_mode(0o600)).expect("chmod");
    }
    // Unreadable folders last, deepest first, after everything in them is made.
    let mut ordner: Vec<PathBuf> = eintraege
        .iter()
        .filter(|e| e["art"] == "ordner" && e["lesbar"] == false)
        .filter_map(|e| ziel(text(&e["pfad"])))
        .collect();
    ordner.sort_by_key(|p| std::cmp::Reverse(p.components().count()));
    for p in &ordner {
        fs::set_permissions(p, fs::Permissions::from_mode(0o000)).expect("chmod folder");
    }
    sperren.extend(ordner);
    Freigabe(sperren)
}

fn argv(fall: &Value, wurzel: &Path) -> Vec<OsString> {
    fall["argv"]
        .as_array()
        .expect("argv")
        .iter()
        .map(|a| OsString::from(abbilden(text(a), wurzel)))
        .collect()
}

/// The question to the holder of a case of the route Copilot (`halter` in the case): it answers per `halter_ziel`
/// either the four counts or a code, and with `erreichbar` `false` it is not reachable at all. The search of the
/// program takes the same question over the fixed endpoint of the holder; here the case stands in for him, because a
/// test reaches no holder.
fn halterfrage(
    fall: &Value,
) -> impl FnMut(&str, &[String]) -> Option<modellweg::halter::Suche> + use<> {
    let h = fall["halter"].clone();
    move |ziel: &str, pfade: &[String]| {
        assert!((1..=16).contains(&pfade.len()), "1 to 16 paths");
        assert!(pfade.iter().all(|p| p.starts_with('/')), "absolute paths");
        if h["erreichbar"] == false {
            return None;
        }
        let a = &h["antworten"][ziel];
        let zahl = |n: &str| a[n].as_u64();
        Some(modellweg::halter::Suche {
            dateien: zahl("dateien")?,
            nicht_lesbar: zahl("nicht_lesbar")?,
            treffer: zahl("treffer")?,
            uebersprungen: zahl("uebersprungen")?,
        })
    }
}

/// Whether the case asks the holder (route Copilot from step I).
fn ueber_halter(fall: &Value) -> bool {
    fall.get("halter").is_some()
}

fn braucht_unlesbares(fall: &Value) -> bool {
    fall["baum"]
        .as_array()
        .expect("tree")
        .iter()
        .any(|e| e["lesbar"] == false)
}

fn faelle() -> Vec<Value> {
    let f = fall_datei("suche.json");
    let faelle = f["faelle"].as_array().expect("cases").clone();
    assert_eq!(faelle.len(), 46);
    faelle
}

/// Runs the program as a process with an empty environment plus the one of the case.
fn prozess(args: &[OsString], umgebung: &[(String, String)]) -> (i32, Vec<u8>, Vec<u8>) {
    let mut cmd = Command::new(programm());
    cmd.args(args)
        .env_clear()
        .stdin(Stdio::null())
        .current_dir("/");
    for (k, v) in umgebung {
        cmd.env(k, v);
    }
    let aus = cmd.output().expect("start the program");
    (
        aus.status.code().expect("exit code"),
        aus.stdout,
        aus.stderr,
    )
}

fn umgebung(fall: &Value, wurzel: &Path) -> Vec<(String, String)> {
    fall["umgebung"]
        .as_object()
        .expect("environment")
        .iter()
        .map(|(k, v)| (k.clone(), abbilden(text(v), wurzel)))
        .collect()
}

fn pruefe(name: &str, fall: &Value, exit: i32, out: &[u8], err: &[u8]) {
    let e = &fall["erwartet"];
    assert_eq!(i64::from(exit), e["exit"].as_i64().expect("exit"), "{name}");
    assert_eq!(
        String::from_utf8_lossy(out),
        text(&e["stdout"]),
        "{name}: stdout"
    );
    assert_eq!(
        String::from_utf8_lossy(err),
        text(&e["stderr"]),
        "{name}: stderr"
    );
}

#[test]
#[allow(clippy::print_stderr)]
fn suche_cases_as_a_process() {
    let mut uebersprungen = Vec::new();
    for fall in faelle() {
        let name = text(&fall["name"]).to_owned();
        if (ist_root() && braucht_unlesbares(&fall)) || ueber_halter(&fall) {
            uebersprungen.push(name);
            continue;
        }
        let w = Wegwerf::neu("suche");
        let frei = aufbauen(&fall, w.ordner());
        let (exit, out, err) = prozess(&argv(&fall, w.ordner()), &umgebung(&fall, w.ordner()));
        pruefe(&name, &fall, exit, &out, &err);
        drop(frei);
    }
    if !uebersprungen.is_empty() {
        eprintln!(
            "skipped (unreadable entries as root, or a case that needs the holder): {uebersprungen:?}"
        );
    }
}

/// Every case that does not depend on stage 0, through the library with a read buffer of `puffer` bytes.
fn faelle_mit_puffer(puffer: usize) {
    for fall in faelle() {
        let name = text(&fall["name"]).to_owned();
        let args = fall["argv"].as_array().expect("argv");
        let erstes = args.first().map(text);
        if erstes != Some("suche") || (ist_root() && braucht_unlesbares(&fall)) {
            continue;
        }
        if text(&fall["erwartet"]["stderr"]).starts_with("mw.umgebung_unsicher") {
            continue;
        }
        let w = Wegwerf::neu("suche-lib");
        let frei = aufbauen(&fall, w.ordner());
        let a = argv(&fall, w.ordner());
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let uid = rustix::process::getuid().as_raw();
        let exit = if ueber_halter(&fall) {
            let mut frage = halterfrage(&fall);
            modellweg::suche::lauf_mit_halter(&a[1..], uid, puffer, &mut frage, &mut out, &mut err)
        } else {
            modellweg::suche::lauf(&a[1..], uid, puffer, &mut out, &mut err)
        };
        pruefe(
            &format!("{name} (puffer {puffer})"),
            &fall,
            i32::from(exit),
            &out,
            &err,
        );
        drop(frei);
    }
}

#[test]
fn suche_cases_with_a_buffer_of_one_byte() {
    faelle_mit_puffer(1);
}

#[test]
fn suche_cases_with_a_buffer_of_seven_bytes() {
    faelle_mit_puffer(7);
}

#[test]
fn suche_cases_with_a_buffer_of_4096_bytes() {
    faelle_mit_puffer(4096);
}

#[test]
fn suche_cases_with_a_buffer_of_65536_bytes() {
    faelle_mit_puffer(65_536);
}

#[test]
fn suche_cases_with_the_buffer_of_the_program() {
    faelle_mit_puffer(modellweg::suche::PUFFER);
}

#[test]
fn the_search_makes_no_file_and_no_socket_and_changes_nothing() {
    let fall = faelle()
        .into_iter()
        .find(|f| text(&f["name"]) == "fund_in_unterordner")
        .expect("case");
    let w = Wegwerf::neu("suche-keine-wirkung");
    let _frei = aufbauen(&fall, w.ordner());
    let vorher = baum(w.ordner());
    let (exit, _, _) = prozess(&argv(&fall, w.ordner()), &[]);
    assert_eq!(exit, 1);
    assert_eq!(baum(w.ordner()), vorher);
}

/// Every path below `p` with its mode and length, sorted.
fn baum(p: &Path) -> Vec<(String, u32, u64)> {
    fn gehe(p: &Path, aus: &mut Vec<(String, u32, u64)>) {
        for e in fs::read_dir(p).expect("read_dir").flatten() {
            let m = fs::symlink_metadata(e.path()).expect("meta");
            aus.push((
                e.path().display().to_string(),
                m.permissions().mode(),
                m.len(),
            ));
            if m.is_dir() {
                gehe(&e.path(), aus);
            }
        }
    }
    let mut aus = Vec::new();
    gehe(p, &mut aus);
    aus.sort();
    aus
}
