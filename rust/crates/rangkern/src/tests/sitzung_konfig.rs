//! The arguments of `rangkern sitzung`: what is a usage error (exit 64) and what the valid ones give.

use std::path::PathBuf;

use super::TempDir;
use crate::sitzung::konfig::{Konfig, Vektoren};
use crate::sitzung::mac::Plattform;

/// The vault of `dir` and the five required options, then `extra`.
fn args(dir: &TempDir, extra: &[&str]) -> Vec<String> {
    let mut args = vec![dir.path().to_string_lossy().to_string()];
    for teil in [
        "--socket",
        "/tmp/s.sock",
        "--route",
        "cloud",
        "--sitzung",
        "s-1",
        "--einstellungen",
        "/e.json",
        "--protokoll",
        "/p",
    ] {
        args.push(teil.to_string());
    }
    args.extend(extra.iter().map(ToString::to_string));
    args
}

/// The rules of contract 4 for Linux, on every system: the rules of macOS are in `sitzung_mac.rs`.
fn lesen(args: &[String]) -> Option<Konfig> {
    Konfig::lesen_fuer(Plattform::Linux, args)
}

#[test]
fn the_five_options_give_the_default_rights_and_no_vectors() {
    let dir = TempDir::new();
    let konfig = lesen(&args(&dir, &[])).expect("valid");
    assert_eq!(konfig.vault, dir.path());
    assert_eq!(konfig.socket, PathBuf::from("/tmp/s.sock"));
    assert_eq!(konfig.route, "cloud");
    assert_eq!(konfig.sitzung, "s-1");
    assert_eq!(konfig.einstellungen, PathBuf::from("/e.json"));
    assert_eq!(konfig.protokoll, PathBuf::from("/p"));
    assert_eq!(konfig.rechte, 0o600);
    assert!(matches!(konfig.vektoren, Vektoren::Datei(None)));
    assert!(konfig.embedder.is_none());
}

#[test]
fn the_rights_are_0600_or_0666_and_nothing_else() {
    let dir = TempDir::new();
    assert_eq!(
        lesen(&args(&dir, &["--socket-rechte", "0600"]))
            .expect("valid")
            .rechte,
        0o600
    );
    assert_eq!(
        lesen(&args(&dir, &["--socket-rechte", "0666"]))
            .expect("valid")
            .rechte,
        0o666
    );
    for schlecht in [
        "0644", "600", "666", "0o600", "0777", "", "0600 ", "00600", "0660",
    ] {
        assert!(
            lesen(&args(&dir, &["--socket-rechte", schlecht])).is_none(),
            "{schlecht:?}"
        );
    }
}

#[test]
fn a_required_option_is_missing() {
    let dir = TempDir::new();
    for fehlt in [
        "--socket",
        "--route",
        "--sitzung",
        "--einstellungen",
        "--protokoll",
    ] {
        let mut a = args(&dir, &[]);
        let at = a.iter().position(|x| x == fehlt).expect("option is there");
        a.drain(at..at + 2);
        assert!(lesen(&a).is_none(), "{fehlt}");
    }
}

#[test]
fn an_option_stands_at_most_once_and_only_in_full() {
    let dir = TempDir::new();
    for (opt, wert) in [
        ("--socket", "/tmp/x.sock"),
        ("--route", "cloud"),
        ("--sitzung", "s-2"),
        ("--einstellungen", "/f.json"),
        ("--protokoll", "/q"),
    ] {
        assert!(lesen(&args(&dir, &[opt, wert])).is_none(), "{opt} twice");
    }
    let zweimal = ["--socket-rechte", "0600", "--socket-rechte", "0600"];
    assert!(
        lesen(&args(&dir, &zweimal)).is_none(),
        "--socket-rechte twice"
    );
    for (opt, wert) in [
        ("--rout", "cloud"),
        ("--sock", "/tmp/y.sock"),
        ("-s", "x"),
        ("--embed-model", "m"),
        ("--keep-alive", "5m"),
        ("--query-vector", "/q.json"),
        ("--space", "kern"),
        ("--today", "2026-11-20"),
    ] {
        assert!(lesen(&args(&dir, &[opt, wert])).is_none(), "{opt}");
    }
    // A value in the form `--route=key` is the full spelling.
    let mut a = args(&dir, &[]);
    let at = a
        .iter()
        .position(|x| x == "--route")
        .expect("option is there");
    a.drain(at..at + 2);
    a.push("--route=lokal".to_string());
    assert_eq!(lesen(&a).expect("valid").route, "lokal");
}

