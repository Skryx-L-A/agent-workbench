//! The words of the settings (OF9, OF14 point 7): every text of the dialog at one place.
//!
//! The words come from the table both views share, `ansicht::woerter` (OF1: one wording in both views); the names here
//! only point at it. What the table does not hold yet carries a marker comment:
//! - `TEXT fehlt`: neither today's app nor the browser view has a word; the window keeps its earlier one.
//!
//! Every marker goes away once `ansicht::woerter` holds the word.

use ansicht::woerter as w;

/// The heading of the user area.
pub const BEREICH_NUTZER: &str = w::BEREICH_NUTZER;
/// The heading of the admin area.
pub const BEREICH_ADMIN: &str = w::VERWALTUNG;
/// The badge of the admin area and of an admin page.
pub const MARKE_ADMIN: &str = w::ADMIN;
/// The name of the close button.
pub const SCHLIESSEN: &str = w::SCHLIESSEN;
/// A page without values.
pub const KEINE_WERTE: &str = w::KEINE_WERTE;

/// The origin of a look choice of this device: the table names it like a value of the user.
pub const HERKUNFT_GERAET: &str = w::HERKUNFT_DU;
/// The origin "company" (`regelebenen` 15.5).
pub const HERKUNFT_FIRMA: &str = w::HERKUNFT_FIRMA;
/// The origin "group", before the name of the group.
pub const HERKUNFT_GRUPPE: &str = w::HERKUNFT_GRUPPE;
/// The origin "user".
pub const HERKUNFT_DU: &str = w::HERKUNFT_DU;

/// The reason `nur_admin` (`regelebenen` 15.5).
pub const GRUND_NUR_ADMIN: &str = w::GRUND_NUR_ADMIN;
/// The reason `nur_strenger`.
pub const GRUND_NUR_STRENGER: &str = w::GRUND_NUR_STRENGER;
/// The reason `fest`.
pub const GRUND_FEST: &str = w::GRUND_FEST;
/// A lock without a reason, before the origin.
pub const GRUND_VORGABE: &str = w::GRUND_VORGABE;

/// A switch that is on.
pub const AN: &str = w::AN;
/// A switch that is off.
pub const AUS: &str = w::AUS;

/// The key of the font size of the terminal in the catalog of the pilot (`regelebenen` 15.1). SCHLUESSEL: the table of
/// words still names the row `aussehen.schriftgroesse`; the window uses the key of the catalog until the table follows.
pub const SCHLUESSEL_SCHRIFTGROESSE: &str = "terminal.schriftgroesse";

/// The pages: title and short description (one line, OF9 point 6).
pub mod seite {
    use ansicht::woerter::seite as s;
    /// The look.
    pub const AUSSEHEN: &str = s::AUSSEHEN;
    /// Its short description.
    pub const AUSSEHEN_KURZ: &str = s::AUSSEHEN_KURZ;
    /// The defaults of a new session.
    pub const SITZUNG: &str = s::SITZUNG;
    /// Its short description.
    pub const SITZUNG_KURZ: &str = s::SITZUNG_KURZ;
    /// Programs and models.
    pub const PROGRAMME: &str = s::PROGRAMME;
    /// User page.
    pub const PROGRAMME_KURZ: &str = s::PROGRAMME_KURZ;
    /// Admin page.
    pub const PROGRAMME_ADMIN_KURZ: &str = s::PROGRAMME_ADMIN_KURZ;
    /// The permissions.
    pub const ERLAUBNISSE: &str = s::ERLAUBNISSE;
    /// Their short description.
    pub const ERLAUBNISSE_KURZ: &str = s::ERLAUBNISSE_KURZ;
    /// Oversight.
    pub const AUFSICHT: &str = s::AUFSICHT;
    /// Its short description.
    pub const AUFSICHT_KURZ: &str = s::AUFSICHT_KURZ;
}

/// The groups of the pages.
pub mod gruppe {
    use ansicht::woerter::gruppe as g;
    /// Light and dark.
    pub const FARBEN: &str = g::FARBEN;
    /// The font.
    pub const SCHRIFT: &str = g::SCHRIFT;
    /// What a new session starts with.
    pub const NEUE_SITZUNGEN: &str = g::NEUE_SITZUNGEN;
    /// Release of programs and models.
    pub const FREIGABE: &str = g::FREIGABE;
    /// Limits of the effort.
    pub const EFFORT: &str = g::EFFORT;
    /// The options of Copilot.
    pub const COPILOT: &str = g::COPILOT;
    /// The network.
    pub const NETZ: &str = g::NETZ;
    /// The access of the maker.
    pub const HERSTELLER: &str = g::HERSTELLER;
    /// The two deadlines of a release.
    pub const FRISTEN: &str = g::FRISTEN;
}

