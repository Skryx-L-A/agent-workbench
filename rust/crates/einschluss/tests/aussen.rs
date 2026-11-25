//! Outside tests of the enclosure on Linux, with the stand-in harness (feature `attrappe`; the gates run with all
//! features). Each case is a function that judges the answer lines of the stand-in; the test calls it twice:
//! with the real profile (it must hold) and with a profile that lacks exactly the boundary under test (it must fail).
//! A case that stays green without its boundary would be empty.
//!
//! The cases follow the script of the demonstration: (a) a note outside is not readable, (b) a foreign address is not
//! reachable, (c) the route stays the route of the start, (d) the search process is not visible (E3), (e) the socket
//! is reachable at one place only. Further: the environment, and the lifetime of `bwrap` against its starter and the
//! thread that started it.
#![cfg(all(target_os = "linux", feature = "attrappe"))]

mod gemeinsam;

use std::fs;
use std::net::TcpListener;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use einschluss::{GAST_MODELL_SOCKET, GAST_SUCH_SOCKET, Profil};
use gemeinsam::{NACHBAR_ROUTE, NOTIZ_MARKE, ROUTE, Such, Welt, davor, ersetzt, ohne};

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

// ---------------------------------------------------------------------------------------------------------------
// the profile as a whole

#[test]
fn the_finished_profile_passes_the_checks_before_the_start() {
    let welt = Welt::neu();
    assert_eq!(welt.profil().vorpruefen(), Ok(()));
}

#[test]
fn the_stand_in_runs_in_the_enclosure_and_reports_its_marker() {
    let welt = Welt::neu();
    let args = welt.profil().argumente();
    assert_eq!(
        welt.probe(&args, &["kennung"]),
        format!("kennung: OK {}", "einschluss-attrappe-marker-7f3a")
    );
}

// ---------------------------------------------------------------------------------------------------------------
// (a) a note outside the enclosure is not readable

fn fall_a(welt: &Welt, args: &[String]) -> Urteil {
    let notiz = s(&welt.notiz);
    let p = |probe: &[&str]| welt.probe(args, probe);
    gleich(
        "read the note",
        &p(&["lesen", &notiz, NOTIZ_MARKE]),
        "lesen: ERR ENOENT",
    )?;
    gleich(
        "read it through /proc/self/root",
        &p(&["lesen", &format!("/proc/self/root{notiz}"), NOTIZ_MARKE]),
        "lesen: ERR ENOENT",
    )?;
    for ordner in [&welt.vault, &welt.protokoll, &welt.start] {
        gleich(
            "list a protected folder",
            &p(&["liste", &s(ordner)]),
            "liste: ERR ENOENT",
        )?;
    }
    gleich(
        "write the start file",
        &p(&["schreiben", &s(&welt.startdatei)]),
        "schreiben: ERR ENOENT",
    )?;
    gleich(
        "list the parent of the vault",
        &p(&["liste", &s(&welt.wurzel)]),
        "liste: OK arbeit,heim,programm",
    )?;
    let baum = p(&["suche", NOTIZ_MARKE, "/"]);
    beginnt(
        "search the whole tree: it looks at files",
        &baum,
        "suche: OK dateien=",
    )?;
    if baum.ends_with(" treffer=0 md=0") {
        Ok(())
    } else {
        Err(format!("search the whole tree: {baum}"))
    }
}

#[test]
fn case_a_the_note_outside_is_not_there() {
    let welt = Welt::neu();
    // the control: the same read without an enclosure works, so the note exists and the probe can see it
    let notiz = s(&welt.notiz);
    assert_eq!(
        welt.probe_aussen(&["lesen", &notiz, NOTIZ_MARKE]),
        format!(
            "lesen: OK len={} marker=1",
            fs::metadata(&welt.notiz).expect("meta").len()
        )
    );
    let aussen = welt.probe_aussen(&["suche", NOTIZ_MARKE, &s(&welt.wurzel)]);
    assert!(aussen == "suche: OK dateien=4 treffer=1 md=1", "{aussen}");
    let hinterher = fs::read(&welt.startdatei).expect("start file");
    let args = welt.profil().argumente();
    assert_eq!(fall_a(&welt, &args), Ok(()));
    assert_eq!(
        fs::read(&welt.startdatei).expect("start file"),
        hinterher,
        "the start file is unchanged"
    );
    assert!(
        fs::read_to_string(&welt.notiz)
            .expect("note")
            .contains(NOTIZ_MARKE)
    );
}

#[test]
fn case_a_counter_probe_each_boundary_missing_turns_it_red() {
    let welt = Welt::neu();
    let args = welt.profil().argumente();
    let vault = s(&welt.vault);
    let wurzel = s(&welt.wurzel);
    let arbeit = s(&welt.arbeit);
    let mutanten = [
        ("vault bound", davor(&args, &["--ro-bind", &vault, &vault])),
        ("root bound", davor(&args, &["--ro-bind", "/", "/"])),
        (
            "parent of the vault as working folder",
            ersetzt(
                &args,
                &["--bind", &arbeit, &arbeit],
                &["--bind", &wurzel, &wurzel],
            ),
        ),
    ];
    for (name, mutant) in mutanten {
        assert!(
            fall_a(&welt, &mutant).is_err(),
            "the case stayed green without its boundary: {name}"
        );
    }
}

