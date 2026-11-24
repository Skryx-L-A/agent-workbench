//! A change of the size with the real service: the viewer that lost its model picture gets a full picture (`Abzug`) before any change
//! against it, and no viewer loses its connection by a change of the size (`raster_luecke`, found in the run of the new view, 21.11.2026).
//! The visible outside cases run in `faelle.rs`; these tests speak the socket directly, with the real child `cat`.
#![cfg(unix)] // the crate is Unix only (see `src/lib.rs`)

mod gemeinsam;

use std::time::{Duration, Instant};

use gemeinsam::{Probe, Prozess, Wegwerf, startdatei};
use serde_json::json;

fn dienst(name: &str) -> (Wegwerf, Prozess) {
    let w = Wegwerf::neu(name);
    // 12 lines of 50 characters fill and scroll the screen, so that a smaller size changes rows of the picture; then the child echoes
    let skript = "i=0; while [ $i -lt 12 ]; do echo \"zeile$i-0123456789012345678901234567890123456789\"; i=$((i+1)); done; exec cat";
    let start = startdatei(&w, &["/bin/sh", "-c", skript], &json!({}));
    let p = Prozess::starten(&w, &start);
    (w, p)
}

fn groesse(p: &mut Probe, spalten: u32, zeilen: u32) {
    p.ereignis(
        &json!({"art": "Groesse", "sitzung": "s-alpha", "spalten": spalten, "zeilen": zeilen}),
    );
}

/// Moves the test clock until `bis` holds or 10 s are over.
fn takt_bis(p: &mut Probe, t: &mut u64, bis: impl Fn(&Probe) -> bool) -> bool {
    let ende = Instant::now() + Duration::from_secs(10);
    while Instant::now() < ende && !p.ende && !bis(p) {
        p.lesen(Duration::from_millis(20));
        if p.ende || bis(p) {
            break;
        }
        *t += 100;
        p.uhr(*t);
    }
    bis(p)
}

fn zwei_gesehen_oder_fehler(p: &Probe) -> bool {
    !p.fehler.is_empty() || p.zeilen.iter().any(|z| z.contains("zwei"))
}

/// One viewer, `Groesse` twice in a row (the way of the view when its area changes): no error frame, the output still arrives.
#[test]
fn ein_betrachter_groesse_zweimal_hintereinander() {
    let (w, _dienst) = dienst("gw-a");
    let mut a = Probe::neu(&w.socket(), false);
    let mut t = 0;
    a.abo(40, 10);
    a.text("eins");
    assert!(
        a.warte_text_takt("zeile11", &mut t),
        "ablehnungen {:?} fehler {:?} arten {:?}",
        a.ablehnungen,
        a.fehler,
        a.arten()
    );
    // a smaller size cuts rows of the picture, so the carrier reports changed rows for it; the last size is the smaller one, so no
    // later full picture hides the error
    // the clock moves before each size, so that the bundling distance is over and the changed rows go out at once
    t += 1000;
    a.uhr(t);
    groesse(&mut a, 82, 32);
    t += 1000;
    a.uhr(t);
    groesse(&mut a, 30, 6);
    a.text("zwei");
    assert!(
        takt_bis(&mut a, &mut t, zwei_gesehen_oder_fehler),
        "no output and no error"
    );
    assert!(a.fehler.is_empty(), "error frames: {:?}", a.fehler);
    assert_eq!(a.zeilen.len(), 6);
    assert!(a.ablehnungen.is_empty(), "{:?}", a.ablehnungen);
}

/// Two viewers with different sizes at one session that writes: the second subscribes with another size, then the first changes its
/// size; neither loses its connection and both see the output.
#[test]
fn zwei_betrachter_verschiedene_groesse_an_einer_schreibenden_sitzung() {
    let (w, _dienst) = dienst("gw-b");
    let mut a = Probe::neu(&w.socket(), false);
    let mut b = Probe::neu(&w.socket(), false);
    let mut t = 0;
    a.abo(40, 10);
    a.text("eins");
    assert!(a.warte_text_takt("zeile11", &mut t), "{:?}", a.ablehnungen);
    b.abo(43, 24);
    assert!(b.warte_text_takt("zeile", &mut t));
    groesse(&mut a, 30, 6);
    // let both viewers read what the change brought before anything is written again
    a.lesen(Duration::from_millis(300));
    b.lesen(Duration::from_millis(300));
    assert!(a.fehler.is_empty(), "error frames of a: {:?}", a.fehler);
    assert!(b.fehler.is_empty(), "error frames of b: {:?}", b.fehler);
    a.text("zwei");
    assert!(
        takt_bis(&mut a, &mut t, zwei_gesehen_oder_fehler),
        "viewer a: no output and no error"
    );
    assert!(
        takt_bis(&mut b, &mut t, zwei_gesehen_oder_fehler),
        "viewer b: no output and no error"
    );
    assert!(a.fehler.is_empty(), "error frames of a: {:?}", a.fehler);
    assert!(b.fehler.is_empty(), "error frames of b: {:?}", b.fehler);
}
