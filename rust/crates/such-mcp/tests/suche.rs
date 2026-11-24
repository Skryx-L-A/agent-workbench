//! The connection to the socket: one request line, one answer line, the deadline of 10 s, the limits.

#![cfg(unix)]

mod gemeinsam;

use std::io::{Read, Write};
use std::time::{Duration, Instant};

use gemeinsam::{StandIn, Wegwerf};
use such_mcp::suche::{Befrager, FRIST, MAX_ANTWORT, Unerreichbar, UnixSocket};

fn antwortet(wegwerf: &Wegwerf, antwort: &'static [u8]) -> (std::path::PathBuf, StandIn) {
    let pfad = wegwerf.pfad("s.sock");
    let stand_in = StandIn::start(&pfad, move |_, mut strom| {
        let _ = strom.write_all(antwort);
    });
    (pfad, stand_in)
}

#[test]
fn the_constants_are_the_ones_of_the_contract() {
    assert_eq!(FRIST, Duration::from_secs(10));
    assert_eq!(MAX_ANTWORT, 8 * 1024 * 1024);
}

#[test]
fn the_request_arrives_as_one_line_and_the_answer_comes_back_without_lf() {
    let wegwerf = Wegwerf::neu("eins");
    let (pfad, stand_in) = antwortet(&wegwerf, b"{\"ok\":true}\n");
    let socket = UnixSocket::mit_grenzen(&pfad, Duration::from_secs(5), 1000);
    assert_eq!(
        socket.frage(r#"{"op":"search","query":"x"}"#),
        Ok(r#"{"ok":true}"#.to_owned())
    );
    assert_eq!(stand_in.empfangen(), [r#"{"op":"search","query":"x"}"#]);
    assert_eq!(socket.pfad(), pfad);
}

#[test]
fn every_question_opens_its_own_connection() {
    let wegwerf = Wegwerf::neu("zwei");
    let (pfad, stand_in) = antwortet(&wegwerf, b"a\n");
    let socket = UnixSocket::mit_grenzen(&pfad, Duration::from_secs(5), 1000);
    for frage in ["1", "2", "3"] {
        assert_eq!(socket.frage(frage), Ok("a".to_owned()));
    }
    assert_eq!(stand_in.empfangen().len(), 3);
}

#[test]
fn only_the_first_line_counts_and_a_carriage_return_stays() {
    let wegwerf = Wegwerf::neu("erste");
    let (pfad, _) = antwortet(&wegwerf, b"erste\r\nzweite\n");
    let socket = UnixSocket::mit_grenzen(&pfad, Duration::from_secs(5), 1000);
    assert_eq!(socket.frage("x"), Ok("erste\r".to_owned()));
}

#[test]
fn an_empty_line_is_an_empty_answer() {
    let wegwerf = Wegwerf::neu("leer");
    let (pfad, _) = antwortet(&wegwerf, b"\n");
    let socket = UnixSocket::mit_grenzen(&pfad, Duration::from_secs(5), 1000);
    assert_eq!(socket.frage("x"), Ok(String::new()));
}

#[test]
fn an_answer_in_pieces_is_put_together() {
    let wegwerf = Wegwerf::neu("stuecke");
    let pfad = wegwerf.pfad("s.sock");
    let _stand_in = StandIn::start(&pfad, |_, mut strom| {
        for stueck in [&b"{\"ab"[..], b"c\":", b"1}", b"\n"] {
            let _ = strom.write_all(stueck);
            let _ = strom.flush();
            std::thread::sleep(Duration::from_millis(30));
        }
    });
    let socket = UnixSocket::mit_grenzen(&pfad, Duration::from_secs(5), 1000);
    assert_eq!(socket.frage("x"), Ok("{\"abc\":1}".to_owned()));
}

#[test]
fn a_long_answer_over_several_reads_comes_back_whole() {
    let wegwerf = Wegwerf::neu("lang");
    let pfad = wegwerf.pfad("s.sock");
    let _stand_in = StandIn::start(&pfad, |_, mut strom| {
        let mut antwort = vec![b'a'; 100_000];
        antwort.push(b'\n');
        let _ = strom.write_all(&antwort);
    });
    let socket = UnixSocket::mit_grenzen(&pfad, Duration::from_secs(5), 200_000);
    assert_eq!(socket.frage("x"), Ok("a".repeat(100_000)));
}

#[test]
fn a_long_request_arrives_whole() {
    let wegwerf = Wegwerf::neu("anfrage");
    let (pfad, stand_in) = antwortet(&wegwerf, b"ok\n");
    let socket = UnixSocket::mit_grenzen(&pfad, Duration::from_secs(5), 1000);
    let frage = "q".repeat(300_000);
    assert_eq!(socket.frage(&frage), Ok("ok".to_owned()));
    assert_eq!(stand_in.empfangen(), [frage]);
}

#[test]
fn a_missing_socket_is_unreachable() {
    let wegwerf = Wegwerf::neu("fehlt");
    let socket =
        UnixSocket::mit_grenzen(wegwerf.pfad("nicht-da.sock"), Duration::from_secs(5), 1000);
    assert_eq!(socket.frage("x"), Err(Unerreichbar));
}

#[test]
fn a_plain_file_instead_of_a_socket_is_unreachable() {
    let wegwerf = Wegwerf::neu("datei");
    let pfad = wegwerf.pfad("datei");
    std::fs::write(&pfad, b"x").expect("write");
    let socket = UnixSocket::mit_grenzen(&pfad, Duration::from_secs(5), 1000);
    assert_eq!(socket.frage("x"), Err(Unerreichbar));
}

#[test]
fn a_closed_connection_without_a_line_is_unreachable() {
    for antwort in [&b""[..], b"ohne Zeilenende", b"{\"a\":1}"] {
        let wegwerf = Wegwerf::neu("zu");
        let pfad = wegwerf.pfad("s.sock");
        let antwort = antwort.to_vec();
        let _stand_in = StandIn::start(&pfad, move |_, mut strom| {
            let _ = strom.write_all(&antwort);
        });
        let socket = UnixSocket::mit_grenzen(&pfad, Duration::from_secs(5), 1000);
        let start = Instant::now();
        assert_eq!(socket.frage("x"), Err(Unerreichbar));
        assert!(
            start.elapsed() < Duration::from_secs(3),
            "it waited for the deadline"
        );
    }
}

#[test]
fn an_answer_that_is_not_utf8_is_unreachable() {
    let wegwerf = Wegwerf::neu("utf8");
    let (pfad, _) = antwortet(&wegwerf, b"\xff\xfe\n");
    let socket = UnixSocket::mit_grenzen(&pfad, Duration::from_secs(5), 1000);
    assert_eq!(socket.frage("x"), Err(Unerreichbar));
}

#[test]
fn an_answer_of_exactly_the_limit_is_accepted_and_one_byte_more_is_not() {
    let wegwerf = Wegwerf::neu("grenze");
    let pfad = wegwerf.pfad("s.sock");
    // The first request line asks for the length: "100" lines of 100 bytes, "101" of 101, in one write or in two.
    let _stand_in = StandIn::start(&pfad, |frage, mut strom| {
        let (laenge, geteilt) = frage
            .split_once(':')
            .map_or((frage.as_str(), "ein"), |(a, b)| (a, b));
        let laenge: usize = laenge.parse().expect("a length");
        let mut antwort = vec![b'a'; laenge];
        antwort.push(b'\n');
        if geteilt == "ein" {
            let _ = strom.write_all(&antwort);
        } else {
            let _ = strom.write_all(&antwort[..laenge]);
            let _ = strom.flush();
            std::thread::sleep(Duration::from_millis(50));
            let _ = strom.write_all(&antwort[laenge..]);
        }
    });
    let socket = UnixSocket::mit_grenzen(&pfad, Duration::from_secs(5), 100);
    for art in ["ein", "geteilt"] {
        assert_eq!(
            socket.frage(&format!("100:{art}")),
            Ok("a".repeat(100)),
            "{art}"
        );
        assert_eq!(
            socket.frage(&format!("101:{art}")),
            Err(Unerreichbar),
            "{art}"
        );
        assert_eq!(
            socket.frage(&format!("0:{art}")),
            Ok(String::new()),
            "{art}"
        );
    }
}

#[test]
fn an_endless_answer_without_lf_ends_at_the_limit_not_at_the_deadline() {
    let wegwerf = Wegwerf::neu("endlos");
    let pfad = wegwerf.pfad("s.sock");
    let _stand_in = StandIn::start(&pfad, |_, mut strom| {
        let block = [b'a'; 4096];
        while strom.write_all(&block).is_ok() {}
    });
    let socket = UnixSocket::mit_grenzen(&pfad, Duration::from_secs(8), 10_000);
    let start = Instant::now();
    assert_eq!(socket.frage("x"), Err(Unerreichbar));
    assert!(
        start.elapsed() < Duration::from_secs(4),
        "{:?}",
        start.elapsed()
    );
}

#[test]
fn a_silent_socket_ends_the_call_at_the_deadline() {
    let wegwerf = Wegwerf::neu("still");
    let pfad = wegwerf.pfad("s.sock");
    let _stand_in = StandIn::start(&pfad, |_, mut strom| {
        let mut puffer = [0u8; 1];
        let _ = strom.read(&mut puffer); // holds the connection open until the caller goes
    });
    let socket = UnixSocket::mit_grenzen(&pfad, Duration::from_millis(400), 1000);
    let start = Instant::now();
    assert_eq!(socket.frage("x"), Err(Unerreichbar));
    let dauer = start.elapsed();
    assert!(dauer >= Duration::from_millis(350), "too early: {dauer:?}");
    assert!(dauer < Duration::from_secs(3), "too late: {dauer:?}");
}

#[test]
fn the_exchange_closes_its_connection_at_the_deadline() {
    let wegwerf = Wegwerf::neu("schliesst");
    let pfad = wegwerf.pfad("s.sock");
    let geschrieben = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let zaehler = std::sync::Arc::clone(&geschrieben);
    let _stand_in = StandIn::start(&pfad, move |_, mut strom| {
        for _ in 0..40 {
            if strom.write_all(b"a").is_err() {
                return;
            }
            zaehler.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            std::thread::sleep(Duration::from_millis(100));
        }
    });
    let socket = UnixSocket::mit_grenzen(&pfad, Duration::from_millis(500), 1000);
    assert_eq!(socket.frage("x"), Err(Unerreichbar));
    // Without a closed connection the stand-in would go on writing about ten more bytes in the next second.
    std::thread::sleep(Duration::from_millis(1000));
    let n = geschrieben.load(std::sync::atomic::Ordering::SeqCst);
    assert!(
        n <= 8,
        "the stand-in wrote {n} bytes: the connection stayed open"
    );
}

#[test]
fn a_slow_answer_does_not_extend_the_deadline() {
    let wegwerf = Wegwerf::neu("tropf");
    let pfad = wegwerf.pfad("s.sock");
    let _stand_in = StandIn::start(&pfad, |_, mut strom| {
        for _ in 0..40 {
            if strom.write_all(b"a").is_err() {
                return;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    });
    let socket = UnixSocket::mit_grenzen(&pfad, Duration::from_millis(600), 1000);
    let start = Instant::now();
    assert_eq!(socket.frage("x"), Err(Unerreichbar));
    assert!(
        start.elapsed() < Duration::from_secs(2),
        "{:?}",
        start.elapsed()
    );
}

#[test]
fn a_late_answer_within_the_deadline_is_accepted() {
    let wegwerf = Wegwerf::neu("spaet");
    let pfad = wegwerf.pfad("s.sock");
    let _stand_in = StandIn::start(&pfad, |_, mut strom| {
        std::thread::sleep(Duration::from_millis(600));
        let _ = strom.write_all(b"spaet\n");
    });
    let socket = UnixSocket::mit_grenzen(&pfad, Duration::from_secs(5), 1000);
    assert_eq!(socket.frage("x"), Ok("spaet".to_owned()));
}

#[test]
fn the_default_deadline_is_ten_seconds() {
    let wegwerf = Wegwerf::neu("zehn");
    let pfad = wegwerf.pfad("s.sock");
    let _stand_in = StandIn::start(&pfad, |_, mut strom| {
        let mut puffer = [0u8; 1];
        let _ = strom.read(&mut puffer);
    });
    let socket = UnixSocket::neu(&pfad);
    let start = Instant::now();
    assert_eq!(socket.frage("x"), Err(Unerreichbar));
    let dauer = start.elapsed();
    assert!(dauer >= Duration::from_millis(9800), "too early: {dauer:?}");
    assert!(dauer < Duration::from_secs(12), "too late: {dauer:?}");
}

#[test]
fn the_default_socket_has_the_default_limit() {
    let wegwerf = Wegwerf::neu("standard");
    let pfad = wegwerf.pfad("s.sock");
    let _stand_in = StandIn::start(&pfad, |_, mut strom| {
        let mut antwort = vec![b'a'; MAX_ANTWORT];
        antwort.push(b'\n');
        let _ = strom.write_all(&antwort);
    });
    let socket = UnixSocket::neu(&pfad);
    assert_eq!(socket.frage("x").map(|a| a.len()), Ok(MAX_ANTWORT));
}

#[test]
fn the_default_socket_refuses_an_answer_over_the_default_limit() {
    let wegwerf = Wegwerf::neu("ueber");
    let pfad = wegwerf.pfad("s.sock");
    let _stand_in = StandIn::start(&pfad, |_, mut strom| {
        let mut antwort = vec![b'a'; MAX_ANTWORT + 1];
        antwort.push(b'\n');
        let _ = strom.write_all(&antwort);
    });
    let socket = UnixSocket::neu(&pfad);
    assert_eq!(socket.frage("x"), Err(Unerreichbar));
}
