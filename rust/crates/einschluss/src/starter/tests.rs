//! Tests of the starter that need no Linux: the two calls, the input limit, the refusals of the description, the
//! table of the exit codes and the fixed lines, the arguments of the self test and the command of the program.

use std::io::{self, Read};

use super::*;
use crate::beschreibung_schreiben;
use crate::{Cloud, Umgebung, Verteilung, Zustand};

fn args(teile: &[&str]) -> Vec<String> {
    teile.iter().map(|t| (*t).to_owned()).collect()
}

fn muster() -> Beschreibung {
    Beschreibung {
        harness: "claude".to_owned(),
        arbeitsordner: "/srv/beispiel/arbeit".into(),
        programmordner: vec!["/srv/beispiel/bin".into()],
        zustand: Zustand {
            host: "/srv/beispiel/zustand".into(),
            name: ".claude".to_owned(),
            fortsetzen: false,
        },
        heim: "/srv/beispiel/heim".into(),
        such_socket: "/srv/beispiel/s/such.sock".into(),
        modell_socket: Some("/srv/beispiel/s/modell.sock".into()),
        lokal_weiter: None,
        dateien: Vec::new(),
        cloud: None,
        paket: None,
        umgebung: Umgebung {
            home: "/srv/beispiel/heim".to_owned(),
            path: "/usr/bin".to_owned(),
            term: "xterm".to_owned(),
            lang: "C.UTF-8".to_owned(),
            tmpdir: "/tmp".to_owned(),
        },
        geschuetzt: vec!["/srv/beispiel/vault".into()],
        argv: vec!["/srv/beispiel/bin/h".to_owned()],
        system: System::usr_merge(1000),
    }
}

fn bauen(b: &Beschreibung) -> Result<Profil, Abbruch> {
    profil_bauen(
        beschreibung_schreiben(b).as_bytes(),
        System::usr_merge(1000),
    )
}

#[test]
fn only_the_two_fixed_calls_are_taken() {
    assert_eq!(
        aufruf_lesen(&args(&["selbsttest"])),
        Some(Aufruf::Selbsttest)
    );
    assert_eq!(
        aufruf_lesen(&args(&["start", "--beschreibung-fd", "3"])),
        Some(Aufruf::Start)
    );
}

#[test]
fn every_other_call_is_refused() {
    let nein: &[&[&str]] = &[
        &[],
        &[""],
        &["selbsttest", "extra"],
        &["selbsttest", ""],
        &["Selbsttest"],
        &["start"],
        &["start", "--beschreibung-fd"],
        &["start", "--beschreibung-fd", "4"],
        &["start", "--beschreibung-fd", "03"],
        &["start", "--beschreibung-fd", "0"],
        &["start", "--beschreibung-fd", "3", "extra"],
        &["start", "--beschreibung-fd=3"],
        &["start", "--beschreibung-fd", "3 "],
        &["start", "--fd", "3"],
        &["start", "3"],
        &["roh"],
        &["roh", "--bind", "/", "/"],
        &["start", "--bind", "/", "/"],
        &["start", "--beschreibung-fd", "3", "--", "/bin/sh"],
        &["--beschreibung-fd", "3", "start"],
        &["selbsttest", "start", "--beschreibung-fd", "3"],
        &["/usr/bin/bwrap"],
        &["--help"],
    ];
    for a in nein {
        assert_eq!(aufruf_lesen(&args(a)), None, "{a:?}");
    }
}

/// A reader that gives its bytes and then the error.
struct Kaputt(usize);

impl Read for Kaputt {
    fn read(&mut self, puffer: &mut [u8]) -> io::Result<usize> {
        if self.0 == 0 {
            return Err(io::Error::other("kaputt"));
        }
        let n = self.0.min(puffer.len());
        puffer[..n].fill(b'x');
        self.0 -= n;
        Ok(n)
    }
}

#[test]
fn the_input_is_read_to_its_end_up_to_the_limit() {
    assert_eq!(eingabe_lesen(&b""[..]), Ok(vec![]));
    assert_eq!(eingabe_lesen(&b"abc"[..]), Ok(b"abc".to_vec()));
    let genau = vec![b'a'; EINGABE_MAX];
    assert_eq!(eingabe_lesen(&genau[..]), Ok(genau.clone()));
}

#[test]
fn one_byte_over_the_limit_is_refused() {
    let zu_gross = vec![b'a'; EINGABE_MAX + 1];
    assert_eq!(eingabe_lesen(&zu_gross[..]), Err(Eingabe::ZuGross));
    let sehr_gross = vec![b'a'; EINGABE_MAX * 4];
    assert_eq!(eingabe_lesen(&sehr_gross[..]), Err(Eingabe::ZuGross));
}

