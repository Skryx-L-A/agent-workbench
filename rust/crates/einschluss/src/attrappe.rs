//! The stand-in harness and the stand-in search process of the outside tests. This module exists only in builds with
//! the feature `attrappe` and in the unit tests of the crate; the delivery build has neither it nor the program
//! `einschluss-attrappe` (`tests/auslieferung.rs` builds without the feature and looks for the marker below). The
//! unit tests below check the probes and stand-ins themselves, so that they run in the default build too (and in the
//! mutation run, which builds without the feature).
//!
//! The stand-in harness makes one probe per call and writes one line `<probe>: <result>`. A result is `OK <detail>`
//! or `ERR <errno name>`. Nothing here is a real harness: it reads, writes, connects and asks, and the test judges
//! the line.

#[cfg(not(unix))]
use fremd::{UnixListener, UnixStream};
use std::collections::BTreeSet;
use std::fs;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
#[cfg(unix)]
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

/// A text that stands only in this module. A file of a delivery build that holds it is a leak of the stand-in.
pub const MARKER: &str = "einschluss-attrappe-marker-7f3a";

/// The names of the errors the probes meet.
#[must_use]
pub fn fehlername(e: &io::Error) -> String {
    match e.raw_os_error() {
        Some(1) => "EPERM".to_owned(),
        Some(2) => "ENOENT".to_owned(),
        Some(3) => "ESRCH".to_owned(),
        Some(13) => "EACCES".to_owned(),
        Some(20) => "ENOTDIR".to_owned(),
        Some(30) => "EROFS".to_owned(),
        Some(97) => "EAFNOSUPPORT".to_owned(),
        Some(101) => "ENETUNREACH".to_owned(),
        Some(110) => "ETIMEDOUT".to_owned(),
        Some(111) => "ECONNREFUSED".to_owned(),
        Some(113) => "EHOSTUNREACH".to_owned(),
        Some(n) => format!("E{n}"),
        None => format!("{:?}", e.kind()),
    }
}

fn ergebnis<T>(r: io::Result<T>, ok: impl FnOnce(T) -> String) -> String {
    match r {
        Ok(wert) => format!("OK {}", ok(wert)),
        Err(e) => format!("ERR {}", fehlername(&e)),
    }
}

/// The answer of the stand-in search process: the route comes from its argument, never from the request. A request
/// that names a route, a customer, a session or a class is `bad_request` (contract 4, fields of a request).
#[must_use]
pub fn antwort(route: &str, zeile: &str) -> String {
    let verboten = ["\"route\"", "\"kunde\"", "\"sitzung\"", "\"klasse\""];
    if verboten.iter().any(|v| zeile.contains(v)) {
        return "{\"error\":\"bad_request\"}".to_owned();
    }
    if zeile.contains("\"ping\"") {
        return "{\"ok\":true}".to_owned();
    }
    format!("{{\"route\":\"{route}\"}}")
}

/// Listens on `socket` and makes the path appear only once the socket listens. `UnixListener::bind` creates the path at
/// `bind` and listens a moment later; a test that waits for the path and connects in that moment gets `ECONNREFUSED`
/// (measured under load: `case_c` and `case_f` red with `frage: ERR ECONNREFUSED`). So the socket listens under a name
/// of its own next to `socket` first, then gets `socket` as a hard link, which fails when the path is taken.
fn hoeren(socket: &Path) -> io::Result<UnixListener> {
    let name = socket
        .file_name()
        .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidInput))?
        .to_string_lossy()
        .into_owned();
    let vorlaeufig = socket.with_file_name(format!(".{name}.{}", std::process::id()));
    let hoerer = UnixListener::bind(&vorlaeufig)?;
    let verknuepft = fs::hard_link(&vorlaeufig, socket);
    fs::remove_file(&vorlaeufig)?;
    verknuepft?;
    Ok(hoerer)
}

/// The stand-in search process: listens on `socket` and answers each line with [`antwort`]. It runs until it is
/// killed. It fails when the socket path is taken.
///
/// # Errors
/// The socket cannot be created.
pub fn such_server(socket: &Path, route: &str) -> io::Result<()> {
    let hoerer = hoeren(socket)?;
    for verbindung in hoerer.incoming() {
        let Ok(verbindung) = verbindung else { continue };
        let route = route.to_owned();
        std::thread::spawn(move || {
            let mut schreiber = &verbindung;
            for zeile in BufReader::new(&verbindung).lines() {
                let Ok(zeile) = zeile else { return };
                if writeln!(schreiber, "{}", antwort(&route, &zeile)).is_err() {
                    return;
                }
            }
        });
    }
    Ok(())
}

