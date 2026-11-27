//! `werkbank-fenster`: the app window of the pilot.
//!
//! ```text
//! werkbank-fenster --attrappe ziel|heute     the window on the mock-up of the local path (target state or today's messages)
//! werkbank-fenster --bilder <ordner>          the pictures of OF13 without a window (1280 px, light and dark)
//! werkbank-fenster --licenses                 the notice of the files taken into the crate (the bold font)
//! ```
//! Without `--attrappe` the window has no path to the service yet (gap L11) and ends with exit code 2.

use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use fenster::attrappe::{Attrappe, Form};
use fenster::fenster::Fenster;
use fenster::merken::{self, Merken};

const HILFE: &str = "werkbank-fenster --attrappe ziel|heute | --bilder <ordner> | --licenses\n";

fn melden(text: &str) {
    let _ = writeln!(std::io::stderr(), "{text}");
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["--bilder", ordner] => match fenster::bilder(&PathBuf::from(ordner)) {
            Ok(dateien) => {
                for d in dateien {
                    let _ = writeln!(std::io::stdout(), "{}", d.display());
                }
                ExitCode::SUCCESS
            }
            Err(e) => {
                melden(&format!("werkbank-fenster: Bilder nicht geschrieben: {e}"));
                ExitCode::from(1)
            }
        },
        ["--attrappe", form] => {
            let form = match *form {
                "ziel" => Form::Ziel,
                "heute" => Form::Heute,
                _ => {
                    melden(HILFE);
                    return ExitCode::from(2);
                }
            };
            fenster_zeigen(form)
        }
        ["--licenses"] => {
            if std::io::stdout()
                .write_all(fenster::NOTICE.as_bytes())
                .is_ok()
            {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            }
        }
        ["--hilfe" | "-h"] => {
            let _ = write!(std::io::stdout(), "{HILFE}");
            ExitCode::SUCCESS
        }
        _ => {
            melden(
                "werkbank-fenster: Der lokale Weg zum Dienst (L11) fehlt noch. Start mit --attrappe ziel oder --attrappe heute.",
            );
            ExitCode::from(2)
        }
    }
}

fn fenster_zeigen(form: Form) -> ExitCode {
    let datei = merken::datei();
    let gemerkt = datei.as_deref().map(Merken::lesen).unwrap_or_default();
    let (b, h) = gemerkt.groesse.unwrap_or((1280.0, 800.0));
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Werkbank")
        .with_inner_size([b, h])
        .with_min_inner_size([720.0, 480.0]);
    if let Some((x, y)) = gemerkt.lage {
        viewport = viewport.with_position([x, y]);
    }
    let optionen = eframe::NativeOptions {
        viewport,
        // Control+C goes to the program as a key (SE1 point 6); the window copies with Control+Shift+C itself
        clipboard_shortcuts: false,
        ..eframe::NativeOptions::default()
    };
    let (attrappe, _griff) = Attrappe::neu(form);
    let ergebnis = eframe::run_native(
        "Werkbank",
        optionen,
        Box::new(move |cc| {
            fenster::einrichten(&cc.egui_ctx);
            #[allow(unused_mut)]
            let mut f = Fenster::neu(Box::new(attrappe), gemerkt, datei);
            // B75 under macOS: the menu bar of the system, now that the application exists
            #[cfg(target_os = "macos")]
            f.menueleiste_einrichten(&cc.egui_ctx);
            Ok(Box::new(f))
        }),
    );
    match ergebnis {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            melden(&format!("werkbank-fenster: {e}"));
            ExitCode::from(1)
        }
    }
}
