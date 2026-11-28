#![allow(
    clippy::many_single_char_names,
    reason = "probe, store and pseudonyms are short names in the tests"
)]

use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

use super::*;

/// A folder of the test below the temp folder, removed at the end.
struct Probe(PathBuf);

impl Probe {
    fn neu(name: &str) -> Self {
        let basis = std::env::temp_dir().canonicalize().expect("tmp");
        let p = basis.join(format!("pb-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(p.join("pb")).expect("dir");
        fs::set_permissions(p.join("pb"), fs::Permissions::from_mode(0o700)).expect("chmod");
        Self(p)
    }

    fn pb(&self) -> PathBuf {
        self.0.join("pb")
    }
}

impl Drop for Probe {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Randomness that counts up: each call gives 32 bytes of the next value.
fn zaehlend() -> Zufall {
    let n = Arc::new(AtomicU8::new(1));
    Box::new(move || Some([n.fetch_add(1, Ordering::SeqCst); 32]))
}

fn fest(wert: u8) -> Zufall {
    Box::new(move || Some([wert; 32]))
}

const T: i64 = 1_795_179_540_000;

#[test]
fn die_vektoren_von_vertrag_9() {
    let fall: Value = serde_json::from_str(include_str!(
        "../../../../../spec/personenbezug/faelle/pseudonym.json"
    ))
    .expect("case");
    let vektoren = fall["vektoren"].as_array().expect("vectors");
    assert!(vektoren.len() >= 3);
    for v in vektoren {
        let s = aus_hex(v["schluessel"].as_str().expect("key")).expect("hex");
        let g = aus_hex(v["geheimnis"].as_str().expect("secret")).expect("hex");
        assert_eq!(pseudonym(&s, &g), v["pseudonym"].as_str().expect("ps"));
    }
    assert_eq!(
        pseudonym(&[1; 32], &[2; 32]),
        "ps_de284346fe75206cff73204c9a0dbbc2"
    );
}

#[test]
fn ein_neuer_speicher_gibt_jeder_person_ein_festes_pseudonym() {
    let p = Probe::neu("neu");
    let s = Speicher::oeffnen(&p.pb(), zaehlend(), T).expect("open");
    let (a, neu_a) = s.person("uid-1000", T).expect("person");
    let (b, neu_b) = s.person("uid-1001", T).expect("person");
    let (a2, neu_a2) = s.person("uid-1000", T + 1).expect("person");
    assert!(form::pseudonym(&a));
    assert_ne!(a, b);
    assert_eq!(a, a2);
    assert_eq!((neu_a, neu_b, neu_a2), (true, true, false));
    for name in ["schluessel.json", "zuordnung.json", "zuordnung.sperre"] {
        let m = fs::symlink_metadata(p.pb().join(name)).expect("file");
        assert_eq!(m.permissions().mode() & 0o777, 0o600, "{name}");
    }
    // the pseudonym follows from the key and the secret in the files (contract 9, section 3)
    let schluessel: Value =
        serde_json::from_slice(&fs::read(p.pb().join("schluessel.json")).expect("key"))
            .expect("json");
    let zuordnung: Value =
        serde_json::from_slice(&fs::read(p.pb().join("zuordnung.json")).expect("map"))
            .expect("json");
    let k = aus_hex(schluessel["schluessel"][0]["wert"].as_str().expect("hex")).expect("key");
    let g = aus_hex(
        zuordnung["personen"]["uid-1000"]["geheimnis"]
            .as_str()
            .expect("hex"),
    )
    .expect("secret");
    assert_eq!(pseudonym(&k, &g), a);
    // a second opening reads the same state; the identifier stands in no file except the assignment
    drop(s);
    let s = Speicher::oeffnen(&p.pb(), zaehlend(), T + 5).expect("open again");
    assert_eq!(s.pseudonym("uid-1000", T + 6).as_deref(), Some(a.as_str()));
    let z = fs::read_to_string(p.pb().join("zuordnung.json")).expect("read");
    assert!(z.ends_with('\n') && z.contains("\"uid-1000\""));
    assert!(
        !fs::read_to_string(p.pb().join("schluessel.json"))
            .expect("read")
            .contains("uid-")
    );
    // no temporary file is left
    let reste: Vec<_> = fs::read_dir(p.pb())
        .expect("dir")
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with('.'))
        .collect();
    assert!(reste.is_empty());
}

#[test]
fn jeder_unsichere_ordner_wird_abgelehnt() {
    let p = Probe::neu("ordner");
    fs::set_permissions(p.pb(), fs::Permissions::from_mode(0o750)).expect("chmod");
    assert_eq!(
        Speicher::oeffnen(&p.pb(), zaehlend(), T).err(),
        Some(fehler::UNSICHER)
    );
    fs::set_permissions(p.pb(), fs::Permissions::from_mode(0o700)).expect("chmod");
    symlink(p.pb(), p.0.join("link")).expect("link");
    assert_eq!(
        Speicher::oeffnen(&p.0.join("link"), zaehlend(), T).err(),
        Some(fehler::UNSICHER)
    );
    assert_eq!(
        Speicher::oeffnen(&p.0.join("fehlt"), zaehlend(), T).err(),
        Some(fehler::UNSICHER)
    );
    fs::create_dir(p.0.join(".git")).expect("git");
    assert_eq!(
        Speicher::oeffnen(&p.pb(), zaehlend(), T).err(),
        Some(fehler::IM_REPO)
    );
    fs::remove_dir(p.0.join(".git")).expect("rm");
    fs::write(p.pb().join(".git"), "gitdir: x").expect("git file");
    assert_eq!(
        Speicher::oeffnen(&p.pb(), zaehlend(), T).err(),
        Some(fehler::IM_REPO)
    );
    fs::remove_file(p.pb().join(".git")).expect("rm");
    assert!(Speicher::oeffnen(&p.pb(), zaehlend(), T).is_ok());
}

#[test]
fn halbe_und_falsche_dateien_legen_nichts_neu_an() {
    let p = Probe::neu("dateien");
    drop(Speicher::oeffnen(&p.pb(), zaehlend(), T).expect("open"));
    let s = fs::read(p.pb().join("schluessel.json")).expect("key");
    let z = fs::read(p.pb().join("zuordnung.json")).expect("assignment");
    fs::remove_file(p.pb().join("schluessel.json")).expect("rm");
    assert_eq!(
        Speicher::oeffnen(&p.pb(), zaehlend(), T).err(),
        Some(fehler::SCHLUESSEL_FEHLT)
    );
    assert!(
        fs::symlink_metadata(p.pb().join("schluessel.json")).is_err(),
        "nothing new"
    );
    fs::write(p.pb().join("schluessel.json"), &s).expect("write");
    fs::set_permissions(
        p.pb().join("schluessel.json"),
        fs::Permissions::from_mode(0o600),
    )
    .expect("chmod");
    fs::remove_file(p.pb().join("zuordnung.json")).expect("rm");
    assert_eq!(
        Speicher::oeffnen(&p.pb(), zaehlend(), T).err(),
        Some(fehler::ZUORDNUNG_FEHLT)
    );
    let schreib = |text: &[u8]| {
        fs::write(p.pb().join("zuordnung.json"), text).expect("write");
        fs::set_permissions(
            p.pb().join("zuordnung.json"),
            fs::Permissions::from_mode(0o600),
        )
        .expect("chmod");
        Speicher::oeffnen(&p.pb(), zaehlend(), T).err()
    };
    assert_eq!(schreib(&z), None);
    let g = "0303030303030303030303030303030303030303030303030303030303030303";
    for falsch in [
        b"{\"fassung\":1,\"personen\":{}}".to_vec(),
        b"{\"personen\":{},\"fassung\":1}\n".to_vec(),
        b"{\"fassung\": 1, \"personen\": {}}\n".to_vec(),
        b"{\"fassung\":2,\"personen\":{}}\n".to_vec(),
        format!("{{\"fassung\":1,\"personen\":{{\"uid-1\":{{\"geheimnis\":\"{g}\",\"seit_ms\":1,\"texte\":{{}}}},\"uid-2\":{{\"geheimnis\":\"{g}\",\"seit_ms\":1,\"texte\":{{}}}}}}}}\n").into_bytes(),
        format!("{{\"fassung\":1,\"personen\":{{\"Uid 1\":{{\"geheimnis\":\"{g}\",\"seit_ms\":1,\"texte\":{{}}}}}}}}\n").into_bytes(),
        format!("{{\"fassung\":1,\"personen\":{{\"uid-1\":{{\"geheimnis\":\"{g}\",\"seit_ms\":1}}}}}}\n").into_bytes(),
    ] {
        assert_eq!(schreib(&falsch), Some(fehler::UNGUELTIG), "{}", String::from_utf8_lossy(&falsch));
    }
    assert_eq!(schreib(&z), None);
    // a file open for the group, and a link in place of a file
    fs::set_permissions(
        p.pb().join("zuordnung.json"),
        fs::Permissions::from_mode(0o640),
    )
    .expect("chmod");
    assert_eq!(
        Speicher::oeffnen(&p.pb(), zaehlend(), T).err(),
        Some(fehler::UNSICHER)
    );
    fs::remove_file(p.pb().join("zuordnung.json")).expect("rm");
    fs::write(p.0.join("echt.json"), &z).expect("write");
    symlink(p.0.join("echt.json"), p.pb().join("zuordnung.json")).expect("link");
    assert_eq!(
        Speicher::oeffnen(&p.pb(), zaehlend(), T).err(),
        Some(fehler::UNSICHER)
    );
    fs::remove_file(p.pb().join("zuordnung.json")).expect("rm");
    symlink(p.0.join("fehlt.json"), p.pb().join("zuordnung.json")).expect("dangling link");
    assert_eq!(
        Speicher::oeffnen(&p.pb(), zaehlend(), T).err(),
        Some(fehler::UNSICHER)
    );
}

#[test]
fn schluessel_folgen_der_form_und_steigen() {
    let s = |text: &str| schluessel_lesen(format!("{text}\n").as_bytes());
    let w1 = "01".repeat(32);
    let w2 = "02".repeat(32);
    assert_eq!(
        s(&format!(
            "{{\"fassung\":1,\"schluessel\":[{{\"ab_ms\":1,\"wert\":\"{w1}\"}},{{\"ab_ms\":2,\"wert\":\"{w2}\"}}]}}"
        )),
        Ok([2; 32])
    );
    for falsch in [
        "{\"fassung\":1,\"schluessel\":[]}".to_owned(),
        format!(
            "{{\"fassung\":1,\"schluessel\":[{{\"ab_ms\":2,\"wert\":\"{w1}\"}},{{\"ab_ms\":2,\"wert\":\"{w2}\"}}]}}"
        ),
        format!(
            "{{\"fassung\":1,\"schluessel\":[{{\"ab_ms\":1,\"wert\":\"{w1}\"}},{{\"ab_ms\":2,\"wert\":\"{w1}\"}}]}}"
        ),
        format!(
            "{{\"fassung\":1,\"schluessel\":[{{\"ab_ms\":1,\"wert\":\"{}\"}}]}}",
            "0G".repeat(32)
        ),
        format!("{{\"fassung\":1,\"schluessel\":[{{\"ab_ms\":1,\"wert\":\"{w1}\",\"x\":1}}]}}"),
    ] {
        assert_eq!(s(&falsch), Err(fehler::UNGUELTIG), "{falsch}");
    }
}

#[test]
fn eine_person_braucht_eine_kennung_und_ein_neues_geheimnis() {
    let p = Probe::neu("person");
    let s = Speicher::oeffnen(&p.pb(), fest(7), T).expect("open");
    assert_eq!(
        s.person("Konto Beispiel", T).err(),
        Some("feld_ungueltig:kennung")
    );
    assert_eq!(s.person("", T).err(), Some("feld_ungueltig:kennung"));
    assert!(s.person("uid-1", T).is_ok());
    // the same randomness again: the secret would be twice
    assert_eq!(s.person("uid-2", T).err(), Some("geheimnis_doppelt"));
    assert_eq!(s.pseudonym("uid-2", T), None);
    // without randomness there is no new person
    let ohne = Speicher::oeffnen(&p.pb(), Box::new(|| None), T).expect("open");
    assert_eq!(ohne.person("uid-3", T).err(), Some(fehler::IO));
    assert!(
        ohne.person("uid-1", T).is_ok(),
        "a known person needs no randomness"
    );
}

#[test]
fn der_zufall_des_systems_gibt_verschiedene_werte() {
    let z = zufall_des_systems();
    let a = z().expect("random");
    let b = z().expect("random");
    assert_ne!(a, b);
}

#[test]
fn die_grenzen_der_dateien_und_der_zuordnung() {
    let p = Probe::neu("grenzen");
    let ordner = openat(CWD, p.pb().as_path(), ORDNER, Mode::empty()).expect("folder");
    let euid = rustix::process::geteuid().as_raw();
    let f = p.pb().join("f.json");
    fs::write(&f, b"12345").expect("file");
    fs::set_permissions(&f, fs::Permissions::from_mode(0o600)).expect("chmod");
    assert_eq!(
        lesen(ordner.as_fd(), "f.json", 5, euid),
        Ok(Some(b"12345".to_vec()))
    );
    assert_eq!(
        lesen(ordner.as_fd(), "f.json", 4, euid),
        Err(fehler::UNGUELTIG)
    );
    assert_eq!(lesen(ordner.as_fd(), "fehlt.json", 5, euid), Ok(None));
    assert!(passt(0) && passt(ZUORDNUNG_MAX));
    assert!(!passt(ZUORDNUNG_MAX + 1));
}

#[test]
fn fassung_und_felder_der_dateien_sind_genau() {
    let w1 = "01".repeat(32);
    let g = "03".repeat(32);
    // the keys: exactly `fassung` 1 and `schluessel`
    for falsch in [
        format!("{{\"fassung\":2,\"schluessel\":[{{\"ab_ms\":1,\"wert\":\"{w1}\"}}]}}\n"),
        format!("{{\"fassung\":1,\"schluessel\":[{{\"ab_ms\":1,\"wert\":\"{w1}\"}}],\"x\":1}}\n"),
    ] {
        assert_eq!(
            schluessel_lesen(falsch.as_bytes()),
            Err(fehler::UNGUELTIG),
            "{falsch}"
        );
    }
    // a person: `seit_ms` a number and `texte` an object, each on its own
    let person = |seit: &str, texte: &str| {
        format!(
            "{{\"fassung\":1,\"personen\":{{\"uid-1\":{{\"geheimnis\":\"{g}\",\"seit_ms\":{seit},\"texte\":{texte}}}}}}}\n"
        )
    };
    assert!(zuordnung_lesen(person("1", "{}").as_bytes()).is_ok());
    for (seit, texte) in [("1", "[]"), ("\"1\"", "{}"), ("1", "1")] {
        assert_eq!(
            zuordnung_lesen(person(seit, texte).as_bytes()).err(),
            Some(fehler::UNGUELTIG),
            "{seit} {texte}"
        );
    }
}
