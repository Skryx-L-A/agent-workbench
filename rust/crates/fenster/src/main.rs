//! `werkbank-fenster`: the app window of the pilot.
//!
//! ```text
//! werkbank-fenster --attrappe ziel|heute     the window on the mock-up of the local path (target state or today's messages)
//! werkbank-fenster --attrappe ziel --admin   the same, the viewer an admin of the mock-up (B78)
//! werkbank-fenster --attrappe ziel --szene <name> --thema hell|dunkel
//!                                             the window on one situation of OF13 (the names of --bilder), for the
//!                                             acceptance pictures of a real start
//! werkbank-fenster --bilder <ordner>          the pictures of OF13 without a window (1280 px, light and dark)
//! werkbank-fenster --licenses                 the notice of the files taken into the crate (the bold font)
//! ```
//! Without `--attrappe` the window has no path to the service yet (gap L11) and ends with exit code 2.

use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use fenster::attrappe::{Attrappe, Form};
use fenster::fenster::{Fenster, Szene};
use fenster::merken::{self, Merken};

const HILFE: &str = "werkbank-fenster --attrappe ziel|heute [--admin] | --attrappe ziel --szene <name> --thema hell|dunkel | --bilder <ordner> | --licenses\n";

/// A situation of OF13 for a real start: the situation, whether the viewer is an admin, the theme of the system.
struct Lage {
    szene: Szene,
    admin: bool,
    dunkel: bool,
}

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
        ["--attrappe", form] | ["--attrappe", form, "--admin"] => {
            let form = match *form {
                "ziel" => Form::Ziel,
                "heute" => Form::Heute,
                _ => {
                    melden(HILFE);
                    return ExitCode::from(2);
                }
            };
            fenster_zeigen(form, args.len() == 3, None)
        }
        ["--attrappe", "ziel", "--szene", name, "--thema", thema] => {
            let Some((_, szene, admin)) = fenster::szenen().into_iter().find(|(n, _, _)| n == name)
            else {
                melden(HILFE);
                return ExitCode::from(2);
            };
            let dunkel = match *thema {
                "hell" => false,
                "dunkel" => true,
                _ => {
                    melden(HILFE);
                    return ExitCode::from(2);
                }
            };
            fenster_zeigen(
                Form::Ziel,
                admin,
                Some(Lage {
                    szene,
                    admin,
                    dunkel,
                }),
            )
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

/// The window that takes the situation after the first frames, as `--bilder` does.
struct Szenenstart {
    f: Fenster,
    lage: Option<Lage>,
    bilder: usize,
}

impl eframe::App for Szenenstart {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        self.bilder += 1;
        // first frames take the stream and subscribe, then the situation; the window paints on until it stands still
        if self.bilder == 4
            && let Some(lage) = self.lage.take()
        {
            self.f.szene(&lage.szene);
        }
        eframe::App::ui(&mut self.f, ui, frame);
        if self.bilder < 30 {
            ui.ctx().request_repaint();
        }
    }

    fn on_exit(&mut self, gl: Option<&eframe::glow::Context>) {
        eframe::App::on_exit(&mut self.f, gl);
    }
}

fn fenster_zeigen(form: Form, admin: bool, lage: Option<Lage>) -> ExitCode {
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
    let (attrappe, griff) = Attrappe::neu(form);
    // a situation sets the viewer as the situation does; otherwise `--admin` does (B78)
    griff
        .borrow_mut()
        .admin_setzen(lage.as_ref().map_or(admin, |l| l.admin));
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
            let Some(l) = lage else {
                return Ok(Box::new(f));
            };
            f.system_dunkel = Some(l.dunkel);
            Ok(Box::new(Szenenstart {
                f,
                lage: Some(l),
                bilder: 0,
            }))
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
