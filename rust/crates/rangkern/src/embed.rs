//! The vector of the question, asked of the local Ollama in the process (`--embed <url>`).
//!
//! The call is the one of `gardener.ollama.OllamaClient.embed`: `POST <url>/api/embed` with
//! `{"model", "input", "keep_alive"}`, the question cut to 8,000 code points, and one more try after
//! a failure. HTTP/1.1 is spoken over `std::net::TcpStream`, so no crate is added. The only network
//! the program has is this call, and it goes to a loopback address (`127.0.0.1`, `localhost`, `[::1]`):
//! no proxy, no environment variable, no name lookup (`localhost` is the two fixed addresses), no child
//! process. The connection is closed after every answer, and a deadline covers one try from the
//! connect to the last byte of the answer. The connect to each address has a deadline of its own
//! (`CONNECT_MS`) inside that one.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, TcpStream};
use std::time::{Duration, Instant};

use serde_json::value::RawValue;

/// The model of the call when `--embed-model` is not given (`config.EMBED_MODEL`).
pub const DEFAULT_MODEL: &str = "embeddinggemma:latest";
/// How long Ollama keeps the model when `--keep-alive` is not given (`config.SEARCH_KEEP_ALIVE`).
pub const DEFAULT_KEEP_ALIVE: &str = "30m";
/// The deadline of one try in milliseconds when `--embed-timeout-ms` is not given (`config.OLLAMA_TIMEOUT`).
pub const DEFAULT_TIMEOUT_MS: u64 = 180_000;
/// The question is cut to this many code points (`config.EMBED_MAX_CHARS`).
pub const MAX_CHARS: usize = 8000;
/// Tries after the first one (`OllamaClient.RETRIES`).
const RETRIES: usize = 1;
/// The longest line of the head of an answer (the bytes before its `\n`) and the most lines of a head.
pub const HEAD_LINE_MAX: usize = 16 * 1024;
/// A head, or the trailer of a chunked body, has at most this many lines, the empty one at its end included.
pub const HEAD_LINES_MAX: usize = 256;
/// The largest body of an answer that is read (an embedding of 3,072 numbers is about 80 kB).
pub const BODY_MAX: usize = 4 * 1024 * 1024;
/// The connect to one address of a try takes at most this long, and never longer than what is left of
/// the deadline of the try (contract `rangkern-einbettung`, paragraph "Verbindungsfrist", named
/// deviation 5). Windows refuses a connection to a closed loopback port only after 2.0 s; with this
/// limit a search with Ollama off costs two tries of 250 ms there, not two of 2.0 s. The kernel
/// completes the connect to a listening loopback port at once, so a slow answer is not cut by it.
const CONNECT_MS: u64 = 250;
/// The deadline is at most this long (about 49 days), so that adding it to the clock cannot overflow.
const TIMEOUT_MAX_MS: u64 = 4_294_967_295;

/// Why the embedding of a question gave no vector: the failure of its last try (contract 4, section "Rückfall der
/// Einbettung"). The three reasons split every failure of a try.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Grund {
    /// No TCP connection to any address of the endpoint: refused, or `CONNECT_MS` (or the smaller rest of the deadline)
    /// ran out.
    KeineVerbindung,
    /// After the connect the deadline of the try ran out: a write or read ended with `TimedOut` or `WouldBlock`, or the
    /// rest was zero and the socket refused it as a timeout.
    Frist,
    /// Any other failure after the connect: a status other than 200, a head or body over its limit, a connection closed
    /// before the end, another input or output error, no JSON, no vector (`first_vector`).
    AntwortUngueltig,
}

impl Grund {
    /// The value of the field `grund` of the log line `suche.rueckfall` (contract 5, section 6.5).
    #[must_use]
    pub const fn text(self) -> &'static str {
        match self {
            Self::KeineVerbindung => "keine_verbindung",
            Self::Frist => "frist",
            Self::AntwortUngueltig => "antwort_ungueltig",
        }
    }
}

/// The reason of an input or output error after the connect: the deadline for `TimedOut` and `WouldBlock` (the socket
/// timeout of Linux and macOS ends a read or write with `EAGAIN`), an invalid answer for every other kind.
pub(crate) fn grund_des_fehlers(kind: io::ErrorKind) -> Grund {
    match kind {
        io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock => Grund::Frist,
        _ => Grund::AntwortUngueltig,
    }
}

