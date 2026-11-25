//! The safe mode of the model path (contract 8, section 3a), outer tests MW-L1 to MW-L5: each one a whole process of
//! the program under Linux, without root and without setgid. Section 3a holds under Linux in every start, so these
//! starts carry the duties as a start under setgid does.
//!
//! Some tests run the program in a user and mount namespace of its own (`unshare -Urm`), owned by this test:
//! - the holder of a test has to sit at the fixed path `/run/user/<uid>/wb-halter/halter.sock`; a bind mount over
//!   `/run/user`, visible only in that namespace, puts the stand-in there without touching the real folder;
//! - the program makes itself non-dumpable (step 1), so `/proc/<pid>/environ` and the files of its threads are closed to
//!   every other process of the same user. The owner of the namespace keeps that right in it, and this test is the
//!   owner (the same way as `rust/crates/bruecke/tests/aussen/pruefclient.py`).
//!
//! Needs `unshare`, `mount` and `sh` and user namespaces; without them the tests are red, not skipped.
#![cfg(target_os = "linux")]

mod gemeinsam;

use std::ffi::OsString;
use std::fs;
use std::io::{BufRead, BufReader};
use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use gemeinsam::{
    FRIST, Halterattrappe, SCHLUESSEL, Wegwerf, argv, koerper, lauf_bis_ende, post, programm,
    ueber_socket,
};
use openssl::asn1::Asn1Time;
use openssl::bn::BigNum;
use openssl::ec::{EcGroup, EcKey};
use openssl::hash::MessageDigest;
use openssl::nid::Nid;
use openssl::pkey::PKey;
use openssl::x509::extension::BasicConstraints;
use openssl::x509::{X509, X509Extension, X509NameBuilder};

/// The host of the bound target of these tests.
const HOST: &str = "api.beispiel.invalid";

/// The program as a running process; killed when the test ends.
struct Lauf {
    kind: Child,
    stdout: mpsc::Receiver<String>,
    stderr: mpsc::Receiver<String>,
}

impl Lauf {
    fn pid(&self) -> u32 {
        self.kind.id()
    }

    /// The first line on stdout, or an empty text after [`FRIST`].
    fn erste_zeile(&self) -> String {
        self.stdout.recv_timeout(FRIST).unwrap_or_default()
    }

    /// Everything on stderr up to the end of the process (at most [`FRIST`]).
    fn stderr_bis_ende(&mut self) -> (Option<i32>, String) {
        let ende = std::time::Instant::now() + FRIST;
        let status = loop {
            if let Some(s) = self.kind.try_wait().expect("wait") {
                break s.code();
            }
            if std::time::Instant::now() > ende {
                break None;
            }
            std::thread::sleep(Duration::from_millis(5));
        };
        let mut text = String::new();
        while let Ok(z) = self.stderr.recv_timeout(Duration::from_millis(200)) {
            text.push_str(&z);
            text.push('\n');
        }
        (status, text)
    }
}

impl Drop for Lauf {
    fn drop(&mut self) {
        let _ = self.kind.kill();
        let _ = self.kind.wait();
    }
}

fn zeilen(r: impl std::io::Read + Send + 'static) -> mpsc::Receiver<String> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for z in BufReader::new(r).lines() {
            let Ok(z) = z else { break };
            if tx.send(z).is_err() {
                break;
            }
        }
    });
    rx
}

/// Starts the program with `args` and exactly `env`.
///
/// With `namensraum` it runs in a user and mount namespace of its own (`unshare -Urm`; uid 0 there, which is this
/// user outside) behind an `sh` with the umask 000, which then replaces itself by the program: the PID stays the one of
/// the child started here. With `run` that folder is bound over `/run/user` inside the namespace first.
fn starte(args: &[OsString], env: &[(&str, &str)], namensraum: bool, run: Option<&Path>) -> Lauf {
    let mut cmd = if namensraum {
        let mut c = Command::new("unshare");
        c.args(["-Urm", "--", "sh", "-c"]);
        c.arg(
            "umask 000 && if [ -n \"${1}\" ]; then mount --bind \"$1\" /run/user || exit 97; fi; shift; exec \"$0\" \"$@\"",
        );
        c.arg(programm());
        c.arg(run.map_or_else(OsString::new, |r| r.as_os_str().to_owned()));
        c
    } else {
        Command::new(programm())
    };
    cmd.args(args)
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (k, v) in env {
        cmd.env(k, v);
    }
    let mut kind = cmd.spawn().expect("start the program");
    let stdout = zeilen(kind.stdout.take().expect("stdout"));
    let stderr = zeilen(kind.stderr.take().expect("stderr"));
    Lauf {
        kind,
        stdout,
        stderr,
    }
}

