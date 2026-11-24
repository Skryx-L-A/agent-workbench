//! Measurement aid for A109 and A114 (not part of the contract): starts one program in the carrier (`Kind::starten`, under
//! Windows `ConPTY`) and measures the time from the start to the input field in the screen of the carrier.
//!
//!   `start_messen <env file> <cwd> <columns>x<rows> <frist ms> [<hex> ...] -- <program> [arguments ...]`
//!
//! The env file holds `NAME=value` lines; the child gets exactly these variables. The input field counts as there when the
//! screen text without white space contains `?forshortcuts`, `❯` or `>Try` (the pattern of the recording script of the contract `traeger`).
//! After that each `<hex>` goes to the child as bytes (the effect `pty`), with a pause of 400 ms after each. The program
//! writes one line with the times from the start in ms (`gestartet_ms`: `Kind::starten` returned; `erstes_ereignis_ms`: the
//! first event; `erster_text_ms`: the first row with text; `bereit_ms`: the input field, `-` when the time ran out), then the
//! rows of the screen after the keys, then ends the child (dropping the `Kind`).

use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use traeger::{Kind, Start};

const MUSTER: [&str; 3] = ["?forshortcuts", "❯", ">Try"];

fn bereit(kind: &Kind) -> bool {
    let text: String = kind
        .raster()
        .klartext()
        .concat()
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    MUSTER.iter().any(|m| text.contains(m))
}

/// Reads for `dauer` and hands every piece to the screen.
fn pumpe(kind: &mut Kind, dauer: Duration) {
    let ende = Instant::now() + dauer;
    while let Some(rest) = ende.checked_duration_since(Instant::now()) {
        if kind.ist_zu_ende() {
            return;
        }
        kind.lies(rest.min(Duration::from_millis(20)));
    }
}

fn unhex(text: &str) -> Option<Vec<u8>> {
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(text.get(i..i + 2)?, 16).ok())
        .collect()
}

fn main() -> ExitCode {
    let argumente: Vec<String> = std::env::args().skip(1).collect();
    let mut aus = std::io::stdout().lock();
    let mut fehler_aus = std::io::stderr().lock();
    let Some(trenner) = argumente.iter().position(|a| a == "--") else {
        let _ = writeln!(
            fehler_aus,
            "Aufruf: start_messen <env> <cwd> <spalten>x<zeilen> <frist ms> [<hex> ...] -- <programm> ..."
        );
        return ExitCode::from(64);
    };
    let (vorn, argv) = (&argumente[..trenner], &argumente[trenner + 1..]);
    let [env_datei, cwd, groesse, frist, tasten @ ..] = vorn else {
        let _ = writeln!(fehler_aus, "zu wenige Argumente");
        return ExitCode::from(64);
    };
    let env: Vec<(String, String)> = fs::read_to_string(env_datei)
        .unwrap_or_default()
        .lines()
        .filter_map(|z| z.split_once('='))
        .map(|(a, b)| (a.to_owned(), b.to_owned()))
        .collect();
    let (spalten, zeilen) = groesse
        .split_once('x')
        .and_then(|(s, z)| Some((s.parse().ok()?, z.parse().ok()?)))
        .unwrap_or((200, 60));
    let frist = Duration::from_millis(frist.parse().unwrap_or(60_000));
    let Some(tasten) = tasten.iter().map(|t| unhex(t)).collect::<Option<Vec<_>>>() else {
        let _ = writeln!(fehler_aus, "hex ungueltig");
        return ExitCode::from(64);
    };
    let anfang = Instant::now();
    let mut kind = match Kind::starten(Start {
        argv: argv.to_vec(),
        env,
        cwd: PathBuf::from(cwd),
        spalten,
        zeilen,
        ruecklauf: 2000,
    }) {
        Ok(kind) => kind,
        Err(fehler) => {
            let _ = writeln!(fehler_aus, "Start: {fehler}");
            return ExitCode::from(1);
        }
    };
    let gestartet = anfang.elapsed();
    let mut bereit_nach = None;
    // the first event (under ConPTY its preamble: the title with the path of the program) and the first row with text
    let mut erstes_ereignis = None;
    let mut erster_text = None;
    while anfang.elapsed() < frist && !kind.ist_zu_ende() {
        let ereignisse = kind.lies(Duration::from_millis(20));
        if erstes_ereignis.is_none() && !ereignisse.is_empty() {
            erstes_ereignis = Some(anfang.elapsed());
        }
        if erster_text.is_none()
            && kind
                .raster()
                .klartext()
                .iter()
                .any(|z| !z.trim().is_empty())
        {
            erster_text = Some(anfang.elapsed());
        }
        if bereit(&kind) {
            bereit_nach = Some(anfang.elapsed());
            break;
        }
    }
    let ms = |d: Option<Duration>| d.map_or_else(|| "-".to_owned(), |d| d.as_millis().to_string());
    let _ = writeln!(
        aus,
        "gestartet_ms={} erstes_ereignis_ms={} erster_text_ms={} bereit_ms={}",
        gestartet.as_millis(),
        ms(erstes_ereignis),
        ms(erster_text),
        ms(bereit_nach)
    );
    if bereit_nach.is_some() {
        pumpe(&mut kind, Duration::from_millis(1500));
        for bytes in &tasten {
            let _ = kind.schreibe(bytes);
            pumpe(&mut kind, Duration::from_millis(400));
        }
        pumpe(&mut kind, Duration::from_millis(1500));
    }
    for zeile in kind.raster().klartext() {
        let _ = writeln!(aus, "| {}", zeile.trim_end());
    }
    let _ = writeln!(aus, "ende_ms={}", anfang.elapsed().as_millis());
    ExitCode::SUCCESS
}
