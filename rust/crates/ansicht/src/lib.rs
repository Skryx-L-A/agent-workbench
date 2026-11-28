//! Browser view of the new interface: the code view of today's workbench (A157 stage 2a) as a thin view over the
//! stream of the service (A118), rendered with Dioxus-web and built for `wasm32-unknown-unknown`.
//!
//! - [`kern`]: the state of the view and the rules of the contract `seite` (events in, effects out, no DOM).
//! - [`tasten`], [`start`], [`ansagen`], [`zeile`]: keyboard, start form, announcements, look of a run.
//! - [`netz`]: the three routes of the bridge; [`oberflaeche`] and [`fenster`]: the components; [`aussehen`]: the look and
//!   what the view remembers per device.
//! - [`ziel`]: the target protocol for the gaps L1 to L11 at one place; [`bedienwege`]: the table of section 2 and the rule
//!   which operating path shows; [`woerter`]: the words both views share.
//!
//! The raster frames are read with `rasterauszug`, the decoder of the new build; no second decoder exists here.

pub mod ansagen;
pub mod aussehen;
pub mod bedienwege;
pub mod fenster;
pub mod kern;
pub mod netz;
pub mod oberflaeche;
pub mod start;
pub mod tasten;
pub mod woerter;
pub mod zeile;
pub mod ziel;
