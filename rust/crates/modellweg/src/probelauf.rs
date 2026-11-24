//! The marker count of a trial run (contract 8, section 9, every mode): per accepted request the model path counts how
//! often each marker occurs byte for byte in the body it sends to the target, and rewrites the count file atomically
//! (0600). No content is ever written: only the numbers.

use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use serde_json::{Map, Value};

use crate::loeschen::{Kennung, Ordner};

/// The count of one session.
pub struct Zaehlung {
    marker: Vec<String>,
    datei: PathBuf,
    sitzung: String,
    stand: Mutex<(u64, Vec<u64>)>,
}

impl Zaehlung {
    /// A count for `marker` (in the order of the call) into `datei`; nothing is written yet.
    #[must_use]
    pub fn neu(marker: Vec<String>, datei: PathBuf, sitzung: String) -> Self {
        let null = vec![0; marker.len()];
        Self {
            marker,
            datei,
            sitzung,
            stand: Mutex::new((0, null)),
        }
    }

    /// The number of markers.
    #[must_use]
    pub fn anzahl(&self) -> usize {
        self.marker.len()
    }

    /// Step 5 of the start: writes the state with `anfragen` 0.
    ///
    /// # Errors
    /// The file cannot be written (`mw.zaehlung_fehler`).
    pub fn anfang(&self) -> io::Result<()> {
        let stand = self.stand.lock().unwrap_or_else(PoisonError::into_inner);
        schreibe_atomar(&self.datei, self.text(stand.0, &stand.1).as_bytes())
    }

    /// Counts the markers in the body of one accepted request and rewrites the file. On a failure the state stays at
    /// the last written value and the request must not go out (`mw.zaehlung_fehler`).
    ///
    /// # Errors
    /// The file cannot be written.
    pub fn zaehle(&self, koerper: &[u8]) -> io::Result<()> {
        let mut stand = self.stand.lock().unwrap_or_else(PoisonError::into_inner);
        let anfragen = stand.0 + 1;
        let zahlen: Vec<u64> = self
            .marker
            .iter()
            .zip(&stand.1)
            .map(|(m, alt)| alt + vorkommen(koerper, m.as_bytes()))
            .collect();
        schreibe_atomar(&self.datei, self.text(anfragen, &zahlen).as_bytes())?;
        *stand = (anfragen, zahlen);
        Ok(())
    }

    /// Counts the markers in the payload of one whole data frame of a switched connection (section 9, Z1 and Z2): over
    /// the sequence `folge` of every data payload of that connection, so a marker spread over two frames counts once.
    /// `anfragen` stays. On a failure the state and `folge` stay at the last written value and the frame must not go
    /// out (`mw.zaehlung_fehler`).
    ///
    /// # Errors
    /// The file cannot be written.
    #[cfg(target_os = "linux")]
    pub(crate) fn rahmen(&self, folge: &mut Folge, nutzdaten: &[u8]) -> io::Result<()> {
        let mut stand = self.stand.lock().unwrap_or_else(PoisonError::into_inner);
        let mut reste = Vec::with_capacity(self.marker.len());
        let mut zahlen = Vec::with_capacity(self.marker.len());
        for ((m, alt), rest) in self.marker.iter().zip(&stand.1).zip(&folge.reste) {
            let (n, neu) = weiter(rest, nutzdaten, m.as_bytes());
            zahlen.push(alt + n);
            reste.push(neu);
        }
        schreibe_atomar(&self.datei, self.text(stand.0, &zahlen).as_bytes())?;
        stand.1 = zahlen;
        folge.reste = reste;
        Ok(())
    }

    /// The sequence of the data payloads of one switched connection, at its start.
    #[must_use]
    #[cfg(target_os = "linux")]
    pub(crate) fn folge(&self) -> Folge {
        Folge {
            reste: vec![Vec::new(); self.marker.len()],
        }
    }

    /// `{"anfragen":n,"marker":{…},"sitzung":…}` in RFC 8785 (keys in sorted order; the markers are ASCII, so the
    /// order of UTF-16 code units is the order of bytes).
    fn text(&self, anfragen: u64, zahlen: &[u64]) -> String {
        let mut paare: Vec<(&String, u64)> =
            self.marker.iter().zip(zahlen.iter().copied()).collect();
        paare.sort();
        let mut marker = Map::new();
        for (m, n) in paare {
            marker.insert(m.clone(), n.into());
        }
        let mut o = Map::new();
        o.insert("anfragen".to_owned(), anfragen.into());
        o.insert("marker".to_owned(), Value::Object(marker));
        o.insert("sitzung".to_owned(), self.sitzung.clone().into());
        Value::Object(o).to_string()
    }
}

/// What the count of one switched connection carries from one data frame to the next: per marker the bytes after
/// its last hit, at most (length of the marker - 1), so a marker that starts in them is new and counts once.
#[cfg(target_os = "linux")]
pub(crate) struct Folge {
    reste: Vec<Vec<u8>>,
}

