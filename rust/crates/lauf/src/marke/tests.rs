//! Tests of the folder mark: the form of the file, the decision (codes, order, rising mark), and the file under the
//! lock with real folders, also with starts at the same time.

use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use super::*;

fn klassen() -> Vec<String> {
    ["oeffentlich", "intern", "kundendaten"]
        .map(str::to_owned)
        .to_vec()
}

/// A key derived from the folder: distinct folders of the tests get distinct keys.
fn k(ordner: &str) -> String {
    let hex = format!("{:0>32}", hex(ordner.as_bytes()));
    hex[hex.len() - 32..].to_owned()
}

/// The randomness of the tests: 1, 2, 3, ... as keys.
fn zz() -> impl FnMut() -> Option<String> {
    let mut n = 0u32;
    move || {
        n += 1;
        Some(format!("{n:032x}"))
    }
}

fn m(ordner: &str, kunde: Option<&str>, klasse: &str) -> Marke {
    Marke {
        ordner: ordner.to_owned(),
        kennung: k(ordner),
        kunde: kunde.map(str::to_owned),
        klasse_hoch: klasse.to_owned(),
    }
}

fn a<'a>(ordner: &'a str, kunde: Option<&'a str>, klasse: &'a str) -> Anfrage<'a> {
    Anfrage {
        ordner,
        kunde,
        klasse,
    }
}

#[test]
fn related_means_the_same_folder_an_ancestor_or_a_descendant_by_whole_segments() {
    assert!(verwandt("/a/b", "/a/b"));
    assert!(verwandt("/a", "/a/b"));
    assert!(verwandt("/a/b/c", "/a/b"));
    assert!(!verwandt("/a/bc", "/a/b"));
    assert!(!verwandt("/a/b", "/a/bc"));
    assert!(!verwandt("/x", "/a"));
}

#[test]
fn a_new_folder_gets_the_customer_and_the_class_of_the_route() {
    let neu = entscheiden(
        Vec::new(),
        a("/p", Some("k"), "intern"),
        &klassen(),
        &mut zz(),
    )
    .expect("ok");
    let mut soll = m("/p", Some("k"), "intern");
    soll.kennung = format!("{:032x}", 1);
    assert_eq!(neu, Some(vec![soll]));
}

#[test]
fn the_mark_rises_and_never_sinks() {
    let alt = vec![m("/p", Some("k"), "intern")];
    assert_eq!(
        entscheiden(
            alt.clone(),
            a("/p", Some("k"), "kundendaten"),
            &klassen(),
            &mut zz()
        ),
        Ok(Some(vec![m("/p", Some("k"), "kundendaten")]))
    );
    // the same class: nothing to write
    assert_eq!(
        entscheiden(
            alt.clone(),
            a("/p", Some("k"), "intern"),
            &klassen(),
            &mut zz()
        ),
        Ok(None)
    );
    // a lower class is refused, the mark stays
    assert_eq!(
        entscheiden(
            vec![m("/p", Some("k"), "kundendaten")],
            a("/p", Some("k"), "intern"),
            &klassen(),
            &mut zz()
        ),
        Err(Fehler::OrdnerHoeherMarkiert)
    );
}

#[test]
fn the_strictest_related_mark_counts_for_ancestors_and_descendants() {
    for ordner in ["/p", "/p/unter", "/p/unter/tief"] {
        let marken = vec![m("/p/unter", Some("k"), "kundendaten")];
        let ist = entscheiden(
            marken,
            a(ordner, Some("k"), "intern"),
            &klassen(),
            &mut zz(),
        );
        let soll = if ordner == "/p" || ordner.starts_with("/p/unter") {
            Err(Fehler::OrdnerHoeherMarkiert)
        } else {
            Ok(None)
        };
        assert_eq!(ist, soll, "{ordner}");
    }
    // a sibling is not related
    let marken = vec![m("/p/unter", Some("k"), "kundendaten")];
    assert!(matches!(
        entscheiden(
            marken,
            a("/p/unterx", Some("k"), "intern"),
            &klassen(),
            &mut zz()
        ),
        Ok(Some(_))
    ));
    // two related marks: the higher one counts
    let marken = vec![
        m("/p", Some("k"), "oeffentlich"),
        m("/p/u", Some("k"), "kundendaten"),
    ];
    assert_eq!(
        entscheiden(marken, a("/p", Some("k"), "intern"), &klassen(), &mut zz()),
        Err(Fehler::OrdnerHoeherMarkiert)
    );
}

#[test]
fn another_customer_is_refused_before_the_class_and_null_is_a_value() {
    let marken = vec![m("/p", Some("k1"), "kundendaten")];
    assert_eq!(
        entscheiden(
            marken.clone(),
            a("/p", Some("k2"), "kundendaten"),
            &klassen(),
            &mut zz()
        ),
        Err(Fehler::OrdnerAndererKunde)
    );
    // customer before class: a lower class with another customer names the customer
    assert_eq!(
        entscheiden(
            marken.clone(),
            a("/p/x", Some("k2"), "intern"),
            &klassen(),
            &mut zz()
        ),
        Err(Fehler::OrdnerAndererKunde)
    );
    assert_eq!(
        entscheiden(marken, a("/p", None, "kundendaten"), &klassen(), &mut zz()),
        Err(Fehler::OrdnerAndererKunde)
    );
    assert_eq!(
        entscheiden(
            vec![m("/p", None, "intern")],
            a("/p", Some("k"), "intern"),
            &klassen(),
            &mut zz()
        ),
        Err(Fehler::OrdnerAndererKunde)
    );
    assert_eq!(
        entscheiden(
            vec![m("/p", None, "intern")],
            a("/p", None, "intern"),
            &klassen(),
            &mut zz()
        ),
        Ok(None)
    );
}

