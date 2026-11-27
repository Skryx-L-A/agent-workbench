//! The order of the stages and the steps of the replacement with injected errors (contract 15.12.3), beyond the cases.

#![allow(
    clippy::many_single_char_names,
    clippy::case_sensitive_file_extension_comparisons
)]

mod gemeinsam;

use std::cell::Cell;

use gemeinsam::{Gestoert, HELFER_UID, Ort, modus, namen};
use serde_json::{Value, json};
use vorgabe_helfer::code::Code;
use vorgabe_helfer::helfer::{
    self, Ergebnis, GRENZE_ANFRAGE, GRENZE_DATEI, Lesefehler, Orte, Partner,
};
use vorgabe_helfer::json::bytes_der;
use vorgabe_helfer::konten::Tabelle;

const NAME: &str = "firmenvorgabe.json";
const ADMIN: u32 = 1001;
const NUTZER: u32 = 1003;

fn konten() -> Tabelle {
    Tabelle(vec![
        ("konto-admin".into(), ADMIN),
        ("konto-nutzer".into(), NUTZER),
    ])
}

fn alt() -> Value {
    json!({"admins": ["konto-admin"], "leseklassen": [{"ebene": "nutzer", "wer": "konto-nutzer", "klasse": "intern"}]})
}

fn neu() -> Value {
    json!({"admins": ["konto-admin"], "leseklassen": [{"ebene": "gruppe", "wer": "g", "klasse": "oeffentlich"}]})
}

fn sha(b: &[u8]) -> String {
    formate::kette::sha256_hex(b)
}

/// `<ort>/e` (root), `<ort>/e/vorgabe` (helper, 0755), the file (helper, 0644) with `inhalt` as bytes.
struct Lage {
    ort: Ort,
    orte: Orte,
    eigner: std::collections::HashMap<u64, u32>,
}

impl Lage {
    fn neu(name: &str, bytes: &[u8]) -> Lage {
        let ort = Ort::neu(&format!("ablauf-{name}"));
        let e = ort.0.join("e");
        let v = e.join("vorgabe");
        std::fs::create_dir_all(&v).expect("folders");
        std::fs::write(v.join(NAME), bytes).expect("file");
        modus(&v.join(NAME), 0o644);
        modus(&v, 0o755);
        modus(&e, 0o755);
        let mut d = Gestoert::neu(&[]);
        d.eigner_von(&e, 0);
        Lage {
            orte: Orte {
                ordner: v,
                name: NAME.into(),
                eltern: vec![e],
                helfer_uid: HELFER_UID,
            },
            eigner: d.eigner,
            ort,
        }
    }

    fn mit(name: &str) -> Lage {
        Lage::neu(name, &bytes_der(&alt()))
    }

    fn d(&self, fehler: &[&str]) -> Gestoert {
        let mut d = Gestoert::neu(fehler);
        d.eigner.clone_from(&self.eigner);
        d
    }

    fn datei(&self) -> std::path::PathBuf {
        self.orte.ordner.join(NAME)
    }

    fn bytes(&self) -> Vec<u8> {
        std::fs::read(self.datei()).expect("file")
    }

    fn lauf(&self, d: &Gestoert, uid: Option<u32>, anfrage: &Value) -> (Ergebnis, u32) {
        let gelesen = Cell::new(0);
        let roh = serde_json::to_vec(anfrage).expect("json");
        let p = Partner {
            uid,
            eingeschlossen: false,
        };
        let e = helfer::bearbeiten(&self.orte, d, &konten(), p, || {
            gelesen.set(gelesen.get() + 1);
            Ok(roh)
        });
        (e, gelesen.get())
    }

    fn anfrage(&self, inhalt: &Value) -> Value {
        json!({"art": "Ersetzen", "fassung": 1, "basis": sha(&self.bytes()), "inhalt": inhalt})
    }

    fn ohne_temp(&self) {
        assert_eq!(
            namen(&self.orte.ordner),
            vec![NAME.to_owned()],
            "only the file stays"
        );
    }
}

fn code(e: &Ergebnis) -> Option<Code> {
    e.code
}

#[test]
fn erfolg_schreibt_die_form_mit_0644() {
    let l = Lage::mit("erfolg");
    let d = l.d(&[]);
    let (e, n) = l.lauf(&d, Some(ADMIN), &l.anfrage(&neu()));
    assert_eq!(e.code, None);
    assert_eq!(n, 1, "the request is read exactly once");
    assert_eq!(l.bytes(), bytes_der(&neu()));
    assert_eq!(e.sha256, Some(sha(&bytes_der(&neu()))));
    let m = std::fs::metadata(l.datei()).expect("stat");
    assert_eq!(
        std::os::unix::fs::PermissionsExt::mode(&m.permissions()) & 0o7777,
        0o644
    );
    l.ohne_temp();
    assert_eq!(d.ordner_syncs.get(), 1);
    assert_eq!(d.syncs.get(), 1);
    assert!(
        d.entfernt.borrow().is_empty(),
        "nothing removed after a success"
    );
}

