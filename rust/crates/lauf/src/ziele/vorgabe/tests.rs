//! Tests of the default of the firm: the rules on folders and file, the reading through descriptors and the
//! intersection rule.

use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use super::*;

static ZAEHLER: AtomicU32 = AtomicU32::new(0);

fn eigen() -> u32 {
    rustix::process::geteuid().as_raw()
}

/// An own folder 0700 below the temporary folder, by its real path (no link on the way, also on macOS).
struct Ordner(PathBuf);

impl Ordner {
    fn neu() -> Ordner {
        let n = ZAEHLER.fetch_add(1, Ordering::SeqCst);
        let basis = fs::canonicalize(std::env::temp_dir()).expect("temp");
        let p = basis.join(format!("wb-vorgabe-{}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir(&p).expect("folder");
        fs::set_permissions(&p, fs::Permissions::from_mode(0o700)).expect("mode");
        Ordner(p)
    }

    fn datei(&self, name: &str, inhalt: &[u8], modus: u32) -> PathBuf {
        let p = self.0.join(name);
        fs::write(&p, inhalt).expect("write");
        fs::set_permissions(&p, fs::Permissions::from_mode(modus)).expect("mode");
        p
    }
}

impl Drop for Ordner {
    fn drop(&mut self) {
        let _ = fs::set_permissions(&self.0, fs::Permissions::from_mode(0o700));
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn lies(p: &Path) -> Result<Vec<u8>, Fehler> {
    lesen_bytes(p, Some(eigen()))
}

#[test]
fn the_delivered_build_takes_only_root_and_one_fixed_path() {
    assert_eq!(VORGABE_PFAD, "/etc/werkbank/vorgabe.json");
    if !cfg!(feature = "pruefbau") {
        assert_eq!(zweiter_eigner(), None);
        assert_eq!(pfad(), Some(PathBuf::from(VORGABE_PFAD)));
    }
}

#[test]
fn the_rule_for_the_file() {
    let r = FileType::RegularFile;
    assert!(datei_gut(r, 0, 0o644, None));
    assert!(datei_gut(r, 0, 0o600, None));
    assert!(datei_gut(r, 7, 0o644, Some(7)));
    assert!(
        !datei_gut(r, 7, 0o644, None),
        "a foreign owner without a second owner"
    );
    assert!(!datei_gut(r, 7, 0o644, Some(8)), "a foreign owner");
    assert!(!datei_gut(r, 0, 0o664, None), "the group may write");
    assert!(!datei_gut(r, 0, 0o646, None), "others may write");
    assert!(!datei_gut(r, 0, 0o620, None), "the group may write");
    assert!(!datei_gut(r, 0, 0o602, None), "others may write");
    assert!(!datei_gut(FileType::Directory, 0, 0o644, None));
    assert!(!datei_gut(FileType::Symlink, 0, 0o644, None));
    assert!(!datei_gut(FileType::Fifo, 0, 0o644, None));
}

#[test]
fn the_rule_for_a_folder() {
    let d = FileType::Directory;
    assert!(ordner_gut(d, 0, 0o755, None, true));
    assert!(ordner_gut(d, 0, 0o755, None, false));
    assert!(ordner_gut(d, 7, 0o700, Some(7), true));
    assert!(!ordner_gut(d, 7, 0o755, None, true), "a foreign owner");
    assert!(!ordner_gut(d, 7, 0o755, Some(8), false), "a foreign owner");
    assert!(!ordner_gut(d, 0, 0o775, None, false), "the group may write");
    assert!(!ordner_gut(d, 0, 0o757, None, false), "others may write");
    assert!(!ordner_gut(d, 0, 0o720, None, false), "the group may write");
    assert!(
        ordner_gut(d, 0, 0o1777, None, false),
        "sticky above the last folder, as /tmp"
    );
    assert!(
        !ordner_gut(d, 0, 0o1777, None, true),
        "the last folder may not be writable, even sticky"
    );
    assert!(
        ordner_gut(d, 0, 0o1755, None, true),
        "sticky without write is fine"
    );
    assert!(!ordner_gut(FileType::RegularFile, 0, 0o755, None, false));
    assert!(!ordner_gut(FileType::Symlink, 0, 0o755, None, false));
}

#[test]
fn a_safe_default_is_read() {
    let o = Ordner::neu();
    let p = o.datei("vorgabe.json", b"{}", 0o644);
    assert_eq!(lies(&p), Ok(b"{}".to_vec()));
    let grenze = o.datei("grenze.json", &vec![b' '; 65_536], 0o600);
    assert_eq!(lies(&grenze).map(|b| b.len()), Ok(65_536));
}

#[test]
fn a_missing_default_or_folder_is_fehlt() {
    let o = Ordner::neu();
    assert_eq!(lies(&o.0.join("fehlt.json")), Err(Fehler::VorgabeFehlt));
    assert_eq!(
        lies(&o.0.join("weg").join("vorgabe.json")),
        Err(Fehler::VorgabeFehlt)
    );
}

#[test]
fn a_foreign_owner_is_unsafe() {
    let o = Ordner::neu();
    let p = o.datei("vorgabe.json", b"{}", 0o644);
    // the own user is not the allowed second owner: the folders and the file are foreign
    assert_eq!(
        lesen_bytes(&p, Some(eigen().wrapping_add(1))),
        Err(Fehler::VorgabeUnsicher)
    );
    if eigen() != 0 {
        assert_eq!(
            lesen_bytes(&p, None),
            Err(Fehler::VorgabeUnsicher),
            "root only"
        );
    }
}

#[test]
fn a_writable_file_is_unsafe() {
    let o = Ordner::neu();
    for modus in [0o664, 0o646, 0o666, 0o622] {
        let p = o.datei("vorgabe.json", b"{}", modus);
        assert_eq!(lies(&p), Err(Fehler::VorgabeUnsicher), "{modus:o}");
    }
}

#[test]
fn a_link_is_unsafe() {
    let o = Ordner::neu();
    let ziel = o.datei("echt.json", b"{}", 0o644);
    let verweis = o.0.join("vorgabe.json");
    symlink(&ziel, &verweis).expect("link");
    assert_eq!(lies(&verweis), Err(Fehler::VorgabeUnsicher));
    // a link to a folder on the way
    fs::create_dir(o.0.join("echt")).expect("folder");
    fs::set_permissions(o.0.join("echt"), fs::Permissions::from_mode(0o700)).expect("mode");
    fs::write(o.0.join("echt").join("vorgabe.json"), b"{}").expect("write");
    fs::set_permissions(
        o.0.join("echt").join("vorgabe.json"),
        fs::Permissions::from_mode(0o644),
    )
    .expect("mode");
    symlink(o.0.join("echt"), o.0.join("ordnerverweis")).expect("link");
    assert_eq!(
        lies(&o.0.join("echt").join("vorgabe.json")),
        Ok(b"{}".to_vec())
    );
    assert_eq!(
        lies(&o.0.join("ordnerverweis").join("vorgabe.json")),
        Err(Fehler::VorgabeUnsicher)
    );
}

#[test]
fn a_writable_folder_is_unsafe() {
    let o = Ordner::neu();
    let p = o.datei("vorgabe.json", b"{}", 0o644);
    for modus in [0o770, 0o707, 0o1777, 0o720] {
        fs::set_permissions(&o.0, fs::Permissions::from_mode(modus)).expect("mode");
        assert_eq!(lies(&p), Err(Fehler::VorgabeUnsicher), "{modus:o}");
    }
    // sticky and writable above the last folder is fine (as /tmp), the last folder below it is not writable
    fs::set_permissions(&o.0, fs::Permissions::from_mode(0o1777)).expect("mode");
    let unten = o.0.join("unten");
    fs::create_dir(&unten).expect("folder");
    fs::set_permissions(&unten, fs::Permissions::from_mode(0o755)).expect("mode");
    let q = unten.join("vorgabe.json");
    fs::write(&q, b"{}").expect("write");
    fs::set_permissions(&q, fs::Permissions::from_mode(0o644)).expect("mode");
    assert_eq!(lies(&q), Ok(b"{}".to_vec()));
    // writable without sticky above the last folder is not
    fs::set_permissions(&o.0, fs::Permissions::from_mode(0o777)).expect("mode");
    assert_eq!(lies(&q), Err(Fehler::VorgabeUnsicher));
}

#[test]
fn no_regular_file_or_a_large_one_is_unsafe() {
    let o = Ordner::neu();
    fs::create_dir(o.0.join("ordner.json")).expect("folder");
    fs::set_permissions(o.0.join("ordner.json"), fs::Permissions::from_mode(0o755)).expect("mode");
    assert_eq!(lies(&o.0.join("ordner.json")), Err(Fehler::VorgabeUnsicher));
    let fifo = o.0.join("fifo.json");
    let st = std::process::Command::new("/usr/bin/mkfifo")
        .arg("-m")
        .arg("644")
        .arg(&fifo)
        .status()
        .expect("mkfifo");
    assert!(st.success());
    assert_eq!(lies(&fifo), Err(Fehler::VorgabeUnsicher));
    let gross = o.datei("gross.json", &vec![b' '; 65_537], 0o644);
    assert_eq!(lies(&gross), Err(Fehler::VorgabeUnsicher));
}

#[test]
fn a_path_that_is_not_absolute_and_plain_is_unsafe() {
    let o = Ordner::neu();
    let p = o.datei("vorgabe.json", b"{}", 0o644);
    let name = p.file_name().expect("name");
    assert_eq!(lies(Path::new(name)), Err(Fehler::VorgabeUnsicher));
    assert_eq!(
        lies(
            &o.0.join("..")
                .join(o.0.file_name().expect("name"))
                .join(name)
        ),
        Err(Fehler::VorgabeUnsicher)
    );
    assert_eq!(
        lies(&o.0.join(".").join(name)),
        Ok(b"{}".to_vec()),
        "Path drops a `.` between parts"
    );
    assert_eq!(lies(Path::new("/")), Err(Fehler::VorgabeUnsicher));
    assert_eq!(lies(Path::new("")), Err(Fehler::VorgabeUnsicher));
}

const VORGABE: &str = r#"{"fassung":2,"klassen":["a","b"],"routen":[{"schluessel":"c","art":"cloud","klasse":"a","netzziele":[{"host":"api.cloud.example","port":443,"zugriff":"kopf-einsetzen","schluessel":true,"pfade":[{"methode":"GET","pfad":"/models"},{"methode":"POST","pfad":"/chat"}]},{"host":"telemetry.cloud.example","port":443,"zugriff":"kopf-einsetzen","schluessel":false,"pfade":[{"methode":"POST","pfad":"/telemetry"}]},{"host":"*.frei.example","port":443,"zugriff":"kopf-einsetzen","schluessel":false,"pfade":[{"methode":"GET","pfad":"/x"}]}]},{"schluessel":"leer","art":"cloud","klasse":"a","netzziele":[],"modell":"m1"},{"schluessel":"l","art":"lokal","klasse":"b","ziel":"http://127.0.0.1:11434/v1","modell":"m","netzziele":[]}],"kunden":[],"kundendaten_an_cloud":{"vorgabe":"nie","freigaben":[]}}"#;

fn einst(text: &str) -> Einstellungen {
    einstellungen::lesen(text.as_bytes()).expect("valid settings")
}

/// The code of the intersection rule for the settings `nutzer` against the default `vorgabe`.
fn tabelle(vorgabe: &str, nutzer: &str) -> Result<(), Fehler> {
    pruefen(&einst(vorgabe), &einst(nutzer))
}

/// The code of the intersection rule for the user's settings made from the default by `aendern`.
fn schnitt(aendern: impl Fn(&str) -> String) -> Result<(), Fehler> {
    tabelle(VORGABE, &aendern(VORGABE))
}

fn ersetzt(alt: impl Into<String>, neu: impl Into<String>) -> impl Fn(&str) -> String {
    let (alt, neu) = (alt.into(), neu.into());
    move |t: &str| {
        assert_eq!(t.matches(alt.as_str()).count(), 1, "{alt}");
        t.replacen(alt.as_str(), &neu, 1)
    }
}

#[test]
fn the_same_table_holds() {
    assert_eq!(schnitt(str::to_owned), Ok(()));
}

#[test]
fn striking_a_target_or_a_path_holds() {
    let ohne_telemetrie = ersetzt(
        r#",{"host":"telemetry.cloud.example","port":443,"zugriff":"kopf-einsetzen","schluessel":false,"pfade":[{"methode":"POST","pfad":"/telemetry"}]}"#,
        "",
    );
    assert_eq!(schnitt(ohne_telemetrie), Ok(()));
    assert_eq!(
        schnitt(ersetzt(r#",{"methode":"POST","pfad":"/chat"}"#, "")),
        Ok(())
    );
    // striking the key binding narrows the target: it holds
    let ohne_schluessel = ersetzt(
        r#""host":"api.cloud.example","port":443,"zugriff":"kopf-einsetzen","schluessel":true"#,
        r#""host":"api.cloud.example","port":443,"zugriff":"kopf-einsetzen","schluessel":false"#,
    );
    assert_eq!(schnitt(ohne_schluessel), Ok(()));
}

#[test]
fn adding_a_target_is_refused() {
    let dazu = ersetzt(
        r#"{"host":"telemetry.cloud.example""#,
        r#"{"host":"neu.cloud.example","port":443,"zugriff":"kopf-einsetzen","schluessel":false,"pfade":[{"methode":"POST","pfad":"/telemetry"}]},{"host":"telemetry.cloud.example""#,
    );
    assert_eq!(schnitt(dazu), Err(Fehler::VorgabeErweitert));
    let an_leer = ersetzt(
        r#""schluessel":"leer","art":"cloud","klasse":"a","netzziele":[]"#,
        r#""schluessel":"leer","art":"cloud","klasse":"a","netzziele":[{"host":"api.cloud.example","port":443,"zugriff":"kopf-einsetzen","schluessel":false,"pfade":[{"methode":"GET","pfad":"/models"}]}]"#,
    );
    assert_eq!(schnitt(an_leer), Err(Fehler::VorgabeErweitert));
}

#[test]
fn widening_a_path_or_changing_a_method_is_refused() {
    let pfad = ersetzt(
        r#"{"methode":"GET","pfad":"/models"}"#,
        r#"{"methode":"GET","pfad":"/models/*"}"#,
    );
    assert_eq!(schnitt(pfad), Err(Fehler::VorgabeErweitert));
    let alles = ersetzt(
        r#"{"methode":"GET","pfad":"/models"}"#,
        r#"{"methode":"GET","pfad":"/*"}"#,
    );
    assert_eq!(schnitt(alles), Err(Fehler::VorgabeErweitert));
    let methode = ersetzt(
        r#"{"methode":"GET","pfad":"/models"}"#,
        r#"{"methode":"POST","pfad":"/models"}"#,
    );
    assert_eq!(schnitt(methode), Err(Fehler::VorgabeErweitert));
    let dazu = ersetzt(
        r#"{"methode":"POST","pfad":"/telemetry"}"#,
        r#"{"methode":"POST","pfad":"/telemetry"},{"methode":"POST","pfad":"/chat"}"#,
    );
    assert_eq!(
        schnitt(dazu),
        Err(Fehler::VorgabeErweitert),
        "a path of another target of the default"
    );
}

#[test]
fn changing_host_port_access_or_key_binding_is_refused() {
    for (alt, neu) in [
        (
            r#""host":"api.cloud.example","port":443"#,
            r#""host":"api.cloud.example","port":8443"#,
        ),
        (
            r#""host":"api.cloud.example","port":443"#,
            r#""host":"api2.cloud.example","port":443"#,
        ),
        (r#""host":"*.frei.example""#, r#""host":"a.frei.example""#),
        (r#""host":"*.frei.example""#, r#""host":"frei.example""#),
        (
            r#""host":"telemetry.cloud.example","port":443,"zugriff":"kopf-einsetzen","schluessel":false"#,
            r#""host":"telemetry.cloud.example","port":443,"zugriff":"kopf-einsetzen","schluessel":true"#,
        ),
        (
            r#"{"host":"*.frei.example","port":443,"zugriff":"kopf-einsetzen","schluessel":false,"pfade":[{"methode":"GET","pfad":"/x"}]}"#,
            r#"{"host":"*.frei.example","port":443,"zugriff":"http-lesen"}"#,
        ),
    ] {
        assert_eq!(
            schnitt(ersetzt(alt, neu)),
            Err(Fehler::VorgabeErweitert),
            "{neu}"
        );
    }
}

const LOKAL: &str = r#"{"schluessel":"l","art":"lokal","klasse":"b","ziel":"http://127.0.0.1:11434/v1","modell":"m","netzziele":[]}"#;
const LEER: &str =
    r#"{"schluessel":"leer","art":"cloud","klasse":"a","netzziele":[],"modell":"m1"}"#;

#[test]
fn striking_a_route_holds() {
    assert_eq!(schnitt(ersetzt(&format!(",{LOKAL}"), "")), Ok(()));
    assert_eq!(schnitt(ersetzt(&format!(",{LEER}"), "")), Ok(()));
}

#[test]
fn a_route_the_default_does_not_know_is_refused() {
    let fremd = r#"{"schluessel":"neu","art":"lokal","klasse":"b","ziel":"https://10.0.0.5:11434/v1","modell":"m","netzziele":[]}"#;
    assert_eq!(
        schnitt(ersetzt(LOKAL, &format!("{LOKAL},{fremd}"))),
        Err(Fehler::VorgabeRouteFremd)
    );
    // the same route under another key is foreign as well
    assert_eq!(
        schnitt(ersetzt(r#""schluessel":"leer""#, r#""schluessel":"neu""#)),
        Err(Fehler::VorgabeRouteFremd)
    );
}

#[test]
fn another_class_kind_or_local_target_is_refused() {
    for (alt, neu) in [
        // class higher and lower
        (LOKAL, LOKAL.replacen(r#""klasse":"b""#, r#""klasse":"a""#, 1)),
        (LEER, LEER.replacen(r#""klasse":"a""#, r#""klasse":"b""#, 1)),
        // kind cloud to local and local to cloud
        (
            LEER,
            r#"{"schluessel":"leer","art":"lokal","klasse":"a","ziel":"http://127.0.0.1:11434/v1","modell":"m1","netzziele":[]}"#.to_owned(),
        ),
        (LOKAL, r#"{"schluessel":"l","art":"cloud","klasse":"b","modell":"m","netzziele":[]}"#.to_owned()),
        // the local target: port, host, path, scheme
        (LOKAL, LOKAL.replacen("127.0.0.1:11434", "127.0.0.1:11435", 1)),
        (LOKAL, LOKAL.replacen("http://127.0.0.1", "https://10.0.0.5", 1)),
        (LOKAL, LOKAL.replacen("/v1", "/x/v1", 1)),
        (LOKAL, LOKAL.replacen("http://", "https://", 1)),
    ] {
        assert_eq!(
            schnitt(ersetzt(alt, neu.clone())),
            Err(Fehler::VorgabeRouteAbweichend),
            "{neu}"
        );
    }
}

#[test]
fn reordering_the_classes_is_refused() {
    // the names stay, the rank of each route changes: the class of `c` would stand higher
    assert_eq!(
        schnitt(ersetzt(r#""klassen":["a","b"]"#, r#""klassen":["b","a"]"#)),
        Err(Fehler::VorgabeRouteAbweichend)
    );
}

#[test]
fn the_model_is_free_only_where_the_default_fixes_none() {
    // the default fixes `m1` at `leer` and `m` at `l`: another one or none is refused
    assert_eq!(
        schnitt(ersetzt(r#""modell":"m1""#, r#""modell":"m2""#)),
        Err(Fehler::VorgabeRouteAbweichend)
    );
    assert_eq!(
        schnitt(ersetzt(r#","modell":"m1""#, "")),
        Err(Fehler::VorgabeRouteAbweichend)
    );
    assert_eq!(
        schnitt(ersetzt(r#""modell":"m","#, r#""modell":"n","#)),
        Err(Fehler::VorgabeRouteAbweichend)
    );
    // the default fixes no model at `c`: the user may choose one
    assert_eq!(
        schnitt(ersetzt(
            r#""schluessel":"c","art":"cloud","klasse":"a","#,
            r#""schluessel":"c","art":"cloud","klasse":"a","modell":"frei","#
        )),
        Ok(())
    );
}

#[test]
fn a_switch_that_differs_from_the_default_is_refused() {
    let an = |t: &str| {
        t.replacen(
            r#""schluessel":"c","art":"cloud","klasse":"a","#,
            r#""schluessel":"c","art":"cloud","klasse":"a","eingeschraenktes_token":true,"#,
            1,
        )
    };
    let aus = |t: &str| {
        t.replacen(
            r#""schluessel":"c","art":"cloud","klasse":"a","#,
            r#""schluessel":"c","art":"cloud","klasse":"a","eingeschraenktes_token":false,"#,
            1,
        )
    };
    assert_ne!(an(VORGABE), VORGABE);
    assert_eq!(tabelle(VORGABE, &an(VORGABE)), Err(Fehler::VorgabeSchalter));
    assert_eq!(tabelle(&an(VORGABE), VORGABE), Err(Fehler::VorgabeSchalter));
    assert_eq!(
        tabelle(&an(VORGABE), &aus(VORGABE)),
        Err(Fehler::VorgabeSchalter)
    );
    assert_eq!(tabelle(&an(VORGABE), &an(VORGABE)), Ok(()));
    assert_eq!(tabelle(VORGABE, &aus(VORGABE)), Ok(()));
}
