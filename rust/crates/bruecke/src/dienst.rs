//! The service on the other side: a Unix socket with frames `[n: u32 big endian][kind: u8][payload]`, `n` counting the
//! kind and the payload. The bridge opens one short connection per start and one long connection per stream.

use std::io::{Read, Write};
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::time::{Duration, Instant};

use serde_json::{Map, Value, json};

use crate::fehler::{Abgelehnt, Antwort};

/// The kind of an event of the view (towards the service).
pub(crate) const ART_EREIGNIS: u8 = 0x01;
/// The kind of a message of the service.
const ART_NACHRICHT: u8 = 0x06;
/// The kind of a refusal of the service.
const ART_ABLEHNUNG: u8 = 0x07;
/// The kind of an error of the service.
const ART_FEHLER: u8 = 0x08;
/// The time for the answer of the service to a start.
pub(crate) const DIENST_FRIST: Duration = Duration::from_secs(10);
/// The largest frame the bridge reads from the service when it waits for the answer to a start.
const MAX_RAHMEN: usize = 1_048_576;

const DIENST: Abgelehnt = Abgelehnt {
    status: 502,
    grund: "dienst",
};

const ZEIT: Abgelehnt = Abgelehnt {
    status: 504,
    grund: "dienst",
};

/// A frame: the kind and the payload.
pub(crate) fn rahmen(art: u8, nutzlast: &[u8]) -> Vec<u8> {
    let n = u32::try_from(nutzlast.len() + 1).unwrap_or(u32::MAX);
    let mut aus = Vec::with_capacity(nutzlast.len() + 5);
    aus.extend_from_slice(&n.to_be_bytes());
    aus.push(art);
    aus.extend_from_slice(nutzlast);
    aus
}

/// Connects to the service. No connection: 502.
pub(crate) fn verbinden(pfad: &str) -> Result<UnixStream, Abgelehnt> {
    UnixStream::connect(pfad).map_err(|_| DIENST)
}

/// Takes the next whole frame off the front of `buf`. `Ok(None)`: not complete yet. `Err`: a frame of size 0 or above the
/// limit.
fn rahmen_nehmen(buf: &mut Vec<u8>) -> Result<Option<(u8, Vec<u8>)>, ()> {
    let Some(kopf) = buf.first_chunk::<4>() else {
        return Ok(None);
    };
    let n = usize::try_from(u32::from_be_bytes(*kopf)).map_err(|_| ())?;
    if n == 0 || n > MAX_RAHMEN {
        return Err(());
    }
    if buf.len() < 4 + n {
        return Ok(None);
    }
    let art = buf[4];
    let nutzlast = buf[5..4 + n].to_vec();
    buf.drain(..4 + n);
    Ok(Some((art, nutzlast)))
}

/// `POST /starten`: sends the event `Starten` with the start structure on a new short connection and waits for the answer.
pub(crate) fn starten(pfad: &str, start: &Value) -> Result<Antwort, Abgelehnt> {
    starten_mit_frist(pfad, start, DIENST_FRIST)
}

/// [`starten`] with the time to wait as a parameter (the tests use a short one).
pub(crate) fn starten_mit_frist(
    pfad: &str,
    start: &Value,
    frist: Duration,
) -> Result<Antwort, Abgelehnt> {
    let mut d = verbinden(pfad)?;
    let ereignis = json!({"art": "Starten", "start": start});
    let nutzlast = serde_json::to_vec(&ereignis).map_err(|_| DIENST)?;
    d.set_write_timeout(Some(frist)).map_err(|_| DIENST)?;
    d.write_all(&rahmen(ART_EREIGNIS, &nutzlast))
        .map_err(|_| DIENST)?;
    let ende = Instant::now() + frist;
    let mut buf: Vec<u8> = Vec::new();
    let antwort = loop {
        loop {
            let vorher = buf.len();
            let Some((art, nutzlast)) = rahmen_nehmen(&mut buf).map_err(|()| DIENST)? else {
                break;
            };
            // a frame always takes bytes off the buffer; if it did not, the loop would never end
            if buf.len() == vorher {
                return Err(DIENST);
            }
            if let Some(a) = antwort_zu(art, &nutzlast)? {
                return Ok(a);
            }
        }
        let rest = ende.saturating_duration_since(Instant::now());
        if rest.is_zero() {
            break Err(ZEIT);
        }
        // macOS refuses a timeout on a socket whose peer has closed (EINVAL); the read then ends at once, so the error is no reason to stop
        let _ = d.set_read_timeout(Some(rest));
        let mut stueck = vec![0u8; 65_536];
        match d.read(&mut stueck) {
            Ok(0) => break Err(DIENST),
            Ok(n) => buf.extend_from_slice(&stueck[..n]),
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                break Err(ZEIT);
            }
            Err(_) => break Err(DIENST),
        }
    };
    let _ = d.shutdown(Shutdown::Both);
    antwort
}

