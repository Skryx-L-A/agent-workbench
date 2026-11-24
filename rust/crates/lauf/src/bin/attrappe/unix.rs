//! The stand-in of the tests (feature `pruefbau` only): the search process, the model path and the harness probe.
//!
//! What it does is written in the file `<program>.verhalten` next to its own path (the children get an empty
//! environment, so a variable cannot carry it). It records its arguments in `<program>.argv` and its PID in
//! `<program>.pid`. Behaviours:
//! - `taub:<behaviour>`: first clears its death signal (`PR_SET_PDEATHSIG` 0), then behaves as `<behaviour>`; it stands for
//!   a child to which the kernel does not deliver the signal (`AppArmor` `signal` denied to the starter);
//! - `bereit`: binds the Unix socket named after `--socket`, writes the ready line and waits for ever;
//! - `stumm`: writes nothing and waits; `zeile:<text>`: writes `<text>` and LF and waits; `ende:<n>`: ends with `n`;
//! - `probe <marke> <pfad>...`: the harness probe. Writes on standard output the environment (`env <name>=<value>`),
//!   its open descriptors (`fd <n> <target>`), the number of entries of `/proc/*/environ` that hold the mark
//!   (`environ_treffer <n>`), the parent PID and for every further path whether it can be opened (`pfad <p> <result>`);
//!   `schreib:<pfad>` tries to make the file and reports `schreib <pfad> <result>`.
//!
//! When `package.json` beside the program names it as the main package of the cloud harness (`name` is
//! `@github/copilot`, `bin.copilot` its file name), it first acts like the loader of the CLI (contract 1, "Paket der
//! Copilot CLI"): the package folder is `($XDG_CACHE_HOME or $HOME/.cache)/copilot/pkg/linux-<arch>/<version of the
//! package>`. With `--version` and an incomplete folder it extracts the four paths (`app.js` of 20 MiB, more than the home
//! of the enclosure holds; the mark last) and ends with 0, or names `ENOSPC` and `Failed to extract bundled package` and
//! ends with 1; with a complete folder it writes its version line only. Every other start without the four paths ends
//! with that line and 1; with them it goes on as below.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

fn eigen() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.canonicalize().ok())
        .unwrap_or_default()
}

fn neben(endung: &str) -> PathBuf {
    let mut p = eigen().into_os_string();
    p.push(endung);
    PathBuf::from(p)
}

fn warten() -> ! {
    loop {
        std::thread::sleep(std::time::Duration::from_secs(3600));
    }
}

fn option(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1).cloned())
}

fn fehlername(e: &std::io::Error) -> String {
    match e.raw_os_error() {
        Some(2) => "ENOENT".to_owned(),
        Some(13) => "EACCES".to_owned(),
        Some(1) => "EPERM".to_owned(),
        Some(28) => "ENOSPC".to_owned(),
        Some(30) => "EROFS".to_owned(),
        Some(3) => "ESRCH".to_owned(),
        Some(n) => format!("E{n}"),
        None => "E?".to_owned(),
    }
}

/// One line for each PID that is seen (readable or unreadable with the error), then the number of hits of the mark.
fn environ_zeilen(marke: &str, aus: &mut dyn Write) {
    let mut treffer = 0;
    let _ = writeln!(aus, "selbst {}", std::process::id());
    if let Ok(liste) = std::fs::read_dir("/proc") {
        let mut pids: Vec<u32> = liste
            .filter_map(Result::ok)
            .filter_map(|e| e.file_name().to_str()?.parse().ok())
            .collect();
        pids.sort_unstable();
        for pid in pids {
            // every process that is seen gets one line: a process whose start environment cannot be read must never
            // count as "no hit" without a word
            let comm = std::fs::read_to_string(format!("/proc/{pid}/comm"))
                .map_or_else(|_| "?".to_owned(), |t| t.trim_end().to_owned());
            let mut inhalt = Vec::new();
            match std::fs::File::open(format!("/proc/{pid}/environ"))
                .and_then(|mut f| f.read_to_end(&mut inhalt))
            {
                Ok(_) => {
                    let _ = writeln!(aus, "environ {pid} lesbar");
                    if !marke.is_empty()
                        && inhalt.windows(marke.len()).any(|w| w == marke.as_bytes())
                    {
                        treffer += 1;
                    }
                }
                Err(e) => {
                    let _ = writeln!(aus, "environ {pid} unlesbar {}", fehlername(&e));
                }
            }
            let _ = writeln!(aus, "prozess {pid} {comm}");
        }
    }
    let _ = writeln!(aus, "environ_treffer {treffer}");
}

