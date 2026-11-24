//! Rejections, answers and start errors of the bridge.

use std::io;

/// An error at the start: the exit code (2 call or start file, 3 rights, 1 bind or randomness) and one line of text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StartFehler {
    /// The exit code of the program.
    pub code: u8,
    /// The text of the one line on stderr (after `wb-bruecke: `). It names no value of the input.
    pub text: &'static str,
}

/// A request that ends with a status and no content. `grund` is one fixed word of the protocol (BR6), never content of
/// the request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Abgelehnt {
    /// The status of the answer (400 and above).
    pub status: u16,
    /// The word of the protocol line.
    pub grund: &'static str,
}

/// The way a connection ends early: silently (the peer left, a read or write failed) or with a rejection.
#[derive(Debug)]
pub(crate) enum Abbruch {
    Stumm,
    Abgelehnt(Abgelehnt),
}

impl From<io::Error> for Abbruch {
    fn from(_: io::Error) -> Self {
        Self::Stumm
    }
}

impl From<Abgelehnt> for Abbruch {
    fn from(a: Abgelehnt) -> Self {
        Self::Abgelehnt(a)
    }
}

/// One answer of the bridge: status, optional content with its type, and extra heads in a fixed order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Antwort {
    /// The status code.
    pub status: u16,
    /// The `Content-Type`, sent only with content.
    pub typ: Option<&'static str>,
    /// The content; a rejection has none.
    pub koerper: Vec<u8>,
    /// Further heads (name, value), written before `Connection`.
    pub extra: Vec<(&'static str, String)>,
}

impl Antwort {
    /// An answer without content.
    #[must_use]
    pub fn leer(status: u16) -> Self {
        Self {
            status,
            typ: None,
            koerper: Vec::new(),
            extra: Vec::new(),
        }
    }

    /// The answer to a rejection: no content.
    #[must_use]
    pub fn abgelehnt(a: Abgelehnt) -> Self {
        Self::leer(a.status)
    }

    /// An answer with content and type.
    #[must_use]
    pub fn mit(status: u16, typ: &'static str, koerper: Vec<u8>) -> Self {
        Self {
            status,
            typ: Some(typ),
            koerper,
            extra: Vec::new(),
        }
    }

    /// The bytes on the wire: status line, `Content-Length`, `Content-Type` (only with content), `Cache-Control`,
    /// `X-Content-Type-Options`, the extra heads, `Connection: close`, the empty line and the content. No `Set-Cookie`
    /// and no `Access-Control-` head ever.
    #[must_use]
    pub fn bytes(&self) -> Vec<u8> {
        let mut kopf = format!(
            "HTTP/1.1 {} {}\r\nContent-Length: {}\r\n",
            self.status,
            grund_text(self.status),
            self.koerper.len()
        );
        if let (Some(typ), false) = (self.typ, self.koerper.is_empty()) {
            kopf.push_str("Content-Type: ");
            kopf.push_str(typ);
            kopf.push_str("\r\n");
        }
        kopf.push_str("Cache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\n");
        for (name, wert) in &self.extra {
            kopf.push_str(name);
            kopf.push_str(": ");
            kopf.push_str(wert);
            kopf.push_str("\r\n");
        }
        kopf.push_str("Connection: close\r\n\r\n");
        let mut aus = kopf.into_bytes();
        aus.extend_from_slice(&self.koerper);
        aus
    }
}

/// The reason phrase of a status.
pub(crate) fn grund_text(status: u16) -> &'static str {
    match status {
        200 => "OK",
        204 => "No Content",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        408 => "Request Timeout",
        411 => "Length Required",
        413 => "Payload Too Large",
        414 => "URI Too Long",
        415 => "Unsupported Media Type",
        422 => "Unprocessable Entity",
        431 => "Request Header Fields Too Large",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        504 => "Gateway Timeout",
        _ => "Status",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reason_phrases_of_every_status() {
        let soll = [
            (200, "OK"),
            (204, "No Content"),
            (400, "Bad Request"),
            (401, "Unauthorized"),
            (403, "Forbidden"),
            (404, "Not Found"),
            (405, "Method Not Allowed"),
            (408, "Request Timeout"),
            (411, "Length Required"),
            (413, "Payload Too Large"),
            (414, "URI Too Long"),
            (415, "Unsupported Media Type"),
            (422, "Unprocessable Entity"),
            (431, "Request Header Fields Too Large"),
            (502, "Bad Gateway"),
            (503, "Service Unavailable"),
            (504, "Gateway Timeout"),
            (999, "Status"),
        ];
        for (status, text) in soll {
            assert_eq!(grund_text(status), text, "{status}");
        }
    }

    #[test]
    fn empty_answer_has_zero_length_and_no_type() {
        let a = Antwort::leer(401).bytes();
        assert_eq!(
            String::from_utf8(a).unwrap_or_default(),
            "HTTP/1.1 401 Unauthorized\r\nContent-Length: 0\r\nCache-Control: no-store\r\n\
             X-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n"
        );
    }

    #[test]
    fn answer_with_content_names_its_type_and_length() {
        let a = Antwort::mit(200, "application/json", b"{}".to_vec()).bytes();
        assert_eq!(
            String::from_utf8(a).unwrap_or_default(),
            "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nContent-Type: application/json\r\n\
             Cache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n{}"
        );
    }

    #[test]
    fn a_type_without_content_is_not_sent() {
        let mut a = Antwort::mit(200, "text/plain", Vec::new());
        a.extra.push(("X-Test", "1".to_string()));
        let text = String::from_utf8(a.bytes()).unwrap_or_default();
        assert!(!text.contains("\r\nContent-Type:"), "{text}");
        assert!(
            text.contains(
                "X-Content-Type-Options: nosniff\r\nX-Test: 1\r\nConnection: close\r\n\r\n"
            ),
            "{text}"
        );
    }

    #[test]
    fn a_rejection_has_no_content() {
        let a = Antwort::abgelehnt(Abgelehnt {
            status: 403,
            grund: "host",
        });
        assert_eq!(a.status, 403);
        assert!(a.koerper.is_empty() && a.typ.is_none() && a.extra.is_empty());
    }

    #[test]
    fn conversions_into_the_early_end() {
        assert!(matches!(
            Abbruch::from(io::Error::from(io::ErrorKind::BrokenPipe)),
            Abbruch::Stumm
        ));
        let a = Abgelehnt {
            status: 400,
            grund: "json",
        };
        assert!(matches!(Abbruch::from(a), Abbruch::Abgelehnt(b) if b == a));
    }
}