#[test]
fn an_unknown_class_of_the_route_or_a_full_list_is_broken() {
    assert_eq!(
        entscheiden(Vec::new(), a("/p", None, "geheim"), &klassen(), &mut zz()),
        Err(Fehler::OrdnerMarkeKaputt)
    );
    let voll: Vec<Marke> = (0..ANZAHL_MAX)
        .map(|i| m(&format!("/x/{i}"), None, "intern"))
        .collect();
    assert_eq!(
        entscheiden(
            voll.clone(),
            a("/neu", None, "intern"),
            &klassen(),
            &mut zz()
        ),
        Err(Fehler::OrdnerMarkeKaputt)
    );
    let mut fast = voll;
    fast.pop();
    assert!(matches!(
        entscheiden(fast, a("/neu", None, "intern"), &klassen(), &mut zz()),
        Ok(Some(l)) if l.len() == ANZAHL_MAX
    ));
}

#[test]
fn the_file_form_is_closed_and_round_trips() {
    let marken = vec![
        m("/p", Some("k"), "intern"),
        m("/q \"x\"", None, "kundendaten"),
    ];
    let text = schreiben_marken(&marken);
    assert_eq!(lesen_marken(text.as_bytes(), &klassen()), Ok(marken));
    assert_eq!(
        lesen_marken(b"{\"fassung\":1,\"marken\":[]}", &klassen()),
        Ok(Vec::new())
    );
    for schlecht in [
        "",
        "[]",
        "{\"fassung\":1}",
        "{\"marken\":[]}",
        "{\"fassung\":2,\"marken\":[]}",
        "{\"fassung\":1.0,\"marken\":[]}",
        "{\"fassung\":1,\"marken\":[],\"x\":1}",
        "{\"fassung\":1,\"marken\":{}}",
        "{\"fassung\":1,\"marken\":[1]}",
        "{\"fassung\":1,\"marken\":[{\"ordner\":\"/p\",\"kunde\":null}]}",
        "{\"fassung\":1,\"marken\":[{\"ordner\":\"/p\",\"kunde\":null,\"klasse_hoch\":\"intern\",\"x\":1}]}",
        "{\"fassung\":1,\"marken\":[{\"ordner\":\"/p\",\"kunde\":null,\"klasse_hoch\":\"geheim\"}]}",
        "{\"fassung\":1,\"marken\":[{\"ordner\":\"p\",\"kunde\":null,\"klasse_hoch\":\"intern\"}]}",
        "{\"fassung\":1,\"marken\":[{\"ordner\":\"/p\",\"kunde\":\"K\",\"klasse_hoch\":\"intern\"}]}",
        "{\"fassung\":1,\"marken\":[{\"ordner\":\"/p\",\"kunde\":1,\"klasse_hoch\":\"intern\"}]}",
        "{\"fassung\":1,\"marken\":[{\"ordner\":1,\"kunde\":null,\"klasse_hoch\":\"intern\"}]}",
        "{\"fassung\":1,\"marken\":[{\"ordner\":\"/p\",\"kunde\":null,\"klasse_hoch\":1}]}",
        "{\"fassung\":1,\"marken\":[{\"ordner\":\"/p\",\"kunde\":null,\"klasse_hoch\":\"intern\"},{\"ordner\":\"/p\",\"kunde\":null,\"klasse_hoch\":\"intern\"}]}",
        "{\"fassung\":1,\"fassung\":1,\"marken\":[]}",
    ] {
        assert_eq!(
            lesen_marken(schlecht.as_bytes(), &klassen()),
            Err(Fehler::OrdnerMarkeKaputt),
            "{schlecht}"
        );
    }
    let gross = format!("{{\"fassung\":1,\"marken\":[]}}{}", " ".repeat(1024 * 1024));
    assert_eq!(
        lesen_marken(gross.as_bytes(), &klassen()),
        Err(Fehler::OrdnerMarkeKaputt)
    );
    let viele: Vec<Marke> = (0..=ANZAHL_MAX)
        .map(|i| m(&format!("/x/{i}"), None, "intern"))
        .collect();
    assert_eq!(
        lesen_marken(schreiben_marken(&viele).as_bytes(), &klassen()),
        Err(Fehler::OrdnerMarkeKaputt)
    );
}