/// The answer of the stand-in relay of the cloud exit to one request line: it counts the places of the fixed
/// placeholder and says how long the line is after the swap for `marke` (the stand-in of the real key). The answer
/// holds neither the placeholder nor the mark.
#[must_use]
pub fn cloud_antwort(zeile: &str, platzhalter: &str, marke: &str) -> String {
    let getauscht = zeile.replace(platzhalter, marke);
    format!(
        "tausch={} laenge={}",
        zeile.matches(platzhalter).count(),
        getauscht.len()
    )
}

/// The stand-in relay of the cloud exit (outside): listens on `socket` and answers each line with [`cloud_antwort`].
/// It runs until it is killed.
///
/// # Errors
/// The socket cannot be created.
pub fn cloud_server(socket: &Path, platzhalter: &str, marke: &str) -> io::Result<()> {
    let hoerer = hoeren(socket)?;
    for verbindung in hoerer.incoming() {
        let Ok(verbindung) = verbindung else { continue };
        let (platzhalter, marke) = (platzhalter.to_owned(), marke.to_owned());
        std::thread::spawn(move || {
            let mut schreiber = &verbindung;
            for zeile in BufReader::new(&verbindung).lines() {
                let Ok(zeile) = zeile else { return };
                if writeln!(schreiber, "{}", cloud_antwort(&zeile, &platzhalter, &marke)).is_err() {
                    return;
                }
            }
        });
    }
    Ok(())
}

/// The stand-in relay outside the enclosure: listens on the Unix socket `socket`, passes each request line to the TCP
/// address `ziel` and the answer line back. It stands for a forwarder that is handed into the enclosure explicitly.
///
/// # Errors
/// The socket cannot be created.
pub fn weiter_server(socket: &Path, ziel: &str) -> io::Result<()> {
    let hoerer = hoeren(socket)?;
    for verbindung in hoerer.incoming() {
        let Ok(verbindung) = verbindung else { continue };
        let ziel = ziel.to_owned();
        std::thread::spawn(move || {
            let _ = zeile_weiterreichen(&verbindung, &ziel);
        });
    }
    Ok(())
}

fn zeile_weiterreichen(verbindung: &UnixStream, ziel: &str) -> io::Result<()> {
    let mut frage = String::new();
    BufReader::new(verbindung).read_line(&mut frage)?;
    let tcp = TcpStream::connect(ziel)?;
    tcp.set_read_timeout(Some(Duration::from_secs(3)))?;
    (&tcp).write_all(frage.as_bytes())?;
    let mut antwort = String::new();
    BufReader::new(&tcp).read_line(&mut antwort)?;
    let mut schreiber = verbindung;
    schreiber.write_all(antwort.as_bytes())
}

/// The bridge inside the enclosure: listens on the own loopback `127.0.0.1:port`, passes one request line to the
/// Unix socket `socket` and the answer back, and asks itself once with `zeile`. Returns the answer.
fn bruecke(socket: &str, port: &str, zeile: &str) -> io::Result<String> {
    let hoerer = TcpListener::bind(format!("127.0.0.1:{port}"))?;
    let adresse = hoerer.local_addr()?;
    let socket = socket.to_owned();
    let faden = std::thread::spawn(move || -> io::Result<()> {
        let (tcp, _) = hoerer.accept()?;
        let mut frage = String::new();
        BufReader::new(&tcp).read_line(&mut frage)?;
        let mut unix = UnixStream::connect(&socket)?;
        unix.set_read_timeout(Some(Duration::from_secs(3)))?;
        unix.write_all(frage.as_bytes())?;
        let mut antwort = String::new();
        BufReader::new(&unix).read_line(&mut antwort)?;
        (&tcp).write_all(antwort.as_bytes())
    });
    let mut kunde = TcpStream::connect(adresse)?;
    kunde.set_read_timeout(Some(Duration::from_secs(5)))?;
    writeln!(kunde, "{zeile}")?;
    let mut antwort = String::new();
    BufReader::new(&kunde).read_line(&mut antwort)?;
    let _ = faden.join();
    Ok(antwort.trim_end().to_owned())
}

/// Walks `wurzel` without following links, skipping the system folders, and counts the files that hold `nadel` and the
/// files whose name ends in `.md`. Returns (files seen, hits, notes).
fn durchsuchen(wurzel: &Path, nadel: &str) -> (u64, u64, u64) {
    let (mut gesehen, mut treffer, mut notizen) = (0, 0, 0);
    let mut stapel: Vec<PathBuf> = vec![wurzel.to_path_buf()];
    while let Some(pfad) = stapel.pop() {
        if ["/usr", "/proc", "/sys", "/dev"]
            .iter()
            .any(|s| pfad == Path::new(s))
        {
            continue;
        }
        let Ok(meta) = fs::symlink_metadata(&pfad) else {
            continue;
        };
        if meta.is_dir() {
            if let Ok(inhalt) = fs::read_dir(&pfad) {
                stapel.extend(inhalt.flatten().map(|e| e.path()));
            }
        } else if meta.is_file() {
            gesehen += 1;
            if pfad.extension().is_some_and(|e| e == "md") {
                notizen += 1;
            }
            if meta.len() <= 1 << 20
                && fs::read(&pfad).is_ok_and(|b| contains(&b, nadel.as_bytes()))
            {
                treffer += 1;
            }
        }
    }
    (gesehen, treffer, notizen)
}