fn probe(teile: &[&str], aus: &mut dyn Write) {
    let marke = teile.first().copied().unwrap_or("");
    for (n, w) in std::env::vars_os() {
        let _ = writeln!(aus, "env {}={}", n.to_string_lossy(), w.to_string_lossy());
    }
    if let Ok(liste) = std::fs::read_dir("/proc/self/fd") {
        let mut fds: Vec<(i32, String)> = liste
            .filter_map(Result::ok)
            .filter_map(|e| {
                let n = e.file_name().to_str()?.parse().ok()?;
                let ziel = std::fs::read_link(e.path()).ok()?;
                Some((n, ziel.to_string_lossy().into_owned()))
            })
            .collect();
        fds.sort();
        for (n, z) in fds {
            let _ = writeln!(aus, "fd {n} {z}");
        }
    }
    environ_zeilen(marke, aus);
    let _ = writeln!(aus, "ppid {}", std::os::unix::process::parent_id());
    for pfad in teile.iter().skip(1) {
        // `kill:<file>`: signal 0 to the PID written in the file (property E3)
        if let Some(datei) = pfad.strip_prefix("kill:") {
            let pid = std::fs::read_to_string(datei)
                .ok()
                .and_then(|t| t.trim().parse::<i32>().ok())
                .and_then(rustix::process::Pid::from_raw);
            let ergebnis = match pid.map(rustix::process::test_kill_process) {
                Some(Ok(())) => "OK".to_owned(),
                Some(Err(e)) => fehlername(&std::io::Error::from_raw_os_error(e.raw_os_error())),
                None => "keine-pid".to_owned(),
            };
            let _ = writeln!(aus, "kill {datei} {ergebnis}");
            continue;
        }
        // `befehl:<program>,<argument>,...`: runs the program and reports its exit code and its line on standard error
        if let Some(rest) = pfad.strip_prefix("befehl:") {
            let mut teile = rest.split(',');
            let programm = teile.next().unwrap_or("");
            let kind = std::process::Command::new(programm)
                .args(teile)
                .env_clear()
                .stdin(std::process::Stdio::null())
                .output();
            match kind {
                Ok(o) => {
                    let _ = writeln!(aus, "befehl exit {:?}", o.status.code());
                    let _ = writeln!(
                        aus,
                        "befehl stderr {}",
                        String::from_utf8_lossy(&o.stderr).trim_end()
                    );
                    let _ = writeln!(aus, "befehl stdout {}", o.stdout.len());
                }
                Err(e) => {
                    let _ = writeln!(aus, "befehl start {}", fehlername(&e));
                }
            }
            continue;
        }
        if let Some(datei) = pfad.strip_prefix("schreib:") {
            let ergebnis = match std::fs::File::create(datei) {
                Ok(_) => "OK".to_owned(),
                Err(e) => fehlername(&e),
            };
            let _ = writeln!(aus, "schreib {datei} {ergebnis}");
            continue;
        }
        let ergebnis = match std::fs::File::open(pfad) {
            Ok(_) => "OK".to_owned(),
            Err(e) => fehlername(&e),
        };
        let _ = writeln!(aus, "pfad {pfad} {ergebnis}");
    }
}

/// The platform and version of the package when this program is the main program of the cloud harness.
fn sea_paket() -> Option<(String, String)> {
    use einstellungen::json::{Wert, lesen};
    let programm = eigen();
    let name = programm.file_name()?.to_str()?.to_owned();
    let text = std::fs::read(programm.parent()?.join("package.json")).ok()?;
    let Wert::Objekt(f) = lesen(&text).ok()? else {
        return None;
    };
    let feld = |n: &str| f.iter().find(|(k, _)| k == n).map(|(_, w)| w);
    if feld("name") != Some(&Wert::Text("@github/copilot".to_owned())) {
        return None;
    }
    let Some(Wert::Objekt(bin)) = feld("bin") else {
        return None;
    };
    if !bin
        .iter()
        .any(|(k, w)| k == "copilot" && *w == Wert::Text(name.clone()))
    {
        return None;
    }
    let Some(Wert::Text(fassung)) = feld("version") else {
        return None;
    };
    let arch = match std::env::consts::ARCH {
        "aarch64" => "arm64",
        _ => "x64",
    };
    Some((format!("linux-{arch}"), fassung.clone()))
}

