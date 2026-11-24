//! The 18 `v-*` cases of the contract (`rust/spec/peerkanal/faelle` and `erwartet`): settings checked for every system the
//! case names, on any machine.

use std::fs;
use std::path::PathBuf;

use peerkanal::{System, client_json_lesen, server_json_pruefen};
use serde_json::Value;

fn spec() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../spec/peerkanal")
}

fn lesen(pfad: &PathBuf) -> Value {
    let text = fs::read_to_string(pfad).unwrap_or_else(|e| panic!("{}: {e}", pfad.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", pfad.display()))
}

fn system(ort: &str) -> System {
    match ort {
        "windows" => System::Windows,
        "linux" => System::Linux,
        "mac" => System::Macos,
        _ => panic!("unbekannter Ort {ort}"),
    }
}

/// The code of the check of one case on one system: `None` when the settings hold.
fn pruefen(fall: &Value, sys: System) -> Option<String> {
    if let Some(server) = fall.get("server").filter(|s| !s.is_null())
        && let Err(f) = server_json_pruefen(server, sys)
    {
        return Some(f.code.als_str().to_owned());
    }
    if let Some(client) = fall.get("client").filter(|c| !c.is_null())
        && let Err(f) = client_json_lesen(client, sys)
    {
        return Some(f.code.als_str().to_owned());
    }
    None
}

#[test]
fn alle_vorgabe_faelle() {
    let mut namen: Vec<String> = fs::read_dir(spec().join("faelle"))
        .expect("faelle")
        .filter_map(|e| e.ok().map(|e| e.file_name().to_string_lossy().into_owned()))
        .filter(|n| {
            n.starts_with("v-")
                && std::path::Path::new(n)
                    .extension()
                    .is_some_and(|e| e == "json")
        })
        .collect();
    namen.sort();
    assert_eq!(namen.len(), 18, "{namen:?}");
    let mut fehler = Vec::new();
    for name in &namen {
        let fall = lesen(&spec().join("faelle").join(name));
        let erwartet = lesen(&spec().join("erwartet").join(name));
        assert_eq!(fall["art"], "vorgabe", "{name}");
        assert_eq!(erwartet["seite"], "vorgabe", "{name}");
        let soll = match erwartet["ergebnis"].as_str() {
            Some("ok") => None,
            Some("fehler") => Some(erwartet["code"].as_str().expect("code").to_owned()),
            _ => panic!("{name}: ergebnis"),
        };
        let orte = fall["ort"].as_array().expect("ort");
        assert!(!orte.is_empty(), "{name}");
        for ort in orte {
            let ort = ort.as_str().expect("ort");
            let ist = pruefen(&fall, system(ort));
            if ist != soll {
                fehler.push(format!("{name} auf {ort}: soll {soll:?}, ist {ist:?}"));
            }
        }
    }
    assert!(fehler.is_empty(), "{}", fehler.join("\n"));
}

/// A valid setting of one system is invalid on the other: a SID list under Unix, a uid list under Windows.
#[test]
fn gueltige_vorgabe_passt_nur_zum_eigenen_system() {
    for (name, fremd) in [
        ("v-gueltig-windows.json", System::Linux),
        ("v-gueltig-unix.json", System::Windows),
    ] {
        let fall = lesen(&spec().join("faelle").join(name));
        assert_eq!(
            pruefen(&fall, fremd).as_deref(),
            Some("pk.vorgabe_ungueltig"),
            "{name}"
        );
    }
}
