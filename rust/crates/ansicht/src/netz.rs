//! The network of the view (contract `seite` SE6, contract `bruecke` section 5): only `GET /strom`, `POST /eingabe`
//! and `POST /starten` of the same origin, the token only in the header `Authorization`, never in the address.
//!
//! Compiled for every target so that the gates check it; it calls the browser only on `wasm32`.

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;

use js_sys::{Promise, Reflect, Uint8Array};
use serde_json::Value;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{AbortController, Headers, ReadableStreamDefaultReader, RequestInit, Response};

/// The token from the fragment `#t=<64 lower-case hex>`; otherwise there is none.
#[must_use]
pub fn token_aus_fragment(hash: &str) -> Option<String> {
    let t = hash.strip_prefix("#t=")?;
    (t.len() == 64 && t.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')))
        .then(|| t.to_owned())
}

/// The largest `n` of a frame on the stream: the art plus the bytes of the largest raster frame.
///
/// Contract `dienst` DI3 bounds only frames into the service (`n = 1 048 576`). The largest frame out is
/// `Raster`, whose payload is a frame on the line after `RA`: at most [`rasterauszug::RAHMEN_MAX`] bytes (RA11).
pub const N_MAX: usize = rasterauszug::RAHMEN_MAX + 1;

/// Why the stream cannot be split any further; after it the stream must end.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ZerlegerFehler {
    /// A head with `n = 0` (DI3 `laenge_ungueltig`).
    LaengeUngueltig,
    /// A head with `n` above [`N_MAX`] (DI3 `rahmen_zu_gross`).
    RahmenZuGross,
}

impl ZerlegerFehler {
    /// The fixed word of the error, the same as in `dienst` DI3.
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            ZerlegerFehler::LaengeUngueltig => "laenge_ungueltig",
            ZerlegerFehler::RahmenZuGross => "rahmen_zu_gross",
        }
    }
}

/// Splits the bytes of the stream into frames `[n: u32 big endian][art: u8][payload]` (contract `dienst` section 5).
#[derive(Debug, Default)]
pub struct Zerleger {
    puffer: Vec<u8>,
    kaputt: Option<ZerlegerFehler>,
}

impl Zerleger {
    /// Takes a piece of the stream and returns the complete frames as (art, payload).
    ///
    /// # Errors
    ///
    /// [`ZerlegerFehler`] as soon as the four bytes of a head with `n = 0` or `n` above [`N_MAX`] are there; the
    /// frames of the same piece before it are dropped with the stream. After an error the splitter keeps nothing
    /// and returns the same error for every later piece.
    pub fn nimm(&mut self, stueck: &[u8]) -> Result<Vec<(u8, Vec<u8>)>, ZerlegerFehler> {
        if let Some(e) = self.kaputt {
            return Err(e);
        }
        self.puffer.extend_from_slice(stueck);
        let mut raus = Vec::new();
        let mut p = 0;
        while self.puffer.len() - p >= 4 {
            let n = u32::from_be_bytes([
                self.puffer[p],
                self.puffer[p + 1],
                self.puffer[p + 2],
                self.puffer[p + 3],
            ]);
            // `n <= N_MAX` before any sum; `checked_add` still keeps a 32-bit `usize` (wasm32) from wrapping
            let ende = match usize::try_from(n) {
                Ok(0) => Err(ZerlegerFehler::LaengeUngueltig),
                Ok(n) if n <= N_MAX => n
                    .checked_add(4)
                    .and_then(|l| l.checked_add(p))
                    .ok_or(ZerlegerFehler::RahmenZuGross),
                _ => Err(ZerlegerFehler::RahmenZuGross),
            };
            let ende = match ende {
                Ok(ende) => ende,
                Err(e) => {
                    self.kaputt = Some(e);
                    self.puffer = Vec::new();
                    return Err(e);
                }
            };
            if self.puffer.len() < ende {
                break;
            }
            raus.push((self.puffer[p + 4], self.puffer[p + 5..ende].to_vec()));
            p = ende;
        }
        self.puffer.drain(..p);
        Ok(raus)
    }
}

fn fenster() -> Option<web_sys::Window> {
    web_sys::window()
}

fn kopf(token: &str, json: bool) -> Result<Headers, JsValue> {
    let h = Headers::new()?;
    h.append("Authorization", &format!("Bearer {token}"))?;
    if json {
        h.append("Content-Type", "application/json")?;
    }
    Ok(h)
}

async fn holen(ziel: &str, init: &RequestInit) -> Result<Response, JsValue> {
    let w = fenster().ok_or(JsValue::NULL)?;
    let r = JsFuture::from(w.fetch_with_str_and_init(ziel, init)).await?;
    r.dyn_into::<Response>()
}

