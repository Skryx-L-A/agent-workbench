//! The question embedded in the process, against a stand-in for Ollama on a free loopback port.
//!
//! - `Embedder::embed` against the stand-in: the request it sends (head and body), every kind of
//!   failure, the one more try, the deadline, `chunked` and `Content-Length`, a long question, no
//!   connection left open.
//! - `search --embed` against `search --query-vector` with the vector the stand-in gives: 320
//!   invented states of the databases and of Ollama; output and exit code equal to the byte.
//! - `serve --embed` with requests that carry `query_vector` and requests that do not.
//! - the options: every address that is no loopback address, the options that exclude each other.
//!
//! The stand-in is the only listener; it is stopped at the end of each test and the port is checked
//! to be closed. Nothing here talks to a real Ollama, and every vault and database is invented.

use std::fs;
use std::io::{BufRead, Cursor, ErrorKind, Read, Write};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, TcpListener, TcpStream};
use std::panic::Location;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{
    Arc, Mutex, MutexGuard, Once, OnceLock, PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard,
    mpsc,
};
use std::thread::{self, JoinHandle, ThreadId, sleep};
use std::time::{Duration, Instant};

use rangkern::embed::{BODY_MAX, Embedder, HEAD_LINE_MAX, HEAD_LINES_MAX, MAX_CHARS};
use rusqlite::Connection;
use serde_json::Value;

/// The length of the vectors of the stand-in.
const DIM: usize = 5;

/// What the stand-in does with one call.
#[derive(Clone, Debug)]
enum Act {
    /// 200 with the vector of the input, `Content-Length`.
    Vector,
    /// 200 with this body, `Content-Length`.
    Json(String),
    /// 200 with this body in `chunked` pieces of this many bytes.
    Chunked(String, usize),
    /// This status with this body, `Content-Length`.
    Status(u16, String),
    /// These bytes, then the connection is closed.
    Raw(Vec<u8>),
    /// The connection is closed after the request is read.
    Close,
    /// The request is read and never answered.
    Hang,
    /// Wait this many milliseconds, then do the other.
    Delay(u64, Box<Act>),
    /// 200 with this body, `Content-Length`, in pieces of this many bytes, a pause of this many
    /// milliseconds between two pieces.
    Trickle(String, u64, usize),
    /// These bytes in this order, a pause of this many milliseconds after each piece; then the
    /// stand-in waits for the client to close.
    Pieces(Vec<(Vec<u8>, u64)>),
    /// 200 in `chunked` pieces of one byte, a pause of `pause` milliseconds after each: the bytes of
    /// `OK_BODY`, then spaces. The last chunk comes only when `until` milliseconds have passed since the
    /// head was sent, so the answer is not complete earlier than that, and the load of the machine can
    /// only make it later, never earlier. If the client closes the connection first, the stand-in stops.
    Drip { pause: u64, until: u64 },
}

/// One call as the stand-in saw it.
#[derive(Clone, Debug)]
struct Call {
    head: String,
    body: String,
    /// Whether the client closed the connection after the answer; `None` if the stand-in closed it
    /// or never answered.
    client_closed: Option<bool>,
    /// Whether the stand-in is through with this call.
    done: bool,
}

/// The stand-in for Ollama.
struct Fake {
    addr: SocketAddr,
    calls: Arc<Mutex<Vec<Call>>>,
    stop: Arc<AtomicBool>,
    /// The accepting thread; it hands the listener back, so that it is closed under
    /// `no_program_starts` and not inside the thread.
    accept: Option<JoinHandle<TcpListener>>,
    /// The claim on the port, held until the stand-in is dropped: `finish` checks the port while it
    /// is still held.
    _claim: Option<Claim>,
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// Reads one request: the head and the body of the length it names.
fn read_request(stream: &mut TcpStream, stop: &AtomicBool) -> Option<(String, String)> {
    let mut data = Vec::new();
    let mut chunk = [0_u8; 4096];
    let end = loop {
        if let Some(at) = find(&data, b"\r\n\r\n") {
            break at + 4;
        }
        if stop.load(Ordering::Relaxed) {
            return None;
        }
        match stream.read(&mut chunk) {
            Ok(0) => return None,
            Ok(n) => data.extend_from_slice(&chunk[..n]),
            Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
            Err(_) => return None,
        }
    };
    let head = String::from_utf8_lossy(&data[..end]).into_owned();
    let length: usize = head
        .lines()
        .find_map(|line| line.strip_prefix("Content-Length: "))
        .and_then(|value| value.trim().parse().ok())
        .unwrap_or(0);
    while data.len() < end + length {
        if stop.load(Ordering::Relaxed) {
            return None;
        }
        match stream.read(&mut chunk) {
            Ok(0) => return None,
            Ok(n) => data.extend_from_slice(&chunk[..n]),
            Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
            Err(_) => return None,
        }
    }
    let body = String::from_utf8_lossy(&data[end..end + length]).into_owned();
    Some((head, body))
}

/// The vector of a text: deterministic, with numbers that print and parse back exactly.
fn vector_for(text: &str) -> Vec<f64> {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in text.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    (0..DIM)
        .map(|_| {
            h ^= h >> 29;
            h = h.wrapping_mul(0xbf58_476d_1ce4_e5b9);
            // Thousandths: the double nearest to a number of at most four digits prints with those
            // digits, so that every reader of the text gets the same double.
            let thousandths = i32::try_from((h >> 40) % 2001).expect("small") - 1000;
            f64::from(thousandths) / 1000.0
        })
        .collect()
}

/// The text of a vector as a JSON list.
fn list_text(vector: &[f64]) -> String {
    let numbers: Vec<String> = vector.iter().map(ToString::to_string).collect();
    format!("[{}]", numbers.join(", "))
}

/// The body of an answer with this vector.
fn answer_body(vector: &[f64]) -> String {
    format!("{{\"embeddings\": [{}]}}", list_text(vector))
}

fn sleep_until_stopped(millis: u64, stop: &AtomicBool) {
    let end = Instant::now() + Duration::from_millis(millis);
    while Instant::now() < end && !stop.load(Ordering::Relaxed) {
        sleep(Duration::from_millis(5));
    }
}

fn head_200(extra: &str) -> String {
    format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n{extra}\r\n")
}

/// Does `act` on the connection; `false` if the stand-in closes the connection itself.
fn perform(stream: &mut TcpStream, act: &Act, input: &str, stop: &AtomicBool) -> bool {
    match act {
        Act::Vector => {
            let body = answer_body(&vector_for(input));
            let text = format!(
                "{}{body}",
                head_200(&format!("Content-Length: {}\r\n", body.len()))
            );
            let _ = stream.write_all(text.as_bytes());
            true
        }
        Act::Json(body) => {
            let text = format!(
                "{}{body}",
                head_200(&format!("Content-Length: {}\r\n", body.len()))
            );
            let _ = stream.write_all(text.as_bytes());
            true
        }
        Act::Chunked(body, piece) => {
            let mut text = head_200("Transfer-Encoding: chunked\r\n").into_bytes();
            for part in body.as_bytes().chunks(*piece) {
                text.extend_from_slice(format!("{:x}\r\n", part.len()).as_bytes());
                text.extend_from_slice(part);
                text.extend_from_slice(b"\r\n");
            }
            text.extend_from_slice(b"0\r\n\r\n");
            let _ = stream.write_all(&text);
            true
        }
        Act::Status(status, body) => {
            let text = format!(
                "HTTP/1.1 {status} Reason\r\nContent-Length: {}\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(text.as_bytes());
            true
        }
        Act::Raw(bytes) => {
            let _ = stream.write_all(bytes);
            false
        }
        Act::Close => false,
        Act::Hang => {
            sleep_until_stopped(60_000, stop);
            false
        }
        Act::Delay(millis, inner) => {
            sleep_until_stopped(*millis, stop);
            perform(stream, inner, input, stop)
        }
        Act::Pieces(pieces) => {
            for (bytes, pause) in pieces {
                let _ = stream.write_all(bytes);
                let _ = stream.flush();
                sleep_until_stopped(*pause, stop);
            }
            true
        }
        Act::Drip { pause, until } => {
            let start = Instant::now();
            let head = head_200("Transfer-Encoding: chunked\r\n");
            if stream.write_all(head.as_bytes()).is_err() {
                return true;
            }
            let mut sent = 0;
            while !stop.load(Ordering::Relaxed) {
                if sent >= OK_BODY.len() && start.elapsed() >= Duration::from_millis(*until) {
                    let _ = stream.write_all(b"0\r\n\r\n");
                    break;
                }
                let byte = OK_BODY.as_bytes().get(sent).copied().unwrap_or(b' ');
                if stream
                    .write_all(&[b'1', b'\r', b'\n', byte, b'\r', b'\n'])
                    .is_err()
                {
                    break;
                }
                sent += 1;
                sleep(Duration::from_millis(*pause));
            }
            true
        }
        Act::Trickle(body, gap, piece) => {
            let head = head_200(&format!("Content-Length: {}\r\n", body.len()));
            let _ = stream.write_all(head.as_bytes());
            for part in body.as_bytes().chunks(*piece) {
                let _ = stream.write_all(part);
                let _ = stream.flush();
                sleep_until_stopped(*gap, stop);
            }
            true
        }
    }
}

/// One connection: reads its request, records the call and does the act the plan has for it. A
/// connection that sends no request is no call: a test program checks with a connect that its port is
/// closed (`finish`), and this stand-in may be the one that answers if the port number is reused.
fn handle(stream: TcpStream, plan: &[Act], calls: &Mutex<Vec<Call>>, stop: &AtomicBool) {
    if let Some(index) = serve_call(stream, plan, calls, stop) {
        calls.lock().expect("lock")[index].done = true;
    }
}

fn serve_call(
    mut stream: TcpStream,
    plan: &[Act],
    calls: &Mutex<Vec<Call>>,
    stop: &AtomicBool,
) -> Option<usize> {
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_nodelay(true);
    let _ = stream.set_read_timeout(Some(Duration::from_millis(20)));
    let (head, body) = read_request(&mut stream, stop)?;
    let input = serde_json::from_str::<Value>(&body)
        .ok()
        .and_then(|value| {
            value
                .get("input")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_default();
    // Call number `n` is the n-th request, not the n-th connection: the plan counts requests.
    let (index, act) = {
        let mut all = calls.lock().expect("lock");
        let index = all.len();
        all.push(Call {
            head,
            body,
            client_closed: None,
            done: false,
        });
        let act = plan
            .get(index)
            .or_else(|| plan.last())
            .cloned()
            .unwrap_or(Act::Vector);
        (index, act)
    };
    if !perform(&mut stream, &act, &input, stop) {
        return Some(index);
    }
    // The client must close the connection; the stand-in waits for that.
    let end = Instant::now() + Duration::from_secs(5);
    let mut chunk = [0_u8; 64];
    let closed = loop {
        match stream.read(&mut chunk) {
            Ok(0) => break true,
            Ok(_) => {}
            Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
            Err(_) => break true,
        }
        if Instant::now() >= end || stop.load(Ordering::Relaxed) {
            break false;
        }
    };
    calls.lock().expect("lock")[index].client_closed = Some(closed);
    Some(index)
}

impl Fake {
    /// Starts the stand-in; call number `n` does `plan[n]`, and every call after the plan does its
    /// last act.
    fn start(plan: Vec<Act>) -> Self {
        Self::start_at(IpAddr::V4(Ipv4Addr::LOCALHOST), plan).expect("bind")
    }

    /// As `start`, on this address; `None` if it cannot be bound (no IPv6 loopback on the machine).
    fn start_at(ip: IpAddr, plan: Vec<Act>) -> Option<Self> {
        watchdog();
        let (listener, claim) = {
            let _no_start = no_program_starts();
            bind_free(ip)?
        };
        Some(Self::on(listener, Some(claim), plan))
    }

    /// As `start`, on a listener that is already bound; `claim` is the claim on its port, if this
    /// stand-in is to hold it.
    fn on(listener: TcpListener, claim: Option<Claim>, plan: Vec<Act>) -> Self {
        listener.set_nonblocking(true).expect("nonblocking");
        let addr = listener.local_addr().expect("address");
        let calls = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let (calls2, stop2) = (Arc::clone(&calls), Arc::clone(&stop));
        let plan = Arc::new(plan);
        let accept = thread::spawn(move || {
            let mut handlers = Vec::new();
            while !stop2.load(Ordering::Relaxed) {
                let accepted = {
                    let _no_start = no_program_starts();
                    listener.accept()
                };
                match accepted {
                    Ok((stream, _)) => {
                        let (calls3, stop3, plan3) =
                            (Arc::clone(&calls2), Arc::clone(&stop2), Arc::clone(&plan));
                        handlers.push(thread::spawn(move || {
                            handle(stream, &plan3, &calls3, &stop3);
                        }));
                    }
                    Err(e) if e.kind() == ErrorKind::WouldBlock => sleep(Duration::from_millis(1)),
                    Err(_) => break,
                }
            }
            for handler in handlers {
                let _ = handler.join();
            }
            listener
        });
        Self {
            addr,
            calls,
            stop,
            accept: Some(accept),
            _claim: claim,
        }
    }

    fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.addr.port())
    }

    /// How many calls came in so far.
    fn count(&self) -> usize {
        self.calls.lock().expect("lock").len()
    }

    /// Waits until the stand-in is through with each of the first `n` calls (it has seen the client
    /// close the connection, or it closed the connection itself).
    fn wait_closed(&self, n: usize) {
        let end = Instant::now() + Duration::from_secs(5);
        while Instant::now() < end {
            let all = self.calls.lock().expect("lock");
            if all.len() >= n && all[..n].iter().all(|call| call.done) {
                return;
            }
            drop(all);
            sleep(Duration::from_millis(2));
        }
    }

    /// Stops the stand-in, checks that its port is closed, and returns the calls.
    fn finish(mut self) -> Vec<Call> {
        watchdog();
        self.stop.store(true, Ordering::Relaxed);
        if let Some(accept) = self.accept.take() {
            let listener = accept.join().expect("stand-in ends");
            let _no_start = no_program_starts();
            drop(listener);
        }
        if let Ok(mut stream) = TcpStream::connect_timeout(&self.addr, Duration::from_millis(500)) {
            // A connection from the port to itself (simultaneous open) is no listener.
            if stream.local_addr().ok() != Some(self.addr) {
                // A connect that completed is not yet an answer of a listener: the kernel may finish
                // the handshake of a port that closes at that moment and reset the connection at once
                // (measured on the Linux build host: the retry a millisecond later was refused, `ss` showed no
                // listener, and no other test program had bound the port). A listener, or a copy of its
                // descriptor in another process, keeps the connection open and silent; a closed port
                // ends or resets it. So the port is open only if the connection stays.
                let _ = stream.set_read_timeout(Some(Duration::from_millis(300)));
                let open = match stream.read(&mut [0_u8; 1]) {
                    Ok(count) => count > 0,
                    Err(e) => matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut),
                };
                assert!(
                    !open,
                    "the port of the stand-in is still open: {:?} reached {:?}",
                    stream.local_addr(),
                    stream.peer_addr()
                );
            }
        }
        self.calls.lock().expect("lock").clone()
    }
}

impl Drop for Fake {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(accept) = self.accept.take()
            && let Ok(listener) = accept.join()
        {
            let _no_start = no_program_starts();
            drop(listener);
        }
    }
}

