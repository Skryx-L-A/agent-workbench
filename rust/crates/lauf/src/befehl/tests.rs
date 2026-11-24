//! Tests of the command: the arguments, the path, each row of the table, the order of the steps and the line.

use std::ffi::OsString;
use std::os::unix::ffi::OsStringExt;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use super::*;

const KENNUNG: &str = "a1b2c3d4e5f60718293a4b5c6d7e8f90";

fn os(a: &[&str]) -> Vec<OsString> {
    a.iter().map(OsString::from).collect()
}

fn klassen() -> Vec<String> {
    ["oeffentlich", "intern", "kundendaten"]
        .map(str::to_owned)
        .to_vec()
}

static ZAEHLER: AtomicU32 = AtomicU32::new(0);

/// A throw-away world: state folder (0700), log folder, a working folder.
struct Welt {
    wurzel: PathBuf,
    lage: Lage,
    projekt: String,
}

impl Welt {
    fn neu(klasse: &str, kunde: Option<&str>) -> Welt {
        let n = ZAEHLER.fetch_add(1, Ordering::SeqCst);
        let basis = std::fs::canonicalize(std::env::temp_dir()).expect("tmp");
        let wurzel = basis.join(format!("lauf-befehl-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&wurzel);
        for (p, m) in [
            ("", 0o700),
            ("zustand", 0o700),
            ("protokoll", 0o700),
            ("projekt", 0o755),
        ] {
            let d = wurzel.join(p);
            std::fs::create_dir_all(&d).expect("folder");
            std::fs::set_permissions(&d, std::fs::Permissions::from_mode(m)).expect("mode");
        }
        let projekt = wurzel.join("projekt").to_string_lossy().into_owned();
        let w = Welt {
            lage: Lage {
                zustand: wurzel.join("zustand"),
                protokoll: wurzel.join("protokoll"),
                klassen: klassen(),
            },
            projekt,
            wurzel,
        };
        let kunde = kunde.map_or_else(|| "null".to_owned(), |k| format!("\"{k}\""));
        w.marken_datei(&format!(
            "{{\"fassung\":1,\"marken\":[{{\"ordner\":\"{}\",\"kennung\":\"{KENNUNG}\",\"kunde\":{kunde},\"klasse_hoch\":\"{klasse}\"}}]}}\n",
            w.projekt
        ));
        w
    }

    fn marken_datei(&self, text: &str) {
        let p = self.lage.zustand.join("ordnermarken.json");
        std::fs::write(&p, text).expect("write");
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o600)).expect("mode");
    }

    fn marken_text(&self) -> String {
        std::fs::read_to_string(self.lage.zustand.join("ordnermarken.json")).expect("read")
    }

    fn protokoll_roh(&self) -> String {
        std::fs::read_dir(&self.lage.protokoll)
            .expect("dir")
            .filter_map(Result::ok)
            .map(|e| std::fs::read_to_string(e.path()).unwrap_or_default())
            .collect()
    }

    fn laufen(&self, befehl: &Befehl) -> Result<String, BefehlFehler> {
        ausfuehren(befehl, &self.lage)
    }

    fn dateien_im_zustand(&self) -> Vec<String> {
        let mut n: Vec<String> = std::fs::read_dir(&self.lage.zustand)
            .expect("dir")
            .filter_map(Result::ok)
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        n.sort();
        n
    }
}

impl Drop for Welt {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.wurzel);
    }
}

/// The log folder made unusable: a file in its place.
fn protokoll_kaputt(w: &Welt) {
    std::fs::remove_dir_all(&w.lage.protokoll).expect("remove");
    std::fs::write(&w.lage.protokoll, b"x").expect("file");
}

