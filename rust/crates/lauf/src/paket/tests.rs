//! Step 6a without an enclosure: the form of the packages, the cache folder, the lock, the renaming aside and the check
//! after an extraction (the extraction itself is a closure here; the outside tests run it in `bwrap`).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};

use super::*;

static ZAEHLER: AtomicU32 = AtomicU32::new(0);

struct Ordner(PathBuf);

impl Ordner {
    fn neu() -> Ordner {
        let n = ZAEHLER.fetch_add(1, Ordering::SeqCst);
        let basis = std::fs::canonicalize(std::env::temp_dir()).expect("tmp");
        let p = basis.join(format!("wb-lpaket-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).expect("folder");
        Ordner(p)
    }
}

impl Drop for Ordner {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn schreiben(p: &Path, text: &str) {
    std::fs::create_dir_all(p.parent().expect("parent")).expect("folder");
    std::fs::write(p, text).expect("write");
}

fn haupt_json(fassung: &str) -> String {
    format!(
        "{{\"name\":\"@github/copilot\",\"version\":\"{fassung}\",\"type\":\"module\",\"bin\":{{\"copilot\":\"npm-loader.js\"}},\"optionalDependencies\":{{\"@github/copilot-linux-x64\":\"{fassung}\"}}}}"
    )
}

fn plattform_json(fassung: &str, arch: &str) -> String {
    format!(
        "{{\"name\":\"@github/copilot-linux-{arch}\",\"version\":\"{fassung}\",\"type\":\"module\",\"os\":[\"linux\"],\"cpu\":[\"{arch}\"],\"libc\":[\"glibc\"],\"bin\":{{\"copilot-linux-{arch}\":\"copilot\"}},\"exports\":{{\".\":\"./copilot\"}}}}"
    )
}

/// An npm prefix below `o` with the main package and the platform package at place 1 (nested) or 2 (beside). Returns
/// the program (`npm-loader.js`) and the folder of the platform package.
fn praefix(o: &Ordner, ort: u8, fassung: &str, arch: &str) -> (PathBuf, PathBuf) {
    let a = o.0.join("pfx/lib/node_modules/@github/copilot");
    schreiben(&a.join("package.json"), &haupt_json(fassung));
    schreiben(&a.join("npm-loader.js"), "#!/usr/bin/env node\n");
    let p = if ort == 1 {
        a.join(format!("node_modules/@github/copilot-linux-{arch}"))
    } else {
        o.0.join(format!("pfx/lib/node_modules/@github/copilot-linux-{arch}"))
    };
    schreiben(&p.join("package.json"), &plattform_json(fassung, arch));
    schreiben(&p.join("copilot"), "sea");
    (a.join("npm-loader.js"), p)
}

#[test]
fn the_architecture_comes_from_the_machine_and_needs_glibc() {
    assert_eq!(architektur("x86_64", true), Some("x64"));
    assert_eq!(architektur("aarch64", true), Some("arm64"));
    assert_eq!(architektur("x86_64", false), None, "musl");
    assert_eq!(architektur("riscv64", true), None);
    assert_eq!(architektur("x86", true), None);
}

#[test]
fn the_form_is_read_at_both_places_with_the_version_of_the_package() {
    for (ort, fassung) in [(1, "1.0.88"), (2, "1.0.88"), (1, "0.0.88"), (2, "3.14.159")] {
        let o = Ordner::neu();
        let (programm, _) = praefix(&o, ort, fassung, "x64");
        assert_eq!(
            form_lesen(&programm, Some("x64")),
            Ok(Form {
                plattform: "linux-x64".to_owned(),
                fassung: fassung.to_owned()
            }),
            "place {ort} {fassung}"
        );
    }
    let o = Ordner::neu();
    let (programm, _) = praefix(&o, 1, "2.0.1", "arm64");
    assert_eq!(
        form_lesen(&programm, Some("arm64")),
        Ok(Form {
            plattform: "linux-arm64".to_owned(),
            fassung: "2.0.1".to_owned()
        })
    );
}

#[test]
fn place_1_comes_before_place_2() {
    let o = Ordner::neu();
    let (programm, beside) = praefix(&o, 2, "1.0.88", "x64");
    // a nested package of another version wins over the one beside: the version then differs from the main package
    let nested = programm
        .parent()
        .expect("a")
        .join("node_modules/@github/copilot-linux-x64");
    schreiben(
        &nested.join("package.json"),
        &plattform_json("1.0.87", "x64"),
    );
    schreiben(&nested.join("copilot"), "sea");
    assert_eq!(form_lesen(&programm, Some("x64")), Err(Fehler::PaketForm));
    std::fs::remove_dir_all(&nested).expect("remove");
    assert!(form_lesen(&programm, Some("x64")).is_ok());
    assert!(beside.is_dir());
}

#[test]
fn the_program_may_be_reached_through_the_link_of_the_prefix() {
    let o = Ordner::neu();
    let (programm, _) = praefix(&o, 1, "1.0.88", "x64");
    let bin = o.0.join("pfx/bin");
    std::fs::create_dir_all(&bin).expect("bin");
    std::os::unix::fs::symlink(&programm, bin.join("copilot")).expect("link");
    assert_eq!(
        form_lesen(&bin.join("copilot"), Some("x64")).map(|f| f.fassung),
        Ok("1.0.88".to_owned())
    );
}

fn ersetzen(datei: &Path, alt: &str, neu: &str) {
    let t = std::fs::read_to_string(datei).expect("read");
    assert!(t.contains(alt), "{alt}");
    std::fs::write(datei, t.replace(alt, neu)).expect("write");
}

fn haupt(a: &Path, alt: &str, neu: &str) {
    ersetzen(&a.join("package.json"), alt, neu);
}

fn als_verweis(p: &Path, name: &str) {
    let echt = p.join(format!("{name}.echt"));
    std::fs::rename(p.join(name), &echt).expect("mv");
    std::os::unix::fs::symlink(&echt, p.join(name)).expect("link");
}

/// One spoiled form per case: (name, change of the main folder `a` and of the platform folder `p`).
type Verderben = fn(&Path, &Path);

const FALSCHE_FORMEN: [(&str, Verderben); 15] = [
    ("main name", |a, _| {
        haupt(a, "@github/copilot\"", "@github/andere\"");
    }),
    ("main version form", |a, _| {
        haupt(a, "\"version\":\"1.0.88\"", "\"version\":\"1.0\"");
    }),
    ("main bin", |a, _| {
        haupt(
            a,
            "\"copilot\":\"npm-loader.js\"",
            "\"copilot\":\"index.js\"",
        );
    }),
    ("main bin is text", |a, _| {
        haupt(a, "{\"copilot\":\"npm-loader.js\"}", "\"npm-loader.js\"");
    }),
    ("main not json", |a, _| {
        std::fs::write(a.join("package.json"), "{").expect("w");
    }),
    ("main missing", |a, _| {
        std::fs::remove_file(a.join("package.json")).expect("rm");
    }),
    ("platform name", |_, p| {
        ersetzen(
            &p.join("package.json"),
            "copilot-linux-x64\",\"version",
            "copilot-linux-arm64\",\"version",
        );
    }),
    ("platform version", |_, p| {
        ersetzen(
            &p.join("package.json"),
            "\"version\":\"1.0.88\"",
            "\"version\":\"1.0.87\"",
        );
    }),
    ("platform os", |_, p| {
        ersetzen(
            &p.join("package.json"),
            "\"os\":[\"linux\"]",
            "\"os\":[\"darwin\"]",
        );
    }),
    ("platform cpu", |_, p| {
        ersetzen(
            &p.join("package.json"),
            "\"cpu\":[\"x64\"]",
            "\"cpu\":[\"arm64\"]",
        );
    }),
    ("platform bin", |_, p| {
        ersetzen(
            &p.join("package.json"),
            "\"copilot-linux-x64\":\"copilot\"",
            "\"copilot-linux-x64\":\"x\"",
        );
    }),
    ("platform program missing", |_, p| {
        std::fs::remove_file(p.join("copilot")).expect("rm");
    }),
    ("platform program is a link", |_, p| {
        als_verweis(p, "copilot");
    }),
    ("platform package missing", |_, p| {
        std::fs::remove_dir_all(p).expect("rm");
    }),
    ("platform package.json is a link", |_, p| {
        als_verweis(p, "package.json");
    }),
];

#[test]
fn every_other_form_is_the_fixed_code() {
    for (name, verderben) in FALSCHE_FORMEN {
        let o = Ordner::neu();
        let (programm, p) = praefix(&o, 1, "1.0.88", "x64");
        assert!(
            form_lesen(&programm, Some("x64")).is_ok(),
            "{name}: control"
        );
        verderben(programm.parent().expect("a"), &p);
        assert_eq!(
            form_lesen(&programm, Some("x64")),
            Err(Fehler::PaketForm),
            "{name}"
        );
    }
    // the right form with an unknown machine, and a program that does not exist
    let o = Ordner::neu();
    let (programm, _) = praefix(&o, 1, "1.0.88", "x64");
    assert_eq!(form_lesen(&programm, None), Err(Fehler::PaketForm));
    assert_eq!(form_lesen(&programm, Some("arm64")), Err(Fehler::PaketForm));
    assert_eq!(
        form_lesen(&o.0.join("fehlt"), Some("x64")),
        Err(Fehler::PaketForm)
    );
}

#[test]
fn place_2_needs_the_scope_folder() {
    let o = Ordner::neu();
    let a = o.0.join("irgendwo/copilot");
    assert_eq!(
        plattform_orte(&a, "linux-x64"),
        [a.join("node_modules/@github/copilot-linux-x64")]
    );
    let a = o.0.join("lib/node_modules/@github/copilot");
    assert_eq!(
        plattform_orte(&a, "linux-x64"),
        [
            a.join("node_modules/@github/copilot-linux-x64"),
            o.0.join("lib/node_modules/@github/copilot-linux-x64")
        ]
    );
}

#[test]
fn the_cache_folder_lies_in_the_home() {
    let heim = Path::new("/srv/beispiel/heim");
    let soll = PathBuf::from("/srv/beispiel/heim/.cache/werkbank/harness/copilot");
    assert_eq!(cache_ordner(heim, None), soll);
    let x = |t: &str| Some(OsString::from(t));
    assert_eq!(
        cache_ordner(heim, x("/srv/beispiel/heim/.xdg").as_ref()),
        PathBuf::from("/srv/beispiel/heim/.xdg/werkbank/harness/copilot")
    );
    for fremd in [
        "/var/tmp",
        "relativ",
        "/srv/beispiel/heim",
        "/srv/beispiel/heim/../x",
        "/srv/beispiel/heimx",
        "",
    ] {
        assert_eq!(cache_ordner(heim, x(fremd).as_ref()), soll, "{fremd}");
    }
}

/// A closure that extracts like the CLI: the four paths, the mark last.
fn entpacker(ordner: PathBuf, zaehler: &AtomicUsize) -> impl FnOnce() -> Result<Ende, Fehler> + '_ {
    move || {
        zaehler.fetch_add(1, Ordering::SeqCst);
        for rel in einschluss::paket_pfade("linux-x64") {
            schreiben(&ordner.join(rel), "x");
        }
        Ok(Ende {
            rc: Some(0),
            kein_platz: false,
            starter: false,
        })
    }
}

fn form() -> Form {
    Form {
        plattform: "linux-x64".to_owned(),
        fassung: "0.0.88".to_owned(),
    }
}

fn lage(o: &Ordner) -> (PathBuf, PathBuf, PathBuf) {
    let heim = o.0.join("heim");
    std::fs::create_dir_all(&heim).expect("home");
    let cache = cache_ordner(&heim, None);
    let ordner = cache.join("pkg/linux-x64/0.0.88");
    (heim, cache, ordner)
}

fn modus(p: &Path) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(p).expect("meta").permissions().mode() & 0o7777
}

#[test]
fn a_missing_folder_is_extracted_and_a_complete_one_is_not() {
    let o = Ordner::neu();
    let (heim, cache, ordner) = lage(&o);
    let n = AtomicUsize::new(0);
    let (q, tat) =
        bereitstellen(&cache, &heim, &form(), entpacker(ordner.clone(), &n)).expect("ready");
    assert_eq!(
        (q.as_path(), tat, n.load(Ordering::SeqCst)),
        (ordner.as_path(), Tat::Entpackt, 1)
    );
    assert_eq!(modus(&cache), 0o700);
    assert!(cache.join(SPERR_DATEI).is_file());
    assert_eq!(modus(&cache.join(SPERR_DATEI)), 0o600);
    let (q, tat) =
        bereitstellen(&cache, &heim, &form(), entpacker(ordner.clone(), &n)).expect("ready");
    assert_eq!(
        (q, tat, n.load(Ordering::SeqCst)),
        (ordner, Tat::Vorhanden, 1),
        "no second extraction"
    );
}

#[test]
fn an_incomplete_folder_is_renamed_aside_and_extracted_again() {
    for rel in einschluss::paket_pfade("linux-x64") {
        let o = Ordner::neu();
        let (heim, cache, ordner) = lage(&o);
        let n = AtomicUsize::new(0);
        bereitstellen(&cache, &heim, &form(), entpacker(ordner.clone(), &n)).expect("first");
        std::fs::remove_file(ordner.join(&rel)).expect("remove");
        std::fs::write(ordner.join("rest.txt"), "alt").expect("rest");
        let (_, tat) =
            bereitstellen(&cache, &heim, &form(), entpacker(ordner.clone(), &n)).expect("again");
        let Tat::Erneuert(name) = tat else {
            panic!("{rel}: {tat:?}");
        };
        assert!(name.starts_with("linux-x64-0.0.88-"), "{name}");
        let beiseite = cache.join(BEISEITE).join(&name);
        assert_eq!(
            std::fs::read_to_string(beiseite.join("rest.txt"))
                .ok()
                .as_deref(),
            Some("alt"),
            "{rel}: kept, not removed"
        );
        assert!(!ordner.join("rest.txt").exists(), "{rel}");
        assert_eq!(
            einschluss::paket_pruefen(&ordner, "linux-x64", rustix::process::geteuid().as_raw()),
            Ok(())
        );
        assert_eq!(n.load(Ordering::SeqCst), 2, "{rel}");
        assert_eq!(modus(&cache.join(BEISEITE)), 0o700);
    }
}

#[test]
fn a_link_or_a_foreign_owner_is_never_bound() {
    use std::os::unix::fs::symlink;
    let n = AtomicUsize::new(0);
    // the package folder is a link to a complete folder
    let o = Ordner::neu();
    let (heim, cache, ordner) = lage(&o);
    let echt = o.0.join("echt");
    for rel in einschluss::paket_pfade("linux-x64") {
        schreiben(&echt.join(rel), "x");
    }
    drop(cache_bereit(&cache, &heim).expect("cache"));
    std::fs::create_dir_all(ordner.parent().expect("p")).expect("folder");
    symlink(&echt, &ordner).expect("link");
    assert_eq!(
        bereitstellen(&cache, &heim, &form(), entpacker(ordner.clone(), &n)),
        Err(Fehler::PaketCache("verweis"))
    );
    // the cache folder itself is a link
    let o = Ordner::neu();
    let (heim, cache, _) = lage(&o);
    std::fs::create_dir_all(o.0.join("woanders")).expect("folder");
    std::fs::create_dir_all(cache.parent().expect("p")).expect("folder");
    symlink(o.0.join("woanders"), &cache).expect("link");
    assert_eq!(
        bereitstellen(&cache, &heim, &form(), entpacker(PathBuf::new(), &n)),
        Err(Fehler::PaketCache("verweis"))
    );
    // a folder of the cache with another mode
    let o = Ordner::neu();
    let (heim, cache, _) = lage(&o);
    std::fs::create_dir_all(&cache).expect("folder");
    std::fs::set_permissions(&cache, std::os::unix::fs::PermissionsExt::from_mode(0o755))
        .expect("mode");
    assert_eq!(
        bereitstellen(&cache, &heim, &form(), entpacker(PathBuf::new(), &n)),
        Err(Fehler::PaketCache("modus"))
    );
    // a cache in a login store
    let o = Ordner::neu();
    let heim = o.0.join("heim");
    let cache = heim.join(".ssh/werkbank/harness/copilot");
    assert_eq!(
        bereitstellen(&cache, &heim, &form(), entpacker(PathBuf::new(), &n)),
        Err(Fehler::PaketCache("anmeldespeicher"))
    );
    assert!(!cache.exists(), "nothing made");
    assert_eq!(n.load(Ordering::SeqCst), 0, "no extraction");
    // a foreign owner: the check of the folder with another user (root cannot be had here; the rule is the same
    // function the step calls)
    let o = Ordner::neu();
    let (heim, cache, ordner) = lage(&o);
    bereitstellen(&cache, &heim, &form(), entpacker(ordner.clone(), &n)).expect("ready");
    let fremd = rustix::process::geteuid().as_raw().wrapping_add(1);
    assert_eq!(
        einschluss::paket_pruefen(&ordner, "linux-x64", fremd),
        Err(einschluss::PaketMangel::Fremd)
    );
}

#[test]
fn a_failed_extraction_is_the_fixed_code_with_its_word() {
    let o = Ordner::neu();
    let (heim, cache, ordner) = lage(&o);
    let ende = |rc, kein_platz, starter| {
        move || {
            Ok(Ende {
                rc,
                kein_platz,
                starter,
            })
        }
    };
    assert_eq!(
        bereitstellen(&cache, &heim, &form(), ende(Some(1), true, false)),
        Err(Fehler::PaketEntpacken {
            rc: Some(1),
            wort: "kein-platz"
        })
    );
    assert_eq!(
        bereitstellen(&cache, &heim, &form(), ende(Some(5), false, true)),
        Err(Fehler::PaketEntpacken {
            rc: Some(5),
            wort: "starter"
        })
    );
    assert_eq!(
        bereitstellen(&cache, &heim, &form(), ende(None, false, false)),
        Err(Fehler::PaketEntpacken {
            rc: None,
            wort: "rc"
        })
    );
    // exit 0, but the four paths are not there
    assert_eq!(
        bereitstellen(&cache, &heim, &form(), ende(Some(0), false, false)),
        Err(Fehler::PaketEntpacken {
            rc: Some(0),
            wort: "unvollstaendig"
        })
    );
    assert!(!ordner.exists());
    assert_eq!(
        bereitstellen(&cache, &heim, &form(), || Err(Fehler::PaketEntpacken {
            rc: None,
            wort: "frist"
        })),
        Err(Fehler::PaketEntpacken {
            rc: None,
            wort: "frist"
        })
    );
}

/// Linux only, like step 6a in the product: elsewhere step 6 ends with `EinschlussFehlt` first (`ablauf.rs`). On macOS two
/// threads that create the lock file at once through `openat` on a directory descriptor get `ENOENT` in about 4 of 10
/// rounds (measured 23.11.2026 on APFS; through the full path never), which this test would report as `PaketCache("sperre")`.
#[cfg(target_os = "linux")]
#[test]
fn two_starts_at_once_extract_once() {
    let o = Ordner::neu();
    let (heim, cache, ordner) = lage(&o);
    let n = AtomicUsize::new(0);
    let ergebnisse: Vec<Result<(PathBuf, Tat), Fehler>> = std::thread::scope(|s| {
        let griffe: Vec<_> = (0..2)
            .map(|_| {
                s.spawn(|| {
                    bereitstellen(&cache, &heim, &form(), || {
                        n.fetch_add(1, Ordering::SeqCst);
                        // the extraction takes a while: the other start waits on the lock meanwhile
                        std::thread::sleep(Duration::from_millis(400));
                        for rel in einschluss::paket_pfade("linux-x64") {
                            schreiben(&ordner.join(rel), "x");
                        }
                        Ok(Ende {
                            rc: Some(0),
                            kein_platz: false,
                            starter: false,
                        })
                    })
                })
            })
            .collect();
        griffe
            .into_iter()
            .map(|g| g.join().expect("thread"))
            .collect()
    });
    assert_eq!(n.load(Ordering::SeqCst), 1, "{ergebnisse:?}");
    let mut taten: Vec<Tat> = ergebnisse
        .into_iter()
        .map(|e| e.expect("ready").1)
        .collect();
    taten.sort_by_key(|t| format!("{t:?}"));
    assert_eq!(taten, [Tat::Entpackt, Tat::Vorhanden]);
}

#[test]
fn the_lock_gives_up_at_its_limit() {
    let o = Ordner::neu();
    let (heim, cache, _) = lage(&o);
    let fd = cache_bereit(&cache, &heim).expect("cache");
    let gehalten = sperren(&fd, Duration::from_millis(10)).expect("first");
    let zweites = cache_bereit(&cache, &heim).expect("cache");
    let start = Instant::now();
    assert_eq!(
        sperren(&zweites, Duration::from_millis(200)).err(),
        Some(Fehler::PaketEntpacken {
            rc: None,
            wort: "sperre-frist"
        })
    );
    assert!(start.elapsed() >= Duration::from_millis(200));
    drop(gehalten);
    assert!(sperren(&zweites, Duration::from_millis(200)).is_ok());
}

#[test]
fn the_measurement_of_an_extraction_names_the_cause_and_ends_a_hang() {
    let lauf = |skript: &str, frist_ms: u64| {
        let mut c = std::process::Command::new("/bin/sh");
        c.args(["-c", skript]).env_clear();
        lauf_messen(c, Duration::from_millis(frist_ms))
    };
    assert_eq!(
        lauf("exit 0", 5000),
        Ok(Ende {
            rc: Some(0),
            kein_platz: false,
            starter: false
        })
    );
    assert_eq!(
        lauf(
            "echo 'TAR_ENTRY_ERROR(ENOSPC): no space' >&2; echo 'Failed to extract bundled package' >&2; exit 1",
            5000
        ),
        Ok(Ende {
            rc: Some(1),
            kein_platz: true,
            starter: false
        })
    );
    assert_eq!(
        lauf(
            "echo 'lauf.einschluss_fehlt: Einschluss nicht verfuegbar' >&2; exit 5",
            5000
        ),
        Ok(Ende {
            rc: Some(5),
            kein_platz: false,
            starter: true
        })
    );
    // much output does not block the child (the pipe is read while it runs)
    assert_eq!(
        lauf(
            "i=0; while [ $i -lt 3000 ]; do echo 'ENOSPC line of some length to fill the pipe quickly' >&2; i=$((i+1)); done; exit 1",
            10000
        ),
        Ok(Ende {
            rc: Some(1),
            kein_platz: true,
            starter: false
        })
    );
    // only the end of standard error counts: a word far before the last 64 KiB is not looked at
    assert_eq!(
        lauf(
            "echo ENOSPC >&2; head -c 200000 /dev/zero | tr '\\0' a >&2; exit 1",
            10000
        ),
        Ok(Ende {
            rc: Some(1),
            kein_platz: false,
            starter: false
        })
    );
    let start = Instant::now();
    assert_eq!(
        lauf("exec sleep 20", 300),
        Err(Fehler::PaketEntpacken {
            rc: None,
            wort: "frist"
        })
    );
    assert!(start.elapsed() < Duration::from_secs(5));
}

#[test]
fn a_package_json_of_one_mib_is_read_and_one_byte_more_is_not() {
    let o = Ordner::neu();
    let (programm, _) = praefix(&o, 1, "1.0.88", "x64");
    let datei = programm.parent().expect("a").join("package.json");
    let kurz = haupt_json("1.0.88");
    // the closing brace is the last byte: a file cut by one byte is no JSON
    let rahmen = kurz.len() - 1 + ",\"pad\":\"\"}".len();
    let max = usize::try_from(PAKET_JSON_MAX).expect("size");
    let genau = format!(
        "{},\"pad\":\"{}\"}}",
        &kurz[..kurz.len() - 1],
        "x".repeat(max - rahmen)
    );
    assert_eq!(genau.len(), max);
    std::fs::write(&datei, &genau).expect("write");
    assert!(form_lesen(&programm, Some("x64")).is_ok(), "exactly 1 MiB");
    std::fs::write(&datei, format!("{genau} ")).expect("write");
    assert_eq!(
        form_lesen(&programm, Some("x64")),
        Err(Fehler::PaketForm),
        "one byte more"
    );
}

#[test]
fn the_renaming_aside_opens_only_real_folders_and_never_follows_a_link() {
    use std::os::unix::fs::symlink;
    let o = Ordner::neu();
    let (heim, cache, _) = lage(&o);
    let fd = cache_bereit(&cache, &heim).expect("cache");
    // pkg is a link to a folder that holds the version: nothing is renamed
    let woanders = o.0.join("woanders");
    std::fs::create_dir_all(woanders.join("linux-x64/0.0.88")).expect("folder");
    symlink(&woanders, cache.join("pkg")).expect("link");
    assert_eq!(beiseite(&fd, &form()), Err(Fehler::PaketCache("beiseite")));
    assert!(woanders.join("linux-x64/0.0.88").is_dir(), "untouched");
    // pkg is a file
    std::fs::remove_file(cache.join("pkg")).expect("remove");
    std::fs::write(cache.join("pkg"), "x").expect("file");
    assert_eq!(beiseite(&fd, &form()), Err(Fehler::PaketCache("beiseite")));
    // a real folder: opened with close-on-exec, the version is renamed
    std::fs::remove_file(cache.join("pkg")).expect("remove");
    std::fs::create_dir_all(cache.join("pkg/linux-x64/0.0.88")).expect("folder");
    let pkg = ordner_at(&fd, "pkg").expect("pkg");
    let flags = rustix::io::fcntl_getfd(&pkg).expect("flags");
    assert!(flags.contains(rustix::io::FdFlags::CLOEXEC));
    let name = beiseite(&fd, &form()).expect("aside");
    assert!(cache.join(BEISEITE).join(name).is_dir());
    assert!(!cache.join("pkg/linux-x64/0.0.88").exists());
}

#[test]
fn the_cache_folder_refuses_a_secret_segment_and_is_opened_with_close_on_exec() {
    let o = Ordner::neu();
    let heim = o.0.join("heim");
    // a secret segment that is no login store
    let cache = heim.join(".config/werkbank/harness/copilot");
    assert_eq!(
        cache_bereit(&cache, &heim).err(),
        Some(Fehler::PaketCache("anmeldespeicher"))
    );
    assert!(!cache.exists(), "nothing made");
    let (heim, cache, _) = lage(&o);
    let fd = cache_bereit(&cache, &heim).expect("cache");
    let flags = rustix::io::fcntl_getfd(&fd).expect("flags");
    assert!(flags.contains(rustix::io::FdFlags::CLOEXEC));
}

/// Another folder of the own user with mode 0700 and one entry: what a swapped link would lead to.
fn woanders(o: &Ordner) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let w = o.0.join("woanders");
    std::fs::create_dir_all(w.join("pkg")).expect("folder");
    std::fs::set_permissions(&w, std::fs::Permissions::from_mode(0o700)).expect("mode");
    schreiben(&w.join("pkg/inhalt"), "x");
    w
}

