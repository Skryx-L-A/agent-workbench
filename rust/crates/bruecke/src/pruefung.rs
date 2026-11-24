//! The order of the checks (BR3) in ONE place.
//!
//! First the form of the request (400, sizes 414 and 431), then `Host` (403), then `Origin` (403), then the token (401;
//! not for `GET /`, `GET /ansicht.js` and `GET /ansicht_bg.wasm`), then method and route (405, 404). For a POST [`post_pruefen`] adds the content
//! type (415) and the length (411, 413). The first check that applies answers; nothing after it is looked at.

use crate::fehler::Abgelehnt;
use crate::form::{self, Anfrage};
use crate::token::token_gleich;

/// The `/starten` body has at most this many bytes.
pub(crate) const MAX_KOERPER_STARTEN: usize = 16_384;
/// The `/eingabe` body has at most this many bytes (1 MiB minus 1: with its kind the frame never exceeds the limit of
/// the service).
pub(crate) const MAX_KOERPER_EINGABE: usize = 1_048_575;

/// The routes of the bridge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Route {
    Seite,
    Skript,
    Wasm,
    Strom,
    Starten,
    Eingabe,
}

fn abgelehnt(status: u16, grund: &'static str) -> Abgelehnt {
    Abgelehnt { status, grund }
}

/// Checks a head in the fixed order. `port` is the bound port, `token` the token of this run.
pub(crate) fn pruefen(kopf: &[u8], port: u16, token: &[u8]) -> Result<(Anfrage, Route), Abgelehnt> {
    let anfrage = form::lesen(kopf)?;
    host_pruefen(&anfrage, port)?;
    origin_pruefen(&anfrage, port)?;
    let oeffentlich = matches!(
        (anfrage.methode.as_str(), anfrage.ziel.as_str()),
        ("GET", "/" | "/ansicht.js" | "/ansicht_bg.wasm")
    );
    if !oeffentlich {
        token_pruefen(&anfrage, token)?;
    }
    let route = route_pruefen(&anfrage)?;
    Ok((anfrage, route))
}

fn host_pruefen(a: &Anfrage, port: u16) -> Result<(), Abgelehnt> {
    let Some(host) = a.kopf("host") else {
        return Err(abgelehnt(400, "anfrage_form"));
    };
    if host == format!("127.0.0.1:{port}").as_bytes()
        || host == format!("localhost:{port}").as_bytes()
    {
        Ok(())
    } else {
        Err(abgelehnt(403, "host"))
    }
}

fn origin_pruefen(a: &Anfrage, port: u16) -> Result<(), Abgelehnt> {
    match a.kopf("origin") {
        None if a.methode == "POST" => Err(abgelehnt(403, "origin")),
        None => Ok(()),
        Some(o)
            if o == format!("http://127.0.0.1:{port}").as_bytes()
                || o == format!("http://localhost:{port}").as_bytes() =>
        {
            Ok(())
        }
        Some(_) => Err(abgelehnt(403, "origin")),
    }
}

fn token_pruefen(a: &Anfrage, token: &[u8]) -> Result<(), Abgelehnt> {
    let vorgelegt = a
        .kopf("authorization")
        .and_then(|w| w.strip_prefix(b"Bearer "));
    match vorgelegt {
        Some(v) if token_gleich(v, token) => Ok(()),
        _ => Err(abgelehnt(401, "token")),
    }
}

fn route_pruefen(a: &Anfrage) -> Result<Route, Abgelehnt> {
    let (route, methode) = match a.ziel.as_str() {
        "/" => (Route::Seite, "GET"),
        "/ansicht.js" => (Route::Skript, "GET"),
        "/ansicht_bg.wasm" => (Route::Wasm, "GET"),
        "/strom" => (Route::Strom, "GET"),
        "/starten" => (Route::Starten, "POST"),
        "/eingabe" => (Route::Eingabe, "POST"),
        _ => return Err(abgelehnt(404, "route")),
    };
    if a.methode == methode {
        Ok(route)
    } else {
        Err(abgelehnt(405, "methode"))
    }
}

