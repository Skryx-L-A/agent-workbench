from gardener import frontmatter
from gardener.vault import load_notes, parse_note

DOUBLE = ("---\npermalink: main/10-global/x\n---\n\n"
          "---\ntitle: X Note\ntype: decision\naliases: [XN, ix]\n---\n\nbody here\n")
SINGLE = "---\ntitle: Y\ntype: note\n---\n\nbody\n"


def test_split_and_merge_double_block():
    blocks, body = frontmatter.split_blocks(DOUBLE)
    assert len(blocks) == 2
    assert body.strip() == "body here"
    fields = frontmatter.parse_fields(blocks)
    assert fields["title"] == "X Note"
    assert fields["type"] == "decision"
    assert fields["permalink"] == "main/10-global/x"
    assert fields["aliases"] == ["XN", "ix"]


def test_single_block_is_left_untouched():
    assert frontmatter.ensure_single(SINGLE) == SINGLE


def test_ensure_single_collapses_double_block():
    out = frontmatter.ensure_single(DOUBLE)
    blocks, body = frontmatter.split_blocks(out)
    assert len(blocks) == 1
    assert "title: X Note" in blocks[0]
    assert "permalink: main/10-global/x" in blocks[0]
    assert body.strip() == "body here"


def test_ensure_single_adds_frontmatter_when_missing():
    out = frontmatter.ensure_single("just a body\n", fallback={"title": "T", "type": "note"})
    blocks, body = frontmatter.split_blocks(out)
    assert len(blocks) == 1
    assert body.strip() == "just a body"


def test_dash_list_and_inline_comments():
    text = ("---\ntitle: Z        # a comment\n"
            "aliases:\n  - alpha\n  - beta\n"
            "review-after: 2026-01   # ttl\n---\n\nbody\n")
    fields, _ = frontmatter.parse(text)
    assert fields["title"] == "Z"
    assert fields["aliases"] == ["alpha", "beta"]
    assert fields["review-after"] == "2026-01"


def test_parse_note_reads_double_block(tmp_vault):
    p = tmp_vault / "10-global" / "double.md"
    p.write_text(DOUBLE)
    n = parse_note(tmp_vault, p)
    assert n.title == "X Note"
    assert n.ntype == "decision"
    assert n.aliases == ["XN", "ix"]
    assert n.alias_keys == {"xn", "ix"}


def test_aliases_reach_embedding_text_and_resolver(tmp_vault):
    p = tmp_vault / "10-global" / "double.md"
    p.write_text(DOUBLE)
    notes = load_notes(tmp_vault)
    aliased = next(n for n in notes if n.rel == "10-global/double.md")
    assert "aliases: XN, ix" in aliased.embed_text
    plain = next(n for n in notes if n.rel == "10-global/beta.md")
    assert plain.embed_text == f"{plain.title}\n\n{plain.text}"  # cache stays valid
    from gardener.vault import build_resolver
    assert build_resolver(notes)["xn"].rel == "10-global/double.md"


def test_a_wrapped_scalar_keeps_its_tail():
    """YAML bricht lange Werte auf eingerueckte Folgezeilen um und fuegt sie mit
    einem Leerzeichen zusammen. Sie wegzuwerfen schnitt jedes umgebrochene Feld
    stumm ab - gemessen 2026-07-29 an 48 Notizen im Vault."""
    text = ("---\n"
            "title: Incident 2026-07-29 - Personendaten im Repo, weil das Tor eine\n"
            "  Ausnahme hatte\n"
            "type: incident\n"
            "---\n\nRumpf\n")
    fields, body = frontmatter.parse(text)
    assert fields["title"] == ("Incident 2026-07-29 - Personendaten im Repo, "
                               "weil das Tor eine Ausnahme hatte")
    assert fields["type"] == "incident"
    assert body.strip() == "Rumpf"


def test_a_wrapped_scalar_over_three_lines_joins_all_of_them():
    text = "---\ndescription: eins\n  zwei\n  drei\nkey: wert\n---\n\nX\n"
    fields, _ = frontmatter.parse(text)
    assert fields["description"] == "eins zwei drei"
    assert fields["key"] == "wert"


def test_a_list_is_still_a_list():
    text = "---\naliases:\n  - Erster\n  - Zweiter\ntitle: T\n---\n\nX\n"
    fields, _ = frontmatter.parse(text)
    assert fields["aliases"] == ["Erster", "Zweiter"]
    assert fields["title"] == "T"
