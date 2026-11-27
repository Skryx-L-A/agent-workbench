//! The outside test against the cases of the write way (`rust/spec/regelebenen/faelle-schreiben/` and
//! `erwartet-schreiben/`, contract 15.12.5).
//!
//! Every step the helper decides alone runs against the crate with real files in a folder of the test: a step
//! `anfrage` as it is, a step `schreiben` with the request the service would build (basis = sha256 of the bytes it
//! loaded last, content = loaded content with the new `leseklassen`). The owners of the case come from the layer of the
//! test (`gemeinsam::Gestoert`), modes and links are real. A case runs up to its first step that needs the service
//! (stages 0, 0a, 1, 2, 3 and 4 of 15.12.3, the view of 15.13); those cases stand in [`FUER_DEN_DIENST`]. The line
//! (`zeile`) of a step `schreiben` is the work of the service and is not compared here.

#![allow(
    clippy::many_single_char_names,
    clippy::case_sensitive_file_extension_comparisons
)]

mod gemeinsam;

use std::collections::BTreeMap;
use std::io::Write;
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use gemeinsam::{Gestoert, HELFER_UID, Ort, modus};
use serde_json::{Value, json};
use vorgabe_helfer::helfer::{self, GRENZE_ANFRAGE, Lesefehler, Orte, Partner};
use vorgabe_helfer::json::bytes_der;
use vorgabe_helfer::konten::Tabelle;
use vorgabe_helfer::lesen::anfrage_lesen;
use vorgabe_helfer::sys::{Art, Dateien, Echt};

const NAME: &str = "firmenvorgabe.json";
const FEHLER_HELFER: [&str; 8] = [
    "eingeschlossen",
    "zu_gross",
    "frist",
    "temp",
    "fsync",
    "rename",
    "ordner_fsync",
    "rueckweg",
];

/// Cases with a step that needs the service, and how many of their steps ran here before it.
const FUER_DEN_DIENST: [(&str, usize); 9] = [
    ("konto-admin-sieht", 0),
    ("konto-nutzer-sieht-keins", 0),
    ("schreiben-admin-erfolgreich", 1),
    ("schreiben-antwort-fehlt", 0),
    ("schreiben-helfer-nicht-erreichbar", 0),
    ("schreiben-nichtadmin", 0),
    ("schreiben-ordner-fsync-scheitert", 1),
    ("schreiben-sperre-bis-neuladen", 1),
    ("schreiben-wert-ungueltig", 0),
];

fn spec() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../spec/regelebenen")
}

fn lies(p: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(p).expect("read")).expect("json")
}

fn sha(b: &[u8]) -> String {
    formate::kette::sha256_hex(b)
}

fn modus_von(m: &Value) -> u32 {
    u32::from_str_radix(m.as_str().expect("mode"), 8).expect("octal")
}

/// The folder of one case: `<ort>/e` is the parent (every parent of the case), `<ort>/e/vorgabe` the folder of the
/// helper, the file in it. A link of the case points at an entry beside it.
struct Aufbau {
    orte: Orte,
    d: Gestoert,
    namen_der_uids: BTreeMap<u32, String>,
    datei_art: String,
    datei_eigner: String,
    datei_modus: String,
}

fn uid_von(name: &str, konten: &BTreeMap<String, u32>) -> u32 {
    match name {
        "root" => 0,
        "werkbank-vorgabe" => HELFER_UID,
        k => konten[k],
    }
}

