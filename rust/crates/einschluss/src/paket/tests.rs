//! The check of the package folder on the host, the small enclosure and its text form, and the package in the profile
//! of a session.

use std::path::{Path, PathBuf};
#[cfg(unix)]
use std::sync::atomic::{AtomicU32, Ordering};

use super::*;
use crate::beschreibung::{Beschreibung, Paket, System, Umgebung, Zustand, paket_ziel};
use crate::profil::Profil;

#[cfg(unix)]
static ZAEHLER: AtomicU32 = AtomicU32::new(0);

#[cfg(unix)]
/// A throw-away folder below `TMPDIR` (removed at the end).
struct Ordner(PathBuf);

#[cfg(unix)]
impl Ordner {
    fn neu() -> Ordner {
        let n = ZAEHLER.fetch_add(1, Ordering::SeqCst);
        let basis = std::fs::canonicalize(std::env::temp_dir()).expect("tmp");
        let p = basis.join(format!("wb-paket-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).expect("folder");
        Ordner(p)
    }
}

#[cfg(unix)]
impl Drop for Ordner {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[cfg(unix)]
fn uid() -> u32 {
    use std::os::unix::fs::MetadataExt;
    std::fs::metadata(std::env::temp_dir()).map_or(0, |_| {
        let o = Ordner::neu();
        std::fs::metadata(&o.0).expect("meta").uid()
    })
}

#[cfg(unix)]
/// A complete package folder `pkg/linux-x64/9.8.7` below `o`.
fn vollstaendig(o: &Ordner) -> PathBuf {
    let ordner = o.0.join("pkg").join("linux-x64").join("9.8.7");
    for rel in paket_pfade("linux-x64") {
        let p = ordner.join(rel);
        std::fs::create_dir_all(p.parent().expect("parent")).expect("folder");
        std::fs::write(&p, b"x").expect("file");
    }
    ordner
}

#[test]
fn the_four_paths_of_the_package_end_with_the_mark() {
    assert_eq!(
        paket_pfade("linux-arm64"),
        [
            "index.js",
            "app.js",
            "prebuilds/linux-arm64/runtime.node",
            ".extraction-complete"
        ]
    );
    assert_eq!(
        paket_ziel(Path::new("/srv/beispiel/heim"), "linux-x64", "0.0.88"),
        PathBuf::from("/srv/beispiel/heim/.cache/copilot/pkg/linux-x64/0.0.88")
    );
}

#[test]
fn a_version_has_three_numbers() {
    for gut in ["1.0.88", "0.0.88", "10.200.3000"] {
        assert!(fassung_gut(gut), "{gut}");
    }
    for schlecht in [
        "",
        "1.0",
        "1.0.88.1",
        "1.0.x",
        "1..88",
        "1.0.88-beta",
        " 1.0.88",
        "../1",
        "1.0.8/8",
        "1234567890.0.0",
    ] {
        assert!(!fassung_gut(schlecht), "{schlecht}");
    }
    assert!(paket_form_gut("linux-x64", "1.0.88"));
    assert!(paket_form_gut("linux-arm64", "0.0.88"));
    assert!(!paket_form_gut("linuxmusl-x64", "1.0.88"));
    assert!(!paket_form_gut("darwin-arm64", "1.0.88"));
    assert!(!paket_form_gut("linux-x64", "1.0"));
}

#[cfg(unix)]
#[test]
fn a_complete_own_folder_passes_the_check() {
    let o = Ordner::neu();
    let ordner = vollstaendig(&o);
    assert_eq!(paket_pruefen(&ordner, "linux-x64", uid()), Ok(()));
}

#[cfg(unix)]
#[test]
fn a_missing_folder_is_missing() {
    let o = Ordner::neu();
    assert_eq!(
        paket_pruefen(&o.0.join("pkg/linux-x64/9.8.7"), "linux-x64", uid()),
        Err(PaketMangel::Fehlt)
    );
}

#[cfg(unix)]
#[test]
fn each_of_the_four_paths_missing_makes_the_folder_incomplete() {
    for rel in paket_pfade("linux-x64") {
        let o = Ordner::neu();
        let ordner = vollstaendig(&o);
        std::fs::remove_file(ordner.join(&rel)).expect("remove");
        assert_eq!(
            paket_pruefen(&ordner, "linux-x64", uid()),
            Err(PaketMangel::Unvollstaendig),
            "{rel}"
        );
    }
    // a folder in the place of a file, and a file in the place of the folder
    let o = Ordner::neu();
    let ordner = vollstaendig(&o);
    std::fs::remove_file(ordner.join("app.js")).expect("remove");
    std::fs::create_dir(ordner.join("app.js")).expect("folder");
    assert_eq!(
        paket_pruefen(&ordner, "linux-x64", uid()),
        Err(PaketMangel::Unvollstaendig)
    );
    let o = Ordner::neu();
    let datei = o.0.join("9.8.7");
    std::fs::write(&datei, b"x").expect("file");
    assert_eq!(
        paket_pruefen(&datei, "linux-x64", uid()),
        Err(PaketMangel::Unvollstaendig)
    );
}

#[cfg(unix)]
#[test]
fn a_place_that_cannot_be_looked_at_is_not_missing() {
    // a file where a folder on the way should be: the place cannot be read, which is not the same as missing
    let o = Ordner::neu();
    std::fs::write(o.0.join("pkg"), b"x").expect("file");
    assert_eq!(
        paket_pruefen(&o.0.join("pkg/linux-x64/9.8.7"), "linux-x64", uid()),
        Err(PaketMangel::Unvollstaendig)
    );
}

#[cfg(unix)]
#[test]
fn a_dangling_link_is_a_link() {
    let o = Ordner::neu();
    std::os::unix::fs::symlink(o.0.join("nirgends"), o.0.join("9.8.7")).expect("link");
    assert_eq!(
        paket_pruefen(&o.0.join("9.8.7"), "linux-x64", uid()),
        Err(PaketMangel::Verweis)
    );
}

#[cfg(unix)]
#[test]
fn a_link_anywhere_is_refused() {
    use std::os::unix::fs::symlink;
    // the folder itself is a link
    let o = Ordner::neu();
    let echt = vollstaendig(&o);
    let verweis = o.0.join("verweis");
    symlink(&echt, &verweis).expect("link");
    assert_eq!(
        paket_pruefen(&verweis, "linux-x64", uid()),
        Err(PaketMangel::Verweis)
    );
    // a folder above it is a link
    let oben = o.0.join("oben");
    symlink(o.0.join("pkg"), &oben).expect("link");
    assert_eq!(
        paket_pruefen(&oben.join("linux-x64/9.8.7"), "linux-x64", uid()),
        Err(PaketMangel::Verweis)
    );
    // one of the four paths is a link (to a file that exists)
    for rel in paket_pfade("linux-x64") {
        let o = Ordner::neu();
        let ordner = vollstaendig(&o);
        let ziel = o.0.join("woanders");
        std::fs::write(&ziel, b"x").expect("file");
        std::fs::remove_file(ordner.join(&rel)).expect("remove");
        symlink(&ziel, ordner.join(&rel)).expect("link");
        assert_eq!(
            paket_pruefen(&ordner, "linux-x64", uid()),
            Err(PaketMangel::Verweis),
            "{rel}"
        );
    }
    // a folder on the way to a path is a link
    let o = Ordner::neu();
    let ordner = vollstaendig(&o);
    let prebuilds = ordner.join("prebuilds");
    let woanders = o.0.join("prebuilds-woanders");
    std::fs::rename(&prebuilds, &woanders).expect("move");
    symlink(&woanders, &prebuilds).expect("link");
    assert_eq!(
        paket_pruefen(&ordner, "linux-x64", uid()),
        Err(PaketMangel::Verweis)
    );
}

#[cfg(unix)]
#[test]
fn a_folder_of_another_user_is_foreign() {
    let o = Ordner::neu();
    let ordner = vollstaendig(&o);
    assert_eq!(
        paket_pruefen(&ordner, "linux-x64", uid().wrapping_add(1)),
        Err(PaketMangel::Fremd)
    );
}

fn vorentpacken_muster() -> Vorentpacken {
    Vorentpacken {
        heim: PathBuf::from("/srv/beispiel/heim"),
        cache: PathBuf::from("/srv/beispiel/heim/.cache/werkbank/harness/copilot"),
        programmordner: vec![PathBuf::from("/srv/beispiel/heim/.npm-global")],
        programm: PathBuf::from(
            "/srv/beispiel/heim/.npm-global/lib/node_modules/@github/copilot/npm-loader.js",
        ),
        path: "/usr/bin:/bin".to_owned(),
        system: System::usr_merge(1000),
    }
}

#[test]
fn the_small_enclosure_has_one_writable_bind_and_runs_the_program_with_version() {
    let v = vorentpacken_muster();
    let a = v.argumente().expect("builds");
    let n = a.iter().position(|x| x == "--").expect("separator");
    assert_eq!(
        &a[n..],
        [
            "--",
            "/srv/beispiel/heim/.npm-global/lib/node_modules/@github/copilot/npm-loader.js",
            "--version"
        ]
    );
    let kopf = &a[..n];
    let binds: Vec<(&str, &str)> = kopf
        .windows(3)
        .filter(|w| w[0] == "--bind")
        .map(|w| (w[1].as_str(), w[2].as_str()))
        .collect();
    assert_eq!(
        binds,
        [(
            "/srv/beispiel/heim/.cache/werkbank/harness/copilot",
            "/srv/beispiel/heim/.cache/copilot"
        )]
    );
    for pflicht in [
        "--unshare-user",
        "--disable-userns",
        "--unshare-pid",
        "--unshare-net",
        "--unshare-ipc",
        "--unshare-uts",
        "--unshare-cgroup",
        "--die-with-parent",
        "--clearenv",
    ] {
        assert!(kopf.iter().any(|x| x == pflicht), "{pflicht}");
    }
    let env: Vec<(&str, &str)> = kopf
        .windows(3)
        .filter(|w| w[0] == "--setenv")
        .map(|w| (w[1].as_str(), w[2].as_str()))
        .collect();
    assert_eq!(
        env,
        [
            ("HOME", "/srv/beispiel/heim"),
            ("PATH", "/usr/bin:/bin"),
            ("COPILOT_AUTO_UPDATE", "false")
        ]
    );
    // the home tmpfs comes before the bind below it, every tmpfs is 16 MiB
    let heim = kopf
        .windows(2)
        .position(|w| w[0] == "--tmpfs" && w[1] == "/srv/beispiel/heim")
        .expect("home tmpfs");
    let bind = kopf.iter().position(|x| x == "--bind").expect("bind");
    assert!(heim < bind);
    let groessen: Vec<&str> = kopf
        .windows(2)
        .filter(|w| w[0] == "--size")
        .map(|w| w[1].as_str())
        .collect();
    assert_eq!(groessen, ["16777216", "16777216", "16777216"]);
    assert!(
        !kopf
            .iter()
            .any(|x| x == "--share-net" || x == "--new-session")
    );
}

#[test]
fn the_small_enclosure_refuses_every_other_cache_folder() {
    let mut v = vorentpacken_muster();
    v.cache = PathBuf::from("/var/tmp/werkbank/copilot");
    assert!(
        matches!(v.argumente(), Err(Fehler::Verboten(_))),
        "outside the home"
    );
    v.cache = v.heim.clone();
    assert!(
        matches!(v.argumente(), Err(Fehler::Verboten(_))),
        "the home"
    );
    v.cache = PathBuf::from("/srv/beispiel/heim/.config/werkbank/harness/copilot");
    assert!(
        matches!(v.argumente(), Err(Fehler::Verboten(_))),
        "a secret segment"
    );
    v.cache = PathBuf::from("/srv/beispiel/heim/.local/share/keyrings/werkbank/harness/copilot");
    assert!(
        matches!(v.argumente(), Err(Fehler::Verboten(_))),
        "a login store"
    );
    v.cache = PathBuf::from("/srv/beispiel/heim/.cache");
    assert!(
        matches!(v.argumente(), Err(Fehler::Verboten(_))),
        "a wider folder"
    );
    v.cache = PathBuf::from("/srv/beispiel/heim/.cache/werkbank/harness/copilot2");
    assert!(
        matches!(v.argumente(), Err(Fehler::Verboten(_))),
        "another name"
    );
    v.cache = PathBuf::from("/var/tmp/werkbank/harness/copilot");
    assert!(
        matches!(v.argumente(), Err(Fehler::Verboten(_))),
        "outside the home again"
    );
    let mut v = vorentpacken_muster();
    v.programmordner = vec![PathBuf::from("/srv/beispiel/heim/.cache/werkbank")];
    assert!(
        matches!(v.argumente(), Err(Fehler::Ueberlappt(_))),
        "over a program folder"
    );
    let mut v = vorentpacken_muster();
    v.cache = PathBuf::from("/srv/beispiel/heim/.cache/../werkbank/harness/copilot");
    assert!(matches!(
        v.argumente(),
        Err(Fehler::BeschreibungUngueltig(_))
    ));
    let mut v = vorentpacken_muster();
    v.programm = PathBuf::from("npm-loader.js");
    assert!(matches!(
        v.argumente(),
        Err(Fehler::BeschreibungUngueltig(_))
    ));
    let mut v = vorentpacken_muster();
    v.path = "/usr/bin\n".to_owned();
    assert!(matches!(
        v.argumente(),
        Err(Fehler::BeschreibungUngueltig(_))
    ));
}

#[test]
fn the_text_form_of_the_small_enclosure_round_trips_and_is_closed() {
    let v = vorentpacken_muster();
    let text = vorentpacken_schreiben(&v);
    assert_eq!(
        vorentpacken_lesen(text.as_bytes(), System::usr_merge(1000)),
        Some(v.clone())
    );
    let mehr = text.replacen('{', "{\"argv\":[\"/bin/sh\"],", 1);
    assert_eq!(
        vorentpacken_lesen(mehr.as_bytes(), System::usr_merge(1000)),
        None
    );
    let weniger = text.replacen("\"path\":\"/usr/bin:/bin\"", "\"x\":1", 1);
    assert_eq!(
        vorentpacken_lesen(weniger.as_bytes(), System::usr_merge(1000)),
        None
    );
    assert_eq!(vorentpacken_lesen(b"", System::usr_merge(1000)), None);
    // valid text padded with spaces: at the limit it is read, one byte more is refused
    let mut grenze = text.clone().into_bytes();
    grenze.resize(EINGABE_MAX, b' ');
    assert_eq!(
        vorentpacken_lesen(&grenze, System::usr_merge(1000)),
        Some(v.clone())
    );
    grenze.push(b' ');
    assert_eq!(vorentpacken_lesen(&grenze, System::usr_merge(1000)), None);
    // two program folders keep their order and their separator
    let mut zwei = v;
    zwei.programmordner
        .push(PathBuf::from("/srv/beispiel/programme"));
    let text = vorentpacken_schreiben(&zwei);
    assert!(
        text.contains(
            "\"programmordner\":[\"/srv/beispiel/heim/.npm-global\",\"/srv/beispiel/programme\"]"
        ),
        "{text}"
    );
    assert_eq!(
        vorentpacken_lesen(text.as_bytes(), System::usr_merge(1000)),
        Some(zwei)
    );
}

fn sitzung_muster() -> Beschreibung {
    Beschreibung {
        harness: "copilot".to_owned(),
        arbeitsordner: PathBuf::from("/srv/beispiel/arbeit"),
        programmordner: vec![PathBuf::from("/srv/beispiel/heim/.npm-global")],
        zustand: Zustand {
            host: PathBuf::from("/var/tmp/z/s-1"),
            name: ".copilot".to_owned(),
            fortsetzen: false,
            gespraech: None,
        },
        heim: PathBuf::from("/srv/beispiel/heim"),
        such_socket: PathBuf::from("/var/tmp/k/s-1/such.sock"),
        modell_socket: Some(PathBuf::from("/var/tmp/k/s-1/modell.sock")),
        lokal_weiter: None,
        cloud: None,
        paket: Some(Paket {
            quelle: PathBuf::from(
                "/srv/beispiel/heim/.cache/werkbank/harness/copilot/pkg/linux-x64/0.0.88",
            ),
            plattform: "linux-x64".to_owned(),
            fassung: "0.0.88".to_owned(),
        }),
        dateien: Vec::new(),
        umgebung: Umgebung {
            home: "/srv/beispiel/heim".to_owned(),
            path: "/usr/bin".to_owned(),
            term: "xterm".to_owned(),
            lang: "C.UTF-8".to_owned(),
            tmpdir: "/tmp".to_owned(),
        },
        geschuetzt: vec![PathBuf::from("/var/lib/wb")],
        argv: vec!["/srv/beispiel/heim/.npm-global/bin/copilot".to_owned()],
        system: System::usr_merge(1000),
    }
}

#[test]
fn the_profile_binds_the_package_read_only_at_the_place_of_the_cli() {
    let p = Profil::bauen(&sitzung_muster()).expect("builds");
    let a = p.argumente();
    let quelle = "/srv/beispiel/heim/.cache/werkbank/harness/copilot/pkg/linux-x64/0.0.88";
    let stelle = a
        .windows(3)
        .position(|w| w[0] == "--ro-bind" && w[1] == quelle)
        .expect("bind of the package");
    assert_eq!(
        a[stelle + 2],
        "/srv/beispiel/heim/.cache/copilot/pkg/linux-x64/0.0.88"
    );
    assert!(!a.windows(2).any(|w| w[0] == "--bind" && w[1] == quelle));
    let heim = a
        .windows(2)
        .position(|w| w[0] == "--tmpfs" && w[1] == "/srv/beispiel/heim")
        .expect("home");
    assert!(heim < stelle, "the bind comes after the home tmpfs");
    // without a package there is no bind below .cache
    let mut ohne = sitzung_muster();
    ohne.paket = None;
    let a = Profil::bauen(&ohne).expect("builds").argumente();
    assert!(!a.iter().any(|x| x.contains("/.cache/")), "{a:?}");
}

#[test]
fn the_profile_refuses_a_package_of_another_form_or_place() {
    let mit = |f: &dyn Fn(&mut Beschreibung)| {
        let mut b = sitzung_muster();
        f(&mut b);
        Profil::bauen(&b)
    };
    assert!(matches!(
        mit(&|b| b.harness = "pi".to_owned()),
        Err(Fehler::BeschreibungUngueltig(_))
    ));
    assert!(matches!(
        mit(&|b| b.paket.as_mut().expect("p").plattform = "linuxmusl-x64".to_owned()),
        Err(Fehler::BeschreibungUngueltig(_))
    ));
    assert!(matches!(
        mit(&|b| b.paket.as_mut().expect("p").fassung = "../../x".to_owned()),
        Err(Fehler::BeschreibungUngueltig(_))
    ));
    assert!(matches!(
        mit(&|b| b.paket.as_mut().expect("p").quelle = PathBuf::from("pkg/x")),
        Err(Fehler::BeschreibungUngueltig(_))
    ));
    assert!(matches!(
        mit(&|b| b.paket.as_mut().expect("p").quelle = PathBuf::from("/srv/beispiel/heim/.ssh/p")),
        Err(Fehler::Verboten(_))
    ));
    assert!(matches!(
        mit(&|b| b.paket.as_mut().expect("p").quelle = PathBuf::from("/srv/beispiel/arbeit/pkg")),
        Err(Fehler::Ueberlappt(_))
    ));
    assert!(matches!(
        mit(&|b| b.paket.as_mut().expect("p").quelle = PathBuf::from("/var/tmp/z/s-1/pkg")),
        Err(Fehler::Ueberlappt(_))
    ));
    assert!(matches!(
        mit(&|b| b.paket.as_mut().expect("p").quelle = PathBuf::from("/var/lib/wb/pkg")),
        Err(Fehler::Ueberlappt(_))
    ));
    assert!(matches!(
        mit(&|b| b.paket.as_mut().expect("p").quelle =
            PathBuf::from("/srv/beispiel/heim/.npm-global/pkg")),
        Err(Fehler::Ueberlappt(_))
    ));
    assert!(matches!(
        mit(&|b| b.arbeitsordner = PathBuf::from("/srv/beispiel/heim/.cache/copilot")),
        Err(Fehler::Ueberlappt(_))
    ));
    assert!(matches!(
        mit(&|b| b.zustand.name = ".cache".to_owned()),
        Err(Fehler::Ueberlappt(_))
    ));
    assert!(Profil::bauen(&sitzung_muster()).is_ok(), "control");
}

#[test]
fn the_package_round_trips_in_the_text_form() {
    let b = sitzung_muster();
    let text = crate::beschreibung_schreiben(&b);
    assert!(
        text.contains("\"paket\":{\"quelle\":\"/srv/beispiel/heim/.cache/werkbank/harness/copilot/pkg/linux-x64/0.0.88\",\"plattform\":\"linux-x64\",\"fassung\":\"0.0.88\"}"),
        "{text}"
    );
    assert_eq!(
        crate::beschreibung_lesen(text.as_bytes(), System::usr_merge(1000)),
        Some(b.clone())
    );
    let mut ohne = b;
    ohne.paket = None;
    let text = crate::beschreibung_schreiben(&ohne);
    assert!(text.contains("\"paket\":null"), "{text}");
    assert_eq!(
        crate::beschreibung_lesen(text.as_bytes(), System::usr_merge(1000)),
        Some(ohne)
    );
    // the field is required, and closed
    let fehlt = text.replacen("\"paket\":null,", "", 1);
    assert_eq!(
        crate::beschreibung_lesen(fehlt.as_bytes(), System::usr_merge(1000)),
        None
    );
    let offen = text.replacen(
        "\"paket\":null",
        "\"paket\":{\"quelle\":\"/a\",\"plattform\":\"linux-x64\",\"fassung\":\"1.0.0\",\"x\":1}",
        1,
    );
    assert_eq!(
        crate::beschreibung_lesen(offen.as_bytes(), System::usr_merge(1000)),
        None
    );
}

#[test]
fn the_arch_pattern_is_not_the_ubuntu_layout() {
    // the pattern of the pure tests links /lib64 to usr/lib (Arch); Ubuntu 26.04 links it to usr/lib64 and /sbin to
    // usr/sbin (measured in the throw-away VM), which a real enclosure takes from the host
    let arch = System::usr_merge(1000);
    assert!(
        arch.symlinks
            .contains(&("usr/lib".to_owned(), "/lib64".to_owned()))
    );
    let ubuntu = System {
        uid: 1000,
        symlinks: vec![
            ("usr/bin".to_owned(), "/bin".to_owned()),
            ("usr/sbin".to_owned(), "/sbin".to_owned()),
            ("usr/lib".to_owned(), "/lib".to_owned()),
            ("usr/lib64".to_owned(), "/lib64".to_owned()),
        ],
        ro_binds: arch.ro_binds.clone(),
    };
    let mut b = sitzung_muster();
    b.system = ubuntu;
    let a = Profil::bauen(&b).expect("builds").argumente();
    for (text, ort) in [("usr/sbin", "/sbin"), ("usr/lib64", "/lib64")] {
        assert!(
            a.windows(3)
                .any(|w| w[0] == "--symlink" && w[1] == text && w[2] == ort),
            "{text} {ort}"
        );
    }
    let mut v = vorentpacken_muster();
    v.system = b.system.clone();
    let a = v.argumente().expect("builds");
    assert!(
        a.windows(3)
            .any(|w| w[0] == "--symlink" && w[1] == "usr/lib64" && w[2] == "/lib64")
    );
}

/// The small enclosure exists only on Linux: elsewhere the host check answers `NichtVerfuegbar` before it looks at anything.
#[cfg(target_os = "linux")]
#[test]
fn the_host_check_of_the_small_enclosure_needs_bwrap_an_own_cache_and_real_program_folders() {
    use crate::vorpruefung::vorentpacken_pruefen;
    let o = Ordner::neu();
    let heim = o.0.join("heim");
    let cache = heim.join(".cache/werkbank/harness/copilot");
    let programme = o.0.join("programme");
    std::fs::create_dir_all(&cache).expect("cache");
    std::fs::create_dir_all(&programme).expect("programs");
    let bwrap = o.0.join("bwrap");
    std::fs::write(&bwrap, "#!/bin/sh\n").expect("bwrap");
    std::fs::set_permissions(&bwrap, std::os::unix::fs::PermissionsExt::from_mode(0o755))
        .expect("mode");
    let v = Vorentpacken {
        heim: heim.clone(),
        cache: cache.clone(),
        programmordner: vec![programme.clone()],
        programm: programme.join("copilot"),
        path: "/usr/bin".to_owned(),
        system: System::usr_merge(uid()),
    };
    assert_eq!(vorentpacken_pruefen(&v, &bwrap), Ok(()));
    assert_eq!(
        vorentpacken_pruefen(&v, &o.0.join("fehlt")),
        Err(Fehler::BwrapFehlt)
    );
    let mut fremd = v.clone();
    fremd.system.uid = uid().wrapping_add(1);
    assert!(matches!(
        vorentpacken_pruefen(&fremd, &bwrap),
        Err(Fehler::QuelleUnbrauchbar(_))
    ));
    let mut kein_ordner = v.clone();
    kein_ordner.programmordner = vec![bwrap.clone()];
    assert!(matches!(
        vorentpacken_pruefen(&kein_ordner, &bwrap),
        Err(Fehler::QuelleUnbrauchbar(_))
    ));
    let mut fehlt = v;
    fehlt.cache = heim.join("fehlt/werkbank/harness/copilot");
    assert!(matches!(
        vorentpacken_pruefen(&fehlt, &bwrap),
        Err(Fehler::QuelleUnbrauchbar(_))
    ));
}

#[cfg(all(unix, not(target_os = "linux")))]
#[test]
fn the_host_check_of_the_small_enclosure_is_not_available_outside_linux() {
    use crate::vorpruefung::vorentpacken_pruefen;
    let o = Ordner::neu();
    let v = Vorentpacken {
        heim: o.0.join("heim"),
        cache: o.0.join("heim/.cache/werkbank/harness/copilot"),
        programmordner: vec![o.0.join("programme")],
        programm: o.0.join("programme/copilot"),
        path: "/usr/bin".to_owned(),
        system: System::usr_merge(uid()),
    };
    assert_eq!(
        vorentpacken_pruefen(&v, &o.0.join("bwrap")),
        Err(Fehler::NichtVerfuegbar)
    );
}
