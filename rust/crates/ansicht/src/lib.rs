//! Browser view of the new interface: the code view of today's workbench (A157 stage 2a) as a thin view over the
//! stream of the service (A118), rendered with Dioxus-web and built for `wasm32-unknown-unknown`.
//!
//! - [`kern`]: the state of the view and the rules of the contract `seite` (events in, effects out, no DOM).
//! - [`tasten`], [`start`], [`ansagen`], [`zeile`]: keyboard, start form, announcements, look of a run.
//! - [`netz`]: the three routes of the bridge; [`oberflaeche`]: the components.
//!
//! The raster frames are read with `rasterauszug`, the decoder of the new build; no second decoder exists here.

pub mod ansagen;
pub mod kern;
pub mod netz;
pub mod oberflaeche;
pub mod start;
pub mod tasten;
pub mod zeile;