/// Starting a program and creating or closing a socket of this test program never overlap. While
/// the system starts a program, the new process holds a copy of every descriptor of the test program
/// until `exec` closes the close-on-exec ones. A listener closed in that time still answers through
/// the copy, so the check in `finish` (or a `closed_port`) reaches it. On macOS, where a socket gets
/// its close-on-exec flag only after it was created, a copy taken in between would even outlive
/// `exec`. Measured on the Mac (19.11.2026, 20 parallel runs each): with the two tests that start
/// `rangkern`, 11 runs red with this check; without them, none apart from port 22000 (the ports are
/// picked by the system now, see `bind_free`).
static PROGRAM_START: RwLock<()> = RwLock::new(());

/// Held while a socket is created or closed: no program starts meanwhile.
fn no_program_starts() -> RwLockReadGuard<'static, ()> {
    PROGRAM_START.read().unwrap_or_else(PoisonError::into_inner)
}

/// Held while a program starts: no socket is created or closed meanwhile.
fn program_starts() -> RwLockWriteGuard<'static, ()> {
    PROGRAM_START
        .write()
        .unwrap_or_else(PoisonError::into_inner)
}

/// Starts a program while no socket of this test program is created or closed.
fn start_program(command: &mut std::process::Command) -> std::io::Result<std::process::Child> {
    let _start = program_starts();
    command.spawn()
}

/// A claim on a port number: a folder in `claims_root`, made with one atomic call, so that of all the
/// test programs of this file that run at once (other worktrees, other workers, `cargo-mutants`) only
/// one has the port. It is held from after the `bind` until after the check that the port is closed;
/// without it another test program may get the port number the system has just taken back, and the
/// check or a `closed_port` reaches its stand-in. The system picks the port (`bind_free` binds port 0);
/// the claim keeps the number of a port that is given back from the next `bind` of another program.
/// A program that holds claims marks them alive every few seconds (`watch`); a folder that nobody has
/// marked for `CLAIM_STALE` was left by a program that was killed; it is taken over.
struct Claim(Option<PathBuf>);

/// The folders of the claims this program holds; `watch` marks them alive and removes them when it
/// ends the program.
static HELD: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

fn held() -> MutexGuard<'static, Vec<PathBuf>> {
    HELD.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Drop for Claim {
    fn drop(&mut self) {
        if let Some(path) = self.0.take() {
            let mut held = held();
            held.retain(|other| *other != path);
            let _ = fs::remove_dir(path);
        }
    }
}

/// How long a claim may go unmarked to be taken over.
const CLAIM_STALE: Duration = Duration::from_secs(90);

/// The folder that holds the claims: `rangkern-embed-ports` in the temporary folder of the run
/// (`std::env::temp_dir()`: `TMPDIR` where it is set, otherwise `/tmp`), made with the mode 0700. A
/// folder that exists already is used only if it is a real folder (no link) of the user who runs this
/// program and has no right for group or others (a folder of this user with more rights gets the
/// mode 0700); in `/tmp`, which anyone may write, another user could have made it first. Otherwise
/// there is no claim and the ports are used without one. `None` also if the temporary folder cannot
/// be written.
fn claims_root() -> Option<&'static Path> {
    static ROOT: OnceLock<Option<PathBuf>> = OnceLock::new();
    ROOT.get_or_init(|| {
        let root = std::env::temp_dir().join("rangkern-embed-ports");
        private_folder(&root).then_some(root)
    })
    .as_deref()
}

#[cfg(unix)]
fn private_folder(root: &Path) -> bool {
    use std::fs::DirBuilder;
    use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
    match DirBuilder::new().mode(0o700).create(root) {
        Ok(()) => {}
        Err(e) if e.kind() == ErrorKind::AlreadyExists => {}
        Err(_) => return false,
    }
    // The user of this program is the owner of a folder it makes itself.
    let probe = root.with_file_name(format!("rangkern-embed-user-{}", std::process::id()));
    let own = match fs::create_dir(&probe) {
        Ok(()) => {
            let uid = fs::metadata(&probe).map(|meta| meta.uid());
            let _ = fs::remove_dir(&probe);
            uid.ok()
        }
        Err(_) => None,
    };
    let Ok(meta) = fs::symlink_metadata(root) else {
        return false;
    };
    if !meta.is_dir() || own != Some(meta.uid()) {
        return false;
    }
    // No right for group or others: the six lowest bits of the mode are 0. A folder of this user that
    // an earlier version of this file made with the mode 0755 gets the mode 0700: only this user could
    // have changed it, so nothing that someone else made is trusted.
    if meta.mode().trailing_zeros() < 6 {
        let _ = fs::set_permissions(root, fs::Permissions::from_mode(0o700));
    }
    fs::symlink_metadata(root).is_ok_and(|meta| meta.mode().trailing_zeros() >= 6)
}

#[cfg(not(unix))]
fn private_folder(root: &Path) -> bool {
    fs::create_dir_all(root).is_ok()
}

/// The claim on this port; `None` if another test program holds it. Where there is no folder for the
/// claims (see `claims_root`) the claim is empty and the port is used without one.
fn claim(port: u16) -> Option<Claim> {
    let Some(root) = claims_root() else {
        return Some(Claim(None));
    };
    let path = root.join(port.to_string());
    for _ in 0..2 {
        let mut held = held();
        match fs::create_dir(&path) {
            Ok(()) => {
                held.push(path.clone());
                return Some(Claim(Some(path)));
            }
            Err(e) if e.kind() == ErrorKind::AlreadyExists => {
                let stale = fs::metadata(&path)
                    .and_then(|meta| meta.modified())
                    .ok()
                    .and_then(|made| made.elapsed().ok())
                    .is_some_and(|age| age > CLAIM_STALE);
                if !stale || fs::remove_dir(&path).is_err() {
                    return None;
                }
            }
            Err(_) => return Some(Claim(None)),
        }
    }
    None
}

