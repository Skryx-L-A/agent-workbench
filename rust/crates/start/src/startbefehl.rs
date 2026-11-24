//! The start command of the harness `copilot` and the form of the key of a folder mark (contract 1).
//!
//! Both are pure: they build a list of arguments or test a text and touch neither the environment nor
//! the file system. The program `Lauf starten` calls them; the case files of contract 1 describe the result.

use einstellungen::Werkzeug;

/// The model name that lets the Copilot CLI choose (contract 1, "Modell `auto`").
pub const MODELL_AUTO: &str = "auto";

/// The argument list of `copilot` (contract 1, table of harnesses and "Startform").
///
/// The model comes first: `--model <modell>`, or `--model auto --auto-tier efficiency` when `modell` is
/// `auto`. Then `--disable-builtin-mcps` and `--additional-mcp-config @<mcp_datei>`. Last, for every tool
/// kind of `werkzeuge`, in the given order, `--allow-tool <wert>` and nothing else: the type [`Werkzeug`]
/// has no value that could mean "allow everything", so no `--allow-all*` option can come out of this
/// function.
#[must_use]
pub fn copilot_argv(modell: &str, werkzeuge: &[Werkzeug], mcp_datei: &str) -> Vec<String> {
    let mut argv: Vec<String> = ["copilot", "--model", modell]
        .iter()
        .map(|teil| (*teil).to_owned())
        .collect();
    if modell == MODELL_AUTO {
        argv.push("--auto-tier".to_owned());
        argv.push("efficiency".to_owned());
    }
    argv.push("--disable-builtin-mcps".to_owned());
    argv.push("--additional-mcp-config".to_owned());
    argv.push(format!("@{mcp_datei}"));
    for werkzeug in werkzeuge {
        argv.push("--allow-tool".to_owned());
        argv.push(werkzeug.wert().to_owned());
    }
    argv
}

/// Tells whether `text` has the form of the key of a folder mark: 32 characters of `0-9` and `a-f`
/// (128 bits, lower-case hex), as contract 1 requires in `ordnermarken.json`.
#[must_use]
pub fn ist_marken_kennung(text: &str) -> bool {
    text.len() == 32
        && text
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}

#[cfg(test)]
mod tests {
    use super::{copilot_argv, ist_marken_kennung};
    use einstellungen::Werkzeug;

    const MCP: &str = "/srv/beispiel/.copilot/wb-mcp.json";

    #[test]
    fn a_fixed_model_has_no_auto_tier_and_no_tool_option() {
        assert_eq!(
            copilot_argv("claude-sonnet-5.5", &[], MCP),
            [
                "copilot",
                "--model",
                "claude-sonnet-5.5",
                "--disable-builtin-mcps",
                "--additional-mcp-config",
                "@/srv/beispiel/.copilot/wb-mcp.json"
            ]
        );
    }

    #[test]
    fn auto_adds_the_efficiency_tier_right_after_the_model() {
        assert_eq!(
            copilot_argv("auto", &[], MCP)[..5],
            ["copilot", "--model", "auto", "--auto-tier", "efficiency"]
        );
        assert!(!copilot_argv("Auto", &[], MCP).contains(&"--auto-tier".to_owned()));
        assert!(!copilot_argv("auto2", &[], MCP).contains(&"--auto-tier".to_owned()));
    }

    #[test]
    fn every_tool_kind_adds_one_pair_in_the_given_order_at_the_end() {
        let argv = copilot_argv("auto", &[Werkzeug::SuchMcp, Werkzeug::Shell], MCP);
        assert_eq!(
            argv[argv.len() - 4..],
            ["--allow-tool", "such-mcp", "--allow-tool", "shell"]
        );
        assert_eq!(argv.len(), 5 + 3 + 4);
        let alle = copilot_argv(
            "m",
            &[Werkzeug::Shell, Werkzeug::Write, Werkzeug::SuchMcp],
            MCP,
        );
        assert_eq!(alle.iter().filter(|a| *a == "--allow-tool").count(), 3);
    }

    #[test]
    fn no_allow_all_option_ever_comes_out() {
        let argv = copilot_argv(
            "auto",
            &[Werkzeug::Shell, Werkzeug::Write, Werkzeug::SuchMcp],
            MCP,
        );
        assert!(
            argv.iter()
                .all(|a| !a.starts_with("--allow-all") && a != "--yolo")
        );
    }

    #[test]
    fn the_key_of_a_mark_has_32_lower_case_hex_characters() {
        assert!(ist_marken_kennung("a1b2c3d4e5f60718293a4b5c6d7e8f90"));
        assert!(ist_marken_kennung(&"0".repeat(32)));
        assert!(ist_marken_kennung(&"f".repeat(32)));
        assert!(!ist_marken_kennung(&"a".repeat(31)));
        assert!(!ist_marken_kennung(&"a".repeat(33)));
        assert!(!ist_marken_kennung(&"A".repeat(32)));
        assert!(!ist_marken_kennung(&"g".repeat(32)));
        assert!(!ist_marken_kennung(&"é".repeat(16)));
        assert!(!ist_marken_kennung(""));
        assert!(!ist_marken_kennung("a1b2c3d4e5f60718293a4b5c6d7e8f9 "));
    }
}
