//! The receiver side of one connection (the decoder of the viewer): the pictures per session number and the
//! Deflate context (contract rules RA10, RA11, RA13).

use std::collections::HashMap;

use crate::bild::{Abbild, Auszug};
use crate::deflate::Entpacker;
use crate::fehler::Fehler;
use crate::rahmen::{RAHMEN_MAX, rahmen_dekodieren};

/// The receiver of one connection. A frame changes the picture of its session number only when the whole frame
/// is valid.
pub struct Empfang {
    abbilder: HashMap<u8, Abbild>,
    entpacker: Option<Entpacker>,
    deflate: bool,
}

impl Empfang {
    /// A new receiver without any picture. With `deflate` the bytes it takes are a raw Deflate stream with one
    /// context for all frames (RA11), created at the first frame.
    #[must_use]
    pub fn neu(deflate: bool) -> Empfang {
        Empfang {
            abbilder: HashMap::new(),
            entpacker: None,
            deflate,
        }
    }

    /// Takes the bytes of the wire and returns the extract and the picture of the session number afterwards.
    ///
    /// # Errors
    ///
    /// Every error of rules RA10 and RA11. After [`Fehler::InflateFehler`] or [`Fehler::RahmenZuGross`] on a
    /// Deflate connection the receiver is broken; every other error leaves the pictures as they were.
    pub fn nimm(&mut self, wire: &[u8]) -> Result<(Auszug, &Abbild), Fehler> {
        let (_, auszug, nr) = self.nimm_intern(wire)?;
        Ok((auszug, &self.abbilder[&nr]))
    }

    /// Like [`Empfang::nimm`], but also returns the frame bytes after inflating.
    ///
    /// # Errors
    ///
    /// See [`Empfang::nimm`].
    pub fn nimm_roh(&mut self, wire: &[u8]) -> Result<(Vec<u8>, Auszug, &Abbild), Fehler> {
        let (roh, auszug, nr) = self.nimm_intern(wire)?;
        Ok((roh, auszug, &self.abbilder[&nr]))
    }

    fn nimm_intern(&mut self, wire: &[u8]) -> Result<(Vec<u8>, Auszug, u8), Fehler> {
        let roh = if self.deflate {
            self.entpacker
                .get_or_insert_with(Entpacker::neu)
                .entpacken(wire)?
        } else if wire.len() > RAHMEN_MAX {
            return Err(Fehler::RahmenZuGross);
        } else {
            wire.to_vec()
        };
        let (auszug, neu) = rahmen_dekodieren(&roh, &self.abbilder)?;
        let nr = auszug.nr;
        self.abbilder.insert(nr, neu);
        Ok((roh, auszug, nr))
    }

    /// The picture of a session number.
    #[must_use]
    pub fn abbild(&self, nr: u8) -> Option<&Abbild> {
        self.abbilder.get(&nr)
    }

    /// Forgets the picture of a session number (RA13).
    pub fn vergessen(&mut self, nr: u8) {
        self.abbilder.remove(&nr);
    }
}