#[test]
fn the_arguments_are_exactly_the_three_forms() {
    assert_eq!(
        aufruf_lesen(&os(&["zeigen", "/a"])),
        Ok(Befehl::Zeigen("/a".into()))
    );
    assert_eq!(
        aufruf_lesen(&os(&["loesen", "/a"])),
        Ok(Befehl::Loesen("/a".into()))
    );
    assert_eq!(
        aufruf_lesen(&os(&["senken", "/a", "--klasse", "intern"])),
        Ok(Befehl::Senken("/a".into(), "intern".to_owned()))
    );
    for schlecht in [
        &[][..],
        &["zeigen"],
        &["zeigen", "/a", "/b"],
        &["loesen"],
        &["loesen", "/a", "x"],
        &["loesen", ""],
        &["zeigen", ""],
        &["senken", "/a"],
        &["senken", "/a", "--klasse"],
        &["senken", "/a", "intern", "--klasse"],
        &["senken", "/a", "--klasse", "intern", "x"],
        &["senken", "", "--klasse", "intern"],
        &["senken", "/a", "-klasse", "intern"],
        &["zeige", "/a"],
        &["--help"],
    ] {
        assert_eq!(
            aufruf_lesen(&os(schlecht)),
            Err(BefehlFehler::Aufruf),
            "{schlecht:?}"
        );
    }
    let nicht_utf8 = OsString::from_vec(vec![0xff, 0xfe]);
    assert_eq!(
        aufruf_lesen(&["senken".into(), "/a".into(), "--klasse".into(), nicht_utf8]),
        Err(BefehlFehler::Aufruf)
    );
}

#[test]
fn every_row_of_the_table_has_code_reason_and_exit() {
    let soll = [
        (
            BefehlFehler::Aufruf,
            64,
            "ordnermarke.aufruf: Aufruf ungueltig\n",
        ),
        (
            BefehlFehler::KeineMarke,
            66,
            "ordnermarke.keine_marke: Keine Marke fuer diesen Ordner\n",
        ),
        (
            BefehlFehler::MarkeKaputt,
            78,
            "ordnermarke.marke_kaputt: Ordnermarken unlesbar oder beschaedigt\n",
        ),
        (
            BefehlFehler::Protokoll,
            74,
            "ordnermarke.protokoll: Protokollzeile nicht schreibbar\n",
        ),
        (
            BefehlFehler::KonfigFehlt,
            78,
            "ordnermarke.konfig_fehlt: Konfigurationsdatei fehlt\n",
        ),
        (
            BefehlFehler::KonfigKaputt,
            78,
            "ordnermarke.konfig_kaputt: Konfigurationsdatei nicht lesbar oder ungueltig\n",
        ),
    ];
    for (f, exit, zeile) in soll {
        assert_eq!((f.exitcode(), f.zeile().as_str()), (exit, zeile));
    }
}

#[test]
fn the_path_is_resolved_also_when_the_folder_is_gone_and_through_a_link() {
    let w = Welt::neu("intern", None);
    let echt = PathBuf::from(&w.projekt);
    assert_eq!(aufloesen(&echt).as_deref(), Some(w.projekt.as_str()));
    let verweis = w.wurzel.join("verweis");
    std::os::unix::fs::symlink(&echt, &verweis).expect("link");
    assert_eq!(aufloesen(&verweis).as_deref(), Some(w.projekt.as_str()));
    // a folder that is gone: the existing part is resolved, the rest appended
    assert_eq!(
        aufloesen(&verweis.join("weg/tief")),
        Some(format!("{}/weg/tief", w.projekt))
    );
    // a trailing slash and a `..` in the existing part
    assert_eq!(
        aufloesen(&echt.join("../projekt/")).as_deref(),
        Some(w.projekt.as_str())
    );
    // a relative path goes through the current folder
    let rel = std::env::current_dir().expect("cwd");
    assert_eq!(
        aufloesen(Path::new(".")).as_deref(),
        std::fs::canonicalize(rel).expect("c").to_str()
    );
    // a path through a file is another fault than a missing folder: no result, not "the rest appended"
    let datei = w.wurzel.join("datei");
    std::fs::write(&datei, b"x").expect("file");
    assert_eq!(aufloesen(&datei.join("unter")), None);
    // the root is no folder of a mark
    assert_eq!(aufloesen(Path::new("/")), None);
}