/// How often `marker` occurs in `heu`, without overlap, from left to right.
pub(crate) fn vorkommen(heu: &[u8], marker: &[u8]) -> u64 {
    treffer(heu, marker).0
}

/// The hits of `marker` in `heu` without overlap, and where the last one ends (0 without a hit).
fn treffer(heu: &[u8], marker: &[u8]) -> (u64, usize) {
    let mut n = 0;
    let mut frei_ab = 0;
    for (i, fenster) in heu.windows(marker.len()).enumerate() {
        if i >= frei_ab && fenster == marker {
            n += 1;
            frei_ab = i + marker.len();
        }
    }
    (n, frei_ab)
}

/// The hits of `marker` in `rest` followed by `neu`, and the new rest: the bytes after the last hit, at most the last
/// (length of the marker - 1).
#[cfg(target_os = "linux")]
fn weiter(rest: &[u8], neu: &[u8], marker: &[u8]) -> (u64, Vec<u8>) {
    let mut heu = rest.to_vec();
    heu.extend_from_slice(neu);
    let (n, frei_ab) = treffer(&heu, marker);
    let ab = frei_ab.max(heu.len().saturating_sub(marker.len() - 1));
    let rest = heu[ab..].to_vec();
    heu.fill(0);
    (n, rest)
}

/// Writes `inhalt` to a new file next to `ziel` (0600) and renames it over `ziel`. The folder of `ziel` is opened once
/// (no link at its last part); the new file is made, written and renamed relative to it. On a failure the new file is
/// deleted again, but only if its name still shows the file this call made. A leftover of that name that this call did
/// not make is not touched: the write then fails.
fn schreibe_atomar(ziel: &Path, inhalt: &[u8]) -> io::Result<()> {
    let (ordner, name) = Ordner::von(ziel)?;
    let mut neu_name = name.clone();
    neu_name.push(format!(".{}.neu", std::process::id()));
    let mut datei = ordner.neu_anlegen(&neu_name, rustix::fs::Mode::from_raw_mode(0o600))?;
    let eigene = Kennung::von_datei(&datei);
    let ergebnis = datei
        .write_all(inhalt)
        .and_then(|()| datei.sync_all())
        .and_then(|()| ordner.umbenennen(&neu_name, &name));
    if ergebnis.is_err()
        && let Ok(soll) = eigene
    {
        ordner.entfernen_wenn(&neu_name, &soll);
    }
    ergebnis
}

#[cfg(test)]
mod tests {
    #[cfg(target_os = "linux")]
    use super::{Zaehlung, weiter};
    use super::{schreibe_atomar, vorkommen};

