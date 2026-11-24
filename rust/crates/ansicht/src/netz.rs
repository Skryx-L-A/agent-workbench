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

/// Splits the bytes of the stream into frames `[n: u32 big endian][art: u8][payload]` (contract `dienst` section 5).
#[derive(Debug, Default)]
pub struct Zerleger {
    puffer: Vec<u8>,
}

impl Zerleger {
    /// Takes a piece of the stream and returns the complete frames as (art, payload).
    pub fn nimm(&mut self, stueck: &[u8]) -> Vec<(u8, Vec<u8>)> {
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
            let Ok(n) = usize::try_from(n) else { break };
            if n < 1 || self.puffer.len() - p < 4 + n {
                break;
            }
            raus.push((self.puffer[p + 4], self.puffer[p + 5..p + 4 + n].to_vec()));
            p += 4 + n;
        }
        self.puffer.drain(..p);
        raus
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
        assert_eq!(z.nimm(&rahmen[..2]), Vec::new());
        assert_eq!(z.nimm(&rahmen[2..9]), vec![(6, b"{}".to_vec())]);
        assert_eq!(z.nimm(&rahmen[9..]), vec![(5, Vec::new())]);
    }
}