/// How many variables of the own environment hold `nadel` in their value.
fn marke_in_umgebung(nadel: &str) -> usize {
    std::env::vars_os()
        .filter(|(_, wert)| contains(wert.as_encoded_bytes(), nadel.as_bytes()))
        .count()
}

/// The cloud request of a harness: reads the proxy address and the token from the own environment as the cloud
/// harness does, sends one line with the token through the proxy and returns the answer line.
fn cloud() -> String {
    let (Ok(proxy), Ok(token)) = (
        std::env::var("HTTPS_PROXY"),
        std::env::var("COPILOT_GITHUB_TOKEN"),
    ) else {
        return "ERR umgebung".to_owned();
    };
    let Some(adresse) = proxy
        .strip_prefix("http://")
        .and_then(|a| a.parse::<SocketAddr>().ok())
    else {
        return "ERR proxy".to_owned();
    };
    ergebnis(
        TcpStream::connect_timeout(&adresse, Duration::from_secs(2)),
        |mut s| {
            let _ = s.set_read_timeout(Some(Duration::from_secs(5)));
            let _ = writeln!(s, "TOKEN {token}");
            let mut antwort = String::new();
            let _ = BufReader::new(&s).read_line(&mut antwort);
            antwort.trim_end().to_owned()
        },
    )
}

fn contains(heu: &[u8], nadel: &[u8]) -> bool {
    !nadel.is_empty() && heu.windows(nadel.len()).any(|f| f == nadel)
}

fn frage(socket: &str, zeile: &str) -> String {
    let verbindung = UnixStream::connect(socket);
    ergebnis(verbindung, |mut s| {
        let _ = s.set_read_timeout(Some(Duration::from_secs(3)));
        let _ = writeln!(s, "{zeile}");
        let mut antwort = String::new();
        let _ = BufReader::new(&s).read_line(&mut antwort);
        antwort.trim_end().to_owned()
    })
}

fn tcp(adresse: &str) -> String {
    match adresse.parse::<SocketAddr>() {
        Ok(a) => ergebnis(
            TcpStream::connect_timeout(&a, Duration::from_secs(2)),
            |_| String::new(),
        ),
        Err(_) => "ERR adresse".to_owned(),
    }
}

/// Connects to `adresse`, sends one line and waits for the answer line. The probe ends only with the answer, so a test
/// that asks through a forwarder waits for the event "the answer came back", not for a time.
fn tcpfrage(adresse: &str, zeile: &str) -> String {
    match adresse.parse::<SocketAddr>() {
        Ok(a) => ergebnis(
            TcpStream::connect_timeout(&a, Duration::from_secs(2)),
            |mut s| {
                let _ = s.set_read_timeout(Some(Duration::from_secs(30)));
                let _ = writeln!(s, "{zeile}");
                let mut antwort = String::new();
                let _ = BufReader::new(&s).read_line(&mut antwort);
                antwort.trim_end().to_owned()
            },
        ),
        Err(_) => "ERR adresse".to_owned(),
    }
}

fn dns(name: &str) -> String {
    match (name, 443).to_socket_addrs() {
        Ok(mut a) => format!("OK {}", a.next().map_or(0, |_| 1)),
        Err(_) => "ERR lookup".to_owned(),
    }
}

fn prozesse() -> String {
    let namen: BTreeSet<u64> = fs::read_dir("/proc")
        .map(|d| {
            d.flatten()
                .filter_map(|e| e.file_name().to_str().and_then(|n| n.parse().ok()))
                .collect()
        })
        .unwrap_or_default();
    let liste: Vec<String> = namen.iter().map(u64::to_string).collect();
    format!("OK {}", liste.join(","))
}

fn signal(pid: &str) -> String {
    let aus = Command::new("/bin/sh")
        .args(["-c", "kill -0 \"$1\"", "sh", pid])
        .output();
    match aus {
        Ok(o) if o.status.success() => "OK".to_owned(),
        Ok(_) => "ERR kill".to_owned(),
        Err(e) => format!("ERR {}", fehlername(&e)),
    }
}

fn umgebung() -> String {
    let namen: BTreeSet<String> = std::env::vars_os()
        .filter_map(|(n, _)| n.into_string().ok())
        .collect();
    format!("OK {}", namen.into_iter().collect::<Vec<_>>().join(","))
}

/// Tries to build an enclosure of its own with `bwrap` (the way out the contract names: a looser enclosure).
fn bwrap_selbst() -> String {
    selbst_einschliessen(Path::new("/usr/bin/bwrap"))
}

