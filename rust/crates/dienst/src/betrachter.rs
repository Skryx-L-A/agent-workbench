//! One viewer: a connection with its queue of frames, its extract (`rasterauszug`) and what the service remembers of it.

use crate::sys::UnixStream;
use std::collections::{BTreeMap, VecDeque};
use std::io::{ErrorKind, Write};
use std::net::Shutdown;

use rasterauszug::{Fehler, RAHMEN_MAX, Verbindung};
use serde_json::Value;

use crate::json;
use crate::rahmen::{A_ABLEHNUNG, A_NACHRICHT, A_ZUSTAND, Eingang, rahmen};

/// A connection that the service accepted (DI4). The name is `b<nr>`; `nr` counts from 1 in the order of accepting and is
/// never given again.
pub(crate) struct Betrachter {
    /// The number in the name.
    pub(crate) nr: u64,
    /// The name `b<nr>`, as the core knows the viewer.
    pub(crate) name: String,
    /// The socket of the loop (non-blocking): the loop reads after `poll`, writes ([`Betrachter::schreiben`]) and shuts the
    /// connection down.
    pub(crate) strom: UnixStream,
    /// What was read and is not a whole frame yet.
    pub(crate) eingang: Eingang,
    /// The frames the service took for this viewer and the socket has not taken yet, oldest first (DI5).
    pub(crate) schlange: VecDeque<Vec<u8>>,
    /// How many bytes of the first frame of the queue are written already.
    pub(crate) vorn: usize,
    /// `ausstehend` (DI5): the bytes of the whole frames of the queue, of a partly written frame the rest.
    pub(crate) laenge: usize,
    /// The sender of the extract of this connection: one model picture, one Deflate context (RA).
    pub(crate) raster: Raster,
    /// The size of the last accepted `Abo` per session (columns, rows), for the rebuild of the core (DI6).
    pub(crate) abos: BTreeMap<String, (i64, i64)>,
    /// Test mode: the viewer takes nothing, the kernel buffer counts as 0 bytes (DI5).
    pub(crate) stumm: bool,
    /// A protocol error came: the queue is written, then the connection ends, and nothing is read from it any more (DI3).
    pub(crate) schliessen: bool,
    /// The core got `Trennen` for this viewer already (at most once, DI4).
    pub(crate) trennen_gemeldet: bool,
}

/// The sender of the extract of one connection with the bound outward (contract `dienst`, DI3, Nachtrag 23.11.2026): a raster frame
/// on the socket has `n` at most [`RAHMEN_MAX`] + 1 (the art and at most [`RAHMEN_MAX`] bytes of the frame of `RA`). The crate
/// `rasterauszug` bounds only what it receives (RA11); its encoder writes a frame of any length. A full picture of 1000 x 1000 cells,
/// each with its own Truecolor style and a character of four bytes, is longer: 16 987 373 bytes before Deflate, 6 851 251 with
/// Deflate (measured on the Mac 23.11.2026); the test below checks it. Such a frame is [`Fehler::RahmenZuGross`] before it reaches the
/// queue; the service answers it like every error of the extract (`raster_rahmen_zu_gross`, the connection ends).
pub(crate) struct Raster(Verbindung);

impl Raster {
    /// The bytes on the wire for a message of the core, as [`Verbindung::nimm`], or [`Fehler::RahmenZuGross`] when the frame before
    /// Deflate or the bytes on the wire are longer than [`RAHMEN_MAX`]. The frame before Deflate is checked first, so a frame that is
    /// too long never goes through Deflate.
    pub(crate) fn nimm(&mut self, nr: u8, nachricht: &Value) -> Result<Option<Vec<u8>>, Fehler> {
        let Some(roh) = self.0.nimm_roh(nr, nachricht)? else {
            return Ok(None);
        };
        if roh.len() > RAHMEN_MAX {
            return Err(Fehler::RahmenZuGross);
        }
        let leitung = self.0.verpacken(&roh);
        if leitung.len() > RAHMEN_MAX {
            return Err(Fehler::RahmenZuGross);
        }
        Ok(Some(leitung))
    }

    /// Forgets the model picture of the raster number `nr`, as [`Verbindung::vergessen`] (P25: a number handed out again, P26: a
    /// session taken out of the list).
    pub(crate) fn vergessen(&mut self, nr: u8) {
        self.0.vergessen(nr);
    }
}