// ---------------------------------------------------------------------------------------------------------------
// (b) a foreign address is not reachable

/// Counts the connections that reach a listener of the host.
struct Zaehler {
    anzahl: Arc<AtomicUsize>,
    stopp: Arc<std::sync::atomic::AtomicBool>,
    faden: Option<std::thread::JoinHandle<()>>,
    pub adresse: String,
}

impl Zaehler {
    fn neu() -> Zaehler {
        let hoerer = TcpListener::bind("127.0.0.1:0").expect("listener");
        hoerer.set_nonblocking(true).expect("nonblocking");
        let adresse = hoerer.local_addr().expect("addr").to_string();
        let anzahl = Arc::new(AtomicUsize::new(0));
        let stopp = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let (a, st) = (Arc::clone(&anzahl), Arc::clone(&stopp));
        let faden = std::thread::spawn(move || {
            while !st.load(Ordering::SeqCst) {
                if let Ok((mut strom, _)) = hoerer.accept() {
                    a.fetch_add(1, Ordering::SeqCst);
                    // a service that answers a line: `dienst:<line>`
                    let _ = strom.set_nonblocking(false);
                    let _ = strom.set_read_timeout(Some(Duration::from_millis(300)));
                    let mut zeile = String::new();
                    if std::io::BufRead::read_line(&mut std::io::BufReader::new(&strom), &mut zeile)
                        .is_ok_and(|n| n > 0)
                    {
                        let _ = std::io::Write::write_all(
                            &mut strom,
                            format!("dienst:{zeile}").as_bytes(),
                        );
                    }
                } else {
                    std::thread::sleep(Duration::from_millis(5));
                }
            }
        });
        Zaehler {
            anzahl,
            stopp,
            faden: Some(faden),
            adresse,
        }
    }

    fn stand(&self) -> usize {
        std::thread::sleep(Duration::from_millis(100));
        self.anzahl.load(Ordering::SeqCst)
    }
}

impl Drop for Zaehler {
    fn drop(&mut self) {
        self.stopp.store(true, Ordering::SeqCst);
        if let Some(f) = self.faden.take() {
            let _ = f.join();
        }
    }
}

/// With `fremd`, the case also tries the foreign addresses. Only the enclosure itself is sent there: a mutant that
/// shares the network is tried with the local counter alone, so that no test packet leaves this machine.
fn fall_b(welt: &Welt, args: &[String], zaehler: &Zaehler, fremd: bool) -> Urteil {
    let p = |probe: &[&str]| welt.probe(args, probe);
    gleich(
        "connect to the counter on the host loopback",
        &p(&["tcp", &zaehler.adresse]),
        "tcp: ERR ECONNREFUSED",
    )?;
    if fremd {
        gleich(
            "connect to 1.1.1.1:443",
            &p(&["tcp", "1.1.1.1:443"]),
            "tcp: ERR ENETUNREACH",
        )?;
        gleich(
            "connect to 192.0.2.1:80",
            &p(&["tcp", "192.0.2.1:80"]),
            "tcp: ERR ENETUNREACH",
        )?;
        gleich(
            "resolve a name",
            &p(&["dns", "example.org"]),
            "dns: ERR lookup",
        )?;
    }
    gleich("the counter on the host", &zaehler.stand().to_string(), "0")
}

#[test]
fn case_b_a_foreign_address_is_not_reachable() {
    let welt = Welt::neu();
    {
        // the control: without an enclosure the same connect reaches a counter
        let kontrolle = Zaehler::neu();
        assert_eq!(welt.probe_aussen(&["tcp", &kontrolle.adresse]), "tcp: OK");
        assert_eq!(kontrolle.stand(), 1);
    }
    let zaehler = Zaehler::neu();
    assert_eq!(
        fall_b(&welt, &welt.profil().argumente(), &zaehler, true),
        Ok(())
    );
}

#[test]
fn case_b_counter_probe_a_shared_network_turns_it_red() {
    let welt = Welt::neu();
    let zaehler = Zaehler::neu();
    let args = welt.profil().argumente();
    let mutant = ersetzt(&args, &["--unshare-net"], &["--share-net"]);
    let urteil = fall_b(&welt, &mutant, &zaehler, false);
    assert!(
        urteil.is_err(),
        "the case stayed green with a shared network"
    );
    assert_eq!(zaehler.stand(), 1, "the shared network reached the counter");
}

// ---------------------------------------------------------------------------------------------------------------
// (f) no loopback port of the host is reachable; only a forwarder handed in by a Unix socket is

