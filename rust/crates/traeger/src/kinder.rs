//! TR12: the fixed child programs of the tests (`tr --kind <name> [arguments]`). Each one calls only the standard library
//! and `stty`; every line ends in `\r\n`. They are part of the program `tr`, so that no other interpreter is needed.

use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc::channel;
use std::time::{Duration, Instant};

use crate::hex::{hex, unhex};

/// Runs the child program `args[0]` with the arguments after it and returns its exit code.
/// An unknown name ends with 64.
#[must_use]
#[allow(
    clippy::too_many_lines,
    reason = "one flat table of the fixed child programs; each arm is a few lines"
)]
pub fn lauf(args: &[String]) -> i32 {
    let mut aus = std::io::stdout().lock();
    let arg = |n: usize| args.get(n).map_or("", String::as_str);
    let zahl = || arg(1).parse::<usize>().unwrap_or(0);
    let code = match arg(0) {
        // writes the bytes (hex) and ends with 0
        "ausgabe" => {
            let _ = aus.write_all(&unhex(arg(1)).unwrap_or_default());
            0
        }
        // ends at once with the code, without output
        "ende" => arg(1).parse::<i32>().unwrap_or(0),
        // the lines Z000001 to Z<n>
        "zeilen" => {
            for i in 1..=zahl() {
                let _ = write!(aus, "Z{i:06}\r\n");
            }
            0
        }
        // BEREIT, reads one line, then the size the PTY reports (`stty size`)
        "groesse" => {
            let _ = write!(aus, "BEREIT\r\n");
            let _ = aus.flush();
            let mut zeile = String::new();
            let _ = std::io::stdin().read_line(&mut zeile);
            let ausgabe = Command::new("stty")
                .arg("size")
                .stdin(Stdio::inherit())
                .output();
            let text = ausgabe.map_or_else(
                |_| String::new(),
                |a| String::from_utf8_lossy(&a.stdout).trim().to_owned(),
            );
            let _ = write!(aus, "GROESSE {text}\r\n");
            0
        }
        // raw mode, BEREIT, reads exactly n bytes and writes them as hex
        "eingabe" => {
            let n = zahl();
            let _ = Command::new("stty")
                .args(["raw", "-echo"])
                .stdin(Stdio::inherit())
                .status();
            let _ = write!(aus, "BEREIT\r\n");
            let _ = aus.flush();
            let mut bytes = Vec::new();
            let _ = std::io::stdin()
                .take(u64::try_from(n).unwrap_or(0))
                .read_to_end(&mut bytes);
            let _ = write!(aus, "EINGABE {}\r\n", hex(&bytes));
            0
        }
        // NAME=<value> or NAME=- for each name
        "umgebung" => {
            for name in args.iter().skip(1) {
                let wert = std::env::var(name).unwrap_or_else(|_| "-".to_owned());
                let _ = write!(aus, "{name}={wert}\r\n");
            }
            0
        }
        // every variable as NAME=<value>, sorted by name (TR15)
        "umgebung_alle" => {
            let mut alle: Vec<(String, String)> = std::env::vars_os()
                .map(|(n, w)| {
                    (
                        n.to_string_lossy().into_owned(),
                        w.to_string_lossy().into_owned(),
                    )
                })
                .collect();
            alle.sort();
            for (name, wert) in alle {
                let _ = write!(aus, "{name}={wert}\r\n");
            }
            0
        }
        // BEREIT, then never reads its input; waits 30 s (TR15)
        "stumm" => {
            let _ = write!(aus, "BEREIT\r\n");
            let _ = aus.flush();
            std::thread::sleep(Duration::from_secs(30));
            0
        }
        // `cursor <hex>` (A109): asks for the cursor and writes the answer, see `cursor`
        "cursor" => {
            cursor(&mut aus, arg(1));
            0
        }
        // ESC ] 2 ; <text> BEL, then "fertig"
        "titel" => {
            let _ = write!(aus, "\x1b]2;{}\x07fertig\r\n", arg(1));
            0
        }
        // `signal <n> [<hex>]`: writes the bytes (if given), then sends itself the signal n (`sh -c 'kill -<n> <pid>'`) and
        // waits; the default action ends it
        "signal" => {
            let _ = aus.write_all(&unhex(arg(2)).unwrap_or_default());
            let _ = aus.flush();
            let _ = Command::new("sh")
                .arg("-c")
                .arg(format!("kill -{} {}", zahl(), std::process::id()))
                .status();
            std::thread::sleep(Duration::from_secs(5));
            0
        }
        // not in the contract, for the tests of this crate: raw mode, asks DA1 (`ESC [ c`), reads the 5 bytes of the answer and
        // writes them as `ANTWORT <hex>`
        "abfrage" => {
            let _ = Command::new("stty")
                .args(["raw", "-echo"])
                .stdin(Stdio::inherit())
                .status();
            let _ = aus.write_all(b"\x1b[c");
            let _ = aus.flush();
            let mut bytes = [0u8; 5];
            let _ = std::io::stdin().read_exact(&mut bytes);
            let _ = write!(aus, "ANTWORT {}\r\n", hex(&bytes));
            0
        }
        // the working folder as `ORDNER <path>` (TR15)
        "ordner" => {
            let _ = write!(
                aus,
                "ORDNER {}\r\n",
                std::env::current_dir()
                    .map_or_else(|_| "-".to_owned(), |p| p.display().to_string())
            );
            0
        }
        _ => 64,
    };
    let _ = aus.flush();
    code
}