/// Every JSON frame outward (`Nachricht`, `Ablehnung`, `Fehler`, `Zustand`), with the bound outward (contract `dienst`, DI3,
/// Nachtrag 23.11.2026, Projektentscheid, 23.11.2026): the whole frame on the socket (head of four bytes, the art and the JSON text with its
/// escapes, [`json::bytes`]) has at most [`RAHMEN_MAX`] bytes. This is the only place that turns a JSON value into a frame; the test
/// `nur_hier_wird_json_gerahmt` checks that no other file of the crate calls [`json::bytes`]. A longer frame never reaches the
/// queue; the viewer gets instead a short frame of the same art with a fixed reason and no byte of the content ([`ersatz`]). The
/// connection stays: the viewer did nothing wrong, so it is no error of DI3.
pub(crate) fn json_rahmen(art: u8, wert: &Value) -> Vec<u8> {
    let voll = rahmen(art, &json::bytes(wert));
    if voll.len() <= RAHMEN_MAX {
        return voll;
    }
    rahmen(art, &json::bytes(&ersatz(art, wert)))
}

/// The short frame instead of a JSON frame over the bound, by art:
/// - `Nachricht`: `{"art": "Hinweis", "hinweis": <reason>, ...}` with `sitzung` and `t` of the message, if they are text or number;
///   the reason is `zwischenablage_zu_gross` for `Zwischenablage` and `nachricht_zu_gross` for every other message.
/// - `Zustand`: `{"art": "Hinweis", "hinweis": "zustand_zu_gross"}`, the same form.
/// - `Ablehnung`: `{"grund": "ablehnung_zu_gross"}`; `Fehler` (and any other art): `{"fehler": "fehler_zu_gross"}`. Both carry only
///   fixed words and never reach the bound (contract DI3); the reasons exist so that the common check has no gap.
fn ersatz(art: u8, wert: &Value) -> Value {
    match art {
        A_NACHRICHT => {
            let grund = if wert.get("art").and_then(Value::as_str) == Some("Zwischenablage") {
                "zwischenablage_zu_gross"
            } else {
                "nachricht_zu_gross"
            };
            let mut hinweis = serde_json::Map::new();
            hinweis.insert("art".into(), Value::from("Hinweis"));
            hinweis.insert("hinweis".into(), Value::from(grund));
            for feld in ["sitzung", "t"] {
                if let Some(w) = wert.get(feld).filter(|w| w.is_string() || w.is_number()) {
                    hinweis.insert(feld.into(), w.clone());
                }
            }
            Value::Object(hinweis)
        }
        A_ZUSTAND => serde_json::json!({"art": "Hinweis", "hinweis": "zustand_zu_gross"}),
        A_ABLEHNUNG => serde_json::json!({"grund": "ablehnung_zu_gross"}),
        _ => serde_json::json!({"fehler": "fehler_zu_gross"}),
    }
}

impl Betrachter {
    /// A viewer for a socket that was just accepted.
    pub(crate) fn neu(nr: u64, strom: UnixStream, deflate: bool) -> Self {
        Self {
            nr,
            name: format!("b{nr}"),
            strom,
            eingang: Eingang::default(),
            schlange: VecDeque::new(),
            vorn: 0,
            laenge: 0,
            raster: Raster(Verbindung::neu(deflate)),
            abos: BTreeMap::new(),
            stumm: false,
            schliessen: false,
            trennen_gemeldet: false,
        }
    }

    /// Puts a frame at the end of the queue.
    pub(crate) fn einreihen(&mut self, daten: Vec<u8>) {
        self.laenge += daten.len();
        self.schlange.push_back(daten);
    }

    /// Takes `n` bytes of the first frame of the queue as written.
    pub(crate) fn abgenommen(&mut self, n: usize) {
        self.laenge -= n;
        self.vorn += n;
        if self.schlange.front().is_some_and(|f| f.len() == self.vorn) {
            self.schlange.pop_front();
            self.vorn = 0;
        }
    }

    /// Drops the whole queue.
    pub(crate) fn verwerfen(&mut self) {
        self.schlange.clear();
        self.vorn = 0;
        self.laenge = 0;
    }

