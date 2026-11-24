//! The error codes of the contract (section 5): a closed set, each with its fixed text.

use std::fmt;

/// An error of the line form, of a message of the core or of a frame. [`Fehler::code`] is the text the contract
/// names; a rejected input never changes any state (except the Deflate context, rule RA11).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Fehler {
    /// `ESC` followed by a character other than `[` (RA4).
    EscUngueltig,
    /// The text ends inside an escape sequence (RA4).
    SgrUnvollstaendig,
    /// An escape sequence with a final character from `@` to `~` other than `m` (RA4).
    SgrUnbekannt,
    /// A malformed or unknown SGR parameter (RA1, RA4).
    SgrParameter,
    /// A control character or a C1 character in a text (RA4).
    ZeichenUngueltig,
    /// A style value out of range (section 4).
    StilUngueltig,
    /// A line that is not in canonical form (RA2).
    ZeileNichtKanonisch,
    /// A message of the core with another `art` than `Abzug` or `Rasteraenderung` (RA12).
    ArtUnbekannt,
    /// A message of the core of the wrong shape (RA12).
    NachrichtUngueltig,
    /// A size outside 2 to 1000 columns and 1 to 1000 rows (RA5).
    GroesseUngueltig,
    /// A cursor outside the picture (RA8).
    CursorUngueltig,
    /// A change or a frame without a full picture of that session number before it (RA12, RA10).
    KeinAbzug,
    /// A change whose `von` is not the stand of the model picture (RA12).
    Luecke,
    /// A stand that does not rise (RA12, RA10).
    StandAlt,
    /// A line index outside the picture (RA12, RA10).
    ZeileAusserhalb,
    /// A line of a message that does not read (RA12).
    ZeileUngueltig,
    /// A frame that ends too early (RA10).
    RahmenAbgeschnitten,
    /// Bytes after the last line of a frame (RA10).
    RahmenRest,
    /// A frame longer than 16 777 216 bytes (RA11).
    RahmenZuGross,
    /// A frame whose first byte is not 5 (RA10).
    ArtUngueltig,
    /// A frame with flag bits 16 to 128 set (RA10).
    FlagsUnbekannt,
    /// A frame with a full picture and a scroll together (RA10).
    RahmenUngueltig,
    /// A malformed varint (RA9, RA10).
    VuUngueltig,
    /// Text in a frame that is not valid UTF-8 (RA10).
    Utf8Ungueltig,
    /// A style number beyond the style table (RA10).
    StilnrUngueltig,
    /// A scroll that does not fit the picture (RA10).
    SchubUngueltig,
    /// Line indexes of a frame that do not rise strictly (RA10).
    ZeileReihenfolge,
    /// A Deflate stream that does not inflate (RA11).
    InflateFehler,
}

impl Fehler {
    /// Every error, in the order of the contract table (section 5).
    pub const ALLE: [Fehler; 28] = [
        Fehler::EscUngueltig,
        Fehler::SgrUnvollstaendig,
        Fehler::SgrUnbekannt,
        Fehler::SgrParameter,
        Fehler::ZeichenUngueltig,
        Fehler::StilUngueltig,
        Fehler::ZeileNichtKanonisch,
        Fehler::ArtUnbekannt,
        Fehler::NachrichtUngueltig,
        Fehler::GroesseUngueltig,
        Fehler::CursorUngueltig,
        Fehler::KeinAbzug,
        Fehler::Luecke,
        Fehler::StandAlt,
        Fehler::ZeileAusserhalb,
        Fehler::ZeileUngueltig,
        Fehler::RahmenAbgeschnitten,
        Fehler::RahmenRest,
        Fehler::RahmenZuGross,
        Fehler::ArtUngueltig,
        Fehler::FlagsUnbekannt,
        Fehler::RahmenUngueltig,
        Fehler::VuUngueltig,
        Fehler::Utf8Ungueltig,
        Fehler::StilnrUngueltig,
        Fehler::SchubUngueltig,
        Fehler::ZeileReihenfolge,
        Fehler::InflateFehler,
    ];

    /// The code of the contract, for example `luecke`.
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Fehler::EscUngueltig => "esc_ungueltig",
            Fehler::SgrUnvollstaendig => "sgr_unvollstaendig",
            Fehler::SgrUnbekannt => "sgr_unbekannt",
            Fehler::SgrParameter => "sgr_parameter",
            Fehler::ZeichenUngueltig => "zeichen_ungueltig",
            Fehler::StilUngueltig => "stil_ungueltig",
            Fehler::ZeileNichtKanonisch => "zeile_nicht_kanonisch",
            Fehler::ArtUnbekannt => "art_unbekannt",
            Fehler::NachrichtUngueltig => "nachricht_ungueltig",
            Fehler::GroesseUngueltig => "groesse_ungueltig",
            Fehler::CursorUngueltig => "cursor_ungueltig",
            Fehler::KeinAbzug => "kein_abzug",
            Fehler::Luecke => "luecke",
            Fehler::StandAlt => "stand_alt",
            Fehler::ZeileAusserhalb => "zeile_ausserhalb",
            Fehler::ZeileUngueltig => "zeile_ungueltig",
            Fehler::RahmenAbgeschnitten => "rahmen_abgeschnitten",
            Fehler::RahmenRest => "rahmen_rest",
            Fehler::RahmenZuGross => "rahmen_zu_gross",
            Fehler::ArtUngueltig => "art_ungueltig",
            Fehler::FlagsUnbekannt => "flags_unbekannt",
            Fehler::RahmenUngueltig => "rahmen_ungueltig",
            Fehler::VuUngueltig => "vu_ungueltig",
            Fehler::Utf8Ungueltig => "utf8_ungueltig",
            Fehler::StilnrUngueltig => "stilnr_ungueltig",
            Fehler::SchubUngueltig => "schub_ungueltig",
            Fehler::ZeileReihenfolge => "zeile_reihenfolge",
            Fehler::InflateFehler => "inflate_fehler",
        }
    }
}

impl fmt::Display for Fehler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

impl std::error::Error for Fehler {}
