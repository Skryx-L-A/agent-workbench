//! Outside tests of the cloud exit on Linux, with the stand-in harness and a stand-in relay (feature `attrappe`).
//! Each case judges the answer lines of the stand-in; the test calls it with the real profile (it must hold) and with
//! profiles that lack exactly one boundary (each must fail).
//!
//! The stand-in relay outside swaps the fixed placeholder of the token for a mark and counts. The real key, the real
//! cloud harness and the real network are never used.
#![cfg(all(target_os = "linux", feature = "attrappe"))]

mod gemeinsam;

use std::fs;

use einschluss::{COPILOT_PLATZHALTER, WEITER_PORT};
use gemeinsam::{BUNDLE_MARKE, CA_MARKE, TOKEN_MARKE, Welt, davor, ersetzt};

type Urteil = Result<(), String>;

fn gleich(was: &str, ist: &str, soll: &str) -> Urteil {
    if ist == soll {
        Ok(())
    } else {
        Err(format!("{was}: got {ist:?}, wanted {soll:?}"))
    }
}

fn beginnt(was: &str, ist: &str, anfang: &str) -> Urteil {
    if ist.starts_with(anfang) {
        Ok(())
    } else {
        Err(format!("{was}: got {ist:?}, wanted a start of {anfang:?}"))
    }
}

fn s(p: &std::path::Path) -> String {
    p.to_string_lossy().into_owned()
}

fn uid() -> u32 {
    einschluss::System::vom_host().expect("host").uid
}

// ---------------------------------------------------------------------------------------------------------------
// E4 cloud: the cloud harness gets no key, no config with a token, no bus, no login of `gh`; it reaches the outside
// only through the forwarder, and sees the bundle of the session at the place of the system bundle

/// The names of the variables of the cloud harness (the sorted list of the probe `umgebung`).
const UMGEBUNG_CLOUD: &str = "umgebung: OK COPILOT_AUTO_UPDATE,COPILOT_GITHUB_TOKEN,COPILOT_MASCOT_BLINK,HOME,HTTPS_PROXY,LANG,PATH,PWD,TERM,TMPDIR";

fn bundle_orte() -> Vec<&'static str> {
    einschluss::Verteilung::vom_host()
        .expect("a known distribution")
        .bundle_orte()
        .to_vec()
}

fn nutzer_ordner() -> String {
    format!("/run/user/{}", uid())
}

fn fall_e4(welt: &Welt, args: &[String]) -> Urteil {
    let teile = welt.cloud_teile();
    let p = |probe: &[&str]| welt.probe(args, probe);
    // the request of the harness goes through the forwarder to the relay, which swaps the placeholder
    let laenge = format!("TOKEN {TOKEN_MARKE}").len();
    gleich(
        "ask through the proxy",
        &p(&["cloud"]),
        &format!("cloud: OK tausch=1 laenge={laenge}"),
    )?;
    // nothing of the key, of the config, of the login or of the key of the authority is inside
    gleich(
        "the mark in the environment",
        &p(&["markeumgebung", TOKEN_MARKE]),
        "markeumgebung: OK 0",
    )?;
    for nadel in [TOKEN_MARKE, CA_MARKE] {
        let treffer = p(&["suche", nadel, "/"]);
        if !treffer.ends_with(" treffer=0 md=0") {
            return Err(format!("search for {nadel}: {treffer}"));
        }
    }
    let bus = format!("{}/bus", nutzer_ordner());
    gleich(
        "ask the bus place",
        &p(&["frage", &bus, "{\"op\":\"ping\"}"]),
        "frage: ERR ENOENT",
    )?;
    gleich(
        "list the bus folder",
        &p(&["liste", &nutzer_ordner()]),
        "liste: OK",
    )?;
    gleich(
        "ask the bus at its host path",
        &p(&["frage", &s(&teile.bus.socket), "{\"op\":\"ping\"}"]),
        "frage: ERR ENOENT",
    )?;
    let heim = welt.heim_text();
    for (was, pfad) in [
        (
            "the config file of the cloud harness",
            format!("{heim}/.copilot/config.json"),
        ),
        (
            "a config below the home",
            format!("{heim}/.config/copilot/config.json"),
        ),
        ("the login of gh", format!("{heim}/.config/gh/hosts.yml")),
        (
            "the config at its host path",
            s(&teile.konfig.join("config.json")),
        ),
        ("the login at its host path", s(&teile.gh.join("hosts.yml"))),
        ("the key of the authority", s(&teile.ca_schluessel)),
    ] {
        gleich(was, &p(&["lesen", &pfad, TOKEN_MARKE]), "lesen: ERR ENOENT")?;
    }
    gleich(
        "list the folder of the authority",
        &p(&["liste", &s(&teile.ca_ordner)]),
        "liste: ERR ENOENT",
    )?;
    fall_e4_umgebung(welt, args)?;
    fall_e4_netz(welt, args)
}

