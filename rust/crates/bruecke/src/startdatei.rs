//! The start file (BR1): one JSON object, every other key is invalid, and no key names the address.

use std::fs::File;
use std::io::Read;
use std::time::Duration;

use serde_json::{Map, Value};

use crate::fehler::StartFehler;

/// The start file has at most this many bytes.
pub(crate) const MAX_STARTDATEI: u64 = 65_536;
/// The most connections at once, and the default.
pub(crate) const MAX_VERBINDUNGEN: usize = 64;
/// The default time to read head and body, in milliseconds.
pub(crate) const LESE_FRIST_MS: u64 = 10_000;

/// The checked content of the start file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// The path of the Unix socket of the service.
    pub dienst: String,
    /// The port to bind; 0 lets the system choose.
    pub port: u16,
    /// The path of the token file.
    pub token_datei: String,
    /// The path of the port file, if any.
    pub port_datei: Option<String>,
    /// Whether the raster frames of the stream are packed (`X-Deflate`).
    pub deflate: bool,
    /// The time to read head and body.
    pub lese_frist: Duration,
    /// The most connections at once.
    pub verbindungen_max: usize,
}

fn ungueltig() -> StartFehler {
    StartFehler {
        code: 2,
        text: "Startdatei ungueltig",
    }
}

/// Reads and checks the start file at `pfad`.
///
/// # Errors
///
/// Exit code 2 for a file that is missing, unreadable, too big or invalid.
pub fn lesen(pfad: &str) -> Result<Config, StartFehler> {
    let nicht_lesbar = StartFehler {
        code: 2,
        text: "Startdatei nicht lesbar",
    };
    let mut daten = Vec::new();
    File::open(pfad)
        .and_then(|f| f.take(MAX_STARTDATEI + 1).read_to_end(&mut daten))
        .map_err(|_| nicht_lesbar)?;
    aus_bytes(&daten)
}

/// Checks the bytes of a start file.
///
/// # Errors
///
/// Exit code 2 when the content is invalid.
pub fn aus_bytes(daten: &[u8]) -> Result<Config, StartFehler> {
    if daten.len() as u64 > MAX_STARTDATEI {
        return Err(StartFehler {
            code: 2,
            text: "Startdatei ist zu gross",
        });
    }
    let Ok(Value::Object(obj)) = serde_json::from_slice::<Value>(daten) else {
        return Err(ungueltig());
    };
    pruefen(&obj)
}

/// An absolute path without NUL that is not `/`, has no `/` at the end and has at most `max` bytes.
fn pfad_gueltig(s: &str, max: usize) -> bool {
    s.starts_with('/') && !s.contains('\0') && s != "/" && !s.ends_with('/') && s.len() <= max
}

fn ganzzahl(v: &Value, lo: u64, hi: u64) -> Option<u64> {
    v.as_u64().filter(|n| (lo..=hi).contains(n))
}