#[test]
fn zeigen_prints_key_customer_and_class_and_changes_and_writes_nothing() {
    let w = Welt::neu("kundendaten", Some("k-beispiel"));
    let vorher = w.marken_text();
    let ausgabe = w
        .laufen(&Befehl::Zeigen(PathBuf::from(&w.projekt)))
        .expect("zeigen");
    assert_eq!(
        ausgabe,
        format!(
            "{{\"kennung\":\"{KENNUNG}\",\"kunde\":\"k-beispiel\",\"klasse_hoch\":\"kundendaten\"}}\n"
        )
    );
    assert!(!ausgabe.contains(&w.projekt), "no path");
    assert_eq!(w.marken_text(), vorher);
    assert_eq!(w.protokoll_roh(), "");
    // no lock file and no other file: zeigen only reads
    assert_eq!(w.dateien_im_zustand(), ["ordnermarken.json"]);
    // "no customer" prints null
    let w = Welt::neu("intern", None);
    assert!(
        w.laufen(&Befehl::Zeigen(PathBuf::from(&w.projekt)))
            .expect("zeigen")
            .contains("\"kunde\":null")
    );
}

#[test]
fn loesen_removes_exactly_the_entry_and_writes_the_line_without_a_path() {
    let w = Welt::neu("kundendaten", Some("k-beispiel"));
    let andere = "{\"ordner\":\"/anderswo\",\"kennung\":\"ffffffffffffffffffffffffffffffff\",\"kunde\":null,\"klasse_hoch\":\"intern\"}";
    w.marken_datei(&format!(
        "{{\"fassung\":1,\"marken\":[{{\"ordner\":\"{}\",\"kennung\":\"{KENNUNG}\",\"kunde\":\"k-beispiel\",\"klasse_hoch\":\"kundendaten\"}},{andere}]}}\n",
        w.projekt
    ));
    assert_eq!(
        w.laufen(&Befehl::Loesen(PathBuf::from(&w.projekt))),
        Ok(String::new())
    );
    assert_eq!(
        w.marken_text(),
        format!("{{\"fassung\":1,\"marken\":[{andere}]}}\n")
    );
    let roh = w.protokoll_roh();
    let zeilen: Vec<&str> = roh.lines().collect();
    assert_eq!(zeilen.len(), 1);
    let z: serde_json::Value = serde_json::from_str(zeilen[0]).expect("json");
    assert_eq!(z["aktionsname"], "ordnermarke.geloest");
    assert_eq!(z["marke"], KENNUNG);
    assert_eq!(z["klasse_alt"], "kundendaten");
    assert_eq!(z["klasse_neu"], serde_json::Value::Null);
    assert_eq!(z["kunde_alt"], "k-beispiel");
    assert_eq!(z["kunde_neu"], serde_json::Value::Null);
    assert_eq!(z["ergebnis"], "ausgefuehrt");
    assert_eq!(z["fassung"], 2);
    assert_eq!(z["vorher"], "0".repeat(64));
    // the time is the time of the command, not 1970
    assert!(z["zeit"].as_str().expect("zeit") > "2026-01", "{z}");
    assert!(
        !roh.contains(&w.projekt) && !roh.contains("projekt"),
        "no path"
    );
    // the chain file is `ordnermarke-<JJJJ-MM>.jsonl`
    let name = std::fs::read_dir(&w.lage.protokoll)
        .expect("dir")
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .find(|n| n.starts_with("ordnermarke-"))
        .expect("file");
    assert_eq!(
        name,
        format!(
            "ordnermarke-{}.jsonl",
            &z["zeit"].as_str().expect("zeit")[..7]
        )
    );
    // the second `loesen` finds no mark any more
    assert_eq!(
        w.laufen(&Befehl::Loesen(PathBuf::from(&w.projekt))),
        Err(BefehlFehler::KeineMarke)
    );
    assert_eq!(w.protokoll_roh(), roh, "no second line");
}