#[test]
fn keine_anfrage_wird_vor_h5_gelesen() {
    let l = Lage::mit("vor-h5");
    let d = l.d(&[]);
    let a = l.anfrage(&neu());
    for (uid, soll) in [
        (None, Code::PartnerUnbekannt),
        (Some(1999), Code::PartnerUnbekannt),
        (Some(NUTZER), Code::PartnerNichtAdmin),
    ] {
        let (e, n) = l.lauf(&d, uid, &a);
        assert_eq!((code(&e), n), (Some(soll), 0), "{uid:?}");
        assert_eq!(e.sha256, None);
    }
    // H2 after H1, before the file
    let gelesen = Cell::new(false);
    for (uid, soll) in [
        (Some(ADMIN), Code::PartnerEingeschlossen),
        (Some(1999), Code::PartnerUnbekannt),
    ] {
        let e = helfer::bearbeiten(
            &l.orte,
            &d,
            &konten(),
            Partner {
                uid,
                eingeschlossen: true,
            },
            || {
                gelesen.set(true);
                Ok(Vec::new())
            },
        );
        assert_eq!(e.code, Some(soll));
    }
    assert!(!gelesen.get());
    assert_eq!(l.bytes(), bytes_der(&alt()));
}

#[test]
fn admin_ohne_konto_im_system_ist_kein_admin() {
    let inhalt = json!({"admins": ["konto-fehlt", "konto-nutzer"], "leseklassen": []});
    let l = Lage::neu("ohne-konto", &bytes_der(&inhalt));
    let (e, n) = l.lauf(&l.d(&[]), Some(ADMIN), &l.anfrage(&inhalt));
    assert_eq!((e.code, n), (Some(Code::PartnerNichtAdmin), 0));
    let (e, _) = l.lauf(&l.d(&[]), Some(NUTZER), &l.anfrage(&inhalt));
    assert_eq!(e.code, None, "the second admin of the list counts");
}

#[test]
fn h3_an_den_griffen() {
    let l = Lage::mit("h3");
    let a = l.anfrage(&neu());
    for f in ["fstat_ordner", "fstat_datei"] {
        let (e, n) = l.lauf(&l.d(&[f]), Some(ADMIN), &a);
        assert_eq!((e.code, n), (Some(Code::DateiUnsicher), 0), "{f}");
    }
    // a parent that is missing, a folder with others' write, a FIFO in place of the file, no file
    let mut o = l.orte.clone();
    o.eltern.push(l.ort.0.join("fehlt"));
    let e = helfer::bearbeiten(
        &o,
        &l.d(&[]),
        &konten(),
        Partner {
            uid: Some(ADMIN),
            eingeschlossen: false,
        },
        || Ok(vec![]),
    );
    assert_eq!(e.code, Some(Code::DateiUnsicher));
    modus(&l.orte.ordner, 0o757);
    assert_eq!(
        l.lauf(&l.d(&[]), Some(ADMIN), &a).0.code,
        Some(Code::DateiUnsicher),
        "folder 0757"
    );
    modus(&l.orte.ordner, 0o755);
    let mut o = l.orte.clone();
    o.helfer_uid = HELFER_UID + 1;
    let e = helfer::bearbeiten(
        &o,
        &l.d(&[]),
        &konten(),
        Partner {
            uid: Some(ADMIN),
            eingeschlossen: false,
        },
        || Ok(vec![]),
    );
    assert_eq!(
        e.code,
        Some(Code::DateiUnsicher),
        "another owner of folder and file"
    );
    let mut o = l.orte.clone();
    o.name = "fehlt.json".into();
    let e = helfer::bearbeiten(
        &o,
        &l.d(&[]),
        &konten(),
        Partner {
            uid: Some(ADMIN),
            eingeschlossen: false,
        },
        || Ok(vec![]),
    );
    assert_eq!(
        e.code,
        Some(Code::DateiUnsicher),
        "no file (regelebenen OPEN 30)"
    );
    let fifo = l.orte.ordner.join("fifo");
    assert!(
        std::process::Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .expect("mkfifo")
            .success()
    );
    let mut o = l.orte.clone();
    o.name = "fifo".into();
    let e = helfer::bearbeiten(
        &o,
        &l.d(&[]),
        &konten(),
        Partner {
            uid: Some(ADMIN),
            eingeschlossen: false,
        },
        || Ok(vec![]),
    );
    assert_eq!(
        e.code,
        Some(Code::DateiUnsicher),
        "a FIFO is no regular file"
    );
    std::fs::remove_file(&fifo).expect("rm fifo");
    // unsafe before a load error
    std::fs::write(l.datei(), b"kein json").expect("write");
    modus(&l.datei(), 0o664);
    assert_eq!(
        l.lauf(&l.d(&[]), Some(ADMIN), &a).0.code,
        Some(Code::DateiUnsicher)
    );
    modus(&l.datei(), 0o644);
    assert_eq!(
        l.lauf(&l.d(&[]), Some(ADMIN), &a).0.code,
        Some(Code::Ladefehler)
    );
}