/// Sends the events of the view one after the other: the next `POST /eingabe` only after the answer to the previous
/// one (SE1 point 11).
#[derive(Debug)]
pub struct Sender {
    token: String,
    verbindung: RefCell<Option<String>>,
    schlange: RefCell<VecDeque<(String, String)>>,
    laeuft: Cell<bool>,
}

impl Sender {
    /// A sender for this token; it sends nothing until the stream names its connection.
    #[must_use]
    pub fn neu(token: &str) -> Rc<Sender> {
        Rc::new(Sender {
            token: token.to_owned(),
            verbindung: RefCell::new(None),
            schlange: RefCell::new(VecDeque::new()),
            laeuft: Cell::new(false),
        })
    }

    /// The connection id of the stream (`X-Verbindung`), or none after its end.
    pub fn verbindung(&self, v: Option<String>) {
        *self.verbindung.borrow_mut() = v;
    }

    /// Queues one event.
    pub fn sende(self: &Rc<Self>, ereignis: &Value) {
        let Some(v) = self.verbindung.borrow().clone() else {
            return;
        };
        self.schlange
            .borrow_mut()
            .push_back((v, ereignis.to_string()));
        if self.laeuft.replace(true) {
            return;
        }
        let s = Rc::clone(self);
        wasm_bindgen_futures::spawn_local(async move {
            loop {
                let naechste = s.schlange.borrow_mut().pop_front();
                let Some((v, body)) = naechste else { break };
                let _ = s.eingabe(&v, &body).await;
            }
            s.laeuft.set(false);
        });
    }

    async fn eingabe(&self, verbindung: &str, body: &str) -> Result<(), JsValue> {
        let h = kopf(&self.token, true)?;
        h.append("X-Verbindung", verbindung)?;
        let init = RequestInit::new();
        init.set_method("POST");
        init.set_headers(&h.into());
        init.set_body(&JsValue::from_str(body));
        holen("/eingabe", &init).await.map(|_| ())
    }
}

/// `POST /starten` with the start structure: (status, answer); status 0 without a connection.
pub async fn starten(token: &str, start: &Value) -> (u16, Option<Value>) {
    let ergebnis = async {
        let init = RequestInit::new();
        init.set_method("POST");
        init.set_headers(&kopf(token, true)?.into());
        init.set_body(&JsValue::from_str(&start.to_string()));
        let r = holen("/starten", &init).await?;
        let text = JsFuture::from(r.text()?)
            .await
            .ok()
            .and_then(|t| t.as_string());
        Ok::<_, JsValue>((r.status(), text))
    }
    .await;
    match ergebnis {
        Ok((status, text)) => (status, text.and_then(|t| serde_json::from_str(&t).ok())),
        Err(_) => (0, None),
    }
}

/// The open stream: what the bridge said in the headers, and the reader of its body.
pub struct Strom {
    /// `X-Verbindung`.
    pub verbindung: Option<String>,
    /// `X-Deflate: 1`.
    pub deflate: bool,
    leser: ReadableStreamDefaultReader,
}

/// Opens `GET /strom`; `None` when it fails or the answer is not 200 with a body.
pub async fn strom_oeffnen(token: &str, abbruch: &AbortController) -> Option<Strom> {
    let init = RequestInit::new();
    init.set_headers(&kopf(token, false).ok()?.into());
    init.set_signal(Some(&abbruch.signal()));
    let r = holen("/strom", &init).await.ok()?;
    if !r.ok() {
        return None;
    }
    let body = r.body()?;
    let leser = body
        .get_reader()
        .dyn_into::<ReadableStreamDefaultReader>()
        .ok()?;
    let h = r.headers();
    Some(Strom {
        verbindung: h.get("X-Verbindung").ok().flatten(),
        deflate: h.get("X-Deflate").ok().flatten().as_deref() == Some("1"),
        leser,
    })
}

impl Strom {
    /// The next piece of the body; `None` at the end or on an error.
    pub async fn lesen(&self) -> Option<Vec<u8>> {
        let r = JsFuture::from(self.leser.read()).await.ok()?;
        if Reflect::get(&r, &JsValue::from_str("done")).ok()?.as_bool() != Some(false) {
            return None;
        }
        let wert = Reflect::get(&r, &JsValue::from_str("value")).ok()?;
        Some(Uint8Array::new(&wert).to_vec())
    }
}

/// Milliseconds of the page clock (`performance.now()`).
#[must_use]
pub fn jetzt() -> f64 {
    fenster()
        .and_then(|w| w.performance())
        .map_or(0.0, |p| p.now())
}

