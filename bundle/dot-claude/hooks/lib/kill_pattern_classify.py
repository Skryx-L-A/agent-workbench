import json
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import cmdshell as cs

OWN_SCOPE_SOCKETS = ('wbtest', 'wbstress', 'wbprobe')
DANGEROUS_KILL = {'pkill', 'killall'}
PIPE_TERMINAL_INTERPRETERS = {
    'bash', 'sh', 'zsh', 'dash', 'ksh',
    'python', 'python3', 'perl', 'ruby', 'node', 'osascript',
}
MAX_DEPTH = 5
MAX_COMMAND_LEN = 200_000

INCIDENT = '[[incident-2026-07-25-killmuster-beendete-live-client]]'


def is_own_scoped(text):
    if re.search(r'-L\s+(' + '|'.join(OWN_SCOPE_SOCKETS) + r')(\s|$)', text, re.IGNORECASE):
        return True
    return 'TMUX_TMPDIR=' in text


def is_too_broad_ending(text):
    return bool(re.search(r"(wb-|claude)['\"]?\s*$", text.strip(), re.IGNORECASE))


def has_pgrep_reference(tokens):
    return any('pgrep' in t for t in tokens)


def is_bare_pid_kill(resolved_tokens):
    if not resolved_tokens:
        return False
    for t in resolved_tokens:
        if re.match(r'^-[A-Za-z0-9]+$', t):
            continue
        if re.match(r'^[0-9]+$', t):
            continue
        if cs.var_name_if_bare_ref(t) is not None:
            continue
        return False
    return True


def _pattern_deny(check_text, display_text):
    if is_own_scoped(check_text):
        return None
    if is_too_broad_ending(check_text):
        return (
            "Kill-Muster endet auf generischem Praefix (wb-/claude) ohne spezifischen Suffix -- trifft "
            "JEDE passende Session/Prozess, nicht nur den eigenen Test. Vorfall: pkill -f \"tmux attach "
            "-t =wb-\" hat Live-Client des Nutzers beendet. Umweg: konkrete PID nennen (kill <pid>) oder "
            "eigenen Test-Socket/-Sessionnamen mit Suffix verwenden. Blockiert von "
            "bash-guard-kill-pattern. Teilbefehl: %s" % display_text
        )
    return (
        "pkill/killall/kill (pattern-basiert, z.B. ueber pgrep) ohne nachweisbar engen Bezug im selben "
        "Teilbefehl (kein eigener Test-Socket/-Sessionname erkennbar). Default-Deny lt. Standing Rule "
        "'Kein Kill-Muster, das ueber die eigenen Testprozesse hinausreicht'. Umweg: konkrete PID nennen "
        "(kill <pid>) oder Muster mit eigenem Test-Socket/-Sessionnamen (z.B. -L wbtest) im selben "
        "Teilbefehl versehen. Blockiert von bash-guard-kill-pattern. Teilbefehl: %s" % display_text
    )


def _looks_unresolvable(tokens):
    for t in tokens:
        if '$(' in t or '`' in t:
            return True
        if re.search(r'\$\{?[A-Za-z_][A-Za-z0-9_]*\}?', t):
            return True
    return False