/// Runs `programm` with the arguments of a looser enclosure and says how it ended: `OK`, `ERR status=<code>` or the
/// name of the error of the start.
fn selbst_einschliessen(programm: &Path) -> String {
    let aus = Command::new(programm)
        .args(["--unshare-user", "--ro-bind", "/", "/", "/usr/bin/true"])
        .env_clear()
        .output();
    match aus {
        Ok(o) if o.status.success() => "OK".to_owned(),
        Ok(o) => format!("ERR status={}", o.status.code().unwrap_or(-1)),
        Err(e) => format!("ERR {}", fehlername(&e)),
    }
}

/// Asks the socket in a child that has route variables of its own set to another route. The route of the answer
/// shows whether a variable of the harness changes it.
/// `selbst` is the program of the stand-in harness.
fn frage_mit_umgebung(selbst: &Path, socket: &str, zeile: &str) -> String {
    let aus = Command::new(selbst)
        .args(["harness", "frage", socket, zeile])
        .env_clear()
        .env("ROUTE", "lokal")
        .env("WB_ROUTE", "lokal")
        .env("SUCH_ROUTE", "lokal")
        .output();
    match aus {
        Ok(o) => String::from_utf8_lossy(&o.stdout)
            .trim_end()
            .strip_prefix("frage: ")
            .unwrap_or("ERR ausgabe")
            .to_owned(),
        Err(e) => format!("ERR {}", fehlername(&e)),
    }
}

/// Runs one probe of the stand-in harness and writes its line.
///
/// # Errors
/// The probe is unknown or lacks an argument (the text names it), or the line cannot be written.
pub fn lauf(args: &[String], aus: &mut dyn Write) -> Result<(), String> {
    let a = |i: usize| {
        args.get(i)
            .map(String::as_str)
            .ok_or_else(|| "argument missing".to_owned())
    };
    let name = a(0)?;
    let zeile = match name {
        "kennung" => format!("OK {MARKER}"),
        "lesen" => {
            let nadel = args.get(2).map_or("", String::as_str);
            ergebnis(fs::read(a(1)?), |b| {
                format!(
                    "len={} marker={}",
                    b.len(),
                    u8::from(contains(&b, nadel.as_bytes()))
                )
            })
        }
        "schreiben" => ergebnis(
            fs::OpenOptions::new()
                .append(true)
                .create(true)
                .open(a(1)?)
                .and_then(|mut f| f.write_all(b"x")),
            |()| String::new(),
        ),
        "liste" => ergebnis(fs::read_dir(a(1)?), |d| {
            let mut namen: Vec<String> = d
                .flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect();
            namen.sort();
            namen.join(",")
        }),
        "suche" => {
            let (gesehen, treffer, notizen) = durchsuchen(Path::new(a(2)?), a(1)?);
            format!("OK dateien={gesehen} treffer={treffer} md={notizen}")
        }
        "tcp" => tcp(a(1)?),
        "tcpfrage" => tcpfrage(a(1)?, a(2)?),
        "dns" => dns(a(1)?),
        "proz" => prozesse(),
        "signal" => signal(a(1)?),
        "environ" => ergebnis(
            fs::File::open(format!("/proc/{}/environ", a(1)?)),
            |mut f| {
                let mut b = Vec::new();
                let _ = f.read_to_end(&mut b);
                format!("len={}", b.len())
            },
        ),
        "umgebung" => umgebung(),
        "markeumgebung" => format!("OK {}", marke_in_umgebung(a(1)?)),
        "cloud" => cloud(),
        "bwrap" => bwrap_selbst(),
        "bruecke" => ergebnis(bruecke(a(1)?, a(2)?, a(3)?), |antwort| antwort),
        "frage" => frage(a(1)?, a(2)?),
        "frageumgebung" => match std::env::current_exe() {
            Ok(selbst) => frage_mit_umgebung(&selbst, a(1)?, a(2)?),
            Err(_) => "ERR exe".to_owned(),
        },
        andere => return Err(format!("unknown probe {andere}")),
    };
    writeln!(aus, "{name}: {zeile}").map_err(|e| e.to_string())
}

