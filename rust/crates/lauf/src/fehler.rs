//! The codes of "Lauf starten" (contract 1, "Ablauf vor dem exec"): exit code, code and fixed reason.

/// Why the program ends before the `exec`. Each variant is one row of the table of contract 1; the codes of the
/// crates `start` and `einstellungen` keep their own text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fehler {
    /// Step 0: `SSL_CERT_FILE`, `SSL_CERT_DIR` or `OPENSSL_CONF` is set.
    TlsUmgebung,
    /// Step 1: not exactly one argument.
    Aufruf,
    /// Step 2: the start file cannot be opened or read.
    DateiUnlesbar,
    /// Step 2: the start file is a link, not regular, not own or not 0600.
    DateiModus,
    /// Step 3: the own configuration file does not exist.
    KonfigFehlt,
    /// Step 3: the own configuration file cannot be read, is not regular, has a foreign owner or a mode others may
    /// write, is too large or has an invalid form.
    KonfigKaputt,
    /// Step 3: the settings file cannot be read.
    EinstUnlesbar,
    /// Step 3: the settings file breaks contract 2 (`einst.*`).
    Einst(einstellungen::Fehler),
    /// Step 4: the start file breaks contract 1 (`start.*`).
    Start(start::Fehler),
    /// Step 4a: a local route without `ziel` or `modell`.
    ZielFehlt,
    /// Step 5: the working folder is missing or no folder.
    OrdnerFehlt,
    /// Step 5a: the counting folder is missing, foreign, not 0700 or a link.
    MarkerOrdner,
    /// Step 5b: the file of folder marks is unreadable or broken (fail closed).
    OrdnerMarkeKaputt,
    /// Step 5b: a mark of the file has no key, a key of the wrong form or the key of another entry (fail closed).
    OrdnerMarkeKennung,
    /// Step 5b: a related mark has another customer.
    OrdnerAndererKunde,
    /// Step 5b: a related mark is higher than the class of the route.
    OrdnerHoeherMarkiert,
    /// Step 5c: a file of an earlier session lies next to the start file (`suche.pid` or `modellweg.pid`, or a link of
    /// that name).
    StartordnerBelegt,
    /// Step 8: the session folder of the session name exists already (the session ran before).
    SitzungBelegt,
    /// Step 6 and 8: the enclosure is not available.
    EinschlussFehlt,
    /// Step 7: the search process cannot be started.
    SucheFehlt,
    /// Step 7: the search process did not report ready in time.
    SucheNichtBereit,
    /// Step 7a0: the target list of the cloud session cannot be written (folder `sitzungen` missing, foreign, not 0700,
    /// the file exists already, or a write fault).
    Zielliste,
    /// Step 7a: the model path cannot be started.
    ModellwegFehlt,
    /// Step 7a: the model path did not report ready in time.
    ModellwegNichtBereit,
    /// Step 6a: the package of the cloud harness has an unknown form or cannot be read (main package, platform package,
    /// a machine without a known architecture or without glibc).
    PaketForm,
    /// Step 6a: the cache folder of the harness or the package folder in it is unusable: a link in the path, a foreign
    /// owner, a mode other than 0700, a login store, or it cannot be made or locked. The word names which (no path).
    PaketCache(&'static str),
    /// Step 6a: the extraction in the small enclosure failed (exit code not 0, the time is up) or the folder is not
    /// complete afterwards. `rc` is the exit code of the extraction, the word the cause (no content of its output).
    PaketEntpacken {
        /// The exit code of the extraction, `None` without one.
        rc: Option<i32>,
        /// `rc`, `kein-platz`, `starter`, `unvollstaendig`, `frist`, `start` or `sperre-frist`.
        wort: &'static str,
    },
    /// Step 8: the `exec` failed.
    HarnessFehlt,
    /// With `--meldung`: the connection to the socket of the service fails, or descriptor 4 is not free at the start.
    MeldungFehlt,
    /// Any step: an unexpected fault.
    Intern,
}