fn pruefen(obj: &Map<String, Value>) -> Result<Config, StartFehler> {
    const ERLAUBT: [&str; 8] = [
        "dienst",
        "port",
        "token_datei",
        "port_datei",
        "deflate",
        "pruefmodus",
        "lese_frist_ms",
        "verbindungen_max",
    ];
    if obj.keys().any(|k| !ERLAUBT.contains(&k.as_str())) {
        return Err(ungueltig());
    }
    let dienst = match obj.get("dienst") {
        Some(Value::Object(d)) if d.len() == 1 => match d.get("socket") {
            Some(Value::String(s))
                if s.starts_with('/') && !s.contains('\0') && (1..=100).contains(&s.len()) =>
            {
                s.clone()
            }
            _ => return Err(ungueltig()),
        },
        _ => return Err(ungueltig()),
    };
    let port = obj
        .get("port")
        .and_then(|v| ganzzahl(v, 0, 65_535))
        .and_then(|n| u16::try_from(n).ok())
        .ok_or_else(ungueltig)?;
    let token_datei = match obj.get("token_datei") {
        Some(Value::String(s)) if pfad_gueltig(s, 4096) => s.clone(),
        _ => return Err(ungueltig()),
    };
    let port_datei = match obj.get("port_datei") {
        None => None,
        Some(Value::String(s)) if pfad_gueltig(s, 4096) && *s != token_datei => Some(s.clone()),
        Some(_) => return Err(ungueltig()),
    };
    let Some(Value::Bool(deflate)) = obj.get("deflate") else {
        return Err(ungueltig());
    };
    let pruefmodus = match obj.get("pruefmodus") {
        None => false,
        Some(Value::Bool(b)) => *b,
        Some(_) => return Err(ungueltig()),
    };
    let mut lese_frist_ms = LESE_FRIST_MS;
    let mut verbindungen_max = MAX_VERBINDUNGEN;
    if let Some(v) = obj.get("lese_frist_ms") {
        lese_frist_ms = ganzzahl(v, 100, 10_000)
            .filter(|_| pruefmodus)
            .ok_or_else(ungueltig)?;
    }
    if let Some(v) = obj.get("verbindungen_max") {
        let n = ganzzahl(v, 1, 64)
            .filter(|_| pruefmodus)
            .ok_or_else(ungueltig)?;
        verbindungen_max = usize::try_from(n).map_err(|_| ungueltig())?;
    }
    Ok(Config {
        dienst,
        port,
        token_datei,
        port_datei,
        deflate: *deflate,
        lese_frist: Duration::from_millis(lese_frist_ms),
        verbindungen_max,
    })
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    fn gut() -> Value {
        serde_json::json!({"dienst": {"socket": "/a/d.sock"}, "port": 0, "token_datei": "/a/t", "deflate": false})
    }

    fn mit(f: impl FnOnce(&mut Map<String, Value>)) -> Result<Config, StartFehler> {
        let mut v = gut();
        f(v.as_object_mut().expect("object"));
        aus_bytes(v.to_string().as_bytes())
    }

    #[test]
    fn a_good_file_gives_the_defaults() {
        let c = aus_bytes(gut().to_string().as_bytes()).expect("valid");
        assert_eq!(
            c,
            Config {
                dienst: "/a/d.sock".into(),
                port: 0,
                token_datei: "/a/t".into(),
                port_datei: None,
                deflate: false,
                lese_frist: Duration::from_millis(10_000),
                verbindungen_max: 64,
            }
        );
    }

    #[test]
    fn all_optional_keys_are_taken() {
        let c = mit(|o| {
            o.insert("port".into(), 65_535.into());
            o.insert("port_datei".into(), "/a/p".into());
            o.insert("deflate".into(), true.into());
            o.insert("pruefmodus".into(), true.into());
            o.insert("lese_frist_ms".into(), 100.into());
            o.insert("verbindungen_max".into(), 1.into());
        })
        .expect("valid");
        assert_eq!(c.port, 65_535);
        assert_eq!(c.port_datei.as_deref(), Some("/a/p"));
        assert!(c.deflate);
        assert_eq!(c.lese_frist, Duration::from_millis(100));
        assert_eq!(c.verbindungen_max, 1);
        let c = mit(|o| {
            o.insert("pruefmodus".into(), true.into());
            o.insert("lese_frist_ms".into(), 10_000.into());
            o.insert("verbindungen_max".into(), 64.into());
        })
        .expect("valid");
        assert_eq!(
            (c.lese_frist, c.verbindungen_max),
            (Duration::from_secs(10), 64)
        );
        assert!(
            mit(|o| {
                o.insert("pruefmodus".into(), false.into());
            })
            .is_ok()
        );
    }

    #[test]
    fn invalid_files_are_exit_code_2() {
        let faelle: Vec<Box<dyn Fn(&mut Map<String, Value>)>> = vec![
            Box::new(|o| {
                o.insert("adresse".into(), "0.0.0.0".into());
            }),
            Box::new(|o| {
                o.insert("host".into(), "x".into());
            }),
            Box::new(|o| {
                o.remove("dienst");
            }),
            Box::new(|o| {
                o.remove("port");
            }),
            Box::new(|o| {
                o.remove("token_datei");
            }),
            Box::new(|o| {
                o.remove("deflate");
            }),
            Box::new(|o| {
                o.insert("deflate".into(), 1.into());
            }),
            Box::new(|o| {
                o.insert("dienst".into(), serde_json::json!({"socket": "rel"}));
            }),
            Box::new(|o| {
                o.insert("dienst".into(), serde_json::json!({"socket": ""}));
            }),
            Box::new(|o| {
                o.insert("dienst".into(), serde_json::json!({"socket": "/a\u{0}b"}));
            }),
            Box::new(|o| {
                o.insert(
                    "dienst".into(),
                    serde_json::json!({"socket": format!("/{}", "a".repeat(100))}),
                );
            }),
            Box::new(|o| {
                o.insert("dienst".into(), serde_json::json!({"socket": "/a", "x": 1}));
            }),
            Box::new(|o| {
                o.insert("dienst".into(), serde_json::json!({}));
            }),
            Box::new(|o| {
                o.insert("dienst".into(), serde_json::json!({"sock": "/a"}));
            }),
            Box::new(|o| {
                o.insert("dienst".into(), serde_json::json!({"socket": 5}));
            }),
            Box::new(|o| {
                o.insert("dienst".into(), "/a".into());
            }),
            Box::new(|o| {
                o.insert("port".into(), 65_536.into());
            }),
            Box::new(|o| {
                o.insert("port".into(), (-1).into());
            }),
            Box::new(|o| {
                o.insert("port".into(), "80".into());
            }),
            Box::new(|o| {
                o.insert("port".into(), true.into());
            }),
            Box::new(|o| {
                o.insert("port".into(), 1.5.into());
            }),
            Box::new(|o| {
                o.insert("token_datei".into(), "rel".into());
            }),
            Box::new(|o| {
                o.insert("token_datei".into(), "/".into());
            }),
            Box::new(|o| {
                o.insert("token_datei".into(), "/a/".into());
            }),
            Box::new(|o| {
                o.insert("token_datei".into(), "/a\u{0}".into());
            }),
            Box::new(|o| {
                o.insert(
                    "token_datei".into(),
                    format!("/{}", "a".repeat(4096)).into(),
                );
            }),
            Box::new(|o| {
                o.insert("token_datei".into(), 7.into());
            }),
            Box::new(|o| {
                o.insert("port_datei".into(), "/a/t".into());
            }),
            Box::new(|o| {
                o.insert("port_datei".into(), "rel".into());
            }),
            Box::new(|o| {
                o.insert("port_datei".into(), Value::Null);
            }),
            Box::new(|o| {
                o.insert("pruefmodus".into(), 1.into());
            }),
            Box::new(|o| {
                o.insert("lese_frist_ms".into(), 200.into());
            }),
            Box::new(|o| {
                o.insert("verbindungen_max".into(), 2.into());
            }),
            Box::new(|o| {
                o.insert("pruefmodus".into(), true.into());
                o.insert("lese_frist_ms".into(), 99.into());
            }),
            Box::new(|o| {
                o.insert("pruefmodus".into(), true.into());
                o.insert("lese_frist_ms".into(), 10_001.into());
            }),
            Box::new(|o| {
                o.insert("pruefmodus".into(), true.into());
                o.insert("lese_frist_ms".into(), 100.5.into());
            }),
            Box::new(|o| {
                o.insert("pruefmodus".into(), true.into());
                o.insert("verbindungen_max".into(), 0.into());
            }),
            Box::new(|o| {
                o.insert("pruefmodus".into(), true.into());
                o.insert("verbindungen_max".into(), 65.into());
            }),
            Box::new(|o| {
                o.insert("pruefmodus".into(), true.into());
                o.insert("verbindungen_max".into(), true.into());
            }),
        ];
        for (i, f) in faelle.iter().enumerate() {
            let e = mit(f).expect_err(&format!("case {i} must be invalid"));
            assert_eq!(e.code, 2, "case {i}");
            assert!(!e.text.is_empty() && !e.text.contains('\n'));
        }
    }

    #[test]
    fn path_limits_are_at_the_byte() {
        let dienst100 = format!("/{}", "a".repeat(99));
        assert!(
            mit(|o| {
                o.insert("dienst".into(), serde_json::json!({"socket": dienst100}));
            })
            .is_ok()
        );
        let tok = format!("/{}", "a".repeat(4095));
        assert!(
            mit(|o| {
                o.insert("token_datei".into(), tok.clone().into());
            })
            .is_ok()
        );
        // a multi-byte character counts by its bytes
        let mehr = format!("/{}", "\u{e4}".repeat(50));
        assert!(
            mit(|o| {
                o.insert("dienst".into(), serde_json::json!({"socket": mehr}));
            })
            .is_err()
        );
    }

    #[test]
    fn not_an_object_or_not_json() {
        for roh in [
            &b"[]"[..],
            b"1",
            b"null",
            b"",
            b"{",
            b"\xef\xbb\xbf{}",
            b"{\"port\": NaN}",
            b"\xff",
        ] {
            assert_eq!(aus_bytes(roh).expect_err("invalid").code, 2);
        }
        assert_eq!(
            aus_bytes(b"{\"deflate\": Infinity}")
                .expect_err("invalid")
                .code,
            2
        );
    }

    #[test]
    fn the_size_limit_is_65536_bytes() {
        let mut ok = gut().to_string().into_bytes();
        ok.pop();
        ok.extend_from_slice(b", \"pruefmodus\": false");
        // pad with spaces to exactly the limit: still valid
        ok.extend(std::iter::repeat_n(b' ', 65_536 - ok.len() - 1));
        ok.push(b'}');
        assert_eq!(ok.len(), 65_536);
        assert!(aus_bytes(&ok).is_ok());
        ok.push(b' ');
        assert_eq!(aus_bytes(&ok).expect_err("too big").code, 2);
    }

    #[test]
    fn reading_a_file_checks_the_size_and_a_missing_file() {
        let ordner = crate::pruefstand::Ordner::neu();
        let p = ordner.pfad("start.json");
        std::fs::write(&p, gut().to_string()).expect("write");
        assert!(lesen(p.to_str().expect("utf8")).is_ok());
        assert_eq!(lesen("/nicht/da/start.json").expect_err("missing").code, 2);
        assert_eq!(
            lesen(ordner.wurzel().to_str().expect("utf8"))
                .expect_err("dir")
                .code,
            2
        );
        // exactly 65 536 bytes are read whole; one byte more is refused
        let mut genau = gut().to_string().into_bytes();
        genau.pop();
        genau.extend_from_slice(b", \"pruefmodus\": false");
        genau.extend(std::iter::repeat_n(b' ', 65_536 - genau.len() - 1));
        genau.push(b'}');
        let datei = ordner.pfad("genau.json");
        std::fs::write(&datei, &genau).expect("write");
        assert!(lesen(datei.to_str().expect("utf8")).is_ok());
        genau.push(b' ');
        std::fs::write(&datei, &genau).expect("write");
        assert_eq!(
            lesen(datei.to_str().expect("utf8"))
                .expect_err("one byte too big")
                .code,
            2
        );
        let gross = ordner.pfad("gross.json");
        std::fs::write(&gross, vec![b' '; 70_000]).expect("write");
        assert_eq!(
            lesen(gross.to_str().expect("utf8")).expect_err("big").code,
            2
        );
    }
}
