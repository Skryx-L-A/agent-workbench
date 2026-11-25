use std::os::fd::OwnedFd;

use super::{GRUND_MAX, MELDUNG_FD, Meldung, grund_gut, zeile, zeile_gut};
use crate::fehler::Fehler;

/// The keys a `code()` gives, read from the source: every quoted text after `=> "` that starts with `prefix`.
fn schluessel(quelle: &str, prefix: &str) -> Vec<String> {
    let mut alle = Vec::new();
    for teil in quelle.split("=> \"").skip(1) {
        let Some((wort, _)) = teil.split_once('"') else {
            continue;
        };
        if wort.starts_with(prefix) {
            alle.push(wort.to_owned());
        }
    }
    alle
}

/// Every key of `lauf/src/fehler.rs` and of the codes of contracts 1 and 2 it passes on (`start.*`, `einst.*`) has the
/// form of the report (agreed with the service), and so has every line the program can write.
#[test]
fn every_key_of_the_faults_has_the_form_of_the_report() {
    let quellen = [
        (include_str!("../fehler.rs"), "lauf."),
        (include_str!("../../../start/src/fehler.rs"), "start."),
        (
            include_str!("../../../einstellungen/src/fehler.rs"),
            "einst.",
        ),
    ];
    let mut zahl = 0;
    for (quelle, prefix) in quellen {
        let alle = schluessel(quelle, prefix);
        // positive control: the reading finds the keys at all
        assert!(alle.len() > 10, "{prefix}: {alle:?}");
        for s in &alle {
            assert!(grund_gut(s), "{s}");
            assert!(zeile_gut(format!("255 {s}\n").as_bytes()), "{s}");
        }
        zahl += alle.len();
    }
    assert!(zahl >= 75, "{zahl}");
    // the lines of the variants the program builds itself
    for f in [
        Fehler::MeldungFehlt,
        Fehler::HarnessFehlt,
        Fehler::Intern,
        Fehler::PaketCache("verweis"),
        Fehler::PaketEntpacken {
            rc: Some(1),
            wort: "kein-platz",
        },
        Fehler::Start(start::Fehler::ZuGross),
        Fehler::Einst(einstellungen::Fehler::ZuGross),
    ] {
        assert!(zeile_gut(zeile(&f).as_bytes()), "{}", zeile(&f));
    }
    assert_eq!(zeile(&Fehler::HarnessFehlt), "69 lauf.harness_fehlt\n");
    assert_eq!(zeile(&Fehler::MeldungFehlt), "69 lauf.meldung_fehlt\n");
}

#[test]
fn the_form_of_the_line_is_closed() {
    let lang = "a".repeat(GRUND_MAX);
    for gut in [
        "69 lauf.harness_fehlt\n".to_owned(),
        "1 a\n".to_owned(),
        "255 x_9.y\n".to_owned(),
        format!("70 {lang}\n"),
    ] {
        assert!(zeile_gut(gut.as_bytes()), "{gut}");
    }
    let zu_lang = format!("70 {lang}a\n");
    for schlecht in [
        "",
        "\n",
        "69 lauf.harness_fehlt",
        "69 lauf.harness_fehlt\n\n",
        "69 lauf.harness_fehlt\nx",
        "69  lauf.harness_fehlt\n",
        "69 lauf.harness_fehlt \n",
        "69 lauf.harness_fehlt: Harness nicht startbar\n",
        "69 Lauf.harness_fehlt\n",
        "69 lauf-harness\n",
        "0 lauf.intern\n",
        "256 lauf.intern\n",
        "069 lauf.intern\n",
        "+69 lauf.intern\n",
        "-1 lauf.intern\n",
        "69\n",
        "69 \n",
        " lauf.intern\n",
        "69 lauf.intern\r\n",
        zu_lang.as_str(),
    ] {
        assert!(!zeile_gut(schlecht.as_bytes()), "{schlecht:?}");
    }
    let mut gross = format!("70 {lang}\n").into_bytes();
    gross.resize(129, b'a');
    assert!(!zeile_gut(&gross));
    assert!(!zeile_gut(b"69 lauf.\xff\n"));
}

#[test]
fn the_longest_line_of_the_form_fits_the_limit_and_a_longer_text_never_passes() {
    let laengste = format!("255 {}\n", "a".repeat(GRUND_MAX));
    assert!(zeile_gut(laengste.as_bytes()));
    assert!(laengste.len() <= super::ZEILE_MAX, "{}", laengste.len());
    // a text of the limit and over it, made of a line and padding, is refused by its form
    for n in [super::ZEILE_MAX, super::ZEILE_MAX + 1] {
        let mut t = laengste.clone().into_bytes();
        t.resize(n, b'\n');
        assert!(!zeile_gut(&t), "{n}");
    }
}

/// A socket of the service in a folder of its own, and the reading end of its first connection.
#[cfg(unix)]
fn dienst() -> (
    crate::testhilfe::Ordner,
    std::path::PathBuf,
    std::os::unix::net::UnixListener,
) {
    let o = crate::testhilfe::Ordner::neu("meldung");
    let pfad = o.0.join("m.sock");
    let hoerer = std::os::unix::net::UnixListener::bind(&pfad).expect("socket");
    (o, pfad, hoerer)
}

#[cfg(unix)]
fn gelesen(hoerer: &std::os::unix::net::UnixListener) -> Vec<u8> {
    use std::io::Read;
    let mut v = crate::testhilfe::annehmen(hoerer, std::time::Duration::from_secs(10));
    // best effort: macOS refuses the option on a socket whose other end has closed already
    let _ = v.set_read_timeout(Some(std::time::Duration::from_secs(10)));
    let mut alles = Vec::new();
    v.read_to_end(&mut alles).expect("read to the end");
    alles
}

