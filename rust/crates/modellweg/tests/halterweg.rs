//! The holder path of mode 2 (contract 8, section 5.1) against the real holder (`halter`, a dev-dependency of this
//! crate) and against a stand-in of him. The holder runs with a test `halter.json` and a test secret; no test uses a
//! real token, and no test opens a connection to anything but the loopback.
//!
//! The holder itself is Linux only, like mode 2.
#![cfg(target_os = "linux")]

mod gemeinsam;

use std::time::Duration;

use gemeinsam::{Halterattrappe, Wegwerf};
use modellweg::halter::{self, Klient};

fn uid() -> u32 {
    rustix::process::getuid().as_raw()
}

#[test]
fn the_endpoint_is_the_fixed_path_of_the_own_uid() {
    assert_eq!(
        halter::fester_pfad(uid()),
        std::path::PathBuf::from(format!("/run/user/{}/wb-halter/halter.sock", uid()))
    );
}

/// The check of the counterparty (section 5.1): the folder is 0700 and the socket 0600 of the own uid, and the uid of
/// the server out of `SO_PEERCRED` is the own one. A folder or a socket with wider rights is `mw.halter_unecht`, a
/// missing endpoint is `mw.halter_fehlt`.
#[test]
fn the_counterparty_is_checked_before_the_first_line() {
    use std::os::unix::fs::PermissionsExt;
    let w = Wegwerf::neu("hw-gegen");
    let a = Halterattrappe::stumm(&w);
    assert!(Klient::verbinde(&a.pfad(), uid()).is_ok());
    // A socket with group rights.
    std::fs::set_permissions(a.pfad(), std::fs::Permissions::from_mode(0o660)).expect("chmod");
    assert_eq!(
        Klient::verbinde(&a.pfad(), uid()).err(),
        Some("mw.halter_unecht")
    );
    std::fs::set_permissions(a.pfad(), std::fs::Permissions::from_mode(0o600)).expect("chmod");
    // A folder others may enter.
    let ordner = a.pfad().parent().expect("folder").to_path_buf();
    std::fs::set_permissions(&ordner, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    assert_eq!(
        Klient::verbinde(&a.pfad(), uid()).err(),
        Some("mw.halter_unecht")
    );
    std::fs::set_permissions(&ordner, std::fs::Permissions::from_mode(0o700)).expect("chmod");
    // Another uid as the expected one: the peer check fails, never the connection.
    assert_eq!(
        Klient::verbinde(&a.pfad(), uid() + 1).err(),
        Some("mw.halter_unecht")
    );
    // Nothing at the path at all.
    assert_eq!(
        Klient::verbinde(&w.pfad("gibt-es-nicht"), uid()).err(),
        Some("mw.halter_fehlt")
    );
    // A file instead of a socket: the connection fails.
    let datei = w.pfad("datei");
    std::fs::write(&datei, b"x").expect("write");
    std::fs::set_permissions(&datei, std::fs::Permissions::from_mode(0o600)).expect("chmod");
    assert_eq!(
        Klient::verbinde(&datei, uid()).err(),
        Some("mw.halter_fehlt")
    );
}

/// A holder that says nothing: the wait ends with the deadline, and the model path answers `mw.halter_frist`. The
/// deadline of the contract is 600 s; the test sets a short one on the same code.
#[test]
fn a_silent_holder_ends_with_the_deadline() {
    let w = Wegwerf::neu("hw-frist");
    let a = Halterattrappe::stumm(&w);
    let mut k = Klient::verbinde(&a.pfad(), uid()).expect("holder");
    k.frist(Some(Duration::from_millis(200))).expect("deadline");
    let id = k.naechste_id();
    k.senden(&halter::modell_aufruf(id, "a.b", "GET", "/x", None, &[], b"").expect("line"))
        .expect("send");
    assert_eq!(k.naechste_zeile().err(), Some("mw.halter_frist"));
}

/// A holder that closes without an answer, and one that sends a line outside the closed form: both are
/// `mw.halter_fehler`, never a fall back on anything.
#[test]
fn a_torn_connection_and_an_unreadable_line_are_a_failure_of_the_holder() {
    let w = Wegwerf::neu("hw-abriss");
    let zu = Halterattrappe::neu(&w, vec!["ZU".to_owned()]);
    let mut k = Klient::verbinde(&zu.pfad(), uid()).expect("holder");
    let id = k.naechste_id();
    k.senden(&halter::modell_suche(id, "a.b", &["/x".to_owned()]).expect("line"))
        .expect("send");
    assert_eq!(k.naechste_zeile().err(), Some("mw.halter_fehler"));
    let w2 = Wegwerf::neu("hw-abriss2");
    let leer = Halterattrappe::neu(
        &w2,
        vec![r#"{"art":"erfunden","fassung":1,"id":1}"#.to_owned()],
    );
    let mut k = Klient::verbinde(&leer.pfad(), uid()).expect("holder");
    let id = k.naechste_id();
    k.senden(&halter::modell_suche(id, "a.b", &["/x".to_owned()]).expect("line"))
        .expect("send");
    assert_eq!(k.naechste_zeile().err(), Some("mw.halter_fehler"));
}

/// Step 3 of the start (section 13.5) against a stand-in of the holder: one question `schluessel.vorhanden` with
/// `ziel` per bound target, each on its own connection, and the mapping of every answer to a code of the start.
mod schritt_drei {
    use std::io::{BufRead, BufReader};
    use std::os::unix::fs::PermissionsExt;
    use std::time::{Duration, Instant};

    use modellweg::halter;
    use modellweg::start::{
        Abbruch, Startfehler, abbruch_melden, halter_bereit, halter_bereit_mit_code,
    };

    use super::{Halterattrappe, Wegwerf, uid};

    const JA: &str = r#"{"art":"antwort","fassung":1,"id":<ID>,"ok":true,"vorhanden":true}"#;
    const NEIN: &str = r#"{"art":"antwort","fassung":1,"id":<ID>,"ok":true,"vorhanden":false}"#;

    fn fehler(code: &str) -> String {
        format!(
            r#"{{"art":"antwort","code":"{code}","fassung":1,"grund":"x","id":<ID>,"ok":false}}"#
        )
    }

    fn bald() -> Instant {
        Instant::now() + Duration::from_secs(10)
    }

    /// The good way: one line per target, in the order of the list, each the canonical line with `ziel` and never
    /// `name`; the start goes on.
    #[test]
    fn every_bound_target_is_asked_by_its_target() {
        let w = Wegwerf::neu("s3-gut");
        let a = Halterattrappe::neu(&w, vec![JA.to_owned()]);
        assert_eq!(
            halter_bereit(&a.pfad(), uid(), &["a.b", "api.beispiel.invalid"], bald()),
            Ok(())
        );
        assert_eq!(
            a.empfangen(),
            vec![
                r#"{"fassung":1,"id":1,"op":"schluessel.vorhanden","ziel":"a.b"}"#.to_owned(),
                r#"{"fassung":1,"id":1,"op":"schluessel.vorhanden","ziel":"api.beispiel.invalid"}"#
                    .to_owned(),
            ]
        );
        // No bound target: no question, nothing reaches the holder.
        let w = Wegwerf::neu("s3-leer");
        let a = Halterattrappe::neu(&w, vec![NEIN.to_owned()]);
        assert_eq!(halter_bereit(&a.pfad(), uid(), &[], bald()), Ok(()));
        assert_eq!(a.empfangen(), Vec::<String>::new());
    }

    /// `{"vorhanden": false}` and `halter.ziel_unbekannt` both end the start with `mw.halter_kein_schluessel`, exit 78.
    #[test]
    fn false_and_an_unknown_target_are_no_key() {
        for antwort in [NEIN.to_owned(), fehler("halter.ziel_unbekannt")] {
            let w = Wegwerf::neu("s3-nein");
            let a = Halterattrappe::neu(&w, vec![antwort.clone()]);
            let f = halter_bereit(&a.pfad(), uid(), &["a.b"], bald());
            assert_eq!(f, Err(Startfehler::HalterKeinSchluessel), "{antwort}");
            assert_eq!(
                Startfehler::HalterKeinSchluessel.zeile(),
                "mw.halter_kein_schluessel: kein Schluessel beim Halter\n"
            );
            assert_eq!(Startfehler::HalterKeinSchluessel.exit(), 78);
        }
        // The first target without a key ends the step: the second one is not asked any more.
        let w = Wegwerf::neu("s3-erstes");
        let a = Halterattrappe::neu(&w, vec![NEIN.to_owned()]);
        assert_eq!(
            halter_bereit(&a.pfad(), uid(), &["a.b", "c.d"], bald()),
            Err(Startfehler::HalterKeinSchluessel)
        );
        assert_eq!(a.empfangen().len(), 1);
    }

    /// Every other code of the holder becomes `mw.halter_fehler` after the mapping of section 5.1; only
    /// `halter.kein_schluessel` keeps its own code there. An answer without `vorhanden`, a line of another kind and an
    /// unreadable line are `mw.halter_fehler` too. Each ends the start with exit 78, never with a start.
    #[test]
    fn every_other_answer_is_a_failure_of_the_holder() {
        for (antwort, soll) in [
            (fehler("halter.rolle"), Startfehler::HalterFehler),
            (fehler("halter.schluessel_fehlt"), Startfehler::HalterFehler),
            (fehler("halter.wert_form"), Startfehler::HalterFehler),
            (fehler("halter.intern"), Startfehler::HalterFehler),
            (fehler("halter.ziel_komprimiert"), Startfehler::HalterFehler),
            (
                fehler("halter.kein_schluessel"),
                Startfehler::HalterKeinSchluessel,
            ),
            (
                r#"{"art":"antwort","fassung":1,"id":<ID>,"ok":true}"#.to_owned(),
                Startfehler::HalterFehler,
            ),
            (
                r#"{"art":"ende","fassung":1,"id":<ID>,"ok":true}"#.to_owned(),
                Startfehler::HalterFehler,
            ),
            (
                r#"{"art":"antwort","fassung":1,"id":<ID>,"ok":true,"vorhanden":"ja"}"#.to_owned(),
                Startfehler::HalterFehler,
            ),
            ("kein json".to_owned(), Startfehler::HalterFehler),
        ] {
            let w = Wegwerf::neu("s3-fehler");
            let a = Halterattrappe::neu(&w, vec![antwort.clone()]);
            assert_eq!(
                halter_bereit(&a.pfad(), uid(), &["a.b"], bald()),
                Err(soll),
                "{antwort}"
            );
        }
        assert_eq!(
            Startfehler::HalterFehler.zeile(),
            "mw.halter_fehler: Halter lehnte ab\n"
        );
        assert_eq!(Startfehler::HalterFehler.exit(), 78);
    }

    /// What the start writes on stderr for a failure of step 3 against `antwort`, with the exit code.
    fn stderr_zu(antwort: &str) -> (u8, Vec<u8>) {
        let w = Wegwerf::neu("s3-zeilen");
        let a = Halterattrappe::neu(&w, vec![antwort.to_owned()]);
        let abbruch =
            halter_bereit_mit_code(&a.pfad(), uid(), &["a.b"], bald()).expect_err("no start");
        let mut err = Vec::new();
        let exit = abbruch_melden(&mut err, &abbruch);
        (exit, err)
    }

    /// A code of the holder in the closed form goes on a second line `mw.halter_grund: <halter-code>`, after the
    /// unchanged first line and with the unchanged exit code. `halter.ziel_unbekannt` is `mw.halter_kein_schluessel`
    /// and carries its code too.
    #[test]
    fn a_code_of_the_holder_stands_on_a_second_line() {
        for (code, erste) in [
            ("halter.rolle", "mw.halter_fehler: Halter lehnte ab\n"),
            ("halter.wert_form", "mw.halter_fehler: Halter lehnte ab\n"),
            (
                "halter.ziel_komprimiert",
                "mw.halter_fehler: Halter lehnte ab\n",
            ),
            (
                "halter.ziel_unbekannt",
                "mw.halter_kein_schluessel: kein Schluessel beim Halter\n",
            ),
            (
                "halter.kein_schluessel",
                "mw.halter_kein_schluessel: kein Schluessel beim Halter\n",
            ),
            // the longest form: 40 characters after the dot
            (
                "halter.abcdefghijklmnopqrstuvwxyz_abcdefghijklm",
                "mw.halter_fehler: Halter lehnte ab\n",
            ),
        ] {
            let (exit, err) = stderr_zu(&fehler(code));
            assert_eq!(exit, 78, "{code}");
            assert_eq!(
                String::from_utf8(err).expect("utf8"),
                format!("{erste}mw.halter_grund: {code}\n"),
                "{code}"
            );
        }
    }

    /// A code outside the form `^halter\.[a-z_]{1,40}$` is not repeated: only the first line stands. Nothing of the
    /// answer (the reason `grund`, the text, a target) goes to stderr either.
    #[test]
    fn a_code_outside_the_form_gives_no_second_line() {
        let zu_lang = format!("halter.{}", "a".repeat(41));
        for code in [
            "halter.ROLLE",
            "halter.Rolle",
            "HALTER.rolle",
            "halter.",
            "halter.a-b",
            "halter.a b",
            "halter.a1",
            "halter.rolle:x",
            "halter.\\u00e4",
            "halter.a\\u0001b",
            "halter.rolle\\n",
            "halter.rolle\\u001b[31m",
            "halter.rolle\\nmw.halter_grund: halter.x",
            "x.rolle",
            "mw.halter_fehler",
            "",
            zu_lang.as_str(),
        ] {
            let (exit, err) = stderr_zu(&fehler(code));
            assert_eq!(exit, 78, "{code}");
            assert_eq!(
                String::from_utf8(err).expect("utf8"),
                "mw.halter_fehler: Halter lehnte ab\n",
                "{code}"
            );
        }
        // an unknown target and the key code keep their first line and give no second one with a wrong form
        let (exit, err) = stderr_zu(&fehler("halter.Ziel_unbekannt"));
        assert_eq!(exit, 78);
        assert_eq!(
            err, b"mw.halter_fehler: Halter lehnte ab\n",
            "a wrong form is another code, not the unknown target"
        );
    }

    /// Without a code of the holder there is no second line: `{"vorhanden": false}`, an answer without `vorhanden`, a
    /// line of another kind, an unreadable line, a torn connection and the deadline.
    #[test]
    fn no_code_of_the_holder_gives_no_second_line() {
        for (antwort, erste) in [
            (
                NEIN.to_owned(),
                "mw.halter_kein_schluessel: kein Schluessel beim Halter\n",
            ),
            (
                r#"{"art":"antwort","fassung":1,"id":<ID>,"ok":true}"#.to_owned(),
                "mw.halter_fehler: Halter lehnte ab\n",
            ),
            (
                r#"{"art":"antwort","fassung":1,"id":<ID>,"ok":true,"vorhanden":"ja"}"#.to_owned(),
                "mw.halter_fehler: Halter lehnte ab\n",
            ),
            (
                r#"{"art":"ende","code":"halter.rolle","fassung":1,"id":<ID>,"ok":false}"#
                    .to_owned(),
                "mw.halter_fehler: Halter lehnte ab\n",
            ),
            (
                "kein json halter.rolle".to_owned(),
                "mw.halter_fehler: Halter lehnte ab\n",
            ),
            ("ZU".to_owned(), "mw.halter_fehler: Halter lehnte ab\n"),
        ] {
            let (exit, err) = stderr_zu(&antwort);
            assert_eq!(exit, 78, "{antwort}");
            assert_eq!(String::from_utf8(err).expect("utf8"), erste, "{antwort}");
        }
        let w = Wegwerf::neu("s3-zeilen-frist");
        let a = Halterattrappe::stumm(&w);
        let abbruch = halter_bereit_mit_code(
            &a.pfad(),
            uid(),
            &["a.b"],
            Instant::now() + Duration::from_millis(300),
        )
        .expect_err("deadline");
        assert_eq!(abbruch, Abbruch::from(Startfehler::HalterFrist));
        let mut err = Vec::new();
        assert_eq!(abbruch_melden(&mut err, &abbruch), 78);
        assert_eq!(err, b"mw.halter_frist: Halter antwortete nicht\n");
    }

    /// The second line belongs to `mw.halter_fehler` and `mw.halter_kein_schluessel` only, and only with the closed
    /// form: a failure of another kind never writes it, even if a code is set by hand.
    #[test]
    fn the_second_line_is_written_for_two_failures_and_the_closed_form_only() {
        let mit = |fehler, code: &str| Abbruch {
            fehler,
            halter_code: Some(code.to_owned()),
        };
        assert_eq!(
            mit(Startfehler::HalterFehler, "halter.rolle").grundzeile(),
            Some("mw.halter_grund: halter.rolle\n".to_owned())
        );
        assert_eq!(
            mit(Startfehler::HalterKeinSchluessel, "halter.rolle").grundzeile(),
            Some("mw.halter_grund: halter.rolle\n".to_owned())
        );
        for fehler in [
            Startfehler::HalterFrist,
            Startfehler::HalterFehlt,
            Startfehler::HalterUnecht,
            Startfehler::Intern,
            Startfehler::ProtokollOrdner,
        ] {
            assert_eq!(mit(fehler, "halter.rolle").grundzeile(), None, "{fehler:?}");
        }
        assert_eq!(
            mit(Startfehler::HalterFehler, "halter.Rolle").grundzeile(),
            None
        );
        assert_eq!(
            mit(Startfehler::HalterFehler, "halter.a\nb").grundzeile(),
            None
        );
        assert_eq!(Abbruch::from(Startfehler::HalterFehler).grundzeile(), None);
        // `halter_bereit` keeps its type: the failure without the code
        let w = Wegwerf::neu("s3-alt");
        let a = Halterattrappe::neu(&w, vec![fehler("halter.rolle")]);
        assert_eq!(
            halter_bereit(&a.pfad(), uid(), &["a.b"], bald()),
            Err(Startfehler::HalterFehler)
        );
    }

    /// A holder that reads the question and closes without an answer: `mw.halter_fehler`, never a start.
    #[test]
    fn a_holder_that_breaks_off_is_a_failure() {
        let w = Wegwerf::neu("s3-abbruch");
        let ordner = w.pfad("halter");
        std::fs::create_dir_all(&ordner).expect("folder");
        std::fs::set_permissions(&ordner, std::fs::Permissions::from_mode(0o700)).expect("chmod");
        let pfad = ordner.join("halter.sock");
        let horcher = std::os::unix::net::UnixListener::bind(&pfad).expect("bind");
        std::fs::set_permissions(&pfad, std::fs::Permissions::from_mode(0o600)).expect("chmod");
        let faden = std::thread::spawn(move || {
            let (strom, _) = horcher.accept().expect("accept");
            let mut zeile = String::new();
            BufReader::new(strom).read_line(&mut zeile).expect("line");
            zeile
        });
        assert_eq!(
            halter_bereit(&pfad, uid(), &["a.b"], bald()),
            Err(Startfehler::HalterFehler)
        );
        assert_eq!(
            faden.join().expect("thread"),
            "{\"fassung\":1,\"id\":1,\"op\":\"schluessel.vorhanden\",\"ziel\":\"a.b\"}\n"
        );
    }

    /// A silent holder: the step ends at its deadline with `mw.halter_frist`, exit 78. The deadline of the start is
    /// `FRIST_START`; the test sets a short one on the same code. A deadline that has passed already sends nothing.
    #[test]
    fn a_silent_holder_ends_the_start_at_the_deadline() {
        assert_eq!(halter::FRIST_START, Duration::from_secs(5));
        let w = Wegwerf::neu("s3-frist");
        let a = Halterattrappe::stumm(&w);
        let vorher = Instant::now();
        assert_eq!(
            halter_bereit(
                &a.pfad(),
                uid(),
                &["a.b"],
                Instant::now() + Duration::from_millis(300)
            ),
            Err(Startfehler::HalterFrist)
        );
        let dauer = vorher.elapsed();
        assert!(dauer >= Duration::from_millis(250), "{dauer:?}");
        assert!(dauer < Duration::from_secs(5), "{dauer:?}");
        assert_eq!(
            Startfehler::HalterFrist.zeile(),
            "mw.halter_frist: Halter antwortete nicht\n"
        );
        assert_eq!(Startfehler::HalterFrist.exit(), 78);
        // The deadline is one for the whole step: past it, the next question does not go out.
        let w = Wegwerf::neu("s3-vorbei");
        let a = Halterattrappe::neu(&w, vec![JA.to_owned()]);
        assert_eq!(
            halter_bereit(&a.pfad(), uid(), &["a.b"], Instant::now()),
            Err(Startfehler::HalterFrist)
        );
        assert_eq!(a.empfangen(), Vec::<String>::new());
    }

    /// The connection is still checked before the question: no endpoint is `mw.halter_fehlt`, a socket with group
    /// rights is `mw.halter_unecht`, and in both cases no line goes out.
    #[test]
    fn the_connection_is_checked_before_the_question() {
        let w = Wegwerf::neu("s3-weg");
        assert_eq!(
            halter_bereit(&w.pfad("gibt-es-nicht"), uid(), &["a.b"], bald()),
            Err(Startfehler::HalterFehlt)
        );
        let a = Halterattrappe::neu(&w, vec![JA.to_owned()]);
        std::fs::set_permissions(a.pfad(), std::fs::Permissions::from_mode(0o660)).expect("chmod");
        assert_eq!(
            halter_bereit(&a.pfad(), uid(), &["a.b"], bald()),
            Err(Startfehler::HalterUnecht)
        );
        assert_eq!(a.empfangen(), Vec::<String>::new());
    }
}

/// The ops of the role `bruecke` against the **real** holder (`halter`, a dev-dependency): the holder runs with a test
/// `halter.json`, a test secret and a stand-in of the target on the loopback, and the client of the model path speaks
/// to him over a socket pair. So the line protocol, the order of checks and the codes of section 5.1 are measured
/// against the real counterpart, not only against a stand-in of him.
mod echt {
    use std::collections::BTreeMap;
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::net::UnixStream;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use ::halter::ablage::{Ablage, EBENEN};
    use ::halter::dienst::Dienst;
    use ::halter::konfig;
    use ::halter::pruefung::Rolle;
    use modellweg::b64;
    use modellweg::halter::{self, Klient, Zeile};
    use serde_json::json;

    /// The invented test secret of these tests; it is never a real token and never tokenlike.
    const GEHEIM: &str = "hgx1-testgeheimnis-0123";
    const JETZT: i64 = 1_795_000_000_000;

    static ZAEHLER: AtomicUsize = AtomicUsize::new(0);

    fn uid() -> u32 {
        rustix::process::getuid().as_raw()
    }

    fn uhr() -> i64 {
        JETZT
    }

    fn ordner(name: &str) -> PathBuf {
        let n = ZAEHLER.fetch_add(1, Ordering::Relaxed);
        let p = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("hw-{}-{n}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).expect("folder");
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o700)).expect("chmod");
        p
    }

    /// A home with the store of the holder; with `mit_geheimnis` the test secret `anmeldung.copilot` lies in it.
    fn heim(name: &str, mit_geheimnis: bool) -> PathBuf {
        let heim = ordner(name);
        let mut g = heim.clone();
        for e in EBENEN {
            g = g.join(e);
            std::fs::create_dir(&g).expect("level");
            std::fs::set_permissions(&g, std::fs::Permissions::from_mode(0o700)).expect("chmod");
        }
        if mit_geheimnis {
            let p = g.join("anmeldung.copilot");
            std::fs::write(&p, format!("{GEHEIM}\n")).expect("secret");
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o600)).expect("chmod");
        }
        heim
    }

    /// A stand-in of a model target on the loopback: it answers every request with `antwort` and keeps what it got.
    /// `tls` is `false`, which `halter.json` allows for `127.0.0.1` only.
    fn ziel_attrappe(antwort: &'static str) -> (u16, Arc<std::sync::Mutex<Vec<String>>>) {
        let horcher = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = horcher.local_addr().expect("addr").port();
        let gesehen = Arc::new(std::sync::Mutex::new(Vec::new()));
        let g = Arc::clone(&gesehen);
        std::thread::spawn(move || {
            for strom in horcher.incoming() {
                let Ok(mut strom) = strom else { break };
                let g = Arc::clone(&g);
                std::thread::spawn(move || {
                    let mut leser = BufReader::new(strom.try_clone().expect("clone"));
                    let mut kopf = String::new();
                    loop {
                        let mut zeile = String::new();
                        match leser.read_line(&mut zeile) {
                            Ok(0) | Err(_) => return,
                            Ok(_) => {}
                        }
                        let fertig = zeile == "\r\n";
                        kopf.push_str(&zeile);
                        if fertig {
                            break;
                        }
                    }
                    g.lock().expect("lock").push(kopf);
                    let _ = strom.write_all(antwort.as_bytes());
                    let _ = strom.flush();
                });
            }
        });
        (port, gesehen)
    }

    /// The real holder for the target `127.0.0.1:<port>` with the paths `pfade`, and a client of the model path on the
    /// other end of a socket pair. The role comes from the effective gid of this process, as in the tests of the
    /// holder himself.
    fn halter_und_klient(
        name: &str,
        port: u16,
        pfade: &[&str],
        mit_geheimnis: bool,
    ) -> (Klient, Arc<Dienst>) {
        let d = halter_dienst(name, port, pfade, mit_geheimnis, "bruecke");
        let (a, b) = UnixStream::pair().expect("pair");
        let dienst = Arc::clone(&d);
        std::thread::spawn(move || dienst.verbindung(b));
        (Klient::auf(a), d)
    }

    /// The real holder with the target `127.0.0.1` as in [`halter_und_klient`]; the effective gid of this process has
    /// the role `rolle`.
    fn halter_dienst(
        name: &str,
        port: u16,
        pfade: &[&str],
        mit_geheimnis: bool,
        rolle: &str,
    ) -> Arc<Dienst> {
        let k = json!({
            "fassung": 1,
            "konten": {uid().to_string(): {"person": null}},
            "rollen": {"bruecke": "wb-test"},
            "ziele": {"127.0.0.1": {"host": "127.0.0.1", "port": port, "tls": false, "praefix": "",
                "kopf": "authorization", "vorsatz": "Bearer ", "schluessel": "anmeldung.copilot",
                "pfade": pfade}},
            "server": {},
            "kids": {},
        });
        let konfig = konfig::lesen(k.to_string().as_bytes()).expect("configuration");
        let heim = heim(name, mit_geheimnis);
        let rollen = BTreeMap::from([(
            rustix::process::getegid().as_raw(),
            Rolle::aus_text(rolle).expect("role"),
        )]);
        Arc::new(Dienst::neu(
            konfig,
            Ablage::neu(&heim, uid()),
            uid(),
            JETZT - 3_600_000,
            rollen,
            None,
            uhr,
        ))
    }

    /// The real holder `d` at a socket `halter.sock` in a folder 0700, socket 0600, as at his fixed path: so the start
    /// step connects and checks the counterparty as in the program. The socket lies in a short throw-away folder
    /// (`Wegwerf`), not under the target folder of cargo: a socket path is limited to `SUN_LEN`.
    fn am_pfad(d: &Arc<Dienst>, w: &super::Wegwerf) -> PathBuf {
        let o = w.pfad("wb-halter");
        std::fs::create_dir(&o).expect("folder");
        std::fs::set_permissions(&o, std::fs::Permissions::from_mode(0o700)).expect("chmod");
        let pfad = o.join("halter.sock");
        let horcher = std::os::unix::net::UnixListener::bind(&pfad).expect("bind");
        std::fs::set_permissions(&pfad, std::fs::Permissions::from_mode(0o600)).expect("chmod");
        let d = Arc::clone(d);
        std::thread::spawn(move || {
            for strom in horcher.incoming() {
                let Ok(strom) = strom else { break };
                let d = Arc::clone(&d);
                std::thread::spawn(move || d.verbindung(strom));
            }
        });
        pfad
    }

    /// Step 3 of the start (section 13.5) against the real holder: a known target with its secret lets the start go
    /// on; a known target without a secret (`{"vorhanden": false}`) and a target the holder does not know
    /// (`halter.ziel_unbekannt`) both end it with `mw.halter_kein_schluessel`; a caller without the role `bruecke`
    /// gets `halter.rolle`, which is `mw.halter_fehler`. Nothing reaches the target.
    #[test]
    fn step_three_asks_the_real_holder_by_target() {
        use modellweg::start::{Startfehler, halter_bereit};
        let bald = || std::time::Instant::now() + std::time::Duration::from_secs(10);
        let (port, gesehen) = ziel_attrappe("HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}");
        let mit = halter_dienst("s3-mit", port, &["GET /models"], true, "bruecke");
        let w = super::Wegwerf::neu("s3-mit");
        let pfad = am_pfad(&mit, &w);
        assert_eq!(halter_bereit(&pfad, uid(), &["127.0.0.1"], bald()), Ok(()));
        assert_eq!(
            halter_bereit(
                &pfad,
                uid(),
                &["127.0.0.1", "api.unbekannt.invalid"],
                bald()
            ),
            Err(Startfehler::HalterKeinSchluessel)
        );
        let ohne = halter_dienst("s3-ohne", port, &["GET /models"], false, "bruecke");
        let w = super::Wegwerf::neu("s3-ohne");
        let pfad = am_pfad(&ohne, &w);
        assert_eq!(
            halter_bereit(&pfad, uid(), &["127.0.0.1"], bald()),
            Err(Startfehler::HalterKeinSchluessel)
        );
        let hub = halter_dienst("s3-hub", port, &["GET /models"], true, "hub");
        let w = super::Wegwerf::neu("s3-hub");
        let pfad = am_pfad(&hub, &w);
        assert_eq!(
            halter_bereit(&pfad, uid(), &["127.0.0.1"], bald()),
            Err(Startfehler::HalterFehler)
        );
        assert!(
            gesehen.lock().expect("lock").is_empty(),
            "the target was touched"
        );
    }

    /// The real holder with the role `hub` refuses the question with `halter.rolle`: the start writes the second line
    /// `mw.halter_grund: halter.rolle` after the unchanged first line, exit 78. A target he does not know is
    /// `mw.halter_kein_schluessel` with `halter.ziel_unbekannt`.
    #[test]
    fn the_real_holder_gives_his_code_on_the_second_line() {
        use modellweg::start::{abbruch_melden, halter_bereit_mit_code};
        let bald = || std::time::Instant::now() + std::time::Duration::from_secs(10);
        let (port, _gesehen) = ziel_attrappe("HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}");
        let hub = halter_dienst("zg-hub", port, &["GET /models"], true, "hub");
        let w = super::Wegwerf::neu("zg-hub");
        let pfad = am_pfad(&hub, &w);
        let a = halter_bereit_mit_code(&pfad, uid(), &["127.0.0.1"], bald()).expect_err("hub");
        let mut err = Vec::new();
        assert_eq!(abbruch_melden(&mut err, &a), 78);
        assert_eq!(
            String::from_utf8(err).expect("utf8"),
            "mw.halter_fehler: Halter lehnte ab\nmw.halter_grund: halter.rolle\n"
        );
        let bruecke = halter_dienst("zg-bruecke", port, &["GET /models"], true, "bruecke");
        let w = super::Wegwerf::neu("zg-bruecke");
        let pfad = am_pfad(&bruecke, &w);
        let a = halter_bereit_mit_code(&pfad, uid(), &["api.unbekannt.invalid"], bald())
            .expect_err("unknown target");
        let mut err = Vec::new();
        assert_eq!(abbruch_melden(&mut err, &a), 78);
        assert_eq!(
            String::from_utf8(err).expect("utf8"),
            "mw.halter_kein_schluessel: kein Schluessel beim Halter\nmw.halter_grund: halter.ziel_unbekannt\n"
        );
    }

    /// The same question on the line level: the real holder answers `vorhanden` for a target by its `ziel`, and
    /// `halter.ziel_unbekannt` (not `false`) for a target he does not know.
    #[test]
    fn the_real_holder_answers_the_question_by_target() {
        let (port, _gesehen) = ziel_attrappe("HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}");
        for (mit_geheimnis, ziel, soll) in [
            (true, "127.0.0.1", Ok(())),
            (false, "127.0.0.1", Err("mw.halter_kein_schluessel")),
            (
                true,
                "api.unbekannt.invalid",
                Err("mw.halter_kein_schluessel"),
            ),
        ] {
            let (mut k, _d) = halter_und_klient("frage", port, &["GET /models"], mit_geheimnis);
            let id = k.naechste_id();
            k.senden(&halter::schluessel_vorhanden(id, ziel).expect("line"))
                .expect("send");
            let zeile = k.naechste_zeile().expect("line");
            match (&soll, &zeile) {
                (
                    Ok(()),
                    Zeile::Antwort {
                        code: None,
                        vorhanden: Some(true),
                        ..
                    },
                ) => {}
                (
                    Err(_),
                    Zeile::Antwort {
                        code: None,
                        vorhanden: Some(false),
                        ..
                    },
                ) => {
                    assert!(!mit_geheimnis);
                }
                (Err(_), Zeile::Antwort { code: Some(c), .. }) => {
                    assert_eq!(c, "halter.ziel_unbekannt");
                }
                _ => panic!("{ziel}: {zeile:?}"),
            }
            let (mut k, _d) = halter_und_klient("frage2", port, &["GET /models"], mit_geheimnis);
            assert_eq!(
                halter::schluessel_da(
                    &mut k,
                    ziel,
                    std::time::Instant::now() + std::time::Duration::from_secs(10)
                ),
                soll,
                "{ziel}"
            );
        }
    }

    /// A good way of `modell.aufruf`: the holder sets the header with the token himself, the answer comes as
    /// `beginn`, `teil` and `ende`, and the value never stands in a line that goes back.
    #[test]
    fn a_call_goes_out_with_the_token_of_the_holder_and_comes_back_as_a_stream() {
        let (port, gesehen) = ziel_attrappe(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 11\r\n\r\n{\"login\":1}",
        );
        let (mut k, _d) = halter_und_klient("ruf", port, &["GET /copilot_internal/user"], true);
        let id = k.naechste_id();
        k.senden(
            &halter::modell_aufruf(
                id,
                "127.0.0.1",
                "GET",
                "/copilot_internal/user",
                None,
                &[("accept".to_owned(), "application/json".to_owned())],
                b"",
            )
            .expect("line"),
        )
        .expect("send");
        let mut koerper = Vec::new();
        let mut status = 0;
        let mut kopf = Vec::new();
        loop {
            match k.naechste_zeile().expect("line of the holder") {
                Zeile::Beginn { status: s, kopf: h } => {
                    status = s;
                    kopf = h;
                }
                Zeile::Teil(d) => koerper.extend_from_slice(&d),
                Zeile::Ende(code) => {
                    assert_eq!(code, None);
                    break;
                }
                Zeile::Antwort { .. } => panic!("a call answers as a stream"),
            }
        }
        assert_eq!(status, 200);
        assert_eq!(koerper, br#"{"login":1}"#);
        // The holder passes the headers of the target after his own block list; the framing is not among them.
        assert!(
            kopf.iter()
                .any(|(n, w)| n == "content-type" && w == "application/json"),
            "{kopf:?}"
        );
        assert!(
            !kopf
                .iter()
                .any(|(n, _)| n == "content-length" || n == "transfer-encoding"),
            "{kopf:?}"
        );
        // The target got the token of the holder and the header `accept` of the request; the model path never had it.
        let an_ziel = gesehen.lock().expect("lock").clone();
        assert_eq!(an_ziel.len(), 1, "{an_ziel:?}");
        assert!(
            an_ziel[0].contains(&format!("authorization: Bearer {GEHEIM}")),
            "{an_ziel:?}"
        );
        assert!(
            an_ziel[0]
                .to_lowercase()
                .contains("accept: application/json"),
            "{an_ziel:?}"
        );
        assert!(
            an_ziel[0]
                .to_lowercase()
                .contains("accept-encoding: identity"),
            "{an_ziel:?}"
        );
    }

    /// A body goes as `koerper_b64` in one piece, and the holder frames it towards the target.
    #[test]
    fn a_body_goes_as_base64_in_one_piece() {
        let (port, gesehen) = ziel_attrappe("HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}");
        let (mut k, _d) = halter_und_klient("koerper", port, &["POST /chat/completions"], true);
        let id = k.naechste_id();
        k.senden(
            &halter::modell_aufruf(
                id,
                "127.0.0.1",
                "POST",
                "/chat/completions",
                Some("x=1"),
                &[("content-type".to_owned(), "application/json".to_owned())],
                br#"{"a":1}"#,
            )
            .expect("line"),
        )
        .expect("send");
        assert!(matches!(
            k.naechste_zeile(),
            Ok(Zeile::Beginn { status: 200, .. })
        ));
        let an_ziel = gesehen.lock().expect("lock").clone();
        assert!(
            an_ziel[0].starts_with("POST /chat/completions?x=1 HTTP/1.1"),
            "{an_ziel:?}"
        );
        assert!(
            an_ziel[0].to_lowercase().contains("content-length: 7"),
            "{an_ziel:?}"
        );
    }

    /// Every code of his order of checks becomes an own code of the model path (section 5.1), and the holder is the
    /// last lock for the path: a path outside `pfade` never reaches the target.
    #[test]
    fn the_codes_of_the_holder_become_own_codes() {
        let (port, gesehen) = ziel_attrappe("HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}");
        let (mut k, _d) = halter_und_klient("codes", port, &["GET /copilot_internal/user"], true);
        // A path no rule covers: stage 15 of the holder, before anything reaches the target.
        let id = k.naechste_id();
        k.senden(
            &halter::modell_aufruf(id, "127.0.0.1", "GET", "/user/repos", None, &[], b"")
                .expect("line"),
        )
        .expect("send");
        let code = match k.naechste_zeile().expect("line") {
            Zeile::Antwort { code, .. } | Zeile::Ende(code) => code.expect("code"),
            _ => panic!("a refusal is one line"),
        };
        assert_eq!(code, "halter.pfad_verboten");
        assert_eq!(halter::auf_mw(&code), "mw.halter_fehler");
        assert!(
            gesehen.lock().expect("lock").is_empty(),
            "the target was touched"
        );
        // An unknown target: stage 14.
        let (mut k, _d) = halter_und_klient("codes2", port, &["GET /copilot_internal/user"], true);
        let id = k.naechste_id();
        k.senden(
            &halter::modell_aufruf(id, "a.b", "GET", "/copilot_internal/user", None, &[], b"")
                .expect("line"),
        )
        .expect("send");
        let code = match k.naechste_zeile().expect("line") {
            Zeile::Antwort { code, .. } | Zeile::Ende(code) => code.expect("code"),
            _ => panic!("a refusal is one line"),
        };
        assert_eq!(code, "halter.ziel_unbekannt");
        assert_eq!(halter::auf_mw(&code), "mw.halter_fehler");
    }

    /// Without a secret for the target the holder answers `halter.kein_schluessel`, which becomes
    /// `mw.halter_kein_schluessel` (503 in the tunnel, section 13.2, stage 9).
    #[test]
    fn a_target_without_a_secret_names_its_own_code() {
        let (port, gesehen) = ziel_attrappe("HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}");
        let (mut k, _d) = halter_und_klient("ohne", port, &["GET /copilot_internal/user"], false);
        let id = k.naechste_id();
        k.senden(
            &halter::modell_aufruf(
                id,
                "127.0.0.1",
                "GET",
                "/copilot_internal/user",
                None,
                &[],
                b"",
            )
            .expect("line"),
        )
        .expect("send");
        let code = match k.naechste_zeile().expect("line") {
            Zeile::Antwort { code, .. } | Zeile::Ende(code) => code.expect("code"),
            _ => panic!("a refusal is one line"),
        };
        assert_eq!(code, "halter.kein_schluessel");
        assert_eq!(halter::auf_mw(&code), "mw.halter_kein_schluessel");
        assert!(
            gesehen.lock().expect("lock").is_empty(),
            "the target was touched"
        );
    }

    /// The leak lock of the holder: a target that mirrors the token back ends the stream with
    /// `halter.leck_gesperrt`, and the model path turns it into `mw.halter_fehler`. No byte of the value comes back.
    #[test]
    fn a_target_that_mirrors_the_token_is_locked_by_the_holder() {
        let antwort: &'static str = Box::leak(
            format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\nToken: {GEHEIM}",
                7 + GEHEIM.len()
            )
            .into_boxed_str(),
        );
        let (port, _gesehen) = ziel_attrappe(antwort);
        let (mut k, _d) = halter_und_klient("leck", port, &["GET /copilot_internal/user"], true);
        let id = k.naechste_id();
        k.senden(
            &halter::modell_aufruf(
                id,
                "127.0.0.1",
                "GET",
                "/copilot_internal/user",
                None,
                &[],
                b"",
            )
            .expect("line"),
        )
        .expect("send");
        let mut gesehen_bytes = Vec::new();
        let code = loop {
            match k.naechste_zeile().expect("line") {
                Zeile::Beginn { .. } => {}
                Zeile::Teil(d) => gesehen_bytes.extend_from_slice(&d),
                Zeile::Ende(code) => break code.expect("the lock gives a code"),
                Zeile::Antwort { code, .. } => break code.expect("code"),
            }
        };
        assert_eq!(code, "halter.leck_gesperrt");
        assert_eq!(halter::auf_mw(&code), "mw.halter_fehler");
        let text = String::from_utf8_lossy(&gesehen_bytes).into_owned();
        assert!(!text.contains(GEHEIM), "the value came back: {text:?}");
    }

    /// `modell.suche` of the search (section 9a, step 3a): the holder searches the given paths for his own secret and
    /// answers only with numbers.
    #[test]
    fn the_search_of_the_holder_answers_only_numbers() {
        let (port, _g) = ziel_attrappe("HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}");
        let arbeit = ordner("suche-in");
        std::fs::write(arbeit.join("harmlos.txt"), b"ohne Geheimnis\n").expect("write");
        std::fs::write(arbeit.join("leck.txt"), format!("x {GEHEIM} y")).expect("write");
        let (mut k, _d) = halter_und_klient("suche", port, &["GET /copilot_internal/user"], true);
        let id = k.naechste_id();
        let pfade = vec![arbeit.to_string_lossy().into_owned()];
        k.senden(&halter::modell_suche(id, "127.0.0.1", &pfade).expect("line"))
            .expect("send");
        match k.naechste_zeile().expect("line") {
            Zeile::Antwort {
                code: None,
                suche: Some(s),
                ..
            } => {
                assert_eq!(s.treffer, 1, "{s:?}");
                assert_eq!(s.dateien, 2, "{s:?}");
                assert_eq!(s.nicht_lesbar, 0, "{s:?}");
            }
            z => panic!("the search answers with numbers: {z:?}"),
        }
    }

    /// A stream over `modell.strom`: the holder answers the 101 without `upgrade` and `connection` (both stand on his
    /// block list), the bytes of the target come as `teil`, and the bytes of the harness go as `strom.teil`.
    #[test]
    fn a_stream_carries_the_bytes_of_both_ways() {
        let horcher = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = horcher.local_addr().expect("addr").port();
        let gesehen = Arc::new(std::sync::Mutex::new(Vec::new()));
        let g = Arc::clone(&gesehen);
        std::thread::spawn(move || {
            let (mut strom, _) = horcher.accept().expect("accept");
            let mut leser = BufReader::new(strom.try_clone().expect("clone"));
            loop {
                let mut zeile = String::new();
                match leser.read_line(&mut zeile) {
                    Ok(0) | Err(_) => return,
                    Ok(_) => {}
                }
                if zeile == "\r\n" {
                    break;
                }
            }
            let _ = strom.write_all(
                b"HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=\r\n\r\n",
            );
            let _ = strom.write_all(b"\x81\x05Hallo");
            let _ = strom.flush();
            // Everything the harness sent into the stream.
            let mut rest = [0_u8; 64];
            if let Ok(n) = std::io::Read::read(&mut strom, &mut rest) {
                g.lock().expect("lock").extend_from_slice(&rest[..n]);
            }
        });
        let (mut k, _d) = halter_und_klient("strom", port, &["GET /responses"], true);
        let id = k.naechste_id();
        k.senden(
            &halter::modell_strom(
                id,
                "127.0.0.1",
                "/responses",
                None,
                &[("sec-websocket-version".to_owned(), "13".to_owned())],
            )
            .expect("line"),
        )
        .expect("send");
        let (status, kopf) = match k.naechste_zeile().expect("line") {
            Zeile::Beginn { status, kopf } => (status, kopf),
            z => panic!("the holder answers the 101 as beginn: {z:?}"),
        };
        assert_eq!(status, 101);
        assert!(
            !kopf
                .iter()
                .any(|(n, _)| n == "upgrade" || n == "connection"),
            "both stand on his block list: {kopf:?}"
        );
        // The way from the harness: the bytes go unchanged into the stream.
        k.senden(&halter::strom_teil(id, b"\x8a\x00").expect("line"))
            .expect("send");
        let frist = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            if gesehen.lock().expect("lock").as_slice() == b"\x8a\x00" {
                break;
            }
            assert!(
                std::time::Instant::now() < frist,
                "the bytes of the harness"
            );
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        // The bytes of the target go as `teil`. The leak lock of the holder holds back the last bytes of the payload
        // until a later part covers them (holder contract, section 8); at the end of the stream they go out as the
        // last `teil` before `ende`. The sum of the parts is therefore the whole frame, measured after `strom.ende`.
        k.frist(Some(std::time::Duration::from_secs(5)))
            .expect("deadline");
        k.senden(&halter::strom_ende(id).expect("line"))
            .expect("send");
        let mut bytes = Vec::new();
        loop {
            match k.naechste_zeile().expect("line") {
                Zeile::Teil(d) => bytes.extend_from_slice(&d),
                Zeile::Ende(code) => {
                    assert_eq!(code, None);
                    break;
                }
                z => panic!("a stream carries parts and one end: {z:?}"),
            }
        }
        assert_eq!(bytes, b"\x81\x05Hallo");
        assert_eq!(b64::kodieren(&bytes), "gQVIYWxsbw==");
    }
}