impl Fehler {
    /// The code of contract 1.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Fehler::TlsUmgebung => "lauf.tls_umgebung",
            Fehler::Aufruf => "lauf.aufruf",
            Fehler::DateiUnlesbar => "lauf.datei_unlesbar",
            Fehler::DateiModus => "lauf.datei_modus",
            Fehler::KonfigFehlt => "lauf.konfig_fehlt",
            Fehler::KonfigKaputt => "lauf.konfig_kaputt",
            Fehler::EinstUnlesbar => "lauf.einst_unlesbar",
            Fehler::Einst(e) => e.code(),
            Fehler::Start(e) => e.code(),
            Fehler::ZielFehlt => "lauf.ziel_fehlt",
            Fehler::OrdnerFehlt => "lauf.ordner_fehlt",
            Fehler::MarkerOrdner => "lauf.marker_ordner",
            Fehler::OrdnerMarkeKaputt => "lauf.ordner_marke_kaputt",
            Fehler::OrdnerMarkeKennung => "lauf.ordner_marke_kennung",
            Fehler::OrdnerAndererKunde => "lauf.ordner_anderer_kunde",
            Fehler::OrdnerHoeherMarkiert => "lauf.ordner_hoeher_markiert",
            Fehler::StartordnerBelegt => "lauf.startordner_belegt",
            Fehler::SitzungBelegt => "lauf.sitzung_belegt",
            Fehler::EinschlussFehlt => "lauf.einschluss_fehlt",
            Fehler::SucheFehlt => "lauf.suche_fehlt",
            Fehler::SucheNichtBereit => "lauf.suche_nicht_bereit",
            Fehler::Zielliste => "lauf.zielliste",
            Fehler::ModellwegFehlt => "lauf.modellweg_fehlt",
            Fehler::ModellwegNichtBereit => "lauf.modellweg_nicht_bereit",
            Fehler::PaketForm => "lauf.paket_form",
            Fehler::PaketCache(_) => "lauf.paket_cache",
            Fehler::PaketEntpacken { .. } => "lauf.paket_entpacken",
            Fehler::HarnessFehlt => "lauf.harness_fehlt",
            Fehler::MeldungFehlt => "lauf.meldung_fehlt",
            Fehler::Intern => "lauf.intern",
        }
    }

    /// The fixed reason of contract 1 (ASCII, no path, no content).
    #[must_use]
    pub fn grund(&self) -> &'static str {
        match self {
            Fehler::TlsUmgebung => "TLS-Umgebung gesetzt",
            Fehler::Aufruf => "genau ein Pfad erwartet",
            Fehler::DateiUnlesbar => "Startdatei nicht lesbar",
            Fehler::DateiModus => "Startdatei nicht eigen, nicht 0600 oder keine Datei",
            Fehler::KonfigFehlt => "Konfigurationsdatei fehlt",
            Fehler::KonfigKaputt => "Konfigurationsdatei nicht lesbar oder ungueltig",
            Fehler::EinstUnlesbar => "Einstellungsdatei nicht lesbar",
            Fehler::Einst(e) => e.grund(),
            Fehler::Start(e) => e.grund(),
            Fehler::ZielFehlt => "Route ohne Ziel",
            Fehler::OrdnerFehlt => "Arbeitsordner fehlt oder ist kein Ordner",
            Fehler::MarkerOrdner => "Zaehlordner fehlt, ist fremd, nicht 0700 oder ein Verweis",
            Fehler::OrdnerMarkeKaputt => "Ordnermarke unlesbar oder beschaedigt",
            Fehler::OrdnerMarkeKennung => "Ordnermarke mit ungueltiger oder doppelter Kennung",
            Fehler::OrdnerAndererKunde => "Ordner gehoert einem anderen Kunden",
            Fehler::OrdnerHoeherMarkiert => "Ordner hoeher markiert als die Route",
            Fehler::StartordnerBelegt => "Startordner enthaelt Dateien einer fruehen Sitzung",
            Fehler::SitzungBelegt => "Sitzung wurde schon gestartet",
            Fehler::EinschlussFehlt => "Einschluss nicht verfuegbar",
            Fehler::SucheFehlt => "Suche nicht startbar",
            Fehler::SucheNichtBereit => "Suche nicht bereit",
            Fehler::Zielliste => "Zielliste nicht schreibbar",
            Fehler::ModellwegFehlt => "Modellweg nicht startbar",
            Fehler::ModellwegNichtBereit => "Modellweg nicht bereit",
            Fehler::PaketForm => "Paket des Harness unbekannt oder nicht lesbar",
            Fehler::PaketCache(_) => "Paketordner des Harness unbrauchbar",
            Fehler::PaketEntpacken { .. } => "Paket des Harness nicht entpackt",
            Fehler::HarnessFehlt => "Harness nicht startbar",
            Fehler::MeldungFehlt => "Meldung an den Dienst nicht moeglich",
            Fehler::Intern => "interner Fehler",
        }
    }

    /// The exit code (`sysexits.h`).
    #[must_use]
    pub fn exitcode(&self) -> u8 {
        match self {
            Fehler::Aufruf => 64,
            Fehler::Start(_) | Fehler::OrdnerFehlt => 65,
            Fehler::DateiUnlesbar | Fehler::DateiModus => 66,
            Fehler::EinschlussFehlt
            | Fehler::SucheFehlt
            | Fehler::SucheNichtBereit
            | Fehler::ModellwegFehlt
            | Fehler::ModellwegNichtBereit
            | Fehler::PaketEntpacken { .. }
            | Fehler::HarnessFehlt
            | Fehler::MeldungFehlt => 69,
            Fehler::Intern => 70,
            Fehler::MarkerOrdner
            | Fehler::Zielliste
            | Fehler::StartordnerBelegt
            | Fehler::PaketCache(_)
            | Fehler::SitzungBelegt => 73,
            Fehler::OrdnerAndererKunde | Fehler::OrdnerHoeherMarkiert => 77,
            Fehler::TlsUmgebung
            | Fehler::KonfigFehlt
            | Fehler::KonfigKaputt
            | Fehler::EinstUnlesbar
            | Fehler::Einst(_)
            | Fehler::ZielFehlt
            | Fehler::OrdnerMarkeKaputt
            | Fehler::OrdnerMarkeKennung
            | Fehler::PaketForm => 78,
        }
    }

    /// The second line of step 6a, without the line end: `lauf.paket_teil: <exit code of the extraction or -> <word>`.
    /// Fixed words only, never a path and never a byte of the output of the extraction.
    #[must_use]
    pub fn teil_zeile(&self) -> Option<String> {
        match self {
            Fehler::PaketCache(wort) => Some(format!("lauf.paket_teil: - {wort}")),
            Fehler::PaketEntpacken { rc, wort } => Some(format!(
                "lauf.paket_teil: {} {wort}",
                rc.map_or_else(|| "-".to_owned(), |n| n.to_string())
            )),
            _ => None,
        }
    }

    /// The line on standard error: `<code>: <grund>` and LF; for `lauf.paket_cache` and `lauf.paket_entpacken` the
    /// second line of [`Fehler::teil_zeile`] follows.
    #[must_use]
    pub fn zeile(&self) -> String {
        let mut z = format!("{}: {}\n", self.code(), self.grund());
        if let Some(t) = self.teil_zeile() {
            z.push_str(&t);
            z.push('\n');
        }
        z
    }
}