fn aufbauen(ort: &Path, fall: &Value) -> Aufbau {
    let konten: BTreeMap<String, u32> =
        serde_json::from_value(fall["konten"].clone()).expect("accounts");
    let mut d = Gestoert::neu(&[]);
    let eltern = ort.join("e");
    std::fs::create_dir(&eltern).expect("parent");
    let ordner_ziel = if fall["ordner"]["art"] == "verweis" {
        let z = eltern.join("echter-ordner");
        std::fs::create_dir(&z).expect("folder");
        std::os::unix::fs::symlink(&z, eltern.join("vorgabe")).expect("link");
        z
    } else {
        let z = eltern.join("vorgabe");
        std::fs::create_dir(&z).expect("folder");
        z
    };
    let datei = &fall["firmenvorgabe"];
    let bytes = bytes_der(&datei["inhalt"]);
    let ziel = if datei["art"] == "verweis" {
        let z = ordner_ziel.join("ziel.json");
        std::fs::write(&z, &bytes).expect("target");
        std::os::unix::fs::symlink("ziel.json", ordner_ziel.join(NAME)).expect("link");
        z
    } else {
        let z = ordner_ziel.join(NAME);
        std::fs::write(&z, &bytes).expect("file");
        z
    };
    modus(&ziel, modus_von(&datei["modus"]));
    d.eigner_von(
        &ziel,
        uid_von(datei["eigentuemer"].as_str().expect("owner"), &konten),
    );
    modus(&ordner_ziel, modus_von(&fall["ordner"]["modus"]));
    d.eigner_von(
        &ordner_ziel,
        uid_von(
            fall["ordner"]["eigentuemer"].as_str().expect("owner"),
            &konten,
        ),
    );
    modus(&eltern, modus_von(&fall["eltern"]["modus"]));
    d.eigner_von(
        &eltern,
        uid_von(
            fall["eltern"]["eigentuemer"].as_str().expect("owner"),
            &konten,
        ),
    );
    let mut namen_der_uids: BTreeMap<u32, String> =
        konten.iter().map(|(k, u)| (*u, k.clone())).collect();
    namen_der_uids.insert(0, "root".into());
    namen_der_uids.insert(HELFER_UID, "werkbank-vorgabe".into());
    Aufbau {
        orte: Orte {
            ordner: eltern.join("vorgabe"),
            name: NAME.into(),
            eltern: vec![eltern],
            helfer_uid: HELFER_UID,
        },
        d,
        namen_der_uids,
        datei_art: datei["art"].as_str().expect("kind").into(),
        datei_eigner: datei["eigentuemer"].as_str().expect("owner").into(),
        datei_modus: datei["modus"].as_str().expect("mode").into(),
    }
}

impl Aufbau {
    fn pfad(&self) -> PathBuf {
        self.orte.ordner.join(NAME)
    }

    fn bytes(&self) -> Vec<u8> {
        std::fs::read(self.pfad()).expect("file")
    }

    /// `datei`, `unveraendert` and `temp_bleibt` after a step, in the form of the expectation.
    fn nachher(&self, vorher: &str) -> Value {
        let b = self.bytes();
        let m = self.d.lstat(&self.pfad()).expect("lstat");
        let (eigner, md) = if m.art == Art::Verweis || self.datei_art == "verweis" {
            (self.datei_eigner.clone(), self.datei_modus.clone())
        } else {
            (
                self.namen_der_uids[&m.uid].clone(),
                format!("{:04o}", m.modus),
            )
        };
        let temp: Vec<String> = gemeinsam::namen(&self.orte.ordner)
            .into_iter()
            .filter(|n| n != NAME && n != "ziel.json")
            .collect();
        json!({
            "datei": {"eigentuemer": eigner, "modus": md, "sha256": sha(&b),
                      "inhalt": serde_json::from_slice::<Value>(&b).expect("json")},
            "unveraendert": sha(&b) == vorher,
            "temp_bleibt": !temp.is_empty(),
        })
    }
}

/// Reads `bytes` as a request through a real pair of sockets, with the injected `zu_gross` or `frist`.
fn ueber_socket(bytes: &[u8], fehler: &[String]) -> Result<Vec<u8>, Lesefehler> {
    let (mut a, mut b) = UnixStream::pair().expect("pair");
    let frist_kurz = fehler.iter().any(|f| f == "frist");
    let zu_gross = fehler.iter().any(|f| f == "zu_gross");
    let inhalt = if zu_gross {
        let mut v = bytes.to_vec();
        v.resize(GRENZE_ANFRAGE + 1, b' ');
        v
    } else {
        bytes.to_vec()
    };
    let schreiber = std::thread::spawn(move || {
        let _ = a.write_all(&inhalt);
        if frist_kurz {
            // the write side stays open beyond the deadline
            std::thread::sleep(Duration::from_millis(400));
        }
        let _ = a.shutdown(Shutdown::Write);
        a
    });
    let frist = Instant::now()
        + if frist_kurz {
            Duration::from_millis(150)
        } else {
            Duration::from_secs(5)
        };
    let erg = anfrage_lesen(&mut b, frist, Instant::now);
    drop(b);
    let _ = schreiber.join();
    erg
}