#[test]
fn a_missing_wrong_or_double_key_of_any_mark_is_its_own_code_after_the_form() {
    let eintrag = |ordner: &str, kennung: &str| {
        format!("{{\"ordner\":\"{ordner}\",{kennung}\"kunde\":null,\"klasse_hoch\":\"intern\"}}")
    };
    let datei = |marken: &[String]| format!("{{\"fassung\":1,\"marken\":[{}]}}", marken.join(","));
    let gut = format!("\"kennung\":\"{}\",", "a".repeat(32));
    let kennung = Err(Fehler::OrdnerMarkeKennung);
    for falsch in [
        String::new(),
        format!("\"kennung\":\"{}\",", "a".repeat(31)),
        format!("\"kennung\":\"{}\",", "a".repeat(33)),
        format!("\"kennung\":\"{}\",", "A".repeat(32)),
        format!("\"kennung\":\"{}\",", "g".repeat(32)),
        "\"kennung\":null,".to_owned(),
        "\"kennung\":1,".to_owned(),
    ] {
        // also when the mark with the wrong key is not related to any folder of a start
        let t = datei(&[eintrag("/p", &gut), eintrag("/fern", &falsch)]);
        assert_eq!(lesen_marken(t.as_bytes(), &klassen()), kennung, "{falsch}");
    }
    // two entries with the same key
    let t = datei(&[eintrag("/p", &gut), eintrag("/q", &gut)]);
    assert_eq!(lesen_marken(t.as_bytes(), &klassen()), kennung);
    // a broken file names the form first, even with a wrong key
    let t = datei(&[
        eintrag("/p", ""),
        eintrag("/q", &gut).replace("intern", "geheim"),
    ]);
    assert_eq!(
        lesen_marken(t.as_bytes(), &klassen()),
        Err(Fehler::OrdnerMarkeKaputt)
    );
    // the right key passes, and the file keeps it
    let t = datei(&[eintrag("/p", &gut)]);
    let gelesen = lesen_marken(t.as_bytes(), &klassen()).expect("valid");
    assert_eq!(gelesen[0].kennung, "a".repeat(32));
    assert!(schreiben_marken(&gelesen).contains(&format!("\"kennung\":\"{}\"", "a".repeat(32))));
}

#[test]
fn a_new_key_is_drawn_again_while_another_mark_carries_it_and_a_rising_mark_keeps_its_key() {
    let mut alt = m("/a", None, "intern");
    alt.kennung = format!("{:032x}", 1);
    let neu = entscheiden(
        vec![alt.clone()],
        a("/b", None, "intern"),
        &klassen(),
        &mut zz(),
    )
    .expect("ok")
    .expect("changed");
    assert_eq!(neu[1].kennung, format!("{:032x}", 2), "drawn again");
    // a value of the wrong form from the source is not taken
    let mut quelle = vec![Some("x".to_owned()), Some(format!("{:032x}", 9))].into_iter();
    let neu = entscheiden(Vec::new(), a("/b", None, "intern"), &klassen(), &mut || {
        quelle.next().flatten()
    })
    .expect("ok")
    .expect("changed");
    assert_eq!(neu[0].kennung, format!("{:032x}", 9));
    // no key at all: an internal fault, never a mark without key
    assert_eq!(
        entscheiden(Vec::new(), a("/b", None, "intern"), &klassen(), &mut || {
            None
        }),
        Err(Fehler::Intern)
    );
    let steigt = entscheiden(
        vec![alt.clone()],
        a("/a", None, "kundendaten"),
        &klassen(),
        &mut zz(),
    )
    .expect("ok")
    .expect("changed");
    assert_eq!(
        steigt[0].kennung, alt.kennung,
        "the key stays when the class rises"
    );
    // the key of the system: 32 lower-case hex characters, two draws differ
    let (x, y) = (
        zufall_kennung().expect("random"),
        zufall_kennung().expect("random"),
    );
    assert!(start::ist_marken_kennung(&x) && x != y);
    assert_eq!(hex(&[0x00, 0x0f, 0xa5, 0xff]), "000fa5ff");
}

static ZAEHLER: AtomicU32 = AtomicU32::new(0);

struct Ordner(PathBuf);

impl Ordner {
    fn neu() -> Ordner {
        let n = ZAEHLER.fetch_add(1, Ordering::SeqCst);
        let basis = std::fs::canonicalize(std::env::temp_dir()).expect("tmp");
        let p = basis.join(format!("lauf-marke-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).expect("folder");
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o700)).expect("mode");
        Ordner(p)
    }
}