/// The program `einschluss-attrappe` without its streams: `args` are the arguments without the program name, `aus` is
/// standard output, `fehler` standard error. Returns the exit code: 0 done, 1 the stand-in search process or a
/// stand-in relay could not start, 2 a wrong call or an unknown probe. A stand-in search process or relay that starts
/// runs until it is killed.
pub fn programm(args: &[String], aus: &mut dyn Write, fehler: &mut dyn Write) -> u8 {
    let mut meldung = |text: &str| {
        let _ = writeln!(fehler, "einschluss-attrappe: {text}");
    };
    match args.first().map(String::as_str) {
        Some("harness") => {
            match lauf(&args[1..], aus).and_then(|()| aus.flush().map_err(|e| e.to_string())) {
                Ok(()) => 0,
                Err(text) => {
                    meldung(&text);
                    2
                }
            }
        }
        Some("such") if args.len() == 3 => match such_server(Path::new(&args[1]), &args[2]) {
            Ok(()) => 0,
            Err(e) => {
                meldung(&format!("search: {e}"));
                1
            }
        },
        Some("cloud") if args.len() == 4 => {
            match cloud_server(Path::new(&args[1]), &args[2], &args[3]) {
                Ok(()) => 0,
                Err(e) => {
                    meldung(&format!("cloud relay: {e}"));
                    1
                }
            }
        }
        Some("weiter") if args.len() == 3 => match weiter_server(Path::new(&args[1]), &args[2]) {
            Ok(()) => 0,
            Err(e) => {
                meldung(&format!("relay: {e}"));
                1
            }
        },
        _ => {
            meldung(
                "usage: harness <probe> [argument ...] | such <socket> <route> | weiter <socket> <address> | cloud <socket> <placeholder> <mark>",
            );
            2
        }
    }
}

/// The stand-in of the Unix sockets on a system without them (Windows), so that the stand-in harness builds there.
/// Binding and connecting fail with "not available": no value of these types can exist, so nothing else can run.
/// The probes and stand-ins that need a socket then answer with that error, the others work as on Unix. The unit tests
/// build it on every system, so that the mutation run (Linux) checks the two ways in.
#[cfg(any(not(unix), test))]
#[cfg_attr(unix, allow(dead_code))]
mod fremd {
    use std::io::{self, Read, Write};
    use std::path::Path;
    use std::time::Duration;

    fn nicht_verfuegbar() -> io::Error {
        io::Error::new(
            io::ErrorKind::Unsupported,
            "not available on this system: Unix sockets exist on Unix only",
        )
    }

    /// Never exists: [`UnixListener::bind`] always fails.
    pub enum UnixListener {}

    impl UnixListener {
        pub fn bind(_pfad: impl AsRef<Path>) -> io::Result<Self> {
            Err(nicht_verfuegbar())
        }

        pub fn incoming(&self) -> std::iter::Empty<io::Result<UnixStream>> {
            match *self {}
        }
    }

    /// Never exists: [`UnixStream::connect`] always fails.
    pub enum UnixStream {}

    impl UnixStream {
        pub fn connect(_pfad: impl AsRef<Path>) -> io::Result<Self> {
            Err(nicht_verfuegbar())
        }

        pub fn set_read_timeout(&self, _frist: Option<Duration>) -> io::Result<()> {
            match *self {}
        }
    }

    impl Read for &UnixStream {
        fn read(&mut self, _puffer: &mut [u8]) -> io::Result<usize> {
            match **self {}
        }
    }

    impl Write for &UnixStream {
        fn write(&mut self, _puffer: &[u8]) -> io::Result<usize> {
            match **self {}
        }

        fn flush(&mut self) -> io::Result<()> {
            match **self {}
        }
    }

    impl Read for UnixStream {
        fn read(&mut self, _puffer: &mut [u8]) -> io::Result<usize> {
            match *self {}
        }
    }

    impl Write for UnixStream {
        fn write(&mut self, _puffer: &[u8]) -> io::Result<usize> {
            match *self {}
        }

