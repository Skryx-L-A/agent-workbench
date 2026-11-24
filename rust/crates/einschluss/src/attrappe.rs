//! The stand-in harness and the stand-in search process of the outside tests. This module exists only in builds with
//! the feature `attrappe`; the delivery build has neither it nor the program `einschluss-attrappe`
//! (`tests/auslieferung.rs` builds without the feature and looks for the marker below).
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

/// The stand-in search process: listens on `socket` and answers each line with [`antwort`]. It runs until it is
/// killed. It fails when the socket path is taken.
///
/// # Errors
/// The socket cannot be created.
pub fn such_server(socket: &Path, route: &str) -> io::Result<()> {
    let hoerer = UnixListener::bind(socket)?;
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
    let hoerer = UnixListener::bind(socket)?;
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
    let hoerer = UnixListener::bind(socket)?;
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
    let aus = Command::new("/usr/bin/bwrap")
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
fn frage_mit_umgebung(socket: &str, zeile: &str) -> String {
    let Ok(selbst) = std::env::current_exe() else {
        return "ERR exe".to_owned();
    };
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
        "frageumgebung" => frage_mit_umgebung(a(1)?, a(2)?),
        andere => return Err(format!("unknown probe {andere}")),
    };
    writeln!(aus, "{name}: {zeile}").map_err(|e| e.to_string())
}

/// The stand-in of the Unix sockets on a system without them (Windows), so that the stand-in harness builds there.
/// Binding and connecting fail with "not available": no value of these types can exist, so nothing else can run.
/// The probes and stand-ins that need a socket then answer with that error, the others work as on Unix.
#[cfg(not(unix))]
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
