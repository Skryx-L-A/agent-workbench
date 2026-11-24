//! The program `tr` on bad, large and odd input: exit codes, empty stdout, one line on stderr, never a panic; and the limits
//! of contract section 6 (TR13): sizes 2 to 1000 columns and 1 to 1000 rows (one column is refused), a scrollback of 100 000 lines, an OSC of more than 100 000 bytes, a title
//! of 3000 characters.

#![allow(clippy::too_many_lines, reason = "one long table of invalid cases")]

use std::fmt::Write as _;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{RecvTimeoutError, channel};
use std::time::Duration;

use serde_json::{Value, json};

const TR: &str = env!("CARGO_BIN_EXE_tr");
const GRENZE: usize = 4_194_304;

struct Lauf {
    code: Option<i32>,
    stdout: Vec<u8>,
    stderr: String,
}

fn starten(args: &[&str], stdin: &[u8]) -> Lauf {
    let mut kind = Command::new(TR)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("tr starts");
    let mut ein = kind.stdin.take().expect("stdin is piped");
    let daten = stdin.to_vec();
    let schreiber = std::thread::spawn(move || {
        let _ = ein.write_all(&daten);
    });
    let aus = kind.wait_with_output().expect("tr ends");
    schreiber.join().expect("writer ends");
    Lauf {
        code: aus.status.code(),
        stdout: aus.stdout,
        stderr: String::from_utf8_lossy(&aus.stderr).into_owned(),
    }
}

fn ungueltig(lauf: &Lauf) {
    assert_eq!(lauf.code, Some(2), "stderr: {}", lauf.stderr);
    assert!(lauf.stdout.is_empty(), "stdout is not empty");
    assert_eq!(lauf.stderr.lines().count(), 1, "stderr: {}", lauf.stderr);
}

fn gueltig(lauf: &Lauf) -> Value {
    assert_eq!(lauf.code, Some(0), "stderr: {}", lauf.stderr);
    assert!(lauf.stderr.is_empty(), "stderr: {}", lauf.stderr);
    assert!(lauf.stdout.ends_with(b"\n"));
    let wert: Value = serde_json::from_slice(&lauf.stdout).expect("stdout is JSON");
    assert!(wert["ende"].is_object());
    wert
}

fn strom(schritte: &Value) -> String {
    json!({"art": "strom", "groesse": [10, 3], "schritte": schritte}).to_string()
}

fn kind(start: &Value, schritte: &Value) -> String {
    json!({"art": "kind", "groesse": [10, 3], "start": start, "schritte": schritte}).to_string()
}

fn start_ausgabe() -> Value {
    json!({"kind": "ausgabe", "argv": ["6869"], "env": {"PATH": "/usr/bin:/bin"}})
}

#[test]
fn an_empty_case_gives_only_the_end_state() {
    let wert = gueltig(&starten(&[], strom(&json!([])).as_bytes()));
    assert_eq!(wert["ergebnisse"], json!([]));
    assert_eq!(
        wert["ende"],
        json!({"zeilen": ["", "", ""], "cursor": [0, 0, true], "groesse": [10, 3],
               "ruecklauf": {"zahl": 0, "erste": null, "letzte": null}, "abbild_gleich": true, "stand": 0})
    );
}