/// The end mark of the child `cursor`: a status query (DSR 5) after the cursor queries, ...
const ENDMARKE_FRAGE: &[u8] = b"\x1b[5n";
/// ... and its answer, which comes after the answers to the queries before it.
const ENDMARKE_ANTWORT: &[u8] = b"\x1b[0n";

/// Reads stdin until the answer to the end mark or until `frist` has passed. Gives the bytes before the end mark and `true`, or
/// all bytes read and `false` when the time ran out. A thread reads, so that the wait has a limit; it ends with the process.
fn antworten_lesen(frist: Duration) -> (Vec<u8>, bool) {
    let (sender, empfaenger) = channel::<u8>();
    std::thread::spawn(move || {
        let mut eingabe = std::io::stdin().lock();
        let mut byte = [0u8; 1];
        while matches!(eingabe.read(&mut byte), Ok(1)) {
            if sender.send(byte[0]).is_err() {
                return;
            }
        }
    });
    let ende = Instant::now() + frist;
    let mut gelesen = Vec::new();
    while let Some(rest) = ende.checked_duration_since(Instant::now()) {
        let Ok(byte) = empfaenger.recv_timeout(rest) else {
            break;
        };
        gelesen.push(byte);
        if let Some(davor) = gelesen.strip_suffix(ENDMARKE_ANTWORT) {
            return (davor.to_vec(), true);
        }
    }
    (gelesen, false)
}

/// The child `cursor <hex>` (A109): raw mode, writes the bytes (with the cursor queries in them) and `ESC[5n` as an end mark,
/// reads stdin until the answer to the mark or for 1 s at most, and writes `ANTWORT <hex of the answers before the mark>` on
/// a new line, or `ZEIT <hex of what came>` when the second ran out.
fn cursor(aus: &mut impl Write, bytes_hex: &str) {
    let _ = Command::new("stty")
        .args(["raw", "-echo"])
        .stdin(Stdio::inherit())
        .status();
    let _ = aus.write_all(&unhex(bytes_hex).unwrap_or_default());
    let _ = aus.write_all(ENDMARKE_FRAGE);
    let _ = aus.flush();
    let (gelesen, fertig) = antworten_lesen(Duration::from_secs(1));
    let _ = if fertig {
        write!(aus, "\r\nANTWORT {}\r\n", hex(&gelesen))
    } else {
        write!(aus, "\r\nZEIT {}\r\n", hex(&gelesen))
    };
}