fn fall_f(welt: &Welt, args: &[String], dienst: &Zaehler, bruecke: bool) -> Urteil {
    let p = |probe: &[&str]| welt.probe(args, probe);
    gleich(
        "connect to the service on the host loopback",
        &p(&["tcp", &dienst.adresse]),
        "tcp: ERR ECONNREFUSED",
    )?;
    if bruecke {
        gleich(
            "ask through the handed-in forwarder",
            &p(&["bruecke", GAST_MODELL_SOCKET, "18080", "hallo"]),
            "bruecke: OK dienst:hallo",
        )?;
    }
    gleich(
        "connections the service saw",
        &dienst.stand().to_string(),
        if bruecke { "1" } else { "0" },
    )
}

#[test]
fn case_f_no_loopback_port_of_the_host_is_reachable_except_the_handed_in_forwarder() {
    let welt = Welt::neu();
    let dienst = Zaehler::neu();
    let ordner = welt.wurzel.join("weiter");
    fs::create_dir_all(&ordner).expect("folder");
    let weiter = Such::weiter(&ordner.join("weiter.sock"), &dienst.adresse);
    // the control: outside, the service answers directly and through the forwarder
    assert_eq!(welt.probe_aussen(&["tcp", &dienst.adresse]), "tcp: OK");
    assert_eq!(
        welt.probe_aussen(&["frage", &s(&weiter.socket), "hallo"]),
        "frage: OK dienst:hallo"
    );
    let vorher = dienst.stand();
    assert_eq!(vorher, 2);
    let mut b = welt.beschreibung();
    b.modell_socket = Some(weiter.socket.clone());
    let args = Profil::bauen(&b).expect("builds").argumente();
    // the enclosure adds exactly one connection: the one the forwarder makes
    let basis = vorher;
    let urteil = {
        let p = |probe: &[&str]| welt.probe(&args, probe);
        gleich(
            "direct",
            &p(&["tcp", &dienst.adresse]),
            "tcp: ERR ECONNREFUSED",
        )
        .and_then(|()| {
            gleich(
                "through the forwarder",
                &p(&["bruecke", GAST_MODELL_SOCKET, "18080", "hallo"]),
                "bruecke: OK dienst:hallo",
            )
        })
    };
    assert_eq!(urteil, Ok(()));
    assert_eq!(
        dienst.stand(),
        basis + 1,
        "only the forwarder reached the service"
    );
}

#[test]
fn case_f_counter_probe_each_boundary_missing_turns_it_red() {
    let welt = Welt::neu();
    let dienst = Zaehler::neu();
    let ordner = welt.wurzel.join("weiter");
    fs::create_dir_all(&ordner).expect("folder");
    let weiter = Such::weiter(&ordner.join("weiter.sock"), &dienst.adresse);
    let mut b = welt.beschreibung();
    b.modell_socket = Some(weiter.socket.clone());
    let args = Profil::bauen(&b).expect("builds").argumente();
    let gast = weiter.socket.to_string_lossy().into_owned();
    // a shared network: the service is reached directly
    let geteilt = ersetzt(&args, &["--unshare-net"], &["--share-net"]);
    assert!(
        fall_f(&welt, &geteilt, &dienst, false).is_err(),
        "green with a shared network"
    );
    assert_eq!(dienst.stand(), 1, "the shared network reached the service");
    // no forwarder handed in: the bridge has nothing to pass to
    let ohne_weiter = ersetzt(
        &args,
        &["--ro-bind", &gast, GAST_MODELL_SOCKET],
        &["--dir", "/run/brain"],
    );
    assert!(
        fall_f(&welt, &ohne_weiter, &dienst, true).is_err(),
        "green without the forwarder"
    );
}

// ---------------------------------------------------------------------------------------------------------------
// (c) the route stays the route of the start

fn fall_c(welt: &Welt, args: &[String]) -> Urteil {
    let p = |probe: &[&str]| welt.probe(args, probe);
    let frage = "{\"op\":\"search\",\"query\":\"f\"}";
    // variables of the harness that name another route change nothing: the answer is the route of the socket
    gleich(
        "ask with route variables set",
        &p(&["frageumgebung", GAST_SUCH_SOCKET, frage]),
        &format!("frageumgebung: OK {{\"route\":\"{ROUTE}\"}}"),
    )?;
    gleich(
        "ask with a route in the request",
        &p(&[
            "frage",
            GAST_SUCH_SOCKET,
            "{\"op\":\"search\",\"query\":\"f\",\"route\":\"lokal-1\"}",
        ]),
        "frage: OK {\"error\":\"bad_request\"}",
    )?;
    gleich(
        "the socket of the neighbour session by its host path",
        &p(&["frage", &s(&welt.nachbar.socket), "{\"op\":\"ping\"}"]),
        "frage: ERR ENOENT",
    )?;
    gleich(
        "the neighbour at the fixed place",
        &p(&["frage", "/run/brain/such-nachbar.sock", "{\"op\":\"ping\"}"]),
        "frage: ERR ENOENT",
    )?;
    gleich(
        "read the start file",
        &p(&["lesen", &s(&welt.startdatei)]),
        "lesen: ERR ENOENT",
    )?;
    gleich(
        "write the start file",
        &p(&["schreiben", &s(&welt.startdatei)]),
        "schreiben: ERR ENOENT",
    )?;
    beginnt(
        "build an enclosure of its own",
        &p(&["bwrap"]),
        "bwrap: ERR",
    )
}