impl Drop for Ordner {
    fn drop(&mut self) {
        let _ = std::fs::set_permissions(&self.0, std::fs::Permissions::from_mode(0o700));
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn markieren(o: &Ordner, ordner: &str, kunde: Option<&str>, klasse: &str) -> Result<(), Fehler> {
    pruefen_und_markieren(
        &o.0,
        a(ordner, kunde, klasse),
        &klassen(),
        Duration::from_millis(300),
    )
}

fn inhalt(o: &Ordner) -> Vec<Marke> {
    let t = std::fs::read(o.0.join(MARKEN_DATEI)).expect("file");
    lesen_marken(&t, &klassen()).expect("valid")
}

#[test]
fn the_file_is_made_with_mode_0600_and_the_mark_rises_through_it() {
    let o = Ordner::neu();
    assert_eq!(markieren(&o, "/p", Some("k"), "intern"), Ok(()));
    let modus = std::fs::metadata(o.0.join(MARKEN_DATEI))
        .expect("meta")
        .permissions()
        .mode();
    assert_eq!(modus & 0o7777, 0o600);
    let erste = inhalt(&o);
    assert!(start::ist_marken_kennung(&erste[0].kennung));
    // the mark rises and keeps its key
    let mut soll = m("/p", Some("k"), "kundendaten");
    soll.kennung.clone_from(&erste[0].kennung);
    assert_eq!(markieren(&o, "/p", Some("k"), "kundendaten"), Ok(()));
    assert_eq!(inhalt(&o), vec![soll.clone()]);
    assert_eq!(
        markieren(&o, "/p", Some("k"), "intern"),
        Err(Fehler::OrdnerHoeherMarkiert)
    );
    assert_eq!(inhalt(&o), vec![soll]);
    // no temporary file is left behind
    let namen: Vec<String> = std::fs::read_dir(&o.0)
        .expect("list")
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    let mut namen = namen;
    namen.sort();
    assert_eq!(namen, [MARKEN_DATEI, SPERR_DATEI]);
}

#[test]
fn every_fault_of_the_file_or_the_folder_is_fail_closed() {
    let kaputt = Err(Fehler::OrdnerMarkeKaputt);
    // a broken file
    let o = Ordner::neu();
    std::fs::write(o.0.join(MARKEN_DATEI), "{\"fassung\":1,\"marken\":[").expect("write");
    std::fs::set_permissions(
        o.0.join(MARKEN_DATEI),
        std::fs::Permissions::from_mode(0o600),
    )
    .expect("mode");
    assert_eq!(
        markieren(&o, "/anders", None, "intern"),
        kaputt,
        "broken file blocks every folder"
    );
    // a file with another mode
    let o = Ordner::neu();
    markieren(&o, "/p", None, "intern").expect("first");
    std::fs::set_permissions(
        o.0.join(MARKEN_DATEI),
        std::fs::Permissions::from_mode(0o644),
    )
    .expect("mode");
    assert_eq!(markieren(&o, "/p", None, "intern"), kaputt, "mode 0644");
    // an unreadable file
    std::fs::set_permissions(
        o.0.join(MARKEN_DATEI),
        std::fs::Permissions::from_mode(0o000),
    )
    .expect("mode");
    if rustix::process::geteuid().as_raw() != 0 {
        assert_eq!(markieren(&o, "/p", None, "intern"), kaputt, "unreadable");
    }
    // the file is a link
    let o = Ordner::neu();
    let echt = o.0.join("echt.json");
    std::fs::write(&echt, "{\"fassung\":1,\"marken\":[]}").expect("write");
    std::fs::set_permissions(&echt, std::fs::Permissions::from_mode(0o600)).expect("mode");
    std::os::unix::fs::symlink(&echt, o.0.join(MARKEN_DATEI)).expect("link");
    assert_eq!(markieren(&o, "/p", None, "intern"), kaputt, "link");
    // the folder has another mode, is a link, is missing
    let o = Ordner::neu();
    std::fs::set_permissions(&o.0, std::fs::Permissions::from_mode(0o755)).expect("mode");
    assert_eq!(markieren(&o, "/p", None, "intern"), kaputt, "folder 0755");
    let o = Ordner::neu();
    let verweis = o.0.with_extension("verweis");
    let _ = std::fs::remove_file(&verweis);
    std::os::unix::fs::symlink(&o.0, &verweis).expect("link");
    assert_eq!(
        pruefen_und_markieren(
            &verweis,
            a("/p", None, "intern"),
            &klassen(),
            Duration::from_millis(100)
        ),
        kaputt,
        "folder link"
    );
    let _ = std::fs::remove_file(&verweis);
    assert_eq!(
        pruefen_und_markieren(
            &o.0.join("fehlt"),
            a("/p", None, "intern"),
            &klassen(),
            Duration::from_millis(100)
        ),
        kaputt,
        "missing folder"
    );
    // a class the settings do not know
    let o = Ordner::neu();
    std::fs::write(
        o.0.join(MARKEN_DATEI),
        "{\"fassung\":1,\"marken\":[{\"ordner\":\"/p\",\"kunde\":null,\"klasse_hoch\":\"geheim\"}]}",
    )
    .expect("write");
    std::fs::set_permissions(
        o.0.join(MARKEN_DATEI),
        std::fs::Permissions::from_mode(0o600),
    )
    .expect("mode");
    assert_eq!(markieren(&o, "/q", None, "intern"), kaputt, "unknown class");
}

#[test]
fn a_held_lock_ends_the_start_after_the_time_limit() {
    let o = Ordner::neu();
    let ordner = ordner_oeffnen(&o.0).expect("folder");
    let gehalten = sperren(&ordner, Duration::from_millis(10)).expect("lock");
    let beginn = Instant::now();
    assert_eq!(
        markieren(&o, "/p", None, "intern"),
        Err(Fehler::OrdnerMarkeKaputt)
    );
    assert!(
        beginn.elapsed() >= Duration::from_millis(250),
        "waited for the lock"
    );
    drop(gehalten);
    assert_eq!(markieren(&o, "/p", None, "intern"), Ok(()));
}

#[test]
fn starts_at_the_same_time_lose_no_mark() {
    let o = Ordner::neu();
    let pfad = o.0.clone();
    let faeden: Vec<_> = (0..16)
        .map(|i| {
            let p = pfad.clone();
            std::thread::spawn(move || {
                let ordner = format!("/arbeit/{i}");
                pruefen_und_markieren(
                    &p,
                    a(&ordner, None, "intern"),
                    &klassen(),
                    Duration::from_secs(10),
                )
            })
        })
        .collect();
    for f in faeden {
        assert_eq!(f.join().expect("thread"), Ok(()));
    }
    let mut ordner: Vec<String> = inhalt(&o).into_iter().map(|m| m.ordner).collect();
    ordner.sort();
    let mut soll: Vec<String> = (0..16).map(|i| format!("/arbeit/{i}")).collect();
    soll.sort();
    assert_eq!(ordner, soll);
}

/// Names of the new files left in the state folder.
fn neue_dateien(o: &Ordner) -> Vec<String> {
    std::fs::read_dir(&o.0)
        .expect("dir")
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| Path::new(n).extension().is_some_and(|e| e == "neu"))
        .collect()
}

/// A start whose new file is slow to reach the disk (it waits in `nach_datei` until `los` is sent) does not hold the
/// lock meanwhile: a second start at that moment gets the lock within its short time limit. Before, the `fsync` ran
/// under the lock, and a second start on a busy disk gave up with `lauf.ordner_marke_kaputt` (measured under load).
fn langsamer_start_und_zweiter(zweiter: &str) -> (Result<(), Fehler>, Result<(), Fehler>, Ordner) {
    let o = Ordner::neu();
    let (geschrieben_tx, geschrieben) = std::sync::mpsc::channel();
    let (los, los_rx) = std::sync::mpsc::channel::<()>();
    let (erster, zweiter) = std::thread::scope(|s| {
        let pfad = o.0.clone();
        let erster = s.spawn(move || {
            pruefen_und_markieren_mit(
                &pfad,
                a("/p", None, "intern"),
                &klassen(),
                Duration::from_millis(300),
                || {
                    geschrieben_tx.send(()).expect("send");
                    los_rx.recv().expect("go");
                },
            )
        });
        geschrieben.recv().expect("the first wrote its new file");
        let zweiter = markieren(&o, zweiter, None, "intern");
        los.send(()).expect("go");
        (erster.join().expect("first"), zweiter)
    });
    (erster, zweiter, o)
}

#[test]
fn a_slow_new_file_does_not_hold_the_lock_and_no_mark_is_lost() {
    let (erster, zweiter, o) = langsamer_start_und_zweiter("/q");
    assert_eq!((erster, zweiter), (Ok(()), Ok(())));
    let mut ordner: Vec<String> = inhalt(&o).into_iter().map(|m| m.ordner).collect();
    ordner.sort();
    assert_eq!(
        ordner,
        ["/p", "/q"],
        "the first decided again on the changed list"
    );
    assert_eq!(neue_dateien(&o), Vec::<String>::new());
}

#[test]
fn two_slow_starts_of_the_same_folder_make_one_mark() {
    let (erster, zweiter, o) = langsamer_start_und_zweiter("/p");
    assert_eq!((erster, zweiter), (Ok(()), Ok(())));
    let marken = inhalt(&o);
    assert_eq!(marken.len(), 1, "{marken:?}");
    assert_eq!(marken[0].ordner, "/p");
    assert_eq!(neue_dateien(&o), Vec::<String>::new());
}

/// The list unchanged between the two holds: the new file of the first round is renamed, nothing is decided again.
#[test]
fn an_unchanged_list_takes_the_new_file_of_the_first_round() {
    let o = Ordner::neu();
    let mut gesehen = None;
    pruefen_und_markieren_mit(
        &o.0,
        a("/p", Some("k"), "intern"),
        &klassen(),
        Duration::from_millis(300),
        || {
            let namen = neue_dateien(&o);
            assert_eq!(namen.len(), 1, "one new file before the rename: {namen:?}");
            gesehen = Some(std::fs::metadata(o.0.join(&namen[0])).expect("new").ino());
        },
    )
    .expect("mark");
    assert_eq!(
        Some(
            std::fs::metadata(o.0.join(MARKEN_DATEI))
                .expect("list")
                .ino()
        ),
        gesehen,
        "the very file of the first round became the list"
    );
    let marken = inhalt(&o);
    assert_eq!(
        (
            marken.len(),
            marken[0].ordner.as_str(),
            marken[0].kunde.as_deref()
        ),
        (1, "/p", Some("k"))
    );
    assert_eq!(neue_dateien(&o), Vec::<String>::new());
}

/// A taken name of a new file (the rest of a process ended hard with the same PID) is never opened, written or
/// removed: the next name is drawn, and when every drawn name is taken the write fails and leaves them all alone.
#[test]
fn a_taken_name_of_a_new_file_is_left_alone_and_never_written() {
    let o = Ordner::neu();
    let ordner = ordner_oeffnen(&o.0).expect("folder");
    // far more taken names than any write of the other tests of this process draws in the meantime
    let ab = NEU_ZAEHLER.load(Ordering::Relaxed);
    let fremd = b"rest of another process, longer than a list of one mark, never to be touched";
    let namen: Vec<PathBuf> = (ab..ab + 1000)
        .map(|n| {
            o.0.join(format!("{MARKEN_DATEI}.{}.{n}.neu", std::process::id()))
        })
        .collect();
    for n in &namen {
        std::fs::write(n, fremd).expect("taken");
    }
    assert_eq!(
        neue_datei(&ordner, &[m("/p", None, "intern")]),
        Err(Schreibfehler::Datei)
    );
    for n in &namen {
        assert_eq!(std::fs::read(n).expect("still there"), fremd);
    }
    assert!(!o.0.join(MARKEN_DATEI).exists(), "no list was made");
}

#[test]
fn a_link_in_place_of_the_lock_or_a_fifo_in_place_of_the_file_is_fail_closed() {
    let kaputt = Err(Fehler::OrdnerMarkeKaputt);
    // the lock is a link: it is not followed
    let o = Ordner::neu();
    let ziel = o.0.with_extension("fremd-lock");
    let _ = std::fs::remove_file(&ziel);
    std::os::unix::fs::symlink(&ziel, o.0.join(SPERR_DATEI)).expect("link");
    assert_eq!(markieren(&o, "/p", None, "intern"), kaputt, "lock link");
    assert!(!ziel.exists(), "the link target was not made");
    // the file is a FIFO: it is opened without blocking and refused
    let o = Ordner::neu();
    let status = std::process::Command::new("mkfifo")
        .arg(o.0.join(MARKEN_DATEI))
        .status()
        .expect("mkfifo");
    assert!(status.success());
    std::fs::set_permissions(
        o.0.join(MARKEN_DATEI),
        std::fs::Permissions::from_mode(0o600),
    )
    .expect("mode");
    let beginn = Instant::now();
    assert_eq!(markieren(&o, "/p", None, "intern"), kaputt, "fifo");
    assert!(
        beginn.elapsed() < Duration::from_secs(3),
        "no blocking open"
    );
}

#[test]
fn a_file_of_one_mebibyte_is_read_and_one_byte_more_is_broken() {
    let o = Ordner::neu();
    let kopf = format!(
        "{{\"fassung\":1,\"marken\":[{{\"ordner\":\"/q\",\"kennung\":\"{}\",\"kunde\":null,\"klasse_hoch\":\"intern\"}}]}}",
        "b".repeat(32)
    );
    let pfad = o.0.join(MARKEN_DATEI);
    let gross = |n: usize| {
        let mut t = kopf.clone().into_bytes();
        t.resize(n, b' ');
        std::fs::write(&pfad, &t).expect("write");
        std::fs::set_permissions(&pfad, std::fs::Permissions::from_mode(0o600)).expect("mode");
    };
    gross(1024 * 1024);
    assert_eq!(markieren(&o, "/p", None, "intern"), Ok(()), "exactly 1 MiB");
    gross(1024 * 1024 + 1);
    assert_eq!(
        markieren(&o, "/p2", None, "intern"),
        Err(Fehler::OrdnerMarkeKaputt),
        "1 MiB and one byte"
    );
    assert_eq!(
        lesen_marken(&vec![b' '; 1024 * 1024 + 1], &klassen()),
        Err(Fehler::OrdnerMarkeKaputt)
    );
}

/// The change of the command through [`Zustand::wechseln`]: the list becomes `neu` whatever it was, `zwischen` is
/// the step between.
fn ersetzen(
    z: &Zustand,
    neu: &[Marke],
    zwischen: impl FnOnce() -> Result<(), Fehler>,
) -> Result<(), Wechselfehler<()>> {
    z.wechseln(
        &klassen(),
        SPERR_FRIST,
        |_| Ok(Some((neu.to_vec(), ()))),
        |()| zwischen(),
        || {},
    )
}

/// Whether another holder could take the lock right now (it is free).
fn sperre_frei(o: &Ordner) -> bool {
    let fd = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(o.0.join(SPERR_DATEI))
        .expect("lock file");
    match rustix::fs::flock(&fd, FlockOperation::NonBlockingLockExclusive) {
        Ok(()) => true,
        Err(rustix::io::Errno::WOULDBLOCK) => false,
        Err(e) => panic!("flock: {e:?}"),
    }
}

#[test]
fn the_step_between_runs_after_the_new_file_and_before_the_rename_under_the_lock() {
    let o = Ordner::neu();
    let z = Zustand::ohne_sperre(&o.0).expect("open");
    let alt = vec![m("/a", None, "intern")];
    let neu = vec![m("/a", None, "intern"), m("/b", None, "intern")];
    ersetzen(&z, &alt, || Ok(())).expect("first");
    let alter_text = schreiben_marken(&alt);
    let neuer_text = schreiben_marken(&neu);
    let mut gesehen = Vec::new();
    let mut frei = None;
    ersetzen(&z, &neu, || {
        gesehen.push(std::fs::read_to_string(o.0.join(MARKEN_DATEI)).expect("old"));
        gesehen.push(
            std::fs::read_dir(&o.0)
                .expect("dir")
                .filter_map(Result::ok)
                .filter(|e| e.file_name().to_string_lossy().ends_with(".neu"))
                .map(|e| std::fs::read_to_string(e.path()).expect("new"))
                .collect::<String>(),
        );
        frei = Some(sperre_frei(&o));
        Ok(())
    })
    .expect("second");
    assert_eq!(gesehen, [alter_text.clone(), neuer_text.clone()]);
    assert_eq!(frei, Some(false), "the step between runs under the lock");
    assert!(sperre_frei(&o), "the lock is given back");
    assert_eq!(
        std::fs::read_to_string(o.0.join(MARKEN_DATEI)).expect("now"),
        neuer_text
    );
    // a failing step: the old list stands and the new file is gone
    assert_eq!(
        ersetzen(&z, &alt, || Err(Fehler::Intern)),
        Err(Wechselfehler::Schreiben(Schreibfehler::Zwischen))
    );
    assert_eq!(
        std::fs::read_to_string(o.0.join(MARKEN_DATEI)).expect("kept"),
        neuer_text
    );
    let mut namen: Vec<String> = std::fs::read_dir(&o.0)
        .expect("dir")
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    namen.sort();
    assert_eq!(namen, [MARKEN_DATEI, SPERR_DATEI]);
}

/// The command `loesen` of `/p` as [`Zustand::wechseln`] sees it: `Err(())` when there is no entry, the value of the
/// step between is the number of the entries before.
fn loesen_p(marken: Vec<Marke>) -> Result<Option<(Vec<Marke>, usize)>, ()> {
    let vorher = marken.len();
    let neu: Vec<Marke> = marken.into_iter().filter(|m| m.ordner != "/p").collect();
    if neu.len() == vorher {
        return Err(());
    }
    Ok(Some((neu, vorher)))
}

/// A command whose new file is slow to reach the disk (it waits in `nach_datei` until `los` is sent) while `zweiter`
/// runs. Returns the result of the command, the values its step between saw, and what `zweiter` returned.
fn langsamer_befehl_und<R: Send>(
    o: &Ordner,
    zweiter: impl FnOnce() -> R + Send,
) -> (Result<(), Wechselfehler<()>>, Vec<usize>, R) {
    let (geschrieben_tx, geschrieben) = std::sync::mpsc::channel();
    let (los, los_rx) = std::sync::mpsc::channel::<()>();
    std::thread::scope(|s| {
        let erster = s.spawn(move || {
            let mut zeilen = Vec::new();
            let z = Zustand::ohne_sperre(&o.0).expect("open");
            let r = z.wechseln(
                &klassen(),
                Duration::from_millis(300),
                loesen_p,
                |vorher| {
                    zeilen.push(vorher);
                    Ok(())
                },
                || {
                    geschrieben_tx.send(()).expect("send");
                    los_rx.recv().expect("go");
                },
            );
            (r, zeilen)
        });
        geschrieben.recv().expect("the command wrote its new file");
        let zweiter = zweiter();
        los.send(()).expect("go");
        let (r, zeilen) = erster.join().expect("command");
        (r, zeilen, zweiter)
    })
}

/// The command path had the `fsync` of the new file under the lock: a second start or command on a busy disk gave
/// up after the time limit (measured: `b: MarkeKaputt` under load). Now a start at that moment gets the lock within
/// its short limit, and the command decides again on the changed list: no mark and no change is lost, one line.
#[test]
fn a_slow_command_does_not_hold_the_lock_and_no_mark_is_lost() {
    let o = Ordner::neu();
    let z = Zustand::ohne_sperre(&o.0).expect("open");
    ersetzen(
        &z,
        &[m("/p", None, "intern"), m("/r", None, "intern")],
        || Ok(()),
    )
    .expect("list");
    let (befehl, zeilen, start) = langsamer_befehl_und(&o, || markieren(&o, "/q", None, "intern"));
    assert_eq!((befehl, start), (Ok(()), Ok(())));
    let mut ordner: Vec<String> = inhalt(&o).into_iter().map(|m| m.ordner).collect();
    ordner.sort();
    assert_eq!(
        ordner,
        ["/q", "/r"],
        "the start's mark stays, /p is released"
    );
    assert_eq!(
        zeilen,
        [3],
        "one line, about the list with the start's mark"
    );
    assert_eq!(neue_dateien(&o), Vec::<String>::new());
}

/// Two commands on the same entry: the one that was faster wins, the slow one finds no entry any more in its second
/// round and changes nothing and writes no line.
#[test]
fn a_slow_command_whose_entry_is_gone_in_between_changes_nothing_and_writes_no_line() {
    let o = Ordner::neu();
    let z = Zustand::ohne_sperre(&o.0).expect("open");
    ersetzen(
        &z,
        &[m("/p", None, "intern"), m("/r", None, "intern")],
        || Ok(()),
    )
    .expect("list");
    let (befehl, zeilen, zweiter) = langsamer_befehl_und(&o, || {
        let mut zeilen = Vec::new();
        let r = Zustand::ohne_sperre(&o.0).expect("open").wechseln(
            &klassen(),
            Duration::from_millis(300),
            loesen_p,
            |vorher| {
                zeilen.push(vorher);
                Ok(())
            },
            || {},
        );
        (r, zeilen)
    });
    assert_eq!(
        zweiter,
        (Ok(()), vec![2]),
        "the second command got the lock"
    );
    assert_eq!(
        (befehl, zeilen),
        (Err(Wechselfehler::Abgelehnt(())), vec![])
    );
    assert_eq!(
        inhalt(&o).into_iter().map(|m| m.ordner).collect::<Vec<_>>(),
        ["/r"]
    );
    assert_eq!(neue_dateien(&o), Vec::<String>::new());
}

/// The second round, all under the lock, also puts its step between under the lock and right before its rename, on
/// the list it decided on.
#[test]
fn the_second_round_runs_the_step_between_under_the_lock_on_the_new_list() {
    let o = Ordner::neu();
    let z = Zustand::ohne_sperre(&o.0).expect("open");
    ersetzen(&z, &[m("/p", None, "intern")], || Ok(())).expect("list");
    let (geschrieben_tx, geschrieben) = std::sync::mpsc::channel();
    let (los, los_rx) = std::sync::mpsc::channel::<()>();
    let o = &o;
    let (start, (r, gesehen)) = std::thread::scope(|s| {
        let erster = s.spawn(move || {
            let mut gesehen = Vec::new();
            let r = Zustand::ohne_sperre(&o.0).expect("open").wechseln(
                &klassen(),
                Duration::from_millis(300),
                loesen_p,
                |vorher| {
                    gesehen.push((
                        vorher,
                        sperre_frei(o),
                        inhalt(o).len(),
                        neue_dateien(o).len(),
                    ));
                    Ok(())
                },
                || {
                    geschrieben_tx.send(()).expect("send");
                    los_rx.recv().expect("go");
                },
            );
            (r, gesehen)
        });
        geschrieben.recv().expect("new file");
        let start = markieren(o, "/q", None, "intern");
        los.send(()).expect("go");
        (start, erster.join().expect("command"))
    });
    assert_eq!(start, Ok(()), "the start got the lock");
    assert_eq!(r, Ok(()));
    // two entries before, lock held, the list still the old one, exactly the new file of the second round beside it
    assert_eq!(gesehen, [(2, false, 2, 1)]);
    assert_eq!(
        inhalt(o).into_iter().map(|m| m.ordner).collect::<Vec<_>>(),
        ["/q"]
    );
    assert_eq!(neue_dateien(o), Vec::<String>::new());
}

/// Nothing to change: no new file, no step between, no lock left behind.
#[test]
fn no_change_writes_nothing_and_runs_no_step_between() {
    let o = Ordner::neu();
    let z = Zustand::ohne_sperre(&o.0).expect("open");
    let mut schritt = false;
    let r: Result<(), Wechselfehler<()>> = z.wechseln(
        &klassen(),
        SPERR_FRIST,
        |_| Ok(None::<(Vec<Marke>, ())>),
        |()| {
            schritt = true;
            Ok(())
        },
        || panic!("no new file"),
    );
    assert_eq!(r, Ok(()));
    assert!(!schritt);
    assert!(!o.0.join(MARKEN_DATEI).exists());
    assert!(sperre_frei(&o));
}

#[test]
fn reading_without_the_lock_makes_no_file() {
    let o = Ordner::neu();
    let z = Zustand::ohne_sperre(&o.0).expect("open");
    assert_eq!(z.lesen(&klassen()), Ok(Vec::new()));
    assert!(std::fs::read_dir(&o.0).expect("dir").next().is_none());
}

#[test]
fn only_a_lost_race_or_a_signal_is_tried_again_and_only_while_time_is_left() {
    use rustix::io::Errno;
    for e in [Errno::NOENT, Errno::INTR] {
        assert!(wiederholen(e, true), "{e:?} with time left");
        assert!(!wiederholen(e, false), "{e:?} without time");
    }
    for e in [Errno::ACCESS, Errno::LOOP, Errno::NOTDIR, Errno::WOULDBLOCK] {
        assert!(!wiederholen(e, true), "{e:?} is a fault, not a race");
    }
}

#[test]
fn a_lost_race_at_the_lock_file_is_tried_again_until_the_time_limit_and_then_fails_closed() {
    // An open folder whose entry was removed answers `ENOENT` to the make of the lock file, as macOS does for a lost
    // race: the open is repeated for the whole time limit, then the start fails closed.
    let o = Ordner::neu();
    let ordner = ordner_oeffnen(&o.0).expect("folder");
    std::fs::remove_dir_all(&o.0).expect("remove");
    let beginn = Instant::now();
    assert_eq!(
        sperren(&ordner, Duration::from_millis(150)).map(drop),
        Err(Fehler::OrdnerMarkeKaputt)
    );
    assert!(
        beginn.elapsed() >= Duration::from_millis(150),
        "tried again until the limit"
    );
}

#[test]
fn a_file_with_the_most_marks_is_read() {
    let alle: Vec<Marke> = (0..ANZAHL_MAX)
        .map(|i| m(&format!("/x/{i}"), None, "intern"))
        .collect();
    assert_eq!(
        lesen_marken(schreiben_marken(&alle).as_bytes(), &klassen()).map(|l| l.len()),
        Ok(ANZAHL_MAX)
    );
}

#[test]
fn only_a_lost_race_or_a_signal_is_tried_again_and_only_before_the_limit() {
    use rustix::io::Errno;
    assert!(wiederholen(Errno::NOENT, true));
    assert!(wiederholen(Errno::INTR, true));
    assert!(!wiederholen(Errno::NOENT, false));
    assert!(!wiederholen(Errno::INTR, false));
    assert!(!wiederholen(Errno::ACCESS, true));
    assert!(!wiederholen(Errno::LOOP, true));
}

#[test]
fn a_lock_file_that_cannot_be_made_ends_at_once_and_a_held_lock_at_the_limit() {
    let o = Ordner::neu();
    let ordner = ordner_oeffnen(&o.0).expect("folder");
    let gehalten = sperren(&ordner, Duration::from_millis(10)).expect("lock");
    // the lock waits at most its limit; a wait past it fails the test within the limit of the thread
    let beginn = Instant::now();
    let zweites = ordner_oeffnen(&o.0).expect("folder");
    let r = crate::testhilfe::mit_frist(Duration::from_secs(10), move || {
        sperren(&zweites, Duration::from_millis(200)).map(drop)
    });
    assert_eq!(r, Err(Fehler::OrdnerMarkeKaputt));
    assert!(beginn.elapsed() >= Duration::from_millis(200));
    drop(gehalten);
    // a folder where the lock file cannot be made (no write right): no retry, the fault at once
    if !rustix::process::geteuid().is_root() {
        let o = Ordner::neu();
        std::fs::set_permissions(&o.0, std::fs::Permissions::from_mode(0o500)).expect("mode");
        let nur_lesen =
            rustix::fs::open(&o.0, OFlags::RDONLY.union(OFlags::DIRECTORY), Mode::empty())
                .expect("folder");
        let r = crate::testhilfe::mit_frist(Duration::from_secs(10), move || {
            sperren(&nur_lesen, Duration::from_secs(60)).map(drop)
        });
        assert_eq!(r, Err(Fehler::OrdnerMarkeKaputt));
    }
}