/// A listener on a free port of this address that the system picks (port 0), with the claim on its
/// port number. A number that another test program has claimed (it gave the port back and checks
/// that it is closed) is not used: the listener is kept until a number is found, so that the system
/// does not offer the same one again, and then closed with the others. `None` if the address cannot
/// be bound at all (no IPv6 loopback). The caller holds `no_program_starts`.
fn bind_free(ip: IpAddr) -> Option<(TcpListener, Claim)> {
    let mut refused = Vec::new();
    for _ in 0..200 {
        let listener = TcpListener::bind((ip, 0)).ok()?;
        let port = listener.local_addr().ok()?.port();
        if let Some(claim) = claim(port) {
            return Some((listener, claim));
        }
        refused.push(listener);
    }
    None
}

/// A bound TCP socket that does not listen. A connect to its address is refused at once (the system
/// answers with a reset), and no other program can listen on that address while it lives: a
/// listener of `std` sets `SO_REUSEADDR`, this socket does not, so its `bind` fails. Unix only.
#[cfg(unix)]
struct Refusing {
    _fd: std::os::fd::OwnedFd,
}

/// A [`Refusing`] socket on this address, close-on-exec. The caller holds `no_program_starts`
/// (the flag comes after the socket, as on macOS there is no flag at creation).
#[cfg(unix)]
fn refusing(addr: SocketAddr) -> rustix::io::Result<Refusing> {
    use rustix::io::{FdFlags, fcntl_setfd};
    use rustix::net::{AddressFamily, SocketType, bind, socket};
    let family = if addr.is_ipv4() {
        AddressFamily::INET
    } else {
        AddressFamily::INET6
    };
    let fd = socket(family, SocketType::STREAM, None)?;
    fcntl_setfd(&fd, FdFlags::CLOEXEC)?;
    bind(&fd, &addr)?;
    Ok(Refusing { _fd: fd })
}

/// The machine has no IPv6 loopback address: binding `[::1]` cannot work at all.
#[cfg(unix)]
fn no_ipv6(e: rustix::io::Errno) -> bool {
    e == rustix::io::Errno::ADDRNOTAVAIL || e == rustix::io::Errno::AFNOSUPPORT
}

/// A port on which nothing listens, on `127.0.0.1` and on `[::1]`, and which no stand-in of any test
/// program gets while this value lives; it prints as the port number. Under Unix both addresses
/// are held by [`Refusing`] sockets, so that no other program can start to listen there meanwhile
/// (a listener at the second address of `localhost` made `a_closed_port_...` wait for an answer).
struct ClosedPort {
    port: u16,
    #[cfg(unix)]
    _held: (Refusing, Option<Refusing>),
    _claim: Claim,
}

impl std::fmt::Display for ClosedPort {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.port.fmt(f)
    }
}

#[cfg(unix)]
fn closed_port() -> ClosedPort {
    let _no_start = no_program_starts();
    for _ in 0..200 {
        let (listener, claim) = bind_free(IpAddr::V4(Ipv4Addr::LOCALHOST)).expect("a free port");
        let port = listener.local_addr().expect("address").port();
        drop(listener);
        let Ok(v4) = refusing(SocketAddr::from((Ipv4Addr::LOCALHOST, port))) else {
            continue;
        };
        let v6 = match refusing(SocketAddr::from((Ipv6Addr::LOCALHOST, port))) {
            Ok(v6) => Some(v6),
            Err(e) if no_ipv6(e) => None,
            Err(_) => continue,
        };
        return ClosedPort {
            port,
            _held: (v4, v6),
            _claim: claim,
        };
    }
    panic!("no port that is closed on both loopback addresses");
}

#[cfg(not(unix))]
fn closed_port() -> ClosedPort {
    let _no_start = no_program_starts();
    let (listener, claim) = bind_free(IpAddr::V4(Ipv4Addr::LOCALHOST)).expect("a free port");
    let port = listener.local_addr().expect("address").port();
    drop(listener);
    ClosedPort {
        port,
        _claim: claim,
    }
}

/// A stand-in that answers with the vector on `[::1]` alone, and `127.0.0.1` at the same port held by
/// a [`Refusing`] socket: `localhost` finds the stand-in only at its second address, the first one
/// fails at once, and no other program can listen there. `None` without an IPv6 loopback.
///
/// With `collide`, a foreign stand-in that answers 500 listens on 127.0.0.1 at the port of the
/// first stand-in tried (the forced collision of 21.11.2026). That port is then not used; it is
/// returned so that a test can show that nothing reached it.
#[cfg(unix)]
fn only_on_ipv6(collide: bool) -> Option<(Fake, Refusing, Option<Fake>)> {
    let mut foreign = None;
    for _ in 0..200 {
        let fake = Fake::start_at(IpAddr::V6(Ipv6Addr::LOCALHOST), vec![Act::Vector])?;
        let first = SocketAddr::from((Ipv4Addr::LOCALHOST, fake.addr.port()));
        if collide && foreign.is_none() {
            let bound = {
                let _no_start = no_program_starts();
                TcpListener::bind(first)
            };
            let Ok(listener) = bound else {
                continue;
            };
            foreign = Some(Fake::on(
                listener,
                None,
                vec![Act::Status(500, "{}".into())],
            ));
        }
        let held = {
            let _no_start = no_program_starts();
            refusing(first)
        };
        if let Ok(held) = held {
            return Some((fake, held, foreign));
        }
    }
    panic!("no port whose IPv4 loopback address could be held");
}

/// How long a test may make no progress before the test program ends.
const SILENT: Duration = Duration::from_secs(12);

/// What the watchdog knows of one test: its thread, when it last marked progress and where.
struct Seen {
    thread: ThreadId,
    name: String,
    at: Instant,
    site: &'static Location<'static>,
}

/// The tests that have marked progress and have not ended.
static SEEN: Mutex<Vec<Seen>> = Mutex::new(Vec::new());

fn seen() -> MutexGuard<'static, Vec<Seen>> {
    SEEN.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Removes the entry of a test when its thread ends, so that a test that is through is not waited for.
struct Ends(ThreadId);

impl Drop for Ends {
    fn drop(&mut self) {
        seen().retain(|test| test.thread != self.0);
    }
}

thread_local! {
    static ENDS: Ends = Ends(thread::current().id());
}

/// A test that runs for ever (a mutant of the reading loops that does not advance) must fail the run,
/// not hang it, and `cargo-mutants` gives a run only about twenty seconds on a machine that is fast.
/// Every test marks its progress here (it starts a stand-in, an embedder, a scratch folder, a call, or
/// ends a stand-in; every test marks at its start). The test program ends with exit code 1 when one
/// test that has not ended has made no progress for `SILENT`: that is a test that is stuck, however
/// many other tests are still running, and it does not matter how long the whole program has run or how
/// slow the machine is, as long as each step of a test is done in time. Before it ends the program
/// writes one line to the error output: the test, the line of its last mark and how long it was silent.
/// The programs the tests start are bounded on their own (`wait_or_kill`, eight seconds): a program
/// that is left running would keep the error output of the test program open.
#[track_caller]
fn watchdog() {
    static START: Once = Once::new();
    START.call_once(|| {
        thread::spawn(watch);
    });
    ENDS.with(|_| {});
    let me = thread::current();
    let site = Location::caller();
    let mut seen = seen();
    if let Some(test) = seen.iter_mut().find(|test| test.thread == me.id()) {
        test.at = Instant::now();
        test.site = site;
    } else {
        seen.push(Seen {
            thread: me.id(),
            name: me.name().unwrap_or("unnamed").to_string(),
            at: Instant::now(),
            site,
        });
    }
}

/// The thread of the watchdog: looks twice a second for a test that is stuck, and marks the claims of
/// this program alive.
fn watch() {
    for round in 0_u64.. {
        sleep(Duration::from_millis(500));
        let stuck = seen()
            .iter()
            .find(|test| test.at.elapsed() > SILENT)
            .map(|test| {
                format!(
                    "embed tests: `{}` made no progress for {} s (last mark at {}:{}); \
                     the test program ends now with exit code 1",
                    test.name,
                    test.at.elapsed().as_secs(),
                    test.site.file(),
                    test.site.line()
                )
            });
        if let Some(line) = stuck {
            // Not `eprintln!`: the thread of a test captures that, and this thread inherited its capture.
            let _ = writeln!(std::io::stderr(), "{line}");
            for path in held().drain(..) {
                let _ = fs::remove_dir(path);
            }
            std::process::exit(1);
        }
        if round % 10 == 0 {
            // A file made and removed in the folder of a claim sets its time of change.
            for path in held().iter() {
                let mark = path.join("alive");
                let _ = fs::write(&mark, b"");
                let _ = fs::remove_file(&mark);
            }
        }
    }
}

/// Waits for a program to end; kills it and fails the test after `secs` seconds.
fn wait_or_kill(child: &mut std::process::Child, secs: u64) -> std::process::ExitStatus {
    let end = Instant::now() + Duration::from_secs(secs);
    loop {
        if let Some(status) = child.try_wait().expect("status") {
            return status;
        }
        if Instant::now() >= end {
            let _ = child.kill();
            let _ = child.wait();
            panic!("the program ends in time");
        }
        sleep(Duration::from_millis(10));
    }
}

fn embedder(fake: &Fake, timeout_ms: u64) -> Embedder {
    watchdog();
    Embedder::new(&fake.url(), "embeddinggemma:latest", "30m", timeout_ms).expect("loopback")
}

const OK_BODY: &str = "{\"embeddings\": [[0.25, -0.5, 1, 0, 0.125]]}";
const OK_VECTOR: [f64; 5] = [0.25, -0.5, 1.0, 0.0, 0.125];

