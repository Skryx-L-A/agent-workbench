//! The words the browser view and the app window share (`rust/spec/VERTRAEGE.md` OF1: one wording in both views). Where today's app
//! has a word, it holds (its tables `the previous implementation` and `the previous implementation`, key named at the
//! constant); else the word of the browser view.
//!
//! Only constants: no `use`, no function, no dependency. The app window reads them through its existing dependency on this
//! crate (`ansicht::woerter`). The modules [`seite`], [`gruppe`], [`aussehen`] and [`werte`] hold the words of the settings
//! in the shape of the list of the app window.

// ---- programs and models ----

/// The program a session runs with: label of the field in the window "Sitzungen" (today `gruppe.harness`, `wahl.harness`).
pub const PROGRAMM: &str = "Programm";
/// The settings page of programs and models (today `seite.harnesses.titel`).
pub const PROGRAMME_UND_MODELLE: &str = "Programme und Modelle";
/// The first choice of the field "Denkstufe": no level of its own, the program decides.
pub const VORGABE_DES_PROGRAMMS: &str = "Vorgabe des Programms";
/// The model of a session (today `gruppe.modell`).
pub const MODELL: &str = "Modell";
/// The thinking level of a new session in the window "Sitzungen" (today `gruppe.effort`).
pub const DENKSTUFE: &str = "Denkstufe";
/// The effort in the settings of the admins (limits); today's app has no word for them.
pub const EFFORT: &str = "Effort";

// ---- window "Sitzungen" ----

/// The hint above the list.
pub const SITZUNGEN_HINWEIS: &str = "Jede bekannte Sitzung, auch die beendeten.";
/// The list without a session (today `satz.keineSitzungBekannt`).
pub const KEINE_SITZUNG_BEKANNT: &str = "Es ist keine Sitzung bekannt.";
/// The foot without a chosen session (today `satz.waehleSitzung`).
pub const SITZUNG_WAEHLEN: &str =
    "Eine Sitzung wählen – was mit ihrer Unterhaltung geschieht, steht dann hier.";
/// The placeholder of the field "Name" (today `platzhalter.name`).
pub const NAME_OPTIONAL: &str = "Name (optional)";
/// The placeholder of the field "Arbeitsordner".
pub const PFAD_PLATZHALTER: &str = "/pfad/zum/projekt";
/// The heading of the sessions without a working folder (today `satz.ohneOrdner`).
pub const OHNE_ORDNER: &str = "(ohne Ordner)";
/// The reason of a locked button: nothing chosen.
pub const GRUND_KEINE_GEWAEHLT: &str = "Keine Sitzung gewählt";
/// The reason of a locked button: the session has ended.
pub const GRUND_IST_BEENDET: &str = "Sitzung ist beendet";
/// The reason of a locked button: the session runs.
pub const GRUND_LAEUFT: &str = "Sitzung läuft";
/// The reason of a locked button: the session was continued already.
pub const GRUND_SCHON_FORTGESETZT: &str = "Sitzung ist schon fortgesetzt";
/// The reason of a locked „Fortsetzen“: the program of the session cannot continue a conversation (opencode, `lauf` 69
/// `lauf.fortsetzung_unmoeglich`).
pub const GRUND_NICHT_FORTSETZBAR: &str = "Mit diesem Programm nicht fortsetzbar";
/// The reason in place of the field "Denkstufe" when the route allows none.
pub const GRUND_KEINE_DENKSTUFE: &str = "Bei dieser Route nicht wählbar";

// ---- sessions ----

/// Ends the chosen session, in the window "Sitzungen" and in the context menu (today `knopf.beenden`).
pub const BEENDEN: &str = "Beenden";
/// Continues an ended session (today `knopf.fortsetzen`).
pub const FORTSETZEN: &str = "Fortsetzen";
/// State of a running session (today `zustand.laeuft`).
pub const ZUSTAND_LAEUFT: &str = "läuft";
/// State of a session waiting for its user (today `zustand.wartet`).
pub const ZUSTAND_WARTET: &str = "wartet";
/// State of a session that ended (today `zustand.beendet`).
pub const ZUSTAND_BEENDET: &str = "beendet";
/// State of a session that crashed (today in the message of a crash, „… abgestürzt“).
pub const ZUSTAND_ABGESTUERZT: &str = "abgestürzt";
/// State of a session being started (today `zustand.startet`).
pub const ZUSTAND_STARTET: &str = "startet…";
/// State of a session whose start failed (today `zustand.startFehler`).
pub const ZUSTAND_START_GESCHEITERT: &str = "Start gescheitert";
/// The exit code after the state of an ended session: „beendet · Code 129“.
pub const CODE: &str = "Code";
/// The separator between the parts of one line.
pub const TRENNER: &str = " · ";