#[test]
fn the_reading_does_not_take_more_than_one_byte_over_the_limit() {
    // an endless source: the call must end by the limit, not by the end of the source
    struct Endlos;
    impl Read for Endlos {
        fn read(&mut self, puffer: &mut [u8]) -> io::Result<usize> {
            puffer.fill(b'a');
            Ok(puffer.len())
        }
    }
    assert_eq!(eingabe_lesen(Endlos), Err(Eingabe::ZuGross));
}

#[test]
fn the_input_with_a_time_limit_reads_a_fast_source() {
    let text = b"abc".to_vec();
    assert_eq!(
        eingabe_lesen_frist(io::Cursor::new(text.clone()), Duration::from_secs(30)),
        Ok(text)
    );
    assert_eq!(
        eingabe_lesen_frist(
            io::Cursor::new(vec![b'a'; EINGABE_MAX + 1]),
            Duration::from_secs(30)
        ),
        Err(Eingabe::ZuGross)
    );
}

#[test]
fn the_input_with_a_time_limit_gives_up_on_a_source_that_never_ends() {
    struct Langsam;
    impl Read for Langsam {
        fn read(&mut self, _puffer: &mut [u8]) -> io::Result<usize> {
            std::thread::sleep(Duration::from_secs(3));
            Ok(0)
        }
    }
    let start = std::time::Instant::now();
    assert_eq!(
        eingabe_lesen_frist(Langsam, Duration::from_millis(100)),
        Err(Eingabe::NichtLesbar)
    );
    assert!(start.elapsed() < Duration::from_secs(2));
    assert_eq!(EINGABE_FRIST, Duration::from_secs(5));
}

#[test]
fn an_unreadable_source_is_refused() {
    assert_eq!(eingabe_lesen(Kaputt(10)), Err(Eingabe::NichtLesbar));
    assert_eq!(eingabe_lesen(Kaputt(0)), Err(Eingabe::NichtLesbar));
}

#[test]
fn a_valid_description_builds_a_profile() {
    let profil = bauen(&muster()).expect("builds");
    assert_eq!(profil.bwrap(), Path::new(BWRAP_PFAD));
    assert_eq!(profil.beschreibung().argv, vec!["/srv/beispiel/bin/h"]);
}

#[test]
fn text_that_is_no_json_is_an_input_refusal() {
    assert_eq!(
        profil_bauen(b"kein json", System::usr_merge(1000)).err(),
        Some(Abbruch::Eingabe(Eingabe::Ungueltig))
    );
    assert_eq!(
        profil_bauen(b"", System::usr_merge(1000)).err(),
        Some(Abbruch::Eingabe(Eingabe::Ungueltig))
    );
    let mut text = beschreibung_schreiben(&muster());
    text.insert_str(1, "\"extra\":1,");
    assert_eq!(
        profil_bauen(text.as_bytes(), System::usr_merge(1000)).err(),
        Some(Abbruch::Eingabe(Eingabe::Ungueltig))
    );
}

#[test]
fn a_relative_program_is_refused() {
    for programm in ["h", "bin/h", "./h", "../h", ""] {
        let mut b = muster();
        b.argv[0] = programm.to_owned();
        assert!(
            matches!(
                bauen(&b),
                Err(Abbruch::Fehler(Fehler::BeschreibungUngueltig(_)))
            ),
            "{programm:?}"
        );
    }
}

#[test]
fn a_program_path_with_dots_or_double_slashes_is_refused() {
    for programm in [
        "/srv/beispiel/bin/../h",
        "/srv/beispiel//bin/h",
        "/srv/beispiel/./h",
        "/srv/beispiel/bin/",
        "/",
        "/srv/beispiel/bin/h\n",
    ] {
        let mut b = muster();
        b.argv[0] = programm.to_owned();
        assert!(
            matches!(
                bauen(&b),
                Err(Abbruch::Fehler(Fehler::BeschreibungUngueltig(_)))
            ),
            "{programm:?}"
        );
    }
}

#[test]
fn an_empty_argv_is_refused_by_the_form_check() {
    let mut b = muster();
    b.argv.clear();
    assert!(matches!(
        bauen(&b),
        Err(Abbruch::Fehler(Fehler::BeschreibungUngueltig("argv")))
    ));
}

#[test]
fn a_path_of_the_description_with_dots_or_double_slashes_is_refused() {
    for ungueltig in [
        "/srv/beispiel/../arbeit",
        "/srv/beispiel//arbeit",
        "srv/beispiel/arbeit",
        "/srv/beispiel/arbeit/",
    ] {
        let mut b = muster();
        b.arbeitsordner = ungueltig.into();
        assert!(
            matches!(
                bauen(&b),
                Err(Abbruch::Fehler(Fehler::BeschreibungUngueltig(
                    "arbeitsordner"
                )))
            ),
            "{ungueltig:?}"
        );
    }
}