#[test]
fn case_c_the_route_stays_the_route_of_the_start() {
    let welt = Welt::neu();
    // the control: the stand-in search answers with its own route, and the request with a route field is refused
    // outside as well; so the route in the answer is the argument of the process, nothing else
    assert_eq!(
        welt.probe_aussen(&[
            "frage",
            &s(&welt.such.socket),
            "{\"op\":\"search\",\"query\":\"f\"}"
        ]),
        format!("frage: OK {{\"route\":\"{ROUTE}\"}}")
    );
    assert_eq!(
        welt.probe_aussen(&[
            "frage",
            &s(&welt.nachbar.socket),
            "{\"op\":\"search\",\"query\":\"f\"}"
        ]),
        format!("frage: OK {{\"route\":\"{NACHBAR_ROUTE}\"}}")
    );
    let vorher = fs::read(&welt.startdatei).expect("start file");
    assert_eq!(fall_c(&welt, &welt.profil().argumente()), Ok(()));
    assert_eq!(
        fs::read(&welt.startdatei).expect("start file"),
        vorher,
        "the start file is unchanged"
    );
}

#[test]
fn case_c_counter_probe_each_boundary_missing_turns_it_red() {
    let welt = Welt::neu();
    let args = welt.profil().argumente();
    let nachbar = s(&welt.nachbar.socket);
    let start = s(&welt.start);
    let eigen = s(&welt.such.socket);
    let mutanten = [
        (
            "neighbour socket bound",
            davor(
                &args,
                &["--ro-bind", &nachbar, "/run/brain/such-nachbar.sock"],
            ),
        ),
        (
            "folder of the start file bound",
            davor(&args, &["--bind", &start, &start]),
        ),
        (
            "another user namespace allowed",
            ohne(&args, "--disable-userns"),
        ),
        (
            "the socket of the neighbour instead of its own",
            ersetzt(
                &args,
                &["--ro-bind", &eigen, GAST_SUCH_SOCKET],
                &["--ro-bind", &nachbar, GAST_SUCH_SOCKET],
            ),
        ),
    ];
    for (name, mutant) in mutanten {
        assert!(
            fall_c(&welt, &mutant).is_err(),
            "the case stayed green without its boundary: {name}"
        );
    }
}

// ---------------------------------------------------------------------------------------------------------------
// (d) E3: the search process is not visible, cannot be signalled, its environment cannot be read

fn fall_d(welt: &Welt, args: &[String]) -> Urteil {
    let pid = welt.such.pid.to_string();
    let p = |probe: &[&str]| welt.probe(args, probe);
    let prozesse = p(&["proz"]);
    let liste = prozesse
        .strip_prefix("proz: OK ")
        .ok_or_else(|| format!("proz: {prozesse}"))?;
    if liste.split(',').any(|n| n == pid) {
        return Err(format!(
            "the search process {pid} is in the process list {liste}"
        ));
    }
    gleich(
        "signal the search process",
        &p(&["signal", &pid]),
        "signal: ERR kill",
    )?;
    gleich(
        "read its environment",
        &p(&["environ", &pid]),
        "environ: ERR ENOENT",
    )?;
    gleich(
        "list its process folder",
        &p(&["liste", &format!("/proc/{pid}")]),
        "liste: ERR ENOENT",
    )
}

#[test]
fn case_d_the_search_process_is_not_visible() {
    let welt = Welt::neu();
    let pid = welt.such.pid.to_string();
    // the control: without an enclosure the process is in the list, a signal reaches it, its environment is readable
    let aussen = welt.probe_aussen(&["proz"]);
    assert!(aussen.split([' ', ',']).any(|n| n == pid), "{aussen}");
    assert_eq!(welt.probe_aussen(&["signal", &pid]), "signal: OK");
    assert!(
        welt.probe_aussen(&["environ", &pid])
            .starts_with("environ: OK")
    );
    assert_eq!(fall_d(&welt, &welt.profil().argumente()), Ok(()));
}

#[test]
fn case_d_counter_probe_without_the_own_process_space_turns_it_red() {
    let welt = Welt::neu();
    let args = welt.profil().argumente();
    let mutant = ersetzt(
        &ohne(&args, "--unshare-pid"),
        &["--proc", "/proc"],
        &["--ro-bind", "/proc", "/proc"],
    );
    let urteil = fall_d(&welt, &mutant);
    assert!(
        urteil.is_err(),
        "the case stayed green without its own process space"
    );
}

// ---------------------------------------------------------------------------------------------------------------
// (e) the socket is reachable at one place only

