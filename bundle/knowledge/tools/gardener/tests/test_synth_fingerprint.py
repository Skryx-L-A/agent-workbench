"""Regel 3 der Synthese: eine erzeugte Seite wird nie ueberschrieben, wenn ein
Mensch sie angefasst hat - erkannt an einem Inhalts-Hash.

Die alte Fassung hashte die exakten Bytes und war bei der Auslieferung kaputt:
alle zehn erzeugten Seiten galten sofort als handbearbeitet, weil ausser dieser
Phase noch andere Schreiber an einer Notiz arbeiten (Basic-Memory stempelt
`permalink:` und normalisiert dabei den Zeilenabschluss, das Schreib-Tor stempelt
`id:`/`schema:`). Die Synthese haette danach keine Seite je wieder aktualisiert.

Die Tests hier gehen deshalb ueber das DATEISYSTEM: schreiben, zurueckleben,
pruefen. Ein Test, der beide Seiten im Speicher gleich falsch rechnet, haette den
Fehler nie gesehen - genau das war passiert.
"""
from __future__ import annotations

from pathlib import Path

from gardener import frontmatter, synth
from gardener.vault import VaultWriter, read_text

# Endet auf ZWEI Zeilenumbrueche, wie die echten erzeugten Seiten. Mit nur einem
# faellt der Fehler nicht auf: der fremde Schreiber normalisiert dann auf genau
# das, was schon dastand, und die kaputte Fassung besteht den Test.
BODY = "\nEine Aussage [[Quelle A]]\n\n## Stand\n\n- Punkt [[Quelle B]]\n\n"
FIELDS = {"title": "thema MOC", "type": "moc", "branch": "30-topics/thema",
          "class": "derived"}


def _page(tmp_path: Path, body: str = BODY) -> Path:
    fields = dict(FIELDS, **{"gardener-content-hash": synth.body_fingerprint(body)})
    writer = VaultWriter(tmp_path)
    path = tmp_path / "30-topics" / "thema" / "MOC.md"
    writer.write(path, frontmatter.render(fields) + body)
    return path


def _still_ours(path: Path) -> bool:
    fm, body = frontmatter.parse(read_text(path))
    return synth.body_fingerprint(body) == fm["gardener-content-hash"]


def test_a_freshly_written_page_is_not_reported_as_hand_edited(tmp_path: Path):
    assert _still_ours(_page(tmp_path))


def test_a_foreign_frontmatter_stamp_does_not_trip_it(tmp_path: Path):
    """Basic-Memory schreibt `permalink:` in jede Notiz. Das ist kein Mensch."""
    path = _page(tmp_path)
    text = read_text(path)
    text = text.replace("class: derived\n",
                        "class: derived\npermalink: main/30-topics/thema/moc\n")
    path.write_text(text, encoding="utf-8")

    assert _still_ours(path)


def test_trailing_whitespace_normalization_does_not_trip_it(tmp_path: Path):
    path = _page(tmp_path)
    text = read_text(path)
    fm_part, body = text.split("\n---\n", 1)
    path.write_text(fm_part + "\n---\n" + body.rstrip("\n") + "\n\n\n",
                    encoding="utf-8")

    assert _still_ours(path)


def test_an_edited_word_does_trip_it(tmp_path: Path):
    path = _page(tmp_path)
    path.write_text(read_text(path).replace("Eine Aussage", "Eine andere Aussage"),
                    encoding="utf-8")

    assert not _still_ours(path)


def test_a_removed_line_does_trip_it(tmp_path: Path):
    path = _page(tmp_path)
    path.write_text(read_text(path).replace("- Punkt [[Quelle B]]\n", ""),
                    encoding="utf-8")

    assert not _still_ours(path)


def test_a_new_line_does_trip_it(tmp_path: Path):
    path = _page(tmp_path)
    path.write_text(read_text(path).replace("## Stand\n",
                                            "## Stand\n\n- Zusatz [[Quelle A]]\n"),
                    encoding="utf-8")

    assert not _still_ours(path)


def test_the_maintain_block_does_not_trip_it(tmp_path: Path):
    """Die maintain-Phase haengt ihren eigenen MOC-Block an DIESELBE Datei, NACH
    der Synthese. Vorher deckte der Fingerabdruck fremden Inhalt mit ab, und nach
    jedem Vollauf galten alle zehn erzeugten Seiten als handbearbeitet - gemessen
    2026-07-29. Ein Schreiber hasht nur seinen eigenen Abschnitt."""
    from gardener.maintain import MOC_END, MOC_START

    path = _page(tmp_path)
    text = read_text(path)
    path.write_text(text.rstrip("\n") + "\n" + MOC_START
                    + "\n- [[Irgendeine Notiz]]\n" + MOC_END + "\n",
                    encoding="utf-8")

    assert _still_ours(path)


def test_an_edit_inside_the_synth_part_still_trips_it_with_a_maintain_block(tmp_path: Path):
    """Die Ausnahme darf nicht mehr freistellen als den fremden Block."""
    from gardener.maintain import MOC_END, MOC_START

    path = _page(tmp_path)
    text = read_text(path).replace("Eine Aussage", "Eine andere Aussage")
    path.write_text(text.rstrip("\n") + "\n" + MOC_START + "\n- [[X]]\n" + MOC_END + "\n",
                    encoding="utf-8")

    assert not _still_ours(path)
