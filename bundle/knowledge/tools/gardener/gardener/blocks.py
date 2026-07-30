"""Safe rewrite of gardener-managed marker blocks (`<!-- gardener:*:start/end -->`).

A `re.sub(START.*?END)` silently corrupts a file whose markers are unbalanced: a
leftover START (truncated file, merge conflict, hand edit) swallows everything up
to the NEXT END. Replacement is therefore index-based, and a file whose markers
are not exactly one well-formed pair is refused instead of rewritten.
"""
from __future__ import annotations


def has_block(text: str, start: str, end: str) -> bool:
    return (text.count(start) == 1 and text.count(end) == 1
            and text.index(start) < text.index(end))


def replace_block(text: str, start: str, end: str, block: str) -> tuple[str, bool]:
    """(new_text, ok).

    ok=False means the markers are malformed - the caller must NOT write.
    With no markers at all the text is returned unchanged (ok=True): appending
    the block is the caller's business, since only it knows the heading.
    """
    n_start, n_end = text.count(start), text.count(end)
    if n_start == 0 and n_end == 0:
        return text, True
    if not has_block(text, start, end):
        return text, False
    i, j = text.index(start), text.index(end)
    return text[:i] + block + text[j + len(end):], True
