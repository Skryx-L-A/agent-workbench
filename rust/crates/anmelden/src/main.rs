//! `werkbank-anmelden copilot [--uebernehmen] [--neu]`: process shell. Arguments, signals and output; the logic is
//! in the library. Seams for tests (`WB_ANMELDEN_PASSWD`, a file read in place of `/etc/passwd` that names the home,
//! `WB_ANMELDEN_COPILOT_HEIM`, `WB_ANMELDEN_WURZEL`, and the probe `WB_ANMELDEN_PROBE` with `WB_ANMELDEN_PROBE_MARKE`,
//! and `grenze-weich-0` for the check before the child) exist only in the test build (feature `pruefbau`) and count there only with `WB_TESTUMGEBUNG=1`. The delivered build has no
//! seam: it never reads these variables, the paths are fixed and there is no probe.

// One `main` with the platform choice inside: cargo-mutants reads the source as text and does not evaluate `cfg`, so a
// second `main` for another platform would be a function no test on this machine can reach.
fn main() -> std::process::ExitCode {
    #[cfg(unix)]
    let code = unix::lauf();
    #[cfg(not(unix))]
    let code = std::process::ExitCode::from(64);
    code
}

#[cfg(unix)]
mod unix {
    use std::io::Write;
    use std::path::PathBuf;
    use std::process::ExitCode;
    use std::sync::Arc;
    use std::sync::atomic::AtomicUsize;

    use anmelden::fehler::Fehler;
    use anmelden::lauf::{
        TempArt, Umgebung, ablage_suchen, anmelden, durchreichen_aus_umgebung, heim_aus_passwd,
        reste_raeumen, temp_reste_in, temp_reste_raeumen, wegwerf_wurzel,
    };
    use anmelden::uebernehmen::{Probe, Punkt, uebernehmen};

    pub fn lauf() -> ExitCode {
        // Before anything is read: no core image may hold a token (contract 9, section 3c).
        match anmelden::schutz::abschirmen() {
            Ok(None) => {}
            Ok(Some(hinweis)) => {
                let _ = writeln!(std::io::stderr(), "Hinweis: {}", hinweis.text());
            }
            Err((fehler, grund)) => {
                let code = fehler_ausgeben(fehler);
                let _ = writeln!(
                    std::io::stderr(),
                    "werkbank-anmelden: {}: {}",
                    grund.grund(),
                    grund.abhilfe()
                );
                return code;
            }
        }
        // Probe of the test build only (feature `pruefbau`, `WB_TESTUMGEBUNG=1`): the soft limit drops to 0 below the hard 1, so the check before the child must refuse.
        if seam(Naht::Probe).is_some_and(|p| p.as_os_str() == "grenze-weich-0") {
            let _ = rustix::process::setrlimit(
                rustix::process::Resource::Core,
                rustix::process::Rlimit {
                    current: Some(0),
                    maximum: Some(kernabbild::GRENZE),
                },
            );
        }
        let args: Vec<String> = std::env::args().skip(1).collect();
        let (uebernahme, neu) = match args
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
            .as_slice()
        {
            ["copilot"] => (false, false),
            ["copilot", "--neu"] => (false, true),
            ["copilot", "--uebernehmen"] => (true, false),
            ["copilot", "--uebernehmen", "--neu"] => (true, true),
            _ => return fehler_ausgeben(Fehler::Aufruf),
        };
        let signal = Arc::new(AtomicUsize::new(0));
        for sig in [
            signal_hook::consts::SIGINT,
            signal_hook::consts::SIGTERM,
            signal_hook::consts::SIGHUP,
        ] {
            let Ok(nummer) = usize::try_from(sig) else {
                continue;
            };
            if signal_hook::flag::register_usize(sig, Arc::clone(&signal), nummer).is_err() {
                return fehler_ausgeben(Fehler::HeimNichtAnlegbar);
            }
        }
        let Some(umg) = umgebung() else {
            return fehler_ausgeben(Fehler::ZielNichtSchreibbar);
        };
        match reste(&umg, uebernahme) {
            Ok(0) => {}
            Ok(n) => {
                let _ = writeln!(
                    std::io::stderr(),
                    "Hinweis: Reste frueherer Anmeldungen wurden geloescht: {n}"
                );
            }
            Err(fehler) => return fehler_ausgeben(fehler),
        }
        let ergebnis = if uebernahme {
            uebernehmen(&umg, neu, &signal, probe().as_ref()).map(|u| {
                if u.wiederanlauf {
                    let _ = writeln!(
                        std::io::stderr(),
                        "Hinweis: Eine unterbrochene Uebernahme wurde abgeschlossen."
                    );
                }
                u.erfolg
            })
        } else {
            anmelden(&umg, neu, &signal)
        };
        match ergebnis {
            Ok(erfolg) => {
                let mut aus = std::io::stdout();
                let _ = writeln!(aus, "Anmeldung gespeichert.");
                if !erfolg.bund_geprueft {
                    let _ = writeln!(
                        std::io::stderr(),
                        "Hinweis: Der Schluesselbund wurde nicht geprueft, weil secret-tool und security fehlen."
                    );
                }
                ExitCode::SUCCESS
            }
            Err(fehler) => fehler_ausgeben(fehler),
        }
    }

