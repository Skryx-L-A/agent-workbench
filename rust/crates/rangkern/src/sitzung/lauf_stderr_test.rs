//! A connection thread that turns a peer away and cannot write the log line writes one line to the error output
//! (`abgewiesen`). It must not depend on a lock that the main thread holds for the whole run: `main` hands `run_with_input`
//! the error output without a lock (review B1, 23.11.2026). The test runs the real path in its own thread, as the accept loop
//! does, with a foreign peer (macOS check) and a log folder that is gone.

use super::*;

use crate::sitzung::einst;
use std::fs;
use std::path::PathBuf;
use std::sync::mpsc;

fn sitzung_ohne_protokoll(ordner: &Path) -> Sitzung {
    fs::create_dir_all(ordner.join("vault")).expect("vault");
    let einst_datei = ordner.join("einst.json");
    fs::write(
        &einst_datei,
        r#"{"fassung":1,"klassen":["oeffentlich","intern"],"vorgabestufe":"intern","routen":[{"schluessel":"cloud","art":"cloud","klasse":"intern"}],"kunden":[],"kundendaten_an_cloud":{"vorgabe":"nie","freigaben":[]}}"#,
    )
    .expect("settings");
    let einst = einst::lesen_datei(&einst_datei).expect("settings are read");
    Sitzung {
        server: Mutex::new(Server::start(ordner.join("vault"), None)),
        tor: Tor::new(einst.vorgabe, "cloud".to_owned()),
        route: "cloud".to_owned(),
        kennung: "s-1".to_owned(),
        // The log folder does not exist: the line `kanal.abgewiesen` cannot be written.
        protokoll: ordner.join("protokoll-fehlt"),
        zaehlung: None,
        peer: Peerpruefung {
            plattform: Plattform::MacOs,
            euid: mac::eigene_euid(),
            messen: messen_fremd,
        },
    }
}

#[allow(clippy::unnecessary_wraps)] // the type of the measurement is fixed by the check
fn messen_fremd(_: &UnixStream) -> std::io::Result<u32> {
    Ok(mac::eigene_euid().wrapping_add(4242))
}

/// Starts one connection in its own thread, as the accept loop does; the channel says when `verbindung` returned.
fn verbindung_im_faden(ordner: &Path) -> mpsc::Receiver<()> {
    let sitzung = Arc::new(sitzung_ohne_protokoll(ordner));
    let (server, client) = UnixStream::pair().expect("pair");
    let (fertig, warten) = mpsc::channel();
    thread::spawn(move || {
        verbindung(server, &sitzung);
        drop(client);
        let _ = fertig.send(());
    });
    warten
}

fn ordner(name: &str) -> PathBuf {
    let ordner = PathBuf::from(format!("/tmp/rkl-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&ordner);
    fs::create_dir_all(&ordner).expect("folder is made");
    ordner
}

#[test]
fn a_turned_away_peer_whose_line_fails_ends_its_thread_while_main_holds_the_error_output() {
    let ordner = ordner("stderr-main");
    // The main thread keeps what `main` hands over for the whole run (`cli::fehlerausgabe`, the type has no lock).
    let _gehalten = crate::cli::fehlerausgabe();
    let warten = verbindung_im_faden(&ordner);
    assert!(
        warten.recv_timeout(Duration::from_secs(2)).is_ok(),
        "the connection thread did not end within 2 s"
    );
    let _ = fs::remove_dir_all(&ordner);
}

/// The positive control: a held lock on the error output (what `main` handed over before the fix) stops the thread on its line,
/// and it ends within 2 s once the lock is let go. So the thread really writes to the error output on this path.
#[test]
fn a_held_lock_on_the_error_output_stops_the_thread_until_it_is_let_go() {
    let ordner = ordner("stderr-sperre");
    let sperre = std::io::stderr().lock();
    let warten = verbindung_im_faden(&ordner);
    assert!(
        warten.recv_timeout(Duration::from_millis(500)).is_err(),
        "the thread ended although the error output was locked: it did not write its line"
    );
    drop(sperre);
    assert!(
        warten.recv_timeout(Duration::from_secs(2)).is_ok(),
        "the connection thread did not end within 2 s after the lock was let go"
    );
    let _ = fs::remove_dir_all(&ordner);
}