    /// Writes what the socket takes now; the rest stays at the front of the queue in order (DI5). A muted viewer gets nothing.
    /// The socket is non-blocking, so the service never blocks here. `false` if the connection is broken.
    pub(crate) fn schreiben(&mut self) -> bool {
        if self.stumm || self.schlange.is_empty() {
            return true;
        }
        loop {
            let Some(vorn) = self.schlange.front() else {
                break true;
            };
            match (&self.strom).write(&vorn[self.vorn..]) {
                Ok(0) => break false,
                Ok(n) => self.abgenommen(n),
                Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::Interrupted) => {
                    break true;
                }
                Err(_) => break false,
            }
        }
    }

    /// Ends the connection: the peer sees the end.
    pub(crate) fn schliessen_strom(&self) {
        let _ = self.strom.shutdown(Shutdown::Both);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paar() -> Betrachter {
        let (a, _b) = UnixStream::pair().expect("pair");
        Betrachter::neu(7, a, false)
    }

    /// A full picture of 1000 x 1000 cells, each with its own Truecolor style and a character of four bytes in UTF-8.
    fn grosser_abzug() -> Value {
        use std::fmt::Write as _;
        let mut zeilen = Vec::with_capacity(1000);
        let mut k: u32 = 0;
        for _ in 0..1000 {
            let mut z = String::new();
            for _ in 0..1000 {
                let fg = k & 0xff_ffff;
                let bg = k.wrapping_mul(2_654_435_761) & 0xff_ffff;
                let _ = write!(
                    z,
                    "\u{1b}[38;2;{};{};{};48;2;{};{};{}m\u{10348}",
                    fg >> 16,
                    (fg >> 8) & 0xff,
                    fg & 0xff,
                    bg >> 16,
                    (bg >> 8) & 0xff,
                    bg & 0xff
                );
                k += 1;
            }
            zeilen.push(Value::String(z));
        }
        serde_json::json!({"art": "Abzug", "sitzung": "s", "stand": 1, "groesse": [1000, 1000], "invers": false, "zeilen": zeilen})
    }

    /// DI3 outward (Nachtrag 23.11.2026): a frame longer than `RAHMEN_MAX` never reaches the queue, with and without Deflate; the crate
    /// alone writes it.
    #[test]
    fn ein_rasterrahmen_ueber_rahmen_max_ist_rahmen_zu_gross() {
        let gross = grosser_abzug();
        let roh = Verbindung::neu(false)
            .nimm_roh(0, &gross)
            .expect("the crate takes it")
            .expect("a frame");
        assert!(
            roh.len() > RAHMEN_MAX,
            "the crate wrote {} bytes",
            roh.len()
        );
        for deflate in [false, true] {
            let mut r = Raster(Verbindung::neu(deflate));
            // `matches!`, not `assert_eq!`: a red result would print a frame of 17 MB
            let ergebnis = r.nimm(0, &gross);
            assert!(
                matches!(ergebnis, Err(Fehler::RahmenZuGross)),
                "deflate {deflate}: {:?}",
                ergebnis.map(|o| o.map(|v| v.len()))
            );
        }
    }

    /// Below the bound the bytes are those of the crate, with and without Deflate.
    #[test]
    fn ein_kleiner_rasterrahmen_bleibt_wie_im_crate() {
        let klein = serde_json::json!({"art": "Abzug", "sitzung": "s", "stand": 1, "groesse": [4, 2], "invers": false, "zeilen": ["ab", ""]});
        for deflate in [false, true] {
            let mut r = Raster(Verbindung::neu(deflate));
            let mut v = Verbindung::neu(deflate);
            let a = r.nimm(0, &klein).expect("ok");
            assert!(a.is_some());
            assert_eq!(a, v.nimm(0, &klein).expect("ok"), "deflate {deflate}");
        }
    }

    /// A message `Zwischenablage` whose frame `Nachricht` has exactly `ziel` bytes on the socket; `daten_b64` ends with `ende`.
    fn ablage_mit_rahmen(ziel: usize, ende: &str) -> Value {
        let mit = |daten: String| serde_json::json!({"t": 9, "art": "Zwischenablage", "sitzung": "s1", "daten_b64": daten});
        let leer = rahmen(A_NACHRICHT, &json::bytes(&mit(String::new()))).len();
        // `ende` counts with its escapes in the JSON text
        let ende_json = json::bytes(&Value::from(ende)).len() - 2;
        let mut daten = "A".repeat(ziel - leer - ende_json);
        daten.push_str(ende);
        mit(daten)
    }

    fn hinweis(grund: &str) -> Vec<u8> {
        rahmen(
            A_NACHRICHT,
            &json::bytes(
                &serde_json::json!({"art": "Hinweis", "hinweis": grund, "sitzung": "s1", "t": 9}),
            ),
        )
    }

    /// DI3 outward for `Nachricht` (Nachtrag 23.11.2026): a frame of exactly `RAHMEN_MAX` bytes goes out as it is.
    #[test]
    fn eine_ablage_an_der_grenze_geht_hinaus() {
        let n = ablage_mit_rahmen(RAHMEN_MAX, "");
        let aus = json_rahmen(A_NACHRICHT, &n);
        assert_eq!(aus.len(), RAHMEN_MAX);
        assert!(
            aus == rahmen(A_NACHRICHT, &json::bytes(&n)),
            "the frame changed"
        );
    }

    /// One byte over the bound: the fixed reason instead, without content; an escape counts with its two bytes.
    #[test]
    fn eine_ablage_knapp_ueber_der_grenze_gibt_den_grund() {
        for (ende, name) in [("", "plain"), ("\"", "escape")] {
            let n = ablage_mit_rahmen(RAHMEN_MAX + 1, ende);
            assert_eq!(
                rahmen(A_NACHRICHT, &json::bytes(&n)).len(),
                RAHMEN_MAX + 1,
                "{name}"
            );
            let aus = json_rahmen(A_NACHRICHT, &n);
            // `assert!`, not `assert_eq!`: a red result would print a frame of 16 MB
            assert!(
                aus == hinweis("zwischenablage_zu_gross"),
                "{name}: {} bytes",
                aus.len()
            );
        }
        // the same JSON length with the escape is at the bound and goes out
        let an_der_grenze = ablage_mit_rahmen(RAHMEN_MAX, "\"");
        assert_eq!(json_rahmen(A_NACHRICHT, &an_der_grenze).len(), RAHMEN_MAX);
    }

    /// Every other message over the bound gets `nachricht_zu_gross`; one without `sitzung` and `t` gets a reason without them.
    #[test]
    fn eine_andere_nachricht_ueber_der_grenze_gibt_den_grund() {
        let gross = "x".repeat(RAHMEN_MAX);
        let n = serde_json::json!({"art": "Nachricht", "text": gross});
        let aus = json_rahmen(A_NACHRICHT, &n);
        assert!(
            aus == rahmen(
                A_NACHRICHT,
                br#"{"art": "Hinweis", "hinweis": "nachricht_zu_gross"}"#
            ),
            "{} bytes",
            aus.len()
        );
        let klein = serde_json::json!({"art": "Nachricht", "text": "x"});
        assert_eq!(
            json_rahmen(A_NACHRICHT, &klein),
            rahmen(A_NACHRICHT, &json::bytes(&klein))
        );
    }

    /// A frame `Zustand` with exactly `ziel` bytes on the socket; the text in `dienst.traeger_zeilen` ends with `ende`.
    fn zustand_mit_rahmen(ziel: usize, ende: &str) -> Value {
        let mit = |zeile: String| serde_json::json!({"kern": {"sitzungen": [], "betrachter": {}}, "dienst": {"kernbau": 1, "traeger_zeilen": [zeile]}});
        let leer = rahmen(A_ZUSTAND, &json::bytes(&mit(String::new()))).len();
        let ende_json = json::bytes(&Value::from(ende)).len() - 2;
        let mut zeile = "z".repeat(ziel - leer - ende_json);
        zeile.push_str(ende);
        mit(zeile)
    }

    /// DI3 outward for `Zustand` (Nachtrag 23.11.2026, Projektentscheid, 23.11.2026): exactly `RAHMEN_MAX` bytes go out as they are, one byte more gives
    /// the fixed reason `zustand_zu_gross` in a frame `Zustand`; an escape counts with its two bytes.
    #[test]
    fn ein_zustand_an_und_knapp_ueber_der_grenze() {
        let ersatz = rahmen(
            A_ZUSTAND,
            br#"{"art": "Hinweis", "hinweis": "zustand_zu_gross"}"#,
        );
        for (ende, name) in [("", "plain"), ("\"", "escape")] {
            let an = zustand_mit_rahmen(RAHMEN_MAX, ende);
            let aus = json_rahmen(A_ZUSTAND, &an);
            assert_eq!(aus.len(), RAHMEN_MAX, "{name}");
            assert!(
                aus == rahmen(A_ZUSTAND, &json::bytes(&an)),
                "{name}: the frame changed"
            );
            let drueber = zustand_mit_rahmen(RAHMEN_MAX + 1, ende);
            assert_eq!(
                rahmen(A_ZUSTAND, &json::bytes(&drueber)).len(),
                RAHMEN_MAX + 1,
                "{name}"
            );
            let aus = json_rahmen(A_ZUSTAND, &drueber);
            // `assert!`, not `assert_eq!`: a red result would print a frame of 16 MB
            assert!(aus == ersatz, "{name}: {} bytes", aus.len());
        }
    }

    /// `Ablehnung` and `Fehler` carry only fixed words and never reach the bound; the common check holds for them all the same.
    #[test]
    fn ablehnung_und_fehler_ueber_der_grenze_geben_den_grund() {
        let gross = "x".repeat(RAHMEN_MAX);
        let aus = json_rahmen(A_ABLEHNUNG, &serde_json::json!({"grund": gross}));
        assert!(
            aus == rahmen(A_ABLEHNUNG, br#"{"grund": "ablehnung_zu_gross"}"#),
            "{} bytes",
            aus.len()
        );
        let aus = json_rahmen(
            crate::rahmen::A_FEHLER,
            &serde_json::json!({"fehler": gross}),
        );
        assert!(
            aus == rahmen(crate::rahmen::A_FEHLER, br#"{"fehler": "fehler_zu_gross"}"#),
            "{} bytes",
            aus.len()
        );
        let klein = serde_json::json!({"grund": "taste_unbekannt"});
        assert_eq!(
            json_rahmen(A_ABLEHNUNG, &klein),
            rahmen(A_ABLEHNUNG, &json::bytes(&klein))
        );
    }

    /// No JSON frame goes out past [`json_rahmen`]: no other file of the crate calls `json::bytes` (outside its tests a call would
    /// build a JSON frame without the bound). The files are read at test time from the source folder of the crate.
    #[test]
    fn nur_hier_wird_json_gerahmt() {
        let ordner = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut gesehen = 0;
        let mut stapel = vec![ordner];
        while let Some(o) = stapel.pop() {
            for e in std::fs::read_dir(&o).expect("src") {
                let pfad = e.expect("entry").path();
                if pfad.is_dir() {
                    stapel.push(pfad);
                    continue;
                }
                let name = pfad.file_name().and_then(|n| n.to_str()).unwrap_or("");
                let rust = pfad.extension().and_then(|e| e.to_str()) == Some("rs");
                if !rust || name == "json.rs" || name == "betrachter.rs" {
                    continue;
                }
                gesehen += 1;
                let text = std::fs::read_to_string(&pfad).expect("read");
                assert!(
                    !text.contains("json::bytes"),
                    "{} builds JSON past json_rahmen",
                    pfad.display()
                );
            }
        }
        assert!(gesehen > 5, "only {gesehen} files seen");
    }

    #[test]
    fn name_aus_nummer() {
        let b = paar();
        assert_eq!((b.nr, b.name.as_str()), (7, "b7"));
        assert!(!b.stumm && !b.schliessen && !b.trennen_gemeldet);
    }

    fn lesen_bis(gegenueber: &mut UnixStream, n: usize) -> Vec<u8> {
        use std::io::Read;
        gegenueber
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .expect("timeout");
        let mut aus = vec![0; n];
        gegenueber.read_exact(&mut aus).expect("read");
        aus
    }

    #[test]
    fn schreibt_alles_was_der_socket_nimmt() {
        let (a, mut gegen) = UnixStream::pair().expect("pair");
        let mut b = Betrachter::neu(1, a, false);
        b.einreihen(vec![1, 2, 3]);
        b.einreihen(vec![4, 5]);
        assert!(b.schreiben());
        assert_eq!((b.laenge, b.schlange.len()), (0, 0));
        assert_eq!(lesen_bis(&mut gegen, 5), vec![1, 2, 3, 4, 5]);
        // empty queue: nothing to do
        assert!(b.schreiben());
    }

    #[test]
    fn stumm_schreibt_nichts() {
        let (a, mut gegen) = UnixStream::pair().expect("pair");
        let mut b = Betrachter::neu(1, a, false);
        b.stumm = true;
        b.einreihen(vec![9; 7]);
        assert!(b.schreiben());
        assert_eq!((b.laenge, b.schlange.len()), (7, 1));
        b.stumm = false;
        assert!(b.schreiben());
        assert_eq!(b.laenge, 0);
        assert_eq!(lesen_bis(&mut gegen, 7), vec![9; 7]);
    }

    #[test]
    fn blockiert_nie_und_haelt_den_rest_in_ordnung() {
        use std::io::Read;
        let (a, mut gegen) = UnixStream::pair().expect("pair");
        a.set_nonblocking(true)
            .expect("the loop makes the socket non-blocking");
        let mut b = Betrachter::neu(1, a, false);
        // far more than any socket buffer: the first write takes some, the rest waits
        let rahmen: Vec<u8> = (0..4_000_000_u32).map(|i| (i % 251) as u8).collect();
        b.einreihen(rahmen.clone());
        b.einreihen(vec![7; 10]);
        assert!(b.schreiben(), "returns although the peer reads nothing");
        assert!(b.laenge > 0 && b.laenge < rahmen.len() + 10);
        assert!(b.vorn > 0, "a partly written frame keeps its place");
        let geschrieben = rahmen.len() + 10 - b.laenge;
        // the peer reads; more can be written, in the same order
        let mut gelesen = Vec::new();
        let mut runden = 0;
        while gelesen.len() < rahmen.len() + 10 {
            runden += 1;
            assert!(runden < 100_000, "the bytes did not arrive");
            let mut puffer = vec![0; 65536];
            gegen.set_nonblocking(true).expect("nb");
            if let Ok(n) = gegen.read(&mut puffer) {
                gelesen.extend_from_slice(&puffer[..n]);
            }
            assert!(b.schreiben());
        }
        assert!(geschrieben > 0);
        assert_eq!(&gelesen[..rahmen.len()], &rahmen[..]);
        assert_eq!(&gelesen[rahmen.len()..], &[7; 10]);
        assert_eq!(b.laenge, 0);
    }

    #[test]
    fn kaputte_verbindung_meldet_false() {
        let (a, gegen) = UnixStream::pair().expect("pair");
        let mut b = Betrachter::neu(1, a, false);
        drop(gegen);
        // The other end is closed only when the last copy of its descriptor is gone: a test thread that forks a child at this moment
        // (the unit tests of this crate start children) leaves a copy in the child until its `exec`, and a write in that time still
        // goes into the buffer. So this waits for the state "the write fails" and queues one more piece each round (a write that
        // went through leaves the queue empty and says `true`); the deadline is only the bound of that wait.
        let frist = std::time::Instant::now() + std::time::Duration::from_secs(30);
        loop {
            b.einreihen(vec![1; 100]);
            if !b.schreiben() {
                break;
            }
            assert!(
                std::time::Instant::now() < frist,
                "the write to a closed connection never failed"
            );
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    #[test]
    fn schliessen_strom_beendet_die_verbindung() {
        use std::io::Read;
        let (a, mut gegen) = UnixStream::pair().expect("pair");
        let b = Betrachter::neu(1, a, false);
        b.schliessen_strom();
        gegen
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .expect("timeout");
        let mut rest = Vec::new();
        assert_eq!(gegen.read_to_end(&mut rest).expect("eof"), 0);
    }

    #[test]
    fn warteschlange_zaehlt_bytes() {
        let mut b = paar();
        b.einreihen(vec![0; 10]);
        b.einreihen(vec![0; 5]);
        assert_eq!(b.laenge, 15);
        b.abgenommen(4);
        assert_eq!((b.laenge, b.vorn, b.schlange.len()), (11, 4, 2));
        b.abgenommen(6);
        assert_eq!((b.laenge, b.vorn, b.schlange.len()), (5, 0, 1));
        b.abgenommen(5);
        assert_eq!((b.laenge, b.vorn, b.schlange.len()), (0, 0, 0));
        b.einreihen(vec![1; 3]);
        b.abgenommen(1);
        b.verwerfen();
        assert_eq!((b.laenge, b.vorn, b.schlange.len()), (0, 0, 0));
    }
}