    /// Section 3a: the throw-away homes and the temp files of the key (both ways), and with `--uebernehmen` the temp
    /// files of `config.json` in the folder of the CLI. One sum for the notice.
    fn reste(umg: &Umgebung, uebernahme: bool) -> Result<usize, Fehler> {
        let pid = std::process::id();
        let mut n = reste_raeumen(&umg.wegwerf_wurzel, umg.uid, pid)?;
        // The search never makes a level of the store: a missing store has no left-overs.
        if let Some(ablage) = ablage_suchen(&umg.heim) {
            n += temp_reste_in(&ablage, TempArt::Schluessel, umg.uid, pid)?;
        }
        if uebernahme {
            n += temp_reste_raeumen(&umg.echtes_copilot_heim, TempArt::Config, umg.uid, pid)?;
        }
        Ok(n)
    }

    /// The probe of the tests: `toeten:<punkt>`, `warten:<punkt>` (with the mark file) or `config-stoeren`.
    fn probe() -> Option<Probe> {
        let text = seam(Naht::Probe)?;
        let text = text.to_str()?;
        if text == "config-stoeren" {
            return Some(Probe::ConfigStoeren);
        }
        let (art, punkt) = text.split_once(':')?;
        let punkt = Punkt::aus_name(punkt)?;
        match art {
            "toeten" => Some(Probe::Toeten(punkt)),
            "warten" => Some(Probe::Warten(punkt, seam(Naht::ProbeMarke)?)),
            _ => None,
        }
    }

    fn fehler_ausgeben(fehler: Fehler) -> ExitCode {
        let _ = std::io::stderr().write_all(fehler.zeile().as_bytes());
        ExitCode::from(fehler.exit())
    }

    /// The seams of the tests. Only the test build knows their names.
    #[derive(Clone, Copy)]
    enum Naht {
        Passwd,
        CopilotHeim,
        Wurzel,
        Probe,
        ProbeMarke,
    }

    /// The value of the seam. Test build (feature `pruefbau`): only with `WB_TESTUMGEBUNG=1`. Delivered build: no seam,
    /// the environment is not read for it. One function with the choice inside, so that the mutation run with the
    /// feature reaches all of it (cargo-mutants does not evaluate `cfg`).
    fn seam(naht: Naht) -> Option<PathBuf> {
        #[cfg(feature = "pruefbau")]
        let wert = {
            let name = match naht {
                Naht::Passwd => "WB_ANMELDEN_PASSWD",
                Naht::CopilotHeim => "WB_ANMELDEN_COPILOT_HEIM",
                Naht::Wurzel => "WB_ANMELDEN_WURZEL",
                Naht::Probe => "WB_ANMELDEN_PROBE",
                Naht::ProbeMarke => "WB_ANMELDEN_PROBE_MARKE",
            };
            if std::env::var_os("WB_TESTUMGEBUNG").is_some_and(|v| v == "1") {
                std::env::var_os(name).map(PathBuf::from)
            } else {
                None
            }
        };
        #[cfg(not(feature = "pruefbau"))]
        let wert = {
            let _ = naht;
            None
        };
        wert
    }

    fn umgebung() -> Option<Umgebung> {
        let uid = rustix::process::getuid().as_raw();
        // The home comes from `passwd` only (never `HOME`): the holder, this command and `postrm` see the same path.
        let passwd_pfad = seam(Naht::Passwd).unwrap_or_else(|| PathBuf::from("/etc/passwd"));
        let passwd = std::fs::read_to_string(passwd_pfad).ok()?;
        let heim = heim_aus_passwd(&passwd, uid)?;
        let echtes = seam(Naht::CopilotHeim)
            .or_else(|| std::env::var_os("COPILOT_HOME").map(PathBuf::from))
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".copilot")))?;
        let wurzel = seam(Naht::Wurzel).unwrap_or_else(|| wegwerf_wurzel(&heim));
        Some(Umgebung {
            heim,
            suchpfad: std::env::var_os("PATH").unwrap_or_default(),
            echtes_copilot_heim: echtes,
            wegwerf_wurzel: wurzel,
            uid,
            durchreichen: durchreichen_aus_umgebung(),
        })
    }
}