#[cfg(test)]
mod tests {
    use super::Fehler;

    #[test]
    fn every_line_is_short_ascii_with_the_code_and_the_reason() {
        let alle = [
            Fehler::TlsUmgebung,
            Fehler::Aufruf,
            Fehler::DateiUnlesbar,
            Fehler::DateiModus,
            Fehler::KonfigFehlt,
            Fehler::KonfigKaputt,
            Fehler::EinstUnlesbar,
            Fehler::Einst(einstellungen::Fehler::ZuGross),
            Fehler::Start(start::Fehler::ZuGross),
            Fehler::ZielFehlt,
            Fehler::OrdnerFehlt,
            Fehler::MarkerOrdner,
            Fehler::OrdnerMarkeKaputt,
            Fehler::OrdnerMarkeKennung,
            Fehler::OrdnerAndererKunde,
            Fehler::OrdnerHoeherMarkiert,
            Fehler::StartordnerBelegt,
            Fehler::SitzungBelegt,
            Fehler::EinschlussFehlt,
            Fehler::SucheFehlt,
            Fehler::SucheNichtBereit,
            Fehler::Zielliste,
            Fehler::ModellwegFehlt,
            Fehler::ModellwegNichtBereit,
            Fehler::HarnessFehlt,
            Fehler::MeldungFehlt,
            Fehler::Intern,
        ];
        let soll: [(u8, &str); 27] = [
            (78, "lauf.tls_umgebung: TLS-Umgebung gesetzt\n"),
            (64, "lauf.aufruf: genau ein Pfad erwartet\n"),
            (66, "lauf.datei_unlesbar: Startdatei nicht lesbar\n"),
            (
                66,
                "lauf.datei_modus: Startdatei nicht eigen, nicht 0600 oder keine Datei\n",
            ),
            (78, "lauf.konfig_fehlt: Konfigurationsdatei fehlt\n"),
            (
                78,
                "lauf.konfig_kaputt: Konfigurationsdatei nicht lesbar oder ungueltig\n",
            ),
            (78, "lauf.einst_unlesbar: Einstellungsdatei nicht lesbar\n"),
            (78, "einst.zu_gross: Einstellungsdatei zu gross\n"),
            (65, "start.zu_gross: Start zu gross\n"),
            (78, "lauf.ziel_fehlt: Route ohne Ziel\n"),
            (
                65,
                "lauf.ordner_fehlt: Arbeitsordner fehlt oder ist kein Ordner\n",
            ),
            (
                73,
                "lauf.marker_ordner: Zaehlordner fehlt, ist fremd, nicht 0700 oder ein Verweis\n",
            ),
            (
                78,
                "lauf.ordner_marke_kaputt: Ordnermarke unlesbar oder beschaedigt\n",
            ),
            (
                78,
                "lauf.ordner_marke_kennung: Ordnermarke mit ungueltiger oder doppelter Kennung\n",
            ),
            (
                77,
                "lauf.ordner_anderer_kunde: Ordner gehoert einem anderen Kunden\n",
            ),
            (
                77,
                "lauf.ordner_hoeher_markiert: Ordner hoeher markiert als die Route\n",
            ),
            (
                73,
                "lauf.startordner_belegt: Startordner enthaelt Dateien einer fruehen Sitzung\n",
            ),
            (73, "lauf.sitzung_belegt: Sitzung wurde schon gestartet\n"),
            (69, "lauf.einschluss_fehlt: Einschluss nicht verfuegbar\n"),
            (69, "lauf.suche_fehlt: Suche nicht startbar\n"),
            (69, "lauf.suche_nicht_bereit: Suche nicht bereit\n"),
            (73, "lauf.zielliste: Zielliste nicht schreibbar\n"),
            (69, "lauf.modellweg_fehlt: Modellweg nicht startbar\n"),
            (69, "lauf.modellweg_nicht_bereit: Modellweg nicht bereit\n"),
            (69, "lauf.harness_fehlt: Harness nicht startbar\n"),
            (
                69,
                "lauf.meldung_fehlt: Meldung an den Dienst nicht moeglich\n",
            ),
            (70, "lauf.intern: interner Fehler\n"),
        ];
        for (f, (exit, zeile)) in alle.iter().zip(soll) {
            assert_eq!((f.exitcode(), f.zeile().as_str()), (exit, zeile));
            assert!(f.zeile().is_ascii() && f.zeile().len() <= 200);
            assert_eq!(f.teil_zeile(), None);
        }
    }