/// A throw-away CA for `hosts` with the properties of section 12 (as in `tests/kopf.rs`).
#[allow(
    deprecated,
    reason = "X509Extension::new_nid is the way the openssl crate writes name constraints"
)]
fn ca(w: &Wegwerf, hosts: &[&str]) -> (PathBuf, PathBuf) {
    let gruppe = EcGroup::from_curve_name(Nid::X9_62_PRIME256V1).expect("curve");
    let schluessel = PKey::from_ec_key(EcKey::generate(&gruppe).expect("key")).expect("pkey");
    let mut name = X509NameBuilder::new().expect("name");
    name.append_entry_by_text("CN", "mw-test-ca").expect("cn");
    let name = name.build();
    let mut bau = X509::builder().expect("builder");
    bau.set_version(2).expect("version");
    bau.set_serial_number(
        &BigNum::from_u32(1)
            .expect("bn")
            .to_asn1_integer()
            .expect("serial"),
    )
    .expect("serial");
    bau.set_subject_name(&name).expect("subject");
    bau.set_issuer_name(&name).expect("issuer");
    bau.set_pubkey(&schluessel).expect("pubkey");
    let beginn = i64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time")
            .as_secs(),
    )
    .expect("secs")
        - 60;
    bau.set_not_before(&Asn1Time::from_unix(beginn).expect("time"))
        .expect("before");
    bau.set_not_after(&Asn1Time::from_unix(beginn + 23 * 3600).expect("time"))
        .expect("after");
    bau.append_extension(
        BasicConstraints::new()
            .critical()
            .ca()
            .pathlen(0)
            .build()
            .expect("bc"),
    )
    .expect("ext");
    let mut wert = String::from("critical");
    for h in hosts {
        wert.push_str(",permitted;DNS:");
        wert.push_str(h);
    }
    wert.push_str(",excluded;IP:0.0.0.0/0.0.0.0,excluded;IP:::/::");
    let ext = X509Extension::new_nid(
        None,
        Some(&bau.x509v3_context(None, None)),
        Nid::NAME_CONSTRAINTS,
        &wert,
    )
    .expect("nc");
    bau.append_extension(ext).expect("ext");
    bau.sign(&schluessel, MessageDigest::sha256())
        .expect("sign");
    let zert = bau.build();
    (
        datei_0600(
            w,
            "ca.key",
            &schluessel.private_key_to_pem_pkcs8().expect("pem"),
        ),
        datei_0600(w, "ca.crt", &zert.to_pem().expect("pem")),
    )
}

fn datei_0600(w: &Wegwerf, name: &str, inhalt: &[u8]) -> PathBuf {
    let p = w.pfad(name);
    fs::write(&p, inhalt).expect("write");
    fs::set_permissions(&p, fs::Permissions::from_mode(0o600)).expect("chmod");
    p
}

/// The call of mode 2 with one bound target whose `halter_ziel` is `halter_ziel`.
fn argv_zwei(w: &Wegwerf, halter_ziel: &str) -> Vec<OsString> {
    let liste = format!(
        r#"{{"fassung":1,"ca_zulassung":"zugelassen","ziele":[{{"name":"modell","host":"{HOST}","port":443,"halter_ziel":"{halter_ziel}","regeln":[{{"methode":"*","pfad":"/*"}}],"upgrade":"ablehnen"}}]}}"#
    );
    let liste = datei_0600(w, "zielliste.json", liste.as_bytes());
    let (k, z) = ca(w, &[HOST]);
    [
        "--betriebsart",
        "kopf-einsetzen",
        "--socket",
        w.pfad("m.sock").to_str().expect("utf8"),
        "--socket-rechte",
        "0600",
        "--sitzung",
        "s-1",
        "--route",
        "cloud-beispiel",
        "--protokoll",
        w.protokoll().to_str().expect("utf8"),
        "--zielliste",
        liste.to_str().expect("utf8"),
        "--ca-schluessel",
        k.to_str().expect("utf8"),
        "--ca-zertifikat",
        z.to_str().expect("utf8"),
    ]
    .iter()
    .map(OsString::from)
    .collect()
}