fn fall_e(welt: &Welt, args: &[String]) -> Urteil {
    let p = |probe: &[&str]| welt.probe(args, probe);
    gleich(
        "ask at the fixed place",
        &p(&["frage", GAST_SUCH_SOCKET, "{\"op\":\"ping\"}"]),
        "frage: OK {\"ok\":true}",
    )?;
    gleich(
        "list the place",
        &p(&["liste", "/run/brain"]),
        "liste: OK such.sock",
    )?;
    gleich("list /run", &p(&["liste", "/run"]), "liste: OK brain,user")?;
    gleich(
        "ask at the host path",
        &p(&["frage", &s(&welt.such.socket), "{\"op\":\"ping\"}"]),
        "frage: ERR ENOENT",
    )?;
    gleich(
        "list the folder of the socket",
        &p(&["liste", &s(&welt.such_ordner)]),
        "liste: ERR ENOENT",
    )?;
    gleich(
        "list the empty bus place",
        &p(&["liste", &format!("/run/user/{}", uid())]),
        "liste: OK",
    )
}

fn uid() -> u32 {
    einschluss::System::vom_host().expect("host").uid
}

#[test]
fn case_e_the_socket_is_reachable_at_one_place_only() {
    let welt = Welt::neu();
    // the control: outside, the socket is reachable at its host path and its folder is there
    assert_eq!(
        welt.probe_aussen(&["frage", &s(&welt.such.socket), "{\"op\":\"ping\"}"]),
        "frage: OK {\"ok\":true}"
    );
    assert_eq!(
        welt.probe_aussen(&["liste", &s(&welt.such_ordner)]),
        "liste: OK such.sock"
    );
    assert_eq!(fall_e(&welt, &welt.profil().argumente()), Ok(()));
}

#[test]
fn case_e_counter_probe_each_boundary_missing_turns_it_red() {
    let welt = Welt::neu();
    let args = welt.profil().argumente();
    let ordner = s(&welt.such_ordner);
    let eigen = s(&welt.such.socket);
    let mutanten = [
        (
            "folder of the socket bound",
            davor(&args, &["--ro-bind", &ordner, &ordner]),
        ),
        (
            "no socket bound",
            ersetzt(
                &args,
                &["--ro-bind", &eigen, GAST_SUCH_SOCKET],
                &["--dir", "/run/brain"],
            ),
        ),
    ];
    for (name, mutant) in mutanten {
        assert!(
            fall_e(&welt, &mutant).is_err(),
            "the case stayed green without its boundary: {name}"
        );
    }
}

// ---------------------------------------------------------------------------------------------------------------
// the environment

fn fall_umgebung(welt: &Welt, args: &[String]) -> Urteil {
    let aus = welt.probe_env(
        args,
        &["umgebung"],
        &[
            ("GEHEIM_X", "1"),
            ("ROUTE", "lokal"),
            ("LD_PRELOAD", "/x.so"),
            ("DBUS_SESSION_BUS_ADDRESS", "unix:path=/run/user/1/bus"),
            ("XDG_RUNTIME_DIR", "/run/user/1"),
            ("SSL_CERT_FILE", "/x.pem"),
            ("NODE_EXTRA_CA_CERTS", "/x.pem"),
            ("HTTPS_PROXY", "http://127.0.0.1:1"),
            ("GIT_CONFIG_COUNT", "1"),
        ],
    );
    // PWD is set by bwrap itself; the five variables are the profile's
    gleich(
        "variables of the harness",
        &aus,
        "umgebung: OK HOME,LANG,PATH,PWD,TERM,TMPDIR",
    )
}

#[test]
fn the_environment_is_cleared_and_set_again() {
    let welt = Welt::neu();
    assert_eq!(fall_umgebung(&welt, &welt.profil().argumente()), Ok(()));
    let mutant = ohne(&welt.profil().argumente(), "--clearenv");
    assert!(
        fall_umgebung(&welt, &mutant).is_err(),
        "the case stayed green without --clearenv"
    );
}

// ---------------------------------------------------------------------------------------------------------------
// the lifetime of bwrap against its starter and against the thread that started it

/// The program in the enclosure for the two lifetime cases: it says `lebt` once it runs, then waits for a line on its
/// standard input and says `ende`. It never ends by a clock: it ends when the test writes the line or drops the pipe, so
/// no process is left over and a slow machine cannot end it early.
fn wartendes_programm(args: &[String]) -> Vec<String> {
    let naht = args.iter().position(|a| a == "--").expect("--");
    let mut alle: Vec<String> = args[..=naht].to_vec();
    alle.extend([
        "/usr/bin/sh".to_owned(),
        "-c".to_owned(),
        "echo lebt; read weiter; echo ende".to_owned(),
    ]);
    alle
}