#[test]
fn invalid_cases_end_with_exit_2() {
    let schlecht: Vec<(&str, String)> = vec![
        ("not JSON", "kein json".to_owned()),
        ("empty input", String::new()),
        ("an array", "[]".to_owned()),
        ("a number", "5".to_owned()),
        ("no art", r#"{"groesse":[5,5],"schritte":[]}"#.to_owned()),
        (
            "art not a text",
            r#"{"art":5,"groesse":[5,5],"schritte":[]}"#.to_owned(),
        ),
        (
            "unknown art",
            r#"{"art":"x","groesse":[5,5],"schritte":[]}"#.to_owned(),
        ),
        ("no groesse", r#"{"art":"strom","schritte":[]}"#.to_owned()),
        (
            "groesse one number",
            r#"{"art":"strom","groesse":[5],"schritte":[]}"#.to_owned(),
        ),
        (
            "groesse three numbers",
            r#"{"art":"strom","groesse":[5,5,5],"schritte":[]}"#.to_owned(),
        ),
        (
            "groesse one column",
            r#"{"art":"strom","groesse":[1,5],"schritte":[]}"#.to_owned(),
        ),
        (
            "groesse one column, child part",
            kind(&start_ausgabe(), &json!([])).replacen("[10,3]", "[1,3]", 1),
        ),
        (
            "groesse zero columns",
            r#"{"art":"strom","groesse":[0,5],"schritte":[]}"#.to_owned(),
        ),
        (
            "groesse zero rows",
            r#"{"art":"strom","groesse":[5,0],"schritte":[]}"#.to_owned(),
        ),
        (
            "groesse 1001 columns",
            r#"{"art":"strom","groesse":[1001,5],"schritte":[]}"#.to_owned(),
        ),
        (
            "groesse 1001 rows",
            r#"{"art":"strom","groesse":[5,1001],"schritte":[]}"#.to_owned(),
        ),
        (
            "groesse a float",
            r#"{"art":"strom","groesse":[5.0,5],"schritte":[]}"#.to_owned(),
        ),
        (
            "groesse a negative",
            r#"{"art":"strom","groesse":[-5,5],"schritte":[]}"#.to_owned(),
        ),
        (
            "groesse a text",
            r#"{"art":"strom","groesse":["5",5],"schritte":[]}"#.to_owned(),
        ),
        (
            "groesse not a list",
            r#"{"art":"strom","groesse":5,"schritte":[]}"#.to_owned(),
        ),
        (
            "no schritte",
            r#"{"art":"strom","groesse":[5,5]}"#.to_owned(),
        ),
        (
            "schritte not a list",
            r#"{"art":"strom","groesse":[5,5],"schritte":{}}"#.to_owned(),
        ),
        ("a step without art", strom(&json!([{"text": "a"}]))),
        ("a step art not a text", strom(&json!([{"art": 1}]))),
        ("a step unknown art", strom(&json!([{"art": "Hallo"}]))),
        ("a step not an object", strom(&json!(["Text"]))),
        ("Text without text", strom(&json!([{"art": "Text"}]))),
        (
            "Text with a number",
            strom(&json!([{"art": "Text", "text": 5}])),
        ),
        ("Bytes without hex", strom(&json!([{"art": "Bytes"}]))),
        (
            "Bytes upper case",
            strom(&json!([{"art": "Bytes", "hex": "AB"}])),
        ),
        (
            "Bytes odd length",
            strom(&json!([{"art": "Bytes", "hex": "abc"}])),
        ),
        (
            "Bytes not hex",
            strom(&json!([{"art": "Bytes", "hex": "zz"}])),
        ),
        (
            "Bytes hex a number",
            strom(&json!([{"art": "Bytes", "hex": 12}])),
        ),
        (
            "Bytes hex with a space",
            strom(&json!([{"art": "Bytes", "hex": "a b"}])),
        ),
        (
            "Bytes hex g",
            strom(&json!([{"art": "Bytes", "hex": "0g"}])),
        ),
        (
            "Bytes hex slash",
            strom(&json!([{"art": "Bytes", "hex": "/0"}])),
        ),
        (
            "Bytes hex colon",
            strom(&json!([{"art": "Bytes", "hex": ":0"}])),
        ),
        (
            "Bytes hex backtick",
            strom(&json!([{"art": "Bytes", "hex": "`0"}])),
        ),
        (
            "Groesse step without spalten",
            strom(&json!([{"art": "Groesse", "zeilen": 5}])),
        ),
        (
            "Groesse step zero zeilen",
            strom(&json!([{"art": "Groesse", "spalten": 5, "zeilen": 0}])),
        ),
        (
            "Groesse step one spalte",
            strom(&json!([{"art": "Groesse", "spalten": 1, "zeilen": 5}])),
        ),
        (
            "Groesse step one spalte, versuch false",
            strom(&json!([{"art": "Groesse", "spalten": 1, "zeilen": 5, "versuch": false}])),
        ),
        (
            "Groesse step versuch not a bool",
            strom(&json!([{"art": "Groesse", "spalten": 5, "zeilen": 5, "versuch": 1}])),
        ),
        (
            "Groesse step versuch with more than 100000",
            strom(&json!([{"art": "Groesse", "spalten": 100_001, "zeilen": 5, "versuch": true}])),
        ),
        (
            "Groesse step versuch with a negative",
            strom(&json!([{"art": "Groesse", "spalten": -1, "zeilen": 5, "versuch": true}])),
        ),
        (
            "Groesse step 1001 spalten",
            strom(&json!([{"art": "Groesse", "spalten": 1001, "zeilen": 5}])),
        ),
        (
            "Groesse step 1001 zeilen",
            strom(&json!([{"art": "Groesse", "spalten": 5, "zeilen": 1001}])),
        ),
        (
            "Groesse step a float",
            strom(&json!([{"art": "Groesse", "spalten": 5.5, "zeilen": 5}])),
        ),
        (
            "Warte in a stream case",
            strom(&json!([{"art": "Warte", "bis": {"kindende": true}}])),
        ),
        (
            "Tasten in a stream case",
            strom(&json!([{"art": "Tasten", "hex": "61"}])),
        ),
        (
            "Text in a child case",
            kind(&start_ausgabe(), &json!([{"art": "Text", "text": "a"}])),
        ),
        (
            "Bytes in a child case",
            kind(&start_ausgabe(), &json!([{"art": "Bytes", "hex": "61"}])),
        ),
        (
            "child case without start",
            json!({"art": "kind", "groesse": [5, 5], "schritte": []}).to_string(),
        ),
        ("child start not an object", kind(&json!("x"), &json!([]))),
        (
            "child start without kind",
            kind(&json!({"env": {}}), &json!([])),
        ),
        (
            "child start kind a number",
            kind(&json!({"kind": 5, "env": {}}), &json!([])),
        ),
        (
            "child start without env",
            kind(&json!({"kind": "ausgabe"}), &json!([])),
        ),
        (
            "child start env a list",
            kind(&json!({"kind": "ausgabe", "env": []}), &json!([])),
        ),
        (
            "child start env value a number",
            kind(&json!({"kind": "ausgabe", "env": {"A": 1}}), &json!([])),
        ),
        (
            "child start argv not a list",
            kind(
                &json!({"kind": "ausgabe", "env": {}, "argv": "x"}),
                &json!([]),
            ),
        ),
        (
            "child start argv with a number",
            kind(
                &json!({"kind": "ausgabe", "env": {}, "argv": [1]}),
                &json!([]),
            ),
        ),
        (
            "child case without schritte",
            json!({"art": "kind", "groesse": [5, 5], "start": start_ausgabe()}).to_string(),
        ),
        (
            "Warte without bis",
            kind(&start_ausgabe(), &json!([{"art": "Warte"}])),
        ),
        (
            "Warte bis empty",
            kind(&start_ausgabe(), &json!([{"art": "Warte", "bis": {}}])),
        ),
        (
            "Warte bis not an object",
            kind(&start_ausgabe(), &json!([{"art": "Warte", "bis": "x"}])),
        ),
        (
            "Warte kindende false",
            kind(
                &start_ausgabe(),
                &json!([{"art": "Warte", "bis": {"kindende": false}}]),
            ),
        ),
        (
            "Warte text a number",
            kind(
                &start_ausgabe(),
                &json!([{"art": "Warte", "bis": {"text": 5}}]),
            ),
        ),
        (
            "Tasten without hex",
            kind(&start_ausgabe(), &json!([{"art": "Tasten"}])),
        ),
        (
            "Tasten bad hex",
            kind(&start_ausgabe(), &json!([{"art": "Tasten", "hex": "xyz"}])),
        ),
    ];
    for (name, fall) in &schlecht {
        let lauf = starten(&[], fall.as_bytes());
        assert_eq!(lauf.code, Some(2), "{name}: stderr: {}", lauf.stderr);
        assert!(lauf.stdout.is_empty(), "{name}: stdout is not empty");
        assert_eq!(
            lauf.stderr.lines().count(),
            1,
            "{name}: stderr: {}",
            lauf.stderr
        );
    }
    // bytes that are not UTF-8 at all
    ungueltig(&starten(&[], b"{\"art\":\"\xff\xfe\"}"));
}

#[test]
fn input_of_exactly_the_limit_is_valid_and_one_more_byte_is_not() {
    let klein = strom(&json!([]));
    let mut bytes = klein.as_bytes().to_vec();
    bytes.resize(GRENZE, b' ');
    gueltig(&starten(&[], &bytes));
    bytes.push(b' ');
    ungueltig(&starten(&[], &bytes));
    // the library applies the same limit
    assert!(traeger::laufen(&bytes, Path::new(TR)).is_err());
    bytes.pop();
    assert!(traeger::laufen(&bytes, Path::new(TR)).is_ok());
}

#[test]
fn a_repeated_tasten_step_is_limited_to_the_input_limit_and_cwd_must_be_a_folder() {
    let tasten = |hex: &str, mal: Value| {
        kind(
            &start_ausgabe(),
            &json!([{"art": "Tasten", "hex": hex, "mal": mal}]),
        )
    };
    // exactly 4,194,304 bytes in one write: valid (the queue refuses it, `ok` is false), one byte more is invalid
    let lauf = starten(&[], tasten("6162", json!(2_097_152)).as_bytes());
    let wert = gueltig(&lauf);
    assert_eq!(wert["ergebnisse"][0]["ok"], json!(false));
    assert_eq!(wert["ergebnisse"][0]["fehler"], json!("schlange_voll"));
    ungueltig(&starten(&[], tasten("616263", json!(1_398_102)).as_bytes()));
    ungueltig(&starten(&[], tasten("61", json!(4_194_305)).as_bytes()));
    ungueltig(&starten(&[], tasten("61", json!(0)).as_bytes()));
    ungueltig(&starten(&[], tasten("61", json!("2")).as_bytes()));
    let ein_mal = gueltig(&starten(&[], tasten("61", json!(1)).as_bytes()));
    assert_ne!(ein_mal["ergebnisse"][0]["fehler"], json!("schlange_voll"));
    // a second folder that exists on the system of the run
    #[cfg(unix)]
    let ordner = "/usr".to_owned();
    #[cfg(not(unix))]
    let ordner = Path::new(TR)
        .parent()
        .expect("tr is in a folder")
        .display()
        .to_string();
    for (cwd, gut) in [
        (json!("/"), true),
        (json!(ordner), true),
        (json!("/nonexistent-folder-of-traeger"), false),
        (json!(5), false),
        (json!(["/"]), false),
    ] {
        let mut start = start_ausgabe();
        start["cwd"] = cwd.clone();
        let lauf = starten(&[], kind(&start, &json!([])).as_bytes());
        if gut {
            gueltig(&lauf);
        } else {
            ungueltig(&lauf);
        }
    }
}

#[test]
fn nesting_up_to_128_levels_is_valid_and_deeper_is_not() {
    let tief = |n: usize| {
        let mut text = String::from(r#"{"art":"strom","groesse":[5,5],"schritte":[],"x":"#);
        text.push_str(&"[".repeat(n));
        text.push_str(&"]".repeat(n));
        text.push('}');
        text
    };
    gueltig(&starten(&[], tief(126).as_bytes()));
    ungueltig(&starten(&[], tief(200).as_bytes()));
}

#[test]
fn the_arguments_name_the_input() {
    let ordner = std::env::temp_dir().join(format!("traeger-grenzen-{}", std::process::id()));
    std::fs::create_dir_all(&ordner).expect("folder is made");
    let datei: PathBuf = ordner.join("fall.json");
    std::fs::write(&datei, strom(&json!([{"art": "Text", "text": "ab"}])))
        .expect("file is written");
    let pfad = datei.to_str().expect("path is text");
    let aus_datei = gueltig(&starten(&[pfad], b""));
    assert_eq!(aus_datei["ende"]["zeilen"][0], "ab");
    // "-" reads standard input, and so does no argument
    let stdin = strom(&json!([{"art": "Text", "text": "cd"}]));
    assert_eq!(
        gueltig(&starten(&["-"], stdin.as_bytes()))["ende"]["zeilen"][0],
        "cd"
    );
    assert_eq!(
        gueltig(&starten(&[], stdin.as_bytes()))["ende"]["zeilen"][0],
        "cd"
    );
    // a missing file, a folder, two arguments
    ungueltig(&starten(
        &[ordner.join("fehlt.json").to_str().expect("text")],
        b"",
    ));
    ungueltig(&starten(&[ordner.to_str().expect("text")], b""));
    ungueltig(&starten(&[pfad, pfad], b""));
    ungueltig(&starten(&["-", "-"], b""));
    std::fs::remove_dir_all(&ordner).expect("folder is removed");
}

#[test]
fn an_unknown_child_program_ends_with_64_and_an_exit_code_is_passed_on() {
    let lauf = starten(&["--kind", "gibtesnicht"], b"");
    assert_eq!(lauf.code, Some(64));
    assert_eq!(lauf.stdout.len(), 0);
    assert_eq!(starten(&["--kind", "ende", "7"], b"").code, Some(7));
    assert_eq!(starten(&["--kind", "ende", "255"], b"").code, Some(255));
    assert_eq!(starten(&["--kind", "ende"], b"").code, Some(0));
}

/// A stream case with one `Text` step and the state after it.
fn text_fall(groesse: [u32; 2], ruecklauf: Option<u64>, text: &str) -> Value {
    let mut fall = json!({"art": "strom", "groesse": groesse,
                          "schritte": [{"art": "Text", "text": text}, {"art": "Zustand"}]});
    if let Some(n) = ruecklauf {
        fall["ruecklauf"] = json!(n);
    }
    fall
}

fn lauf_bibliothek(fall: &Value) -> Value {
    traeger::laufen(fall.to_string().as_bytes(), Path::new(TR)).expect("case is valid")
}

#[test]
fn sizes_from_2_by_1_to_1000_by_1000_work() {
    for groesse in [[2, 1], [1000, 1000], [1000, 1], [2, 1000]] {
        let wert = lauf_bibliothek(&text_fall(
            groesse,
            None,
            "ab\r\ncd\u{1b}[31mXY\u{1b}[0m\u{1f44d}\u{1f3fd}",
        ));
        assert_eq!(wert["ende"]["groesse"], json!(groesse));
        let zeilen = wert["ende"]["zeilen"].as_array().expect("rows");
        assert_eq!(zeilen.len(), groesse[1] as usize);
    }
}

#[test]
fn a_scrollback_of_100000_lines_fills_to_the_limit() {
    let mut text = String::new();
    for i in 1..=100_500 {
        let _ = write!(text, "Z{i:06}\r\n");
    }
    let wert = lauf_bibliothek(&text_fall([20, 4], Some(100_000), &text));
    let ruecklauf = &wert["ende"]["ruecklauf"];
    assert_eq!(ruecklauf["zahl"], 100_000);
    // 100 500 lines and the cursor row leave 100 500 + 1 - 4 lines above the screen; the oldest 497 are gone
    assert_eq!(ruecklauf["erste"], "Z000498");
    assert_eq!(ruecklauf["letzte"], "Z100497");
}

#[test]
fn an_osc_52_of_more_than_100000_bytes_and_a_title_of_3000_characters_go_out_whole() {
    let daten = "QUJD".repeat(34_160);
    assert_eq!(daten.len(), 136_640);
    let titel = "t".repeat(3000);
    let text = format!("\u{1b}]52;c;{daten}\u{7}\u{1b}]2;{titel}\u{7}");
    let wert = lauf_bibliothek(
        &json!({"art": "strom", "groesse": [20, 4], "schritte": [{"art": "Text", "text": text}]}),
    );
    let ereignisse = &wert["ergebnisse"][0]["ereignisse"];
    assert_eq!(
        ereignisse[0],
        json!({"art": "Zwischenablage", "daten_b64": daten})
    );
    assert_eq!(ereignisse[1], json!({"art": "Titel", "text": titel}));
    assert_eq!(wert["ergebnisse"][0]["pty_antworten"], "");
}

/// A small deterministic generator (the same on every machine and every day).
struct Zufall(u64);

impl Zufall {
    /// A number from 0 up to (not including) `n`.
    fn bis(&mut self, n: usize) -> usize {
        let n = u64::try_from(n).unwrap_or(1);
        usize::try_from(self.naechste() % n).unwrap_or(0)
    }

    fn naechste(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 33
    }
}

#[test]
fn odd_bytes_at_odd_sizes_never_panic_or_hang() {
    mit_frist(|| {
        // pieces that make the parser and the grid do the hard things: escape starts, wide and combining characters, skin
        // tones cut in the middle, joiners, size changes, scrolling, erasing, the alternate screen, insert mode, no wrap
        let stuecke: &[&[u8]] = &[
            b"\x1b[",
            b"\x1b]",
            b"\x1b]52;c;",
            b"\x1b]2;",
            b"\x07",
            b"\x1b\\",
            b"\x1b[8;",
            b"\x1b[?5",
            b"h",
            b"l",
            b"2026",
            b"\xf0\x9f\x91\x8d",
            b"\xf0\x9f\x8f\xbd",
            b"\xf0\x9f",
            b"\xe4\xb8\xad",
            "e\u{301}".as_bytes(),
            b"\r\n",
            b"\x1b[2J",
            b"\x1b[100;100H",
            b"\x1b[10L",
            b"\x1b[5@",
            b"\x1b[?1049h",
            b"\xff\xfe",
            b"x",
            b"\x1b[?1049l",
            b"\x1b[?7l",
            b"\x1b[?7h",
            b"\x1b[4h",
            b"\x1b[4l",
            "\u{200d}\u{fe0f}".as_bytes(),
            b"\x1b[1;1H",
            b"\x1b[3J",
            b"\x1b[2K",
            b"\x08",
            b"\t",
            b"\x1b[5P",
            b"\x1b[3X",
            b"\x1bM",
            b"\x1b[c",
            b"\x1b[6n",
            b"\x1b[s",
            b"\x1b[u",
        ];
        let mut zufall = Zufall(7);
        let mut waehle = |n: usize| zufall.bis(n);
        for runde in 0..400 {
            let (spalten, zeilen) = (2 + waehle(11), 1 + waehle(6));
            let mut raster =
                traeger::Raster::neu(spalten, zeilen, waehle(8)).expect("size is valid");
            for _ in 0..40 {
                let mut bytes = Vec::new();
                for _ in 0..=waehle(6) {
                    bytes.extend_from_slice(stuecke[waehle(stuecke.len())]);
                }
                let _ = raster.bytes(&bytes);
                let _ = raster.antworten();
                if waehle(7) == 0 {
                    // some tries have one column and are refused; the others change the size
                    let _ = raster.groesse(1 + waehle(12), 1 + waehle(6));
                }
                assert!(raster.abbild_gleich(), "round {runde}");
            }
        }
    });
}

#[test]
fn a_wide_character_in_a_grid_of_two_columns_fills_a_row_and_never_panics() {
    mit_frist(|| {
        // two columns take the wide character as it is, and it fills a whole row
        let mut raster = traeger::Raster::neu(2, 2, 10).expect("two columns are valid");
        let _ = raster.bytes("\u{4e2d}".as_bytes());
        assert_eq!(raster.klartext(), ["\u{4e2d}", ""]);
        // a wide character in the last column wraps; the three bytes in three chunks give the same
        let mut raster = traeger::Raster::neu(2, 4, 10).expect("two columns are valid");
        let _ = raster.bytes(b"a");
        for stueck in [&b"\xe4"[..], b"\xb8", b"\xad", "\u{1f44d}x".as_bytes()] {
            let _ = raster.bytes(stueck);
        }
        assert_eq!(raster.klartext(), ["a", "\u{4e2d}", "\u{1f44d}", "x"]);
        // invalid bytes go on to the grid unchanged (it makes U+FFFD of them)
        let mut raster = traeger::Raster::neu(2, 5, 10).expect("two columns are valid");
        let _ = raster.bytes(b"a\xffb\xe4\xb8x");
        assert_eq!(raster.klartext(), ["a\u{fffd}", "b\u{fffd}", "x", "", ""]);
        // a grid that becomes two columns later takes a wide character too
        let mut raster = traeger::Raster::neu(4, 2, 10).expect("size is valid");
        let _ = raster.groesse(2, 2).expect("two columns are valid");
        let _ = raster.bytes("\u{4e2d}".as_bytes());
        assert_eq!(raster.klartext(), ["\u{4e2d}", ""]);
    });
}

#[test]
fn shrinking_to_two_columns_with_wide_characters_present_ends() {
    mit_frist(|| {
        // the size `Term::resize` of `alacritty_terminal` 0.26.0 never ends for is one column; at two it ends
        let mut raster = traeger::Raster::neu(11, 6, 10).expect("size is valid");
        let _ = raster.bytes("a\u{4e2d}b".as_bytes());
        let _ = raster.groesse(2, 6).expect("two columns are valid");
        assert_eq!(raster.groesse_jetzt(), (2, 6));
        assert!(raster.abbild_gleich());
        let alles = bildschirm_und_ruecklauf(&raster);
        assert!(alles.contains('\u{4e2d}'), "{alles:?}");
        // the wide character is in the scrollback only
        let mut raster = traeger::Raster::neu(11, 2, 10).expect("size is valid");
        let _ = raster.bytes("\u{1f44d}\r\n\r\n\r\n".as_bytes());
        assert_eq!(raster.ruecklauf_zahl(), 2);
        let _ = raster.groesse(2, 2).expect("two columns are valid");
        assert_eq!(
            raster.ruecklauf_enden().map(|(erste, _)| erste),
            Some("\u{1f44d}".to_owned())
        );
        // the alternate screen is on: the primary grid holds the wide character
        let mut raster = traeger::Raster::neu(11, 3, 10).expect("size is valid");
        let _ = raster.bytes("\u{4e2d}\x1b[?1049h\x1b[Hxyz".as_bytes());
        let _ = raster.groesse(2, 3).expect("two columns are valid");
        assert_eq!(raster.groesse_jetzt(), (2, 3));
        let _ = raster.bytes(b"\x1b[?1049l");
        let alles = bildschirm_und_ruecklauf(&raster);
        assert!(alles.contains('\u{4e2d}'), "{alles:?}");
        // growing again and a wide character afterwards work as before
        let _ = raster.groesse(6, 3).expect("size is valid");
        let _ = raster.bytes("\x1b[2J\x1b[H\u{4e2d}".as_bytes());
        assert_eq!(raster.klartext()[0], "\u{4e2d}");
    });
}

/// The text of the screen and of the oldest and newest line of the scrollback (enough while the scrollback is short).
fn bildschirm_und_ruecklauf(raster: &traeger::Raster) -> String {
    let (erste, letzte) = raster.ruecklauf_enden().unwrap_or_default();
    format!("{}{erste}{letzte}", raster.klartext().concat())
}

/// Everything a refused size must leave as it was.
fn stand_des_rasters(raster: &traeger::Raster) -> impl PartialEq + std::fmt::Debug + use<> {
    (
        raster.zeilen(),
        raster.cursor(),
        raster.groesse_jetzt(),
        raster.ruecklauf_zahl(),
        raster.ruecklauf_enden(),
        raster.stand(),
        raster.abbild_gleich(),
    )
}

#[test]
fn a_size_outside_the_limits_is_refused_at_the_start() {
    use traeger::GroesseUngueltig;
    for (spalten, zeilen) in [(1, 5), (0, 5), (5, 0), (1001, 5), (5, 1001), (1, 1), (0, 0)] {
        assert_eq!(
            traeger::Raster::neu(spalten, zeilen, 10).err(),
            Some(GroesseUngueltig),
            "{spalten} x {zeilen}"
        );
    }
    for (spalten, zeilen) in [(2, 1), (1000, 1000), (2, 1000), (1000, 1)] {
        assert!(
            traeger::Raster::neu(spalten, zeilen, 10).is_ok(),
            "{spalten} x {zeilen}"
        );
    }
}

#[test]
fn a_refused_size_changes_nothing_in_the_main_and_the_alternate_screen() {
    use traeger::GroesseUngueltig;
    for vorspann in [
        &b"main\r\nline 2 \xe4\xb8\xad\r\n3\r\n4\r\n5\x1b[2;3H"[..],
        b"main\x1b[?1049h\x1b[Hxyz\x1b[2;2H\xe4\xb8\xad",
    ] {
        let mut raster = traeger::Raster::neu(9, 3, 10).expect("size is valid");
        let _ = raster.bytes(vorspann);
        let vorher = stand_des_rasters(&raster);
        for (spalten, zeilen) in [(1, 3), (0, 3), (3, 0), (1001, 3), (3, 1001), (1, 1)] {
            assert_eq!(
                raster.groesse(spalten, zeilen),
                Err(GroesseUngueltig),
                "{spalten} x {zeilen}"
            );
            assert_eq!(stand_des_rasters(&raster), vorher, "{spalten} x {zeilen}");
        }
        // the grid still takes the size it did not take: a valid size works afterwards
        assert!(raster.groesse(2, 1).is_ok());
        assert_eq!(raster.groesse_jetzt(), (2, 1));
        assert!(raster.abbild_gleich());
    }
}

#[test]
fn the_errors_say_what_is_wrong() {
    let fehler = traeger::laufen(b"[]", Path::new(TR)).expect_err("an array is no case");
    assert_eq!(fehler.to_string(), "art");
    let fehler = traeger::laufen(
        br#"{"art":"strom","groesse":[0,1],"schritte":[]}"#,
        Path::new(TR),
    )
    .expect_err("size 0");
    assert_eq!(fehler.to_string(), "groesse");
    let fehler = traeger::laufen(
        br#"{"art":"strom","groesse":[5,5],"schritte":[{"art":"Bytes","hex":"0"}]}"#,
        Path::new(TR),
    )
    .expect_err("odd hex");
    assert_eq!(fehler.to_string(), "hex");
    let fehler = traeger::laufen(
        br#"{"art":"strom","groesse":[1,5],"schritte":[]}"#,
        Path::new(TR),
    )
    .expect_err("one column");
    assert_eq!(fehler.to_string(), "groesse");
    let fehler = traeger::laufen(b"nope", Path::new(TR)).expect_err("not JSON");
    assert!(fehler.to_string().contains("expected"), "{fehler}");
}

#[test]
fn outputs_after_the_end_of_the_child_are_counted() {
    // the child ends; two later size changes each move rows, and each is one `Ausgabe` after `Kindende`
    let fall = json!({
        "art": "kind", "groesse": [10, 3],
        "start": {"kind": "ausgabe", "argv": ["610d0a620d0a63"], "env": {"PATH": "/usr/bin:/bin"}},
        "schritte": [
            {"art": "Warte", "bis": {"kindende": true}}, {"art": "Zustand"},
            {"art": "Groesse", "spalten": 10, "zeilen": 2}, {"art": "Zustand"},
            {"art": "Groesse", "spalten": 10, "zeilen": 3}, {"art": "Zustand"},
            {"art": "Groesse", "spalten": 10, "zeilen": 3}, {"art": "Zustand"}
        ]
    });
    let wert = lauf_bibliothek(&fall);
    let zaehler: Vec<Value> = [1, 3, 5, 7]
        .iter()
        .map(|i| wert["ergebnisse"][*i]["zustand"]["ausgaben_nach_kindende"].clone())
        .collect();
    assert_eq!(zaehler, [json!(0), json!(1), json!(2), json!(2)]);
    assert_eq!(wert["ende"]["ausgaben_nach_kindende"], 2);
    assert_eq!(
        wert["ergebnisse"][0]["ereignisse"],
        json!([{"art": "Kindende", "code": 0}])
    );
}

#[test]
fn variation_selectors_and_joiners_behave_as_in_the_grid_and_do_not_crash() {
    // decision of 19.11.2026: no table of emoji presentation; the width is the one `alacritty_terminal` gives
    let mut raster = traeger::Raster::neu(12, 3, 10).expect("size is valid");
    let _ = raster.bytes("a\u{263a}\u{fe0f}b".as_bytes());
    assert_eq!(raster.klartext()[0], "a\u{263a}\u{fe0f}b");
    assert_eq!(raster.cursor(), (0, 3, true));
    // man, woman, girl joined by U+200D: three wide characters, the joiners take no cell
    let mut raster = traeger::Raster::neu(12, 3, 10).expect("size is valid");
    let _ = raster.bytes("\u{1f468}\u{200d}\u{1f469}\u{200d}\u{1f467}".as_bytes());
    assert_eq!(raster.cursor(), (0, 6, true));
    assert!(raster.abbild_gleich());
}

/// Runs `f` on its own thread and fails when it does not end within 8 s. The code of the grid has loops that never end for
/// some input; a test that meets one must fail and not hang (the thread is left behind and ends with the test program).
fn mit_frist(f: impl FnOnce() + Send + 'static) {
    let (sender, empfaenger) = channel();
    let faden = std::thread::spawn(move || {
        f();
        let _ = sender.send(());
    });
    match empfaenger.recv_timeout(Duration::from_secs(8)) {
        Ok(()) => faden.join().expect("the body ran"),
        Err(RecvTimeoutError::Timeout) => panic!("hangs: no end within 8 s"),
        Err(RecvTimeoutError::Disconnected) => match faden.join() {
            Err(grund) => std::panic::resume_unwind(grund),
            Ok(()) => unreachable!("the body ended without a signal"),
        },
    }
}

#[test]
fn the_five_skin_tones_are_zero_width_and_come_back_as_they_are() {
    let mut raster = traeger::Raster::neu(20, 2, 10).expect("size is valid");
    let text = "a\u{1f3fb}b\u{1f3fc}c\u{1f3fd}d\u{1f3fe}e\u{1f3ff}f";
    let _ = raster.bytes(text.as_bytes());
    assert_eq!(raster.klartext()[0], text);
    assert_eq!(raster.cursor(), (0, 6, true));
    // characters next to the stand-ins are not touched: U+E0105 is a variation selector, U+E00FF is unassigned
    let mut raster = traeger::Raster::neu(20, 2, 10).expect("size is valid");
    let text = "a\u{e0105}b\u{e00ff}c";
    let _ = raster.bytes(text.as_bytes());
    assert_eq!(raster.klartext()[0], text);
    // other four-byte characters that start like a modifier pass unchanged, and in pieces too
    let mut raster = traeger::Raster::neu(20, 2, 10).expect("size is valid");
    for stueck in [
        &b"\xf0"[..],
        b"\x9f",
        b"\x8f",
        b"\xba",
        "\u{1f600}".as_bytes(),
    ] {
        let _ = raster.bytes(stueck);
    }
    assert_eq!(raster.klartext()[0], "\u{1f3fa}\u{1f600}");
}

#[test]
fn the_alternate_grid_keeps_its_content_and_cursor_when_the_grid_becomes_two_columns() {
    let mut raster = traeger::Raster::neu(5, 3, 10).expect("size is valid");
    let _ = raster.bytes(b"main\x1b[?1049h\x1b[Hxyz");
    assert_eq!(raster.klartext()[0], "xyz");
    let _ = raster.groesse(2, 3).expect("two columns are valid");
    assert_eq!(raster.klartext()[0], "xy");
    // a wide character was seen in the primary grid: the alternate grid keeps its content and its cursor
    let mut raster = traeger::Raster::neu(5, 3, 10).expect("size is valid");
    let _ = raster.bytes("\u{4e2d}a\x1b[?1049h\x1b[2;1Hxyz\x1b[3;1H".as_bytes());
    assert_eq!(raster.klartext()[1], "xyz");
    assert_eq!(raster.cursor(), (2, 0, true));
    mit_frist(move || {
        let _ = raster.groesse(2, 3).expect("two columns are valid");
        assert_eq!(
            raster.klartext()[1],
            "xy",
            "the content of the alternate grid stays"
        );
        assert_eq!(raster.cursor(), (2, 0, true), "so does its cursor");
        assert!(raster.abbild_gleich());
        // leaving the alternate screen gives the primary grid with its wide character
        let _ = raster.bytes(b"\x1b[?1049l");
        let text = bildschirm_und_ruecklauf(&raster);
        assert!(text.contains('a') && text.contains('\u{4e2d}'), "{text:?}");
    });
    // a wide character in the alternate grid itself stays wide
    let mut raster = traeger::Raster::neu(5, 3, 10).expect("size is valid");
    let _ = raster.bytes("main\x1b[?1049h\x1b[H\u{4e2d}b".as_bytes());
    mit_frist(move || {
        let _ = raster.groesse(2, 3).expect("two columns are valid");
        assert_eq!(raster.klartext()[0], "\u{4e2d}");
    });
}

#[test]
fn a_wide_character_stays_wide_when_the_grid_changes_its_columns() {
    let mut raster = traeger::Raster::neu(4, 2, 10).expect("size is valid");
    let _ = raster.bytes("\u{4e2d}x".as_bytes());
    let _ = raster.groesse(6, 2).expect("size is valid");
    assert_eq!(raster.klartext()[0], "\u{4e2d}x");
    assert_eq!(raster.cursor(), (0, 3, true));
}

// ---------- the cursor in `Ausgabe` (TR2, TR3) ----------

/// No events at all, for the comparison with the result of a call.
fn keine() -> Vec<traeger::Ereignis> {
    Vec::new()
}

/// The cursor of an event `Ausgabe` as (row, column, visible), or `None` for another event.
fn ausgabe_cursor(ereignis: &traeger::Ereignis) -> Option<(usize, usize, bool)> {
    match ereignis {
        traeger::Ereignis::Ausgabe { cursor, .. } => Some(*cursor),
        _ => None,
    }
}

#[test]
fn every_output_carries_the_cursor_and_the_json_of_the_event_shows_it() {
    let mut raster = traeger::Raster::neu(10, 4, 10).expect("size is valid");
    let ereignisse = raster.bytes(b"ab\r\ncd");
    assert_eq!(ereignisse.len(), 1);
    assert_eq!(ausgabe_cursor(&ereignisse[0]), Some((1, 2, true)));
    assert_eq!(
        ereignisse[0].zu_json(),
        json!({"art": "Ausgabe", "stand": 1, "zeilen": {"0": "ab", "1": "cd"}, "cursor": [1, 2, true]})
    );
}

#[test]
fn a_pure_cursor_step_is_an_output_without_rows_and_the_stand_rises() {
    let mut raster = traeger::Raster::neu(10, 4, 10).expect("size is valid");
    let erste = raster.bytes(b"ab");
    assert_eq!(erste.len(), 1);
    let schritt = raster.bytes(b"\x1b[3;5H");
    assert_eq!(schritt.len(), 1);
    assert_eq!(
        schritt[0].zu_json(),
        json!({"art": "Ausgabe", "stand": 2, "zeilen": {}, "cursor": [2, 4, true]})
    );
    // the same place again is no step
    assert_eq!(raster.bytes(b"\x1b[3;5H"), keine());
    assert_eq!(raster.bytes(b""), keine());
    assert_eq!(raster.stand(), 2);
}

#[test]
fn the_start_cursor_counts_as_reported_and_a_mode_alone_reports_no_output() {
    let mut raster = traeger::Raster::neu(10, 4, 10).expect("size is valid");
    assert!(raster.abbild_gleich());
    let ereignisse = raster.bytes(b"\x1b[?5h");
    assert_eq!(ereignisse.len(), 1, "only the mode: {ereignisse:?}");
    assert!(!ereignisse[0].ist_ausgabe());
    assert!(
        raster.bytes(b"\x1b[1;1H").is_empty(),
        "the cursor is at its start place"
    );
}

#[test]
fn hiding_and_showing_the_cursor_is_reported_and_shares_the_stand_with_the_mode() {
    let mut raster = traeger::Raster::neu(10, 4, 10).expect("size is valid");
    let aus = raster.bytes(b"\x1b[?25l");
    assert_eq!(
        aus.iter()
            .map(traeger::Ereignis::zu_json)
            .collect::<Vec<_>>(),
        [json!({"art": "Ausgabe", "stand": 1, "zeilen": {}, "cursor": [0, 0, false]})]
    );
    let mit_modus = raster.bytes(b"\x1b[?5h\x1b[?25h");
    assert_eq!(mit_modus.len(), 2);
    assert_eq!(
        mit_modus[0].zu_json(),
        json!({"art": "Modus", "invers": true, "stand": 2})
    );
    assert_eq!(
        mit_modus[1].zu_json(),
        json!({"art": "Ausgabe", "stand": 3, "zeilen": {}, "cursor": [0, 0, true]})
    );
}

#[test]
fn the_output_before_another_event_carries_the_cursor_of_that_moment() {
    let mut raster = traeger::Raster::neu(10, 4, 10).expect("size is valid");
    let ereignisse = raster.bytes(b"ab\x1b[?2004hcd");
    assert_eq!(ereignisse.len(), 3);
    assert_eq!(ausgabe_cursor(&ereignisse[0]), Some((0, 2, true)));
    assert!(!ereignisse[1].ist_ausgabe());
    assert_eq!(ausgabe_cursor(&ereignisse[2]), Some((0, 4, true)));
    // only the cursor moved before the event: an output without rows
    let mut raster = traeger::Raster::neu(10, 4, 10).expect("size is valid");
    let ereignisse = raster.bytes(b"\x1b[2;3H\x1b[?2026h");
    assert_eq!(ereignisse.len(), 2);
    assert_eq!(
        ereignisse[0].zu_json(),
        json!({"art": "Ausgabe", "stand": 1, "zeilen": {}, "cursor": [1, 2, true]})
    );
}

#[test]
fn the_picture_is_not_equal_while_only_the_cursor_is_unreported() {
    let mut raster = traeger::Raster::neu(10, 4, 10).expect("size is valid");
    let _ = raster.bytes(b"ab");
    assert!(raster.abbild_gleich());
    // a partial sequence moves nothing; the report of the cursor is part of the picture
    let _ = raster.bytes(b"\x1b[?25l");
    assert!(raster.abbild_gleich());
}

#[test]
fn a_size_change_clamps_the_reported_cursor_and_reports_the_cursor_of_the_new_grid_when_it_differs()
{
    // the grid keeps the cursor where the text is; the core clamps its cursor to the last row and column: a report only
    // follows when the grid ends up elsewhere
    let mut raster = traeger::Raster::neu(10, 4, 10).expect("size is valid");
    let _ = raster.bytes(b"\x1b[4;9H");
    let ereignisse = raster.groesse(6, 4).expect("size is valid");
    let letzter = ereignisse.iter().rev().find_map(ausgabe_cursor);
    assert_eq!(letzter, Some(raster.cursor()), "{ereignisse:?}");
    assert!(raster.abbild_gleich());
    // growing keeps the place: nothing differs from the cursor the core has, so no event
    let mut raster = traeger::Raster::neu(10, 4, 10).expect("size is valid");
    let _ = raster.bytes(b"\x1b[2;3H");
    assert_eq!(raster.groesse(20, 8), Ok(keine()));
    assert!(raster.abbild_gleich());
    // shrinking at the cursor: the clamp of the core is [1, 2] here and the grid agrees, so no event either
    let mut raster = traeger::Raster::neu(10, 4, 10).expect("size is valid");
    let _ = raster.bytes(b"\x1b[2;3H");
    assert_eq!(raster.groesse(3, 2), Ok(keine()));
    assert_eq!(raster.cursor(), (1, 2, true));
    assert!(raster.abbild_gleich());
}

#[test]
fn the_cursor_is_in_the_scrollback_never() {
    let mut raster = traeger::Raster::neu(5, 2, 10).expect("size is valid");
    let ereignisse = raster.bytes(b"1\r\n2\r\n3\r\n4\r\n5");
    assert!(raster.ruecklauf_zahl() > 0);
    assert_eq!(
        ereignisse.iter().rev().find_map(ausgabe_cursor),
        Some((1, 1, true))
    );
}

// ---------- the answers to the child (TR10) ----------

#[test]
fn only_the_fixed_answers_of_the_grid_go_back_to_the_child() {
    let mut raster = traeger::Raster::neu(30, 8, 10).expect("size is valid");
    // DA1 and the cursor position are answered, with fixed bytes and numbers of the grid
    let _ = raster.bytes(b"\x1b[c");
    assert_eq!(raster.antworten(), b"\x1b[?6c");
    let _ = raster.bytes(b"\x1b[3;5H\x1b[6n");
    assert_eq!(raster.antworten(), b"\x1b[3;5R");
    // the requests the grid reports as events of their own are never answered: colours, text area size, clipboard, title
    for frage in [
        &b"\x1b]10;?\x07"[..],
        b"\x1b]11;?\x1b\\",
        b"\x1b]4;1;?\x07",
        b"\x1b[14t",
        b"\x1b[16t",
        b"\x1b[19t",
        b"\x1b[21t",
        b"\x1b]52;c;?\x07",
        b"\x1b[?6n",
        b"\x05",
    ] {
        let _ = raster.bytes(frage);
        assert_eq!(raster.antworten(), b"", "an answer to {frage:?}");
    }
    // text that was set before never comes back in an answer
    let _ = raster.bytes(b"\x1b]2;geheimer-titel\x07\x1b]11;rgb:12/34/56\x07\x1b[21t\x1b]11;?\x07\x1bP$qgeheim\x1b\\");
    assert_eq!(raster.antworten(), b"");
}
