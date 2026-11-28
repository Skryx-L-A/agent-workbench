#![cfg(unix)] // the stand-in of the point is a Unix socket

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::sync::atomic::{AtomicUsize, Ordering};

use peerkanal::{Annahme, Erlaubnis, Server, ServerVorgabe, SocketModus, UnixOrt};

use super::*;

/// A folder 0700 for the sockets of the stand-in of the point; removed at the end.
struct Ordner(PathBuf);

impl Ordner {
    fn neu(name: &str) -> Self {
        let basis = std::env::temp_dir().canonicalize().expect("tmp");
        let p = basis.join(format!("ds{}{name}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir(&p).expect("dir");
        fs::set_permissions(&p, fs::Permissions::from_mode(0o700)).expect("chmod");
        Self(p)
    }
}

impl Drop for Ordner {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn ich() -> u32 {
    rustix::process::geteuid().as_raw()
}

fn ort(o: &Ordner, uid: u32) -> Ort {
    Ort {
        ordner: Some(o.0.clone()),
        erwartet: Erwartet::Unix { uid },
        nachweis: None,
    }
}

/// The stand-in of the point: one channel, answers each connection with the next line of `antworten` (an empty line closes
/// without an answer) and records every frame it got.
fn attrappe(
    o: &Ordner,
    kanal: &str,
    antworten: Vec<String>,
) -> (Arc<Mutex<Vec<Value>>>, std::thread::JoinHandle<()>) {
    let roh = antworten
        .into_iter()
        .map(|a| {
            if a.is_empty() {
                Vec::new()
            } else {
                format!("{a}\n").into_bytes()
            }
        })
        .collect();
    attrappe_roh(o, kanal, roh)
}

/// [`attrappe`] with the bytes of each answer as they are (no LF added). An answer longer than the bound meets a client that
/// has stopped reading; the write fails then (`EPIPE`). A handler for `SIGPIPE` keeps that from ending the test process in
/// case the signal is not ignored in it.
fn attrappe_roh(
    o: &Ordner,
    kanal: &str,
    antworten: Vec<Vec<u8>>,
) -> (Arc<Mutex<Vec<Value>>>, std::thread::JoinHandle<()>) {
    static SIGPIPE: std::sync::Once = std::sync::Once::new();
    SIGPIPE.call_once(|| {
        let _ = signal_hook::flag::register(
            signal_hook::consts::SIGPIPE,
            Arc::new(std::sync::atomic::AtomicBool::new(false)),
        );
    });
    let mut v = ServerVorgabe::neu(
        Kennung::neu(kanal).expect("name"),
        vec![Erlaubnis::Unix {
            uid: ich(),
            gid: None,
        }],
        4,
    );
    v.unix = Some(UnixOrt {
        ordner: o.0.clone(),
        modus: SocketModus::M0600,
    });
    let server = Server::anlegen(v).expect("server");
    let gesehen = Arc::new(Mutex::new(Vec::new()));
    let g = Arc::clone(&gesehen);
    let faden = std::thread::spawn(move || {
        for antwort in antworten {
            let Ok(Annahme::Vorlaeufig(vl)) = server.annehmen() else {
                continue;
            };
            let Ok(v) = vl.bestaetigen() else { continue };
            let mut leser = BufReader::new(v);
            let mut zeile = Vec::new();
            if leser.read_until(b'\n', &mut zeile).unwrap_or(0) > 0 {
                g.lock()
                    .expect("lock")
                    .push(serde_json::from_slice(&zeile).expect("frame"));
            }
            if !antwort.is_empty() {
                let _ = leser.get_mut().write_all(&antwort);
            }
        }
    });
    (gesehen, faden)
}

const MF: &str = "mf-1795179540000-1";
/// A digest of the point (64 hex digits).
const D: &str = "d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0";

#[test]
fn die_freigabe_geht_mit_genau_drei_werten_und_die_antwort_ist_fest() {
    let o = Ordner::neu("ent");
    let erteilt = json!({"ok": true, "ergebnis": "erteilt", "anfrage": MF, "einloesen_bis_ms": 7})
        .to_string();
    let abgelehnt = json!({"ok": true, "ergebnis": "abgelehnt", "anfrage": MF}).to_string();
    let (gesehen, f) = attrappe(&o, KANAL_MENSCH, vec![erteilt, abgelehnt]);
    let ort = ort(&o, ich());
    assert_eq!(
        entscheiden(&ort, MF, "erteilen", "h1:x"),
        Ok(Entschieden::Erteilt)
    );
    assert_eq!(
        entscheiden(&ort, MF, "ablehnen", "h1:y"),
        Ok(Entschieden::Abgelehnt)
    );
    f.join().expect("stand-in");
    let g = gesehen.lock().expect("lock");
    assert_eq!(
        g[0],
        json!({"vorgang": "mcp.entscheiden", "eingabe": {"anfrage": MF, "entscheidung": "erteilen", "nachweis": "h1:x"}})
    );
    assert_eq!(
        g[1]["eingabe"].as_object().map(Map::len),
        Some(3),
        "no reason goes to the point"
    );
}

#[test]
fn jeder_ausfall_ist_keine_freigabe() {
    let o = Ordner::neu("aus");
    let ort_ = ort(&o, ich());
    // no point at all
    assert_eq!(
        entscheiden(&ort_, MF, "erteilen", "h1:x"),
        Err(Fehlschlag::Dienst(NICHT_ERREICHBAR))
    );
    let antworten = vec![
        String::new(),
        "kein json".to_owned(),
        json!({"ok": true, "ergebnis": "erteilt", "anfrage": "mf-1795179540000-2", "einloesen_bis_ms": 7}).to_string(),
        json!({"ok": true, "ergebnis": "erteilt", "anfrage": MF}).to_string(),
        json!({"ok": true, "ergebnis": "erteilt", "anfrage": MF, "einloesen_bis_ms": 7, "x": 1}).to_string(),
        json!({"ok": true, "ergebnis": "eingeloest", "anfrage": MF}).to_string(),
        json!({"ok": false, "code": "stelle.nachweis_ungueltig", "grund": "Nachweis ungueltig"}).to_string(),
        json!({"ok": false, "code": "dienst.x", "grund": "x"}).to_string(),
    ];
    let (_, f) = attrappe(&o, KANAL_MENSCH, antworten);
    let erwartet = [
        Err(Fehlschlag::Dienst(NICHT_ERREICHBAR)),
        Err(Fehlschlag::Dienst(ANTWORT_UNGUELTIG)),
        Err(Fehlschlag::Dienst(ANTWORT_UNGUELTIG)),
        Err(Fehlschlag::Dienst(ANTWORT_UNGUELTIG)),
        Err(Fehlschlag::Dienst(ANTWORT_UNGUELTIG)),
        Err(Fehlschlag::Dienst(ANTWORT_UNGUELTIG)),
        Err(Fehlschlag::Stelle("stelle.nachweis_ungueltig".to_owned())),
        Err(Fehlschlag::Dienst(ANTWORT_UNGUELTIG)),
    ];
    for (n, e) in erwartet.into_iter().enumerate() {
        assert_eq!(entscheiden(&ort_, MF, "erteilen", "h1:x"), e, "answer {n}");
    }
    f.join().expect("stand-in");
}

#[test]
fn ein_server_mit_fremder_kennung_bekommt_kein_byte() {
    let o = Ordner::neu("fremd");
    let (gesehen, f) = attrappe(&o, KANAL_MENSCH, vec![String::new()]);
    assert_eq!(
        entscheiden(&ort(&o, ich() + 1), MF, "erteilen", "h1:x"),
        Err(Fehlschlag::Dienst(NICHT_ERREICHBAR))
    );
    f.join().expect("stand-in");
    assert!(gesehen.lock().expect("lock").is_empty());
}

#[test]
fn offene_pseudonym_und_worker_folgen_ihrer_form() {
    let o = Ordner::neu("form");
    let offen = json!({"ok": true, "ergebnis": "offene", "offene": [
        {"anfrage": MF, "server": "tickets", "sitzung": "s-1", "werkzeug": "update", "vorschau": null,
         "vorschau_sha256": null, "digest": D, "frist_bis_ms": 9}]});
    let mut ohne_sitzung = offen.clone();
    ohne_sitzung["offene"][0]
        .as_object_mut()
        .expect("entry")
        .remove("sitzung");
    let (_, f) = attrappe(
        &o,
        KANAL_MENSCH,
        vec![offen.to_string(), ohne_sitzung.to_string()],
    );
    let ort_ = ort(&o, ich());
    let liste = offene(&ort_).expect("open");
    assert_eq!(
        (
            liste[0].sitzung.as_str(),
            liste[0].frist_bis_ms,
            liste[0].vorschau.clone()
        ),
        ("s-1", 9, None)
    );
    assert_eq!(offene(&ort_), Err(Fehlschlag::Dienst(ANTWORT_UNGUELTIG)));
    f.join().expect("stand-in");
    let ps = "ps_0123456789abcdef0123456789abcdef";
    let antworten = vec![
        json!({"ok": true, "ergebnis": "pseudonym", "pseudonym": ps}).to_string(),
        json!({"ok": true, "ergebnis": "pseudonym", "pseudonym": "uid-501"}).to_string(),
        json!({"ok": true, "ergebnis": "frei", "sitzung": "s-1"}).to_string(),
        json!({"ok": true, "ergebnis": "frei", "sitzung": "s-2"}).to_string(),
        json!({"ok": false, "code": "stelle.worker_hoechstzahl", "grund": "x"}).to_string(),
    ];
    let (gesehen, f) = attrappe(&o, KANAL_KERN, antworten);
    assert_eq!(pseudonym(&ort_, "uid-501").as_deref(), Some(ps));
    assert_eq!(pseudonym(&ort_, "uid-501"), None);
    let a = meldezeile_lesen(b"anfordern pruefer claude m-1 mittel\n").expect("line");
    assert_eq!(worker_anfordern(&ort_, "s-1", &a, 0), Ok(()));
    assert_eq!(
        worker_anfordern(&ort_, "s-1", &a, 0),
        Err(Fehlschlag::Dienst(ANTWORT_UNGUELTIG))
    );
    let hoch = worker_anfordern(&ort_, "s-1", &a, 2);
    assert_eq!(
        hoch,
        Err(Fehlschlag::Stelle("stelle.worker_hoechstzahl".to_owned()))
    );
    assert_eq!(
        meldeantwort(&hoch, "s-1"),
        b"abgelehnt stelle.worker_hoechstzahl\n"
    );
    assert_eq!(meldeantwort(&Ok(()), "s-1"), b"frei s-1\n");
    assert_eq!(
        meldeantwort(&Err(Fehlschlag::Dienst(NICHT_ERREICHBAR)), "s-1"),
        b"abgelehnt stelle_nicht_erreichbar\n"
    );
    f.join().expect("stand-in");
    let g = gesehen.lock().expect("lock");
    assert_eq!(g[0]["eingabe"], json!({"kennung": "uid-501"}));
    assert_eq!(g[2]["eingabe"]["sitzung"], "s-1");
}

#[test]
fn die_meldezeile_hat_eine_feste_grammatik() {
    assert!(meldezeile_lesen(b"anfordern pruefer claude m-1 mittel\n").is_some());
    let lang = format!("anfordern {} claude m-1 mittel\n", "r".repeat(64));
    assert!(meldezeile_lesen(lang.as_bytes()).is_some());
    for falsch in [
        &b"anfordern pruefer claude m-1 mittel"[..],
        b"anfordern pruefer claude m-1\n",
        b"anfordern pruefer claude m-1 mittel extra\n",
        b"anfordern  pruefer claude m-1 mittel\n",
        b"anfordern Pruefer claude m-1 mittel\n",
        b"anfordern pruefer claude m-1 mittel\n\n",
        b"starten pruefer claude m-1 mittel\n",
        b"anfordern s-1 claude m-1 mittel\r\n",
        b"anfordern pruefer claude m/1 mittel\n",
    ] {
        assert!(
            meldezeile_lesen(falsch).is_none(),
            "{}",
            String::from_utf8_lossy(falsch)
        );
    }
    let zu_lang = format!(
        "anfordern {} {} {} mittel\n",
        "r".repeat(64),
        "h".repeat(64),
        "m".repeat(64)
    );
    assert!(zu_lang.len() <= MELDEZEILE_MAX);
    let genau = format!(
        "anfordern {} {} {} {}\n",
        "r".repeat(64),
        "h".repeat(64),
        "m".repeat(64),
        "e".repeat(50)
    );
    assert_eq!(genau.len(), MELDEZEILE_MAX);
    assert!(meldezeile_lesen(genau.as_bytes()).is_some());
    let ueber = format!(
        "anfordern {} {} {} {}\n",
        "r".repeat(64),
        "h".repeat(64),
        "m".repeat(64),
        "e".repeat(64)
    );
    assert!(ueber.len() > MELDEZEILE_MAX);
    assert!(meldezeile_lesen(ueber.as_bytes()).is_none());
}

#[test]
fn lauf_stelle_braucht_die_erwartete_kennung() {
    let gut = json!({"ordner": "/srv/beispiel/stelle", "erwartet": {"uid": 501}});
    assert_eq!(
        ort_lesen(&gut),
        Some(Ort {
            ordner: Some(PathBuf::from("/srv/beispiel/stelle")),
            erwartet: Erwartet::Unix { uid: 501 },
            nachweis: None
        })
    );
    for falsch in [
        json!({"ordner": "/srv/beispiel/stelle"}),
        json!({"ordner": "/srv/beispiel/stelle", "uid": 501}),
        json!({"ordner": "/srv/beispiel/stelle", "kanal": "stelle"}),
        json!({"ordner": "/srv/beispiel/stelle", "erwartet": {}}),
        json!({"ordner": "/srv/beispiel/stelle", "erwartet": {"uid": 501, "gid": 1}}),
        json!({"ordner": "relativ", "erwartet": {"uid": 501}}),
        json!({"ordner": "/srv/beispiel/stelle", "erwartet": {"uid": -1}}),
        json!({"ordner": "/srv/beispiel/stelle", "erwartet": {"uid": 501}, "kanal": "stelle"}),
        json!({"erwartet": {"sid": "S-1-5-18"}}),
        json!("stelle"),
    ] {
        assert_eq!(ort_lesen(&falsch), None, "{falsch}");
    }
}

#[test]
fn der_faden_weckt_die_schleife_und_blockiert_sie_nie() {
    let o = Ordner::neu("faden");
    let offen = json!({"ok": true, "ergebnis": "offene", "offene": []}).to_string();
    let (_, f) = attrappe(&o, KANAL_MENSCH, vec![offen]);
    let geweckt = Arc::new(AtomicUsize::new(0));
    let w = Arc::clone(&geweckt);
    let faden = Faden::starten(
        ort(&o, ich()),
        Box::new(move || {
            w.fetch_add(1, Ordering::SeqCst);
        }),
    );
    assert!(faden.auftrag(Auftrag::Offene));
    let ende = std::time::Instant::now() + Duration::from_secs(5);
    let mut da = Vec::new();
    while da.is_empty() && std::time::Instant::now() < ende {
        std::thread::sleep(Duration::from_millis(10));
        da = faden.abholen();
    }
    assert_eq!(da, vec![Rueckmeldung::Offene(Ok(Vec::new()))]);
    assert_eq!(geweckt.load(Ordering::SeqCst), 1);
    f.join().expect("stand-in");
    // a release while the point is gone: an answer comes, and it is no release
    assert!(faden.auftrag(Auftrag::Entscheiden {
        nr: 3,
        anfrage: MF.to_owned(),
        entscheidung: "erteilen".to_owned(),
        nachweis: "h1:x".to_owned(),
    }));
    let mut da = Vec::new();
    while da.is_empty() && std::time::Instant::now() < ende + Duration::from_secs(5) {
        std::thread::sleep(Duration::from_millis(10));
        da = faden.abholen();
    }
    assert_eq!(
        da,
        vec![Rueckmeldung::Entschieden {
            nr: 3,
            anfrage: MF.to_owned(),
            ergebnis: Err(Fehlschlag::Dienst(NICHT_ERREICHBAR))
        }]
    );
}

#[test]
fn die_ablehnungen_haben_feste_formen() {
    assert_eq!(
        ablehnung(&Fehlschlag::Dienst(NICHT_ERREICHBAR)),
        json!({"grund": NICHT_ERREICHBAR, "code": NICHT_ERREICHBAR, "text": "Stelle nicht erreichbar"})
    );
    assert_eq!(
        ablehnung(&Fehlschlag::Dienst(ANTWORT_UNGUELTIG)),
        json!({"grund": "stelle_antwort_ungueltig", "code": "stelle_antwort_ungueltig", "text": "Antwort der Stelle ungueltig"})
    );
    assert_eq!(
        ablehnung(&Fehlschlag::Stelle("stelle.nachweis_ungueltig".to_owned())),
        json!({"grund": "stelle_abgelehnt", "code": "stelle.nachweis_ungueltig"})
    );
    assert_eq!(zweite_anforderung(), b"abgelehnt anfordern_offen\n");
}

#[test]
fn die_wanduhr_ist_die_zeit_des_systems() {
    let jetzt = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_millis();
    let jetzt = i64::try_from(jetzt).expect("ms");
    assert!((wanduhr_ms() - jetzt).abs() < 60_000);
}

#[test]
fn lauf_stelle_unter_windows_nennt_nur_die_sid() {
    let sid = "S-1-5-18";
    assert_eq!(
        ort_lesen_fuer(&json!({"erwartet": {"sid": sid}}), true),
        Some(Ort {
            ordner: None,
            erwartet: Erwartet::Windows {
                sid: peerkanal::Sid::neu(sid).expect("sid")
            },
            nachweis: None
        })
    );
    for falsch in [
        json!({"erwartet": {"sid": sid}, "ordner": "/srv/beispiel/stelle"}),
        json!({"erwartet": {"uid": 501}}),
        json!({"erwartet": {"sid": "x"}}),
        json!({"ordner": "/srv/beispiel/stelle", "erwartet": {"uid": 501}}),
    ] {
        assert_eq!(ort_lesen_fuer(&falsch, true), None, "{falsch}");
    }
    assert_eq!(
        ort_lesen_fuer(&json!({"erwartet": {"sid": sid}}), false),
        None
    );
}

/// A line of exactly `n` bytes with its LF: the object `{"x": "aaa..."}`.
fn antwort_der_laenge(n: usize) -> String {
    let leer = json!({"x": ""}).to_string().len() + 1;
    json!({"x": "a".repeat(n - leer)}).to_string()
}

#[test]
fn rahmen_und_antwort_haben_mit_lf_hoechstens_die_grenze() {
    let o = Ordner::neu("grenze");
    let ort_ = ort(&o, ich());
    let (gesehen, f) = attrappe(
        &o,
        KANAL_MENSCH,
        vec![
            antwort_der_laenge(RAHMEN_MAX),
            antwort_der_laenge(RAHMEN_MAX + 1),
            String::new(),
        ],
    );
    // the frame `{"eingabe":{"a":"..."},"vorgang":"v"}` with its LF of exactly n bytes
    let eingabe = |n: usize| {
        let leer = json!({"vorgang": "v", "eingabe": {"a": ""}})
            .to_string()
            .len()
            + 1;
        json!({"a": "a".repeat(n - leer)})
    };
    let erste = fragen(&ort_, KANAL_MENSCH, "v", &eingabe(RAHMEN_MAX)).expect("at the bound");
    assert_eq!(erste.len(), 1);
    assert_eq!(
        fragen(&ort_, KANAL_MENSCH, "v", &json!({})),
        Err(NICHT_ERREICHBAR),
        "an answer above the bound"
    );
    assert_eq!(
        fragen(&ort_, KANAL_MENSCH, "v", &eingabe(RAHMEN_MAX + 1)),
        Err(NICHT_ERREICHBAR),
        "a frame above the bound"
    );
    f.join().expect("stand-in");
    let g = gesehen.lock().expect("lock");
    assert_eq!(g.len(), 2, "the frame above the bound sends no byte");
    assert_eq!(g[1], json!({"vorgang": "v", "eingabe": {}}));
}

#[test]
fn eine_antwort_ohne_lf_ist_keine_antwort() {
    let o = Ordner::neu("ohnelf");
    let (_, f) = attrappe_roh(&o, KANAL_MENSCH, vec![b"{\"ok\":true}".to_vec()]);
    assert_eq!(
        fragen(&ort(&o, ich()), KANAL_MENSCH, "v", &json!({})),
        Err(NICHT_ERREICHBAR)
    );
    f.join().expect("stand-in");
}

fn objekt(x: &Value) -> Map<String, Value> {
    x.as_object().expect("object").clone()
}

#[test]
fn die_antworten_haben_genau_ihre_schluessel() {
    let ent = |x: Value| entscheidung_lesen(&objekt(&x), MF);
    assert_eq!(
        ent(json!({"ok": true, "ergebnis": "abgelehnt", "anfrage": MF})),
        Some(Entschieden::Abgelehnt)
    );
    for falsch in [
        json!({"ok": true, "ergebnis": "abgelehnt", "anfrage": MF, "x": 1}),
        json!({"ok": true, "ergebnis": "abgelehnt", "anfrage": "mf-1795179540000-2"}),
        json!({"ok": false, "ergebnis": "abgelehnt", "anfrage": MF}),
        json!({"ok": true, "ergebnis": "erteilt", "anfrage": "mf-1795179540000-2", "einloesen_bis_ms": 7}),
        json!({"ok": true, "ergebnis": "erteilt", "anfrage": MF, "einloesen_bis_ms": "7"}),
    ] {
        assert_eq!(ent(falsch.clone()), None, "{falsch}");
    }
    let eintrag = json!({"anfrage": MF, "server": "tickets", "sitzung": "s-1", "werkzeug": "update", "vorschau": "v",
                         "vorschau_sha256": null, "digest": D, "frist_bis_ms": 9});
    let gut = json!({"ok": true, "ergebnis": "offene", "offene": [eintrag]});
    let liste = offene_lesen(&objekt(&gut)).expect("open");
    assert_eq!(liste[0].vorschau.as_deref(), Some("v"));
    let mut falsch = Vec::new();
    for (k, v) in [
        ("ok", json!(false)),
        ("ergebnis", json!("andere")),
        ("x", json!(1)),
    ] {
        let mut f = gut.clone();
        f[k] = v;
        falsch.push(f);
    }
    let mut zahl = gut.clone();
    zahl["offene"][0]["vorschau"] = json!(3);
    falsch.push(zahl);
    for f in falsch {
        assert_eq!(offene_lesen(&objekt(&f)), None, "{f}");
    }
    let hex = "0123456789abcdef0123456789abcdef";
    let ps = |p: &str| json!({"ok": true, "ergebnis": "pseudonym", "pseudonym": p});
    assert_eq!(
        pseudonym_lesen(&objekt(&ps(&format!("ps_{hex}")))),
        Some(format!("ps_{hex}"))
    );
    for falsch in [
        ps(&format!("ps_{hex}0")),
        ps(&format!("px_{hex}")),
        ps(&format!("ps_{}", hex.to_uppercase())),
        json!({"ok": true, "ergebnis": "anders", "pseudonym": format!("ps_{hex}")}),
        json!({"ok": true, "ergebnis": "pseudonym", "pseudonym": format!("ps_{hex}"), "x": 1}),
    ] {
        assert_eq!(pseudonym_lesen(&objekt(&falsch)), None, "{falsch}");
    }
}

#[test]
fn ein_faden_nur_mit_lauf_stelle() {
    assert!(Faden::aus_lauf(None).is_none());
    let o = Ordner::neu("auslauf");
    assert!(Faden::aus_lauf(Some(ort(&o, ich()))).is_some());
}

#[test]
fn die_form_der_freigabe_folgt_r_l6_dann_die_anzahl() {
    let f = |x: Value| freigabe_form(&objekt(&x));
    assert_eq!(
        f(json!({"art": "Freigabe", "anfragen": [MF]})),
        Ok(Freigabe {
            anfrage: MF.to_owned(),
            entscheidung: "erteilen",
            begruendung: None
        })
    );
    assert_eq!(
        f(
            json!({"art": "Freigabe", "anfragen": [MF], "entscheidung": "ablehnen", "begruendung": "b".repeat(500)})
        ),
        Ok(Freigabe {
            anfrage: MF.to_owned(),
            entscheidung: "ablehnen",
            begruendung: Some("b".repeat(500))
        })
    );
    assert_eq!(
        f(json!({"art": "Freigabe", "anfragen": [MF], "entscheidung": "erteilen"}))
            .map(|g| g.entscheidung),
        Ok("erteilen")
    );
    let ab = |b: Value| json!({"art": "Freigabe", "anfragen": [MF], "entscheidung": "ablehnen", "begruendung": b});
    for (x, grund) in [
        (
            json!({"art": "Freigabe", "anfragen": [MF], "sitzung": "s-1"}),
            "ereignis_ungueltig",
        ),
        (
            json!({"art": "Freigabe", "anfragen": MF}),
            "ereignis_ungueltig",
        ),
        (json!({"art": "Freigabe"}), "ereignis_ungueltig"),
        (
            json!({"art": "Freigabe", "anfragen": [MF, 3]}),
            "ereignis_ungueltig",
        ),
        (
            json!({"art": "Freigabe", "anfragen": [MF], "entscheidung": "vielleicht"}),
            "ereignis_ungueltig",
        ),
        (
            json!({"art": "Freigabe", "anfragen": [MF], "entscheidung": 1}),
            "ereignis_ungueltig",
        ),
        (ab(json!(null)), "ereignis_ungueltig"),
        (
            json!({"art": "Freigabe", "anfragen": [MF], "begruendung": "b"}),
            "begruendung_nur_bei_ablehnen",
        ),
        (ab(json!("")), "begruendung_ungueltig"),
        (ab(json!("b".repeat(501))), "begruendung_ungueltig"),
        (ab(json!("a\nb")), "begruendung_ungueltig"),
        (ab(json!("a\u{7f}")), "begruendung_ungueltig"),
        (ab(json!("a\u{9f}")), "begruendung_ungueltig"),
        (
            json!({"art": "Freigabe", "anfragen": [MF, "mf-2"]}),
            "sammelfreigabe",
        ),
        (json!({"art": "Freigabe", "anfragen": []}), "sammelfreigabe"),
        // the form comes before the count
        (
            json!({"art": "Freigabe", "anfragen": [MF, "mf-2"], "entscheidung": "x"}),
            "ereignis_ungueltig",
        ),
    ] {
        assert_eq!(f(x.clone()), Err(grund), "{x}");
    }
    // the characters at the edges of the forbidden ranges are allowed
    assert!(f(ab(json!("a\u{20}\u{7e}\u{a0}"))).is_ok());
    assert_eq!(
        VERBOTEN,
        ["nachweis", "wer", "konto", "person", "uid", "sid"]
    );
}

#[test]
fn die_lage_der_anfrage_in_ihrer_reihenfolge() {
    assert_eq!(freigabe_lage(true, true, Some("offen"), false), Ok(()));
    for (bekannt, raum, zustand, wartet, grund) in [
        (false, true, Some("offen"), false, "anfrage_unbekannt"),
        (false, false, None, false, "anfrage_unbekannt"),
        (true, false, Some("offen"), false, "fremder_kundenraum"),
        (true, true, Some("abgelaufen"), false, "abgelaufen"),
        (true, true, Some("erteilt"), false, "nicht_offen"),
        (true, true, Some("offen"), true, "nicht_offen"),
        (true, true, None, false, "anfrage_unbekannt"),
    ] {
        assert_eq!(
            freigabe_lage(bekannt, raum, zustand, wartet),
            Err(grund),
            "{bekannt} {raum} {zustand:?} {wartet}"
        );
    }
}

const PS: &str = "ps_0123456789abcdef0123456789abcdef";

fn offen_mit(vorschau_sha256: Option<&str>) -> Offen {
    Offen {
        anfrage: MF.to_owned(),
        sitzung: "s-1".to_owned(),
        werkzeug: "update".to_owned(),
        vorschau: None,
        frist_bis_ms: 9,
        digest: D.to_owned(),
        vorschau_sha256: vorschau_sha256.map(str::to_owned),
    }
}

#[test]
fn den_nachweis_bildet_der_dienst_und_die_stelle_nimmt_ihn_an() {
    use stelle::Nachweispruefer;
    let o = Ordner::neu("nachweis");
    let pfad = o.0.join("schluessel");
    let schluessel = [9u8; 32];
    fs::write(&pfad, schluessel).expect("key");
    fs::set_permissions(&pfad, fs::Permissions::from_mode(0o600)).expect("chmod");
    let ort = Nachweisort {
        schluessel: pfad.clone(),
        konten: [("uid-501".to_owned(), PS.to_owned())]
            .into_iter()
            .collect(),
    };
    let pruefer = stelle::PrueferH1::neu(schluessel);
    let h = "e1".repeat(32);
    for (entscheidung, vorschau) in [("erteilen", None), ("ablehnen", Some(h.as_str()))] {
        let offen = offen_mit(vorschau);
        let n = nachweis_bilden(Some(&ort), "uid-501", &offen, entscheidung).expect("proof");
        let herausforderung = stelle::herausforderung(MF, D, entscheidung, vorschau);
        assert_eq!(
            pruefer.pruefe(&herausforderung, &n),
            Ok(PS.to_owned()),
            "{entscheidung}"
        );
        // the binding of the request: another digest or preview, or the other decision, does not hold
        let andere = if entscheidung == "erteilen" {
            "ablehnen"
        } else {
            "erteilen"
        };
        assert!(
            pruefer
                .pruefe(&stelle::herausforderung(MF, D, andere, vorschau), &n)
                .is_err()
        );
        assert!(
            pruefer
                .pruefe(&stelle::herausforderung(MF, &h, entscheidung, vorschau), &n)
                .is_err()
        );
        assert!(!n.contains(&format!("{schluessel:?}")));
    }
    let offen = offen_mit(None);
    // no account of this name, no configuration
    assert_eq!(
        nachweis_bilden(Some(&ort), "uid-502", &offen, "erteilen"),
        Err(KONTO_OHNE_PERSON)
    );
    assert_eq!(
        nachweis_bilden(None, "uid-501", &offen, "erteilen"),
        Err(KONTO_OHNE_PERSON)
    );
    // the key file: not 0600, missing, a link
    fs::set_permissions(&pfad, fs::Permissions::from_mode(0o640)).expect("chmod");
    assert_eq!(
        nachweis_bilden(Some(&ort), "uid-501", &offen, "erteilen"),
        Err(SCHLUESSEL_FEHLT)
    );
    fs::set_permissions(&pfad, fs::Permissions::from_mode(0o600)).expect("chmod");
    let link = o.0.join("link");
    std::os::unix::fs::symlink(&pfad, &link).expect("link");
    let mut ueber_link = ort.clone();
    ueber_link.schluessel = link;
    assert_eq!(
        nachweis_bilden(Some(&ueber_link), "uid-501", &offen, "erteilen"),
        Err(SCHLUESSEL_FEHLT)
    );
    fs::remove_file(&pfad).expect("remove");
    assert_eq!(
        nachweis_bilden(Some(&ort), "uid-501", &offen, "erteilen"),
        Err(SCHLUESSEL_FEHLT)
    );
}

#[test]
fn lauf_stelle_nennt_den_nachweis_streng() {
    let n = json!({"schluessel": "/srv/beispiel/h1.key", "konten": {"uid-501": PS}});
    let ort = ort_lesen_fuer(
        &json!({"ordner": "/srv/beispiel/stelle", "erwartet": {"uid": 501}, "nachweis": n}),
        false,
    )
    .expect("ort");
    assert_eq!(
        ort.nachweis,
        Some(Nachweisort {
            schluessel: PathBuf::from("/srv/beispiel/h1.key"),
            konten: [("uid-501".to_owned(), PS.to_owned())]
                .into_iter()
                .collect()
        })
    );
    assert!(
        ort_lesen_fuer(
            &json!({"ordner": "/srv/beispiel/stelle", "erwartet": {"uid": 501},
                    "nachweis": {"schluessel": "/k", "konten": {}}}),
            false
        )
        .is_some()
    );
    let mut viele = Map::new();
    for u in 0..=KONTEN_MAX {
        viele.insert(format!("uid-{u}"), json!(PS));
    }
    for falsch in [
        json!({"schluessel": "relativ.key", "konten": {"uid-501": PS}}),
        json!({"schluessel": "/k", "konten": {"uid-0501": PS}}),
        json!({"schluessel": "/k", "konten": {"uid-x": PS}}),
        json!({"schluessel": "/k", "konten": {"sid-S-1-5-18": PS}}),
        json!({"schluessel": "/k", "konten": {"uid-501": "ps_x"}}),
        json!({"schluessel": "/k", "konten": {"uid-501": PS}, "x": 1}),
        json!({"schluessel": "/k"}),
        json!({"schluessel": "/k", "konten": Value::Object(viele)}),
        json!("/k"),
    ] {
        let wert =
            json!({"ordner": "/srv/beispiel/stelle", "erwartet": {"uid": 501}, "nachweis": falsch});
        assert_eq!(ort_lesen_fuer(&wert, false), None, "{wert}");
    }
    // under Windows the accounts are SIDs
    let w = json!({"erwartet": {"sid": "S-1-5-18"}, "nachweis": {"schluessel": "/k", "konten": {"sid-S-1-5-18": PS}}});
    let w_ort = ort_lesen_fuer(&w, true).expect("windows");
    assert!(
        w_ort
            .nachweis
            .is_some_and(|n| n.konten.contains_key("sid-S-1-5-18"))
    );
    let w_uid = json!({"erwartet": {"sid": "S-1-5-18"}, "nachweis": {"schluessel": "/k", "konten": {"uid-1": PS}}});
    assert_eq!(ort_lesen_fuer(&w_uid, true), None);
    assert!(konto_form("uid-0", false) && konto_form("uid-4294967295", false));
    assert!(!konto_form("uid-4294967296", false) && !konto_form("uid-", false));
}

#[test]
fn offene_nennen_digest_und_vorschau_als_sha256() {
    let eintrag = |digest: Value, vs: Value| {
        json!({"ok": true, "ergebnis": "offene", "offene": [{"anfrage": MF, "server": "tickets", "sitzung": "s-1",
            "werkzeug": "update", "vorschau": "v", "vorschau_sha256": vs, "digest": digest, "frist_bis_ms": 9}]})
    };
    let liste = offene_lesen(&objekt(&eintrag(json!(D), json!(D)))).expect("open");
    assert_eq!(
        (
            liste[0].digest.as_str(),
            liste[0].vorschau_sha256.as_deref()
        ),
        (D, Some(D))
    );
    for (digest, vs) in [
        (json!("d"), json!(null)),
        (json!(D.to_uppercase()), json!(null)),
        (json!(null), json!(null)),
        (json!(D), json!("v")),
        (json!(D), json!(3)),
    ] {
        assert_eq!(
            offene_lesen(&objekt(&eintrag(digest.clone(), vs.clone()))),
            None,
            "{digest} {vs}"
        );
    }
}

#[test]
fn das_konto_kommt_nur_aus_der_messung_des_lokalen_wegs() {
    let sid = "S-1-5-21-1000000001-1000000002-1000000003-1001";
    assert_eq!(
        konto_aus(&json!({"system": "linux", "uid": 501, "sid": null, "name": "anna"})),
        Some("uid-501".to_owned())
    );
    assert_eq!(
        konto_aus(&json!({"system": "macos", "uid": 0, "sid": null, "name": "root"})),
        Some("uid-0".to_owned())
    );
    assert_eq!(
        konto_aus(&json!({"system": "linux", "uid": u32::MAX, "sid": null, "name": "x"})),
        Some(format!("uid-{}", u32::MAX))
    );
    assert_eq!(
        konto_aus(&json!({"system": "windows", "uid": null, "sid": sid, "name": "D\\anna"})),
        Some(format!("sid-{sid}"))
    );
    for falsch in [
        json!({"system": "linux", "uid": null, "sid": null, "name": "anna"}),
        json!({"system": "linux", "name": "anna"}),
        json!({"system": "linux", "uid": 501, "sid": sid, "name": "anna"}),
        json!({"system": "linux", "uid": u64::from(u32::MAX) + 1, "sid": null, "name": "x"}),
        json!({"system": "linux", "uid": -1, "sid": null, "name": "x"}),
        json!({"system": "linux", "uid": "501", "sid": null, "name": "x"}),
        json!({"system": "windows", "uid": null, "sid": "S-x", "name": "x"}),
        json!({"system": "windows", "uid": null, "sid": 5, "name": "x"}),
        json!(null),
    ] {
        assert_eq!(konto_aus(&falsch), None, "{falsch}");
    }
}