/// The reason when the socket refuses the timeout `left`: a zero rest is the end of the deadline, anything else an
/// invalid answer.
pub(crate) fn grund_der_frist(left: Duration) -> Grund {
    if left.is_zero() {
        Grund::Frist
    } else {
        Grund::AntwortUngueltig
    }
}

/// One try failed, with its reason.
struct Failed(Grund);

/// A failure after the connect that is no input or output error: the answer is not what the call needs.
const INVALID: Failed = Failed(Grund::AntwortUngueltig);

impl From<io::Error> for Failed {
    fn from(error: io::Error) -> Self {
        Failed(grund_des_fehlers(error.kind()))
    }
}

/// Sets the timeout `left` with `set`; a refusal counts by [`grund_der_frist`].
fn frist_setzen(
    left: Duration,
    set: impl FnOnce(Option<Duration>) -> io::Result<()>,
) -> Result<(), Failed> {
    set(Some(left)).map_err(|_| Failed(grund_der_frist(left)))
}

/// The loopback address of the call: where to connect (in this order) and what to write as `Host`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Endpoint {
    pub(crate) addrs: Vec<SocketAddr>,
    pub(crate) host: String,
}

/// `http://127.0.0.1:<port>`, `http://localhost:<port>` or `http://[::1]:<port>`, nothing else: no
/// path (not even `/`), no user, no other host, no other spelling, a port from 1 to 65535.
pub(crate) fn parse_endpoint(url: &str) -> Option<Endpoint> {
    let rest = url.strip_prefix("http://")?;
    let (name, port) = if let Some(inner) = rest.strip_prefix('[') {
        let (name, port) = inner.split_once("]:")?;
        (format!("[{name}]"), port)
    } else {
        let (name, port) = rest.split_once(':')?;
        (name.to_string(), port)
    };
    if !all_digits(port, u8::is_ascii_digit) {
        return None;
    }
    let port: u16 = port.parse().ok().filter(|port| *port != 0)?;
    let loopback4 = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let loopback6 = SocketAddr::from((Ipv6Addr::LOCALHOST, port));
    let addrs = match name.as_str() {
        "127.0.0.1" => vec![loopback4],
        "localhost" => vec![loopback4, loopback6],
        "[::1]" => vec![loopback6],
        _ => return None,
    };
    Some(Endpoint {
        addrs,
        host: format!("{name}:{port}"),
    })
}

/// A text as Python's `json.dumps` writes it with `ensure_ascii`: the body of the call is then the
/// bytes the model's own client sends.
pub(crate) fn json_string(text: &str, out: &mut String) {
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            ' '..='~' => out.push(c),
            _ => {
                for unit in c.encode_utf16(&mut [0; 2]) {
                    let _ = write!(out, "\\u{unit:04x}");
                }
            }
        }
    }
    out.push('"');
}

/// The JSON body of the call.
pub(crate) fn request_body(model: &str, question: &str, keep_alive: &str) -> String {
    let input: String = question.chars().take(MAX_CHARS).collect();
    let mut body = String::from("{\"model\": ");
    json_string(model, &mut body);
    body.push_str(", \"input\": ");
    json_string(&input, &mut body);
    body.push_str(", \"keep_alive\": ");
    json_string(keep_alive, &mut body);
    body.push('}');
    body
}

/// The first vector of an answer `{"embeddings": [[number, ...], ...]}`: a list of finite numbers
/// that is not empty. A number is read from its text by the standard library, which rounds it as
/// Python's `float()` does; a number beyond the range of a double is no finite number. The last of
/// a repeated key counts, as in Python's `json`.
pub(crate) fn first_vector(body: &[u8]) -> Option<Vec<f64>> {
    let top: BTreeMap<String, Box<RawValue>> = serde_json::from_slice(body).ok()?;
    let lists: Vec<&RawValue> = serde_json::from_str(top.get("embeddings")?.get()).ok()?;
    let first: Vec<&RawValue> = serde_json::from_str(lists.first()?.get()).ok()?;
    let vector = first
        .iter()
        .map(|item| {
            item.get()
                .parse::<f64>()
                .ok()
                .filter(|number| number.is_finite())
        })
        .collect::<Option<Vec<f64>>>()?;
    (!vector.is_empty()).then_some(vector)
}

/// The end of one try.
#[derive(Clone, Copy)]
struct Deadline(Instant);

impl Deadline {
    /// The time left. At the end it is zero, and a socket refuses a timeout of zero, so that the next
    /// connect, write or read of this try fails.
    fn left(self) -> Duration {
        self.0.saturating_duration_since(Instant::now())
    }
}