#[test]
fn the_vault_is_a_folder_and_the_only_positional_argument() {
    let dir = TempDir::new();
    let mut a = args(&dir, &[]);
    a.remove(0);
    assert!(lesen(&a).is_none());
    let mut a = args(&dir, &[]);
    a.insert(1, dir.path().to_string_lossy().to_string());
    assert!(lesen(&a).is_none());
    let mut a = args(&dir, &[]);
    a[0] = dir.path().join("fehlt").to_string_lossy().to_string();
    assert!(lesen(&a).is_none());
    let datei = dir.write("datei.md", b"x");
    let mut a = args(&dir, &[]);
    a[0] = datei.to_string_lossy().to_string();
    assert!(lesen(&a).is_none());
    assert!(lesen(&[]).is_none());
}

#[test]
fn route_and_session_have_the_key_form() {
    let dir = TempDir::new();
    let lang = "a".repeat(64);
    let zu_lang = "a".repeat(65);
    for opt in ["--route", "--sitzung"] {
        let mit = |wert: &str| {
            let mut a = args(&dir, &[]);
            let at = a.iter().position(|x| x == opt).expect("option is there");
            a[at + 1] = wert.to_string();
            lesen(&a)
        };
        assert!(mit(&lang).is_some(), "{opt} 64");
        assert!(mit("a").is_some());
        assert!(mit("0.a_b-c").is_some());
        for schlecht in [
            "",
            "Cloud",
            "-a",
            ".a",
            "_a",
            "a b",
            "ä",
            "a/b",
            zu_lang.as_str(),
        ] {
            assert!(mit(schlecht).is_none(), "{opt} {schlecht:?}");
        }
    }
}

#[test]
fn the_options_for_vectors_and_embedding_are_those_of_serve() {
    let dir = TempDir::new();
    let konfig = lesen(&args(&dir, &["--vectors", "/v.json"])).expect("valid");
    match konfig.vektoren {
        Vektoren::Datei(Some(pfad)) => assert_eq!(pfad, PathBuf::from("/v.json")),
        andere @ (Vektoren::Datei(None) | Vektoren::Datenbanken(_)) => panic!("{andere:?}"),
    }
    let konfig =
        lesen(&args(&dir, &["--index-db", "/i.db", "--extra-db", "/x.db"])).expect("valid");
    match konfig.vektoren {
        Vektoren::Datenbanken(dbs) => {
            assert_eq!(dbs.index, PathBuf::from("/i.db"));
            assert_eq!(dbs.extra, Some(PathBuf::from("/x.db")));
        }
        andere @ Vektoren::Datei(_) => panic!("{andere:?}"),
    }
    let konfig = lesen(&args(
        &dir,
        &[
            "--index-db",
            "/i.db",
            "--embed",
            "http://127.0.0.1:11434",
            "--embed-timeout-ms",
            "250",
        ],
    ))
    .expect("valid");
    assert!(konfig.embedder.is_some());
    // What `serve` refuses is refused here too.
    for extra in [
        vec!["--extra-db", "/x.db"],
        vec!["--vectors", "/v.json", "--index-db", "/i.db"],
        vec!["--embed", "http://127.0.0.1:11434"],
        vec!["--embed-timeout-ms", "250"],
        vec![
            "--index-db",
            "/i.db",
            "--embed",
            "http://127.0.0.1:11434",
            "--embed-timeout-ms",
            "0",
        ],
        vec!["--index-db", "/i.db", "--embed", "http://example.com:11434"],
        vec!["--vectors", "/v.json", "--vectors", "/w.json"],
    ] {
        assert!(lesen(&args(&dir, &extra)).is_none(), "{extra:?}");
    }
}

const PROBE: [&str; 5] = [
    "--probelauf",
    "--marker",
    "MI-1A,MK-1B",
    "--marker-zaehlung",
    "/z.json",
];