#[test]
fn ladefehler() {
    let mut gross = bytes_der(&json!({"admins": ["konto-admin"], "leseklassen": []}));
    gross.pop();
    gross.resize(GRENZE_DATEI + 1, b' ');
    let mut genau = gross.clone();
    genau.truncate(GRENZE_DATEI);
    for (name, bytes, soll) in [
        (
            "doppelt",
            br#"{"admins":["konto-nutzer"],"admins":["konto-admin"],"leseklassen":[]}"#.to_vec(),
            Some(Code::Ladefehler),
        ),
        (
            "utf8",
            b"{\"admins\":[\"konto-admin\"],\"leseklassen\":[],\"x\":\"\xff\"}".to_vec(),
            Some(Code::Ladefehler),
        ),
        (
            "ohne-liste",
            br#"{"leseklassen":[]}"#.to_vec(),
            Some(Code::Ladefehler),
        ),
        ("zu-gross", gross, Some(Code::Ladefehler)),
        ("genau", genau, Some(Code::AnfrageUngueltig)),
    ] {
        let l = Lage::neu(name, &bytes);
        let (e, n) = l.lauf(&l.d(&[]), Some(ADMIN), &json!({}));
        assert_eq!(e.code, soll, "{name}");
        assert_eq!(n, u32::from(soll == Some(Code::AnfrageUngueltig)), "{name}");
        assert_eq!(l.bytes(), bytes, "{name}");
    }
    let l = Lage::mit("lesen");
    assert_eq!(
        l.lauf(&l.d(&["lesen"]), Some(ADMIN), &json!({})).0.code,
        Some(Code::Ladefehler)
    );
}

#[test]
fn lesefehler_und_grenze() {
    let l = Lage::mit("lesefehler");
    let p = Partner {
        uid: Some(ADMIN),
        eingeschlossen: false,
    };
    let d = l.d(&[]);
    for (f, soll) in [
        (Lesefehler::ZuGross, Code::AnfrageZuGross),
        (Lesefehler::Frist, Code::AnfrageFrist),
        (Lesefehler::Abbruch, Code::AnfrageUngueltig),
    ] {
        let e = helfer::bearbeiten(&l.orte, &d, &konten(), p, || Err(f));
        assert_eq!(e.code, Some(soll));
    }
    let gross = l.anfrage(&neu());
    let mut roh = serde_json::to_vec(&gross).expect("json");
    roh.resize(GRENZE_ANFRAGE + 1, b' ');
    let e = helfer::bearbeiten(&l.orte, &d, &konten(), p, || Ok(roh));
    assert_eq!(
        e.code,
        Some(Code::AnfrageZuGross),
        "one byte over the limit"
    );
    let mut roh = serde_json::to_vec(&gross).expect("json");
    roh.resize(GRENZE_ANFRAGE, b' ');
    let e = helfer::bearbeiten(&l.orte, &d, &konten(), p, || Ok(roh));
    assert_eq!(e.code, None, "exactly the limit");
    assert_eq!(l.bytes(), bytes_der(&neu()));
}

#[test]
fn reihenfolge_h8_h9_h10() {
    let l = Lage::mit("reihe");
    let d = l.d(&[]);
    let fremd = json!({"admins": ["konto-admin", "konto-nutzer"], "leseklassen": []});
    let mut a = l.anfrage(&fremd);
    a["basis"] = json!("0".repeat(64));
    assert_eq!(
        l.lauf(&d, Some(ADMIN), &a).0.code,
        Some(Code::Veraltet),
        "H9 before H10"
    );
    let mut a = l.anfrage(&fremd);
    assert_eq!(
        l.lauf(&d, Some(ADMIN), &a).0.code,
        Some(Code::AusserLeseklassen)
    );
    a["basis"] = json!("x");
    assert_eq!(
        l.lauf(&d, Some(ADMIN), &a).0.code,
        Some(Code::AnfrageUngueltig),
        "H8 before H9"
    );
    let mit_schluessel = json!({"admins": ["konto-admin"], "leseklassen": [], "neu": true});
    assert_eq!(
        l.lauf(&d, Some(ADMIN), &l.anfrage(&mit_schluessel)).0.code,
        Some(Code::AusserLeseklassen)
    );
    assert_eq!(l.bytes(), bytes_der(&alt()));
    l.ohne_temp();
    // the basis is the sha256 of the bytes, not of the form: a file in another form of the same content
    let anders = serde_json::to_vec(&alt()).expect("json");
    let l = Lage::neu("andere-form", &anders);
    let mut a = l.anfrage(&neu());
    assert_eq!(a["basis"], json!(sha(&anders)));
    a["basis"] = json!(sha(&bytes_der(&alt())));
    assert_eq!(
        l.lauf(&l.d(&[]), Some(ADMIN), &a).0.code,
        Some(Code::Veraltet)
    );
    let (e, _) = l.lauf(&l.d(&[]), Some(ADMIN), &l.anfrage(&neu()));
    assert_eq!(e.code, None);
    assert_eq!(
        l.bytes(),
        bytes_der(&neu()),
        "the helper writes its own form"
    );
}