// ---- settings: frame ----

/// The heading of the user area.
pub const BEREICH_NUTZER: &str = "Für dich";
/// The area of the admins in the settings (OF9 point 1).
pub const VERWALTUNG: &str = "Verwaltung";
/// The badge of the admin area and of its pages.
pub const ADMIN: &str = "Admin";
/// The name of a close button.
pub const SCHLIESSEN: &str = "Schließen";
/// A page without values.
pub const KEINE_WERTE: &str = "Der Dienst hat für diese Seite keine Werte geschickt.";
/// A switch that is on (today `wort.an`).
pub const AN: &str = "an";
/// A switch that is off (today `wort.aus`).
pub const AUS: &str = "aus";

// ---- settings: origin of a value and reason of a lock (`regelebenen` 15.5) ----

/// A value set by the company.
pub const HERKUNFT_FIRMA: &str = "Firma";
/// A value set by a group; the name of the group follows: „Gruppe Entwicklung“.
pub const HERKUNFT_GRUPPE: &str = "Gruppe";
/// A value set by the user, in the browser also the look of this device.
pub const HERKUNFT_DU: &str = "du";
/// The reason `nur_admin`.
pub const GRUND_NUR_ADMIN: &str = "Vorgabe der Firma";
/// The reason `nur_strenger`.
pub const GRUND_NUR_STRENGER: &str = "Nur strenger als die Vorgabe";
/// The reason `fest`.
pub const GRUND_FEST: &str = "Im Pilot fest aus";
/// A lock without a reason, before the origin: „Vorgabe: Firma“.
pub const GRUND_VORGABE: &str = "Vorgabe:";

// ---- settings: pages (flat names kept for the first users of this table) ----

/// The page of the look (today `seite.aussehen.titel`).
pub const AUSSEHEN: &str = "Aussehen";
/// The page of the defaults for new sessions (today `seite.sitzung.titel`).
pub const SITZUNG: &str = "Sitzung";
/// The page of the permissions (today `seite.erlaubnisse.titel`).
pub const ERLAUBNISSE: &str = "Erlaubnisse";
/// The page of oversight (today `seite.aufsicht.titel`).
pub const AUFSICHT: &str = "Aufsicht und Meldungen";

// ---- settings: look ----

/// The choice of the theme (today `feld.thema.name`).
pub const THEMA: &str = "Hell oder dunkel";
/// Theme: value and text; like the system, light, dark (today `wort.thema.*`).
pub const THEMEN: [(&str, &str); 3] = [
    ("system", "Wie das System"),
    ("hell", "Hell"),
    ("dunkel", "Dunkel"),
];
/// The colour set of the dark theme (not in today's app).
pub const FARBSATZ: &str = "Farben im dunklen Thema";
/// Colour set: value and text.
pub const FARBSAETZE: [(&str, &str); 2] = [("grau", "Grau"), ("blauschwarz", "Blauschwarz")];
/// The colours of the terminal.
pub const TERMINAL: &str = "Terminal";
/// Terminal: value and text.
pub const TERMINALS: [(&str, &str); 3] = [
    ("dunkel", "Dunkel"),
    ("hell", "Hell"),
    ("thema", "Wie das Thema"),
];
/// The font size of the terminal (today `feld.terminalFontSize.name`).
pub const SCHRIFTGROESSE: &str = "Schriftgröße im Terminal";
/// The unit of the font size, after the number: „13 Punkt“ (today `wort.einheit.punkt`); 1 point is 1 CSS pixel.
pub const EINHEIT_PUNKT: &str = "Punkt";

// ---- settings: the protection layers in effect (read only, page "Aufsicht und Meldungen") ----
//
// Only what was measured, each with its date; never „an“ or „läuft“, no light, no tick (decision of 27.11.2026).