#[test]
fn the_request_is_the_one_of_the_model() {
    watchdog();
    let fake = Fake::start(vec![Act::Vector]);
    let vector = embedder(&fake, 5000).embed("Wie geht \u{e9}s?\n");
    fake.wait_closed(1);
    let port = fake.addr.port();
    let calls = fake.finish();
    assert_eq!(vector, Some(vector_for("Wie geht \u{e9}s?\n")));
    assert_eq!(calls.len(), 1);
    let body = "{\"model\": \"embeddinggemma:latest\", \"input\": \"Wie geht \\u00e9s?\\n\", \
                \"keep_alive\": \"30m\"}";
    assert_eq!(calls[0].body, body);
    assert_eq!(
        calls[0].head,
        format!(
            "POST /api/embed HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Type: application/json\r\n\
             Content-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        )
    );
    assert_eq!(calls[0].client_closed, Some(true));
}

#[test]
fn the_options_reach_the_body() {
    watchdog();
    let fake = Fake::start(vec![Act::Vector]);
    let custom = Embedder::new(&fake.url(), "other-model", "0s", 5000).expect("loopback");
    assert!(custom.embed("q").is_some());
    let calls = fake.finish();
    assert_eq!(
        calls[0].body,
        "{\"model\": \"other-model\", \"input\": \"q\", \"keep_alive\": \"0s\"}"
    );
}

#[test]
fn a_long_question_is_cut_to_8000_code_points() {
    watchdog();
    let fake = Fake::start(vec![Act::Vector]);
    let question = format!("{}{}", "a".repeat(MAX_CHARS - 1), "\u{1f600}\u{1f600}tail");
    assert!(embedder(&fake, 5000).embed(&question).is_some());
    let calls = fake.finish();
    let sent: Value = serde_json::from_str(&calls[0].body).expect("JSON");
    let input = sent["input"].as_str().expect("text");
    assert_eq!(input.chars().count(), MAX_CHARS);
    assert!(input.ends_with('\u{1f600}'));
    assert!(!input.contains("tail"));
}

#[test]
fn the_answer_may_come_with_content_length_or_in_chunks_or_until_the_end() {
    watchdog();
    let acts = [
        Act::Json(OK_BODY.to_string()),
        Act::Chunked(OK_BODY.to_string(), 1),
        Act::Chunked(OK_BODY.to_string(), 7),
        Act::Chunked(OK_BODY.to_string(), 4096),
        Act::Trickle(OK_BODY.to_string(), 1, 3),
        Act::Raw(format!("HTTP/1.0 200 OK\r\n\r\n{OK_BODY}").into_bytes()),
        Act::Raw(format!("HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n{OK_BODY}").into_bytes()),
        Act::Raw(
            format!(
                "HTTP/1.1 200\r\ncontent-length:   {}  \r\nX-Other: 1\r\n\r\n{OK_BODY}",
                OK_BODY.len()
            )
            .into_bytes(),
        ),
        Act::Raw(
            format!(
                "HTTP/1.1 200 OK\nContent-Length: {}\n\n{OK_BODY}",
                OK_BODY.len()
            )
            .into_bytes(),
        ),
        // The length wins over what follows it; chunked wins over the length.
        Act::Raw(
            format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{OK_BODY}trailing junk",
                OK_BODY.len()
            )
            .into_bytes(),
        ),
        Act::Raw(
            format!(
                "HTTP/1.1 200 OK\r\nTransfer-Encoding: Chunked\r\nContent-Length: 3\r\n\r\n{:X};ext=1\r\n{OK_BODY}\r\n0\r\nTrailer: x\r\n\r\n",
                OK_BODY.len()
            )
            .into_bytes(),
        ),
    ];
    for (n, act) in acts.into_iter().enumerate() {
        let fake = Fake::start(vec![act]);
        let vector = embedder(&fake, 5000).embed("q");
        let calls = fake.finish();
        assert_eq!(vector, Some(OK_VECTOR.to_vec()), "act {n}");
        assert_eq!(calls.len(), 1, "act {n}");
    }
}

#[test]
fn a_big_answer_is_read_whole() {
    watchdog();
    let vector: Vec<f64> = (0..20_000).map(|i| f64::from(i) / 7.0).collect();
    let body = answer_body(&vector);
    for act in [Act::Json(body.clone()), Act::Chunked(body, 1000)] {
        let fake = Fake::start(vec![act]);
        assert_eq!(embedder(&fake, 5000).embed("q"), Some(vector.clone()));
        assert_eq!(fake.finish().len(), 1);
    }
}

/// Every way an answer can be no answer.
#[allow(clippy::too_many_lines)]
// A flat table of cases; split up, a case would be harder to find.
fn failures() -> Vec<(&'static str, Act)> {
    let raw = |text: &str| Act::Raw(text.as_bytes().to_vec());
    let status = |code: u16| Act::Status(code, OK_BODY.to_string());
    vec![
        ("closed", Act::Close),
        ("status 500", status(500)),
        ("status 404", status(404)),
        ("status 204", status(204)),
        ("status 201", status(201)),
        ("status 301", status(301)),
        ("status 100", status(100)),
        ("not json", Act::Json("not json".to_string())),
        ("empty body", Act::Json(String::new())),
        ("no embeddings", Act::Json("{}".to_string())),
        (
            "embeddings empty",
            Act::Json("{\"embeddings\": []}".to_string()),
        ),
        (
            "first entry empty",
            Act::Json("{\"embeddings\": [[]]}".to_string()),
        ),
        (
            "first entry text",
            Act::Json("{\"embeddings\": [\"x\"]}".to_string()),
        ),
        (
            "text in the list",
            Act::Json("{\"embeddings\": [[1, \"2\"]]}".to_string()),
        ),
        (
            "null in the list",
            Act::Json("{\"embeddings\": [[1, null]]}".to_string()),
        ),
        (
            "boolean",
            Act::Json("{\"embeddings\": [[true]]}".to_string()),
        ),
        (
            "not finite",
            Act::Json("{\"embeddings\": [[1e999]]}".to_string()),
        ),
        ("list", Act::Json("[[1]]".to_string())),
        ("chunked not json", Act::Chunked("nope".to_string(), 2)),
        ("garbage", raw("garbage garbage garbage")),
        ("http 0.9", raw(OK_BODY)),
        ("http 2", raw("HTTP/2 200\r\n\r\n{}")),
        ("status text", raw("HTTP/1.1 OK 200\r\n\r\n{}")),
        ("no status", raw("HTTP/1.1\r\n\r\n{}")),
        (
            "short body",
            raw("HTTP/1.1 200 OK\r\nContent-Length: 100\r\n\r\n{\"emb"),
        ),
        (
            "bad length",
            raw("HTTP/1.1 200 OK\r\nContent-Length: abc\r\n\r\n{}"),
        ),
        (
            "signed length",
            // A good body: only the sign is wrong.
            raw(&format!(
                "HTTP/1.1 200 OK\r\nContent-Length: +{}\r\n\r\n{OK_BODY}",
                OK_BODY.len()
            )),
        ),
        (
            "two lengths",
            // The second length alone would give a good body.
            raw(&format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Length: {}\r\n\r\n{OK_BODY}   ",
                OK_BODY.len() + 3,
                OK_BODY.len()
            )),
        ),
        (
            "length too big",
            raw("HTTP/1.1 200 OK\r\nContent-Length: 99999999999999999999\r\n\r\n{}"),
        ),
        (
            "other coding",
            raw("HTTP/1.1 200 OK\r\nTransfer-Encoding: gzip\r\n\r\n{}"),
        ),
        (
            "header without colon",
            raw("HTTP/1.1 200 OK\r\nbroken\r\n\r\n{}"),
        ),
        (
            "bad chunk size",
            raw("HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\nzz\r\n{}\r\n0\r\n\r\n"),
        ),
        (
            "signed chunk size",
            raw(&format!(
                "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n+{:x}\r\n{OK_BODY}\r\n0\r\n\r\n",
                OK_BODY.len()
            )),
        ),
        (
            "empty chunk size",
            raw("HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n\r\n{}\r\n0\r\n\r\n"),
        ),
        (
            "chunk too short",
            raw("HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n9\r\n{}\r\n"),
        ),
        (
            "chunk without end",
            raw("HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n2\r\n{}\r\n"),
        ),
        (
            "chunk not closed",
            raw("HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n2\r\n{}xx\r\n0\r\n\r\n"),
        ),
        (
            "chunk size too big",
            raw("HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\nFFFFFFFFFFFFFFFFFF\r\n"),
        ),
        ("no head end", raw("HTTP/1.1 200 OK\r\nContent-Length: 2")),
        (
            "long line",
            Act::Raw(
                format!("HTTP/1.1 200 OK\r\nX: {}\r\n\r\n{{}}", "a".repeat(40_000)).into_bytes(),
            ),
        ),
        (
            "too many headers",
            Act::Raw(format!("HTTP/1.1 200 OK\r\n{}\r\n{{}}", "X: 1\r\n".repeat(300)).into_bytes()),
        ),
        (
            "too many trailers",
            Act::Raw(
                format!(
                    "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n0\r\n{}\r\n",
                    "X: 1\r\n".repeat(300)
                )
                .into_bytes(),
            ),
        ),
    ]
}

#[test]
fn every_kind_of_failure_is_tried_twice_and_then_gives_up() {
    watchdog();
    for (name, act) in failures() {
        let fake = Fake::start(vec![act]);
        let vector = embedder(&fake, 5000).embed("q");
        let calls = fake.finish();
        assert_eq!(vector, None, "{name}");
        assert_eq!(calls.len(), 2, "{name}");
        assert_eq!(calls[0].body, calls[1].body, "{name}");
    }
}

#[test]
fn a_failure_is_followed_by_one_more_try_that_may_work() {
    watchdog();
    for (name, act) in failures() {
        let fake = Fake::start(vec![act, Act::Vector]);
        let vector = embedder(&fake, 5000).embed("q");
        let calls = fake.finish();
        assert_eq!(vector, Some(vector_for("q")), "{name}");
        assert_eq!(calls.len(), 2, "{name}");
    }
}

#[test]
fn a_success_is_not_asked_twice() {
    watchdog();
    let fake = Fake::start(vec![Act::Vector, Act::Close]);
    assert!(embedder(&fake, 5000).embed("q").is_some());
    assert_eq!(fake.finish().len(), 1);
}

#[test]
fn a_third_try_is_not_made() {
    watchdog();
    let fake = Fake::start(vec![Act::Close, Act::Close, Act::Vector]);
    assert_eq!(embedder(&fake, 5000).embed("q"), None);
    assert_eq!(fake.finish().len(), 2);
}

