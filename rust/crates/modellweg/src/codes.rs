//! The fixed codes of the own answers (contract 8, sections 6, 7 and 9): code, status, text and the result of the line.

/// An own answer of the model path: a refusal of a stage, an error at the target or a blocked leak.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Code {
    /// Stage 1: appending a line failed before.
    ProtokollFehlt,
    /// Stage 2: not HTTP/1.1, head too long, `Content-Length` doubled, not decimal or not the length of the body.
    AnfrageUnlesbar,
    /// Stage 3: 8 requests are already in work.
    Ausgelastet,
    /// Stage 4: the request target is not exactly the path of the mode.
    PfadUnbekannt,
    /// Stage 5: the method is not `POST`.
    MethodeFalsch,
    /// Stage 6: a field `Transfer-Encoding` is present.
    ChunkedNichtUnterstuetzt,
    /// Stage 7: no `Content-Length`.
    LaengeFehlt,
    /// Stage 8: `Content-Length` above 33554432.
    KoerperZuGross,
    /// Stage 9: the body is no valid JSON object.
    JsonUngueltig,
    /// Stage 10: `model` is missing, no text or not the model of the call.
    ModellNichtErlaubt,
    /// Stage 11, only in a trial run: the count file cannot be written.
    ZaehlungFehler,
    /// The connection to the target failed.
    ZielNichtErreichbar,
    /// The target sent no head in time.
    ZielFrist,
    /// The answer of the target carried the key.
    LeckGesperrt,
}

impl Code {
    /// The code as the contract writes it (`mw.…`).
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Self::ProtokollFehlt => "mw.protokoll_fehlt",
            Self::AnfrageUnlesbar => "mw.anfrage_unlesbar",
            Self::Ausgelastet => "mw.ausgelastet",
            Self::PfadUnbekannt => "mw.pfad_unbekannt",
            Self::MethodeFalsch => "mw.methode_falsch",
            Self::ChunkedNichtUnterstuetzt => "mw.chunked_nicht_unterstuetzt",
            Self::LaengeFehlt => "mw.laenge_fehlt",
            Self::KoerperZuGross => "mw.koerper_zu_gross",
            Self::JsonUngueltig => "mw.json_ungueltig",
            Self::ModellNichtErlaubt => "mw.modell_nicht_erlaubt",
            Self::ZaehlungFehler => "mw.zaehlung_fehler",
            Self::ZielNichtErreichbar => "mw.ziel_nicht_erreichbar",
            Self::ZielFrist => "mw.ziel_frist",
            Self::LeckGesperrt => "mw.leck_gesperrt",
        }
    }

    /// The HTTP status of the answer.
    #[must_use]
    pub fn status(self) -> u16 {
        match self {
            Self::ProtokollFehlt | Self::ZaehlungFehler => 503,
            Self::AnfrageUnlesbar | Self::JsonUngueltig => 400,
            Self::Ausgelastet => 429,
            Self::PfadUnbekannt => 404,
            Self::MethodeFalsch => 405,
            Self::ChunkedNichtUnterstuetzt | Self::LaengeFehlt => 411,
            Self::KoerperZuGross => 413,
            Self::ModellNichtErlaubt => 403,
            Self::ZielNichtErreichbar | Self::LeckGesperrt => 502,
            Self::ZielFrist => 504,
        }
    }

    /// The fixed text of the answer (ASCII).
    #[must_use]
    pub fn text(self) -> &'static str {
        match self {
            Self::ProtokollFehlt => "Protokoll nicht schreibbar",
            Self::AnfrageUnlesbar => "Anfrage nicht lesbar",
            Self::Ausgelastet => "zu viele gleichzeitige Anfragen",
            Self::PfadUnbekannt => "Weg unbekannt",
            Self::MethodeFalsch => "nur POST",
            Self::ChunkedNichtUnterstuetzt => {
                "Transfer-Encoding nicht unterstuetzt, Content-Length ist Pflicht"
            }
            Self::LaengeFehlt => "Content-Length fehlt",
            Self::KoerperZuGross => "Koerper zu gross",
            Self::JsonUngueltig => "kein gueltiges JSON",
            Self::ModellNichtErlaubt => "Modell nicht erlaubt",
            Self::ZaehlungFehler => "Zaehlung nicht schreibbar",
            Self::ZielNichtErreichbar => "Ziel nicht erreichbar",
            Self::ZielFrist => "Ziel antwortet nicht",
            Self::LeckGesperrt => "Antwort des Ziels gesperrt",
        }
    }

    /// The field `ergebnis` of the line for this code (`mw.protokoll_fehlt` writes no line; it counts as a refusal).
    #[must_use]
    pub fn ergebnis(self) -> &'static str {
        match self {
            Self::ZielNichtErreichbar | Self::ZielFrist => "ziel_fehler",
            Self::LeckGesperrt => "leck_gesperrt",
            _ => "abgelehnt",
        }
    }

    /// The body of the answer, `{"error":{"code":…,"message":…}}` in RFC 8785 (code and text are ASCII without escapes).
    #[must_use]
    pub fn koerper(self) -> String {
        format!(
            "{{\"error\":{{\"code\":\"{}\",\"message\":\"{}\"}}}}",
            self.code(),
            self.text()
        )
    }

    /// The whole answer to the harness: status, `Content-Type: application/json`, length, `Connection: close`, body.
    #[must_use]
    pub fn antwort(self) -> Vec<u8> {
        let koerper = self.koerper();
        format!(
            "HTTP/1.1 {} \r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{koerper}",
            self.status(),
            koerper.len()
        )
        .into_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::Code;

    #[test]
    fn the_answer_frames_the_body() {
        let a = String::from_utf8(Code::MethodeFalsch.antwort()).expect("ascii");
        assert_eq!(
            a,
            "HTTP/1.1 405 \r\nContent-Type: application/json\r\nContent-Length: 59\r\nConnection: close\r\n\r\n{\"error\":{\"code\":\"mw.methode_falsch\",\"message\":\"nur POST\"}}"
        );
    }

    #[test]
    fn errors_at_the_target_have_their_own_result() {
        assert_eq!(Code::ZielNichtErreichbar.ergebnis(), "ziel_fehler");
        assert_eq!(Code::ZielFrist.ergebnis(), "ziel_fehler");
        assert_eq!(Code::LeckGesperrt.ergebnis(), "leck_gesperrt");
        assert_eq!(Code::JsonUngueltig.ergebnis(), "abgelehnt");
        assert_eq!(Code::ZielFrist.status(), 504);
        assert_eq!(Code::ZielFrist.text(), "Ziel antwortet nicht");
        assert_eq!(Code::ZielNichtErreichbar.status(), 502);
        assert_eq!(Code::ZielNichtErreichbar.text(), "Ziel nicht erreichbar");
    }
}