/// A JSON value in the form of Python's `json.dumps(v, sort_keys=True)`: the separators `", "` and `": "`, keys in order, and
/// every character above ASCII as `\uXXXX` (a surrogate pair above the plane 0). The answers of the bridge keep this form.
pub(crate) fn py_json(v: &Value) -> String {
    match v {
        Value::String(s) => py_text(s),
        Value::Array(a) => format!("[{}]", a.iter().map(py_json).collect::<Vec<_>>().join(", ")),
        Value::Object(o) => {
            let mut paare: Vec<(&String, &Value)> = o.iter().collect();
            paare.sort_by(|x, y| x.0.cmp(y.0));
            let teile: Vec<String> = paare
                .iter()
                .map(|(k, w)| format!("{}: {}", py_text(k), py_json(w)))
                .collect();
            format!("{{{}}}", teile.join(", "))
        }
        _ => v.to_string(),
    }
}

fn py_text(s: &str) -> String {
    use std::fmt::Write as _;
    let mut aus = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => aus.push_str("\\\""),
            '\\' => aus.push_str("\\\\"),
            '\n' => aus.push_str("\\n"),
            '\r' => aus.push_str("\\r"),
            '\t' => aus.push_str("\\t"),
            '\u{8}' => aus.push_str("\\b"),
            '\u{c}' => aus.push_str("\\f"),
            c if u32::from(c) < 0x20 || u32::from(c) > 0x7f => {
                let mut einheiten = [0u16; 2];
                for e in c.encode_utf16(&mut einheiten) {
                    let _ = write!(aus, "\\u{e:04x}");
                }
            }
            c => aus.push(c),
        }
    }
    aus.push('"');
    aus
}

