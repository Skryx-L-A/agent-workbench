//! The effective deadline of a release for each MCP server (contract `rust/spec/regelebenen/README.md`, 15.1.2; decision of
//! the operator, 27.11.2026).
//!
//! For each server the smaller value of the catalogue and the company file holds. Shorter is stricter for both deadlines
//! (`aufsicht.freigabefrist_ms` and `aufsicht.einloesefrist_ms`). A global value never loosens a stricter value of a single
//! server. A server without an own value gets the value of the catalogue. With the same value the value of the server holds:
//! the catalogue neither loosens nor tightens it.
//!
//! A pure function: the caller hands in the effective value of the catalogue (from [`crate::sicht`]) and the servers of the
//! company file with their own values, in its order; it gets back one entry per server in the same order.

use serde_json::{Value, json};

/// Where the effective value of a server comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Herkunft {
    /// The catalogue: the server has no own value, or a larger one.
    Katalog,
    /// The company file: the own value of the server, smaller than or equal to the catalogue.
    Firmendatei,
}

impl Herkunft {
    /// The fixed text of the origin, as in the cases (`katalog`, `firmendatei`).
    #[must_use]
    pub fn text(self) -> &'static str {
        match self {
            Herkunft::Katalog => "katalog",
            Herkunft::Firmendatei => "firmendatei",
        }
    }
}

/// The effective deadline of one server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wirksam {
    /// The key of the server in the block `mcp` of the company file.
    pub server: String,
    /// The effective value in milliseconds.
    pub wert: i64,
    /// Where the value comes from.
    pub herkunft: Herkunft,
}

impl Wirksam {
    /// The entry as in the cases: `{"server", "wert", "herkunft"}`.
    #[must_use]
    pub fn als_wert(&self) -> Value {
        json!({"server": self.server, "wert": self.wert, "herkunft": self.herkunft.text()})
    }
}

/// The effective deadline for each server: the own value of the server when it is smaller than or equal to `katalog`, else
/// `katalog`. `server` holds the key of each server and its own value of the company file (`None`: no own value).
#[must_use]
pub fn wirksam(katalog: i64, server: &[(&str, Option<i64>)]) -> Vec<Wirksam> {
    server
        .iter()
        .map(|&(name, eigen)| match eigen {
            Some(w) if w <= katalog => Wirksam {
                server: name.to_owned(),
                wert: w,
                herkunft: Herkunft::Firmendatei,
            },
            _ => Wirksam {
                server: name.to_owned(),
                wert: katalog,
                herkunft: Herkunft::Katalog,
            },
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kurz(w: &[Wirksam]) -> Vec<(&str, i64, &str)> {
        w.iter()
            .map(|x| (x.server.as_str(), x.wert, x.herkunft.text()))
            .collect()
    }

    #[test]
    fn the_case_of_the_decision_a_stricter_server_holds() {
        assert_eq!(
            kurz(&wirksam(120_000, &[("x", Some(30_000))])),
            [("x", 30_000, "firmendatei")]
        );
    }

    #[test]
    fn a_global_value_never_loosens_and_may_tighten() {
        let w = wirksam(
            60_000,
            &[
                ("streng", Some(1)),
                ("gleich", Some(60_000)),
                ("locker", Some(86_400_000)),
                ("ohne", None),
            ],
        );
        assert_eq!(
            kurz(&w),
            [
                ("streng", 1, "firmendatei"),
                ("gleich", 60_000, "firmendatei"),
                ("locker", 60_000, "katalog"),
                ("ohne", 60_000, "katalog"),
            ]
        );
        assert_eq!(wirksam(60_000, &[]), Vec::<Wirksam>::new());
        assert_eq!(
            w[0].als_wert(),
            json!({"server": "streng", "wert": 1, "herkunft": "firmendatei"})
        );
    }
}
