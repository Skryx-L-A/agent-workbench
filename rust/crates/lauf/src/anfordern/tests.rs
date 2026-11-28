use super::zeile_lesen;
use std::io::Read;
use std::os::unix::net::UnixStream;

use super::{
    ANFORDERUNG_MAX, ANTWORT_MAX, Annahme, LAEUFT, OFFEN, Offen, anforderung, anforderung_gut,
    annahme, antwort_geben, antwort_gut, erste_zeile, kennung_gut, modell_gut,
};

#[test]
fn an_identifier_has_its_form_and_nothing_else() {
    for gut in ["a", "0", "worker", "w-1.b_c", &"a".repeat(64)] {
        assert!(kennung_gut(gut.as_bytes()), "{gut}");
    }
    for schlecht in [
        "",
        "-a",
        ".a",
        "_a",
        "A",
        "a b",
        "ä",
        "a/b",
        "a:b",
        &"a".repeat(65),
    ] {
        assert!(!kennung_gut(schlecht.as_bytes()), "{schlecht}");
    }
}

#[test]
fn a_model_has_the_form_of_contract_1() {
    for gut in [
        "qwen3:8b",
        "gpt-6-luna",
        "Org/Modell@v1+x_y.z",
        "A",
        &"m".repeat(128),
    ] {
        assert!(modell_gut(gut.as_bytes()), "{gut}");
    }
    for schlecht in ["", "a b", "a\n", "a;b", "a$b", "ä", &"m".repeat(129)] {
        assert!(!modell_gut(schlecht.as_bytes()), "{schlecht}");
    }
}

#[test]
fn a_request_is_exactly_one_line_of_five_words() {
    let gut = b"anfordern worker pi qwen3:8b medium\n";
    assert!(anforderung_gut(gut));
    for schlecht in [
        &b"anfordern worker pi qwen3:8b medium"[..],
        b"anfordern worker pi qwen3:8b medium\n\n",
        b"anfordern worker pi qwen3:8b medium\nanfordern worker pi qwen3:8b medium\n",
        b"anfordern  worker pi qwen3:8b medium\n",
        b"anfordern worker pi qwen3:8b medium \n",
        b" anfordern worker pi qwen3:8b medium\n",
        b"anfordern worker pi qwen3:8b\n",
        b"anfordern worker pi qwen3:8b medium extra\n",
        b"Anfordern worker pi qwen3:8b medium\n",
        b"starten worker pi qwen3:8b medium\n",
        b"anfordern Worker pi qwen3:8b medium\n",
        b"anfordern worker p\xc3\xa4 qwen3:8b medium\n",
        b"anfordern worker pi qwen3;8b medium\n",
        b"anfordern worker pi qwen3:8b MEDIUM\n",
        b"anfordern worker pi qwen3:8b medium\r\n",
        b"anfordern worker pi qwen3:8b medium\0\n",
        b"",
        b"\n",
    ] {
        assert!(
            !anforderung_gut(schlecht),
            "{}",
            String::from_utf8_lossy(schlecht)
        );
    }
}

/// The longest request: four fields at their limits (64, 64, 128, 64) with five words and the line end give 334 bytes,
/// above the limit of 256; the limit decides then, not the fields.
#[test]
fn a_request_longer_than_256_bytes_is_refused_although_each_field_is_good() {
    let feld = "a".repeat(64);
    let lang = anforderung(&feld, &feld, &"m".repeat(110), "e");
    // 10 + 65 + 65 + 111 + 2 = 253 bytes: allowed
    assert_eq!(lang.as_ref().map(String::len), Some(253));
    let grenze = format!("anfordern {feld} {feld} {} eeee\n", "m".repeat(110));
    assert_eq!(grenze.len(), ANFORDERUNG_MAX);
    assert!(anforderung_gut(grenze.as_bytes()));
    let drueber = format!("anfordern {feld} {feld} {} eeeee\n", "m".repeat(110));
    assert_eq!(drueber.len(), ANFORDERUNG_MAX + 1);
    assert!(!anforderung_gut(drueber.as_bytes()));
    assert_eq!(anforderung(&feld, &feld, &"m".repeat(128), &feld), None);
}

