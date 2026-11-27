//! The narrow interface between the window and the service (OF14 point 2, gap L11).
//!
//! The window speaks the messages and events of the stream of the bridge (contract `dienst`): frames
//! `[art][payload]` come in (5 raster, 6 message, 7 rejection, 8 error), events of the view go out as JSON objects. The
//! local path of the service for viewers on the same computer (a Unix socket, a named pipe under Windows) is not built
//! yet; until then [`crate::attrappe::Attrappe`] stands behind this interface. The real path replaces only it.

use serde_json::Value;

use crate::bedienwege::Luecken;

/// One frame of the stream: its kind and its payload.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rahmen {
    /// Kind (contract `dienst`, section 5).
    pub art: u8,
    /// Payload.
    pub nutz: Vec<u8>,
}

/// The path to the service.
pub trait Weg {
    /// Sends one event of the view, in order (SE1 point 11).
    fn senden(&mut self, ereignis: &Value);
    /// Takes the frames that arrived since the last call.
    fn abholen(&mut self) -> Vec<Rahmen>;
    /// Whether the stream is open; `false` after its end.
    fn offen(&self) -> bool;
    /// The gaps of section 4 this path has closed: which operating paths the window may show (OF8).
    fn geschlossen(&self) -> Luecken;
    /// Whether this path is a mock-up: then every picture of the window carries the caption of OF8 point 3.
    fn ist_attrappe(&self) -> bool;
}