#[test]
fn senken_keeps_the_key_and_writes_the_line_and_a_second_line_chains_to_the_first() {
    let w = Welt::neu("kundendaten", Some("k-beispiel"));
    let b = |k: &str| Befehl::Senken(PathBuf::from(&w.projekt), k.to_owned());
    assert_eq!(w.laufen(&b("intern")), Ok(String::new()));
    assert_eq!(
        w.marken_text(),
        format!(
            "{{\"fassung\":1,\"marken\":[{{\"ordner\":\"{}\",\"kennung\":\"{KENNUNG}\",\"kunde\":\"k-beispiel\",\"klasse_hoch\":\"intern\"}}]}}\n",
            w.projekt
        )
    );
    assert_eq!(w.laufen(&b("oeffentlich")), Ok(String::new()));
    let roh = w.protokoll_roh();
    let zeilen: Vec<serde_json::Value> = roh
        .lines()
        .map(|z| serde_json::from_str(z).expect("json"))
        .collect();
    assert_eq!(zeilen.len(), 2);
    assert_eq!(zeilen[0]["aktionsname"], "ordnermarke.gesenkt");
    assert_eq!(zeilen[0]["klasse_alt"], "kundendaten");
    assert_eq!(zeilen[0]["klasse_neu"], "intern");
    assert_eq!(zeilen[0]["kunde_alt"], "k-beispiel");
    assert_eq!(zeilen[0]["kunde_neu"], "k-beispiel");
    assert_eq!(zeilen[1]["klasse_alt"], "intern");
    assert_eq!(zeilen[1]["klasse_neu"], "oeffentlich");
    assert_eq!(
        zeilen[1]["vorher"].as_str().expect("v"),
        formate::kette::sha256_hex(roh.lines().next().expect("l").as_bytes())
    );
    assert!(!roh.contains("projekt"));
}

#[test]
fn senken_to_the_same_or_a_higher_or_an_unknown_class_is_refused_without_a_line() {
    let w = Welt::neu("intern", None);
    let vorher = w.marken_text();
    for klasse in ["intern", "kundendaten", "geheim", ""] {
        assert_eq!(
            w.laufen(&Befehl::Senken(
                PathBuf::from(&w.projekt),
                klasse.to_owned()
            )),
            Err(BefehlFehler::Aufruf),
            "{klasse:?}"
        );
    }
    assert_eq!(w.marken_text(), vorher);
    assert_eq!(w.protokoll_roh(), "");
    assert_eq!(w.dateien_im_zustand().len(), 1 + 1, "list and lock only");
}

#[test]
fn a_folder_without_a_mark_is_no_mark_and_a_related_folder_is_not_touched() {
    let w = Welt::neu("kundendaten", None);
    let vorher = w.marken_text();
    for ordner in [w.wurzel.join("projekt/unter"), w.wurzel.clone()] {
        for b in [
            Befehl::Zeigen(ordner.clone()),
            Befehl::Loesen(ordner.clone()),
            Befehl::Senken(ordner.clone(), "intern".to_owned()),
        ] {
            assert_eq!(w.laufen(&b), Err(BefehlFehler::KeineMarke), "{b:?}");
        }
    }
    assert_eq!(w.marken_text(), vorher);
    assert_eq!(w.protokoll_roh(), "");
}

#[test]
fn a_link_to_the_folder_reaches_the_same_mark() {
    let w = Welt::neu("kundendaten", None);
    let verweis = w.wurzel.join("verweis");
    std::os::unix::fs::symlink(&w.projekt, &verweis).expect("link");
    assert_eq!(w.laufen(&Befehl::Loesen(verweis)), Ok(String::new()));
    assert!(w.marken_text().contains("\"marken\":[]"));
}

#[test]
fn a_failing_log_leaves_the_mark_and_no_temporary_file() {
    for b in [
        |p: &str| Befehl::Loesen(PathBuf::from(p)),
        |p: &str| Befehl::Senken(PathBuf::from(p), "intern".to_owned()),
    ] {
        let w = Welt::neu("kundendaten", Some("k-beispiel"));
        let vorher = w.marken_text();
        protokoll_kaputt(&w);
        assert_eq!(w.laufen(&b(&w.projekt)), Err(BefehlFehler::Protokoll));
        assert_eq!(w.marken_text(), vorher, "the mark stays");
        assert_eq!(
            w.dateien_im_zustand(),
            ["ordnermarken.json", "ordnermarken.lock"],
            "the new file is removed"
        );
    }
}

