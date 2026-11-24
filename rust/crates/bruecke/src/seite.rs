//! The view (BR4, addendum of 21.11.2026): the three files of `rust/ansicht/ausgabe/` (crate `ansicht`, Dioxus-web),
//! built into the program. The bridge reads no file at run time except the start file.

/// The page for `GET /`.
pub(crate) const HTML: &[u8] = include_bytes!("../../../ansicht/ausgabe/index.html");
/// The script for `GET /ansicht.js` (the glue of wasm-bindgen, which loads the module).
pub(crate) const SKRIPT: &[u8] = include_bytes!("../../../ansicht/ausgabe/ansicht.js");
/// The module for `GET /ansicht_bg.wasm`.
pub(crate) const WASM: &[u8] = include_bytes!("../../../ansicht/ausgabe/ansicht_bg.wasm");

/// The `Content-Security-Policy` of all three: only `'self'`, `'none'`, `'unsafe-inline'`, `'wasm-unsafe-eval'` (in
/// `script-src`, so that the browser compiles the module) and `data:`.
pub(crate) const CSP: &str = "default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline'; \
connect-src 'self'; img-src 'self' data:; base-uri 'none'; form-action 'none'; frame-ancestors 'none'";

#[cfg(test)]
mod tests {
    use super::*;

    /// The namespaces of XML are names, not addresses; the module names the one of SVG (contract `bruecke`, addendum).
    const NAMENSRAEUME: [&[u8]; 6] = [
        b"http://www.w3.org/2000/svg",
        b"http://www.w3.org/1999/xlink",
        b"http://www.w3.org/1998/Math/MathML",
        b"http://www.w3.org/1999/xhtml",
        b"http://www.w3.org/XML/1998/namespace",
        b"http://www.w3.org/2000/xmlns/",
    ];

    fn ohne_adresse(d: &[u8]) -> bool {
        (0..d.len().saturating_sub(2))
            .filter(|&i| &d[i..i + 3] == b"://")
            .all(|i| i >= 4 && NAMENSRAEUME.iter().any(|n| d[i - 4..].starts_with(n)))
    }

    #[test]
    fn no_file_names_an_address() {
        for d in [HTML, SKRIPT] {
            assert!(!d.is_empty());
            assert!(!d.windows(3).any(|w| w == b"://"));
        }
        assert!(WASM.starts_with(b"\0asm\x01\0\0\0"));
        assert!(ohne_adresse(WASM));
        assert!(!ohne_adresse(b"x https://example.invalid/"));
        assert!(ohne_adresse(b"<svg xmlns=\"http://www.w3.org/2000/svg\">"));
    }

    #[test]
    fn the_files_load_each_other_relatively() {
        let html = String::from_utf8_lossy(HTML);
        let js = String::from_utf8_lossy(SKRIPT);
        assert!(html.contains("<script type=\"module\" src=\"ansicht.js\"></script>"));
        assert!(js.contains("new URL('ansicht_bg.wasm', import.meta.url)"));
        assert!(!html.contains("seite.js") && !js.contains("snippets/"));
    }

    #[test]
    fn the_policy_names_only_allowed_sources() {
        let erlaubt = [
            "'self'",
            "'none'",
            "'unsafe-inline'",
            "'wasm-unsafe-eval'",
            "data:",
        ];
        let mut namen = Vec::new();
        for teil in CSP.split(';') {
            let mut worte = teil.split_whitespace();
            let name = worte.next().expect("a directive");
            namen.push(name);
            for q in worte {
                assert!(erlaubt.contains(&q), "{q}");
            }
        }
        assert!(namen.contains(&"default-src") && namen.contains(&"frame-ancestors"));
        assert!(
            CSP.contains("base-uri 'none'")
                && CSP.contains("form-action 'none'")
                && CSP.contains("frame-ancestors 'none'")
        );
    }
}