/// Is `typ` `application/json`, with or without `; charset=utf-8` (case does not matter)?
fn json_typ(typ: &[u8]) -> bool {
    let klein = typ.to_ascii_lowercase();
    match klein.strip_prefix(b"application/json") {
        Some([]) => true,
        Some([b';', rest @ ..]) => {
            let von = rest
                .iter()
                .position(|b| !matches!(b, b' ' | b'\t'))
                .unwrap_or(rest.len());
            &rest[von..] == b"charset=utf-8"
        }
        _ => false,
    }
}

/// Step 6 of the order for a POST: content type (415), length present (411), length within the limit (413). Returns
/// the length of the body.
pub(crate) fn post_pruefen(a: &Anfrage, route: Route) -> Result<usize, Abgelehnt> {
    if !a.kopf("content-type").is_some_and(json_typ) {
        return Err(abgelehnt(415, "inhalt_typ"));
    }
    let Some(laenge) = a.kopf("content-length") else {
        return Err(abgelehnt(411, "koerper_laenge"));
    };
    let grenze = if route == Route::Eingabe {
        MAX_KOERPER_EINGABE
    } else {
        MAX_KOERPER_STARTEN
    };
    // the form check made it 1 to 10 digits
    let n = laenge
        .iter()
        .fold(0u64, |s, d| s * 10 + u64::from(d - b'0'));
    match usize::try_from(n) {
        Ok(n) if n <= grenze => Ok(n),
        _ => Err(abgelehnt(413, "koerper_zu_gross")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOKEN: &[u8] = b"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    fn kopf(zeilen: &[&str]) -> Vec<u8> {
        zeilen.join("\r\n").into_bytes()
    }

    fn mit_token() -> String {
        format!("Authorization: Bearer {}", String::from_utf8_lossy(TOKEN))
    }

    fn pruefen_z(zeilen: &[&str]) -> Result<(Anfrage, Route), Abgelehnt> {
        pruefen(&kopf(zeilen), 4711, TOKEN)
    }

    fn ablehnung(zeilen: &[&str]) -> (u16, &'static str) {
        let e = pruefen_z(zeilen).expect_err("must be rejected");
        (e.status, e.grund)
    }

    #[test]
    fn the_public_pages_need_no_token_and_no_origin() {
        let (a, r) = pruefen_z(&["GET / HTTP/1.1", "Host: 127.0.0.1:4711"]).expect("ok");
        assert_eq!((a.ziel.as_str(), r), ("/", Route::Seite));
        let (_, r) = pruefen_z(&[
            "GET /ansicht.js HTTP/1.1",
            "Host: localhost:4711",
            "Origin: http://localhost:4711",
        ])
        .expect("ok");
        assert_eq!(r, Route::Skript);
        let (_, r) =
            pruefen_z(&["GET /ansicht_bg.wasm HTTP/1.1", "Host: 127.0.0.1:4711"]).expect("ok");
        assert_eq!(r, Route::Wasm);
    }

    #[test]
    fn every_route_with_the_token() {
        let t = mit_token();
        let h = "Host: 127.0.0.1:4711";
        let o = "Origin: http://127.0.0.1:4711";
        assert_eq!(
            pruefen_z(&["GET /strom HTTP/1.1", h, &t]).expect("ok").1,
            Route::Strom
        );
        assert_eq!(
            pruefen_z(&["POST /starten HTTP/1.1", h, o, &t])
                .expect("ok")
                .1,
            Route::Starten
        );
        assert_eq!(
            pruefen_z(&["POST /eingabe HTTP/1.1", h, o, &t])
                .expect("ok")
                .1,
            Route::Eingabe
        );
    }

    #[test]
    fn the_order_is_form_host_origin_token_route() {
        let t = mit_token();
        let h = "Host: 127.0.0.1:4711";
        let o = "Origin: http://127.0.0.1:4711";
        // each request breaks the rule named and every rule after it; the earlier one answers
        assert_eq!(
            ablehnung(&["GET / HTTP/1.0", "Host: x", "Origin: x"]),
            (400, "anfrage_form")
        );
        assert_eq!(
            ablehnung(&["GET /x HTTP/1.1", "Origin: x"]),
            (400, "anfrage_form")
        );
        assert_eq!(
            ablehnung(&["POST /x HTTP/1.1", "Host: evil", "Origin: evil"]),
            (403, "host")
        );
        assert_eq!(
            ablehnung(&["POST /x HTTP/1.1", h, "Origin: evil"]),
            (403, "origin")
        );
        assert_eq!(ablehnung(&["POST /x HTTP/1.1", h]), (403, "origin"));
        assert_eq!(ablehnung(&["POST /x HTTP/1.1", h, o]), (401, "token"));
        assert_eq!(ablehnung(&["POST /x HTTP/1.1", h, o, &t]), (404, "route"));
        assert_eq!(
            ablehnung(&["PUT /eingabe HTTP/1.1", h, o, &t]),
            (405, "methode")
        );
        assert_eq!(ablehnung(&["GET /x HTTP/1.1", h]), (401, "token"));
        assert_eq!(ablehnung(&["GET /x HTTP/1.1", h, &t]), (404, "route"));
        assert_eq!(
            ablehnung(&["GET /starten HTTP/1.1", h, &t]),
            (405, "methode")
        );
        assert_eq!(
            ablehnung(&["POST /strom HTTP/1.1", h, o, &t]),
            (405, "methode")
        );
    }

    #[test]
    fn the_public_pages_with_another_method_need_the_token() {
        let h = "Host: 127.0.0.1:4711";
        for m in ["HEAD", "OPTIONS", "POST", "PUT"] {
            let o = "Origin: http://127.0.0.1:4711";
            assert_eq!(
                ablehnung(&[&format!("{m} / HTTP/1.1"), h, o]),
                (401, "token"),
                "{m}"
            );
        }
        assert_eq!(ablehnung(&["GET /?a=1 HTTP/1.1", h]), (401, "token"));
        assert_eq!(ablehnung(&["GET /ansicht.js? HTTP/1.1", h]), (401, "token"));
        // the page of the walking skeleton is gone (addendum of 21.11.2026)
        assert_eq!(ablehnung(&["GET /seite.js HTTP/1.1", h]), (401, "token"));
        assert_eq!(ablehnung(&["GET /strom HTTP/1.1", h]), (401, "token"));
    }

    #[test]
    fn host_is_byte_for_byte() {
        for h in [
            "127.0.0.1",
            "localhost",
            "127.0.0.1:4712",
            "127.0.0.1:04711",
            "LOCALHOST:4711",
            "[::1]:4711",
            "evil127.0.0.1:4711",
            "127.0.0.1:4711x",
            "x127.0.0.1:4711",
            "localhost:4711.evil",
            "",
            "evil.example:4711",
        ] {
            let e = pruefen_z(&["GET / HTTP/1.1", &format!("Host: {h}")]).expect_err(h);
            assert_eq!((e.status, e.grund), (403, "host"), "{h:?}");
        }
        for h in ["127.0.0.1:4711", "localhost:4711"] {
            assert!(
                pruefen_z(&["GET / HTTP/1.1", &format!("Host: {h}")]).is_ok(),
                "{h}"
            );
        }
    }

    #[test]
    fn origin_is_byte_for_byte_and_a_post_needs_one() {
        let h = "Host: 127.0.0.1:4711";
        for o in [
            "null",
            "",
            "https://127.0.0.1:4711",
            "http://127.0.0.1",
            "http://127.0.0.1:4712",
            "http://127.0.0.1:4711/",
            "http://127.0.0.1:4711.evil",
            "http://127.0.0.1.evil:4711",
            "http://LOCALHOST:4711",
            "127.0.0.1:4711",
            "http://evil:4711",
        ] {
            let e = pruefen_z(&["GET / HTTP/1.1", h, &format!("Origin: {o}")]).expect_err(o);
            assert_eq!((e.status, e.grund), (403, "origin"), "{o:?}");
        }
        for o in ["http://127.0.0.1:4711", "http://localhost:4711"] {
            assert!(
                pruefen_z(&["GET / HTTP/1.1", h, &format!("Origin: {o}")]).is_ok(),
                "{o}"
            );
        }
        assert!(pruefen_z(&["GET / HTTP/1.1", h]).is_ok());
    }

    #[test]
    fn the_token_is_exactly_bearer_and_the_token() {
        let h = "Host: 127.0.0.1:4711";
        let t = String::from_utf8_lossy(TOKEN).into_owned();
        let letzter_anders = format!("{}0", &t[..63]);
        let erster_anders = format!("f{}", &t[1..]);
        for a in [
            format!("Authorization: {t}"),
            format!("Authorization: bearer {t}"),
            format!("Authorization: BEARER {t}"),
            format!("Authorization: Bearer{t}"),
            format!("Authorization: Bearer  {t}"),
            format!("Authorization: Bearer {t}x"),
            format!("Authorization: Bearer {}", &t[..63]),
            format!("Authorization: Bearer {}", &t[..8]),
            format!("Authorization: Bearer {letzter_anders}"),
            format!("Authorization: Bearer {erster_anders}"),
            format!("Authorization: Bearer {}", "a".repeat(64)),
            "Authorization: Bearer ".to_string(),
            "Authorization: Bearer".to_string(),
            "Authorization:".to_string(),
            format!("Authorization: Basic {t}"),
            format!("X-Authorization: Bearer {t}"),
        ] {
            let e = pruefen_z(&["GET /strom HTTP/1.1", h, &a]).expect_err(&a);
            assert_eq!((e.status, e.grund), (401, "token"), "{a}");
        }
        assert_eq!(
            ablehnung(&[&format!("GET /strom?token={t} HTTP/1.1"), h]),
            (401, "token")
        );
        assert!(
            pruefen_z(&[
                "GET /strom HTTP/1.1",
                h,
                &format!("Authorization: Bearer {t}")
            ])
            .is_ok()
        );
    }

    #[test]
    fn json_types() {
        for t in [
            "application/json",
            "Application/JSON",
            "application/json; charset=utf-8",
            "application/json;charset=utf-8",
            "application/json;\t charset=UTF-8",
        ] {
            assert!(json_typ(t.as_bytes()), "{t}");
        }
        for t in [
            "",
            "application/jsonx",
            "application/json;",
            "application/json; charset=latin1",
            "application/json; charset=utf-8x",
            "application/json charset=utf-8",
            "application/json; charset=utf-8; a=b",
            "text/plain",
            "application/json;\u{b}charset=utf-8",
            "xapplication/json",
            "application/json ",
        ] {
            assert!(!json_typ(t.as_bytes()), "{t:?}");
        }
    }

    fn post(route: &str, kopfe: &[&str]) -> Result<usize, Abgelehnt> {
        let mut z = vec![format!("POST {route} HTTP/1.1")];
        z.extend(kopfe.iter().map(|s| (*s).to_string()));
        let r: Vec<&str> = z.iter().map(String::as_str).collect();
        let a = form::lesen(&kopf(&r)).expect("form");
        post_pruefen(
            &a,
            if route == "/eingabe" {
                Route::Eingabe
            } else {
                Route::Starten
            },
        )
    }

    #[test]
    fn a_post_checks_type_then_length_then_size() {
        let json = "Content-Type: application/json";
        assert_eq!(post("/starten", &[]).expect_err("no type").status, 415);
        assert_eq!(
            post(
                "/starten",
                &["Content-Type: text/plain", "Content-Length: 99999999"]
            )
            .expect_err("type first")
            .status,
            415
        );
        assert_eq!(
            post("/starten", &[json]),
            Err(abgelehnt(411, "koerper_laenge"))
        );
        assert_eq!(
            post("/starten", &[json, "Content-Length: 9999999999"]),
            Err(abgelehnt(413, "koerper_zu_gross"))
        );
        assert_eq!(post("/starten", &["Content-Length: 5", json]), Ok(5));
        assert_eq!(post("/starten", &[json, "Content-Length: 0"]), Ok(0));
    }

    #[test]
    fn the_body_limits_are_at_the_byte() {
        let json = "Content-Type: application/json";
        let l = |n: usize| format!("Content-Length: {n}");
        assert_eq!(post("/starten", &[json, &l(16_384)]), Ok(16_384));
        assert_eq!(
            post("/starten", &[json, &l(16_385)]),
            Err(abgelehnt(413, "koerper_zu_gross"))
        );
        assert_eq!(post("/eingabe", &[json, &l(1_048_575)]), Ok(1_048_575));
        assert_eq!(
            post("/eingabe", &[json, &l(1_048_576)]),
            Err(abgelehnt(413, "koerper_zu_gross"))
        );
        assert_eq!(post("/eingabe", &[json, &l(16_385)]), Ok(16_385));
        assert_eq!(
            post("/eingabe", &[json, "Content-Length: 9999999999"]),
            Err(abgelehnt(413, "koerper_zu_gross"))
        );
        assert_eq!(
            post("/starten", &[json, "Content-Length: 0000000001"]),
            Ok(1)
        );
    }
}