fn fall_e4_umgebung(welt: &Welt, args: &[String]) -> Urteil {
    let p = |probe: &[&str]| welt.probe(args, probe);
    let pe = |probe: &[&str], env: &[(&str, &str)]| welt.probe_env(args, probe, env);
    // the environment: exactly the list, also when the caller sets the variables of the host
    gleich(
        "variables of the cloud harness",
        &pe(
            &["umgebung"],
            &[
                ("DBUS_SESSION_BUS_ADDRESS", "unix:path=/run/user/1/bus"),
                ("XDG_RUNTIME_DIR", "/run/user/1"),
                ("SSL_CERT_FILE", "/x.pem"),
                ("NODE_EXTRA_CA_CERTS", "/x.pem"),
                ("HTTPS_PROXY", "http://127.0.0.1:1"),
                ("COPILOT_GITHUB_TOKEN", TOKEN_MARKE),
                ("GITHUB_TOKEN", TOKEN_MARKE),
                ("GH_TOKEN", TOKEN_MARKE),
            ],
        ),
        UMGEBUNG_CLOUD,
    )?;
    gleich(
        "the mark of a caller in the environment",
        &pe(
            &["markeumgebung", TOKEN_MARKE],
            &[
                ("GH_TOKEN", TOKEN_MARKE),
                ("COPILOT_GITHUB_TOKEN", TOKEN_MARKE),
            ],
        ),
        "markeumgebung: OK 0",
    )?;
    // the bundle of the session stands at every place of the system bundle, read-only
    for ort in bundle_orte() {
        let gelesen = p(&["lesen", ort, BUNDLE_MARKE]);
        beginnt("read the bundle", &gelesen, "lesen: OK len=")?;
        if !gelesen.ends_with(" marker=1") {
            return Err(format!("the bundle at {ort}: {gelesen}"));
        }
        gleich(
            "write the bundle",
            &p(&["schreiben", ort]),
            "schreiben: ERR EROFS",
        )?;
    }
    Ok(())
}

fn fall_e4_netz(welt: &Welt, args: &[String]) -> Urteil {
    let teile = welt.cloud_teile();
    let p = |probe: &[&str]| welt.probe(args, probe);
    // the network: the forwarder is the one address, nothing else
    gleich(
        "the forwarder",
        &p(&["tcp", &format!("127.0.0.1:{WEITER_PORT}")]),
        "tcp: OK",
    )?;
    gleich(
        "another port of the loopback",
        &p(&["tcp", "127.0.0.1:8403"]),
        "tcp: ERR ECONNREFUSED",
    )?;
    gleich(
        "a foreign address",
        &p(&["tcp", "192.0.2.1:80"]),
        "tcp: ERR ENETUNREACH",
    )?;
    gleich(
        "the relay at the host path",
        &p(&["frage", &s(&teile.relay.socket), "x"]),
        "frage: ERR ENOENT",
    )?;
    gleich(
        "the files of the exit",
        &p(&["liste", "/run/brain"]),
        "liste: OK modell.sock,such.sock,weiter",
    )
}