#[test]
fn the_request_of_the_tool_is_built_from_the_four_fields() {
    assert_eq!(
        anforderung("worker", "pi", "qwen3:8b", "medium").as_deref(),
        Some("anfordern worker pi qwen3:8b medium\n")
    );
    assert_eq!(anforderung("worker", "pi", "qwen3:8b", "a b"), None);
    assert_eq!(anforderung("", "pi", "qwen3:8b", "medium"), None);
}

#[test]
fn an_answer_is_frei_with_a_session_or_abgelehnt_with_a_code() {
    for gut in [
        &b"frei s-9\n"[..],
        b"abgelehnt anfordern_offen\n",
        b"abgelehnt stelle.nein\n",
        OFFEN,
    ] {
        assert!(antwort_gut(gut), "{}", String::from_utf8_lossy(gut));
    }
    for schlecht in [
        &b"frei s-9"[..],
        b"frei\n",
        b"frei \n",
        b"frei S-9\n",
        b"frei s 9\n",
        b"abgelehnt\n",
        b"abgelehnt a-b\n",
        b"abgelehnt A\n",
        b"laeuft\n",
        b"ok s-9\n",
        b"frei s-9\nfrei s-9\n",
        b"\xff\xfe\n",
    ] {
        assert!(
            !antwort_gut(schlecht),
            "{}",
            String::from_utf8_lossy(schlecht)
        );
    }
    let lang = format!("abgelehnt {}\n", "a".repeat(118));
    assert_eq!(lang.len(), ANTWORT_MAX + 1);
    assert!(!antwort_gut(lang.as_bytes()));
}

#[test]
fn one_request_is_open_at_a_time_and_a_wrong_form_is_dropped() {
    let gut = b"anfordern worker pi qwen3:8b medium\n";
    assert_eq!(annahme(Some(gut), false), Annahme::Weiter);
    assert_eq!(annahme(Some(gut), true), Annahme::Offen);
    assert_eq!(annahme(Some(b"anfordern x\n"), false), Annahme::Verworfen);
    assert_eq!(annahme(Some(b"anfordern x\n"), true), Annahme::Verworfen);
    assert_eq!(annahme(None, false), Annahme::Verworfen);
}

#[test]
fn the_first_line_is_the_code_line_or_laeuft_while_the_parent_lives() {
    assert_eq!(
        erste_zeile(b"69 lauf.harness_fehlt\n", true).as_deref(),
        Some(&b"69 lauf.harness_fehlt\n"[..])
    );
    assert_eq!(
        erste_zeile(b"69 lauf.harness_fehlt\n", false).as_deref(),
        Some(&b"69 lauf.harness_fehlt\n"[..])
    );
    assert_eq!(erste_zeile(b"", true).as_deref(), Some(LAEUFT));
    assert_eq!(erste_zeile(b"", false), None);
    assert_eq!(LAEUFT.len(), 7);
}

#[test]
fn an_answer_reaches_the_open_request_once_and_a_wrong_one_never() {
    let (agent, halter) = UnixStream::pair().expect("pair");
    let mut offen = Offen(Some(halter));
    antwort_geben(b"frei S-9\n", &mut offen);
    assert!(offen.0.is_some(), "a wrong answer leaves the request open");
    antwort_geben(b"frei s-9\n", &mut offen);
    assert!(offen.0.is_none(), "the answer closes the request");
    // without an open request nothing happens
    antwort_geben(b"frei s-10\n", &mut offen);
    let mut alles = Vec::new();
    let mut agent = agent;
    agent.read_to_end(&mut alles).expect("read");
    assert_eq!(alles, b"frei s-9\n");
}

/// The second phase in this process: a child `sleep` stands for "Lauf starten", a socket pair for the service, a
/// listener in a folder of the test for the socket of the enclosure.
#[cfg(target_os = "linux")]
mod vermittlung {
    use std::io::{Read, Write};
    use std::os::unix::net::{UnixListener, UnixStream};
    use std::time::Duration;

