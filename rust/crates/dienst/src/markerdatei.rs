//! The marker file of a trial run (contract `dienst`, DI12 point 14).
//!
//! `lauf.marker_datei` of the start file names a file that `werkbank-probelauf` makes only for a trial run. The service reads it exactly once,
//! at its start (after the start file and the rights of the socket folder, before the socket), and never again: what the file says at that
//! moment is the state of the run, also when it changes or disappears later. From the file comes only the list of the markers (`{"liste": [...]}`);
//! the folder of the count comes from `lauf.marker_ordner` of the start file, which only root writes. The markers are used for nothing but
//! `Startanfrage::mit_sitzung` (they decide on no check, route, class or target).
//!
//! SECURITY: the file lies in a folder that the user owns, so it is read like a claim of the user. The folder is opened without following a
//! link (`O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC`) and must be owned by the effective user with the mode exactly 0700; the file is opened
//! relative to that descriptor with `O_NOFOLLOW`, must be a regular file of the effective user with the mode exactly 0600, at most 4096 bytes, UTF-8
//! and JSON with exactly the key `liste`. A missing file or a missing folder is the normal case (no markers). Every other result is exit
//! code 2 with the line `lauf.marker_datei ungueltig: <fixed reason>`; the line holds neither the path nor the content.

use std::ffi::CString;
use std::fs::File;
use std::io::Read;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

use rustix::fs::{CWD, Mode, openat};
use rustix::io::Errno;
use serde_json::Value;

use crate::fehler::StartFehler;
use crate::loeschen::{ORDNER_OFFEN, name_gut};

/// The most bytes of the file.
pub(crate) const DATEI_MAX: usize = 4096;

/// A folder of the count that `start::Marker::neu` accepts: it stands in while only the list is checked.
const GUTER_ORDNER: &str = "/zaehlung";

/// The flags to open the file without following a link. `O_NONBLOCK`: opening a FIFO for reading would wait for a writer, and a process of the user
/// could hold the start of the service that way; with the flag the FIFO opens at once and `fstat` refuses it (`datei_art`). On a regular file it has no effect.
/// (The folder is opened with `O_DIRECTORY`: a FIFO there is `ENOTDIR`, no wait.)
const DATEI_OFFEN: rustix::fs::OFlags = rustix::fs::OFlags::RDONLY
    .union(rustix::fs::OFlags::NOFOLLOW)
    .union(rustix::fs::OFlags::CLOEXEC)
    .union(rustix::fs::OFlags::NONBLOCK);

fn ungueltig(grund: &str) -> StartFehler {
    StartFehler::neu(2, &format!("lauf.marker_datei ungueltig: {grund}"))
}

/// Reads the markers once. `ordner` is `lauf.marker_ordner`. `Ok(None)`: no file (or no folder), so no markers.
///
/// # Errors
/// Exit code 2 for every file that is not as described above, and for a valid list without a usable `lauf.marker_ordner`.
pub(crate) fn lesen(
    pfad: &str,
    ordner: Option<&str>,
) -> Result<Option<start::Marker>, StartFehler> {
    let euid = rustix::process::geteuid().as_raw();
    lesen_mit(pfad, ordner, euid, euid)
}

