//! The seams of `main` exist only in the test build (feature `pruefbau`). This file runs in both builds: the delivered
//! build (without the feature) must ignore every seam, also with `WB_TESTUMGEBUNG=1`; the test build must follow them.
//!
//! What a seam changes is seen through the left-overs: the program clears stale left-overs first, in the folder of the
//! key (`WB_ANMELDEN_ZIEL`), below the root of the throw-away homes (`WB_ANMELDEN_WURZEL`) and, with `--uebernehmen`, in
//! the folder of the CLI (`WB_ANMELDEN_COPILOT_HEIM`). One stale left-over lies in each of the three throw-away folders.
#![cfg(unix)] // the crate is Unix only (see `src/lib.rs`)

mod gemeinsam;

use std::fs;
use std::path::Path;

use gemeinsam::{BIN, aufbau, ausgang, befehl, rest_anlegen, tote_pid};

const KEY_TEMP: &str = ".copilot.schluessel.";
const CONFIG_TEMP: &str = ".config.json.wb-anmelden-";

/// The names of the seams; none of them may stand in the delivered program.
const NAMEN: [&str; 6] = [
    "WB_TESTUMGEBUNG",
    "WB_ANMELDEN_ZIEL",
    "WB_ANMELDEN_COPILOT_HEIM",
    "WB_ANMELDEN_WURZEL",
    "WB_ANMELDEN_PROBE",
    "WB_ANMELDEN_PROBE_MARKE",
];

fn enthaelt(heu: &[u8], nadel: &str) -> bool {
    heu.windows(nadel.len()).any(|w| w == nadel.as_bytes())
}

#[test]
fn the_seams_count_only_in_the_test_build() {
    let a = aufbau();
    let tot = tote_pid();
    let rest_wegwerf = rest_anlegen(&a.wegwerf, tot);
    let rest_ziel = a.ziel.join(format!("{KEY_TEMP}{tot}.tmp"));
    fs::write(&rest_ziel, "rest\n").expect("left-over of the key");
    let rest_echt = a.echt.join(format!("{CONFIG_TEMP}{tot}.tmp"));
    fs::write(&rest_echt, "{}").expect("left-over of config.json");
    // HOME points away from the folder of the seam, so that only the seam can lead the program to `a.echt`.
    let heim = a.wurzel.join("heim");
    fs::create_dir_all(&heim).expect("home");
    let mut c = befehl(&a, &["copilot".to_owned(), "--uebernehmen".to_owned()]);
    c.env("HOME", &heim);
    let aus = ausgang(c.spawn().expect("spawn"));
    let liegen = [
        rest_wegwerf.exists(),
        rest_ziel.exists(),
        rest_echt.exists(),
    ];
    if cfg!(feature = "pruefbau") {
        // Counter-check: the test build clears the three left-overs in the folders of the seams.
        assert_eq!(liegen, [false, false, false], "{}", aus.stderr);
        assert!(
            aus.stderr
                .contains("Hinweis: Reste frueherer Anmeldungen wurden geloescht: 3\n"),
            "{}",
            aus.stderr
        );
    } else {
        // The delivered build never looks into the folders of the seams.
        assert_eq!(liegen, [true, true, true], "{}", aus.stderr);
        if !Path::new("/var/lib/werkbank").exists() && cfg!(target_os = "linux") {
            // Linux knows the user in `/etc/passwd`, so the fixed target stands; the source is the folder of the CLI below
            // HOME (empty), not the folder of the seam with its left-over: the call ends there.
            assert_eq!(aus.exit, 19, "{}", aus.stderr);
            assert!(
                aus.stderr.ends_with(
                    "werkbank-anmelden: anmelden.quelle_ohne_token: In der Konfiguration der Copilot CLI liegt nicht genau ein Token.\n"
                ),
                "{}",
                aus.stderr
            );
        } else if !Path::new("/var/lib/werkbank").exists() {
            // Without the folder of the package there is no target: the fixed path is used, not the seam.
            assert_eq!(aus.exit, 14, "{}", aus.stderr);
            assert_eq!(
                aus.stderr,
                "werkbank-anmelden: anmelden.ziel_nicht_schreibbar: Der Zielordner ist nicht schreibbar.\n"
            );
        }
    }
    fs::remove_dir_all(&a.wurzel).ok();
}

#[test]
fn the_delivered_program_holds_no_name_of_a_seam() {
    let programm = fs::read(BIN).expect("program readable");
    let gefunden: Vec<&str> = NAMEN
        .iter()
        .copied()
        .filter(|n| enthaelt(&programm, n))
        .collect();
    if cfg!(feature = "pruefbau") {
        // Counter-check: the test build knows every name, so the search finds what is there.
        assert_eq!(gefunden, NAMEN.to_vec());
    } else {
        assert!(gefunden.is_empty(), "names in the program: {gefunden:?}");
    }
}