/// The time the connect to one address gets, given what is left of the deadline of the try: `CONNECT_MS`
/// at most, and the rest if that is less (zero if nothing is left, so that the connect fails at once).
pub(crate) fn connect_limit(left: Duration) -> Duration {
    left.min(Duration::from_millis(CONNECT_MS))
}

/// The answer being read: what has come in, and how much of it is used.
struct Wire {
    stream: TcpStream,
    deadline: Deadline,
    buf: Vec<u8>,
    used: usize,
}

impl Wire {
    /// Reads what is there into the buffer; `Ok(false)` at the end of the stream.
    fn fill(&mut self) -> Result<bool, Failed> {
        let mut chunk = [0_u8; 8192];
        frist_setzen(self.deadline.left(), |left| {
            self.stream.set_read_timeout(left)
        })?;
        let count = self.stream.read(&mut chunk)?;
        self.buf.extend_from_slice(&chunk[..count]);
        Ok(count > 0)
    }

    /// The next line without its line ending (`\n`, with a `\r` before it dropped). A line has at most
    /// `HEAD_LINE_MAX` bytes before the `\n`.
    fn line(&mut self) -> Result<Vec<u8>, Failed> {
        let mut from = self.used;
        loop {
            // Never look further than the longest line and its `\n`.
            let window = self.buf.len().min(self.used + HEAD_LINE_MAX + 1);
            if let Some(at) = self.buf[from..window].iter().position(|b| *b == b'\n') {
                let end = from + at;
                let mut line = self.buf[self.used..end].to_vec();
                if line.last() == Some(&b'\r') {
                    line.pop();
                }
                self.used = end + 1;
                return Ok(line);
            }
            if window - self.used > HEAD_LINE_MAX {
                return Err(INVALID);
            }
            from = window;
            if !self.fill()? {
                return Err(INVALID);
            }
        }
    }
}

/// Whether `text` is not empty and every byte is a digit of the kind `is_digit` says (no sign, no space).
fn all_digits(text: &str, is_digit: fn(&u8) -> bool) -> bool {
    !text.is_empty() && text.bytes().all(|b| is_digit(&b))
}

/// What the head of an answer says about its body.
struct Head {
    chunked: bool,
    length: Option<usize>,
}

/// Reads the status line and the header lines; `Err` unless the status is 200.
fn read_head(wire: &mut Wire) -> Result<Head, Failed> {
    let status = wire.line()?;
    let status = std::str::from_utf8(&status).map_err(|_| INVALID)?;
    let mut parts = status.splitn(3, ' ');
    let version = parts.next().unwrap_or_default();
    if version != "HTTP/1.1" && version != "HTTP/1.0" {
        return Err(INVALID);
    }
    if parts.next() != Some("200") {
        return Err(INVALID);
    }
    let mut head = Head {
        chunked: false,
        length: None,
    };
    for _ in 0..HEAD_LINES_MAX {
        let line = wire.line()?;
        if line.is_empty() {
            return Ok(head);
        }
        let line = std::str::from_utf8(&line).map_err(|_| INVALID)?;
        let (name, value) = line.split_once(':').ok_or(INVALID)?;
        let value = value.trim_matches([' ', '\t']);
        if name.eq_ignore_ascii_case("transfer-encoding") {
            if !value.eq_ignore_ascii_case("chunked") {
                return Err(INVALID);
            }
            head.chunked = true;
        } else if name.eq_ignore_ascii_case("content-length") {
            if !all_digits(value, u8::is_ascii_digit) {
                return Err(INVALID);
            }
            let length: usize = value.parse().map_err(|_| INVALID)?;
            if head.length.is_some_and(|before| before != length) {
                return Err(INVALID);
            }
            head.length = Some(length);
        }
    }
    Err(INVALID)
}

/// Reads `count` bytes of the body.
fn take(wire: &mut Wire, count: usize) -> Result<Vec<u8>, Failed> {
    if count > BODY_MAX {
        return Err(INVALID);
    }
    while wire.buf.len() - wire.used < count {
        if !wire.fill()? {
            return Err(INVALID);
        }
    }
    let part = wire.buf[wire.used..wire.used + count].to_vec();
    wire.used += count;
    Ok(part)
}