fn schritt_scheitert(fehler: &[&str], soll: Code, bytes_danach: &Value) {
    let l = Lage::mit(&fehler.join("-").replace(':', ""));
    let d = l.d(fehler);
    let (e, _) = l.lauf(&d, Some(ADMIN), &l.anfrage(&neu()));
    assert_eq!(e.code, Some(soll), "{fehler:?}");
    assert_eq!(e.sha256, None);
    assert_eq!(l.bytes(), bytes_der(bytes_danach), "{fehler:?}");
    l.ohne_temp();
    for n in d.entfernt.borrow().iter() {
        assert!(
            n.starts_with(".firmenvorgabe.json.") && n.ends_with(".neu"),
            "only a temporary file: {n}"
        );
    }
    let m = std::fs::metadata(l.datei()).expect("stat");
    assert_eq!(
        std::os::unix::fs::PermissionsExt::mode(&m.permissions()) & 0o7777,
        0o644
    );
}

#[test]
fn schritte_mit_fehlern() {
    schritt_scheitert(&["temp"], Code::TempFehlgeschlagen, &alt());
    schritt_scheitert(&["schreiben"], Code::TempFehlgeschlagen, &alt());
    schritt_scheitert(&["chmod"], Code::TempFehlgeschlagen, &alt());
    schritt_scheitert(&["fsync"], Code::FsyncFehlgeschlagen, &alt());
    schritt_scheitert(&["rename"], Code::RenameFehlgeschlagen, &alt());
    schritt_scheitert(&["ordner_fsync"], Code::OrdnerFsyncFehlgeschlagen, &alt());
    schritt_scheitert(&["ordner_fsync", "rueckweg"], Code::ZustandUnklar, &neu());
    schritt_scheitert(
        &["ordner_fsync", "ordner_fsync_zwei"],
        Code::ZustandUnklar,
        &alt(),
    );
    schritt_scheitert(&["exist:8"], Code::TempFehlgeschlagen, &alt());
}

#[test]
fn freier_name_nach_eexist() {
    let l = Lage::mit("eexist");
    let d = l.d(&["exist:7"]);
    let (e, _) = l.lauf(&d, Some(ADMIN), &l.anfrage(&neu()));
    assert_eq!(e.code, None);
    assert_eq!(d.angelegt.get(), 8);
    assert_eq!(l.bytes(), bytes_der(&neu()));
    l.ohne_temp();
}

#[test]
fn rueckweg_schreibt_die_alten_bytes() {
    let l = Lage::mit("rueckweg");
    let alt_bytes = l.bytes();
    let d = l.d(&["ordner_fsync"]);
    let (e, _) = l.lauf(&d, Some(ADMIN), &l.anfrage(&neu()));
    assert_eq!(e.code, Some(Code::OrdnerFsyncFehlgeschlagen));
    assert_eq!(l.bytes(), alt_bytes);
    assert_eq!(
        (d.angelegt.get(), d.renames.get(), d.ordner_syncs.get()),
        (2, 2, 2)
    );
    assert!(d.entfernt.borrow().is_empty());
}

#[test]
fn verweis_im_ordner_bleibt_unberuehrt() {
    let l = Lage::mit("verweis");
    let ziel = l.ort.0.join("ziel");
    std::fs::write(&ziel, bytes_der(&alt())).expect("target");
    std::fs::remove_file(l.datei()).expect("rm");
    std::os::unix::fs::symlink(&ziel, l.datei()).expect("link");
    let a =
        json!({"art": "Ersetzen", "fassung": 1, "basis": sha(&bytes_der(&alt())), "inhalt": neu()});
    let (e, n) = l.lauf(&l.d(&[]), Some(ADMIN), &a);
    assert_eq!((e.code, n), (Some(Code::DateiUnsicher), 0));
    assert_eq!(std::fs::read(&ziel).expect("target"), bytes_der(&alt()));
    assert!(
        std::fs::symlink_metadata(l.datei())
            .expect("lstat")
            .file_type()
            .is_symlink()
    );
}