#[test]
fn case_e4_the_cloud_harness_has_the_placeholder_only() {
    let welt = Welt::neu_cloud();
    let teile = welt.cloud_teile();
    // the controls: outside, the relay swaps, and every thing a mutant would mount exists and holds its mark
    let aussen = welt.probe_aussen(&[
        "frage",
        &s(&teile.relay.socket),
        &format!("TOKEN {COPILOT_PLATZHALTER}"),
    ]);
    assert_eq!(
        aussen,
        format!(
            "frage: OK tausch=1 laenge={}",
            format!("TOKEN {TOKEN_MARKE}").len()
        )
    );
    for (nadel, wurzel) in [
        (TOKEN_MARKE, &teile.konfig),
        (TOKEN_MARKE, &teile.gh),
        (CA_MARKE, &teile.ca_ordner),
    ] {
        let aussen = welt.probe_aussen(&["suche", nadel, &s(wurzel)]);
        assert!(aussen.ends_with(" treffer=1 md=0"), "{aussen}");
    }
    assert_eq!(
        welt.probe_aussen(&["frage", &s(&teile.bus.socket), "{\"op\":\"search\"}"]),
        format!("frage: OK {{\"route\":\"{TOKEN_MARKE}\"}}")
    );
    let bundle_vorher = fs::read(&teile.bundle).expect("bundle");
    let profil = welt.profil_cloud();
    assert_eq!(profil.vorpruefen(), Ok(()));
    assert_eq!(fall_e4(&welt, &profil.argumente()), Ok(()));
    assert_eq!(
        fs::read(&teile.bundle).expect("bundle"),
        bundle_vorher,
        "the host bundle is unchanged"
    );
}

/// The list without the forwarder: the program runs directly below `bwrap`.
fn ohne_weiterleiter(args: &[String]) -> Vec<String> {
    let naht = args.iter().position(|a| a == "--").expect("--");
    let zweite = args[naht + 1..]
        .iter()
        .position(|a| a == "--")
        .expect("second --")
        + naht
        + 1;
    let mut r = args[..=naht].to_vec();
    r.extend_from_slice(&args[zweite + 1..]);
    r
}

#[test]
fn case_e4_counter_probe_each_boundary_missing_turns_it_red() {
    let welt = Welt::neu_cloud();
    let teile = welt.cloud_teile();
    let args = welt.profil_cloud().argumente();
    let heim = welt.heim_text();
    let nutzer = nutzer_ordner();
    let ort = bundle_orte()[0];
    let mutanten = [
        (
            "mutant 1: the token in the environment",
            ersetzt(
                &args,
                &["--setenv", "COPILOT_GITHUB_TOKEN", COPILOT_PLATZHALTER],
                &["--setenv", "COPILOT_GITHUB_TOKEN", TOKEN_MARKE],
            ),
        ),
        (
            "mutant 2: the config file with the token bound",
            davor(
                &args,
                &["--ro-bind", &s(&teile.konfig), &format!("{heim}/.copilot")],
            ),
        ),
        (
            "mutant 3: the bus of the host bound and named",
            davor(
                &args,
                &[
                    "--ro-bind",
                    &s(&teile.bus.socket),
                    &format!("{nutzer}/bus"),
                    "--setenv",
                    "DBUS_SESSION_BUS_ADDRESS",
                    &format!("unix:path={nutzer}/bus"),
                ],
            ),
        ),
        (
            "mutant 4: gh with its login bound",
            davor(
                &args,
                &["--ro-bind", &s(&teile.gh), &format!("{heim}/.config/gh")],
            ),
        ),
        (
            "the key of the authority bound",
            davor(
                &args,
                &["--ro-bind", &s(&teile.ca_ordner), &s(&teile.ca_ordner)],
            ),
        ),
        (
            "the bundle writable",
            ersetzt(
                &args,
                &["--ro-bind", &s(&teile.bundle), ort],
                &["--bind", &s(&teile.bundle), ort],
            ),
        ),
        (
            "a variable of the certificate path set",
            davor(&args, &["--setenv", "SSL_CERT_FILE", ort]),
        ),
        ("no forwarder", ohne_weiterleiter(&args)),
    ];
    for (name, mutant) in mutanten {
        assert!(
            fall_e4(&welt, &mutant).is_err(),
            "the case stayed green without its boundary: {name}"
        );
    }
}