#[test]
fn nothing_listening_is_a_failure_at_once() {
    watchdog();
    let port = closed_port();
    let url = format!("http://127.0.0.1:{port}");
    let started = Instant::now();
    assert_eq!(
        Embedder::new(&url, "m", "k", 5000)
            .expect("loopback")
            .embed("q"),
        None
    );
    assert!(started.elapsed() < Duration::from_secs(3));
}

#[test]
fn a_slow_answer_is_cut_at_the_deadline_and_tried_once_more() {
    watchdog();
    let fake = Fake::start(vec![Act::Delay(1500, Box::new(Act::Vector))]);
    let started = Instant::now();
    let vector = embedder(&fake, 200).embed("q");
    let spent = started.elapsed();
    assert_eq!(vector, None);
    assert!(spent >= Duration::from_millis(400), "{spent:?}");
    assert!(spent < Duration::from_millis(1400), "{spent:?}");
    assert_eq!(fake.finish().len(), 2);
}

#[test]
fn a_try_that_hangs_is_cut_at_the_deadline_and_the_next_one_may_work() {
    watchdog();
    let fake = Fake::start(vec![Act::Hang, Act::Vector]);
    let started = Instant::now();
    let vector = embedder(&fake, 200).embed("q");
    let spent = started.elapsed();
    assert_eq!(vector, Some(vector_for("q")));
    assert!(spent >= Duration::from_millis(200), "{spent:?}");
    assert!(spent < Duration::from_millis(1400), "{spent:?}");
    assert_eq!(fake.finish().len(), 2);
}

#[test]
fn the_deadline_covers_the_whole_try_not_each_read() {
    watchdog();
    // The stand-in gives one byte every 10 ms, so that every pause is far shorter than the deadline,
    // but it ends the answer only 700 ms after the head. Nothing here depends on how fast the machine
    // is: the load can make the answer later, never earlier, and the checks below are lower bounds.
    let answer = || Act::Drip {
        pause: 10,
        until: 700,
    };
    // A deadline of 250 ms ends before the answer can end, in each of the two tries.
    let fake = Fake::start(vec![answer()]);
    let started = Instant::now();
    assert_eq!(embedder(&fake, 250).embed("q"), None);
    assert!(started.elapsed() >= Duration::from_millis(500));
    fake.wait_closed(2);
    let calls = fake.finish();
    assert_eq!(calls.len(), 2);
    assert!(calls.iter().all(|call| call.client_closed == Some(true)));
    // With a deadline that is long enough the same answer is read, though it takes longer than the
    // 250 ms that cut it before.
    let fake = Fake::start(vec![answer()]);
    let started = Instant::now();
    assert_eq!(embedder(&fake, 20_000).embed("q"), Some(OK_VECTOR.to_vec()));
    assert!(started.elapsed() >= Duration::from_millis(700));
    assert_eq!(fake.finish().len(), 1);
}

#[test]
fn a_slow_answer_inside_the_deadline_is_read() {
    watchdog();
    let fake = Fake::start(vec![Act::Delay(150, Box::new(Act::Vector))]);
    assert_eq!(embedder(&fake, 3000).embed("q"), Some(vector_for("q")));
    assert_eq!(fake.finish().len(), 1);
}

#[test]
fn localhost_is_the_loopback_address() {
    watchdog();
    let fake = Fake::start(vec![Act::Vector]);
    let url = format!("http://localhost:{}", fake.addr.port());
    let vector = Embedder::new(&url, "m", "k", 5000)
        .expect("loopback")
        .embed("q");
    assert_eq!(vector, Some(vector_for("q")));
    let calls = fake.finish();
    assert!(
        calls[0].head.contains("\r\nHost: localhost:"),
        "{}",
        calls[0].head
    );
}

/// The answer of `OK_BODY`, then spaces, `len` bytes in all.
fn padded(len: usize) -> String {
    format!("{OK_BODY}{}", " ".repeat(len - OK_BODY.len()))
}

#[test]
fn a_body_may_have_body_max_bytes_and_not_one_more() {
    watchdog();
    for (len, fits) in [(BODY_MAX, true), (BODY_MAX + 1, false)] {
        let body = padded(len);
        let acts = [
            Act::Json(body.clone()),
            Act::Chunked(body.clone(), len),
            Act::Chunked(body.clone(), len - 7),
            Act::Chunked(body.clone(), len / 2 + 1),
            Act::Raw(format!("HTTP/1.0 200 OK\r\n\r\n{body}").into_bytes()),
        ];
        for (n, act) in acts.into_iter().enumerate() {
            let fake = Fake::start(vec![act]);
            let vector = embedder(&fake, 20_000).embed("q");
            let calls = fake.finish().len();
            if fits {
                assert_eq!(
                    (vector, calls),
                    (Some(OK_VECTOR.to_vec()), 1),
                    "{len} act {n}"
                );
            } else {
                assert_eq!((vector, calls), (None, 2), "{len} act {n}");
            }
        }
    }
}

#[test]
fn a_header_line_may_have_head_line_max_bytes_before_its_newline_and_not_one_more() {
    watchdog();
    let status = b"HTTP/1.1 200 OK\r\n".to_vec();
    for (len, fits) in [(HEAD_LINE_MAX, true), (HEAD_LINE_MAX + 1, false)] {
        // The long line comes alone and without its newline, so that the client holds exactly `len`
        // bytes of it when it looks at the line for the first time; the newline follows later.
        let mut line = b"X: ".to_vec();
        line.extend(vec![b'a'; len - 4]);
        line.push(b'\r');
        let rest = format!("\nContent-Length: {}\r\n\r\n{OK_BODY}", OK_BODY.len()).into_bytes();
        let fake = Fake::start(vec![Act::Pieces(vec![
            (status.clone(), 0),
            (line, 150),
            (rest, 0),
        ])]);
        let vector = embedder(&fake, 5000).embed("q");
        let calls = fake.finish().len();
        if fits {
            assert_eq!((vector, calls), (Some(OK_VECTOR.to_vec()), 1), "{len}");
        } else {
            assert_eq!((vector, calls), (None, 2), "{len}");
        }
    }
}

#[test]
fn a_head_and_a_trailer_have_at_most_head_lines_max_lines() {
    watchdog();
    for (lines, fits) in [(HEAD_LINES_MAX - 1, true), (HEAD_LINES_MAX, false)] {
        let mut head = format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n", OK_BODY.len());
        head.push_str(&"X: 1\r\n".repeat(lines - 1));
        head.push_str("\r\n");
        head.push_str(OK_BODY);
        let mut trailer = format!(
            "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n{:x}\r\n{OK_BODY}\r\n0\r\n",
            OK_BODY.len()
        );
        trailer.push_str(&"X: 1\r\n".repeat(lines));
        trailer.push_str("\r\n");
        for (name, text) in [("head", head), ("trailer", trailer)] {
            let fake = Fake::start(vec![Act::Raw(text.into_bytes())]);
            let vector = embedder(&fake, 5000).embed("q");
            let calls = fake.finish().len();
            if fits {
                assert_eq!(
                    (vector, calls),
                    (Some(OK_VECTOR.to_vec()), 1),
                    "{name} {lines}"
                );
            } else {
                assert_eq!((vector, calls), (None, 2), "{name} {lines}");
            }
        }
    }
}

#[test]
fn a_length_that_is_given_twice_with_the_same_value_is_read() {
    watchdog();
    let text = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {n}\r\nContent-Length: {n}\r\n\r\n{OK_BODY}",
        n = OK_BODY.len()
    );
    let fake = Fake::start(vec![Act::Raw(text.into_bytes())]);
    assert_eq!(embedder(&fake, 5000).embed("q"), Some(OK_VECTOR.to_vec()));
    assert_eq!(fake.finish().len(), 1);
}

#[test]
fn the_second_loopback_address_is_tried_after_the_first() {
    watchdog();
    // Only where the machine has an IPv6 loopback address; the stand-in listens on it alone. Under
    // Unix 127.0.0.1 at its port is held refused, so the first address fails at once and no other
    // listener can answer there (one red run in 149 on the Linux build host, 21.11.2026, before this).
    #[cfg(unix)]
    let Some((fake, _first, _)) = only_on_ipv6(false) else {
        return;
    };
    #[cfg(not(unix))]
    let Some(fake) = Fake::start_at(IpAddr::V6(Ipv6Addr::LOCALHOST), vec![Act::Vector]) else {
        return;
    };
    second_address_answers(fake);
}

/// The forced collision: another listener holds 127.0.0.1 at the port of the first stand-in. The
/// stand-in is placed on a port whose IPv4 address can be held, and nothing reaches the other one.
#[cfg(unix)]
#[test]
fn the_second_loopback_address_is_tried_after_the_first_while_another_listener_had_its_port() {
    watchdog();
    let Some((fake, _first, foreign)) = only_on_ipv6(true) else {
        return;
    };
    let foreign = foreign.expect("the other listener");
    let port = fake.addr.port();
    second_address_answers(fake);
    assert_ne!(
        foreign.addr.port(),
        port,
        "the stand-in left the taken port"
    );
    assert!(
        foreign.finish().is_empty(),
        "nothing reached the other listener"
    );
}

/// `[::1]` and then `localhost` at the port of this stand-in on `[::1]`: both get the vector, the
/// stand-in sees two calls with their own `Host` lines.
fn second_address_answers(fake: Fake) {
    let port = fake.addr.port();
    for host in ["[::1]", "localhost"] {
        let url = format!("http://{host}:{port}");
        let vector = Embedder::new(&url, "m", "k", 5000)
            .expect("loopback")
            .embed("q");
        assert_eq!(vector, Some(vector_for("q")), "{host}");
    }
    let calls = fake.finish();
    assert_eq!(calls.len(), 2);
    assert!(calls[0].head.contains(&format!("Host: [::1]:{port}\r\n")));
    assert!(
        calls[1]
            .head
            .contains(&format!("Host: localhost:{port}\r\n"))
    );
}

// ---- the connect deadline ----------------------------------------------------------------------

/// A listener that never accepts, with its queue full: the next connect to its port gets no answer
/// (the system drops the SYN), so it lasts until a deadline cuts it. Only `std` is used, so the queue
/// is filled the way a client fills it: by connecting until one connect does not finish in 50 ms.
struct Jam {
    addr: SocketAddr,
    /// The listener and the connections that fill its queue, closed together under
    /// `no_program_starts`.
    held: Option<(TcpListener, Vec<TcpStream>)>,
    /// The claim on the port, released after `held` is closed.
    _claim: Claim,
}