/// Runs one request against the helper; `true` in the second value if the request was read at all.
fn helfer_lauf(
    a: &Aufbau,
    partner: Option<u32>,
    anfrage: &Value,
    fehler: &[String],
    konten: &Tabelle,
) -> (Value, bool) {
    let mut gelesen = false;
    let mut d = Gestoert::neu(
        &fehler
            .iter()
            .map(String::as_str)
            .filter(|f| !matches!(*f, "eingeschlossen" | "zu_gross" | "frist"))
            .collect::<Vec<_>>(),
    );
    d.eigner.clone_from(&a.d.eigner);
    let p = Partner {
        uid: partner,
        eingeschlossen: fehler.iter().any(|f| f == "eingeschlossen"),
    };
    let roh = serde_json::to_vec(anfrage).expect("json");
    let erg = helfer::bearbeiten(&a.orte, &d, konten, p, || {
        gelesen = true;
        ueber_socket(&roh, fehler)
    });
    let antwort: Value = serde_json::from_slice(&erg.antwort()).expect("answer is json");
    (antwort, gelesen)
}

fn ist_helferstufe_vor_dem_lesen(code: &Value) -> bool {
    matches!(
        code.as_str(),
        Some(
            "firmenvorgabe.partner_unbekannt"
                | "firmenvorgabe.partner_eingeschlossen"
                | "firmenvorgabe.datei_unsicher"
                | "firmenvorgabe.ladefehler"
                | "firmenvorgabe.partner_nichtadmin"
                | "firmenvorgabe.partner_nicht_admin"
        )
    )
}