        fn flush(&mut self) -> io::Result<()> {
            match *self {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stand_in_search_takes_its_route_from_its_argument() {
        assert_eq!(
            antwort("cloud", "{\"op\":\"search\",\"query\":\"x\"}"),
            "{\"route\":\"cloud\"}"
        );
        assert_eq!(antwort("cloud", "{\"op\":\"ping\"}"), "{\"ok\":true}");
        for feld in ["route", "kunde", "sitzung", "klasse"] {
            let zeile = format!("{{\"op\":\"search\",\"{feld}\":\"lokal\"}}");
            assert_eq!(
                antwort("cloud", &zeile),
                "{\"error\":\"bad_request\"}",
                "{feld}"
            );
        }
    }

    #[test]
    fn the_error_names_are_stable() {
        let e = |n| fehlername(&io::Error::from_raw_os_error(n));
        assert_eq!(e(2), "ENOENT");
        assert_eq!(e(101), "ENETUNREACH");
        assert_eq!(e(111), "ECONNREFUSED");
        assert_eq!(e(1), "EPERM");
        assert_eq!(e(3), "ESRCH");
        assert_eq!(e(13), "EACCES");
        assert_eq!(e(9999), "E9999");
        assert_eq!(fehlername(&io::Error::other("x")), "Other");
    }

    #[test]
    fn the_line_probe_ends_with_the_answer_of_the_other_side() {
        let hoerer = TcpListener::bind("127.0.0.1:0").expect("listener");
        let adresse = hoerer.local_addr().expect("addr").to_string();
        let faden = std::thread::spawn(move || {
            let (mut s, _) = hoerer.accept().expect("accept");
            let mut zeile = String::new();
            BufReader::new(&s).read_line(&mut zeile).expect("line");
            s.write_all(format!("echo:{zeile}").as_bytes())
                .expect("answer");
        });
        let mut aus = Vec::new();
        lauf(
            &["tcpfrage".to_owned(), adresse, "hallo".to_owned()],
            &mut aus,
        )
        .expect("probe");
        assert_eq!(String::from_utf8_lossy(&aus), "tcpfrage: OK echo:hallo\n");
        faden.join().expect("thread");
        assert!(
            lauf(
                &["tcpfrage".to_owned(), "127.0.0.1:1".to_owned()],
                &mut Vec::new()
            )
            .is_err()
        );
    }

    #[test]
    fn a_probe_writes_one_line_and_an_unknown_one_fails() {
        let mut aus = Vec::new();
        lauf(&["kennung".to_owned()], &mut aus).expect("probe");
        assert_eq!(
            String::from_utf8_lossy(&aus),
            format!("kennung: OK {MARKER}\n")
        );
        assert!(lauf(&["nichts".to_owned()], &mut Vec::new()).is_err());
        assert!(lauf(&["lesen".to_owned()], &mut Vec::new()).is_err());
    }

    #[test]
    fn the_stand_in_relay_counts_the_placeholders_and_swaps_them_for_the_mark() {
        let platz = "gho_000";
        let marke = "gho_MARKE-LANG";
        assert_eq!(
            cloud_antwort("TOKEN gho_000", platz, marke),
            format!("tausch=1 laenge={}", "TOKEN gho_MARKE-LANG".len())
        );
        assert_eq!(
            cloud_antwort("a gho_000 b gho_000", platz, marke),
            format!(
                "tausch=2 laenge={}",
                "a gho_MARKE-LANG b gho_MARKE-LANG".len()
            )
        );
        assert_eq!(
            cloud_antwort("TOKEN gho_111", platz, marke),
            "tausch=0 laenge=13"
        );
        assert!(!cloud_antwort("gho_000", platz, marke).contains(marke));
    }

    #[test]
    fn the_probe_for_a_mark_counts_the_variables_that_hold_it() {
        // only reads the environment of the test process, which has PATH
        let pfad = std::env::var("PATH").expect("PATH");
        assert!(marke_in_umgebung(&pfad) >= 1);
        assert_eq!(marke_in_umgebung("kommt-in-keiner-variable-vor-9f2c"), 0);
        let mut aus = Vec::new();
        lauf(
            &[
                "markeumgebung".to_owned(),
                "kommt-in-keiner-variable-vor-9f2c".to_owned(),
            ],
            &mut aus,
        )
        .expect("probe");
        assert_eq!(String::from_utf8_lossy(&aus), "markeumgebung: OK 0\n");
    }

    #[test]
    fn the_cloud_probe_needs_the_proxy_and_the_token_of_its_environment() {
        // the test process has neither variable
        assert_eq!(cloud(), "ERR umgebung");
    }
}

/// The probes and stand-ins with sockets, processes and files of this test process (Unix). Every wait has a limit.
#[cfg(all(test, unix))]
mod pruefungen_unix {
    use super::*;
    use std::time::Instant;

    /// An own empty folder of this test below the temporary folder.
    fn ordner(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("ei-at-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).expect("folder");
        p
    }

    /// Waits until the socket `pfad` takes a connection; a stand-in that never listens fails the test after 10 s.
    fn warten(pfad: &Path) {
        let ende = Instant::now() + Duration::from_secs(10);
        while UnixStream::connect(pfad).is_err() {
            assert!(
                Instant::now() < ende,
                "nobody listens on {}",
                pfad.display()
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn text(pfad: &Path) -> String {
        pfad.to_string_lossy().into_owned()
    }

    fn zeichen(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn the_stand_in_search_answers_on_its_socket_and_refuses_a_taken_path() {
        let o = ordner("such");
        let socket = o.join("s");
        let s = socket.clone();
        std::thread::spawn(move || such_server(&s, "cloud-9"));
        warten(&socket);
        assert_eq!(
            frage(&text(&socket), "{\"op\":\"ping\"}"),
            "OK {\"ok\":true}"
        );
        assert_eq!(
            frage(&text(&socket), "{\"op\":\"search\"}"),
            "OK {\"route\":\"cloud-9\"}"
        );
        // the path is taken: a second stand-in does not start
        assert!(such_server(&socket, "x").is_err());
        assert_eq!(frage(&text(&o.join("fehlt")), "x"), "ERR ENOENT");
        let _ = fs::remove_dir_all(&o);
    }

    #[test]
    fn the_stand_in_relay_of_the_cloud_exit_answers_on_its_socket() {
        let o = ordner("cloud");
        let socket = o.join("c");
        let s = socket.clone();
        std::thread::spawn(move || cloud_server(&s, "gho_000", "gho_MARKE"));
        warten(&socket);
        assert_eq!(
            frage(&text(&socket), "TOKEN gho_000"),
            format!("OK tausch=1 laenge={}", "TOKEN gho_MARKE".len())
        );
        let _ = fs::remove_dir_all(&o);
    }

    /// A TCP listener on the own loopback that answers one line per connection with `echo:<line>`.
    fn echo() -> String {
        let hoerer = TcpListener::bind("127.0.0.1:0").expect("listener");
        let adresse = hoerer.local_addr().expect("addr").to_string();
        std::thread::spawn(move || {
            // one thread per connection: the probe of `warten` reaches the listener as an empty request too
            for verbindung in hoerer.incoming() {
                let Ok(mut s) = verbindung else { continue };
                std::thread::spawn(move || {
                    let mut zeile = String::new();
                    if BufReader::new(&s).read_line(&mut zeile).is_ok() {
                        let _ = s.write_all(format!("echo:{zeile}").as_bytes());
                    }
                });
            }
        });
        adresse
    }

    #[test]
    fn the_stand_in_relay_passes_a_line_to_the_tcp_address_and_the_answer_back() {
        let o = ordner("weiter");
        let socket = o.join("w");
        let s = socket.clone();
        let ziel = echo();
        std::thread::spawn(move || weiter_server(&s, &ziel));
        warten(&socket);
        assert_eq!(frage(&text(&socket), "hallo"), "OK echo:hallo");
        let _ = fs::remove_dir_all(&o);
    }

    #[test]
    fn the_bridge_passes_its_line_to_the_socket_and_returns_the_answer() {
        let o = ordner("bruecke");
        let socket = o.join("b");
        let s = socket.clone();
        std::thread::spawn(move || such_server(&s, "lokal-3"));
        warten(&socket);
        let mut aus = Vec::new();
        lauf(
            &zeichen(&["bruecke", &text(&socket), "0", "{\"op\":\"search\"}"]),
            &mut aus,
        )
        .expect("probe");
        assert_eq!(
            String::from_utf8_lossy(&aus),
            "bruecke: OK {\"route\":\"lokal-3\"}\n"
        );
        let _ = fs::remove_dir_all(&o);
    }

    #[test]
    fn the_search_counts_files_hits_and_notes_without_links_and_big_files() {
        let o = ordner("suche");
        let nadel = "NADEL-5k2";
        fs::write(o.join("a.md"), format!("x {nadel} y")).expect("a");
        fs::write(o.join("b.txt"), "nichts").expect("b");
        fs::create_dir_all(o.join("sub")).expect("sub");
        fs::write(o.join("sub/c.md"), "nichts").expect("c");
        let mut genau = vec![b' '; 1 << 20];
        genau[..nadel.len()].copy_from_slice(nadel.as_bytes());
        fs::write(o.join("sub/genau.txt"), &genau).expect("exactly the limit");
        genau.push(b' ');
        fs::write(o.join("sub/gross.txt"), &genau).expect("one byte over");
        std::os::unix::fs::symlink(o.join("a.md"), o.join("verweis.md")).expect("link");
        // five files: a.md and genau.txt hold the needle, gross.txt is over the limit, the link is not followed
        assert_eq!(durchsuchen(&o, nadel), (5, 2, 2));
        let mut aus = Vec::new();
        lauf(&zeichen(&["suche", nadel, &text(&o)]), &mut aus).expect("probe");
        assert_eq!(
            String::from_utf8_lossy(&aus),
            "suche: OK dateien=5 treffer=2 md=2\n"
        );
        // the system folders are skipped
        assert_eq!(durchsuchen(Path::new("/dev"), nadel), (0, 0, 0));
        let _ = fs::remove_dir_all(&o);
    }

    #[test]
    fn the_search_for_a_needle_takes_only_a_true_part() {
        assert!(contains(b"abc", b"bc"));
        assert!(contains(b"abc", b"abc"));
        assert!(!contains(b"abc", b"x"));
        assert!(!contains(b"abc", b"abcd"));
        assert!(!contains(b"abc", b""));
    }

    #[test]
    fn the_connection_probes_name_their_end() {
        let hoerer = TcpListener::bind("127.0.0.1:0").expect("listener");
        let adresse = hoerer.local_addr().expect("addr").to_string();
        assert_eq!(tcp(&adresse), "OK ");
        assert_eq!(tcp("keine adresse"), "ERR adresse");
        assert_eq!(tcpfrage("keine adresse", "x"), "ERR adresse");
        drop(hoerer);
        assert_eq!(dns("127.0.0.1"), "OK 1");
        let mut aus = Vec::new();
        lauf(&zeichen(&["dns", "127.0.0.1"]), &mut aus).expect("probe");
        assert_eq!(String::from_utf8_lossy(&aus), "dns: OK 1\n");
    }

    #[test]
    fn the_signal_probe_reaches_only_a_living_process() {
        assert_eq!(signal(&std::process::id().to_string()), "OK");
        assert_eq!(signal("999999999"), "ERR kill");
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn the_process_probe_lists_the_own_process() {
        let zeile = prozesse();
        let liste = zeile.strip_prefix("OK ").expect("OK");
        let eigen = std::process::id().to_string();
        assert!(liste.split(',').any(|n| n == eigen), "{zeile}");
    }

    #[test]
    fn the_environment_probe_names_the_variables() {
        let zeile = umgebung();
        let namen = zeile.strip_prefix("OK ").expect("OK");
        assert!(namen.split(',').any(|n| n == "PATH"), "{zeile}");
    }

    #[test]
    fn the_probe_of_an_own_enclosure_says_how_it_ended() {
        assert_eq!(selbst_einschliessen(Path::new("/usr/bin/true")), "OK");
        assert_eq!(
            selbst_einschliessen(Path::new("/usr/bin/false")),
            "ERR status=1"
        );
        assert_eq!(
            selbst_einschliessen(Path::new("/var/empty/kein-programm")),
            "ERR ENOENT"
        );
        let bwrap = bwrap_selbst();
        assert!(bwrap == "OK" || bwrap.starts_with("ERR "), "{bwrap}");
    }

    #[test]
    fn the_question_with_route_variables_takes_the_line_of_the_child() {
        // the child is not the stand-in here: its output has no line of the probe
        assert_eq!(
            frage_mit_umgebung(Path::new("/bin/echo"), "s", "z"),
            "ERR ausgabe"
        );
        assert_eq!(
            frage_mit_umgebung(Path::new("/var/empty/kein-programm"), "s", "z"),
            "ERR ENOENT"
        );
    }

    /// The program on the arguments `args`: exit code, standard output, standard error.
    fn programm_mit(args: &[&str]) -> (u8, String, String) {
        let (mut aus, mut fehler) = (Vec::new(), Vec::new());
        let code = programm(&zeichen(args), &mut aus, &mut fehler);
        (
            code,
            String::from_utf8_lossy(&aus).into_owned(),
            String::from_utf8_lossy(&fehler).into_owned(),
        )
    }

    #[test]
    fn the_program_takes_exactly_its_four_calls() {
        assert_eq!(
            programm_mit(&["harness", "kennung"]),
            (0, format!("kennung: OK {MARKER}\n"), String::new())
        );
        assert_eq!(
            programm_mit(&["harness", "nichts"]),
            (
                2,
                String::new(),
                "einschluss-attrappe: unknown probe nichts\n".to_owned()
            )
        );
        let fehlt = "/var/empty/kein-ordner/s";
        // the three stand-ins with their number of arguments: a socket that cannot be made ends with 1
        for (args, wort) in [
            (&["such", fehlt, "r"][..], "search: "),
            (&["cloud", fehlt, "p", "m"][..], "cloud relay: "),
            (&["weiter", fehlt, "127.0.0.1:1"][..], "relay: "),
        ] {
            let (code, aus, fehler) = programm_mit(args);
            assert_eq!((code, aus.as_str()), (1, ""), "{args:?}");
            assert!(
                fehler.starts_with(&format!("einschluss-attrappe: {wort}")),
                "{args:?}: {fehler}"
            );
        }
        // one argument more or less is the usage line
        for args in [
            &[][..],
            &["such", fehlt][..],
            &["such", fehlt, "r", "x"][..],
            &["cloud", fehlt, "p"][..],
            &["cloud", fehlt, "p", "m", "x"][..],
            &["weiter", fehlt][..],
            &["weiter", fehlt, "a", "x"][..],
            &["anders"][..],
        ] {
            let (code, aus, fehler) = programm_mit(args);
            assert_eq!((code, aus.as_str()), (2, ""), "{args:?}");
            assert!(
                fehler.starts_with("einschluss-attrappe: usage: harness <probe>"),
                "{args:?}: {fehler}"
            );
        }
    }

    #[test]
    fn more_error_names_are_stable() {
        let e = |n| fehlername(&io::Error::from_raw_os_error(n));
        assert_eq!(e(20), "ENOTDIR");
        assert_eq!(e(30), "EROFS");
        assert_eq!(e(97), "EAFNOSUPPORT");
        assert_eq!(e(110), "ETIMEDOUT");
        assert_eq!(e(113), "EHOSTUNREACH");
    }
}

/// The stand-in of the Unix sockets refuses both ways in, on every system.
#[cfg(test)]
mod pruefungen_fremd {
    use super::fremd;
    use std::io;

    #[test]
    fn the_socket_stand_in_binds_and_connects_nothing() {
        let e = fremd::UnixListener::bind("/srv/beispiel/s")
            .err()
            .map(|e| e.kind());
        assert_eq!(e, Some(io::ErrorKind::Unsupported));
        let e = fremd::UnixStream::connect("/srv/beispiel/s")
            .err()
            .map(|e| e.kind());
        assert_eq!(e, Some(io::ErrorKind::Unsupported));
    }
}