impl Jam {
    /// A full queue on a free port of this address; `None` if the address cannot be bound.
    fn at(ip: IpAddr) -> Option<Self> {
        watchdog();
        let _no_start = no_program_starts();
        let (listener, claim) = bind_free(ip)?;
        let addr = listener.local_addr().expect("address");
        let mut fillers = Vec::new();
        loop {
            match TcpStream::connect_timeout(&addr, Duration::from_millis(50)) {
                Ok(stream) => fillers.push(stream),
                Err(_) => break,
            }
            assert!(
                fillers.len() < 3000,
                "the queue of the listener does not fill"
            );
        }
        Some(Self {
            addr,
            held: Some((listener, fillers)),
            _claim: claim,
        })
    }

    fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.addr.port())
    }
}

impl Drop for Jam {
    fn drop(&mut self) {
        let _no_start = no_program_starts();
        drop(self.held.take());
    }
}

#[test]
fn a_connect_that_gets_no_answer_is_cut_after_250_ms_in_each_try() {
    watchdog();
    // The try has 3 s, the connect 250 ms: two tries of about 250 ms, not two of 3 s.
    let jam = Jam::at(IpAddr::V4(Ipv4Addr::LOCALHOST)).expect("bind");
    let started = Instant::now();
    let vector = Embedder::new(&jam.url(), "m", "k", 3000)
        .expect("loopback")
        .embed("q");
    let spent = started.elapsed();
    assert_eq!(vector, None);
    assert!(spent >= Duration::from_millis(450), "{spent:?}");
    assert!(spent < Duration::from_millis(750), "{spent:?}");
}

#[test]
fn the_deadline_of_the_try_cuts_a_connect_that_is_shorter_than_250_ms() {
    watchdog();
    // The try has 100 ms: the connect gets what is left, not the 250 ms of its own limit.
    let jam = Jam::at(IpAddr::V4(Ipv4Addr::LOCALHOST)).expect("bind");
    let started = Instant::now();
    let vector = Embedder::new(&jam.url(), "m", "k", 100)
        .expect("loopback")
        .embed("q");
    let spent = started.elapsed();
    assert_eq!(vector, None);
    assert!(spent >= Duration::from_millis(180), "{spent:?}");
    assert!(spent < Duration::from_millis(420), "{spent:?}");
}

#[test]
fn localhost_tries_the_second_address_after_a_connect_that_got_no_answer() {
    watchdog();
    // 127.0.0.1 takes no connection and gives no answer; the stand-in listens on [::1] at the same
    // port. The first address costs 250 ms, then the second one answers (there is no second try).
    let Some(jam) = Jam::at(IpAddr::V4(Ipv4Addr::LOCALHOST)) else {
        return;
    };
    let port = jam.addr.port();
    let listener = {
        let _no_start = no_program_starts();
        match TcpListener::bind((Ipv6Addr::LOCALHOST, port)) {
            Ok(listener) => listener,
            Err(_) => return,
        }
    };
    let fake = Fake::on(listener, None, vec![Act::Vector]);
    let url = format!("http://localhost:{port}");
    let started = Instant::now();
    let vector = Embedder::new(&url, "m", "k", 3000)
        .expect("loopback")
        .embed("q");
    let spent = started.elapsed();
    assert_eq!(vector, Some(vector_for("q")));
    assert!(spent >= Duration::from_millis(200), "{spent:?}");
    assert!(spent < Duration::from_millis(1500), "{spent:?}");
    assert_eq!(fake.finish().len(), 1);
}

#[test]
fn a_connect_to_a_listening_port_is_far_below_the_limit_and_the_answer_may_take_longer() {
    watchdog();
    // The kernel completes the connect before the stand-in accepts; the 250 ms do not cover the
    // answer, which here takes 600 ms and is read.
    let fake = Fake::start(vec![Act::Delay(600, Box::new(Act::Vector))]);
    let started = Instant::now();
    assert_eq!(embedder(&fake, 5000).embed("q"), Some(vector_for("q")));
    assert!(started.elapsed() >= Duration::from_millis(600));
    assert_eq!(fake.finish().len(), 1);
}

#[test]
fn a_closed_port_is_the_lexical_way_within_the_bound_that_holds_on_windows_too() {
    watchdog();
    // Windows refuses a connect to a closed port only after 2.0 s; with the limit of 250 ms the two
    // tries take about 0.5 s there. On the Mac and on Linux the refusal is immediate.
    let port = closed_port();
    let url = format!("http://localhost:{port}");
    let started = Instant::now();
    let vector = Embedder::new(&url, "m", "k", 5000)
        .expect("loopback")
        .embed("q");
    assert_eq!(vector, None);
    assert!(started.elapsed() < Duration::from_millis(1500));
}

#[test]
fn the_connection_is_closed_after_every_answer_and_after_a_failure() {
    watchdog();
    let fake = Fake::start(vec![
        Act::Vector,
        Act::Chunked(OK_BODY.to_string(), 5),
        Act::Status(500, String::new()),
        Act::Json("not json".to_string()),
        Act::Json("{\"embeddings\": []}".to_string()),
    ]);
    let ollama = embedder(&fake, 5000);
    assert!(ollama.embed("a").is_some());
    assert!(ollama.embed("b").is_some());
    assert_eq!(ollama.embed("c"), None);
    fake.wait_closed(4);
    let calls = fake.finish();
    assert_eq!(calls.len(), 4);
    for (n, call) in calls.iter().enumerate() {
        assert_eq!(call.client_closed, Some(true), "call {n}");
    }
}

// ---- the program -----------------------------------------------------------------------------

/// The notes of the invented vault.
const NOTES: [(&str, &str); 7] = [
    ("a.md", "---\ntitle: Garten\n---\nBeet und Gartenarbeit\n"),
    ("b.md", "Kaffee Beet Beet Kaffeemaschine\n"),
    (
        "c.md",
        "---\ntitle: Wasserfall\n---\nWasserfall und Berge\n",
    ),
    ("sub/d.md", "Bergwanderung im Garten\nBeet\n"),
    ("e.md", "Entscheidung zur Aufgabe\n"),
    (
        "f.md",
        "---\nclass: derived\n---\nBeet Kaffee Zusammenfassung\n",
    ),
    ("90-secrets/geheim.md", "Beet Kaffee geheim\n"),
];

/// Paths a database can hold: the notes, an excluded one, and paths without a file (relics).
const RELS: [&str; 10] = [
    "a.md",
    "b.md",
    "c.md",
    "sub/d.md",
    "e.md",
    "f.md",
    "90-secrets/geheim.md",
    "gone.md",
    "sub/x.md",
    "90-secrets/x.md",
];

const QUERIES: [&str; 8] = [
    "beet",
    "garten",
    "kaffee",
    "beet kaffee",
    "wasserfall berge",
    "zzz",
    "entscheidung",
    "der die das",
];

/// A small deterministic generator (xorshift64*), so that a failure names its seed.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, bound: usize) -> usize {
        usize::try_from(self.next() % u64::try_from(bound).expect("fits")).expect("fits")
    }

    fn chance(&mut self, percent: usize) -> bool {
        self.below(100) < percent
    }

    fn pick<'a, T>(&mut self, list: &'a [T]) -> &'a T {
        &list[self.below(list.len())]
    }
}

struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        watchdog();
        let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("embed-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("vault/sub")).expect("vault is created");
        fs::create_dir_all(root.join("vault/90-secrets")).expect("vault is created");
        fs::create_dir_all(root.join("state")).expect("state is created");
        for (rel, text) in NOTES {
            fs::write(root.join("vault").join(rel), text).expect("note is written");
        }
        Self(root)
    }

    fn vault(&self) -> String {
        self.0.join("vault").to_string_lossy().into_owned()
    }

    fn index(&self) -> String {
        self.0
            .join("state/gardener.db")
            .to_string_lossy()
            .into_owned()
    }

    fn extra(&self) -> String {
        self.0
            .join("state/extra-embeddings.db")
            .to_string_lossy()
            .into_owned()
    }

    fn file(&self, name: &str) -> String {
        self.0.join(name).to_string_lossy().into_owned()
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn write_db(path: &str, rows: &[(String, String)]) {
    let path = Path::new(path);
    let _ = fs::remove_file(path);
    let connection = Connection::open(path).expect("database is created");
    connection
        .execute_batch(
            "PRAGMA synchronous = OFF; PRAGMA journal_mode = OFF; \
             CREATE TABLE embeddings (rel TEXT PRIMARY KEY, hash TEXT NOT NULL, \
             vector TEXT NOT NULL, updated REAL NOT NULL); BEGIN;",
        )
        .expect("table is created");
    for (rel, vector) in rows {
        connection
            .execute(
                "INSERT INTO embeddings VALUES (?1, 'h', ?2, 0.0)",
                (rel, vector),
            )
            .expect("row is written");
    }
    connection.execute_batch("COMMIT").expect("committed");
}

/// Some paths of `RELS` with vectors of the length of the stand-in's.
fn rows(rng: &mut Rng, kinds: usize) -> Vec<(String, String)> {
    let mut paths: Vec<&str> = RELS.to_vec();
    let mut chosen = Vec::new();
    for _ in 0..rng.below(kinds + 1) {
        let at = rng.below(paths.len());
        chosen.push(paths.remove(at));
    }
    chosen
        .into_iter()
        .map(|rel| {
            let vector = vector_for(&format!("{rel}{}", rng.next()));
            (rel.to_string(), list_text(&vector))
        })
        .collect()
}

fn strings(args: &[&str]) -> Vec<String> {
    args.iter().map(ToString::to_string).collect()
}

fn call(args: &[String]) -> (u8, String) {
    watchdog();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = rangkern::run(args, &mut out, &mut err);
    (code, String::from_utf8(out).expect("UTF-8"))
}

/// One `search` call with these extra arguments.
fn search(scratch: &Scratch, question: &str, rest: &[String]) -> (u8, String) {
    let mut args = strings(&[
        "search",
        &scratch.vault(),
        question,
        "--today",
        "2026-01-01",
    ]);
    args.extend_from_slice(rest);
    call(&args)
}

#[test]
#[allow(clippy::too_many_lines)]
// One flat generator of the states of the databases and of Ollama and the calls to compare; split
// up, the seed of a failing state would be harder to follow.
fn search_with_embed_equals_search_with_the_vector_of_the_stand_in_in_320_states() {
    watchdog();
    let scratch = Scratch::new("states");
    let mut rng = Rng(0xE4B3_D000_5EED_0001);
    let (mut asked, mut lexical, mut relics) = (0, 0, 0);
    for state in 0..320 {
        let index_rows = rows(&mut rng, RELS.len());
        let extra_rows = rows(&mut rng, 4);
        write_db(&scratch.index(), &index_rows);
        let has_extra = rng.chance(60);
        if has_extra {
            write_db(&scratch.extra(), &extra_rows);
        } else {
            let _ = fs::remove_file(scratch.extra());
        }
        let question = *rng.pick(&QUERIES);
        let k = (1 + rng.below(8)).to_string();
        let mut common = strings(&["-k", &k, "--index-db", &scratch.index()]);
        if has_extra {
            common.extend(strings(&["--extra-db", &scratch.extra()]));
        }
        if rng.chance(25) {
            common.extend(strings(&["--space", "kern"]));
        }
        // Does one of the vectors belong to a note of the corpus?
        let on_corpus = |rel: &String| NOTES.iter().take(6).any(|(note, _)| note == rel);
        let corpus_vector = index_rows.iter().any(|(rel, _)| on_corpus(rel))
            || (has_extra && extra_rows.iter().any(|(rel, _)| on_corpus(rel)));
        // The plan of the stand-in and whether Ollama answers at the end of it.
        let (plan, answers) = match rng.below(100) {
            0..=54 => (vec![Act::Vector], true),
            55..=64 => (
                vec![Act::Chunked(
                    answer_body(&vector_for(question)),
                    1 + rng.below(40),
                )],
                true,
            ),
            65..=79 => {
                let failing = failures();
                let (_, act) = failing[rng.below(failing.len())].clone();
                (vec![act, Act::Vector], true)
            }
            _ => {
                let failing = failures();
                let (_, act) = failing[rng.below(failing.len())].clone();
                (vec![act], false)
            }
        };
        let fails = !answers;
        let shown = format!("{plan:?}");
        let fake = Fake::start(plan);
        let mut with_embed = common.clone();
        with_embed.extend(strings(&[
            "--embed",
            &fake.url(),
            "--embed-timeout-ms",
            "5000",
        ]));
        let (code, out) = search(&scratch, question, &with_embed);
        fake.wait_closed(fake.count());
        let calls = fake.finish();
        let label = format!("state {state}, question {question:?}, plan {shown}, calls {calls:?}");
        if !corpus_vector {
            // Relics and nothing else: Ollama is not asked, the hybrid way has no semantic list.
            relics += 1;
            assert_eq!(calls.len(), 0, "{label}");
            fs::write(scratch.file("q.json"), "[1]").expect("written");
            let mut reference = common.clone();
            reference.extend(strings(&["--query-vector", &scratch.file("q.json")]));
            assert_eq!(
                search(&scratch, question, &reference),
                (code, out),
                "{label}"
            );
            continue;
        }
        if fails {
            lexical += 1;
            assert_eq!(calls.len(), 2, "{label}");
            // The lexical way of the model: the same as a search without vectors.
            let mut reference = strings(&["-k", &k]);
            if common.contains(&"--space".to_string()) {
                reference.extend(strings(&["--space", "kern"]));
            }
            assert_eq!(
                search(&scratch, question, &reference),
                (code, out),
                "{label}"
            );
        } else {
            asked += 1;
            assert!(!calls.is_empty() && calls.len() <= 2, "{label}");
            fs::write(scratch.file("q.json"), list_text(&vector_for(question))).expect("written");
            let mut reference = common.clone();
            reference.extend(strings(&["--query-vector", &scratch.file("q.json")]));
            assert_eq!(
                search(&scratch, question, &reference),
                (code, out),
                "{label}"
            );
        }
        assert_eq!(code, 0, "{label}");
    }
    // The states cover the three ways, not just one.
    assert!(
        asked > 100 && lexical > 20 && relics > 3,
        "{asked} {lexical} {relics}"
    );
}

/// A scratch vault with databases that hold a vector for every note of the corpus.
fn scratch_with_vectors(name: &str) -> Scratch {
    let scratch = Scratch::new(name);
    let rows: Vec<(String, String)> = NOTES
        .iter()
        .take(6)
        .map(|(rel, _)| ((*rel).to_string(), list_text(&vector_for(rel))))
        .collect();
    write_db(&scratch.index(), &rows);
    scratch
}

#[test]
fn search_with_embed_asks_once_with_the_question_and_prints_the_hybrid_list() {
    watchdog();
    let scratch = scratch_with_vectors("once");
    let fake = Fake::start(vec![Act::Vector]);
    let (code, out) = search(
        &scratch,
        "beet kaffee",
        &strings(&["--index-db", &scratch.index(), "--embed", &fake.url()]),
    );
    let calls = fake.finish();
    assert_eq!(code, 0);
    assert!(out.starts_with("{\"fallback\":false,\"hits\":[{"), "{out}");
    assert_eq!(calls.len(), 1);
    let sent: Value = serde_json::from_str(&calls[0].body).expect("JSON");
    assert_eq!(sent["input"], "beet kaffee");
    assert_eq!(sent["model"], "embeddinggemma:latest");
    assert_eq!(sent["keep_alive"], "30m");
}

#[test]
fn search_with_embed_and_ollama_down_takes_the_lexical_way() {
    watchdog();
    let scratch = scratch_with_vectors("down");
    let port = closed_port();
    let url = format!("http://127.0.0.1:{port}");
    let (code, out) = search(
        &scratch,
        "beet kaffee",
        &strings(&["--index-db", &scratch.index(), "--embed", &url]),
    );
    assert_eq!(code, 0);
    assert!(out.starts_with("{\"fallback\":true,\"hits\":["), "{out}");
    assert_eq!(search(&scratch, "beet kaffee", &[]), (code, out));
}

#[test]
fn an_unreadable_database_is_exit_3_and_ollama_is_not_asked() {
    watchdog();
    let scratch = Scratch::new("unreadable");
    fs::write(scratch.index(), b"garbage, not a database, but long enough").expect("written");
    let fake = Fake::start(vec![Act::Vector]);
    let args = strings(&["--index-db", &scratch.index(), "--embed", &fake.url()]);
    assert_eq!(
        search(&scratch, "beet", &args),
        (3, "{\"error\":\"unreadable\"}\n".to_string())
    );
    assert_eq!(fake.finish().len(), 0);
}

#[test]
fn the_options_set_model_keep_alive_and_deadline() {
    watchdog();
    let scratch = scratch_with_vectors("options");
    let fake = Fake::start(vec![Act::Delay(1500, Box::new(Act::Vector))]);
    let started = Instant::now();
    let (code, out) = search(
        &scratch,
        "beet",
        &strings(&[
            "--index-db",
            &scratch.index(),
            "--embed",
            &fake.url(),
            "--embed-model",
            "m:1",
            "--keep-alive",
            "5m",
            "--embed-timeout-ms",
            "150",
        ]),
    );
    let calls = fake.finish();
    assert!(started.elapsed() < Duration::from_millis(1400));
    assert_eq!(code, 0);
    assert!(out.starts_with("{\"fallback\":true"), "{out}");
    assert_eq!(calls.len(), 2);
    let sent: Value = serde_json::from_str(&calls[0].body).expect("JSON");
    assert_eq!(
        (sent["model"].as_str(), sent["keep_alive"].as_str()),
        (Some("m:1"), Some("5m"))
    );
}

#[test]
fn every_address_but_a_loopback_address_and_every_clash_of_options_is_exit_64() {
    watchdog();
    let scratch = scratch_with_vectors("usage");
    let index = scratch.index();
    let bad = |rest: &[&str]| {
        let mut args = strings(&["search", &scratch.vault(), "beet"]);
        args.extend(strings(rest));
        assert_eq!(call(&args), (64, String::new()), "search {rest:?}");
        // `serve` knows no `--query-vector`; that is a usage error there anyway.
        let mut args = strings(&["serve", &scratch.vault()]);
        args.extend(strings(rest));
        assert_eq!(call_serve(&args, ""), (64, String::new()), "serve {rest:?}");
    };
    for url in [
        "http://example.com:1",
        "https://127.0.0.1:1",
        "http://127.0.0.1",
        "http://127.0.0.1:0",
        "http://127.0.0.1:1/api",
        "http://127.0.0.1:1/",
        "http://10.0.0.1:1",
        "http://[::1]",
        "127.0.0.1:1",
        "",
    ] {
        bad(&["--index-db", &index, "--embed", url]);
    }
    // `--embed` needs `--index-db` and excludes `--query-vector` and `--vectors`.
    bad(&["--embed", "http://127.0.0.1:1"]);
    bad(&["--vectors", "v.json", "--embed", "http://127.0.0.1:1"]);
    bad(&[
        "--index-db",
        &index,
        "--embed",
        "http://127.0.0.1:1",
        "--query-vector",
        "q.json",
    ]);
    // The other embedding options need `--embed` and a good value.
    bad(&["--index-db", &index, "--embed-model", "m"]);
    bad(&["--index-db", &index, "--keep-alive", "5m"]);
    bad(&["--index-db", &index, "--embed-timeout-ms", "5"]);
    for value in ["0", "-1", "abc", "", "1.5", "1e3"] {
        bad(&[
            "--index-db",
            &index,
            "--embed",
            "http://127.0.0.1:1",
            "--embed-timeout-ms",
            value,
        ]);
    }
}

/// `serve` with `input` on its standard input; the answers as text.
fn call_serve(args: &[String], input: &str) -> (u8, String) {
    watchdog();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = rangkern::run_with_input(
        args,
        &mut Cursor::new(input.as_bytes().to_vec()),
        &mut out,
        &mut err,
    );
    (code, String::from_utf8(out).expect("UTF-8"))
}

#[test]
fn serve_takes_each_embedding_option_once() {
    watchdog();
    let scratch = scratch_with_vectors("serve-once");
    for option in [
        "--embed",
        "--embed-model",
        "--keep-alive",
        "--embed-timeout-ms",
    ] {
        let value = match option {
            "--embed" => "http://127.0.0.1:1",
            "--embed-timeout-ms" => "5",
            _ => "x",
        };
        let mut args = strings(&["serve", &scratch.vault(), "--index-db", &scratch.index()]);
        args.extend(strings(&[
            "--embed",
            "http://127.0.0.1:1",
            option,
            value,
            option,
            value,
        ]));
        assert_eq!(
            call_serve(&args, "{\"op\":\"quit\"}\n"),
            (64, String::new()),
            "{option}"
        );
    }
    let mut args = strings(&["serve", &scratch.vault(), "--index-db", &scratch.index()]);
    args.extend(strings(&[
        "--embed",
        "http://127.0.0.1:1",
        "--embed-model",
        "x",
        "--keep-alive",
        "1m",
        "--embed-timeout-ms",
        "5",
    ]));
    assert_eq!(call_serve(&args, "{\"op\":\"quit\"}\n").0, 0);
}

#[test]
fn serve_embeds_a_request_without_query_vector_and_takes_the_vector_of_one_with_it() {
    watchdog();
    let scratch = scratch_with_vectors("serve");
    let fake = Fake::start(vec![Act::Vector]);
    let given = vector_for("beet");
    let lines: Vec<String> = vec![
        // Without a vector: embedded, 1 call.
        "{\"op\":\"search\",\"id\":1,\"query\":\"beet\",\"today\":\"2026-01-01\"}".to_string(),
        // With a vector: no call.
        format!(
            "{{\"op\":\"search\",\"id\":2,\"query\":\"beet\",\"today\":\"2026-01-01\",\"query_vector\":{}}}",
            list_text(&given)
        ),
        // With a vector of another question: that vector counts, the question is not embedded.
        format!(
            "{{\"op\":\"search\",\"id\":3,\"query\":\"kaffee\",\"today\":\"2026-01-01\",\"query_vector\":{}}}",
            list_text(&given)
        ),
        // Without a vector again: embedded, 2 calls so far.
        "{\"op\":\"search\",\"id\":4,\"query\":\"kaffee\",\"today\":\"2026-01-01\"}".to_string(),
        "{\"op\":\"ping\",\"id\":5}".to_string(),
        "{\"op\":\"quit\"}".to_string(),
    ];
    let input = format!("{}\n", lines.join("\n"));
    let args = strings(&[
        "serve",
        &scratch.vault(),
        "--index-db",
        &scratch.index(),
        "--embed",
        &fake.url(),
    ]);
    let (code, out) = call_serve(&args, &input);
    fake.wait_closed(2);
    let calls = fake.finish();
    assert_eq!(code, 0);
    assert_eq!(calls.len(), 2);
    assert!(calls.iter().all(|call| call.client_closed == Some(true)));
    let inputs: Vec<Value> = calls
        .iter()
        .map(|call| serde_json::from_str::<Value>(&call.body).expect("JSON")["input"].clone())
        .collect();
    assert_eq!(inputs, [Value::from("beet"), Value::from("kaffee")]);
    let answers: Vec<Value> = out
        .lines()
        .map(|line| serde_json::from_str(line).expect("JSON"))
        .collect();
    assert_eq!(answers.len(), 6);
    // Request 1 (embedded) equals request 2 (the same vector given).
    let mut first = answers[0].clone();
    let mut second = answers[1].clone();
    first["id"] = Value::Null;
    second["id"] = Value::Null;
    assert_eq!(first, second);
    assert_eq!(answers[0]["fallback"], false);
    // Request 4 equals a search of the same question with the vector of the stand-in.
    let reference = search(&scratch, "kaffee", &{
        fs::write(scratch.file("q.json"), list_text(&vector_for("kaffee"))).expect("written");
        strings(&[
            "--index-db",
            &scratch.index(),
            "--query-vector",
            &scratch.file("q.json"),
        ])
    });
    let mut fourth = answers[3].clone();
    fourth.as_object_mut().expect("object").remove("id");
    assert_eq!(
        fourth,
        serde_json::from_str::<Value>(&reference.1).expect("JSON")
    );
}

#[test]
fn serve_without_embed_leaves_a_request_without_query_vector_lexical() {
    watchdog();
    let scratch = scratch_with_vectors("serve-plain");
    let args = strings(&["serve", &scratch.vault(), "--index-db", &scratch.index()]);
    let (code, out) = call_serve(
        &args,
        "{\"op\":\"search\",\"query\":\"beet\",\"today\":\"2026-01-01\"}\n",
    );
    assert_eq!(code, 0);
    assert!(out.starts_with("{\"fallback\":true,"), "{out}");
}

#[test]
fn serve_with_embed_and_ollama_down_answers_lexically_and_keeps_serving() {
    watchdog();
    let scratch = scratch_with_vectors("serve-down");
    let port = closed_port();
    let url = format!("http://127.0.0.1:{port}");
    let args = strings(&[
        "serve",
        &scratch.vault(),
        "--index-db",
        &scratch.index(),
        "--embed",
        &url,
    ]);
    let request = "{\"op\":\"search\",\"query\":\"beet\",\"today\":\"2026-01-01\"}\n";
    let (code, out) = call_serve(&args, &format!("{request}{request}{{\"op\":\"ping\"}}\n"));
    assert_eq!(code, 0);
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), 3);
    assert!(lines[0].starts_with("{\"fallback\":true,"));
    assert_eq!(lines[0], lines[1]);
    assert_eq!(lines[2], "{\"ok\":true}");
    let reference = search(&scratch, "beet", &[]);
    assert_eq!(format!("{}\n", lines[0]), reference.1);
}

