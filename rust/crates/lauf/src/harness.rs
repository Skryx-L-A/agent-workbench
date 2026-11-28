//! The start form of each harness (contract 1, harness table and "Startform"): the full argv and the configuration file
//! that "Lauf starten" writes outside and the enclosure binds read-only.

use std::path::Path;

use einstellungen::json::text_schreiben;

/// The provider of `pi` and of `opencode` (contract 1).
pub const PI_ANBIETER: &str = "wb-lokal";
/// The fixed text in place of a key in the provider file of `pi`; it is no key.
pub const PI_PLATZHALTER: &str = "wb-ohne-schluessel";
/// The guest point of the model path of the local route (E5).
pub const LOKAL_BASIS: &str = "http://127.0.0.1:11434/v1";

/// The state folder name of the harness below the home (one segment).
#[must_use]
pub fn zustand_name(harness: &str) -> &'static str {
    match harness {
        "pi" => ".pi",
        "copilot" => ".copilot",
        "opencode" => einschluss::OPENCODE_ZUSTAND,
        _ => ".claude",
    }
}

/// The place of the configuration file below the home, for the harnesses that have one.
#[must_use]
pub fn datei_ziel(harness: &str) -> Option<&'static str> {
    match harness {
        "pi" => Some(".pi/agent/models.json"),
        "copilot" => Some(".copilot/wb-mcp.json"),
        "opencode" => Some(".opencode-sitzung/config/opencode/opencode.json"),
        _ => None,
    }
}

/// Whether the effort of `harness` is an option of the start command. `opencode` takes it as an entry of its
/// configuration file instead ([`opencode_konfig`]): its interface has no option for it (measured with 1.19.5).
#[must_use]
pub fn effort_im_argv(harness: &str) -> bool {
    harness != "opencode"
}

/// The option of the effort of each harness (contract 1, "Startform mit Effort"), right after the model.
#[must_use]
pub fn effort_option(harness: &str) -> &'static str {
    match harness {
        "pi" => "--thinking",
        "copilot" => "--reasoning-effort",
        _ => "--effort",
    }
}

/// The option of a new start of version 2: the key the program chose (contract 1, "Optionen je Harness").
#[must_use]
pub fn neu_optionen(kennung: &str) -> Vec<String> {
    vec!["--session-id".to_owned(), kennung.to_owned()]
}

/// The option to continue the harness session `kennung` (column "Fortsetzen" of the harness table): `--resume <uuid>`
/// for `claude`, one element `--resume=<uuid>` for `copilot`, `--session <uuid>` for `pi`. `None` for a harness without
/// one, among them `opencode`: it takes no key of a new session from outside, and its `--continue` starts a new
/// session silently when there is none (measured with 1.19.5). Never `--session-id`, `--continue` or a fork.
#[must_use]
pub fn fortsetzen_optionen(harness: &str, kennung: &str) -> Option<Vec<String>> {
    match harness {
        "claude" => Some(vec!["--resume".to_owned(), kennung.to_owned()]),
        "copilot" => Some(vec![format!("--resume={kennung}")]),
        "pi" => Some(vec!["--session".to_owned(), kennung.to_owned()]),
        _ => None,
    }
}

/// The full argv of the harness. `programm` comes from the own configuration, `modell` from the checked start file
/// and stays one element. For `copilot` the list is the one of `start::copilot_argv` (model or `auto` with the
/// efficiency tier, the MCP file, one `--allow-tool` per value of the settings block `harness`), with the program path
/// in place of the bare name. An `effort` adds exactly one pair right after the model; `sitzung` (the session option of
/// version 2) stands at the end.
#[must_use]
pub fn argv(
    harness: &str,
    programm: &Path,
    modell: &str,
    heim: &Path,
    werkzeuge: &[einstellungen::Werkzeug],
    effort: Option<&str>,
    sitzung: &[String],
) -> Vec<String> {
    let pfad = programm.to_string_lossy().into_owned();
    let mut a = match harness {
        "pi" => [pfad.as_str(), "--provider", PI_ANBIETER, "--model", modell]
            .map(str::to_owned)
            .to_vec(),
        "copilot" => {
            let konfig = heim.join(".copilot/wb-mcp.json");
            let mut a = start::copilot_argv(modell, werkzeuge, &konfig.to_string_lossy());
            a[0] = pfad;
            a
        }
        "opencode" => vec![
            pfad,
            "--model".to_owned(),
            format!("{PI_ANBIETER}/{modell}"),
        ],
        _ => [pfad.as_str(), "--model", modell]
            .map(str::to_owned)
            .to_vec(),
    };
    if let Some(e) = effort.filter(|_| effort_im_argv(harness)) {
        // the model is the element after `--model` in every form
        let nach = a
            .iter()
            .position(|x| x == "--model")
            .map_or(a.len(), |i| i + 2);
        a.splice(
            nach..nach,
            [effort_option(harness).to_owned(), e.to_owned()],
        );
    }
    a.extend_from_slice(sitzung);
    a
}