/// The heading of the section.
pub const SCHUTZ: &str = "Wirksamer Stand der Schutzschichten";
/// Its line under the heading.
pub const SCHUTZ_KURZ: &str = "Nur zum Lesen: was gemessen ist, je mit Datum.";
/// The confinement of the workers.
pub const SCHUTZ_EINSCHLUSS: &str = "Einschluss";
/// The trial run of the confinement at the installation, before its date: „Probelauf bei der Installation grün, 27.11.2026“.
pub const SCHUTZ_PROBELAUF: &str = "Probelauf bei der Installation grün,";
/// The holder of the approvals.
pub const SCHUTZ_HALTER: &str = "Halter";
/// The holder set up at the installation, before its date.
pub const SCHUTZ_EINGERICHTET: &str = "eingerichtet bei der Installation,";
/// A layer without a measurement.
pub const SCHUTZ_NICHT_BEKANNT: &str = "nicht bekannt";
/// The last start of a session.
pub const SCHUTZ_LETZTER_START: &str = "Letzter Start";
/// The last start went through.
pub const SCHUTZ_GESTARTET: &str = "gestartet";
/// No start since the service started.
pub const SCHUTZ_KEIN_START: &str = "noch kein Start seit dem Start des Dienstes";
/// The network destinations that are released, as a number per origin, never a host.
pub const SCHUTZ_NETZZIELE: &str = "Freigegebene Netzziele";

/// The heading of the deadlines per MCP server (message `Fristen`), read only.
pub const FRISTEN_JE_SERVER: &str = "Fristen je MCP-Server";
/// The column of the server.
pub const SERVER: &str = "Server";
/// A deadline from the file of the company.
pub const HERKUNFT_FIRMENDATEI: &str = "Firma";
/// A deadline from the catalog: the default.
pub const HERKUNFT_KATALOG: &str = "Vorgabe";
/// The unit of a deadline in the table, after the number: „60 s“.
pub const EINHEIT_SEKUNDEN: &str = "s";

/// The reasons of `lauf` in plain text (contract `start`, table of the reasons), with today's word „Programm“ for the harness; a
/// reason outside the table shows as it comes.
pub const LAUF_GRUENDE: [(&str, &str); 32] = [
    ("lauf.tls_umgebung", "TLS-Umgebung gesetzt"),
    ("lauf.aufruf", "genau ein Pfad erwartet"),
    ("lauf.datei_unlesbar", "Startdatei nicht lesbar"),
    (
        "lauf.datei_modus",
        "Startdatei nicht eigen, nicht 0600 oder keine Datei",
    ),
    ("lauf.konfig_fehlt", "Konfigurationsdatei fehlt"),
    (
        "lauf.konfig_kaputt",
        "Konfigurationsdatei nicht lesbar oder ungültig",
    ),
    ("lauf.einst_unlesbar", "Einstellungsdatei nicht lesbar"),
    ("lauf.ziel_fehlt", "Route ohne Ziel"),
    (
        "lauf.ordner_fehlt",
        "Arbeitsordner fehlt oder ist kein Ordner",
    ),
    (
        "lauf.marker_ordner",
        "Zählordner fehlt, ist fremd, nicht 0700 oder ein Verweis",
    ),
    (
        "lauf.ordner_marke_kaputt",
        "Ordnermarke unlesbar oder beschädigt",
    ),
    (
        "lauf.ordner_marke_kennung",
        "Ordnermarke mit ungültiger oder doppelter Kennung",
    ),
    (
        "lauf.ordner_anderer_kunde",
        "Ordner gehört einem anderen Kunden",
    ),
    (
        "lauf.ordner_hoeher_markiert",
        "Ordner höher markiert als die Route",
    ),
    (
        "lauf.startordner_belegt",
        "Startordner enthält Dateien einer frühen Sitzung",
    ),
    ("lauf.sitzung_belegt", "Sitzung wurde schon gestartet"),
    ("lauf.fortsetzung_unmoeglich", "Gespräch nicht fortsetzbar"),
    (
        "lauf.fortsetzung_fehlt",
        "Gespräch der Vorgängersitzung fehlt",
    ),
    (
        "lauf.fortsetzung_kaputt",
        "Gespräch der Vorgängersitzung unbrauchbar",
    ),
    (
        "lauf.fortsetzung_fremd",
        "Gespräch gehört zu anderem Programm oder Ordner",
    ),
    ("lauf.einschluss_fehlt", "Einschluss nicht verfügbar"),
    ("lauf.suche_fehlt", "Suche nicht startbar"),
    ("lauf.suche_nicht_bereit", "Suche nicht bereit"),
    ("lauf.zielliste", "Zielliste nicht schreibbar"),
    ("lauf.modellweg_fehlt", "Modellweg nicht startbar"),
    ("lauf.modellweg_nicht_bereit", "Modellweg nicht bereit"),
    (
        "lauf.paket_form",
        "Paket des Programms unbekannt oder nicht lesbar",
    ),
    ("lauf.paket_cache", "Paketordner des Programms unbrauchbar"),
    ("lauf.paket_entpacken", "Paket des Programms nicht entpackt"),
    ("lauf.harness_fehlt", "Programm nicht startbar"),
    ("lauf.meldung_fehlt", "Meldung an den Dienst nicht möglich"),
    ("lauf.intern", "interner Fehler"),
];