/// [`lesen`] with the owners to compare with: `euid_ordner` for the folder and `euid_datei` for the file (a test can name another user for either).
pub(crate) fn lesen_mit(
    pfad: &str,
    ordner: Option<&str>,
    euid_ordner: u32,
    euid_datei: u32,
) -> Result<Option<start::Marker>, StartFehler> {
    let p = Path::new(pfad);
    let (Some(eltern), Some(name)) = (p.parent(), p.file_name().and_then(|n| n.to_str())) else {
        return Err(ungueltig("pfad"));
    };
    if !name_gut(name) {
        return Err(ungueltig("pfad"));
    }
    // 1. the folder: a missing folder is no error, every other failure is
    let eltern_fd = match openat(CWD, eltern, ORDNER_OFFEN, Mode::empty()) {
        Ok(fd) => File::from(fd),
        Err(Errno::NOENT) => return Ok(None),
        Err(_) => return Err(ungueltig("ordner")),
    };
    let meta = eltern_fd.metadata().map_err(|_| ungueltig("ordner"))?;
    if meta.uid() != euid_ordner {
        return Err(ungueltig("ordner_eigentuemer"));
    }
    if meta.mode() & 0o7777 != 0o700 {
        return Err(ungueltig("ordner_modus"));
    }
    // 2. the file relative to the folder: a missing file is no error
    let cname = CString::new(name).map_err(|_| ungueltig("pfad"))?;
    let datei = match openat(&eltern_fd, &cname, DATEI_OFFEN, Mode::empty()) {
        Ok(fd) => File::from(fd),
        Err(Errno::NOENT) => return Ok(None),
        Err(Errno::LOOP) => return Err(ungueltig("datei_verknuepfung")),
        Err(_) => return Err(ungueltig("datei_unlesbar")),
    };
    let meta = datei.metadata().map_err(|_| ungueltig("datei_unlesbar"))?;
    if !meta.is_file() {
        return Err(ungueltig("datei_art"));
    }
    if meta.uid() != euid_datei {
        return Err(ungueltig("datei_eigentuemer"));
    }
    if meta.mode() & 0o7777 != 0o600 {
        return Err(ungueltig("datei_modus"));
    }
    let mut roh = Vec::new();
    datei
        .take(DATEI_MAX as u64 + 1)
        .read_to_end(&mut roh)
        .map_err(|_| ungueltig("datei_unlesbar"))?;
    if roh.len() > DATEI_MAX {
        return Err(ungueltig("zu_gross"));
    }
    // 3. the content: UTF-8, JSON, exactly the key `liste`
    let text = std::str::from_utf8(&roh).map_err(|_| ungueltig("kein_utf8"))?;
    let wert: Value = serde_json::from_str(text).map_err(|_| ungueltig("kein_json"))?;
    let liste = liste_lesen(&wert).ok_or_else(|| ungueltig("form"))?;
    // the list is checked by the crate `start` alone: first with a folder that is good, then with the folder of the start file
    if start::Marker::neu(liste.clone(), GUTER_ORDNER).is_err() {
        return Err(ungueltig("form"));
    }
    let Some(ordner) = ordner else {
        return Err(StartFehler::neu(
            2,
            "lauf.marker_ordner fehlt, die Marker-Datei braucht ihn",
        ));
    };
    start::Marker::neu(liste, ordner)
        .map(Some)
        .map_err(|_| StartFehler::neu(2, "lauf.marker_ordner ungueltig"))
}