    use super::super::{OFFEN, vermitteln};

    struct Lage {
        eltern: std::process::Child,
        dienst: UnixStream,
        socket: std::path::PathBuf,
        faden: Option<std::thread::JoinHandle<()>>,
        ordner: std::path::PathBuf,
    }

    impl Lage {
        fn neu(name: &str) -> Lage {
            let ordner = std::env::temp_dir()
                .join(format!("lauf-vermittlung-{}-{name}", std::process::id()));
            let _ = std::fs::remove_dir_all(&ordner);
            std::fs::create_dir_all(&ordner).expect("folder");
            let socket = ordner.join("a.sock");
            let hoerer = UnixListener::bind(&socket).expect("bind");
            hoerer.set_nonblocking(true).expect("nonblocking");
            let eltern = std::process::Command::new("sleep")
                .arg("60")
                .spawn()
                .expect("sleep");
            let pid = rustix::process::Pid::from_raw(i32::try_from(eltern.id()).expect("pid"))
                .expect("pid");
            let pidfd = rustix::process::pidfd_open(pid, rustix::process::PidfdFlags::empty())
                .expect("pidfd");
            let (dienst, mut halter) = UnixStream::pair().expect("pair");
            let faden = std::thread::spawn(move || vermitteln(&pidfd, &mut halter, &hoerer));
            dienst
                .set_read_timeout(Some(Duration::from_secs(10)))
                .expect("timeout");
            Lage {
                eltern,
                dienst,
                socket,
                faden: Some(faden),
                ordner,
            }
        }

        fn agent(&self, zeile: &[u8]) -> UnixStream {
            let mut a = UnixStream::connect(&self.socket).expect("connect");
            a.set_read_timeout(Some(Duration::from_secs(10)))
                .expect("timeout");
            a.write_all(zeile).expect("write");
            a
        }

        fn dienst_liest(&mut self, n: usize) -> Vec<u8> {
            let mut d = vec![0u8; n];
            self.dienst.read_exact(&mut d).expect("the service reads");
            d
        }

        fn dienst_still(&mut self) -> bool {
            self.dienst
                .set_read_timeout(Some(Duration::from_millis(300)))
                .expect("timeout");
            let mut b = [0u8; 1];
            let still = self.dienst.read(&mut b).is_err();
            self.dienst
                .set_read_timeout(Some(Duration::from_secs(10)))
                .expect("timeout");
            still
        }