/// One row: key, name and a description of at most one line.
pub type Zeile = (&'static str, &'static str, &'static str);

/// The pages: title and short description (one line, OF9 point 6).
pub mod seite {
    /// Today `seite.aussehen.titel`.
    pub const AUSSEHEN: &str = super::AUSSEHEN;
    /// The browser view; today's `seite.aussehen.wofuer` names the language, which this page has not.
    pub const AUSSEHEN_KURZ: &str = "Farben und Schrift auf diesem Gerät.";
    /// Today `seite.sitzung.titel`.
    pub const SITZUNG: &str = super::SITZUNG;
    /// Today `seite.sitzung.wofuer`.
    pub const SITZUNG_KURZ: &str = "Womit eine neue Sitzung anfängt";
    /// Today `seite.harnesses.titel`.
    pub const PROGRAMME: &str = super::PROGRAMME_UND_MODELLE;
    /// User page; today's `seite.harnesses.wofuer` names the sign-in, which this page has not.
    pub const PROGRAMME_KURZ: &str = "Deine Auswahl aus der Freigabe.";
    /// Admin page.
    pub const PROGRAMME_ADMIN_KURZ: &str = "Freigabe und Effort je Stufe.";
    /// Today `seite.erlaubnisse.titel`.
    pub const ERLAUBNISSE: &str = super::ERLAUBNISSE;
    /// Today `seite.erlaubnisse.wofuer`.
    pub const ERLAUBNISSE_KURZ: &str = "Was die Agenten dürfen";
    /// Today `seite.aufsicht.titel`.
    pub const AUFSICHT: &str = super::AUFSICHT;
    /// Today's `seite.aufsicht.wofuer` names standstill and notes, which this page has not.
    pub const AUFSICHT_KURZ: &str = "Wer mitliest und eingreift.";
    /// The page of routes and data classes.
    pub const ROUTEN: &str = "Routen und Datenklassen";
    /// Its short description.
    pub const ROUTEN_KURZ: &str = "Wohin welche Daten gehen.";
    /// The user page of the rules in effect.
    pub const REGELN: &str = "Regeln";
    /// Its short description.
    pub const REGELN_KURZ: &str = "Was gerade für dich gilt.";
}

/// The groups of the pages.
pub mod gruppe {
    /// Today `gruppe.aussehen.thema`.
    pub const FARBEN: &str = "Hell und dunkel";
    /// The browser view; today's `gruppe.aussehen.terminal` names the roles, which this group has not.
    pub const SCHRIFT: &str = "Schrift";
    /// Today `gruppe.sitzung.start`.
    pub const NEUE_SITZUNGEN: &str = "Womit eine neue Sitzung anfängt";
    /// Release of programs and models.
    pub const FREIGABE: &str = "Freigabe";
    /// Limits of the effort.
    pub const EFFORT: &str = super::EFFORT;
    /// The options of Copilot.
    pub const COPILOT: &str = "Copilot";
    /// The network.
    pub const NETZ: &str = "Netz";
    /// The access of the maker.
    pub const HERSTELLER: &str = "Hersteller";
    /// The data classes.
    pub const DATENKLASSEN: &str = "Datenklassen";
    /// Today `gruppe.erlaubnisse.guards`.
    pub const SICHERUNGEN: &str = "Sicherungen vor jedem Befehl";
    /// Neither view had a word: the two deadlines of a release (contract 2, block `mcp`).
    pub const FRISTEN: &str = "Fristen einer Freigabe";
}

/// The rows of the look (choices of this device in the browser).
pub mod aussehen {
    /// Name today `feld.thema.name`.
    pub const THEMA: super::Zeile = (
        "aussehen.thema",
        super::THEMA,
        "Hell, dunkel oder wie das System.",
    );
    /// Today's app has no colour set.
    pub const FARBSATZ: super::Zeile = (
        "aussehen.farbsatz",
        super::FARBSATZ,
        "Grau oder Blauschwarz.",
    );
    /// Today's app has no choice of the terminal.
    pub const TERMINAL: super::Zeile = (
        "aussehen.terminal",
        super::TERMINAL,
        "Dunkel, hell oder wie das Thema.",
    );
    /// Name today `feld.terminalFontSize.name`.
    pub const SCHRIFTGROESSE: super::Zeile = (
        "terminal.schriftgroesse",
        super::SCHRIFTGROESSE,
        "Größe in Punkten, gilt sofort in jedem Terminal.",
    );
}

/// The rows of the values of the service (`regelebenen` 15.1), in the order of the pages.
pub mod werte {
    /// Name today `feld.orchestratorHarness.name`.
    pub const HARNESS: super::Zeile = (
        "sitzung.harness",
        "Programm im Hauptfenster",
        "Womit eine neue Sitzung startet.",
    );
    /// Name today `feld.orchestratorModel.name`.
    pub const MODELL: super::Zeile = (
        "sitzung.modell",
        "Modell der Sitzung",
        "Vorgabe für das Modell.",
    );
    /// Name today `feld.orchestratorEffort.name`.
    pub const EFFORT: super::Zeile = (
        "sitzung.effort",
        "Wie tief die Sitzung denkt",
        "Wie gründlich das Modell denkt.",
    );
    /// With today's word "Programme" for the harnesses.
    pub const HARNESSES: super::Zeile = (
        "harnesses",
        "Programme je Datenklasse",
        "Was Nutzer je Klasse wählen dürfen.",
    );
    /// The models per data class.
    pub const MODELLE: super::Zeile = (
        "modelle",
        "Modelle je Datenklasse",
        "Was Nutzer je Klasse wählen dürfen.",
    );
    /// With today's word "Programm" for the harness.
    pub const EFFORT_GRENZEN: super::Zeile = (
        "effort.grenzen",
        "Grenzen des Efforts",
        "Höchster Effort je Programm und Klasse.",
    );
    /// An option of Copilot.
    pub const OHNE_RUECKFRAGE: super::Zeile = (
        "optionen.copilot_ohne_rueckfrage",
        "Ohne Rückfrage",
        "Werkzeuge der Firmenliste ohne Frage.",
    );
    /// An option of Copilot.
    pub const TELEMETRIE: super::Zeile = (
        "optionen.copilot_telemetrie",
        "Telemetrie",
        "Das Telemetrie-Ziel der Copilot CLI.",
    );
    /// An option of Copilot.
    pub const TOKEN: super::Zeile = (
        "optionen.copilot_eingeschraenktes_token",
        "Eingeschränktes Token",
        "Statt der Geräte-Anmeldung.",
    );
    /// The local CA.
    pub const LOKALE_CA: super::Zeile = (
        "optionen.lokale_ca",
        "Lokale CA zulassen",
        "Cloud-Sitzungen über die lokale CA.",
    );
    /// The access of the maker.
    pub const HERSTELLERZUGANG: super::Zeile = (
        "optionen.herstellerzugang",
        "Herstellerzugang",
        "Zugang des Herstellers.",
    );
    /// Lowering a class.
    pub const HERABSTUFEN: super::Zeile = (
        "optionen.herabstufen",
        "Herabstufen",
        "Die Automatik darf eine Klasse senken.",
    );
    /// Who reads which class.
    pub const LESEKLASSEN: super::Zeile = (
        "leseklassen",
        "Leseklassen",
        "Wer welche Klasse lesen darf.",
    );
    /// Neither view had a word; contract 2, block `mcp` (`frist_ms`).
    pub const FREIGABEFRIST: super::Zeile = (
        "aufsicht.freigabefrist_ms",
        "Frist der Entscheidung",
        "Längste Frist für die Entscheidung eines Menschen.",
    );
    /// Neither view had a word; contract 2, block `mcp` (`einloesefrist_ms`).
    pub const EINLOESEFRIST: super::Zeile = (
        "aufsicht.einloesefrist_ms",
        "Frist zum Einlösen",
        "Frist zum Einlösen nach der Erteilung.",
    );
    /// The fixed choices of the deadline of a decision in milliseconds; the effective value joins them when it is none.
    pub const FREIGABEFRIST_WAHLEN: [u64; 4] = [30_000, 60_000, 300_000, 900_000];
    /// The fixed choices of the deadline to redeem in milliseconds.
    pub const EINLOESEFRIST_WAHLEN: [u64; 4] = [60_000, 300_000, 3_600_000, 86_400_000];
}
