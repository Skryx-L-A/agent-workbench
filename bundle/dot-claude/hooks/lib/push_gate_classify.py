import json
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import cmdshell as cs

MAX_DEPTH = 5
MAX_COMMAND_LEN = 200_000


def git_subcommand(remaining, varmap):
    i, n = 0, len(remaining)
    while i < n:
        t = cs.resolve_vars(remaining[i], varmap)
        if t in ('-C', '-c'):
            i += 2
            continue
        if t.startswith('--git-dir=') or t.startswith('--work-tree='):
            i += 1
            continue
        if t.startswith('-'):
            i += 1
            continue
        return t
    return None


def _looks_unresolvable(tokens):
    for t in tokens:
        if '$(' in t or '`' in t:
            return True
        if re.search(r'\$\{?[A-Za-z_][A-Za-z0-9_]*\}?', t):
            return True
    return False


def finds_push(command_text, varmap, depth=0):
    if depth > MAX_DEPTH:
        return True

    statements = cs.all_statements(command_text)
    for stmt in statements:
        if stmt is None:
            return True
        for stage in cs.split_pipeline(stmt):
            name, idx, remaining = cs.resolve_command(stage, varmap)
            if name is None:
                continue

            if name == 'eval':
                if not remaining:
                    continue
                inner = ' '.join(remaining)
                if _looks_unresolvable(remaining):
                    return True
                if finds_push(inner, varmap, depth + 1):
                    return True
                continue

            if name in cs.SHELL_INTERPRETERS and len(remaining) >= 2 and remaining[0] == '-c':
                inner = remaining[1]
                if _looks_unresolvable([inner]):
                    return True
                if finds_push(inner, varmap, depth + 1):
                    return True
                continue

            if name == 'git':
                sub = git_subcommand(remaining, varmap)
                if sub == 'push':
                    return True
                continue

            if name == 'gh':
                resolved = [cs.resolve_vars(t, varmap) for t in remaining]
                stripped = [t for t in resolved if t and not t.startswith('-')]
                if len(stripped) >= 2 and stripped[0] == 'pr' and stripped[1] == 'create':
                    return True
                continue

    return False


def main():
    try:
        data = json.load(sys.stdin)
    except Exception:
        return 0

    command = (data.get('tool_input') or {}).get('command')
    if not isinstance(command, str) or not command.strip():
        return 0

    if len(command) > MAX_COMMAND_LEN:
        # Same shlex scaling issue as bash-guard-kill-pattern (see there) --
        # too large to tokenize within the hook's time budget. Report as a
        # hit so the role check downstream still gets a chance to gate it,
        # instead of silently waving an unanalyzable command through.
        print('1')
        return 0

    varmap = cs.collect_assignments(cs.all_statements(command))
    print('1' if finds_push(command, varmap) else '0')
    return 0


if __name__ == '__main__':
    sys.exit(main())