#[test]
fn every_check_of_the_crate_runs_in_the_starter() {
    // the deny lists
    let mut b = muster();
    b.arbeitsordner = "/usr/lib".into();
    assert!(matches!(
        bauen(&b),
        Err(Abbruch::Fehler(Fehler::Verboten(_)))
    ));
    let mut b = muster();
    b.arbeitsordner = "/srv/beispiel/heim/.ssh".into();
    assert!(matches!(
        bauen(&b),
        Err(Abbruch::Fehler(Fehler::Verboten(_)))
    ));
    let mut b = muster();
    b.programmordner = vec!["/srv/beispiel/heim/.config/gh".into()];
    assert!(matches!(
        bauen(&b),
        Err(Abbruch::Fehler(Fehler::Verboten(_)))
    ));
    // the overlaps
    let mut b = muster();
    b.arbeitsordner = "/srv/beispiel/vault/p".into();
    assert!(matches!(
        bauen(&b),
        Err(Abbruch::Fehler(Fehler::Ueberlappt(_)))
    ));
    let mut b = muster();
    b.programmordner = vec!["/srv/beispiel/arbeit/bin".into()];
    b.zustand.host = "/srv/beispiel/arbeit/bin/z".into();
    assert!(matches!(
        bauen(&b),
        Err(Abbruch::Fehler(Fehler::Ueberlappt(_)))
    ));
    // the cloud rules
    let mut b = muster();
    b.cloud = Some(Cloud {
        weiterreicher: "/srv/beispiel/s/w.sock".into(),
        weiter_programm: "/srv/beispiel/w".into(),
        bundle: "/srv/beispiel/b.pem".into(),
        verteilung: Verteilung::Ubuntu,
    });
    assert!(matches!(
        bauen(&b),
        Err(Abbruch::Fehler(Fehler::BeschreibungUngueltig(_)))
    ));
}

#[test]
fn the_table_of_exit_codes_and_lines() {
    let fest = "lauf.einschluss_fehlt: Einschluss nicht verfuegbar";
    let tabelle: Vec<(Abbruch, u8, &str)> = vec![
        (Abbruch::Aufruf, 2, "werkbank-einschluss: Aufruf ungueltig"),
        (Abbruch::Eingabe(Eingabe::NichtLesbar), 3, fest),
        (Abbruch::Eingabe(Eingabe::KeinRohr), 3, fest),
        (Abbruch::Eingabe(Eingabe::ZuGross), 3, fest),
        (Abbruch::Eingabe(Eingabe::Ungueltig), 3, fest),
        (
            Abbruch::Fehler(Fehler::ArbeitsordnerFehlt),
            4,
            "lauf.ordner_fehlt: Arbeitsordner fehlt oder ist kein Ordner",
        ),
        (Abbruch::Fehler(Fehler::BeschreibungUngueltig("x")), 5, fest),
        (Abbruch::Fehler(Fehler::Verboten("x")), 5, fest),
        (Abbruch::Fehler(Fehler::Ueberlappt("x")), 5, fest),
        (Abbruch::Fehler(Fehler::QuelleUnbrauchbar("x")), 5, fest),
        (Abbruch::Fehler(Fehler::NichtVerfuegbar), 6, fest),
        (Abbruch::Fehler(Fehler::BwrapFehlt), 6, fest),
        (Abbruch::Fehler(Fehler::BwrapNichtAusfuehrbar), 6, fest),
        (Abbruch::Fehler(Fehler::TiocstiOffen), 6, fest),
        (Abbruch::Fehler(Fehler::NamensraumGesperrt), 7, fest),
        (
            Abbruch::Fehler(Fehler::Probelauf {
                status: Some(1),
                meldung: "pfad /geheim".to_owned(),
            }),
            8,
            fest,
        ),
        (Abbruch::Exec, 9, fest),
    ];
    for (abbruch, code, zeile) in tabelle {
        assert_eq!(abbruch.exitcode(), code, "{abbruch:?}");
        assert_eq!(abbruch.zeile(), zeile, "{abbruch:?}");
        assert!(!abbruch.zeile().contains('\n'));
    }
}