def _check_single(name, remaining, stage, resolved_stages, pos, varmap, depth):
    if name is None:
        return None

    if name == 'eval':
        if not remaining:
            return None
        inner = ' '.join(remaining)
        if _looks_unresolvable(remaining):
            return (
                "eval eines Ausdrucks, der noch eine nicht aufloesbare Kommandosubstitution/Variable "
                "enthaelt -- Inhalt zur Laufzeit nicht vorhersagbar, damit statisch nicht pruefbar. "
                "Blockiert von bash-guard-kill-pattern (Default-Deny fuer unentscheidbare Formen). "
                "eval-Argument: %s" % inner
            )
        return scan_for_danger(inner, varmap, depth + 1)

    if name in cs.SHELL_INTERPRETERS and len(remaining) >= 2 and remaining[0] == '-c':
        inner = remaining[1]
        if _looks_unresolvable([inner]):
            return (
                "%s -c mit einem Ausdruck, der noch eine nicht aufloesbare Kommandosubstitution/"
                "Variable enthaelt -- statisch nicht pruefbar. Blockiert von bash-guard-kill-pattern "
                "(Default-Deny fuer unentscheidbare Formen). -c-Argument: %s" % (name, inner)
            )
        return scan_for_danger(inner, varmap, depth + 1)

    if name == 'tmux':
        if any(t in ('kill-server', 'kill-session') for t in remaining):
            check_text = cs.resolved_stage_text(stage, varmap)
            if is_own_scoped(check_text):
                return None
            # Jeder EXPLIZIT benannte Socket ist sicher, nicht nur die drei bekannten
            # Namen: der Live-Server laeuft auf dem Default-Socket, den `-L <name>`
            # per Definition nicht treffen kann. Die Namensliste war zu eng — sie hat
            # am 2026-07-28 zwei legitime Aufraeum-Befehle auf einem Wegwerf-Socket
            # blockiert und damit zu einem Umweg ueber rohe PIDs gezwungen, der
            # riskanter ist als das, was sie verhindern sollte. `-L default` bleibt
            # gesperrt, das IST der Live-Socket.
            for i, tok in enumerate(remaining):
                sock = None
                if tok == '-L' and i + 1 < len(remaining):
                    sock = remaining[i + 1]
                elif tok.startswith('-L') and len(tok) > 2:
                    sock = tok[2:]
                if sock and sock not in ('default',) and '$' not in sock:
                    return None
            return (
                "tmux kill-server/kill-session ohne eigenen Test-Socket (-L <name>) erkannt -- "
                "toetet direkt eine LIVE-tmux-Session/den ganzen Server, nicht nur eigene "
                "Testprozesse. Vorfall-Klasse: %s. Umweg: eigenen Testsocket verwenden "
                "(tmux -L wbtest ...) oder die Session ueber wb-session-close schliessen, das "
                "die Sicherheitspruefungen selbst mitbringt. Blockiert von bash-guard-kill-pattern. "
                "Teilbefehl: %s"
                % (INCIDENT, cs.stage_text(stage))
            )
        return None

    if name in DANGEROUS_KILL:
        check_text = cs.resolved_stage_text(stage, varmap)
        display_text = cs.stage_text(stage)
        return _pattern_deny(check_text, display_text)

    if name == 'kill':
        resolved_remaining = [cs.resolve_vars(t, varmap) for t in remaining]
        if has_pgrep_reference(resolved_remaining):
            return _pattern_deny(' '.join(resolved_remaining), cs.stage_text(stage))
        if is_bare_pid_kill(resolved_remaining):
            return None
        if not remaining:
            return None
        return (
            "kill ohne erkennbare reine PID-/Signal-Argumente und ohne pgrep/ps-Herkunft im selben "
            "Teilbefehl -- Argument nicht als sicher einstufbar. Blockiert von bash-guard-kill-pattern "
            "(Default-Deny). Teilbefehl: %s" % cs.stage_text(stage)
        )

    if name == 'xargs':
        first_real = None
        for t in remaining:
            if t.startswith('-'):
                continue
            first_real = cs.resolve_vars(t, varmap)
            break
        if first_real in ('kill', 'pkill', 'killall'):
            earlier_pgrep = [s for (n, _, _, s) in resolved_stages[:pos] if n == 'pgrep']
            if earlier_pgrep:
                src_stage = earlier_pgrep[-1]
                check_text = cs.resolved_stage_text(src_stage, varmap)
                display_text = 'pgrep ... | xargs %s (Quelle: %s)' % (
                    first_real, cs.stage_text(src_stage))
                return _pattern_deny(check_text, display_text)
        return None

    return None


def _check_for_loop(statements):
    # Statement-positionsgebunden (stmt[0] == "for"), NICHT eine Textsuche
    # ueber die ganze Kommandozeile -- sonst triggert jede Erwaehnung eines
    # solchen Konstrukts als Text (Doku, Heredoc-Inhalt) genau das False-
    # Positive-Problem, das dieser Hook eigentlich beheben soll (B13).
    for i, stmt in enumerate(statements):
        if not stmt or stmt[0] != 'for':
            continue
        text = cs.stage_text(stmt)
        m = re.match(r'^for\s+([A-Za-z_][A-Za-z0-9_]*)\s+in\s+\$\(([^)]*\bpgrep\b[^)]*)\)', text)
        if not m:
            continue
        loopvar, pgrep_expr = m.group(1), m.group(2)
        for later in statements[i + 1:i + 7]:
            if not later:
                continue
            later_text = cs.stage_text(later)
            if re.search(r'\bkill\s+"?\$\{?' + re.escape(loopvar) + r'\}?"?\b', later_text):
                display = 'for %s in $(%s); do ... kill $%s ...; done' % (
                    loopvar, pgrep_expr.strip(), loopvar)
                return _pattern_deny(pgrep_expr.strip(), display)
    return None