    #[test]
    fn markers_are_counted_without_overlap() {
        assert_eq!(vorkommen(b"AAAA", b"AA"), 2);
        assert_eq!(vorkommen(b"AAA", b"AA"), 1);
        assert_eq!(vorkommen(b"xMKyMK", b"MK"), 2);
        assert_eq!(vorkommen(b"", b"MK"), 0);
        assert_eq!(vorkommen(b"M", b"MK"), 0);
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn the_rest_keeps_only_what_can_start_a_marker() {
        assert_eq!(weiter(b"", b"xxMK-", b"MK-KU"), (0, b"xMK-".to_vec()));
        assert_eq!(weiter(b"xMK-", b"KUy", b"MK-KU"), (1, b"y".to_vec()));
        assert_eq!(weiter(b"", b"MK-KU", b"MK-KU"), (1, Vec::new()));
        assert_eq!(weiter(b"", b"MK-KUM", b"MK-KU"), (1, b"M".to_vec()));
        assert_eq!(weiter(b"", b"abcdefgh", b"MK-KU"), (0, b"efgh".to_vec()));
        assert_eq!(weiter(b"", b"ab", b"MK-KU"), (0, b"ab".to_vec()));
        assert_eq!(weiter(b"AB-A", b"B-AB", b"AB-AB"), (1, b"-AB".to_vec()));
        assert_eq!(weiter(b"", b"xyz", b"M"), (0, Vec::new()));
        assert_eq!(weiter(b"", b"MM", b"M"), (2, Vec::new()));
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn frames_count_over_their_borders_and_keep_the_requests() {
        let ordner = std::env::temp_dir().join(format!("mw-folge-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&ordner);
        std::fs::create_dir(&ordner).expect("folder");
        let datei = ordner.join("z.json");
        let z = Zaehlung::neu(
            vec!["MK-KU".to_owned(), "AB".to_owned()],
            datei.clone(),
            "s-1".to_owned(),
        );
        z.zaehle(b"").expect("request");
        let mut f = z.folge();
        let lies = || std::fs::read_to_string(&datei).expect("read");
        z.rahmen(&mut f, b"xMK-").expect("frame");
        assert_eq!(
            lies(),
            r#"{"anfragen":1,"marker":{"AB":0,"MK-KU":0},"sitzung":"s-1"}"#
        );
        z.rahmen(&mut f, b"KU AB A").expect("frame");
        assert_eq!(
            lies(),
            r#"{"anfragen":1,"marker":{"AB":1,"MK-KU":1},"sitzung":"s-1"}"#
        );
        z.rahmen(&mut f, b"B").expect("frame");
        assert_eq!(
            lies(),
            r#"{"anfragen":1,"marker":{"AB":2,"MK-KU":1},"sitzung":"s-1"}"#
        );
        let mut g = z.folge();
        z.rahmen(&mut g, b"KU")
            .expect("frame of another connection");
        assert_eq!(
            lies(),
            r#"{"anfragen":1,"marker":{"AB":2,"MK-KU":1},"sitzung":"s-1"}"#
        );
        z.zaehle(b"AB").expect("request");
        assert_eq!(
            lies(),
            r#"{"anfragen":2,"marker":{"AB":3,"MK-KU":1},"sitzung":"s-1"}"#
        );
        z.rahmen(&mut f, b"MK-").expect("frame");
        std::fs::remove_dir_all(&ordner).expect("remove");
        assert!(z.rahmen(&mut f, b"KU").is_err());
        std::fs::create_dir(&ordner).expect("folder");
        z.rahmen(&mut f, b"KU").expect("frame");
        assert_eq!(
            lies(),
            r#"{"anfragen":2,"marker":{"AB":3,"MK-KU":2},"sitzung":"s-1"}"#,
            "a failed write keeps the state and the rest"
        );
        std::fs::remove_dir_all(&ordner).expect("remove");
    }

    fn atomar_ordner(name: &str) -> std::path::PathBuf {
        let o = std::env::temp_dir().join(format!("mw-atomar-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&o);
        std::fs::create_dir_all(&o).expect("folder");
        o
    }

    fn neu_name(o: &std::path::Path, name: &str) -> std::path::PathBuf {
        o.join(format!("{name}.{}.neu", std::process::id()))
    }

    #[test]
    fn an_atomic_write_replaces_the_target_with_0600_and_leaves_no_temp_file() {
        use std::os::unix::fs::PermissionsExt;
        let o = atomar_ordner("ok");
        let ziel = o.join("z.json");
        std::fs::write(&ziel, "alt").expect("write");
        schreibe_atomar(&ziel, b"neu").expect("write");
        assert_eq!(std::fs::read_to_string(&ziel).expect("read"), "neu");
        let modus = std::fs::metadata(&ziel).expect("stat").permissions().mode();
        assert_eq!(modus & 0o777, 0o600);
        assert!(!neu_name(&o, "z.json").exists());
        let _ = std::fs::remove_dir_all(&o);
    }

    #[test]
    fn a_failed_rename_deletes_the_own_temp_file_and_nothing_else() {
        let o = atomar_ordner("rename");
        // A folder as target: the rename fails after the temp file is written.
        let ziel = o.join("z.json");
        std::fs::create_dir(&ziel).expect("folder");
        assert!(schreibe_atomar(&ziel, b"x").is_err());
        assert!(
            !neu_name(&o, "z.json").exists(),
            "the own temp file is gone"
        );
        assert!(ziel.is_dir());
        let _ = std::fs::remove_dir_all(&o);
    }

    #[test]
    fn a_leftover_or_a_link_under_the_temp_name_is_not_deleted_and_not_followed() {
        let o = atomar_ordner("fremd");
        let ziel = o.join("z.json");
        let neu = neu_name(&o, "z.json");
        // A leftover that this call did not make: the write fails, the leftover stays.
        std::fs::write(&neu, "rest").expect("write");
        assert!(schreibe_atomar(&ziel, b"x").is_err());
        assert_eq!(std::fs::read_to_string(&neu).expect("read"), "rest");
        assert!(!ziel.exists());
        // A link to a file outside: the link stays, the target is not written and not deleted.
        std::fs::remove_file(&neu).expect("remove");
        let aussen = o.join("aussen");
        std::fs::write(&aussen, "aussen").expect("write");
        std::os::unix::fs::symlink(&aussen, &neu).expect("link");
        assert!(schreibe_atomar(&ziel, b"x").is_err());
        assert!(std::fs::symlink_metadata(&neu).expect("link").is_symlink());
        assert_eq!(std::fs::read_to_string(&aussen).expect("read"), "aussen");
        assert!(!ziel.exists());
        let _ = std::fs::remove_dir_all(&o);
    }

    #[test]
    fn a_target_without_a_single_name_or_in_a_linked_folder_writes_nothing() {
        let o = atomar_ordner("name");
        assert!(schreibe_atomar(&o.join(".."), b"x").is_err());
        assert!(schreibe_atomar(std::path::Path::new("/"), b"x").is_err());
        let echt = o.join("echt");
        std::fs::create_dir(&echt).expect("folder");
        std::os::unix::fs::symlink(&echt, o.join("lnk")).expect("link");
        assert!(schreibe_atomar(&o.join("lnk").join("z.json"), b"x").is_err());
        assert_eq!(std::fs::read_dir(&echt).expect("dir").count(), 0);
        let _ = std::fs::remove_dir_all(&o);
    }
}