#[test]
fn serve_does_not_ask_ollama_without_a_note_vector_of_the_corpus() {
    watchdog();
    let scratch = Scratch::new("serve-relics");
    write_db(
        &scratch.index(),
        &[
            ("gone.md".to_string(), list_text(&vector_for("x"))),
            (
                "90-secrets/geheim.md".to_string(),
                list_text(&vector_for("y")),
            ),
        ],
    );
    let fake = Fake::start(vec![Act::Vector]);
    let args = strings(&[
        "serve",
        &scratch.vault(),
        "--index-db",
        &scratch.index(),
        "--embed",
        &fake.url(),
    ]);
    let (code, out) = call_serve(
        &args,
        "{\"op\":\"search\",\"query\":\"beet\",\"today\":\"2026-01-01\"}\n",
    );
    assert_eq!(fake.finish().len(), 0);
    assert_eq!(code, 0);
    assert!(out.starts_with("{\"fallback\":false,\"hits\":["), "{out}");
    assert!(!out.contains("gone.md") && !out.contains("geheim"));
}

#[test]
fn a_vector_of_an_excluded_or_vanished_note_never_shows_with_embedding() {
    watchdog();
    let scratch = Scratch::new("relics");
    // The relics are the nearest to the question; the notes of the corpus are not.
    let near = list_text(&vector_for("beet"));
    write_db(
        &scratch.index(),
        &[
            ("gone.md".to_string(), near.clone()),
            ("90-secrets/geheim.md".to_string(), near.clone()),
            ("90-secrets/x.md".to_string(), near),
            ("b.md".to_string(), list_text(&[0.0, 0.0, 0.0, 0.0, 1.0])),
        ],
    );
    let fake = Fake::start(vec![Act::Vector]);
    let (code, out) = search(
        &scratch,
        "beet",
        &strings(&[
            "--index-db",
            &scratch.index(),
            "--embed",
            &fake.url(),
            "-k",
            "20",
        ]),
    );
    assert_eq!(fake.finish().len(), 1);
    assert_eq!(code, 0);
    assert!(
        !out.contains("gone.md") && !out.contains("geheim") && !out.contains("x.md"),
        "{out}"
    );
}

#[test]
fn the_program_reads_no_environment_variable_for_the_address() {
    watchdog();
    // A proxy variable must change nothing: the call goes straight to the loopback address.
    let scratch = scratch_with_vectors("env");
    let fake = Fake::start(vec![Act::Vector]);
    let mut child = start_program(
        std::process::Command::new(env!("CARGO_BIN_EXE_rangkern"))
            .args(["search", &scratch.vault(), "beet", "--today", "2026-01-01"])
            .args(["--index-db", &scratch.index(), "--embed", &fake.url()])
            .env("http_proxy", "http://127.0.0.1:9")
            .env("HTTP_PROXY", "http://127.0.0.1:9")
            .env("ALL_PROXY", "http://127.0.0.1:9")
            .env("no_proxy", "")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null()),
    )
    .expect("program runs");
    let status = wait_or_kill(&mut child, 8);
    let mut stdout = String::new();
    child
        .stdout
        .take()
        .expect("stdout")
        .read_to_string(&mut stdout)
        .expect("output");
    let calls = fake.finish();
    assert_eq!(status.code(), Some(0));
    assert_eq!(calls.len(), 1);
    assert!(stdout.starts_with("{\"fallback\":false"), "{stdout}");
}

#[test]
fn the_program_on_pipes_embeds_every_request_and_leaves_no_connection_open() {
    watchdog();
    let scratch = scratch_with_vectors("pipes");
    let fake = Fake::start(vec![Act::Vector]);
    let mut child = start_program(
        std::process::Command::new(env!("CARGO_BIN_EXE_rangkern"))
            .args([
                "serve",
                &scratch.vault(),
                "--index-db",
                &scratch.index(),
                "--embed",
                &fake.url(),
            ])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null()),
    )
    .expect("program starts");
    let mut stdin = child.stdin.take().expect("stdin");
    let stdout = std::io::BufReader::new(child.stdout.take().expect("stdout"));
    // The answers are read in a thread, so that a program that does not answer is killed, not waited for.
    let (send, receive) = mpsc::channel();
    thread::spawn(move || {
        for line in stdout.lines() {
            if send.send(line).is_err() {
                break;
            }
        }
    });
    for round in 1..=3_usize {
        writeln!(
            stdin,
            "{{\"op\":\"search\",\"query\":\"beet {round}\",\"today\":\"2026-01-01\"}}"
        )
        .expect("written");
        stdin.flush().expect("flushed");
        let Ok(line) = receive.recv_timeout(Duration::from_secs(8)) else {
            let _ = child.kill();
            let _ = child.wait();
            panic!("the program answers in time");
        };
        let line = line.expect("answer");
        assert!(line.starts_with("{\"fallback\":false"), "{line}");
        // The process is still running and the connection of this request is already closed.
        fake.wait_closed(round);
        assert_eq!(fake.count(), round);
        let closed = fake.calls.lock().expect("lock")[round - 1].client_closed;
        assert_eq!(closed, Some(true), "round {round}");
        assert!(child.try_wait().expect("status").is_none());
    }
    drop(stdin);
    assert_eq!(wait_or_kill(&mut child, 8).code(), Some(0));
    assert_eq!(fake.finish().len(), 3);
}