fn inhalt_von(w: &Path) -> (Vec<String>, u32) {
    let mut namen: Vec<String> = std::fs::read_dir(w.join("pkg"))
        .expect("read")
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .collect();
    namen.sort();
    (namen, modus(w))
}

#[test]
fn a_swap_of_the_cache_folder_after_the_check_is_refused_by_the_open() {
    use std::os::unix::fs::symlink;
    // the folder becomes a link to another folder of the own user (right mode, right owner): only O_NOFOLLOW stops it
    let o = Ordner::neu();
    let (heim, cache, _) = lage(&o);
    let w = woanders(&o);
    let vorher = inhalt_von(&w);
    let ergebnis = cache_bereit_mit(&cache, &heim, || {
        std::fs::remove_dir(&cache).expect("remove");
        symlink(&w, &cache).expect("link");
    });
    assert_eq!(ergebnis.err(), Some(Fehler::PaketCache("verweis")));
    assert_eq!(
        inhalt_von(&w),
        vorher,
        "nothing outside the cache is touched"
    );
    // the folder becomes a file of the own user: only O_DIRECTORY gives the word of the swap, not a later check
    let o = Ordner::neu();
    let (heim, cache, _) = lage(&o);
    let ergebnis = cache_bereit_mit(&cache, &heim, || {
        std::fs::remove_dir(&cache).expect("remove");
        std::fs::write(&cache, "x").expect("file");
    });
    assert_eq!(ergebnis.err(), Some(Fehler::PaketCache("verweis")));
    assert_eq!(
        std::fs::read(&cache).expect("read"),
        b"x",
        "the file stands"
    );
    // without a swap the seam changes nothing
    let o = Ordner::neu();
    let (heim, cache, _) = lage(&o);
    let mut gerufen = 0;
    cache_bereit_mit(&cache, &heim, || gerufen += 1).expect("cache");
    assert_eq!(gerufen, 1, "the seam runs once");
}