/// Waits `ms` milliseconds of the page clock (`setTimeout`).
pub async fn warten(ms: f64) {
    let Some(w) = fenster() else { return };
    let p = Promise::new(&mut |ja, _nein| {
        // the delay is at most a few seconds; the cast keeps whole milliseconds
        #[allow(clippy::cast_possible_truncation)]
        let ms = ms.ceil().clamp(0.0, 600_000.0) as i32;
        let _ = w.set_timeout_with_callback_and_timeout_and_arguments_0(&ja, ms);
    });
    let _ = JsFuture::from(p).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_token_comes_only_from_its_fragment_form() {
        let t = "a".repeat(64);
        assert_eq!(token_aus_fragment(&format!("#t={t}")), Some(t.clone()));
        assert_eq!(token_aus_fragment(&format!("#{t}")), None);
        assert_eq!(token_aus_fragment(&format!("#t={}", "A".repeat(64))), None);
        assert_eq!(token_aus_fragment(&format!("#t={t}0")), None);
        assert_eq!(token_aus_fragment(""), None);
    }

    #[test]
    fn frames_in_any_pieces() {
        let mut z = Zerleger::default();
        let rahmen = [0, 0, 0, 3, 6, b'{', b'}', 0, 0, 0, 1, 5];
        assert_eq!(z.nimm(&rahmen[..2]), Ok(Vec::new()));
        assert_eq!(z.nimm(&rahmen[2..9]), Ok(vec![(6, b"{}".to_vec())]));
        assert_eq!(z.nimm(&rahmen[9..]), Ok(vec![(5, Vec::new())]));
    }

    #[test]
    fn a_head_with_zero_or_above_the_bound_ends_the_stream() {
        // n = 0: an error, not a silent stop; every later piece gets the same error
        let mut z = Zerleger::default();
        assert_eq!(
            z.nimm(&[0, 0, 0, 0, 6]),
            Err(ZerlegerFehler::LaengeUngueltig)
        );
        assert_eq!(
            z.nimm(&[0, 0, 0, 1, 5]),
            Err(ZerlegerFehler::LaengeUngueltig)
        );
        // n just above the largest frame: an error as soon as the four bytes of the head are there
        let mut z = Zerleger::default();
        let zu_gross = u32::try_from(N_MAX + 1).expect("fits");
        assert_eq!(
            z.nimm(&zu_gross.to_be_bytes()),
            Err(ZerlegerFehler::RahmenZuGross)
        );
        // the n where `4 + n` wraps a 32-bit usize (wasm32), and the largest n of a u32: an error, never a panic
        for n in [0xFFFF_FFFC_u32, u32::MAX] {
            let mut z = Zerleger::default();
            assert_eq!(z.nimm(&n.to_be_bytes()), Err(ZerlegerFehler::RahmenZuGross));
        }
        // a frame before the bad head in the same piece goes down with the stream
        let mut z = Zerleger::default();
        assert_eq!(
            z.nimm(&[0, 0, 0, 1, 5, 0, 0, 0, 0]),
            Err(ZerlegerFehler::LaengeUngueltig)
        );
        assert_eq!(ZerlegerFehler::LaengeUngueltig.code(), "laenge_ungueltig");
        assert_eq!(ZerlegerFehler::RahmenZuGross.code(), "rahmen_zu_gross");
    }

    #[test]
    fn a_head_at_the_bound_waits_for_its_payload() {
        let mut z = Zerleger::default();
        let n = u32::try_from(N_MAX).expect("fits");
        assert_eq!(z.nimm(&n.to_be_bytes()), Ok(Vec::new()));
        assert_eq!(z.nimm(&[5]), Ok(Vec::new()));
        let nutz = vec![7; N_MAX - 1];
        let (a, b) = nutz.split_at(N_MAX / 2);
        assert_eq!(z.nimm(a), Ok(Vec::new()));
        assert_eq!(z.nimm(b), Ok(vec![(5, nutz.clone())]));
        assert_eq!(z.puffer, Vec::<u8>::new());
    }

    #[test]
    fn a_head_outside_the_bound_keeps_no_buffer() {
        // n = 0: the stream must not stop silently with every later frame stuck in the buffer
        let mut z = Zerleger::default();
        let _ = z.nimm(&[0, 0, 0, 0, 6]);
        let _ = z.nimm(&[0, 0, 0, 1, 5]);
        assert!(z.puffer.is_empty(), "n = 0 keeps {} bytes", z.puffer.len());
        // a huge n: the buffer must not grow with every later piece
        let mut z = Zerleger::default();
        let _ = z.nimm(&0x7FFF_FFFF_u32.to_be_bytes());
        let _ = z.nimm(&[0; 1000]);
        assert!(
            z.puffer.is_empty(),
            "a huge n keeps {} bytes",
            z.puffer.len()
        );
    }
}
