//! `werkbank-anmelden copilot [--uebernehmen] [--neu]`: process shell. Arguments, signals and output; the logic is
//! in the library. Seams for tests (`WB_ANMELDEN_ZIEL`, `WB_ANMELDEN_COPILOT_HEIM`, `WB_ANMELDEN_WURZEL`, and the
//! probe `WB_ANMELDEN_PROBE` with `WB_ANMELDEN_PROBE_MARKE`, and `grenze-weich-0` for the check before the child) count only with `WB_TESTUMGEBUNG=1`; without it the
//! paths are fixed and there is no probe.

#[cfg(unix)]
fn main() -> std::process::ExitCode {
    unix::lauf()
}

#[cfg(not(unix))]
fn main() -> std::process::ExitCode {
    std::process::ExitCode::from(64)
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
        TempArt, Umgebung, anmelden, durchreichen_aus_umgebung, nutzer_ordner, reste_raeumen,
        temp_reste_raeumen,
    };
    use anmelden::uebernehmen::{Probe, Punkt, uebernehmen};

    const NUTZER_ORT: &str = "/var/lib/werkbank";

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
        // Probe of the tests only (`WB_TESTUMGEBUNG=1`): the soft limit drops to 0 below the hard 1, so the check before the child must refuse.
        if seam("WB_ANMELDEN_PROBE").is_some_and(|p| p.as_os_str() == "grenze-weich-0") {
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
        n += temp_reste_raeumen(&umg.ziel_ordner, TempArt::Schluessel, umg.uid, pid)?;
        if uebernahme {
            n += temp_reste_raeumen(&umg.echtes_copilot_heim, TempArt::Config, umg.uid, pid)?;
        }
        Ok(n)
    }

    /// The probe of the tests: `toeten:<punkt>`, `warten:<punkt>` (with the mark file) or `config-stoeren`.
    fn probe() -> Option<Probe> {
        let text = seam("WB_ANMELDEN_PROBE")?;
        let text = text.to_str()?;
        if text == "config-stoeren" {
            return Some(Probe::ConfigStoeren);
        }
        let (art, punkt) = text.split_once(':')?;
        let punkt = Punkt::aus_name(punkt)?;
        match art {
            "toeten" => Some(Probe::Toeten(punkt)),
            "warten" => Some(Probe::Warten(punkt, seam("WB_ANMELDEN_PROBE_MARKE")?)),
            _ => None,
        }
    }

    fn fehler_ausgeben(fehler: Fehler) -> ExitCode {
        let _ = std::io::stderr().write_all(fehler.zeile().as_bytes());
        ExitCode::from(fehler.exit())
    }

    fn seam(name: &str) -> Option<PathBuf> {
        if std::env::var_os("WB_TESTUMGEBUNG").is_some_and(|v| v == "1") {
            std::env::var_os(name).map(PathBuf::from)
        } else {
            None
        }
    }

    fn umgebung() -> Option<Umgebung> {
        let uid = rustix::process::getuid().as_raw();
        let ziel_ordner = if let Some(p) = seam("WB_ANMELDEN_ZIEL") {
            p
        } else {
            let passwd = std::fs::read_to_string("/etc/passwd").ok()?;
            nutzer_ordner(std::path::Path::new(NUTZER_ORT), &passwd, uid)?
        };
        let echtes = seam("WB_ANMELDEN_COPILOT_HEIM")
            .or_else(|| std::env::var_os("COPILOT_HOME").map(PathBuf::from))
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".copilot")))?;
        let wurzel = seam("WB_ANMELDEN_WURZEL").unwrap_or_else(|| PathBuf::from("/var/tmp"));
        Some(Umgebung {
            ziel_ordner,
            suchpfad: std::env::var_os("PATH").unwrap_or_default(),
            echtes_copilot_heim: echtes,
            wegwerf_wurzel: wurzel,
            uid,
            durchreichen: durchreichen_aus_umgebung(),
        })
    }
}