#[test]
fn a_swap_of_a_folder_on_the_way_after_the_lookup_is_refused_by_the_openat() {
    use std::os::unix::fs::symlink;
    let o = Ordner::neu();
    let (heim, cache, _) = lage(&o);
    let fd = cache_bereit(&cache, &heim).expect("cache");
    let w = woanders(&o);
    let vorher = inhalt_von(&w);
    // pkg is a real folder at the lookup and a link to another folder at the open
    std::fs::create_dir_all(cache.join("pkg")).expect("folder");
    let ergebnis = ordner_at_mit(&fd, "pkg", || {
        std::fs::remove_dir(cache.join("pkg")).expect("remove");
        symlink(&w, cache.join("pkg")).expect("link");
    });
    assert_eq!(ergebnis.err(), Some(Fehler::PaketCache("beiseite")));
    assert_eq!(
        inhalt_von(&w),
        vorher,
        "nothing outside the cache is touched"
    );
    // pkg is a file of the own user at the open: the descriptor of a file is never taken for a folder
    std::fs::remove_file(cache.join("pkg")).expect("remove");
    std::fs::create_dir_all(cache.join("pkg")).expect("folder");
    let ergebnis = ordner_at_mit(&fd, "pkg", || {
        std::fs::remove_dir(cache.join("pkg")).expect("remove");
        std::fs::write(cache.join("pkg"), "x").expect("file");
    });
    assert_eq!(ergebnis.err(), Some(Fehler::PaketCache("beiseite")));
    assert_eq!(std::fs::read(cache.join("pkg")).expect("read"), b"x");
}
