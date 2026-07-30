import re
import shlex

STATEMENT_SEPS = {';', '&', '&&', '||'}
WRAPPER_CMDS = {'command', 'exec', 'builtin', 'nohup', 'sudo', 'env'}
SHELL_INTERPRETERS = {'bash', 'sh', 'zsh', 'dash', 'ksh'}


def _quote_aware_prepass(command):
    # Turns every unquoted, unescaped real newline into ';' (a bash newline
    # ends a statement just like ';' does) and consumes backslash+newline as
    # a line continuation (also only when not inside single quotes, where
    # bash treats backslash as a plain character). Newlines and backslashes
    # INSIDE a quote are left untouched -- a multi-line quoted string (a
    # heredoc-free `bash -c '...\n...'`, for example) must stay one token,
    # not look like several statements or an unbalanced quote.
    out = []
    in_single = in_double = False
    i, n = 0, len(command)
    while i < n:
        ch = command[i]
        if not in_single and ch == '\\' and i + 1 < n:
            nxt = command[i + 1]
            if nxt == '\n':
                i += 2
                continue
            out.append(ch)
            out.append(nxt)
            i += 2
            continue
        if not in_double and ch == "'":
            in_single = not in_single
            out.append(ch)
            i += 1
            continue
        if not in_single and ch == '"':
            in_double = not in_double
            out.append(ch)
            i += 1
            continue
        if not in_single and not in_double and ch == '\n':
            out.append(';')
            i += 1
            continue
        out.append(ch)
        i += 1
    return ''.join(out)


def tokenize(text):
    # '&' is deliberately NOT a punctuation char: shlex would then split
    # inside plain redirections like `2>&1` (no whitespace around the '&'),
    # corrupting an everyday, harmless construct. `&&` still works fine as
    # its own token whenever it appears with the usual surrounding
    # whitespace, which is the only form worth recognizing here.
    lexer = shlex.shlex(text, posix=True, punctuation_chars=';|')
    lexer.whitespace_split = True
    toks = []
    try:
        for t in lexer:
            toks.append(t)
    except ValueError:
        return None
    return toks


def split_statements(tokens):
    stmts, cur = [], []
    for t in tokens:
        if t in STATEMENT_SEPS:
            stmts.append(cur)
            cur = []
        else:
            cur.append(t)
    stmts.append(cur)
    return [s for s in stmts if s]


_REDIRECT_RE = re.compile(r'^(&|[0-9]+)?(>>?|<<?<?|>&|&>>?)')
_REDIRECT_ONLY_RE = re.compile(r'^(&|[0-9]+)?(>>?|<<?<?|>&|&>>?)$')


def strip_redirections(tokens):
    # `2>&1`, `2>/dev/null`, `> out.log` etc. are shell redirections, not
    # arguments the command itself ever sees -- left in, they make an
    # everyday `kill -9 1234 2>&1` look like it has a suspicious extra
    # argument. Handles both the glued form (redirect+target as one token)
    # and the spaced form (operator and target as two tokens).
    out = []
    skip_next = False
    for t in tokens:
        if skip_next:
            skip_next = False
            continue
        if _REDIRECT_RE.match(t):
            if _REDIRECT_ONLY_RE.match(t):
                skip_next = True
            continue
        out.append(t)
    return out


def split_pipeline(tokens):
    stages, cur = [], []
    for t in tokens:
        if t == '|':
            stages.append(strip_redirections(cur))
            cur = []
        else:
            cur.append(t)
    stages.append(strip_redirections(cur))
    return [s for s in stages if s]


def all_statements(command):
    # [None] marks a command that could not be tokenized at all (a genuinely
    # unbalanced quote) -- callers must treat that as unresolvable, not as
    # "nothing found".
    toks = tokenize(_quote_aware_prepass(command))
    if toks is None:
        return [None]
    return split_statements(toks)


def collect_assignments(statements):
    varmap = {}
    for stmt in statements:
        if not stmt:
            continue
        for tok in stmt:
            m = re.match(r'^([A-Za-z_][A-Za-z0-9_]*)=(.*)$', tok)
            if not m:
                continue
            varmap[m.group(1)] = m.group(2)
    return varmap


def resolve_vars(word, varmap, depth=0):
    if depth > 6:
        return word

    def repl(m):
        return varmap.get(m.group(1), m.group(0))

    new = re.sub(r'\$\{?([A-Za-z_][A-Za-z0-9_]*)\}?', repl, word)
    return resolve_vars(new, varmap, depth + 1) if new != word else new


def var_name_if_bare_ref(token):
    m = re.match(r'^"?\$\{?([A-Za-z_][A-Za-z0-9_]*)\}?"?$', token)
    return m.group(1) if m else None


def resolve_command(stage_tokens, varmap):
    # returns (resolved_name, index_of_command_token, remaining_raw_tokens) or
    # (None, len(stage_tokens), []) if the stage is only assignments/empty.
    i, n = 0, len(stage_tokens)
    skip_flags = False
    while i < n:
        raw = stage_tokens[i]
        if re.match(r'^[A-Za-z_][A-Za-z0-9_]*=', raw):
            i += 1
            continue
        if skip_flags and raw.startswith('-'):
            i += 1
            continue
        word = resolve_vars(raw, varmap)
        name = word.split('/')[-1]
        if name in WRAPPER_CMDS:
            skip_flags = True
            i += 1
            continue
        return name, i, stage_tokens[i + 1:]
    return None, i, []


def stage_text(stage_tokens):
    return ' '.join(stage_tokens)


def resolved_stage_text(stage_tokens, varmap):
    return ' '.join(resolve_vars(t, varmap) for t in stage_tokens)