#[cfg(unix)]
#[test]
fn without_the_option_nothing_is_held_and_nothing_is_reported() {
    let mut m = Meldung::default();
    assert!(!m.aktiv());
    assert!(!m.vier_da());
    assert_eq!(m.eigene(), Vec::<i32>::new());
    assert_eq!(m.auf_vier_legen(), Ok(false));
    m.melden(&Fehler::Intern);
    assert!(!m.aktiv());
}

#[cfg(unix)]
#[test]
fn a_missing_socket_is_the_fault_of_the_report() {
    let o = crate::testhilfe::Ordner::neu("meldung-fehlt");
    assert_eq!(
        Meldung::verbinden(&o.0.join("fehlt.sock"), None).err(),
        Some(Fehler::MeldungFehlt)
    );
}

#[cfg(unix)]
#[test]
fn the_report_holds_its_connection_and_descriptor_and_writes_one_line() {
    use std::os::fd::AsRawFd;
    let (_o, pfad, hoerer) = dienst();
    let vier: OwnedFd = std::fs::File::open("/dev/null").expect("null").into();
    let vier_nr = vier.as_raw_fd();
    let mut m = Meldung::verbinden(&pfad, Some(vier)).expect("connect");
    assert!(m.aktiv());
    assert!(m.vier_da());
    let eigene = m.eigene();
    assert_eq!(eigene.len(), 2, "{eigene:?}");
    assert_eq!(eigene[1], vier_nr);
    assert_ne!(eigene[0], vier_nr);
    m.vier_schliessen();
    assert!(!m.vier_da());
    assert!(m.aktiv(), "the connection stays for a line");
    assert_eq!(m.eigene().len(), 1);
    m.melden(&Fehler::HarnessFehlt);
    assert!(!m.aktiv());
    assert_eq!(gelesen(&hoerer), zeile(&Fehler::HarnessFehlt).as_bytes());
}

#[cfg(unix)]
#[test]
fn closing_both_copies_ends_the_connection_without_a_line() {
    let (_o, pfad, hoerer) = dienst();
    let vier: OwnedFd = std::fs::File::open("/dev/null").expect("null").into();
    let mut m = Meldung::verbinden(&pfad, Some(vier)).expect("connect");
    m.schliessen();
    assert!(!m.aktiv());
    assert!(!m.vier_da());
    assert_eq!(m.eigene(), Vec::<i32>::new());
    m.melden(&Fehler::Intern);
    assert_eq!(gelesen(&hoerer), Vec::<u8>::new());
}

#[cfg(unix)]
#[test]
fn a_connection_without_descriptor_4_cannot_be_laid_over_it() {
    let (_o, pfad, _hoerer) = dienst();
    let mut ohne = Meldung::verbinden(&pfad, None).expect("connect");
    assert_eq!(ohne.auf_vier_legen(), Err(Fehler::MeldungFehlt));
    // a held descriptor that is not number 4 is no place either
    let anderer: OwnedFd = std::fs::File::open("/dev/null").expect("null").into();
    let mut falsch = Meldung::verbinden(&pfad, Some(anderer)).expect("connect");
    assert_eq!(falsch.auf_vier_legen(), Err(Fehler::MeldungFehlt));
}

/// The helper process of the cases below (see `crate::testhilfe`).
#[test]
fn kind_fall() {
    let Some(fall) = crate::testhilfe::fall() else {
        return;
    };
    match fall.as_str() {
        "vier-frei" => fall_vier_frei(),
        "vier-belegt" => fall_vier_belegt(),
        andere => panic!("unknown case {andere}"),
    }
    crate::testhilfe::fertig();
}

#[cfg(target_os = "linux")]
const KIND: &str = "meldung::tests::kind_fall";

fn fall_vier_frei() {
    use std::os::fd::AsRawFd;
    // 3 is taken by the shell, 4 is the lowest free number
    let vier = Meldung::reservieren().expect("descriptor 4");
    assert_eq!(vier.as_raw_fd(), MELDUNG_FD);
    let flags = rustix::io::fcntl_getfd(&vier).expect("flags");
    assert!(flags.contains(rustix::io::FdFlags::CLOEXEC));
    let (_o, pfad, hoerer) = dienst();
    let mut m = Meldung::verbinden(&pfad, Some(vier)).expect("connect");
    assert_eq!(m.auf_vier_legen(), Ok(true));
    // descriptor 4 is the connection now, without close-on-exec
    let flags = rustix::io::fcntl_getfd(m.vier.as_ref().expect("held")).expect("flags");
    assert!(!flags.contains(rustix::io::FdFlags::CLOEXEC));
    let st = rustix::fs::fstat(m.vier.as_ref().expect("held")).expect("fstat");
    assert_eq!(
        rustix::fs::FileType::from_raw_mode(st.st_mode),
        rustix::fs::FileType::Socket
    );
    m.melden(&Fehler::Intern);
    assert_eq!(gelesen(&hoerer), zeile(&Fehler::Intern).as_bytes());
}

fn fall_vier_belegt() {
    // the shell handed down 4: the reservation keeps nothing
    assert!(Meldung::reservieren().is_none());
}

#[cfg(target_os = "linux")]
#[test]
fn descriptor_4_is_reserved_only_when_it_is_the_lowest_free_number() {
    crate::testhilfe::helfer_gut(KIND, "vier-frei", "3</dev/null 4>&-");
    crate::testhilfe::helfer_gut(KIND, "vier-belegt", "3</dev/null 4</dev/null");
}