    #[test]
    fn the_codes_of_step_6a_carry_a_fixed_second_line() {
        let soll: [(Fehler, u8, &str); 5] = [
            (
                Fehler::PaketForm,
                78,
                "lauf.paket_form: Paket des Harness unbekannt oder nicht lesbar\n",
            ),
            (
                Fehler::PaketCache("verweis"),
                73,
                "lauf.paket_cache: Paketordner des Harness unbrauchbar\nlauf.paket_teil: - verweis\n",
            ),
            (
                Fehler::PaketCache("fremd"),
                73,
                "lauf.paket_cache: Paketordner des Harness unbrauchbar\nlauf.paket_teil: - fremd\n",
            ),
            (
                Fehler::PaketEntpacken {
                    rc: Some(1),
                    wort: "kein-platz",
                },
                69,
                "lauf.paket_entpacken: Paket des Harness nicht entpackt\nlauf.paket_teil: 1 kein-platz\n",
            ),
            (
                Fehler::PaketEntpacken {
                    rc: None,
                    wort: "frist",
                },
                69,
                "lauf.paket_entpacken: Paket des Harness nicht entpackt\nlauf.paket_teil: - frist\n",
            ),
        ];
        for (f, exit, zeile) in soll {
            assert_eq!((f.exitcode(), f.zeile().as_str()), (exit, zeile));
            assert!(f.zeile().is_ascii());
        }
    }
}