#[test]
fn the_probe_run_needs_the_switch_the_markers_and_the_file_together() {
    let dir = TempDir::new();
    assert!(lesen(&args(&dir, &[])).expect("valid").probelauf.is_none());
    let probe = lesen(&args(&dir, &PROBE))
        .expect("valid")
        .probelauf
        .expect("the probe run");
    assert_eq!(probe.marker, ["MI-1A", "MK-1B"]);
    assert_eq!(probe.zaehlung, PathBuf::from("/z.json"));
    // The switch can stand anywhere among the options.
    let mut vorn = vec!["--probelauf"];
    vorn.extend(&PROBE[1..]);
    assert!(lesen(&args(&dir, &vorn)).is_some());
    // Every part alone, and every pair, is a usage error.
    for teil in [
        vec!["--probelauf"],
        vec!["--marker", "A"],
        vec!["--marker-zaehlung", "/z.json"],
        vec!["--probelauf", "--marker", "A"],
        vec!["--probelauf", "--marker-zaehlung", "/z.json"],
        vec!["--marker", "A", "--marker-zaehlung", "/z.json"],
    ] {
        assert!(lesen(&args(&dir, &teil)).is_none(), "{teil:?}");
    }
    // The switch twice, with a value, or an option twice.
    let mut zweimal = PROBE.to_vec();
    zweimal.push("--probelauf");
    assert!(lesen(&args(&dir, &zweimal)).is_none());
    let mut mit_wert = PROBE.to_vec();
    mit_wert[0] = "--probelauf=1";
    assert!(lesen(&args(&dir, &mit_wert)).is_none());
    let mut doppelt = PROBE.to_vec();
    doppelt.extend(["--marker", "X"]);
    assert!(lesen(&args(&dir, &doppelt)).is_none());
}

#[test]
fn a_marker_has_the_form_and_there_are_one_to_sixteen_of_them() {
    let dir = TempDir::new();
    let mit = |marker: &str| {
        lesen(&args(
            &dir,
            &[
                "--probelauf",
                "--marker",
                marker,
                "--marker-zaehlung",
                "/z.json",
            ],
        ))
    };
    let lang = "A".repeat(32);
    let zu_lang = "A".repeat(33);
    let sechzehn: Vec<String> = (0..16).map(|i| format!("M-{i}")).collect();
    let siebzehn: Vec<String> = (0..17).map(|i| format!("M-{i}")).collect();
    for gut in [
        "A",
        "0",
        "-",
        "MI-4F9",
        lang.as_str(),
        sechzehn.join(",").as_str(),
    ] {
        assert!(mit(gut).is_some(), "{gut}");
    }
    for schlecht in [
        "",
        ",",
        "A,",
        ",A",
        "A,,B",
        "mi-1",
        "Mi-1",
        "A B",
        "A_B",
        "Ä",
        "A;B",
        "A,A",
        zu_lang.as_str(),
        siebzehn.join(",").as_str(),
    ] {
        assert!(mit(schlecht).is_none(), "{schlecht:?}");
    }
    assert_eq!(
        mit(sechzehn.join(",").as_str())
            .expect("valid")
            .probelauf
            .expect("probe")
            .marker
            .len(),
        16
    );
}

#[test]
fn an_option_with_an_empty_value_is_a_usage_error() {
    let dir = TempDir::new();
    // Each of the five required options, with the empty text in its place.
    for name in [
        "--socket",
        "--route",
        "--sitzung",
        "--einstellungen",
        "--protokoll",
    ] {
        let mut a = args(&dir, &[]);
        let at = a.iter().position(|x| x == name).expect("option is there");
        a[at + 1] = String::new();
        assert!(lesen(&a).is_none(), "{name}");
    }
    // The optional ones too, also where the value alone would be valid for `serve`.
    for extra in [
        vec!["--socket-rechte", ""],
        vec!["--vectors", ""],
        vec!["--index-db", ""],
        vec!["--index-db", "/i.db", "--extra-db", ""],
        vec!["--index-db", "/i.db", "--embed", ""],
        vec![
            "--index-db",
            "/i.db",
            "--embed",
            "http://127.0.0.1:11434",
            "--embed-timeout-ms",
            "",
        ],
        vec![
            "--probelauf",
            "--marker",
            "",
            "--marker-zaehlung",
            "/z.json",
        ],
        vec!["--probelauf", "--marker", "A", "--marker-zaehlung", ""],
    ] {
        assert!(lesen(&args(&dir, &extra)).is_none(), "{extra:?}");
    }
    // The same options with a value are valid.
    assert!(lesen(&args(&dir, &["--vectors", "/v.json"])).is_some());
    assert!(lesen(&args(&dir, &["--index-db", "/i.db", "--extra-db", "/x.db"])).is_some());
}