/// A folder for `/run/user` in the namespace, with a stand-in of the holder at `<run>/0/wb-halter/halter.sock` (uid 0
/// in the namespace is this user outside). The stand-in answers the question of step 3 (`schluessel.vorhanden` by
/// `ziel`) with `true`, so that the start runs (contract 8, section 13.5).
fn run_mit_halter(w: &Wegwerf) -> (PathBuf, Halterattrappe) {
    let run = w.pfad("run");
    fs::create_dir_all(run.join("0")).expect("run");
    fs::set_permissions(&run, fs::Permissions::from_mode(0o755)).expect("chmod");
    fs::set_permissions(run.join("0"), fs::Permissions::from_mode(0o700)).expect("chmod");
    let h = Halterattrappe::neu_in(
        &run.join("0").join("wb-halter"),
        vec![r#"{"art":"antwort","fassung":1,"id":<ID>,"ok":true,"vorhanden":true}"#.to_owned()],
    );
    (run, h)
}

/// MW-L1 (duty 1): a `halter_ziel` other than the `host` ends the start with exit 78 `mw.ziel_halter_name`, before the
/// holder and before the socket; with `halter_ziel` equal to the `host` and the stand-in at the fixed path the start
/// runs.
#[test]
fn mw_l1_the_holder_target_is_the_host() {
    for falsch in [
        "api.anderes.invalid",
        "api.beispiel.invalid:443",
        "API.BEISPIEL.INVALID",
    ] {
        let w = Wegwerf::neu("l1");
        let (exit, out, err) = lauf_bis_ende(&argv_zwei(&w, falsch));
        assert_eq!(
            (exit, out.as_slice(), err.as_slice()),
            (
                78,
                &b""[..],
                &b"mw.ziel_halter_name: Halterziel nicht der Host\n"[..]
            ),
            "{falsch}"
        );
        assert!(!w.pfad("m.sock").exists(), "{falsch}");
    }
    let w = Wegwerf::neu("l1gut");
    let (run, h) = run_mit_halter(&w);
    let l = starte(&argv_zwei(&w, HOST), &[], true, Some(&run));
    assert_eq!(l.erste_zeile(), "{\"bereit\":true}");
    // Step 3 asked the holder at the fixed path by the bound target, which is the host, never by a name.
    assert_eq!(
        h.empfangen(),
        vec![format!(
            r#"{{"fassung":1,"id":1,"op":"schluessel.vorhanden","ziel":"{HOST}"}}"#
        )]
    );
}

/// MW-L2 (duty 2): `--halter-socket <pfad>` is a call failure (exit 64 `mw.aufruf`), also with a stand-in at that path;
/// `XDG_RUNTIME_DIR` is not read (a stand-in there and none at the fixed path is `mw.halter_fehlt`); the stand-in at
/// the fixed path lets the start run.
#[test]
fn mw_l2_the_holder_socket_is_a_fixed_path() {
    let w = Wegwerf::neu("l2");
    let anderswo = Halterattrappe::neu_in(&w.pfad("anderswo").join("wb-halter"), Vec::new());
    let mut a = argv_zwei(&w, HOST);
    a.push("--halter-socket".into());
    a.push(anderswo.pfad().into_os_string());
    let (exit, out, err) = lauf_bis_ende(&a);
    assert_eq!(
        (exit, out.as_slice(), err.as_slice()),
        (64, &b""[..], &b"mw.aufruf: Aufruf ungueltig\n"[..])
    );
    assert_eq!(anderswo.empfangen(), Vec::<String>::new());

    // a folder for /run/user without a holder, and the variable pointing at the stand-in
    let leer = w.pfad("run-leer");
    fs::create_dir_all(leer.join("0")).expect("run");
    let xdg = w.pfad("anderswo");
    let mut l = starte(
        &argv_zwei(&w, HOST),
        &[("XDG_RUNTIME_DIR", xdg.to_str().expect("utf8"))],
        true,
        Some(&leer),
    );
    assert_eq!(
        l.stderr_bis_ende(),
        (
            Some(78),
            "mw.halter_fehlt: Halter nicht erreichbar\n".to_owned()
        )
    );

    let w = Wegwerf::neu("l2gut");
    let (run, _h) = run_mit_halter(&w);
    let l = starte(&argv_zwei(&w, HOST), &[], true, Some(&run));
    assert_eq!(l.erste_zeile(), "{\"bereit\":true}");
}

/// The text of a file of `/proc` of the process `pid`.
fn proc_text(pid: u32, name: &str) -> Vec<u8> {
    fs::read(format!("/proc/{pid}/{name}")).unwrap_or_else(|f| panic!("/proc/{pid}/{name}: {f}"))
}

/// The call of mode 1 with a trial run, against a closed port of the loopback.
fn argv_eins(w: &Wegwerf) -> Vec<OsString> {
    let schluessel = w.schluessel(format!("{SCHLUESSEL}\n").as_bytes());
    let mut a = argv(
        &w.pfad("m.sock"),
        "http://127.0.0.1:9/v1",
        &schluessel,
        &w.protokoll(),
    );
    for x in [
        "--probelauf",
        "--marker",
        "MK-A",
        "--marker-zaehlung",
        w.pfad("marker.json").to_str().expect("utf8"),
    ] {
        a.push(x.into());
    }
    a
}

/// MW-L3 (duty 3): started with names outside the positive list (which is empty), the running process has an empty
/// environment, also in `/proc/<pid>/environ`, in both modes. The control: the same reading sees the environment of a
/// plain process in the same kind of namespace.
#[test]
fn mw_l3_the_environment_of_the_running_process_is_empty() {
    let env = [
        ("PATH", "/usr/bin:/bin"),
        ("LANG", "C"),
        ("HOME", "/srv/beispiel"),
        ("XDG_RUNTIME_DIR", "/srv/beispiel/run"),
        ("LD_PRELOAD", "/srv/beispiel/x.so"),
        ("WB_REST", "mw-l3-wert"),
    ];
    let kontrolle = Command::new("unshare")
        .args(["-Urm", "--", "sh", "-c", "exec sleep 30"])
        .env_clear()
        .env("WB_REST", "mw-l3-wert")
        .stdin(Stdio::null())
        .spawn()
        .expect("control");
    let mut kontrolle = scopeguard(kontrolle);
    let mut gesehen = Vec::new();
    for _ in 0..200 {
        gesehen = proc_text(kontrolle.0.id(), "environ");
        if gemeinsam::enthaelt(&gesehen, b"WB_REST=mw-l3-wert") {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(
        gemeinsam::enthaelt(&gesehen, b"WB_REST=mw-l3-wert"),
        "the control sees no environment"
    );
    let _ = kontrolle.0.kill();

    let w = Wegwerf::neu("l3eins");
    let l = starte(&argv_eins(&w), &env, true, None);
    assert_eq!(l.erste_zeile(), "{\"bereit\":true}");
    assert_eq!(proc_text(l.pid(), "environ"), b"");

    let w = Wegwerf::neu("l3zwei");
    let (run, _h) = run_mit_halter(&w);
    let l = starte(&argv_zwei(&w, HOST), &env, true, Some(&run));
    assert_eq!(l.erste_zeile(), "{\"bereit\":true}");
    assert_eq!(proc_text(l.pid(), "environ"), b"");
}

/// A child that is killed when the test ends.
struct Wache(Child);

fn scopeguard(c: Child) -> Wache {
    Wache(c)
}

impl Drop for Wache {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Whether `pfad` and everything below it carries no right of the group and of others; returns the first entry that
/// does.
fn mit_gruppenrecht(pfad: &Path) -> Option<(PathBuf, u32)> {
    let m = fs::symlink_metadata(pfad).expect("stat").mode() & 0o7777;
    if m & 0o077 != 0 {
        return Some((pfad.to_path_buf(), m));
    }
    if fs::symlink_metadata(pfad).expect("stat").is_dir() {
        for e in fs::read_dir(pfad).expect("folder") {
            if let Some(f) = mit_gruppenrecht(&e.expect("entry").path()) {
                return Some(f);
            }
        }
    }
    None
}

/// MW-L4 (duty 4): `--socket-rechte 0660` and every other value with a right of the group or of others is exit 64
/// `mw.aufruf`; after a start behind the umask 000 and one request the process has the umask 077, and the socket,
/// the protocol folder with all its entries and the count file carry no right of the group or of others.
#[test]
fn mw_l4_no_rights_of_the_group() {
    for rechte in [
        "0660", "0640", "0620", "0606", "0604", "0602", "0601", "0610", "0700", "600",
    ] {
        let w = Wegwerf::neu("l4auf");
        let mut a = argv_eins(&w);
        let i = a
            .iter()
            .position(|x| x == "--socket-rechte")
            .expect("option")
            + 1;
        a[i] = rechte.into();
        let (exit, out, err) = lauf_bis_ende(&a);
        assert_eq!(
            (exit, out.as_slice(), err.as_slice()),
            (64, &b""[..], &b"mw.aufruf: Aufruf ungueltig\n"[..]),
            "{rechte}"
        );
        assert!(!w.pfad("m.sock").exists(), "{rechte}");
    }
    let w = Wegwerf::neu("l4");
    let l = starte(&argv_eins(&w), &[("PATH", "/usr/bin:/bin")], true, None);
    assert_eq!(l.erste_zeile(), "{\"bereit\":true}");
    let _ = ueber_socket(&w.pfad("m.sock"), &post(&koerper("MK-A"), ""));
    let status = String::from_utf8(proc_text(l.pid(), "status")).expect("utf8");
    let umask = status
        .lines()
        .find_map(|z| z.strip_prefix("Umask:"))
        .map(str::trim);
    assert_eq!(umask, Some("0077"), "{status}");
    let socket = fs::symlink_metadata(w.pfad("m.sock")).expect("socket");
    assert!(socket.file_type().is_socket());
    assert_eq!(socket.mode() & 0o7777, 0o600);
    assert_eq!(mit_gruppenrecht(&w.protokoll()), None);
    assert!(
        fs::read_dir(w.protokoll()).expect("folder").count() > 0,
        "the request wrote no line"
    );
    assert_eq!(mit_gruppenrecht(&w.pfad("marker.json")), None);
}

/// Every `children` file of the threads of `pid`; the texts.
fn kinder_texte(pid: u32) -> Vec<String> {
    let mut v = Vec::new();
    for e in fs::read_dir(format!("/proc/{pid}/task")).expect("tasks") {
        let p = e.expect("task").path().join("children");
        match fs::read_to_string(&p) {
            Ok(t) => v.push(t),
            Err(f) if f.kind() == std::io::ErrorKind::NotFound => {}
            Err(f) => panic!("{}: {f}", p.display()),
        }
    }
    v
}

/// MW-L5 (duty 5): after readiness and after a request, no thread of the process has a child. The control: the same
/// reading sees the child of a plain `sh` in the same kind of namespace.
#[test]
fn mw_l5_no_child() {
    let kontrolle = Command::new("unshare")
        .args(["-Urm", "--", "sh", "-c", "sleep 30; true"])
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .stdin(Stdio::null())
        .spawn()
        .expect("control");
    let kontrolle = scopeguard(kontrolle);
    let mut gesehen = Vec::new();
    for _ in 0..200 {
        gesehen = kinder_texte(kontrolle.0.id());
        if gesehen.iter().any(|t| !t.trim().is_empty()) {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(
        gesehen.iter().any(|t| !t.trim().is_empty()),
        "the control sees no child"
    );
    drop(kontrolle);

    let w = Wegwerf::neu("l5eins");
    let l = starte(&argv_eins(&w), &[("PATH", "/usr/bin:/bin")], true, None);
    assert_eq!(l.erste_zeile(), "{\"bereit\":true}");
    let leer = |t: &Vec<String>| t.iter().all(|x| x.trim().is_empty());
    let t = kinder_texte(l.pid());
    assert!(!t.is_empty() && leer(&t), "{t:?}");
    let _ = ueber_socket(&w.pfad("m.sock"), &post(&koerper("MK-A"), ""));
    let t = kinder_texte(l.pid());
    assert!(!t.is_empty() && leer(&t), "{t:?}");

    let w = Wegwerf::neu("l5zwei");
    let (run, _h) = run_mit_halter(&w);
    let l = starte(&argv_zwei(&w, HOST), &[], true, Some(&run));
    assert_eq!(l.erste_zeile(), "{\"bereit\":true}");
    let t = kinder_texte(l.pid());
    assert!(!t.is_empty() && leer(&t), "{t:?}");
}