/// Reads the lines of `aus` in a thread of its own; the channel ends when every holder of the pipe is gone.
fn zeilen(aus: impl std::io::Read + Send + 'static) -> std::sync::mpsc::Receiver<String> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for zeile in std::io::BufRead::lines(std::io::BufReader::new(aus)) {
            let Ok(zeile) = zeile else { break };
            if tx.send(zeile).is_err() {
                break;
            }
        }
    });
    rx
}

/// How long the death of the harness after the end of its starter may take. It bounds only the real case (death); the
/// counter probe (no death) waits this long and then shows by an answer that the harness still runs.
const TOD_FRIST: Duration = Duration::from_secs(10);

/// The verdict after the end of the starter: `true` when the output of the program closes within `TOD_FRIST` without
/// another line (the program was ended), `false` when the program still answers the line of the test with `ende`.
fn beendet(rx: &std::sync::mpsc::Receiver<String>, mut ein: std::process::ChildStdin) -> bool {
    match rx.recv_timeout(TOD_FRIST) {
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return true,
        Ok(zeile) => panic!("unexpected line {zeile:?} after the end of the starter"),
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
    }
    std::io::Write::write_all(&mut ein, b"weiter\n").expect("the line for the program");
    drop(ein);
    assert_eq!(
        rx.recv().ok().as_deref(),
        Some("ende"),
        "the program neither ended nor answered"
    );
    false
}

/// Starts `bwrap` below a shell, kills the shell with `SIGKILL` once the program in the enclosure said `lebt` (the set-up
/// of `bwrap` is over), and says whether the program was ended with it ([`beendet`]). No step waits for a fixed time
/// against the start of the enclosure, so a slow start cannot turn the answer around. A kill during the set-up is not
/// this case: there the inner `bwrap` has no death signal yet (see `profil.rs`, the row of `--die-with-parent`).
fn endet_mit_dem_starter(args: &[String]) -> bool {
    // the shell keeps its standard input for bwrap (without it, sh gives a job in the background /dev/null)
    let mut shell = std::process::Command::new("/bin/sh")
        .args([
            "-c",
            "exec 3<&0; /usr/bin/bwrap \"$@\" <&3 3<&- & wait",
            "sh",
        ])
        .args(wartendes_programm(args))
        .env_clear()
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("shell starts");
    let ein = shell.stdin.take().expect("stdin");
    let rx = zeilen(shell.stdout.take().expect("stdout"));
    assert_eq!(
        rx.recv().ok().as_deref(),
        Some("lebt"),
        "the enclosure did not start"
    );
    shell.kill().expect("kill the shell");
    let _ = shell.wait();
    beendet(&rx, ein)
}

/// Starts `bwrap` from a thread, lets that thread end only after the program in the enclosure said `lebt`, and says
/// whether the program was ended with the thread ([`beendet`]). The kernel ties the death signal to the thread that
/// forked the child, so with `--die-with-parent` the end of that thread ends the session. The thread ends after the
/// set-up on purpose: a thread that ended before `bwrap` set its signal would leave nothing to measure (that was the
/// old form of this case, and under load the race turned around).
fn endet_mit_dem_startfaden(args: &[String]) -> bool {
    let alle = wartendes_programm(args);
    let (mut kind, ein, rx) = std::thread::spawn(move || {
        let mut kind = std::process::Command::new("/usr/bin/bwrap")
            .args(alle)
            .env_clear()
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("bwrap starts");
        let ein = kind.stdin.take().expect("stdin");
        let rx = zeilen(kind.stdout.take().expect("stdout"));
        assert_eq!(
            rx.recv().ok().as_deref(),
            Some("lebt"),
            "the enclosure did not start"
        );
        (kind, ein, rx)
    })
    .join()
    .expect("the starting thread");
    let ende = beendet(&rx, ein);
    let _ = kind.wait();
    ende
}

#[test]
fn elternbindung_the_session_ends_with_its_starter_and_with_its_starting_thread() {
    let welt = Welt::neu();
    let args = welt.profil().argumente();
    // the starter dies with SIGKILL after the set-up: the session ends with it ...
    assert!(
        endet_mit_dem_starter(&args),
        "the harness outlived its starter"
    );
    // ... and the counter probe: without --die-with-parent it stays behind as an orphan
    let mutant = ohne(&args, "--die-with-parent");
    assert!(
        !endet_mit_dem_starter(&mutant),
        "without --die-with-parent the harness should outlive its starter"
    );
    // the rule a starter must follow: the thread that forks bwrap lives until the end (werkbank-lauf forks it from its
    // main thread), because the session ends with that thread ...
    assert!(
        endet_mit_dem_startfaden(&args),
        "the harness outlived the thread that started it"
    );
    // ... and the counter probe: without --die-with-parent the end of the thread does not end it
    assert!(
        !endet_mit_dem_startfaden(&mutant),
        "without --die-with-parent the harness should outlive the thread that started it"
    );
}

// ---------------------------------------------------------------------------------------------------------------
// (L1) no setgid acts in the enclosure: a program with the setgid bit does not get its group inside