/// One row: key, name and a description of at most one line.
pub type Zeile = ansicht::woerter::Zeile;

/// The rows of the look (choices of this device).
pub mod aussehen {
    use super::Zeile;
    use ansicht::woerter::aussehen as a;
    /// Light, dark or like the system.
    pub const THEMA: Zeile = a::THEMA;
    /// The colours of the dark theme.
    pub const FARBSATZ: Zeile = a::FARBSATZ;
    /// The colours of the terminal.
    pub const TERMINAL: Zeile = a::TERMINAL;
    /// The font size of the terminal: name and description of the table, the key of the catalog (see
    /// [`super::SCHLUESSEL_SCHRIFTGROESSE`]).
    pub const SCHRIFTGROESSE: Zeile = (
        super::SCHLUESSEL_SCHRIFTGROESSE,
        a::SCHRIFTGROESSE.1,
        a::SCHRIFTGROESSE.2,
    );
}

/// The rows of the values of the service (`regelebenen` 15.1), in the order of the pages.
pub mod werte {
    // the names of the two deadlines are also the heads of the columns of the table of deadlines per server
    pub use ansicht::woerter::werte::{
        EFFORT, EFFORT_GRENZEN, EINLOESEFRIST, FREIGABEFRIST, HARNESS, HARNESSES, HERSTELLERZUGANG,
        LOKALE_CA, MODELL, MODELLE, OHNE_RUECKFRAGE, TELEMETRIE, TOKEN,
    };
}

/// The information on the page of supervision: the measured state of the protection layers with its date, read only,
/// never "on" or "running" (decision of the operator, 27.11.2026).
pub mod schutz {
    use ansicht::woerter as w;
    /// The heading of the group.
    pub const GRUPPE: &str = w::SCHUTZ;
    /// The enclosure.
    pub const EINSCHLUSS: &str = w::SCHUTZ_EINSCHLUSS;
    /// The holder.
    pub const HALTER: &str = w::SCHUTZ_HALTER;
    /// The last start of a session.
    pub const LETZTER_START: &str = w::SCHUTZ_LETZTER_START;
    /// The number of the released network targets.
    pub const NETZZIELE: &str = w::SCHUTZ_NETZZIELE;
    /// The value of the enclosure, followed by the date.
    pub const PROBELAUF: &str = w::SCHUTZ_PROBELAUF;
    /// The value of the holder, followed by the date.
    pub const EINGERICHTET: &str = w::SCHUTZ_EINGERICHTET;
    /// The value of the last start when there was none yet.
    pub const KEIN_START: &str = w::SCHUTZ_KEIN_START;
    /// The value of a part that the service did not report (`null` or missing).
    pub const NICHT_GEMELDET: &str = w::SCHUTZ_NICHT_BEKANNT;
    /// After the time of the last start: the window has no time zone of its own. TEXT fehlt.
    pub const UTC: &str = "UTC";
}

/// The effective deadline of a release for each MCP server, read only (`regelebenen` 15.1.2): one table like in the
/// browser view, one row per server, each deadline with its origin (decision of the operator, 28.11.2026).
pub mod server {
    use ansicht::woerter as w;
    /// The heading of the table.
    pub const TABELLE: &str = w::FRISTEN_JE_SERVER;
    /// The column of the server.
    pub const SERVER: &str = w::SERVER;
    /// The origin of a deadline from the file of the company.
    pub const HERKUNFT_FIRMENDATEI: &str = w::HERKUNFT_FIRMENDATEI;
    /// The origin of a deadline from the catalogue: the default.
    pub const HERKUNFT_KATALOG: &str = w::HERKUNFT_KATALOG;
    /// The unit after the number of seconds: „60 s“.
    pub const SEKUNDEN: &str = w::EINHEIT_SEKUNDEN;
}

/// The words of a choice.
pub mod wahl {
    use ansicht::woerter as w;
    /// The first choice of the thinking level: no level of its own, the program decides.
    pub const VORGABE_DES_PROGRAMMS: &str = w::VORGABE_DES_PROGRAMMS;
    /// The thinking level that is off (`none` of copilot and opencode, `off` of pi).
    pub const AUS: &str = w::AUS;
    /// The unit after a font size: „13 Punkt“.
    pub const PUNKT: &str = w::EINHEIT_PUNKT;
    /// The separator between value and origin in one cell.
    pub const TRENNER: &str = w::TRENNER;
}