/// Reads a body in chunks: sizes in hexadecimal (an extension after `;` is ignored), a last chunk of
/// size 0, then trailer lines up to the empty line.
fn read_chunked(wire: &mut Wire) -> Result<Vec<u8>, Failed> {
    let mut body = Vec::new();
    loop {
        let line = wire.line()?;
        let size = std::str::from_utf8(&line).map_err(|_| INVALID)?;
        let size = size
            .split(';')
            .next()
            .unwrap_or_default()
            .trim_matches([' ', '\t']);
        if !all_digits(size, u8::is_ascii_hexdigit) {
            return Err(INVALID);
        }
        let size = usize::from_str_radix(size, 16).map_err(|_| INVALID)?;
        if size == 0 {
            for _ in 0..HEAD_LINES_MAX {
                if wire.line()?.is_empty() {
                    return Ok(body);
                }
            }
            return Err(INVALID);
        }
        if body.len() + size > BODY_MAX {
            return Err(INVALID);
        }
        body.extend_from_slice(&take(wire, size)?);
        if !wire.line()?.is_empty() {
            return Err(INVALID);
        }
    }
}

/// Reads the body that follows the head: in chunks, of the length the head names, or to the end of
/// the stream (the request says `Connection: close`).
fn read_body(wire: &mut Wire, head: &Head) -> Result<Vec<u8>, Failed> {
    if head.chunked {
        return read_chunked(wire);
    }
    if let Some(length) = head.length {
        return take(wire, length);
    }
    while wire.fill()? {
        if wire.buf.len() - wire.used > BODY_MAX {
            return Err(INVALID);
        }
    }
    Ok(wire.buf[wire.used..].to_vec())
}

/// The call to the local Ollama.
#[derive(Clone, Debug)]
pub struct Embedder {
    endpoint: Endpoint,
    model: String,
    keep_alive: String,
    pub(crate) timeout: Duration,
}

impl Embedder {
    /// An embedder for the address `url`; `None` unless it is a loopback address of the form
    /// `http://127.0.0.1:<port>`, `http://localhost:<port>` or `http://[::1]:<port>`. `timeout_ms`
    /// is the deadline of one try, at least 1.
    #[must_use]
    pub fn new(url: &str, model: &str, keep_alive: &str, timeout_ms: u64) -> Option<Self> {
        Some(Self {
            endpoint: parse_endpoint(url)?,
            model: model.to_string(),
            keep_alive: keep_alive.to_string(),
            timeout: Duration::from_millis(timeout_ms.clamp(1, TIMEOUT_MAX_MS)),
        })
    }

    /// The vector of `question`: the first try and, after a failure, one more. `None` when both fail.
    #[must_use]
    pub fn embed(&self, question: &str) -> Option<Vec<f64>> {
        self.einbetten(question).ok()
    }

    /// As [`Embedder::embed`], with the reason of the last try when both fail. The tries are the same.
    ///
    /// # Errors
    ///
    /// The [`Grund`] of the last try when no try gives a vector.
    pub fn einbetten(&self, question: &str) -> Result<Vec<f64>, Grund> {
        let body = request_body(&self.model, question, &self.keep_alive);
        let mut grund = Grund::AntwortUngueltig;
        for _ in 0..=RETRIES {
            match self
                .exchange(&body)
                .and_then(|answer| first_vector(&answer).ok_or(INVALID))
            {
                Ok(vector) => return Ok(vector),
                Err(Failed(last)) => grund = last,
            }
        }
        Err(grund)
    }

    /// One try: connects, writes the request, reads the answer, closes the connection. The body of
    /// the answer is returned only for the status 200.
    fn exchange(&self, body: &str) -> Result<Vec<u8>, Failed> {
        let deadline = Deadline(Instant::now() + self.timeout);
        let stream = self.connect(deadline)?;
        let request = format!(
            "POST /api/embed HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\n\
             Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
            self.endpoint.host,
            body.len()
        );
        let mut wire = Wire {
            stream,
            deadline,
            buf: Vec::new(),
            used: 0,
        };
        frist_setzen(deadline.left(), |left| wire.stream.set_write_timeout(left))?;
        wire.stream.write_all(request.as_bytes())?;
        // The connection is closed when `wire` goes out of scope, after every answer and every failure.
        read_head(&mut wire).and_then(|head| read_body(&mut wire, &head))
    }

    /// Connects to the first address that answers; each address gets `CONNECT_MS` at most, and never
    /// more than the rest of the deadline of the try.
    fn connect(&self, deadline: Deadline) -> Result<TcpStream, Failed> {
        for addr in &self.endpoint.addrs {
            if let Ok(stream) = TcpStream::connect_timeout(addr, connect_limit(deadline.left())) {
                return Ok(stream);
            }
        }
        Err(Failed(Grund::KeineVerbindung))
    }
}
