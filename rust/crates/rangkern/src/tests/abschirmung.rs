//! The mapping of `kernabbild::abschirmen` to the behaviour of `rangkern serve` and `rangkern sitzung`: one line and exit 5, or a note and
//! go on. The mapping is a pure function; no test here shields its own process.

use kernabbild::{Abbruch, Abgeschirmt, Hinweis};

use crate::abschirmung::{EXIT_ABSCHIRMUNG, abbilden, betrifft};

const BEFEHLE: [&str; 2] = ["serve", "sitzung"];

fn lauf(befehl: &str, ergebnis: Result<Abgeschirmt, Abbruch>) -> (Option<u8>, Vec<u8>) {
    let mut fehler = Vec::new();
    let code = abbilden(befehl, ergebnis, &mut fehler);
    (code, fehler)
}

#[test]
fn nothing_to_report_goes_on_without_a_byte() {
    for befehl in BEFEHLE {
        let (code, fehler) = lauf(befehl, Ok(Abgeschirmt { hinweis: None }));
        assert_eq!(code, None, "{befehl}");
        assert_eq!(fehler, b"", "{befehl}");
    }
}

#[test]
fn a_note_is_one_line_and_the_program_goes_on() {
    for befehl in BEFEHLE {
        for hinweis in [Hinweis::MusterDatei, Hinweis::MusterFehlt] {
            let (code, fehler) = lauf(
                befehl,
                Ok(Abgeschirmt {
                    hinweis: Some(hinweis),
                }),
            );
            assert_eq!(code, None, "{befehl} {hinweis:?}");
            assert_eq!(
                String::from_utf8(fehler).expect("text"),
                format!("rangkern {befehl}: {}\n", hinweis.text()),
                "{befehl} {hinweis:?}"
            );
        }
    }
}

#[test]
fn every_one_of_the_four_reasons_is_one_line_and_exit_5() {
    for befehl in BEFEHLE {
        for grund in [
            Abbruch::AbzugSperren,
            Abbruch::GrenzeSetzen,
            Abbruch::MusterPipe,
            Abbruch::MusterNichtLesbar,
        ] {
            let (code, fehler) = lauf(befehl, Err(grund));
            assert_eq!(code, Some(5), "{befehl} {grund:?}");
            assert_eq!(
                String::from_utf8(fehler).expect("text"),
                format!(
                    "rangkern {befehl}: Kernabbild nicht abschirmbar: {} ({})\n",
                    grund.grund(),
                    grund.abhilfe()
                ),
                "{befehl} {grund:?}"
            );
        }
    }
}

#[test]
fn the_code_5_is_none_of_the_other_codes_of_the_two_commands() {
    assert_eq!(EXIT_ABSCHIRMUNG, 5);
    // 0 success, 2 usage of `serve`, 64 usage of `sitzung`, 3 unreadable, 74 socket, 78 settings.
    for anderer in [0, 2, 3, 64, 74, 78] {
        assert_ne!(EXIT_ABSCHIRMUNG, anderer);
    }
}

#[test]
fn only_serve_and_sitzung_take_the_step() {
    assert!(betrifft("serve"));
    assert!(betrifft("sitzung"));
    for anderer in [
        "search",
        "tokens",
        "snippet",
        "--licenses",
        "",
        "Serve",
        "serve ",
    ] {
        assert!(!betrifft(anderer), "{anderer:?}");
    }
}

/// No in-process call of `serve` or `sitzung` shields the test process: the limit and the dump flag stay as they were.
#[cfg(target_os = "linux")]
#[test]
fn a_call_in_the_test_process_leaves_the_limit_and_the_dump_flag_alone() {
    use rustix::process::{DumpableBehavior, Resource, dumpable_behavior, getrlimit};

    use super::TempDir;
    use crate::cli::run_with_input;

    let vorher = (
        getrlimit(Resource::Core),
        dumpable_behavior().expect("dump flag"),
    );
    let dir = TempDir::new();
    let vault = dir.path().join("vault");
    std::fs::create_dir_all(&vault).expect("vault");
    let vault = vault.to_string_lossy().into_owned();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = run_with_input(
        &["serve".to_string(), vault],
        &mut std::io::empty(),
        &mut out,
        &mut err,
    );
    assert_eq!(code, 0);
    let code = crate::sitzung::run(&["x".to_string()], &mut out, &mut err);
    assert_eq!(code, 64);
    let nachher = (
        getrlimit(Resource::Core),
        dumpable_behavior().expect("dump flag"),
    );
    assert_eq!(
        (vorher.0.current, vorher.0.maximum),
        (nachher.0.current, nachher.0.maximum)
    );
    assert_eq!(vorher.1, nachher.1);
    assert_ne!(nachher.1, DumpableBehavior::NotDumpable);
}
