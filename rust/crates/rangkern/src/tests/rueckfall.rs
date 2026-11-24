//! The reason of a failed embedding (contract 4, "Rückfall der Einbettung"): every failure of a try is one of
//! `keine_verbindung`, `frist` and `antwort_ungueltig`, and `einbetten` gives the reason of the last try. Small servers on
//! loopback stand in for Ollama; nothing leaves the machine.

use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::Duration;

use super::guarded;
use crate::embed::{Embedder, Grund, grund_der_frist, grund_des_fehlers};

/// What the stand-in does with one connection.
#[derive(Clone, Copy)]
enum Art {
    /// Reads the request and answers with this status and body.
    Antwort(u16, &'static str),
    /// Reads the request and answers nothing for this long, then closes.
    Schweigen(Duration),
    /// Closes at once without reading.
    Zu,
}

/// A stand-in that serves one behaviour per connection, in order. After the last one it closes its port, so that a further
/// connect is refused. Returns the address and the number of connections it has taken.
fn attrappe(plan: Vec<Art>) -> (String, Arc<AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a free port");
    let url = format!(
        "http://127.0.0.1:{}",
        listener.local_addr().expect("addr").port()
    );
    let gezaehlt = Arc::new(AtomicUsize::new(0));
    let zaehler = Arc::clone(&gezaehlt);
    if plan.is_empty() {
        // Closed before the first connect, not in a thread that may come too late.
        drop(listener);
        return (url, gezaehlt);
    }
    thread::spawn(move || {
        let mut offen = Vec::new();
        for art in plan {
            let Ok((strom, _)) = listener.accept() else {
                return;
            };
            zaehler.fetch_add(1, Ordering::SeqCst);
            offen.push(thread::spawn(move || bedienen(strom, art)));
        }
        // The port is closed: the next connect is refused.
        drop(listener);
        for faden in offen {
            let _ = faden.join();
        }
    });
    (url, gezaehlt)
}

/// Reads the head and the body of one request.
fn anfrage_lesen(strom: &mut TcpStream) {
    let mut gelesen = Vec::new();
    let mut stueck = [0_u8; 4096];
    strom
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("timeout");
    loop {
        if let Some(ende) = gelesen.windows(4).position(|w| w == b"\r\n\r\n") {
            let kopf = String::from_utf8_lossy(&gelesen[..ende]).to_ascii_lowercase();
            let laenge: usize = kopf
                .lines()
                .find_map(|z| z.strip_prefix("content-length:"))
                .and_then(|v| v.trim().parse().ok())
                .unwrap_or(0);
            if gelesen.len() >= ende + 4 + laenge {
                return;
            }
        }
        match strom.read(&mut stueck) {
            Ok(0) | Err(_) => return,
            Ok(n) => gelesen.extend_from_slice(&stueck[..n]),
        }
    }
}

fn bedienen(mut strom: TcpStream, art: Art) {
    match art {
        Art::Zu => {}
        Art::Schweigen(dauer) => {
            anfrage_lesen(&mut strom);
            thread::sleep(dauer);
        }
        Art::Antwort(status, koerper) => {
            anfrage_lesen(&mut strom);
            let _ = write!(
                strom,
                "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{koerper}",
                koerper.len()
            );
        }
    }
}

fn einbetter(url: &str, frist_ms: u64) -> Embedder {
    Embedder::new(url, "m", "30m", frist_ms).expect("a loopback address")
}

/// The reason of a question asked of the stand-in with `plan`, and the number of connections it took.
fn grund(plan: Vec<Art>, frist_ms: u64) -> (Result<Vec<f64>, Grund>, usize) {
    guarded(move || {
        let (url, gezaehlt) = attrappe(plan);
        let ergebnis = einbetter(&url, frist_ms).einbetten("q");
        (ergebnis, gezaehlt.load(Ordering::SeqCst))
    })
}

const VEKTOR: &str = r#"{"embeddings": [[0.5, 0.25]]}"#;
const LANG: Duration = Duration::from_millis(1500);

#[test]
fn the_three_reasons_have_the_values_of_the_log_line() {
    assert_eq!(Grund::KeineVerbindung.text(), "keine_verbindung");
    assert_eq!(Grund::Frist.text(), "frist");
    assert_eq!(Grund::AntwortUngueltig.text(), "antwort_ungueltig");
}

#[test]
fn a_timeout_of_the_socket_is_the_deadline_and_every_other_error_an_invalid_answer() {
    assert_eq!(grund_des_fehlers(io::ErrorKind::TimedOut), Grund::Frist);
    assert_eq!(grund_des_fehlers(io::ErrorKind::WouldBlock), Grund::Frist);
    for kind in [
        io::ErrorKind::ConnectionReset,
        io::ErrorKind::ConnectionAborted,
        io::ErrorKind::BrokenPipe,
        io::ErrorKind::UnexpectedEof,
        io::ErrorKind::InvalidInput,
        io::ErrorKind::Other,
    ] {
        assert_eq!(grund_des_fehlers(kind), Grund::AntwortUngueltig, "{kind:?}");
    }
}

#[test]
fn a_refused_timeout_is_the_deadline_only_when_nothing_is_left() {
    assert_eq!(grund_der_frist(Duration::ZERO), Grund::Frist);
    assert_eq!(
        grund_der_frist(Duration::from_millis(1)),
        Grund::AntwortUngueltig
    );
}

#[test]
fn a_closed_port_is_no_connection() {
    let (ergebnis, _) = grund(Vec::new(), 2000);
    assert_eq!(ergebnis, Err(Grund::KeineVerbindung));
}

#[test]
fn an_answer_that_does_not_come_in_time_is_the_deadline_after_two_tries() {
    let (ergebnis, verbindungen) = grund(vec![Art::Schweigen(LANG), Art::Schweigen(LANG)], 300);
    assert_eq!(ergebnis, Err(Grund::Frist));
    assert_eq!(verbindungen, 2, "the same two tries as before");
}

#[test]
fn a_status_other_than_200_is_an_invalid_answer() {
    let (ergebnis, verbindungen) = grund(
        vec![Art::Antwort(500, VEKTOR), Art::Antwort(503, VEKTOR)],
        2000,
    );
    assert_eq!(ergebnis, Err(Grund::AntwortUngueltig));
    assert_eq!(verbindungen, 2);
}

#[test]
fn broken_json_and_an_empty_list_are_an_invalid_answer() {
    let (ergebnis, _) = grund(
        vec![
            Art::Antwort(200, r#"{"embeddings": [[0.5,"#),
            Art::Antwort(200, r#"{"embeddings": []}"#),
        ],
        2000,
    );
    assert_eq!(ergebnis, Err(Grund::AntwortUngueltig));
}

#[test]
fn a_connection_closed_before_the_answer_is_an_invalid_answer() {
    let (ergebnis, _) = grund(vec![Art::Zu, Art::Zu], 2000);
    assert_eq!(ergebnis, Err(Grund::AntwortUngueltig));
}

#[test]
fn the_last_try_counts_the_deadline_then_a_refused_connect_is_no_connection() {
    // The stand-in takes one connection and closes its port: the first try runs out of time, the second is refused.
    let (ergebnis, verbindungen) = grund(vec![Art::Schweigen(LANG)], 300);
    assert_eq!(ergebnis, Err(Grund::KeineVerbindung));
    assert_eq!(verbindungen, 1);
}

#[test]
fn the_last_try_counts_the_deadline_then_an_invalid_answer() {
    let (ergebnis, _) = grund(vec![Art::Schweigen(LANG), Art::Antwort(500, VEKTOR)], 300);
    assert_eq!(ergebnis, Err(Grund::AntwortUngueltig));
}

#[test]
fn a_second_try_with_a_vector_is_no_failure() {
    let (ergebnis, verbindungen) = grund(
        vec![Art::Antwort(500, VEKTOR), Art::Antwort(200, VEKTOR)],
        2000,
    );
    assert_eq!(ergebnis, Ok(vec![0.5, 0.25]));
    assert_eq!(verbindungen, 2);
}

#[test]
fn embed_gives_the_same_vector_and_none_where_einbetten_fails() {
    let ok = guarded(|| {
        let (url, _) = attrappe(vec![Art::Antwort(200, VEKTOR)]);
        einbetter(&url, 2000).embed("q")
    });
    assert_eq!(ok, Some(vec![0.5, 0.25]));
    let aus = guarded(|| {
        let (url, _) = attrappe(Vec::new());
        einbetter(&url, 2000).embed("q")
    });
    assert_eq!(aus, None);
}