/// The answer of the bridge to one frame of the service, or `None` if the frame does not decide.
fn antwort_zu(art: u8, nutzlast: &[u8]) -> Result<Option<Antwort>, Abgelehnt> {
    let wert = serde_json::from_slice::<Value>(nutzlast).ok();
    match art {
        ART_ABLEHNUNG => {
            let leer = Map::new();
            let o = wert.as_ref().and_then(Value::as_object).unwrap_or(&leer);
            let grund = o.get("grund").cloned().unwrap_or_else(|| json!(""));
            let code = o.get("code").cloned().unwrap_or_else(|| grund.clone());
            let text = o.get("text").cloned().unwrap_or_else(|| json!(""));
            let koerper = py_json(&json!({"code": code, "grund": grund, "text": text}));
            Ok(Some(Antwort::mit(
                422,
                "application/json",
                koerper.into_bytes(),
            )))
        }
        ART_FEHLER => Err(DIENST),
        ART_NACHRICHT => {
            let sitzung = wert
                .as_ref()
                .filter(|o| o.get("art") == Some(&json!("Gestartet")))
                .and_then(|o| o.get("sitzung"))
                .filter(|s| s.is_string());
            let Some(sitzung) = sitzung else {
                return Ok(None);
            };
            let koerper = py_json(&json!({"sitzung": sitzung}));
            Ok(Some(Antwort::mit(
                200,
                "application/json",
                koerper.into_bytes(),
            )))
        }
        _ => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pruefstand::{FakeDienst, Ordner};

    fn start() -> Value {
        json!({"a": 1})
    }

    #[test]
    fn python_form_of_json() {
        assert_eq!(
            py_json(&json!({"b": 1, "a": ["x", true, null]})),
            r#"{"a": ["x", true, null], "b": 1}"#
        );
        assert_eq!(
            py_json(&json!("a\"b\\c\n\r\t\u{8}\u{c}\u{1}\u{7f}\u{e4}\u{1f600}")),
            "\"a\\\"b\\\\c\\n\\r\\t\\b\\f\\u0001\u{7f}\\u00e4\\ud83d\\ude00\""
        );
        assert_eq!(py_json(&json!({})), "{}");
        assert_eq!(py_json(&json!([])), "[]");
        assert_eq!(py_json(&json!(-3)), "-3");
        assert_eq!(py_json(&json!({"\u{e4}": 1})), "{\"\\u00e4\": 1}");
    }

    #[test]
    fn a_frame_counts_the_kind_and_the_payload() {
        assert_eq!(rahmen(1, b"ab"), vec![0, 0, 0, 3, 1, b'a', b'b']);
        assert_eq!(rahmen(7, b""), vec![0, 0, 0, 1, 7]);
    }

    #[test]
    fn frames_are_taken_off_the_front() {
        let mut b = Vec::new();
        assert_eq!(rahmen_nehmen(&mut b), Ok(None));
        b.extend_from_slice(&[0, 0, 0]);
        assert_eq!(rahmen_nehmen(&mut b), Ok(None));
        b.extend_from_slice(&[3, 6, b'x']);
        assert_eq!(rahmen_nehmen(&mut b), Ok(None));
        b.extend_from_slice(&[b'y', 0, 0, 0, 1, 9, 0]);
        assert_eq!(rahmen_nehmen(&mut b), Ok(Some((6, b"xy".to_vec()))));
        assert_eq!(rahmen_nehmen(&mut b), Ok(Some((9, Vec::new()))));
        assert_eq!(b, vec![0]);
    }

    #[test]
    fn a_frame_of_size_zero_or_above_the_limit_is_an_error() {
        assert_eq!(rahmen_nehmen(&mut vec![0, 0, 0, 0, 1]), Err(()));
        let mut gross = u32::try_from(MAX_RAHMEN + 1)
            .expect("fits")
            .to_be_bytes()
            .to_vec();
        gross.push(1);
        assert_eq!(rahmen_nehmen(&mut gross), Err(()));
        let mut genau = u32::try_from(MAX_RAHMEN)
            .expect("fits")
            .to_be_bytes()
            .to_vec();
        genau.push(1);
        assert_eq!(rahmen_nehmen(&mut genau), Ok(None));
    }

    fn mit_dienst(f: impl FnOnce(&mut UnixStream) + Send + 'static) -> (Ordner, FakeDienst) {
        let o = Ordner::neu();
        let d = FakeDienst::neu(&o.pfad("d.sock"), f);
        (o, d)
    }

    #[test]
    fn started_session_is_a_200_with_the_identifier() {
        let (o, d) = mit_dienst(|s| {
            let mut buf = [0u8; 4096];
            let n = s.read(&mut buf).expect("read");
            // the event: frame kind 1, payload with art Starten and the structure
            assert_eq!(buf[4], 1);
            let v: Value = serde_json::from_slice(&buf[5..n]).expect("json");
            assert_eq!(v, json!({"art": "Starten", "start": {"a": 1}}));
            s.write_all(&rahmen(6, br#"{"art":"Sitzungsliste"}"#))
                .expect("write");
            s.write_all(&rahmen(5, b"x")).expect("write");
            s.write_all(&rahmen(6, br#"{"art":"Gestartet","sitzung":7}"#))
                .expect("write");
            s.write_all(&rahmen(6, br#"{"art":"Gestartet","sitzung":"s-2"}"#))
                .expect("write");
        });
        let a = starten(&d.pfad(), &start()).expect("answer");
        assert_eq!((a.status, a.typ), (200, Some("application/json")));
        let v: Value = serde_json::from_slice(&a.koerper).expect("json");
        assert_eq!(v, json!({"sitzung": "s-2"}));
        drop(o);
    }

    #[test]
    fn a_refusal_is_a_422_with_code_reason_and_text_unchanged() {
        let (_o, d) = mit_dienst(|s| {
            let mut buf = [0u8; 4096];
            let _ = s.read(&mut buf);
            s.write_all(&rahmen(
                7,
                r#"{"grund":"g","code":"start.x","text":"Täxt"}"#.as_bytes(),
            ))
            .expect("write");
        });
        let a = starten(&d.pfad(), &start()).expect("answer");
        assert_eq!(a.status, 422);
        let v: Value = serde_json::from_slice(&a.koerper).expect("json");
        assert_eq!(
            v,
            json!({"grund": "g", "code": "start.x", "text": "T\u{e4}xt"})
        );
    }

    #[test]
    fn a_refusal_without_code_and_text_gets_the_defaults() {
        let (_o, d) = mit_dienst(|s| {
            let mut buf = [0u8; 4096];
            let _ = s.read(&mut buf);
            s.write_all(&rahmen(7, br#"{"grund":"g"}"#)).expect("write");
        });
        let v: Value =
            serde_json::from_slice(&starten(&d.pfad(), &start()).expect("answer").koerper)
                .expect("json");
        assert_eq!(v, json!({"grund": "g", "code": "g", "text": ""}));
        let (_o, d) = mit_dienst(|s| {
            let mut buf = [0u8; 4096];
            let _ = s.read(&mut buf);
            s.write_all(&rahmen(7, b"[1]")).expect("write");
        });
        let v: Value =
            serde_json::from_slice(&starten(&d.pfad(), &start()).expect("answer").koerper)
                .expect("json");
        assert_eq!(v, json!({"grund": "", "code": "", "text": ""}));
    }

    #[test]
    fn an_error_frame_the_end_and_no_service_are_502() {
        let (_o, d) = mit_dienst(|s| {
            let mut buf = [0u8; 4096];
            let _ = s.read(&mut buf);
            s.write_all(&rahmen(8, br#"{"fehler":"x"}"#))
                .expect("write");
        });
        assert_eq!(starten(&d.pfad(), &start()), Err(DIENST));
        let (_o, d) = mit_dienst(|s| {
            let mut buf = [0u8; 4096];
            let _ = s.read(&mut buf);
        });
        assert_eq!(starten(&d.pfad(), &start()), Err(DIENST));
        let (_o, d) = mit_dienst(|s| {
            let mut buf = [0u8; 4096];
            let _ = s.read(&mut buf);
            s.write_all(&[0, 0, 0, 0, 1]).expect("write");
        });
        assert_eq!(starten(&d.pfad(), &start()), Err(DIENST));
        assert_eq!(starten("/nicht/da.sock", &start()), Err(DIENST));
        assert_eq!(verbinden("/nicht/da.sock").err(), Some(DIENST));
    }

    #[test]
    fn a_silent_service_is_a_504_within_the_time() {
        let (_o, d) = mit_dienst(|s| {
            let mut buf = [0u8; 4096];
            let _ = s.read(&mut buf);
            std::thread::sleep(Duration::from_millis(1500));
        });
        let t = Instant::now();
        let e =
            starten_mit_frist(&d.pfad(), &start(), Duration::from_millis(300)).expect_err("silent");
        assert_eq!(e, ZEIT);
        let dauer = t.elapsed();
        assert!(
            dauer >= Duration::from_millis(250) && dauer < Duration::from_millis(1200),
            "{dauer:?}"
        );
    }

    #[test]
    fn a_very_short_time_is_a_504() {
        let (_o, d) = mit_dienst(|_| std::thread::sleep(Duration::from_millis(300)));
        assert_eq!(
            starten_mit_frist(&d.pfad(), &start(), Duration::from_millis(1)).err(),
            Some(ZEIT)
        );
    }

    #[test]
    fn the_answer_to_messages_that_do_not_decide() {
        assert_eq!(antwort_zu(ART_NACHRICHT, b"not json"), Ok(None));
        assert_eq!(
            antwort_zu(ART_NACHRICHT, br#"{"art":"Gestartet"}"#),
            Ok(None)
        );
        assert_eq!(
            antwort_zu(ART_NACHRICHT, br#"{"art":"Anderes","sitzung":"s"}"#),
            Ok(None)
        );
        assert_eq!(antwort_zu(ART_NACHRICHT, br"[1]"), Ok(None));
        assert_eq!(
            antwort_zu(1, br#"{"art":"Gestartet","sitzung":"s"}"#),
            Ok(None)
        );
        assert_eq!(antwort_zu(ART_FEHLER, b""), Err(DIENST));
    }
}
