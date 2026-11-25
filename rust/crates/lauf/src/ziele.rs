//! The target list of mode 2 of the model path (contract 8, section 12), built from `netzziele` of the cloud route.
//!
//! Only targets with `zugriff: kopf-einsetzen` in the plain host form go into the list: the list knows neither the
//! subdomain form nor the kinds `verbinden` and `http-lesen` (contract 8, section 12: "keine IP-Adresse, kein `*`").
//! A target with `kopf-einsetzen` in the subdomain form cannot be carried: the start ends with `lauf.zielliste` before
//! the list and the CA (see [`hat_unterdomaene`]). Targets with `verbinden` or `http-lesen` stay out of the list and
//! are not terminated by the model path; the enclosure has no way to them (fail closed).

use einstellungen::json::text_schreiben;
use einstellungen::{Route, Zugriff};

/// The most targets the model path takes in its list (contract 8, section 12: 1 to 16).
pub const ZIELE_MAX: usize = 16;

/// One target of the list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ziel {
    /// The name for the log line, `ziel-<n>`.
    pub name: String,
    /// The host.
    pub host: String,
    /// The port.
    pub port: u16,
    /// Whether the key of the route is bound to the target.
    pub gebunden: bool,
    /// The (method, path) pairs.
    pub regeln: Vec<(String, String)>,
}

/// Whether the route has a target with `kopf-einsetzen` in the subdomain form `*.name`. The list cannot carry it
/// (contract 8, section 12: no `*`), and a start without it would run silently without that target.
#[must_use]
pub fn hat_unterdomaene(route: &Route) -> bool {
    route
        .netzziele()
        .iter()
        .any(|z| z.zugriff() == Zugriff::KopfEinsetzen && z.ist_unterdomaene())
}

/// The targets of the route that the list can carry.
#[must_use]
pub fn ziele(route: &Route) -> Vec<Ziel> {
    route
        .netzziele()
        .iter()
        .filter(|z| z.zugriff() == Zugriff::KopfEinsetzen && !z.ist_unterdomaene())
        .enumerate()
        .map(|(i, z)| Ziel {
            name: format!("ziel-{}", i + 1),
            host: z.host().to_owned(),
            port: z.port(),
            gebunden: z.schluessel(),
            regeln: z
                .pfade()
                .iter()
                .map(|r| (r.methode().to_owned(), r.pfad().to_owned()))
                .collect(),
        })
        .collect()
}

/// The text of the list file. A bound target names `halter_ziel`, the host that the holder knows (contract 8,
/// section 12); the key of such a target never lies in a file of the model path any more, so the list names no key
/// file, no header and no value prefix.
#[must_use]
pub fn text(ziele: &[Ziel], ca_zulassung: &str) -> String {
    let mut s = String::from("{\"fassung\":1,\"ca_zulassung\":");
    s.push_str(&text_schreiben(ca_zulassung));
    s.push_str(",\"ziele\":[");
    for (i, z) in ziele.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str("{\"name\":");
        s.push_str(&text_schreiben(&z.name));
        s.push_str(",\"host\":");
        s.push_str(&text_schreiben(&z.host));
        s.push_str(",\"port\":");
        s.push_str(&z.port.to_string());
        s.push_str(",\"halter_ziel\":");
        if z.gebunden {
            s.push_str(&text_schreiben(&z.host));
        } else {
            s.push_str("null");
        }
        s.push_str(",\"regeln\":[");
        for (j, (methode, pfad)) in z.regeln.iter().enumerate() {
            if j > 0 {
                s.push(',');
            }
            s.push_str("{\"methode\":");
            s.push_str(&text_schreiben(methode));
            s.push_str(",\"pfad\":");
            s.push_str(&text_schreiben(pfad));
            s.push('}');
        }
        s.push_str("],\"upgrade\":");
        s.push_str(if z.gebunden {
            "\"durchreichen\""
        } else {
            "\"ablehnen\""
        });
        s.push('}');
    }
    s.push_str("]}\n");
    s
}

#[cfg(test)]
mod tests;