/// The second line of the starter: the exit code and one fixed word per check that refused. No path, no detail text.
#[test]
fn the_second_line_names_the_exit_code_and_the_check() {
    let tabelle: Vec<(Abbruch, Option<&str>)> = vec![
        (Abbruch::Aufruf, None),
        (
            Abbruch::Eingabe(Eingabe::NichtLesbar),
            Some("lauf.einschluss_teil: 3 eingabe-nicht-lesbar"),
        ),
        (
            Abbruch::Eingabe(Eingabe::KeinRohr),
            Some("lauf.einschluss_teil: 3 eingabe-kein-rohr"),
        ),
        (
            Abbruch::Eingabe(Eingabe::ZuGross),
            Some("lauf.einschluss_teil: 3 eingabe-zu-gross"),
        ),
        (
            Abbruch::Eingabe(Eingabe::Ungueltig),
            Some("lauf.einschluss_teil: 3 eingabe-ungueltig"),
        ),
        (Abbruch::Fehler(Fehler::ArbeitsordnerFehlt), None),
        (
            Abbruch::Fehler(Fehler::BeschreibungUngueltig("pfad /geheim")),
            Some("lauf.einschluss_teil: 5 beschreibung-ungueltig"),
        ),
        (
            Abbruch::Fehler(Fehler::Verboten("pfad /geheim")),
            Some("lauf.einschluss_teil: 5 verboten"),
        ),
        (
            Abbruch::Fehler(Fehler::Ueberlappt("pfad /geheim")),
            Some("lauf.einschluss_teil: 5 ueberlappt"),
        ),
        (
            Abbruch::Fehler(Fehler::QuelleUnbrauchbar("pfad /geheim")),
            Some("lauf.einschluss_teil: 5 quelle-unbrauchbar"),
        ),
        (
            Abbruch::Fehler(Fehler::NichtVerfuegbar),
            Some("lauf.einschluss_teil: 6 keine-plattform"),
        ),
        (
            Abbruch::Fehler(Fehler::BwrapFehlt),
            Some("lauf.einschluss_teil: 6 bwrap-fehlt"),
        ),
        (
            Abbruch::Fehler(Fehler::BwrapNichtAusfuehrbar),
            Some("lauf.einschluss_teil: 6 bwrap-nicht-ausfuehrbar"),
        ),
        (
            Abbruch::Fehler(Fehler::TiocstiOffen),
            Some("lauf.einschluss_teil: 6 tiocsti-offen"),
        ),
        (
            Abbruch::Fehler(Fehler::NamensraumGesperrt),
            Some("lauf.einschluss_teil: 7 namensraum-gesperrt"),
        ),
        (
            Abbruch::Fehler(Fehler::Probelauf {
                status: Some(1),
                meldung: "pfad /geheim".to_owned(),
            }),
            Some("lauf.einschluss_teil: 8 probelauf"),
        ),
        (
            Abbruch::Exec,
            Some("lauf.einschluss_teil: 9 exec-gescheitert"),
        ),
    ];
    for (abbruch, soll) in tabelle {
        assert_eq!(abbruch.teil_zeile().as_deref(), soll, "{abbruch:?}");
    }
}

#[test]
fn the_selbsttest_has_the_fixed_enclosure_and_no_input() {
    let a = selbsttest_argumente(&System::usr_merge(1000));
    let text = a.join(" ");
    assert!(
        text.starts_with(
            "--unshare-user --disable-userns --unshare-pid --unshare-net --unshare-ipc --unshare-uts \
             --unshare-cgroup --die-with-parent --cap-drop ALL --clearenv --ro-bind /usr /usr \
             --symlink usr/bin /bin"
        ),
        "{text}"
    );
    assert!(
        text.ends_with("--proc /proc --dev /dev --tmpfs /tmp -- /usr/bin/true"),
        "{text}"
    );
    assert!(text.contains("--ro-bind /etc/passwd /etc/passwd"), "{text}");
    for verboten in [
        "--setenv",
        "--share-net",
        "--cap-add",
        "--bind ",
        "--new-session",
        "--userns",
    ] {
        assert!(!text.contains(verboten), "{verboten}: {text}");
    }
}

#[test]
fn the_selbsttest_arguments_follow_the_host_layout() {
    let leer = System {
        uid: 1,
        symlinks: vec![],
        ro_binds: vec![],
    };
    assert_eq!(
        selbsttest_argumente(&leer).join(" "),
        "--unshare-user --disable-userns --unshare-pid --unshare-net --unshare-ipc --unshare-uts \
         --unshare-cgroup --die-with-parent --cap-drop ALL --clearenv --ro-bind /usr /usr --proc /proc \
         --dev /dev --tmpfs /tmp -- /usr/bin/true"
    );
}

#[test]
fn the_command_of_the_program_has_the_fixed_arguments_and_no_environment() {
    let befehl = start_befehl(Path::new("/srv/beispiel/starter"));
    assert_eq!(befehl.get_program(), "/srv/beispiel/starter");
    let a: Vec<_> = befehl.get_args().collect();
    assert_eq!(a, ["start", "--beschreibung-fd", "3"]);
    assert!(befehl.get_envs().next().is_none());
}

#[test]
fn the_fixed_path_of_the_starter() {
    assert_eq!(STARTER_PFAD, "/usr/lib/werkbank/werkbank-einschluss");
    assert_eq!(EINGABE_FD, "3");
}