/// Runs the helper steps of one case; returns how many steps ran and whether the case ran in full.
fn fall_laufen(name: &str) -> (usize, bool) {
    let fall = lies(&spec().join("faelle-schreiben").join(format!("{name}.json")));
    let erwartet = lies(
        &spec()
            .join("erwartet-schreiben")
            .join(format!("{name}.json")),
    );
    let ort = Ort::neu(&format!("fall-{name}"));
    let a = aufbauen(&ort.0, &fall);
    let konten = Tabelle(
        fall["konten"]
            .as_object()
            .expect("accounts")
            .iter()
            .map(|(k, u)| {
                (
                    k.clone(),
                    u32::try_from(u.as_u64().expect("uid")).expect("uid"),
                )
            })
            .collect(),
    );
    let mut geladen = fall["firmenvorgabe"]["inhalt"].clone();
    let schritte = fall["schritte"].as_array().expect("steps");
    for (i, (s, e)) in schritte
        .iter()
        .zip(erwartet["schritte"].as_array().expect("steps"))
        .enumerate()
    {
        let vorher = sha(&a.bytes());
        if let Some(an) = s.get("anfrage") {
            let fehler: Vec<String> = serde_json::from_value(an["fehler"].clone()).expect("errors");
            let (antwort, gelesen) = helfer_lauf(
                &a,
                an["partner"]
                    .as_u64()
                    .map(|u| u32::try_from(u).expect("uid")),
                &an["anfrage"],
                &fehler,
                &konten,
            );
            assert_eq!(antwort, e["antwort"], "{name} step {i}: answer");
            if ist_helferstufe_vor_dem_lesen(&antwort["code"]) {
                assert!(!gelesen, "{name} step {i}: read before H5");
            }
            let mut soll = e.clone();
            soll.as_object_mut().expect("object").remove("antwort");
            assert_eq!(a.nachher(&vorher), soll, "{name} step {i}: file");
            continue;
        }
        let Some(sw) = s.get("schreiben") else {
            return (i, false);
        };
        let fehler: Vec<String> = serde_json::from_value(sw["fehler"].clone()).expect("errors");
        let code = &e["code"];
        let vom_helfer = code.is_null()
            || code
                .as_str()
                .is_some_and(|c| c.starts_with("firmenvorgabe."));
        let nur_dienst = fehler
            .iter()
            .any(|f| matches!(f.as_str(), "helfer_weg" | "antwort_fehlt"));
        if !vom_helfer || nur_dienst || code == "firmenvorgabe.helfer_nicht_erreichbar" {
            return (i, false);
        }
        // the request the service builds (stage 4); `fremd_geaendert`: the bytes it loaded are no longer the file
        let basis = if fehler.iter().any(|f| f == "fremd_geaendert") {
            sha(&bytes_der(&json!({"veraltet": geladen})))
        } else {
            sha(&bytes_der(&geladen))
        };
        let mut inhalt = geladen.clone();
        inhalt["leseklassen"] = sw["wert"].clone();
        let anfrage = json!({"art": "Ersetzen", "fassung": 1, "basis": basis, "inhalt": inhalt});
        let fehler_helfer: Vec<String> = fehler
            .iter()
            .filter(|f| FEHLER_HELFER.contains(&f.as_str()))
            .cloned()
            .collect();
        let partner = sw["partner"]
            .as_u64()
            .map(|u| u32::try_from(u).expect("uid"));
        let (antwort, _) = helfer_lauf(&a, partner, &anfrage, &fehler_helfer, &konten);
        assert_eq!(&antwort["code"], code, "{name} step {i}: code");
        assert_eq!(e["angenommen"], json!(code.is_null()), "{name} step {i}");
        let mut soll = json!({});
        for k in ["datei", "unveraendert", "temp_bleibt"] {
            soll[k] = e[k].clone();
        }
        assert_eq!(a.nachher(&vorher), soll, "{name} step {i}: file");
        if code.is_null() {
            assert_eq!(
                antwort["sha256"], e["datei"]["sha256"],
                "{name} step {i}: sha256 of the answer"
            );
            geladen = e["datei"]["inhalt"].clone();
        }
        if code == "firmenvorgabe.zustand_unklar" {
            // the service locks now (stage 0a); the following steps are its work
            return (i + 1, i + 1 == schritte.len());
        }
    }
    (schritte.len(), true)
}

#[test]
fn faelle_des_helfers() {
    let mut namen: Vec<String> = std::fs::read_dir(spec().join("faelle-schreiben"))
        .expect("cases")
        .flatten()
        .filter_map(|e| {
            e.file_name()
                .to_str()
                .and_then(|n| n.strip_suffix(".json"))
                .map(str::to_owned)
        })
        .collect();
    namen.sort();
    assert_eq!(namen.len(), 37, "cases of the write way");
    let mut fuer_dienst = Vec::new();
    let mut voll = 0;
    for n in &namen {
        let (gelaufen, ganz) = fall_laufen(n);
        if ganz {
            voll += 1;
        } else {
            fuer_dienst.push((n.as_str(), gelaufen));
        }
    }
    assert_eq!(
        fuer_dienst,
        FUER_DEN_DIENST.to_vec(),
        "cases that need the service"
    );
    assert_eq!(voll, 28);
}

/// The layer of the test changes no file call: without an injected error it gives what the system gives.
#[test]
fn schicht_ohne_fehler_wie_das_system() {
    let ort = Ort::neu("schicht");
    let d = Gestoert::neu(&[]);
    let o = d.ordner_oeffnen(&ort.0).expect("folder");
    let e = Echt.ordner_oeffnen(&ort.0).expect("folder");
    let a = d.fstat(std::os::fd::AsFd::as_fd(&o)).expect("fstat");
    let b = Echt.fstat(std::os::fd::AsFd::as_fd(&e)).expect("fstat");
    assert_eq!((a.art, a.modus, a.ino), (b.art, b.modus, b.ino));
    assert_eq!(a.uid, HELFER_UID, "the test user is reported as the helper");
}