        /// Whether the mediation has returned within 10 s.
        fn endet(&mut self) -> bool {
            let ende = std::time::Instant::now() + Duration::from_secs(10);
            while std::time::Instant::now() < ende {
                if self
                    .faden
                    .as_ref()
                    .is_some_and(std::thread::JoinHandle::is_finished)
                {
                    let _ = self.faden.take().map(std::thread::JoinHandle::join);
                    return true;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            false
        }
    }

    impl Drop for Lage {
        fn drop(&mut self) {
            let _ = self.eltern.kill();
            let _ = self.eltern.wait();
            let _ = std::fs::remove_dir_all(&self.ordner);
        }
    }

    fn antwort(a: &mut UnixStream) -> Vec<u8> {
        let mut alles = Vec::new();
        let _ = a.read_to_end(&mut alles);
        alles
    }

    #[test]
    fn a_request_goes_to_the_service_and_its_answer_back() {
        let mut l = Lage::neu("weiter");
        let zeile = b"anfordern worker pi qwen3:8b medium\n";
        let mut a = l.agent(zeile);
        assert_eq!(l.dienst_liest(zeile.len()), zeile);
        // two answers in one write: the first goes to the open request, the second has none and is dropped
        l.dienst
            .write_all(b"frei s-9\nfrei s-10\n")
            .expect("answer");
        assert_eq!(antwort(&mut a), b"frei s-9\n");
        // a request after the answer is a new open one
        let mut b = l.agent(zeile);
        assert_eq!(l.dienst_liest(zeile.len()), zeile);
        // an answer split over two writes arrives whole
        l.dienst.write_all(b"abgelehnt st").expect("answer");
        std::thread::sleep(Duration::from_millis(100));
        l.dienst.write_all(b"elle.nein\n").expect("answer");
        assert_eq!(antwort(&mut b), b"abgelehnt stelle.nein\n");
    }

    #[test]
    fn a_wrong_line_and_a_second_request_never_reach_the_service() {
        let mut l = Lage::neu("falsch");
        let mut falsch = l.agent(b"anfordern worker pi\n");
        assert_eq!(antwort(&mut falsch), b"");
        let zeile = b"anfordern worker pi qwen3:8b medium\n";
        let mut erster = l.agent(zeile);
        assert_eq!(l.dienst_liest(zeile.len()), zeile);
        let mut zweiter = l.agent(zeile);
        assert_eq!(antwort(&mut zweiter), OFFEN);
        assert!(
            l.dienst_still(),
            "the second request stayed with the holding process"
        );
        l.dienst.write_all(b"frei s-3\n").expect("answer");
        assert_eq!(antwort(&mut erster), b"frei s-3\n");
    }

    #[test]
    fn the_mediation_ends_with_the_parent() {
        let mut l = Lage::neu("eltern");
        assert!(!l.endet(), "it waits while the parent lives");
        let _ = l.eltern.kill();
        let _ = l.eltern.wait();
        assert!(l.endet(), "it ends with the parent");
    }

    #[test]
    fn the_mediation_ends_when_the_service_closes_or_breaks_the_form() {
        let mut l = Lage::neu("dienst");
        let (ersatz, _) = UnixStream::pair().expect("pair");
        drop(std::mem::replace(&mut l.dienst, ersatz));
        assert!(l.endet(), "it ends with the connection");
        let mut l = Lage::neu("lang");
        l.dienst.write_all(&[b'x'; 200]).expect("write");
        assert!(
            l.endet(),
            "a line of the service longer than 128 bytes ends it"
        );
        let mut l = Lage::neu("grenze");
        l.dienst.write_all(&[b'x'; 128]).expect("write");
        assert!(l.endet(), "128 bytes without a line end end it");
        let mut l = Lage::neu("kurz");
        l.dienst.write_all(&[b'x'; 127]).expect("write");
        assert!(
            !l.endet(),
            "127 bytes without a line end are still a line in progress"
        );
    }
}

/// A source that is interrupted once before each byte, then gives the bytes; another error ends it.
struct Stockend(Vec<u8>, bool, Option<std::io::ErrorKind>);

impl Read for Stockend {
    fn read(&mut self, puffer: &mut [u8]) -> std::io::Result<usize> {
        self.1 = !self.1;
        if self.1 {
            return Err(std::io::ErrorKind::Interrupted.into());
        }
        if self.0.is_empty() {
            return match self.2 {
                Some(k) => Err(k.into()),
                None => Ok(0),
            };
        }
        puffer[0] = self.0.remove(0);
        Ok(1)
    }
}

#[test]
fn a_line_is_read_up_to_its_end_and_at_most_its_limit() {
    let mut q = Stockend(b"ab\ncd".to_vec(), false, None);
    assert_eq!(
        zeile_lesen(&mut q, 8).as_deref(),
        Some(&b"ab\n"[..]),
        "an interrupt repeats the read"
    );
    assert_eq!(q.0, b"cd", "nothing after the line end is taken");
    let mut grenze = vec![b'a'; 7];
    grenze.push(b'\n');
    assert_eq!(
        zeile_lesen(&mut &grenze[..], 8).as_deref(),
        Some(&grenze[..])
    );
    let mut drueber = vec![b'a'; 8];
    drueber.push(b'\n');
    assert_eq!(
        zeile_lesen(&mut &drueber[..], 8),
        None,
        "the limit counts the line end"
    );
    assert_eq!(
        zeile_lesen(&mut &b"ab"[..], 8),
        None,
        "an end before the line end"
    );
    let mut fehler = Stockend(b"ab".to_vec(), false, Some(std::io::ErrorKind::BrokenPipe));
    assert_eq!(
        zeile_lesen(&mut fehler, 8),
        None,
        "another error ends the read"
    );
}
