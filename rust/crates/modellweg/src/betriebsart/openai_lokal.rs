//! Mode 1 `openai-lokal` (contract 8, part B): its options (section 9), the stages that belong to it (path, method, body,
//! model), the exact request to the target and the tokens of the answer (section 10).

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde_json::Value;

use super::Regeln;
use crate::aufruf::{PFAD_MAX, modell_ok, pfad_ok};
use crate::codes::Code;
use crate::http::Anfragekopf;
use crate::json::lies_streng;
use crate::schluessel::Schluessel;
use crate::tokens::Zaehler;
use crate::ziel::Ziel;

/// The options of this mode.
pub const OPTIONEN: [&str; 3] = ["--ziel", "--modell", "--schluessel-datei"];

/// The only path of the harness.
pub const PFAD: &str = "/v1/chat/completions";

/// The checked options of this mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Optionen {
    /// The target.
    pub ziel: Ziel,
    /// The one model let through.
    pub modell: String,
    /// The key file.
    pub schluessel_datei: PathBuf,
}

impl Optionen {
    /// Takes the three options out of `werte` and checks their form; `None` is `mw.aufruf`.
    pub(crate) fn lies(werte: &mut BTreeMap<String, String>) -> Option<Self> {
        let [ziel, modell, datei] = OPTIONEN.map(|name| werte.remove(name));
        let (ziel, modell, datei) = (ziel?, modell?, datei?);
        if !(modell_ok(&modell) && pfad_ok(&datei, PFAD_MAX)) {
            return None;
        }
        Some(Self {
            ziel: Ziel::lies(&ziel)?,
            modell,
            schluessel_datei: PathBuf::from(datei),
        })
    }
}

/// The rules of this mode for one process.
pub(crate) struct OpenaiLokal {
    modell: String,
}

impl OpenaiLokal {
    pub(crate) fn neu(modell: String) -> Self {
        Self { modell }
    }
}

impl Regeln for OpenaiLokal {
    fn pruefe_kopf(&self, kopf: &Anfragekopf) -> Result<(), Code> {
        if kopf.ziel != PFAD {
            Err(Code::PfadUnbekannt)
        } else if kopf.methode != "POST" {
            Err(Code::MethodeFalsch)
        } else {
            Ok(())
        }
    }

    fn pruefe_koerper(&self, koerper: &[u8]) -> Result<(), Code> {
        let Some(Value::Object(objekt)) = lies_streng(koerper) else {
            return Err(Code::JsonUngueltig);
        };
        if objekt.get("model").and_then(Value::as_str) == Some(self.modell.as_str()) {
            Ok(())
        } else {
            Err(Code::ModellNichtErlaubt)
        }
    }

    fn an_ziel(
        &self,
        kopf: &Anfragekopf,
        koerper: &[u8],
        ziel: &Ziel,
        schluessel: &Schluessel,
    ) -> Vec<u8> {
        let mut v = Vec::with_capacity(koerper.len() + schluessel.bytes().len() + 256);
        v.extend_from_slice(
            format!("POST {}/chat/completions HTTP/1.1\r\n", ziel.praefix).as_bytes(),
        );
        v.extend_from_slice(format!("Host: {}\r\n", ziel.host_kopf()).as_bytes());
        v.extend_from_slice(b"Authorization: Bearer ");
        v.extend_from_slice(schluessel.bytes());
        v.extend_from_slice(b"\r\nContent-Type: application/json\r\n");
        let mut accept = kopf.werte("accept");
        if let (Some(wert), None) = (accept.next(), accept.next())
            && (wert == "application/json" || wert == "text/event-stream")
        {
            v.extend_from_slice(format!("Accept: {wert}\r\n").as_bytes());
        }
        v.extend_from_slice(
            format!(
                "Content-Length: {}\r\nConnection: close\r\n\r\n",
                koerper.len()
            )
            .as_bytes(),
        );
        v.extend_from_slice(koerper);
        v
    }

    fn zaehler(&self) -> Zaehler {
        Zaehler::neu()
    }
}

#[cfg(test)]
mod tests {
    use super::{OpenaiLokal, Regeln};
    use crate::http::Anfragekopf;
    use crate::schluessel;
    use crate::ziel::Ziel;

    fn kopf(accept: &[&str]) -> Anfragekopf {
        Anfragekopf {
            methode: "POST".to_owned(),
            ziel: "/v1/chat/completions".to_owned(),
            felder: accept
                .iter()
                .map(|a| ("accept".to_owned(), (*a).to_owned()))
                .collect(),
        }
    }

    fn an_ziel(accept: &[&str]) -> String {
        let regeln = OpenaiLokal::neu("m".to_owned());
        let ziel = Ziel::lies("https://h:8/p/v1").expect("valid");
        let dir =
            std::env::temp_dir().join(format!("mw-unit-{}-{}", std::process::id(), accept.len()));
        std::fs::write(&dir, b"K\n").expect("write");
        let mut rechte = std::fs::metadata(&dir).expect("meta").permissions();
        std::os::unix::fs::PermissionsExt::set_mode(&mut rechte, 0o600);
        std::fs::set_permissions(&dir, rechte).expect("chmod");
        let k = schluessel::lies(&dir, crate::os::eigene_uid()).expect("key");
        std::fs::remove_file(&dir).expect("remove");
        String::from_utf8(regeln.an_ziel(&kopf(accept), b"{}", &ziel, &k)).expect("ascii")
    }

    #[test]
    fn accept_goes_out_only_alone_and_known() {
        let ohne = "POST /p/v1/chat/completions HTTP/1.1\r\nHost: h:8\r\nAuthorization: Bearer K\r\nContent-Type: application/json\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}";
        assert_eq!(an_ziel(&[]), ohne);
        assert_eq!(an_ziel(&["text/html"]), ohne);
        assert_eq!(an_ziel(&["application/json", "application/json"]), ohne);
        assert!(an_ziel(&["application/json"]).contains("Accept: application/json\r\n"));
        assert!(an_ziel(&["text/event-stream"]).contains("Accept: text/event-stream\r\n"));
    }

    #[test]
    fn the_model_must_be_a_text_equal_to_the_call() {
        let r = OpenaiLokal::neu("m".to_owned());
        assert_eq!(r.pruefe_koerper(br#"{"model":"m"}"#), Ok(()));
        assert!(r.pruefe_koerper(br#"{"model":["m"]}"#).is_err());
        assert!(r.pruefe_koerper(br#"{"x":{"model":"m"}}"#).is_err());
    }
}