/// The four paths of a complete package folder, the mark last.
fn sea_pfade(plattform: &str) -> [String; 4] {
    [
        "index.js".to_owned(),
        "app.js".to_owned(),
        format!("prebuilds/{plattform}/runtime.node"),
        ".extraction-complete".to_owned(),
    ]
}

/// Extracts like the CLI. `Err` carries the name of the fault.
fn sea_entpacken(ordner: &Path, plattform: &str) -> Result<(), String> {
    for rel in sea_pfade(plattform) {
        let p = ordner.join(&rel);
        if let Some(e) = p.parent() {
            std::fs::create_dir_all(e).map_err(|e| fehlername(&e))?;
        }
        let groesse = if rel == "app.js" { 20 << 20 } else { 64 };
        std::fs::write(&p, vec![b'x'; groesse]).map_err(|e| fehlername(&e))?;
    }
    Ok(())
}

/// The loader part. `Some(code)` ends the program.
fn sea(args: &[String]) -> Option<std::process::ExitCode> {
    let (plattform, fassung) = sea_paket()?;
    let basis = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))?;
    let ordner = basis.join("copilot/pkg").join(&plattform).join(&fassung);
    let vollstaendig = sea_pfade(&plattform)
        .iter()
        .all(|r| ordner.join(r).is_file());
    let mut fehler = std::io::stderr();
    if args == ["--version"] {
        if !vollstaendig {
            // one line per extraction, beside the package folders (the tests count the extractions)
            if let Ok(mut log) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(basis.join("copilot/attrappe-entpackt.log"))
            {
                let _ = writeln!(log, "entpackt {}", std::process::id());
            }
        }
        if !vollstaendig && let Err(name) = sea_entpacken(&ordner, &plattform) {
            let _ = writeln!(
                fehler,
                "Package extraction attempt 1/1 failed: TAR_ENTRY_ERROR({name})"
            );
            let _ = writeln!(fehler, "Failed to extract bundled package");
            return Some(std::process::ExitCode::from(1));
        }
        let _ = writeln!(std::io::stdout(), "attrappe {fassung}");
        return Some(std::process::ExitCode::SUCCESS);
    }
    if !vollstaendig {
        let _ = writeln!(fehler, "Failed to extract bundled package");
        return Some(std::process::ExitCode::from(1));
    }
    None
}

pub fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(code) = sea(&args) {
        return code;
    }
    let verhalten = std::fs::read_to_string(neben(".verhalten")).unwrap_or_default();
    let verhalten = verhalten.trim_end_matches('\n');
    let verhalten = match verhalten.strip_prefix("taub:") {
        Some(rest) => {
            // only Linux has a death signal of the parent; elsewhere there is nothing to switch off
            #[cfg(target_os = "linux")]
            let _ = rustix::process::set_parent_process_death_signal(None);
            rest
        }
        None => verhalten,
    };
    // `spaet:<ms>:<behaviour>`: waits before the behaviour (a slow start that is still within the limit)
    let verhalten = match verhalten
        .strip_prefix("spaet:")
        .and_then(|r| r.split_once(':'))
    {
        Some((ms, rest)) => {
            std::thread::sleep(std::time::Duration::from_millis(ms.parse().unwrap_or(0)));
            rest
        }
        None => verhalten,
    };
    let _ = std::fs::write(neben(".argv"), args.join("\n") + "\n");
    let _ = std::fs::write(neben(".pid"), format!("{}\n", std::process::id()));
    let mut aus = std::io::stdout();
    if let Some(rest) = verhalten.strip_prefix("probe") {
        let teile: Vec<&str> = rest.split_whitespace().collect();
        probe(&teile, &mut aus);
        let _ = aus.flush();
        return std::process::ExitCode::SUCCESS;
    }
    if let Some(n) = verhalten.strip_prefix("ende:") {
        return std::process::ExitCode::from(n.parse::<u8>().unwrap_or(1));
    }
    if let Some(zeile) = verhalten.strip_prefix("zeile:") {
        let _ = writeln!(aus, "{zeile}");
        let _ = aus.flush();
        warten();
    }
    if verhalten == "stumm" {
        warten();
    }
    // bereit (also the default)
    let hoerer =
        option(&args, "--socket").map(|s| std::os::unix::net::UnixListener::bind(Path::new(&s)));
    if matches!(hoerer, Some(Err(_))) {
        return std::process::ExitCode::from(74);
    }
    let _ = aus.write_all(b"{\"bereit\":true}\n");
    let _ = aus.flush();
    let _halten = hoerer;
    warten();
}
