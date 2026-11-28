use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::sync::atomic::{AtomicU8, Ordering};

use peerkanal::{ClientVorgabe, Erwartet, verbinden};

use super::*;

const LADEN: i64 = 1_795_179_540_000;

fn einstellungen() -> String {
    json!({
        "fassung": 2, "klassen": ["oeffentlich", "intern", "kundendaten"], "vorgabestufe": "kundendaten",
        "routen": [{"schluessel": "cloud-beispiel", "art": "cloud", "klasse": "intern", "netzziele": []}],
        "kunden": [], "kundendaten_an_cloud": {"vorgabe": "nie", "freigaben": []},
        "mcp": {"tickets": {"lesen": ["get"], "schreiben": ["update"], "freigabe": true, "frist_ms": 60000, "einloesefrist_ms": 30000}}
    })
    .to_string()
}

/// A folder of the test with everything a point needs; removed at the end.
struct Aufbau(PathBuf);

impl Aufbau {
    fn neu(name: &str) -> Self {
        let basis = std::env::temp_dir().canonicalize().expect("tmp");
        let p = basis.join(format!("sh{}{name}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        for d in ["", "k", "pb", "pr"] {
            fs::create_dir_all(p.join(d)).expect("dir");
            fs::set_permissions(p.join(d), fs::Permissions::from_mode(0o700)).expect("chmod");
        }
        let datei = |n: &str, b: &[u8]| {
            fs::write(p.join(n), b).expect("file");
            fs::set_permissions(p.join(n), fs::Permissions::from_mode(0o600)).expect("chmod");
        };
        datei("einstellungen.json", einstellungen().as_bytes());
        datei("h1.key", &[1u8; 32]);
        Self(p)
    }

    fn konfig(&self) -> Konfig {
        let ich = rustix::process::geteuid().as_raw();
        let e = || {
            vec![Erlaubnis::Unix {
                uid: ich,
                gid: None,
            }]
        };
        Konfig {
            ordner: self.0.join("k"),
            erlaubt: [e(), e(), e()],
            einstellungen: self.0.join("einstellungen.json"),
            schluessel_h1: self.0.join("h1.key"),
            protokoll: self.0.join("pr"),
            personenbezug: self.0.join("pb"),
            vorgabe: self.0.join("fehlt.json"),
        }
    }
}

impl Drop for Aufbau {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn zaehlend() -> personen::Zufall {
    let n = Arc::new(AtomicU8::new(1));
    Box::new(move || Some([n.fetch_add(1, Ordering::SeqCst); 32]))
}

fn rahmen(vorgang: &str, eingabe: &Value) -> Vorgang {
    rahmen_lesen(
        json!({"vorgang": vorgang, "eingabe": eingabe})
            .to_string()
            .as_bytes(),
    )
    .expect("frame")
}

fn start() -> Value {
    let s = json!({"fassung": 1, "sitzung": "s-1", "harness": "claude", "modell": "m", "route": "cloud-beispiel",
                   "kunde": null, "arbeitsordner": "/srv/beispiel/projekt"});
    json!({"start": {"text": s.to_string()}})
}

fn antwort(z: &[u8]) -> Value {
    serde_json::from_slice(z).expect("answer")
}

#[test]
fn die_konfiguration_ist_geschlossen() {
    let gut = json!({"fassung": 1, "ordner": "/srv/beispiel/k", "einstellungen": "/srv/beispiel/e.json",
        "schluessel_h1": "/srv/beispiel/h1.key", "protokoll": "/srv/beispiel/pr", "personenbezug": "/srv/beispiel/pb",
        "vorgabe": "/etc/beispiel/vorgabe.json",
        "erlaubt": {"kern": [{"uid": 501}], "hub": [{"uid": 502}], "mensch": [{"uid": 501}, {"uid": 503}]}});
    let k = konfig_lesen(gut.to_string().as_bytes()).expect("config");
    assert_eq!(
        k.erlaubt[2],
        vec![
            Erlaubnis::Unix {
                uid: 501,
                gid: None
            },
            Erlaubnis::Unix {
                uid: 503,
                gid: None
            }
        ]
    );
    let mit = |f: &dyn Fn(&mut Value)| {
        let mut v = gut.clone();
        f(&mut v);
        konfig_lesen(v.to_string().as_bytes()).err()
    };
    let falsch: [&dyn Fn(&mut Value); 8] = [
        &|v| v["fassung"] = json!(2),
        &|v| v["x"] = json!(1),
        &|v| v["ordner"] = json!("relativ/k"),
        &|v| v["erlaubt"]["hub"] = json!([]),
        &|v| v["erlaubt"]["agent"] = json!([{"uid": 1}]),
        &|v| v["erlaubt"]["kern"] = json!([{"uid": 1, "gid": 2}]),
        &|v| v["erlaubt"]["kern"] = json!([{"sid": "S-1-5-18"}]),
        &|v| {
            v.as_object_mut().expect("object").remove("vorgabe");
        },
    ];
    for f in falsch {
        assert_eq!(mit(f), Some("konfig_ungueltig"));
    }
}

#[test]
fn der_rahmen_ist_streng() {
    assert!(rahmen_lesen(br#"{"vorgang":"mcp.offene","eingabe":{}}"#).is_some());
    for falsch in [
        &br#"{"vorgang":"mcp.offene"}"#[..],
        br#"{"vorgang":"mcp.offene","eingabe":{},"aufrufer":"mensch"}"#,
        br#"{"vorgang":"mcp.unbekannt","eingabe":{}}"#,
        br#"{"vorgang":7,"eingabe":{}}"#,
        br#"{"vorgang":"mcp.offene","eingabe":{},"eingabe":{}}"#,
        br#"{"vorgang":"mcp.offene","eingabe":{}} x"#,
        b"[]",
    ] {
        assert!(
            rahmen_lesen(falsch).is_none(),
            "{}",
            String::from_utf8_lossy(falsch)
        );
    }
    let gross = format!(
        r#"{{"vorgang":"mcp.offene","eingabe":{{"x":"{}"}}}}"#,
        "a".repeat(RAHMEN_MAX)
    );
    assert!(rahmen_lesen(gross.as_bytes()).is_none());
    assert_eq!(antwort(&rahmenfehler())["code"], "stelle.eingabe_ungueltig");
}

#[test]
fn erst_die_zeilen_dann_zustand_und_antwort() {
    let a = Aufbau::neu("zeilen");
    let k = a.konfig();
    let mut s = laden_aus(&k, zaehlend(), None).expect("load");
    let r = antwort(&s.ereignis(
        Aufrufer::Kern,
        rahmen("sitzung.starten", &start()),
        LADEN + 1,
    ));
    assert_eq!(r["ergebnis"], "gestartet");
    // the month file of the event by its name: the folder also holds the anchor file
    let datei = k
        .protokoll
        .join(formate::kette::abschnitt("stelle", LADEN + 1).expect("name"));
    let erste = fs::read_to_string(&datei).expect("read");
    assert!(erste.contains("sitzung.gestartet"));
    // a second writer changes the chain: every event with lines fails closed, the state stays
    fs::write(&datei, format!("{erste}x\n")).expect("write");
    let r = antwort(&s.ereignis(
        Aufrufer::Kern,
        rahmen("sitzung.starten", &start()),
        LADEN + 2,
    ));
    assert_eq!(r["code"], "stelle.protokoll_fehler");
    // an event without lines still answers
    let r = antwort(&s.ereignis(
        Aufrufer::Mensch,
        rahmen("mcp.offene", &json!({})),
        LADEN + 3,
    ));
    assert_eq!(r["ergebnis"], "offene");
    // a new point refuses to start on a broken chain
    assert_eq!(
        laden_aus(&k, zaehlend(), None).err(),
        Some("protokoll_gebrochen")
    );
}

#[test]
fn das_pseudonym_kommt_aus_dem_speicher_der_huelle() {
    let a = Aufbau::neu("ps");
    let mut s = laden_aus(&a.konfig(), zaehlend(), None).expect("load");
    let eins = antwort(&s.ereignis(
        Aufrufer::Kern,
        rahmen("person.pseudonym", &json!({"kennung": "uid-501"})),
        LADEN,
    ));
    let zwei = antwort(&s.ereignis(
        Aufrufer::Kern,
        rahmen("person.pseudonym", &json!({"kennung": "uid-501"})),
        LADEN,
    ));
    assert_eq!(eins["ergebnis"], "pseudonym");
    assert_eq!(eins, zwei);
    let z = fs::read_to_string(a.0.join("pb/zuordnung.json")).expect("store");
    assert!(z.contains("uid-501"));
    for d in fs::read_dir(a.0.join("pr")).expect("dir").flatten() {
        assert!(
            !fs::read_to_string(d.path())
                .expect("read")
                .contains("uid-501"),
            "no identifier in the chain"
        );
    }
}

#[test]
fn ohne_sichere_teile_startet_die_stelle_nicht() {
    let a = Aufbau::neu("unsicher");
    let k = a.konfig();
    fs::set_permissions(&k.schluessel_h1, fs::Permissions::from_mode(0o644)).expect("chmod");
    assert_eq!(
        laden_aus(&k, zaehlend(), None).err(),
        Some("datei_unsicher")
    );
    fs::set_permissions(&k.schluessel_h1, fs::Permissions::from_mode(0o600)).expect("chmod");
    fs::write(&k.schluessel_h1, [1u8; 31]).expect("short key");
    assert_eq!(
        laden_aus(&k, zaehlend(), None).err(),
        Some("schluessel_ungueltig")
    );
    fs::write(&k.schluessel_h1, [1u8; 32]).expect("key");
    fs::set_permissions(&k.personenbezug, fs::Permissions::from_mode(0o755)).expect("chmod");
    assert_eq!(
        laden_aus(&k, zaehlend(), None).err(),
        Some(personen::fehler::UNSICHER)
    );
    fs::set_permissions(&k.personenbezug, fs::Permissions::from_mode(0o700)).expect("chmod");
    assert!(laden_aus(&k, zaehlend(), None).is_ok());
}

#[test]
fn ohne_vorgabe_gibt_es_keinen_worker_mit_vorgabe_nach_den_regeln() {
    let a = Aufbau::neu("worker");
    let mut k = a.konfig();
    let anfordern = |s: &mut Stelle| {
        // section 15.3: route and kunde are those of the orchestrator
        let e = json!({"sitzung": "s-1", "rolle": "pruefer", "harness": "claude", "modell": "m-1", "effort": "mittel", "laufend": 0,
                       "route": "cloud-beispiel", "kunde": null});
        antwort(&s.ereignis(Aufrufer::Kern, rahmen("worker.anfordern", &e), LADEN + 5))
    };
    let mut s = laden_aus(&k, zaehlend(), None).expect("load");
    s.ereignis(
        Aufrufer::Kern,
        rahmen("sitzung.starten", &start()),
        LADEN + 1,
    );
    assert_eq!(anfordern(&mut s)["code"], "stelle.vorgabe_fehlt");
    // the default of a user that is not root counts only in a test build that names this owner
    let v = a.0.join("vorgabe.json");
    fs::write(&v, br#"{"fassung":1,"worker":{"hoechstzahl":1,"harness":["claude"],"modelle":["m-1"],"effort":["mittel"]}}"#)
        .expect("default");
    fs::set_permissions(&v, fs::Permissions::from_mode(0o644)).expect("chmod");
    k.vorgabe = v;
    drop(s);
    let mut s = laden_aus(&k, zaehlend(), None).expect("load");
    s.ereignis(
        Aufrufer::Kern,
        rahmen("sitzung.starten", &start()),
        LADEN + 1,
    );
    assert_eq!(
        anfordern(&mut s)["code"],
        "stelle.vorgabe_fehlt",
        "owner not root"
    );
    let ich = rustix::process::geteuid().as_raw();
    if vorgabe::lesen(&k.vorgabe, Some(ich)).is_none() {
        return; // a folder above the temp folder is open for others: the rest cannot be shown here
    }
    let mut s = laden_aus(&k, zaehlend(), Some(ich)).expect("load");
    s.ereignis(
        Aufrufer::Kern,
        rahmen("sitzung.starten", &start()),
        LADEN + 1,
    );
    assert_eq!(anfordern(&mut s)["ergebnis"], "frei");
}

#[test]
fn die_kanaele_pruefen_die_gegenseite_und_bedienen_frames() {
    let a = Aufbau::neu("kanal");
    let k = a.konfig();
    let stelle = laden_aus(&k, zaehlend(), None).expect("load");
    let server = kanaele_anlegen(&k).expect("channels");
    // a second point on the same folder finds the names taken
    assert_eq!(kanaele_anlegen(&k).err(), Some("pk.name_belegt"));
    let (tx, rx) = mpsc::channel();
    for (art, s) in server {
        let s = Arc::new(s);
        let tx = tx.clone();
        std::thread::spawn(move || annehmen_schleife(&s, art, &tx));
    }
    drop(tx);
    std::thread::spawn(move || schleife(stelle, &rx));
    let ich = rustix::process::geteuid().as_raw();
    let client = |name: &str, uid: u32| ClientVorgabe {
        name: Kennung::neu(name).expect("name"),
        erwartet: Erwartet::Unix { uid },
        frist: Duration::from_secs(2),
        unix_ordner: Some(k.ordner.clone()),
    };
    let frage = |name: &str, zeile: &str| -> Value {
        let mut v = verbinden(&client(name, ich)).expect("connect");
        v.write_all(zeile.as_bytes()).expect("write");
        let mut leser = BufReader::new(v);
        let mut a = Vec::new();
        leser.read_until(b'\n', &mut a).expect("read");
        antwort(&a)
    };
    let s = json!({"vorgang": "sitzung.starten", "eingabe": start()}).to_string() + "\n";
    assert_eq!(frage("stelle-kern", &s)["ergebnis"], "gestartet");
    // the kind is the kind of the channel: the same frame on the channel of the human is refused
    assert_eq!(
        frage("stelle-mensch", &s)["code"],
        "stelle.aufrufer_nicht_erlaubt"
    );
    assert_eq!(
        frage(
            "stelle-mensch",
            "{\"vorgang\":\"mcp.offene\",\"eingabe\":{}}\n"
        )["ergebnis"],
        "offene"
    );
    // a frame error answers once and closes
    assert_eq!(
        frage("stelle-hub", "kein json\n")["code"],
        "stelle.eingabe_ungueltig"
    );
    // a client that expects another owner never reaches the point
    let fremd = verbinden(&client("stelle-kern", ich + 1))
        .err()
        .map(|f| f.code);
    assert_eq!(fremd, Some(peerkanal::Code::FalscherServer));
}

/// A FIFO of the own user with the mode 0600 and a writer that sends `bytes` once a reader opens it: no regular file, but
/// readable to the end, so only the check of the type refuses it.
fn fifo_mit(p: &Path, bytes: &'static [u8]) -> std::thread::JoinHandle<()> {
    let status = std::process::Command::new("mkfifo")
        .args(["-m", "600"])
        .arg(p)
        .status()
        .expect("mkfifo");
    assert!(status.success(), "mkfifo");
    let ziel = p.to_path_buf();
    std::thread::spawn(move || {
        use std::io::Write;
        if let Ok(mut f) = fs::OpenOptions::new().write(true).open(&ziel) {
            let _ = f.write_all(bytes);
        }
    })
}

#[test]
fn der_schluessel_des_dienstes_ist_genau_0600_und_32_byte() {
    let a = Aufbau::neu("schluessel");
    let p = a.0.join("sk");
    let schreiben = |bytes: &[u8], modus: u32| {
        let _ = fs::remove_file(&p);
        fs::write(&p, bytes).expect("write");
        fs::set_permissions(&p, fs::Permissions::from_mode(modus)).expect("chmod");
    };
    schreiben(&[3; 32], 0o600);
    assert_eq!(schluessel_lesen(&p), Ok([3; 32]));
    assert_eq!(crate::schluessel_h1(&p), Some([3; 32]));
    for (bytes, modus) in [
        (&[3u8; 32][..], 0o644),
        (&[3; 32][..], 0o400),
        (&[3; 32][..], 0o4600),
        (&[3; 31][..], 0o600),
        (&[3; 33][..], 0o600),
    ] {
        schreiben(bytes, modus);
        assert_eq!(
            schluessel_lesen(&p),
            Err("schluessel_fehlt"),
            "{modus:o} {}",
            bytes.len()
        );
    }
    schreiben(&[3; 32], 0o600);
    let link = a.0.join("sl");
    std::os::unix::fs::symlink(&p, &link).expect("link");
    assert_eq!(schluessel_lesen(&link), Err("schluessel_fehlt"));
    assert_eq!(
        schluessel_lesen(&a.0.join("fehlt")),
        Err("schluessel_fehlt")
    );
    assert_eq!(schluessel_lesen(&a.0), Err("schluessel_fehlt"));
    assert_eq!(crate::schluessel_h1(&link), None);
    // a FIFO of the own user with 0600 and 32 bytes: no regular file
    let fifo = a.0.join("sf");
    let gegenseite = fifo_mit(&fifo, &[3; 32]);
    assert_eq!(schluessel_lesen(&fifo), Err("schluessel_fehlt"));
    gegenseite.join().expect("writer");
}

#[test]
fn rahmen_und_zeile_haben_hoechstens_die_grenze() {
    // a frame of exactly RAHMEN_MAX bytes is read, one byte more is not
    let leer = r#"{"vorgang":"mcp.offene","eingabe":{"x":""}}"#.len();
    let rahmen = |n: usize| {
        format!(
            r#"{{"vorgang":"mcp.offene","eingabe":{{"x":"{}"}}}}"#,
            "a".repeat(n - leer)
        )
    };
    assert!(rahmen_lesen(rahmen(RAHMEN_MAX).as_bytes()).is_some());
    assert!(rahmen_lesen(rahmen(RAHMEN_MAX + 1).as_bytes()).is_none());
    // a line: up to RAHMEN_MAX bytes and its LF; nothing without LF
    let zeile = |text: &[u8]| zeile_lesen(&mut std::io::Cursor::new(text.to_vec()));
    let mut genau = vec![b'a'; RAHMEN_MAX];
    genau.push(b'\n');
    assert_eq!(zeile(&genau).map(|z| z.len()), Some(RAHMEN_MAX));
    let mut zu_lang = vec![b'a'; RAHMEN_MAX + 1];
    zu_lang.push(b'\n');
    assert_eq!(zeile(&zu_lang), None);
    assert_eq!(zeile(b"abc"), None);
    assert_eq!(zeile(b""), None);
    assert_eq!(zeile(b"ab\ncd\n"), Some(b"ab".to_vec()));
}

#[test]
fn eine_eigene_datei_ist_regulaer_geschlossen_und_klein() {
    let a = Aufbau::neu("eigen");
    let p = a.0.join("e");
    fs::write(&p, [1u8; 32]).expect("write");
    fs::set_permissions(&p, fs::Permissions::from_mode(0o600)).expect("chmod");
    assert_eq!(eigene_datei(&p, 32), Ok(vec![1; 32]));
    assert_eq!(eigene_datei(&p, 31), Err("datei_unsicher"));
    fs::set_permissions(&p, fs::Permissions::from_mode(0o400)).expect("chmod");
    assert!(eigene_datei(&p, 32).is_ok());
    for modus in [0o640, 0o604] {
        fs::set_permissions(&p, fs::Permissions::from_mode(modus)).expect("chmod");
        assert_eq!(eigene_datei(&p, 32), Err("datei_unsicher"), "{modus:o}");
    }
    // a folder of the own user, closed: no regular file
    fs::create_dir(a.0.join("d")).expect("dir");
    fs::set_permissions(a.0.join("d"), fs::Permissions::from_mode(0o700)).expect("chmod");
    assert_eq!(eigene_datei(&a.0.join("d"), 32), Err("datei_unsicher"));
    fs::set_permissions(&p, fs::Permissions::from_mode(0o600)).expect("chmod");
    std::os::unix::fs::symlink(&p, a.0.join("l")).expect("link");
    assert_eq!(eigene_datei(&a.0.join("l"), 32), Err("datei_unsicher"));
    // a FIFO of the own user, closed, readable to the end: no regular file
    let fifo = a.0.join("f");
    let gegenseite = fifo_mit(&fifo, &[1; 32]);
    assert_eq!(eigene_datei(&fifo, 32), Err("datei_unsicher"));
    gegenseite.join().expect("writer");
}

#[test]
fn die_uhr_der_huelle_ist_die_zeit_des_systems() {
    let jetzt = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_millis();
    let jetzt = i64::try_from(jetzt).expect("ms");
    assert!((jetzt_ms() - jetzt).abs() < 60_000);
}

#[test]
fn laufen_endet_nur_mit_dem_grund_des_starts() {
    let a = Aufbau::neu("laufen");
    assert_eq!(
        laufen(&a.0.join("fehlt.json"), None),
        (78, "konfig_unlesbar")
    );
    let k = a.0.join("k.json");
    fs::write(&k, "{}").expect("write");
    let (code, grund) = laufen(&k, None);
    assert_eq!(code, 78);
    assert_eq!(Some(grund), konfig_lesen(b"{}").err());
}
