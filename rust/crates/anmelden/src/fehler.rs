//! The error codes of contract 9, their exit codes and their fixed sentences.

/// Every way the command can end without a stored key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fehler {
    /// The arguments are not `copilot [--uebernehmen] [--neu]`.
    Aufruf,
    /// No executable `copilot` in the PATH.
    CliFehlt,
    /// The CLI ended with an error or was killed.
    LoginAbgebrochen,
    /// The throw-away home holds no (or no single) token.
    KeinToken,
    /// The token does not have the expected form.
    FormFalsch,
    /// The target folder is missing, not ours or not writable.
    ZielNichtSchreibbar,
    /// The key file exists and `--neu` is missing.
    DateiDa,
    /// A login of the CLI lies in the real home after the run. The key file is stored.
    EchteAnmeldung,
    /// The throw-away home cannot be created.
    HeimNichtAnlegbar,
    /// SIGINT, SIGTERM or SIGHUP arrived; the number is the signal.
    Signal(i32),
    /// A left-over throw-away home of an earlier run cannot be deleted. No login starts.
    RestNichtLoeschbar,
    /// `--uebernehmen`: `config.json` of the CLI is missing, is not JSON or has not exactly one token.
    QuelleOhneToken,
    /// `--uebernehmen`: `config.json` or its folder is a link, not regular, not own, shared or larger than 1 MiB.
    QuelleUnsicher,
    /// `--uebernehmen`: `config.json` cannot be written anew or changed meanwhile.
    QuelleNichtSchreibbar,
    /// The process or the child `copilot login` cannot be protected against a core image (crate `kernabbild`: `RLIMIT_CORE` 1/1, under Linux
    /// `PR_SET_DUMPABLE` 0).
    Abschirmung,
}

impl Fehler {
    /// The code of the error as it appears in the line on stderr.
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Fehler::Aufruf => "anmelden.aufruf",
            Fehler::CliFehlt => "anmelden.cli_fehlt",
            Fehler::LoginAbgebrochen => "anmelden.login_abgebrochen",
            Fehler::KeinToken => "anmelden.kein_token",
            Fehler::FormFalsch => "anmelden.form_falsch",
            Fehler::ZielNichtSchreibbar => "anmelden.ziel_nicht_schreibbar",
            Fehler::DateiDa => "anmelden.datei_da",
            Fehler::EchteAnmeldung => "anmelden.echte_anmeldung",
            Fehler::HeimNichtAnlegbar => "anmelden.heim_nicht_anlegbar",
            Fehler::Signal(_) => "anmelden.signal",
            Fehler::RestNichtLoeschbar => "anmelden.rest_nicht_loeschbar",
            Fehler::QuelleOhneToken => "anmelden.quelle_ohne_token",
            Fehler::QuelleUnsicher => "anmelden.quelle_unsicher",
            Fehler::QuelleNichtSchreibbar => "anmelden.quelle_nicht_schreibbar",
            Fehler::Abschirmung => "anmelden.abschirmung",
        }
    }

    /// The exit code: 64 for a wrong call, 5 for a missing protection against a core image (default of the Werkbank), 10 to 21 for the
    /// others (18: a left-over that cannot be deleted; 19 to 21: the source of `--uebernehmen`), 128 + signal number for a signal.
    #[must_use]
    pub fn exit(self) -> u8 {
        match self {
            Fehler::Aufruf => 64,
            Fehler::CliFehlt => 10,
            Fehler::LoginAbgebrochen => 11,
            Fehler::KeinToken => 12,
            Fehler::FormFalsch => 13,
            Fehler::ZielNichtSchreibbar => 14,
            Fehler::DateiDa => 15,
            Fehler::EchteAnmeldung => 16,
            Fehler::HeimNichtAnlegbar => 17,
            Fehler::Signal(n) => u8::try_from(128 + n).unwrap_or(255),
            Fehler::RestNichtLoeschbar => 18,
            Fehler::QuelleOhneToken => 19,
            Fehler::QuelleUnsicher => 20,
            Fehler::QuelleNichtSchreibbar => 21,
            Fehler::Abschirmung => crate::schutz::EXIT_ABSCHIRMUNG,
        }
    }

    /// The fixed sentence. It never holds a path, a name or a part of a token.
    #[must_use]
    pub fn satz(self) -> &'static str {
        match self {
            Fehler::Aufruf => "Aufruf: werkbank-anmelden copilot [--uebernehmen] [--neu]",
            Fehler::CliFehlt => "Die Copilot CLI wurde nicht gefunden.",
            Fehler::LoginAbgebrochen => "Die Anmeldung wurde nicht abgeschlossen.",
            Fehler::KeinToken => "Die Anmeldung hat kein Token geliefert.",
            Fehler::FormFalsch => "Das Token hat nicht die erwartete Form.",
            Fehler::ZielNichtSchreibbar => "Der Zielordner ist nicht schreibbar.",
            Fehler::DateiDa => "Die Schluesseldatei ist schon da. Mit --neu wird sie ersetzt.",
            Fehler::EchteAnmeldung => {
                "Im Heim des Nutzers liegt eine Anmeldung der Copilot CLI. Sie muss weg, bevor eine Sitzung startet."
            }
            Fehler::HeimNichtAnlegbar => "Das Wegwerf-Heim ist nicht anlegbar.",
            Fehler::Signal(_) => "Abgebrochen durch ein Signal.",
            Fehler::RestNichtLoeschbar => "Ein Rest eines frueheren Laufs ist nicht loeschbar.",
            Fehler::QuelleOhneToken => {
                "In der Konfiguration der Copilot CLI liegt nicht genau ein Token."
            }
            Fehler::QuelleUnsicher => {
                "Die Konfiguration der Copilot CLI ist keine eigene regulaere Datei bis 1 MiB."
            }
            Fehler::QuelleNichtSchreibbar => {
                "Die Konfiguration der Copilot CLI ist nicht neu schreibbar."
            }
            Fehler::Abschirmung => "Der Schutz vor einem Kernabbild ist nicht setzbar.",
        }
    }

    /// The whole line for stderr, with the line feed.
    #[must_use]
    pub fn zeile(self) -> String {
        format!("werkbank-anmelden: {}: {}\n", self.code(), self.satz())
    }
}
