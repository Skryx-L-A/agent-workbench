//! A64: the end of a child under `ConPTY`. The logic is the same on every system and is tested under Linux; only Windows needs it.
//!
//! Under `ConPTY` the output of a child does not end when the child ends: the pipe closes only with `ClosePseudoConsole` (measured in the Windows
//! VM, 20.11.2026: the child ended, but no case got `Kindende`; the reading of the output waits for its end, see `Kind::lies`). So, where the
//! output stays open, the session watches the process itself. When the process has ended and the output has not, it closes the pseudo console
//! once (in `portable-pty` 0.9.0 the pseudo console closes when the master of the PTY is dropped: `win/psuedocon.rs:73-77`, `Drop for
//! PsuedoCon`, and `win/conpty.rs:26-33`, the master holds it), keeps reading the rest of the output until the pipe ends, and only then reports
//! `Kindende` (TR2: the output comes first, then `Kindende`). The process is asked without waiting: `Child::try_wait`
//! (`win/mod.rs:88-90`, `GetExitCodeProcess`, no `unsafe` in this crate).

/// What the session does next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Aktion {
    /// Wait for output or for the end of the process.
    Warten,
    /// The process has ended and the output has not: the pseudo console was closed just now; the rest of the output follows, then the end.
    Schliessen,
    /// The output has ended: wait for the process and report `Kindende`.
    Melden,
}

/// The side of the session that the logic asks and tells.
pub(crate) trait Prozessende {
    /// `true` once the process has ended (asked without waiting).
    fn beendet(&mut self) -> bool;
    /// Closes the pseudo console.
    fn schliessen(&mut self);
}

/// The state of the end logic of one session.
#[derive(Debug, Default)]
pub(crate) struct Abschluss {
    geschlossen: bool,
}

impl Abschluss {
    /// One step. `ausgabe_zu_ende`: the reading thread has reported the end of the output.
    ///
    /// - The output has ended: [`Aktion::Melden`]; the process is not asked and nothing is closed (the output had all of its bytes).
    /// - The output goes on, the pseudo console is not closed yet and the process has ended: close it once, [`Aktion::Schliessen`].
    /// - Otherwise [`Aktion::Warten`]. After closing, the process is not asked again.
    pub(crate) fn schritt(
        &mut self,
        ausgabe_zu_ende: bool,
        quelle: &mut impl Prozessende,
    ) -> Aktion {
        if ausgabe_zu_ende {
            return Aktion::Melden;
        }
        if !self.geschlossen && quelle.beendet() {
            self.geschlossen = true;
            quelle.schliessen();
            return Aktion::Schliessen;
        }
        Aktion::Warten
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A stand-in for the session: it says whether the process has ended and counts what it was asked and told.
    #[derive(Default)]
    struct Attrappe {
        beendet: bool,
        gefragt: usize,
        geschlossen: usize,
    }

    impl Prozessende for Attrappe {
        fn beendet(&mut self) -> bool {
            self.gefragt += 1;
            self.beendet
        }

        fn schliessen(&mut self) {
            self.geschlossen += 1;
        }
    }

    #[test]
    fn solange_der_prozess_laeuft_wird_gewartet_und_nichts_geschlossen() {
        let mut abschluss = Abschluss::default();
        let mut quelle = Attrappe::default();
        for _ in 0..3 {
            assert_eq!(abschluss.schritt(false, &mut quelle), Aktion::Warten);
        }
        assert_eq!((quelle.gefragt, quelle.geschlossen), (3, 0));
    }

    #[test]
    fn ist_der_prozess_zu_ende_und_die_ausgabe_nicht_wird_einmal_geschlossen_und_nicht_gemeldet() {
        let mut abschluss = Abschluss::default();
        let mut quelle = Attrappe {
            beendet: true,
            ..Attrappe::default()
        };
        assert_eq!(abschluss.schritt(false, &mut quelle), Aktion::Schliessen);
        assert_eq!(quelle.geschlossen, 1);
        // the rest of the output is still on its way: wait, close nothing more, ask no more
        for _ in 0..3 {
            assert_eq!(abschluss.schritt(false, &mut quelle), Aktion::Warten);
        }
        assert_eq!((quelle.gefragt, quelle.geschlossen), (1, 1));
    }

    #[test]
    fn kindende_kommt_erst_nach_dem_ende_der_ausgabe() {
        let mut abschluss = Abschluss::default();
        let mut quelle = Attrappe::default();
        // the process runs, then ends; at no step before the end of the output is `Melden` the answer
        let mut aktionen = vec![abschluss.schritt(false, &mut quelle)];
        quelle.beendet = true;
        aktionen.push(abschluss.schritt(false, &mut quelle));
        aktionen.push(abschluss.schritt(false, &mut quelle));
        assert_eq!(
            aktionen,
            [Aktion::Warten, Aktion::Schliessen, Aktion::Warten]
        );
        // the output ends after the closing: now, and only now, the end is reported
        assert_eq!(abschluss.schritt(true, &mut quelle), Aktion::Melden);
        assert_eq!(quelle.geschlossen, 1);
    }

    #[test]
    fn endet_die_ausgabe_von_selbst_wird_gemeldet_ohne_den_prozess_zu_fragen_und_ohne_zu_schliessen()
     {
        // Linux and macOS: the output ends with the child
        let mut abschluss = Abschluss::default();
        let mut quelle = Attrappe {
            beendet: true,
            ..Attrappe::default()
        };
        assert_eq!(abschluss.schritt(true, &mut quelle), Aktion::Melden);
        assert_eq!((quelle.gefragt, quelle.geschlossen), (0, 0));
    }
}