/// The excerpt of the argv of the harness for a log line: the name of the harness, every option name (`--x`, of
/// `--x=<wert>` only `--x`), and only the values of `--model` and of the effort option of the harness. Every other value
/// (paths, keys of a session, a prompt) is left out, so the line never carries a path with a user name. The model and the
/// effort come from the checked start file and settings (forms of contract 1 and 2).
#[must_use]
pub fn argv_auszug(harness: &str, argv: &[String]) -> String {
    let mut teile = vec![harness.to_owned()];
    let mut wert_folgt = false;
    for a in argv.iter().skip(1) {
        if wert_folgt {
            teile.push(a.clone());
            wert_folgt = false;
        } else if let Some(name) = a.strip_prefix("--") {
            let name = name.split('=').next().unwrap_or_default();
            teile.push(format!("--{name}"));
            wert_folgt = !a.contains('=')
                && (a == "--model" || (effort_im_argv(harness) && a == effort_option(harness)));
        }
    }
    teile.join(" ")
}

/// The action name of the log line of the hand-over (contract 5, section 6.9).
pub const ZEILE_AKTION: &str = "lauf.harness";

/// The object of the log line `lauf.harness` (contract 5, section 6.9) without `fassung`, `vorher` and `zeit`, which
/// `formate::kette::anhaengen_zeile` sets: the excerpt of the argv ([`argv_auszug`]), the harness, the model and the
/// effort passed on (`null` without one), route and session. No other value: no environment, no path, no prompt.
#[must_use]
pub fn zeile(
    harness: &str,
    modell: &str,
    effort: Option<&str>,
    route: &str,
    sitzung: &str,
    argv: &[String],
) -> serde_json::Value {
    serde_json::json!({
        "aktionsname": ZEILE_AKTION,
        "argv": argv_auszug(harness, argv),
        "effort": effort,
        "ergebnis": "vor_uebergabe",
        "harness": harness,
        "modell": modell,
        "route": route,
        "sitzung": sitzung,
    })
}

/// The provider file of `pi`: the guest point of the model path and the one model of the route.
#[must_use]
pub fn pi_anbieter(modell: &str) -> String {
    format!(
        "{{\"providers\":{{\"{PI_ANBIETER}\":{{\"baseUrl\":\"{LOKAL_BASIS}\",\"api\":\"openai-completions\",\"apiKey\":\"{PI_PLATZHALTER}\",\"models\":[{{\"id\":{}}}]}}}}}}\n",
        text_schreiben(modell)
    )
}

/// The configuration file of `opencode`: the provider `wb-lokal` at the guest point of the model path, without a key,
/// with the one model of the route, and its effort as `options.reasoningEffort` of the model (sent as
/// `reasoning_effort`, measured with 1.19.5 for all seven values). No MCP server, no plugin, no sharing, no update.
#[must_use]
pub fn opencode_konfig(modell: &str, effort: Option<&str>) -> String {
    let m = text_schreiben(modell);
    let optionen = effort.map_or_else(String::new, |e| {
        format!(",\"options\":{{\"reasoningEffort\":{}}}", text_schreiben(e))
    });
    format!(
        "{{\"autoupdate\":false,\"share\":\"disabled\",\"model\":{},\"provider\":{{\"{PI_ANBIETER}\":{{\"npm\":\"@ai-sdk/openai-compatible\",\"name\":\"{PI_ANBIETER}\",\"options\":{{\"baseURL\":\"{LOKAL_BASIS}\"}},\"models\":{{{m}:{{\"name\":{m},\"tool_call\":true{optionen}}}}}}}}}}}\n",
        text_schreiben(&format!("{PI_ANBIETER}/{modell}"))
    )
}

/// The MCP file of copilot: the search tool of the session as the only server (contract 4).
#[must_use]
pub fn copilot_mcp(such_mcp: &Path) -> String {
    format!(
        "{{\"mcpServers\":{{\"such-mcp\":{{\"type\":\"local\",\"command\":{},\"args\":[],\"tools\":[\"*\"]}}}}}}\n",
        text_schreiben(&such_mcp.to_string_lossy())
    )
}

/// The text of the configuration file of the harness, if it has one. `effort` matters only for `opencode`.
#[must_use]
pub fn datei_text(
    harness: &str,
    modell: &str,
    such_mcp: &Path,
    effort: Option<&str>,
) -> Option<String> {
    match harness {
        "pi" => Some(pi_anbieter(modell)),
        "copilot" => Some(copilot_mcp(such_mcp)),
        "opencode" => Some(opencode_konfig(modell, effort)),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