/// The effective gid and the supplementary groups of this process (`/proc/self/status`, lines `Gid:` and `Groups:`).
fn eigene_gruppen() -> (u32, Vec<u32>) {
    let status = fs::read_to_string("/proc/self/status").expect("status");
    let zahlen = |name: &str| -> Vec<u32> {
        status
            .lines()
            .find_map(|z| z.strip_prefix(name))
            .map(|r| {
                r.split_whitespace()
                    .filter_map(|x| x.parse().ok())
                    .collect()
            })
            .unwrap_or_default()
    };
    let egid = *zahlen("Gid:").get(1).expect("the effective gid");
    (egid, zahlen("Groups:"))
}

/// A copy of `/usr/bin/id` in `ordner` with the group `gruppe` and the mode 2755 (setgid). The group is set first: a
/// change of the group clears the setgid bit.
fn setgid_kopie(ordner: &std::path::Path, gruppe: u32) -> std::path::PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let kopie = ordner.join("id-sgid");
    fs::copy("/usr/bin/id", &kopie).expect("copy of id");
    std::os::unix::fs::chown(&kopie, None, Some(gruppe)).expect("group of the copy");
    fs::set_permissions(&kopie, fs::Permissions::from_mode(0o2755)).expect("mode 2755");
    let modus = fs::metadata(&kopie).expect("metadata").permissions().mode();
    assert_eq!(modus & 0o7777, 0o2755, "the copy has no setgid bit");
    kopie
}

/// The output of `programm -g` (the effective gid of that program), outside or with the arguments `args` of `bwrap`.
fn egid_von(args: Option<&[String]>, programm: &std::path::Path) -> String {
    let mut befehl = match args {
        None => std::process::Command::new(programm),
        Some(args) => {
            let naht = args.iter().position(|a| a == "--").expect("--");
            let mut b = std::process::Command::new("/usr/bin/bwrap");
            b.args(&args[..=naht]).arg(programm);
            b
        }
    };
    let aus = befehl
        .arg("-g")
        .env_clear()
        .stdin(std::process::Stdio::null())
        .output()
        .expect("the program runs");
    assert!(aus.status.success(), "{}: {aus:?}", programm.display());
    String::from_utf8_lossy(&aus.stdout).trim().to_owned()
}

#[test]
// the one line of a case that cannot be measured on this host (no supplementary group)
#[allow(clippy::print_stderr)]
fn l1_no_setgid_acts_in_the_enclosure() {
    let (egid, gruppen) = eigene_gruppen();
    // a group that is not mapped into the user namespace of the test shows as the overflow gid and cannot be given
    let ueberlauf: Option<u32> = fs::read_to_string("/proc/sys/kernel/overflowgid")
        .ok()
        .and_then(|t| t.trim().parse().ok());
    let Some(gruppe) = gruppen
        .into_iter()
        .find(|g| *g != egid && Some(*g) != ueberlauf)
    else {
        // no group of its own to give a copy: the case cannot be measured on this host, and it says so
        eprintln!(
            "SKIPPED l1_no_setgid_acts_in_the_enclosure: the test process has no mapped supplementary group"
        );
        return;
    };
    let welt = Welt::neu();
    let args = welt.profil().argumente();
    // read-only in the enclosure (the program folder, like /usr) and writable (the working folder)
    for ordner in [&welt.programm, &welt.arbeit] {
        let kopie = setgid_kopie(ordner, gruppe);
        // the counter probe outside: the bit works, the program runs with the group of the file
        assert_eq!(
            egid_von(None, &kopie),
            gruppe.to_string(),
            "outside, the setgid copy in {} should run with its group",
            ordner.display()
        );
        // inside: the same gid as the plain id there (no_new_privs of bwrap, nosuid on each mount)
        let ohne_bit = egid_von(Some(&args), std::path::Path::new("/usr/bin/id"));
        let mit_bit = egid_von(Some(&args), &kopie);
        assert_eq!(
            mit_bit,
            ohne_bit,
            "the setgid copy in {} changed the gid inside the enclosure",
            ordner.display()
        );
        assert_ne!(
            mit_bit,
            gruppe.to_string(),
            "the setgid copy in {} got its group inside the enclosure",
            ordner.display()
        );
        fs::remove_file(&kopie).expect("remove the copy");
    }
}

// ---------------------------------------------------------------------------------------------------------------
// (b), the counter probe against the real network. Ignored: it sends one connection attempt to 1.1.1.1:443 and one
// name lookup of example.org out of this machine, to show that the probe can reach the network when the boundary is
// missing. Run it by hand: `cargo test -p einschluss --all-features -- --ignored case_b_real_network`.