/// The list of the object `{"liste": [...]}`: exactly that key, an array of texts.
fn liste_lesen(wert: &Value) -> Option<Vec<String>> {
    let karte = wert.as_object()?;
    if karte.len() != 1 {
        return None;
    }
    karte
        .get("liste")?
        .as_array()?
        .iter()
        .map(|m| m.as_str().map(str::to_owned))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::{PermissionsExt, symlink};
    use std::path::PathBuf;

    struct Probe(PathBuf);

    impl Probe {
        fn neu(name: &str) -> Self {
            let p = std::env::temp_dir().join(format!("dm-{}-{name}", std::process::id()));
            let _ = fs::remove_dir_all(&p);
            fs::create_dir_all(&p).expect("dir");
            Self(p)
        }

        /// A folder `mk` (0700) with the file `marker.json` (0600) holding `inhalt`.
        fn datei(&self, inhalt: &str) -> String {
            let mk = self.0.join("mk");
            fs::create_dir_all(&mk).expect("mk");
            fs::set_permissions(&mk, fs::Permissions::from_mode(0o700)).expect("mode");
            let f = mk.join("marker.json");
            fs::write(&f, inhalt).expect("write");
            fs::set_permissions(&f, fs::Permissions::from_mode(0o600)).expect("mode");
            f.to_string_lossy().into_owned()
        }
    }

    impl Drop for Probe {
        fn drop(&mut self) {
            let _ = fs::set_permissions(self.0.join("mk"), fs::Permissions::from_mode(0o700));
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn euid() -> u32 {
        rustix::process::geteuid().as_raw()
    }

    const ZAEHL: &str = "/var/lib/zaehlung";

    fn grund(r: &Result<Option<start::Marker>, StartFehler>) -> String {
        let e = r.as_ref().expect_err("an error");
        assert_eq!(e.code, 2);
        e.to_string()
    }

    fn lesen_gut(pfad: &str) -> Result<Option<start::Marker>, StartFehler> {
        lesen_mit(pfad, Some(ZAEHL), euid(), euid())
    }

    #[test]
    fn eine_gute_datei_gibt_die_marker_mit_dem_ordner_der_startdatei() {
        let p = Probe::neu("gut");
        let f = p.datei(r#"{"liste": ["MP-2C5", "AB"]}"#);
        let m = lesen_gut(&f).expect("ok").expect("markers");
        assert_eq!(m.liste(), ["MP-2C5", "AB"]);
        assert_eq!(m.ordner(), ZAEHL);
        // `lesen` uses the effective user for both
        assert_eq!(lesen(&f, Some(ZAEHL)).expect("ok"), Some(m));
    }

    #[test]
    fn eine_fehlende_datei_oder_ein_fehlender_ordner_ist_kein_fehler() {
        let p = Probe::neu("fehlt");
        let f = p.datei("{}");
        fs::remove_file(&f).expect("rm");
        assert_eq!(lesen_gut(&f), Ok(None));
        let ohne_ordner = p.0.join("gibt-es-nicht/marker.json");
        assert_eq!(lesen_gut(&ohne_ordner.to_string_lossy()), Ok(None));
        // a missing file with a start file that has no folder of the count is no error either
        assert_eq!(lesen_mit(&f, None, euid(), euid()), Ok(None));
    }

    #[test]
    fn eine_verknuepfung_als_datei_oder_ordner_wird_nicht_gefolgt() {
        let p = Probe::neu("link");
        let f = p.datei(r#"{"liste": ["AA"]}"#);
        let echt = p.0.join("mk");
        // the file is a link to a good file
        let ziel = p.0.join("ziel.json");
        fs::write(&ziel, r#"{"liste": ["AA"]}"#).expect("write");
        fs::set_permissions(&ziel, fs::Permissions::from_mode(0o600)).expect("mode");
        fs::remove_file(&f).expect("rm");
        symlink(&ziel, &f).expect("link");
        assert!(grund(&lesen_gut(&f)).ends_with("datei_verknuepfung"));
        // the folder is a link to a good folder
        fs::remove_file(&f).expect("rm");
        fs::write(&f, r#"{"liste": ["AA"]}"#).expect("write");
        fs::set_permissions(&f, fs::Permissions::from_mode(0o600)).expect("mode");
        let link = p.0.join("mk-link");
        symlink(&echt, &link).expect("link");
        assert!(grund(&lesen_gut(&link.join("marker.json").to_string_lossy())).ends_with("ordner"));
    }

    #[test]
    fn modus_und_eigentuemer_von_ordner_und_datei_zaehlen() {
        let p = Probe::neu("rechte");
        let f = p.datei(r#"{"liste": ["AA"]}"#);
        let mk = p.0.join("mk");
        for modus in [0o644, 0o640, 0o400, 0o666, 0o700, 0o601] {
            fs::set_permissions(&f, fs::Permissions::from_mode(modus)).expect("mode");
            assert!(grund(&lesen_gut(&f)).ends_with("datei_modus"), "{modus:o}");
        }
        fs::set_permissions(&f, fs::Permissions::from_mode(0o600)).expect("mode");
        for modus in [0o755, 0o750, 0o770, 0o500, 0o701, 0o540] {
            fs::set_permissions(&mk, fs::Permissions::from_mode(modus)).expect("mode");
            assert!(grund(&lesen_gut(&f)).ends_with("ordner_modus"), "{modus:o}");
        }
        fs::set_permissions(&mk, fs::Permissions::from_mode(0o700)).expect("mode");
        assert!(lesen_gut(&f).is_ok());
        // another user as the owner of the folder, then of the file
        let e = lesen_mit(&f, Some(ZAEHL), euid().wrapping_add(1), euid());
        assert!(grund(&e).ends_with("ordner_eigentuemer"));
        let e = lesen_mit(&f, Some(ZAEHL), euid(), euid().wrapping_add(1));
        assert!(grund(&e).ends_with("datei_eigentuemer"));
    }

    #[test]
    fn eine_datei_ist_hoechstens_4096_byte_gross() {
        let p = Probe::neu("gross");
        let kern = r#"{"liste": ["AA"]}"#;
        let f = p.datei(&format!("{kern}{}", " ".repeat(DATEI_MAX - kern.len())));
        assert_eq!(fs::metadata(&f).expect("meta").len(), 4096);
        assert!(lesen_gut(&f).expect("4096 go").is_some());
        let f = p.datei(&format!("{kern}{}", " ".repeat(DATEI_MAX + 1 - kern.len())));
        assert!(grund(&lesen_gut(&f)).ends_with("zu_gross"));
        let f = p.datei(&format!("{kern}{}", " ".repeat(10_000 - kern.len())));
        assert!(grund(&lesen_gut(&f)).ends_with("zu_gross"));
    }

    #[test]
    fn eine_datei_ist_eine_regulaere_datei() {
        let p = Probe::neu("art");
        let f = p.datei("{}");
        fs::remove_file(&f).expect("rm");
        fs::create_dir(&f).expect("dir");
        fs::set_permissions(&f, fs::Permissions::from_mode(0o600)).expect("mode");
        assert!(grund(&lesen_gut(&f)).ends_with("datei_art"));
    }

    #[test]
    fn eine_fifo_an_der_stelle_der_datei_haelt_den_start_nicht_an() {
        use std::sync::mpsc;
        use std::time::Duration;
        let probe = Probe::neu("fifo");
        let datei = probe.datei("{}");
        fs::remove_file(&datei).expect("rm");
        // `mkfifo` exists under Linux and macOS (`rustix::fs::mknodat` does not exist under macOS)
        let status = std::process::Command::new("mkfifo")
            .arg(&datei)
            .status()
            .expect("mkfifo runs");
        assert!(status.success());
        // a time limit: without `O_NONBLOCK` the open waits for a writer and the thread never answers
        let (sender, empfaenger) = mpsc::channel();
        let pfad = datei.clone();
        std::thread::spawn(move || {
            let _ = sender.send(lesen_gut(&pfad));
        });
        let antwort = empfaenger
            .recv_timeout(Duration::from_secs(10))
            .expect("the read of a FIFO must not wait");
        assert!(grund(&antwort).ends_with("datei_art"));
    }

    #[test]
    fn form_und_inhalt() {
        let p = Probe::neu("form");
        let sechzehn: Vec<String> = (0..16).map(|i| format!("M{i}")).collect();
        let f = p.datei(&json_text(&serde_json::json!({"liste": sechzehn})));
        assert_eq!(lesen_gut(&f).expect("16").expect("m").liste().len(), 16);
        let siebzehn: Vec<String> = (0..17).map(|i| format!("M{i}")).collect();
        let schlecht: [(&str, &str); 18] = [
            ("", "kein_json"),
            ("nein", "kein_json"),
            ("{\"liste\": [", "kein_json"),
            ("[]", "form"),
            ("\"AA\"", "form"),
            ("null", "form"),
            ("{}", "form"),
            (r#"{"liste": "AA"}"#, "form"),
            (r#"{"liste": [1]}"#, "form"),
            (r#"{"liste": []}"#, "form"),
            (r#"{"liste": ["ab"]}"#, "form"),
            (r#"{"liste": ["A,B"]}"#, "form"),
            (r#"{"liste": ["AA", "AA"]}"#, "form"),
            (r#"{"liste": [""]}"#, "form"),
            (r#"{"liste": ["AA"], "extra": 1}"#, "form"),
            (r#"{"liste": ["AA"], "ordner": "/z"}"#, "form"),
            (r#"{"ordner": "/z"}"#, "form"),
            ("{\"liste\": [\"A\u{e4}\"]}", "form"),
        ];
        for (inhalt, g) in schlecht {
            let f = p.datei(inhalt);
            assert!(grund(&lesen_gut(&f)).ends_with(g), "{inhalt:?}");
        }
        let f = p.datei(&json_text(&serde_json::json!({"liste": siebzehn})));
        assert!(grund(&lesen_gut(&f)).ends_with("form"));
        let f = p.datei(&json_text(&serde_json::json!({"liste": ["A".repeat(33)]})));
        assert!(grund(&lesen_gut(&f)).ends_with("form"));
        let f = p.datei(&json_text(&serde_json::json!({"liste": ["A".repeat(32)]})));
        assert!(lesen_gut(&f).is_ok());
        // not UTF-8
        let f = p.datei("");
        fs::write(&f, [b'{', b'"', 0xff, b'}']).expect("write");
        assert!(grund(&lesen_gut(&f)).ends_with("kein_utf8"));
        // the line holds neither the path nor the content
        let f = p.datei("geheim");
        let z = grund(&lesen_gut(&f));
        assert!(!z.contains("geheim") && !z.contains("marker.json"), "{z}");
    }

    #[test]
    fn der_zaehlordner_kommt_nur_aus_der_startdatei() {
        let p = Probe::neu("ordner");
        let f = p.datei(r#"{"liste": ["AA"]}"#);
        // a valid list without a folder of the count: exit code 2
        let e = lesen_mit(&f, None, euid(), euid());
        assert!(grund(&e).contains("lauf.marker_ordner fehlt"));
        // a folder that `start::Marker::neu` refuses: exit code 2
        for schlecht in ["relativ", "/a//b", "/a/", "/a/../b", "/a/./b", ""] {
            let e = lesen_mit(&f, Some(schlecht), euid(), euid());
            assert!(
                grund(&e).contains("lauf.marker_ordner ungueltig"),
                "{schlecht:?}"
            );
        }
        // a missing file gives no markers, also without a folder of the count
        assert_eq!(
            lesen_mit("/nirgends-xyz/marker.json", None, euid(), euid()),
            Ok(None)
        );
    }

    #[test]
    fn schlechte_pfade() {
        for p in ["/", "marker.json", "/a/.."] {
            let e = lesen_gut(p);
            assert!(e.is_err() || e == Ok(None), "{p}");
        }
    }

    fn json_text(v: &Value) -> String {
        v.to_string()
    }
}