def scan_for_danger(command_text, varmap, depth=0):
    if depth > MAX_DEPTH:
        return (
            "Verschachtelte eval/-c-Aufloesung zu tief (>%d Ebenen) -- als nicht auswertbar geblockt. "
            "Blockiert von bash-guard-kill-pattern." % MAX_DEPTH
        )

    statements = cs.all_statements(command_text)

    for_reason = _check_for_loop(statements)
    if for_reason:
        return for_reason

    for stmt in statements:
        if stmt is None:
            return (
                "Kommando konnte nicht sauber in Teilbefehle zerlegt werden (unausgeglichene "
                "Anfuehrungszeichen) -- als nicht auswertbar geblockt. Blockiert von "
                "bash-guard-kill-pattern (Default-Deny fuer unentscheidbare Formen)."
            )

        pipeline = cs.split_pipeline(stmt)
        resolved_stages = []
        for stage in pipeline:
            name, idx, remaining = cs.resolve_command(stage, varmap)
            resolved_stages.append((name, idx, remaining, stage))

        for pos, (name, idx, remaining, stage) in enumerate(resolved_stages):
            reason = _check_single(name, remaining, stage, resolved_stages, pos, varmap, depth)
            if reason:
                return reason

        if len(resolved_stages) >= 2:
            last_name, _, last_remaining, _ = resolved_stages[-1]
            if last_name in PIPE_TERMINAL_INTERPRETERS:
                extra = [t for t in last_remaining if not t.startswith('-')]
                if not extra:
                    return (
                        "Pipeline endet in einem nackten Interpreter-Aufruf (%s) ohne Skriptdatei -- "
                        "liest das Kommando aus der vorherigen Pipe-Stufe und fuehrt es aus. Statisch "
                        "nicht pruefbar (z.B. base64/xxd/openssl vor bash|sh). Umweg: Skript in eine "
                        "Datei schreiben, die dann geprueft/separat ausgefuehrt wird. Blockiert von "
                        "bash-guard-kill-pattern (Default-Deny fuer unentscheidbare Formen). "
                        "Pipeline: %s" % (last_name, cs.stage_text(stmt))
                    )
    return None


def main():
    try:
        data = json.load(sys.stdin)
    except Exception:
        return 0

    command = (data.get('tool_input') or {}).get('command')
    if not isinstance(command, str) or not command.strip():
        return 0

    if len(command) > MAX_COMMAND_LEN:
        # shlex-based tokenizing scales worse than linearly on one very long
        # token (measured: ~2.6s at 400k chars) -- a real 10MB command would
        # blow past the hook's 5s timeout. Deny fast instead of hanging;
        # this is well above any realistic legitimate command.
        print(json.dumps({
            'hookSpecificOutput': {
                'hookEventName': 'PreToolUse',
                'permissionDecision': 'deny',
                'permissionDecisionReason': (
                    "Kommando ist %d Zeichen lang (Limit %d) -- zu gross, um innerhalb des "
                    "Hook-Zeitbudgets sicher zerlegt zu werden. Blockiert von "
                    "bash-guard-kill-pattern (Default-Deny fuer unentscheidbare/zu grosse Formen)."
                    % (len(command), MAX_COMMAND_LEN)
                ),
            }
        }))
        return 0

    varmap = cs.collect_assignments(cs.all_statements(command))
    reason = scan_for_danger(command, varmap)
    if reason:
        print(json.dumps({
            'hookSpecificOutput': {
                'hookEventName': 'PreToolUse',
                'permissionDecision': 'deny',
                'permissionDecisionReason': reason,
            }
        }))
    return 0


if __name__ == '__main__':
    sys.exit(main())