#[test]
#[ignore = "sends one connection attempt and one lookup to the real network"]
fn case_b_real_network_the_probe_reaches_it_without_the_boundary_and_not_with_it() {
    let welt = Welt::neu();
    let args = welt.profil().argumente();
    // the profile: no way out
    assert_eq!(
        welt.probe(&args, &["tcp", "1.1.1.1:443"]),
        "tcp: ERR ENETUNREACH"
    );
    assert_eq!(
        welt.probe(&args, &["dns", "example.org"]),
        "dns: ERR lookup"
    );
    // the mutant: the network of the host is shared and the resolver file is bound
    let mutant = davor(
        &ohne(&args, "--unshare-net"),
        &[
            "--ro-bind",
            "/run/systemd/resolve/stub-resolv.conf",
            "/etc/resolv.conf",
        ],
    );
    assert_eq!(welt.probe(&mutant, &["tcp", "1.1.1.1:443"]), "tcp: OK");
    assert_eq!(welt.probe(&mutant, &["dns", "example.org"]), "dns: OK 1");
}

/// The model socket of the host in a test: counts the connections that reach it and answers each request line with
/// `modell:<line>`. It waits in `accept` and ends with `Drop` by a connection of its own.
struct Modellsocket {
    anzahl: Arc<AtomicUsize>,
    stopp: Arc<std::sync::atomic::AtomicBool>,
    socket: std::path::PathBuf,
    faden: Option<std::thread::JoinHandle<()>>,
}

impl Modellsocket {
    fn neu(socket: &std::path::Path) -> Modellsocket {
        let hoerer = std::os::unix::net::UnixListener::bind(socket).expect("socket");
        let anzahl = Arc::new(AtomicUsize::new(0));
        let stopp = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let (a, st) = (Arc::clone(&anzahl), Arc::clone(&stopp));
        let faden = std::thread::spawn(move || {
            for strom in hoerer.incoming() {
                if st.load(Ordering::SeqCst) {
                    return;
                }
                let Ok(mut strom) = strom else { continue };
                a.fetch_add(1, Ordering::SeqCst);
                let mut zeile = String::new();
                if std::io::BufRead::read_line(&mut std::io::BufReader::new(&strom), &mut zeile)
                    .is_ok_and(|n| n > 0)
                {
                    let _ =
                        std::io::Write::write_all(&mut strom, format!("modell:{zeile}").as_bytes());
                }
            }
        });
        Modellsocket {
            anzahl,
            stopp,
            socket: socket.to_path_buf(),
            faden: Some(faden),
        }
    }

    /// The number of connections that reached the socket. No waiting: it is read after an event that follows the
    /// arrival (the answer), or after a refusal that never goes to the socket.
    fn stand(&self) -> usize {
        self.anzahl.load(Ordering::SeqCst)
    }
}

impl Drop for Modellsocket {
    fn drop(&mut self) {
        self.stopp.store(true, Ordering::SeqCst);
        let _ = std::os::unix::net::UnixStream::connect(&self.socket);
        if let Some(f) = self.faden.take() {
            let _ = f.join();
        }
    }
}

#[test]
fn case_f_the_local_route_reaches_its_model_path_on_11434_and_no_other_port() {
    let welt = Welt::neu();
    let dienst = Zaehler::neu();
    let ordner = welt.wurzel.join("lokal");
    fs::create_dir_all(&ordner).expect("folder");
    // the model socket of the host: a service of the test that counts the connections and answers one line. The
    // forwarder ends with the program in the enclosure, so the probe must stay until the answer of this socket is
    // back: the answer is the event the test waits for, never a time.
    let socket = ordner.join("modell.sock");
    let modell = Modellsocket::neu(&socket);
    let mut b = welt.beschreibung();
    b.modell_socket = Some(socket.clone());
    b.lokal_weiter = Some(std::path::PathBuf::from(gemeinsam::WEITER));
    let args = Profil::bauen(&b).expect("builds").argumente();
    let p = |probe: &[&str]| welt.probe(&args, probe);
    // the connection on 11434 arrived at the model socket of the host and its answer came back through the forwarder
    assert_eq!(
        p(&["tcpfrage", "127.0.0.1:11434", "hallo"]),
        "tcpfrage: OK modell:hallo",
        "the forwarder passed the connection to the model socket"
    );
    assert_eq!(modell.stand(), 1);
    // the refusals below come from the network of the enclosure itself, before any forwarder: nothing is on the way
    assert_eq!(p(&["tcp", "127.0.0.1:8402"]), "tcp: ERR ECONNREFUSED");
    assert_eq!(p(&["tcp", "127.0.0.1:11435"]), "tcp: ERR ECONNREFUSED");
    assert_eq!(p(&["tcp", &dienst.adresse]), "tcp: ERR ECONNREFUSED");
    assert_eq!(
        dienst.stand(),
        0,
        "the loopback of the host is not reachable"
    );
    assert_eq!(modell.stand(), 1, "no other port reached the model socket");
    // counter probe: the same description without the forwarder has nothing on 11434
    b.lokal_weiter = None;
    let ohne = Profil::bauen(&b).expect("builds").argumente();
    assert_eq!(
        welt.probe(&ohne, &["tcp", "127.0.0.1:11434"]),
        "tcp: ERR ECONNREFUSED"
    );
}
