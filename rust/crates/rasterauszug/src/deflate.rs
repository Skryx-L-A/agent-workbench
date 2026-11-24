//! Raw Deflate with one context per connection (contract rule RA11, RFC 7692 section 7.2.1).
//!
//! The sender compresses each frame with a sync flush and cuts the four end bytes `00 00 ff ff`; the receiver puts
//! them back and inflates with the context of the connection, never producing more than [`RAHMEN_MAX`] bytes.
//!
//! Both directions work in one call on a buffer whose size is known in advance, so no loop decides when to stop.

use flate2::{Compress, Compression, FlushCompress};
use miniz_oxide::inflate::TINFLStatus;
use miniz_oxide::inflate::core::inflate_flags::{
    TINFL_FLAG_HAS_MORE_INPUT, TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF,
};
use miniz_oxide::inflate::core::{DecompressorOxide, decompress};

use crate::fehler::Fehler;
use crate::rahmen::RAHMEN_MAX;

/// The four bytes a sync flush ends with.
const ENDE: [u8; 4] = [0x00, 0x00, 0xff, 0xff];

/// Slack added to every buffer.
const SPIELRAUM: usize = 65_536;

/// The window of Deflate: a match reaches back at most this many bytes.
const FENSTER: usize = 32_768;

/// The most bytes one input byte can inflate to: a match of 258 bytes costs at least 2 bits (one bit for the
/// length, one for the distance), so 8 bits give 4 matches, 1032 bytes. One more for the block headers.
const AUFBLAEHUNG: usize = 1033;

/// The most bytes one frame may inflate to, plus one byte that tells a frame that is too long.
const AUSGABE_MAX: usize = RAHMEN_MAX + 1;

/// The flags of the inflater: more input may follow (a frame ends inside the stream), and the output buffer is
/// contiguous (so a distance is checked against the bytes produced). The two flags are different bits.
const FLAGS: u32 = TINFL_FLAG_HAS_MORE_INPUT + TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF;

/// The compression level (the default of zlib).
const STUFE: u32 = 6;

/// The compressor of a connection: one context for all frames.
pub(crate) struct Verpacker {
    c: Compress,
}

impl Verpacker {
    pub(crate) fn neu() -> Verpacker {
        Verpacker {
            c: Compress::new(Compression::new(STUFE), false),
        }
    }

    /// The bytes on the wire for one frame: compressed with a sync flush, without the four end bytes.
    ///
    /// The compressor never writes more than the input plus a few bytes per 64 KiB block (it stores a block it
    /// cannot shrink), so a buffer of the input plus [`SPIELRAUM`] takes the whole frame and the flush in one
    /// call. A second sync flush without new input would add an empty block, so there is exactly one call.
    pub(crate) fn verpacken(&mut self, roh: &[u8]) -> Vec<u8> {
        let mut puffer = vec![0_u8; roh.len().saturating_add(SPIELRAUM)];
        let vorher = self.c.total_out();
        // The status says only whether all input was taken; the buffer is large enough that it was.
        let _ = self.c.compress(roh, &mut puffer, FlushCompress::Sync);
        let erzeugt = usize::try_from(self.c.total_out() - vorher).unwrap_or(0);
        let ohne_ende = erzeugt.saturating_sub(ENDE.len());
        let ende = if puffer[ohne_ende..erzeugt] == ENDE {
            ohne_ende
        } else {
            erzeugt
        };
        puffer[..ende].to_vec()
    }
}

/// The inflater of a connection: one context for all frames. After an inflate error or a frame that is too long
/// it is broken and gives the same error again.
pub(crate) struct Entpacker {
    d: Box<DecompressorOxide>,
    /// The last [`FENSTER`] bytes the connection produced: the window the next frame may reach back into.
    historie: Vec<u8>,
    /// The stream had its final block; later frames give no bytes (zlib does the same).
    ende: bool,
    defekt: Option<Fehler>,
}

impl Entpacker {
    pub(crate) fn neu() -> Entpacker {
        Entpacker {
            d: Box::new(DecompressorOxide::new()),
            historie: Vec::new(),
            ende: false,
            defekt: None,
        }
    }

    /// The frame bytes for the bytes of the wire.
    ///
    /// # Errors
    ///
    /// [`Fehler::InflateFehler`] for a stream that does not inflate, [`Fehler::RahmenZuGross`] when the frame
    /// would be longer than [`RAHMEN_MAX`]; after either the inflater is broken.
    pub(crate) fn entpacken(&mut self, wire: &[u8]) -> Result<Vec<u8>, Fehler> {
        if let Some(f) = self.defekt {
            return Err(f);
        }
        let ergebnis = self.inflate(wire);
        if let Err(f) = ergebnis {
            self.defekt = Some(f);
        }
        ergebnis
    }

    /// Inflates in one call into a buffer that holds the window in front and the new bytes behind it. The buffer
    /// is contiguous, so the inflater checks every distance against the bytes produced so far, and it is as
    /// large as the input can inflate to (at most one byte more than a frame may hold), so the call needs no
    /// second round.
    fn inflate(&mut self, wire: &[u8]) -> Result<Vec<u8>, Fehler> {
        if self.ende {
            return Ok(Vec::new());
        }
        let mut eingabe = wire.to_vec();
        eingabe.extend_from_slice(&ENDE);
        let platz = eingabe
            .len()
            .saturating_mul(AUFBLAEHUNG)
            .saturating_add(SPIELRAUM)
            .min(AUSGABE_MAX);
        let fenster = self.historie.len();
        let mut puffer = self.historie.clone();
        puffer.resize(fenster.saturating_add(platz), 0);
        let (status, _, erzeugt) = decompress(&mut self.d, &eingabe, &mut puffer, fenster, FLAGS);
        if erzeugt > RAHMEN_MAX {
            return Err(Fehler::RahmenZuGross);
        }
        match status {
            TINFLStatus::Done => self.ende = true,
            TINFLStatus::NeedsMoreInput => {}
            _ => return Err(Fehler::InflateFehler),
        }
        let ende = fenster.saturating_add(erzeugt);
        self.historie = puffer[ende.saturating_sub(FENSTER)..ende].to_vec();
        Ok(puffer[fenster..ende].to_vec())
    }
}