#[test]
fn a_broken_file_or_a_wrong_key_is_marke_kaputt_with_no_change_and_no_line() {
    let faelle = [
        "{\"fassung\":1,\"marken\":[".to_owned(),
        // no key
        "{\"fassung\":1,\"marken\":[{\"ordner\":\"/p\",\"kunde\":null,\"klasse_hoch\":\"intern\"}]}"
            .to_owned(),
        // unknown class
        format!("{{\"fassung\":1,\"marken\":[{{\"ordner\":\"/p\",\"kennung\":\"{KENNUNG}\",\"kunde\":null,\"klasse_hoch\":\"geheim\"}}]}}"),
    ];
    for text in faelle {
        let w = Welt::neu("intern", None);
        w.marken_datei(&text);
        for b in [
            Befehl::Zeigen(PathBuf::from("/p")),
            Befehl::Loesen(PathBuf::from("/p")),
            Befehl::Senken(PathBuf::from("/p"), "oeffentlich".to_owned()),
        ] {
            assert_eq!(w.laufen(&b), Err(BefehlFehler::MarkeKaputt), "{text} {b:?}");
        }
        assert_eq!(w.marken_text(), text);
        assert_eq!(w.protokoll_roh(), "");
    }
}

#[test]
fn a_missing_file_is_no_mark_and_a_bad_state_folder_is_marke_kaputt() {
    let w = Welt::neu("intern", None);
    std::fs::remove_file(w.lage.zustand.join("ordnermarken.json")).expect("rm");
    assert_eq!(
        w.laufen(&Befehl::Loesen(PathBuf::from(&w.projekt))),
        Err(BefehlFehler::KeineMarke)
    );
    // not 0700: fail closed, also for zeigen
    std::fs::set_permissions(&w.lage.zustand, std::fs::Permissions::from_mode(0o755)).expect("m");
    for b in [
        Befehl::Zeigen(PathBuf::from(&w.projekt)),
        Befehl::Loesen(PathBuf::from(&w.projekt)),
    ] {
        assert_eq!(w.laufen(&b), Err(BefehlFehler::MarkeKaputt));
    }
    std::fs::set_permissions(&w.lage.zustand, std::fs::Permissions::from_mode(0o700)).expect("m");
}

#[test]
fn the_arguments_come_before_the_files() {
    // an invalid path of the root, and a class that is unknown, are 64 before any file is read
    let w = Welt::neu("intern", None);
    w.marken_datei("kaputt");
    assert_eq!(
        w.laufen(&Befehl::Senken(
            PathBuf::from(&w.projekt),
            "geheim".to_owned()
        )),
        Err(BefehlFehler::Aufruf)
    );
    assert_eq!(
        w.laufen(&Befehl::Loesen(PathBuf::from("/"))),
        Err(BefehlFehler::Aufruf)
    );
}

#[test]
fn two_commands_at_the_same_time_do_not_overtake_each_other() {
    let w = Welt::neu("kundendaten", None);
    let zweite = w.wurzel.join("zweit");
    std::fs::create_dir_all(&zweite).expect("dir");
    let z = zweite.to_string_lossy().into_owned();
    w.marken_datei(&format!(
        "{{\"fassung\":1,\"marken\":[{{\"ordner\":\"{}\",\"kennung\":\"{KENNUNG}\",\"kunde\":null,\"klasse_hoch\":\"kundendaten\"}},{{\"ordner\":\"{z}\",\"kennung\":\"{}\",\"kunde\":null,\"klasse_hoch\":\"kundendaten\"}}]}}\n",
        w.projekt,
        "e".repeat(32)
    ));
    std::thread::scope(|s| {
        s.spawn(|| {
            w.laufen(&Befehl::Loesen(PathBuf::from(&w.projekt)))
                .expect("a")
        });
        s.spawn(|| w.laufen(&Befehl::Loesen(zweite.clone())).expect("b"));
    });
    assert!(w.marken_text().contains("\"marken\":[]"), "both are gone");
    let roh = w.protokoll_roh();
    assert_eq!(roh.lines().count(), 2);
}

#[test]
fn the_text_of_zeigen_names_no_path() {
    let m = Marke {
        ordner: "/geheim/ordner".to_owned(),
        kennung: KENNUNG.to_owned(),
        kunde: None,
        klasse_hoch: "intern".to_owned(),
    };
    assert_eq!(
        zeigen_text(&m),
        format!("{{\"kennung\":\"{KENNUNG}\",\"kunde\":null,\"klasse_hoch\":\"intern\"}}\n")
    );
}
