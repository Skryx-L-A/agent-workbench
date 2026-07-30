"""brain: CLI over the Knowledge vault + Gardener.

Usage:
    brain search <query> [-k N] [--json]
    brain gardener run [--phase linking|consolidate|maintain|synth|all] [--dry-run]
                       [--audit] [--topic T] [--min-notes N]
    brain gardener status [--json]
    brain sidecar scan [--path P] [--json]
    brain sidecar generate [--path P] [--force] [--model M] [--dry-run]
    brain sidecar check [--path P]
    brain contradict [--note P] [--since ISO-DATE] [--all] [--write] [--json] [-k N]
    brain contradict --resolve <id> --by <wer> --why <text> [--rule N] [--write]
    brain ingest <quelle> [--branch P] [--title T] [--source URL] [--write] [--json]
                          [--no-contradict]
    brain stats [--json]
"""
from __future__ import annotations

import argparse
import os
import datetime as dt
import json
import sys
from pathlib import Path

from gardener import config
from gardener import contradict as contradict_mod
from gardener import sidecar as sidecar_mod
from gardener.linking import embed_notes
from gardener.ollama import OllamaClient, OllamaError, OllamaUnavailable
from gardener.runtime import Deadline
from gardener.store import Store
from gardener.vault import VaultWriter, load_notes

from . import gardener_wrap, ingest as ingest_mod, search as search_mod, stats as stats_mod

DEFAULT_VAULT = config.DEFAULT_VAULT


def cmd_search(args) -> int:
    vault = Path(args.vault)
    hits, used_fallback = search_mod.search(vault, args.query, args.k)
    if args.json:
        print(json.dumps({
            "query": args.query,
            "fallback": used_fallback,
            "hits": [h.__dict__ for h in hits],
        }, ensure_ascii=False, indent=2))
        return 0
    if used_fallback:
        print("Ollama nicht erreichbar - rg-Volltextsuche als Fallback:\n")
    if not hits:
        print("Keine Treffer.")
        return 0
    for h in hits:
        flag = "[!] " if h.contradiction else ""
        print(f"{flag}{h.rel}")
        if h.title:
            tag = f" [{h.match}]" if h.match else ""
            print(f"  Titel: {h.title}  (score {h.score:.3f}){tag}")
        if h.contradiction:
            print("  [!] traegt einen offenen Widerspruch - siehe review-queue.md")
        if h.snippet:
            print(f"  {h.snippet}")
        print()
    return 0


def cmd_contradict(args) -> int:
    if getattr(args, "resolve", None):
        # Aufloesen ist kein Scan: kein Modell, keine Nachbarsuche, nur der
        # Eintrag wer/wann/warum plus das Umschreiben der beiden Marker.
        if not args.why:
            print("FEHLER: --why fehlt - eine Aufloesung ohne Begruendung ist "
                  "im Protokoll wertlos.", file=sys.stderr)
            return 2
        vault = Path(args.vault)
        store = contradict_mod.ContradictionStore(vault / contradict_mod.config.CONTRADICTIONS_FILE
                                                  if not str(contradict_mod.config.CONTRADICTIONS_FILE).startswith("/")
                                                  else contradict_mod.config.CONTRADICTIONS_FILE)
        if store.get(args.resolve) is None:
            print(f"FEHLER: kein Befund mit der id {args.resolve!r}.", file=sys.stderr)
            return 1
        writer = VaultWriter(vault, dry_run=not args.write)
        finding = contradict_mod.resolve_finding(
            store, args.resolve, by=args.by, why=args.why, rule=args.rule,
            vault=vault, writer=writer)
        if args.write:
            store.save()
        # Die Queue mitschreiben, sonst steht ein aufgeloester Befund dort fuer
        # immer weiter (gemessen 2026-07-29: Marker sagte `resolved`, die Queue
        # meldete ihn unveraendert als offen). Der Schreiber fasst nur seinen
        # eigenen markierten Abschnitt an, Fremdeintraege bleiben stehen.
        contradict_mod.write_review_queue(vault, store.open_findings(),
                                          dry_run=not args.write)
        out = {"resolved": args.resolve, "status": finding.get("status"),
               "by": args.by, "why": args.why, "rule": args.rule,
               "written": bool(args.write)}
        print(json.dumps(out, ensure_ascii=False, indent=2) if args.json
              else f"Befund {args.resolve}: {finding.get('status')}"
                   + ("" if args.write else "  (Trockenlauf - nichts geschrieben)"))
        return 0

    vault = Path(args.vault)
    client = OllamaClient()
    try:
        big = client.big_model_loaded()
    except OllamaError as e:
        print(f"Ollama nicht erreichbar: {e}")
        return 2
    if big:
        print(f"48-GB-Regel: {big} ist geladen (>15 GB) - Lauf verschoben.")
        return 3

    notes = load_notes(vault)
    by_rel = {n.rel: n for n in notes}

    if args.note:
        target = contradict_mod.resolve_note_arg(vault, args.note, notes)
        if target is None:
            print(f"Notiz nicht gefunden: {args.note}")
            return 1
        to_check = [target]
    elif args.all:
        to_check = notes
    elif args.since:
        to_check = contradict_mod.changed_since(notes, dt.datetime.fromisoformat(args.since))
    else:
        cutoff = contradict_mod.load_last_run(vault) or dt.datetime.fromtimestamp(0)
        to_check = contradict_mod.changed_since(notes, cutoff)

    # Ein Vollscan ueber den ganzen Korpus dauert Stunden (gemessen: ~19 s je
    # Paar, 210 Notizen mal 5 Nachbarn). Das 45-Minuten-Budget ist fuer den
    # taeglichen --since-Lauf richtig und fuer einen unbeaufsichtigten Vollscan
    # zu knapp - deshalb ueberschreibbar, statt den Lauf jedes Mal auf einem
    # Sechstel abzuschneiden.
    budget = config.RUN_BUDGET_SECONDS
    env_budget = os.environ.get("BRAIN_CONTRADICT_BUDGET_SECONDS")
    if env_budget:
        try:
            budget = max(60, int(env_budget))
        except ValueError:
            pass
    deadline = Deadline(budget)
    embed_store = Store(config.STATE_DIR / "gardener.db", read_only=not args.write)
    try:
        vectors = embed_notes(notes, embed_store, client, deadline)
    except OllamaUnavailable as e:
        print(f"Ollama nicht erreichbar: {e}")
        return 2
    finally:
        embed_store.close()

    cstore = contradict_mod.ContradictionStore(vault / config.CONTRADICTIONS_FILE)
    result = contradict_mod.run_contradict(
        [n for n in to_check if n.rel in vectors], notes, vectors, client, cstore,
        top_k=args.k, deadline=deadline)

    writer = VaultWriter(vault, dry_run=not args.write)
    for finding in result.findings:
        a, b = by_rel.get(finding["note_a"]["rel"]), by_rel.get(finding["note_b"]["rel"])
        if a is not None and b is not None:
            contradict_mod.apply_markers(writer, a, b, finding)

    cstore.save(dry_run=not args.write)
    contradict_mod.write_review_queue(vault, cstore.open_findings(), dry_run=not args.write)
    contradict_mod.save_last_run(vault, dt.datetime.now(), dry_run=not args.write)

    if args.json:
        print(json.dumps({
            "checked_notes": len(to_check),
            "pairs_checked": result.pairs_checked,
            "found": len(result.findings),
            "compatible": result.compatible,
            "below_threshold": result.below_threshold,
            "hallucinated": result.hallucinated,
            "judge_failed": result.judge_failed,
            "dry_run": not args.write,
            "findings": result.findings,
        }, ensure_ascii=False, indent=2))
        return 0

    print(f"geprueft: {len(to_check)} Notiz(en), {result.pairs_checked} Paar(e)")
    print(f"Befunde: {len(result.findings)}  "
          f"(kompatibel: {result.compatible}, unter Schwelle: {result.below_threshold}, "
          f"halluziniert verworfen: {result.hallucinated})")
    for f in result.findings:
        tag = "ESKALIERT" if f["status"] == "escalated" else f["verdict"]
        print(f"  [{tag}] {f['note_a']['title']} <-> {f['note_b']['title']} "
              f"(Konfidenz {f['confidence']:.2f}, id {f['id']})")
    if not args.write:
        print("(dry-run: keine Schreibzugriffe - siehe --write)")
    return 0


def cmd_ingest(args) -> int:
    vault = Path(args.vault)
    client = OllamaClient()
    outcome = ingest_mod.run_ingest(
        vault, args.quelle, branch=args.branch, title_override=args.title,
        origin=args.origin,
        write=args.write, check_contradict=not args.no_contradict, client=client)
    if args.json:
        print(json.dumps(outcome, ensure_ascii=False, indent=2))
        return 0
    if outcome["duplicate"]:
        print(f"bereits eingelesen: {outcome['note']} ({outcome['title']})")
        return 0
    tag = " (dry-run)" if not args.write else ""
    print(f"Notiz: {outcome['note']}{tag}")
    print(f"extrahiert: {outcome['extracted']} ({outcome['extractor'] or 'kein Extraktor'})")
    if outcome["extraction_error"]:
        print(f"Hinweis: {outcome['extraction_error']}")
    if outcome["related_notes"]:
        print("verwandte Notizen:")
        for r in outcome["related_notes"]:
            print(f"  - {r['title']} ({r['rel']})")
    cr = outcome["contradictions"]
    if cr["checked"]:
        print(f"Widerspruchspruefung: {cr['found']} Befund(e) von {cr['pairs_checked']} Paar(en)")
    elif cr["skipped_reason"]:
        print(f"Widerspruchspruefung uebersprungen: {cr['skipped_reason']}")
    if not args.write:
        print("(dry-run: keine Schreibzugriffe - siehe --write)")
    return 0


def cmd_gardener_run(args) -> int:
    vault = Path(args.vault)
    return gardener_wrap.run(vault, phase=args.phase, dry_run=args.dry_run,
                             audit=args.audit, verbose=args.verbose,
                             topic=getattr(args, "topic", None),
                             min_notes=getattr(args, "min_notes", None))


def cmd_gardener_status(args) -> int:
    vault = Path(args.vault)
    result = gardener_wrap.status(vault)
    if args.json:
        print(json.dumps(result, ensure_ascii=False, indent=2))
        return 0
    lock = result["lock"]
    print(f"laeuft: {lock['running']}")
    if lock.get("lock_age_seconds") is not None:
        print(f"lock-alter: {lock['lock_age_seconds']:.0f}s ({lock['lock_path']})")
    last = result["last_run"]
    if last:
        print(f"letzter lauf: {last['finished']} - {last['summary']}")
        if last.get("conflicts"):
            print(f"  nicht ueberschrieben (waehrend des Laufs geaendert): "
                  f"{last['conflicts']}")
    else:
        print("letzter lauf: keiner")
    report = result["latest_report"]
    print(f"letzter report: {report['path']} ({report['modified']})" if report
          else "letzter report: keiner")
    log = result["latest_log"]
    print(f"letztes log: {log['path']} ({log['modified']})" if log
          else "letztes log: keins")
    return 0


def cmd_sidecar_scan(args) -> int:
    vault = Path(args.vault)
    entries = sidecar_mod.scan(vault, path=args.path)
    by_status: dict[str, int] = {}
    for e in entries:
        by_status[e.status] = by_status.get(e.status, 0) + 1
    if args.json:
        print(json.dumps({"entries": [e.__dict__ for e in entries],
                          "by_status": by_status}, ensure_ascii=False, indent=2))
        return 0
    print(f"Assets gesamt: {len(entries)}")
    for status, count in sorted(by_status.items()):
        print(f"  {status}: {count}")
    for e in entries:
        if e.status in ("missing", "stale"):
            print(f"- [{e.status}] {e.rel}")
    return 0


def cmd_sidecar_generate(args) -> int:
    vault = Path(args.vault)
    client = OllamaClient(judge_model=args.model) if args.model else OllamaClient()
    writer = VaultWriter(vault, dry_run=args.dry_run)
    result = sidecar_mod.generate(vault, writer, client, path=args.path,
                                  force=args.force)
    print(f"neu erzeugt: {len(result.generated)}")
    print(f"aktualisiert: {len(result.updated)}")
    print(f"Legacy-Stubs ergaenzt: {len(result.legacy_enriched)}")
    print(f"ohne Beschreibung (Metadaten-only): {len(result.metadata_only)}")
    if result.external:
        print(f"extern (> {config.SIDECAR_EXTERNAL_MB} MB, nicht committen - "
              f".gitignore/.gitattributes pruefen): {len(result.external)}")
        for rel in result.external:
            print(f"  - {rel}")
    if result.skipped_human_edited:
        print(f"human-edited uebersprungen: {len(result.skipped_human_edited)}")
    if result.skipped_malformed:
        print(f"malformte Marker uebersprungen: {len(result.skipped_malformed)}")
    if args.dry_run:
        print("(dry-run: keine Schreibzugriffe)")
    return 0


def cmd_sidecar_check(args) -> int:
    vault = Path(args.vault)
    entries = sidecar_mod.scan(vault, path=args.path)
    bad = [e for e in entries if e.status in ("missing", "stale")]
    if bad:
        for e in bad:
            print(f"[{e.status}] {e.rel}")
        print(f"{len(bad)} Sidecar(s) fehlen oder sind veraltet.")
        return 1
    print("alle Sidecars aktuell.")
    return 0


def cmd_stats(args) -> int:
    vault = Path(args.vault)
    result = stats_mod.collect(vault)
    if args.json:
        print(json.dumps(result, ensure_ascii=False, indent=2))
        return 0
    print(f"Notes gesamt: {result['notes_total']}")
    print("Pro Branch:")
    for b, n in result["notes_per_branch"].items():
        print(f"  {b}: {n}")
    print(f"  davon in Gardeners Link-Korpus: {result['link_corpus_notes_total']} "
          "(schliesst MOC.md/DECISIONS.md/review-queue.md aus, siehe stats.collect docstring)")
    print(f"Wikilinks gesamt: {result['wikilinks_total']}")
    # Getrennt, weil nur die erste Zahl handlungsfaehig macht: Quellnotizen
    # (00-sources/, sessions/) werden ueber die Suche gefunden, nicht ueber Links.
    print(f"Orphans (Wissen, verlinkenswert): {len(result['orphans_knowledge'])}")
    for rel in result["orphans_knowledge"]:
        print(f"  {rel}")
    print(f"Orphans (Quellen, erwartbar): {len(result['orphans_source'])}")
    print(f"Assets: {result['assets_total']}")
    mb = result["lfs_object_size_bytes"] / (1024 * 1024)
    print(f"LFS-Objekte lokal: {mb:.1f} MB")
    print(f"Letztes Backup-Bundle: {result['last_backup_bundle'] or 'keins'}")
    return 0


def build_parser() -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(prog="brain", description=__doc__,
                                formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--vault", default=str(DEFAULT_VAULT))
    sub = p.add_subparsers(dest="cmd", required=True)

    p_search = sub.add_parser("search", help="semantic search over the vault")
    p_search.add_argument("query")
    p_search.add_argument("-k", type=int, default=5)
    p_search.add_argument("--json", action="store_true")
    p_search.set_defaults(func=cmd_search)

    p_gardener = sub.add_parser("gardener", help="gardener control")
    gsub = p_gardener.add_subparsers(dest="gcmd", required=True)

    p_grun = gsub.add_parser("run", help="run gardener in the foreground")
    p_grun.add_argument("--phase", choices=list(gardener_wrap.VALID_PHASES),
                        default="all")
    p_grun.add_argument("--dry-run", action="store_true")
    p_grun.add_argument("--audit", action="store_true")
    p_grun.add_argument("--verbose", action="store_true")
    p_grun.add_argument("--topic", default=None,
                        help="nur diese eine Themenseite (Phase synth)")
    p_grun.add_argument("--min-notes", type=int, default=None,
                        help="Mindestzahl Quellnotizen je Themenseite (Phase synth)")
    p_grun.set_defaults(func=cmd_gardener_run)

    p_gstatus = gsub.add_parser("status", help="last gardener run / lock status")
    p_gstatus.add_argument("--json", action="store_true")
    p_gstatus.set_defaults(func=cmd_gardener_status)

    p_sidecar = sub.add_parser("sidecar", help="asset sidecar layer (per-file .md description)")
    ssub = p_sidecar.add_subparsers(dest="scmd", required=True)

    p_sscan = ssub.add_parser("scan", help="report missing/stale sidecars (read-only)")
    p_sscan.add_argument("--path", default=None)
    p_sscan.add_argument("--json", action="store_true")
    p_sscan.set_defaults(func=cmd_sidecar_scan)

    p_sgen = ssub.add_parser("generate", help="write missing/stale sidecars")
    p_sgen.add_argument("--path", default=None)
    p_sgen.add_argument("--force", action="store_true")
    p_sgen.add_argument("--model", default=None)
    p_sgen.add_argument("--dry-run", action="store_true")
    p_sgen.set_defaults(func=cmd_sidecar_generate)

    p_scheck = ssub.add_parser(
        "check", help="exit != 0 if sidecars are missing/stale (pre-commit gate)")
    p_scheck.add_argument("--path", default=None)
    p_scheck.set_defaults(func=cmd_sidecar_check)

    p_contradict = sub.add_parser(
        "contradict", help="find knowledge contradictions between notes (not write conflicts)")
    p_contradict.add_argument("--note", default=None,
                              help="check only this note (vault-relative or absolute path)")
    p_contradict.add_argument("--since", default=None,
                              help="check notes changed since this ISO date/datetime")
    p_contradict.add_argument("--all", action="store_true",
                              help="check every note, ignoring the last-run timestamp")
    p_contradict.add_argument("--write", action="store_true",
                              help="persist findings + marker blocks (default: dry-run)")
    p_contradict.add_argument("--json", action="store_true")
    p_contradict.add_argument("-k", type=int, default=config.CONTRADICT_TOP_K,
                              help="semantic neighbors checked per note")
    # Aufloesen: kein Scan, nur der Eintrag wer/wann/warum plus das Umschreiben
    # der beiden Marker. Ohne dieses Kommando bliebe jeder Befund fuer immer
    # offen - genau der Zustand, den 10-global/contradiction-rules.md vermeiden will.
    p_contradict.add_argument("--resolve", default=None, metavar="ID",
                              help="Befund als aufgeloest eintragen")
    p_contradict.add_argument("--by", default="orchestrator",
                              help="wer aufgeloest hat")
    p_contradict.add_argument("--why", default="",
                              help="warum - ein Satz, kommt ins Protokoll")
    p_contradict.add_argument("--rule", default="",
                              help="welche Regel aus 10-global/contradiction-rules.md griff")
    p_contradict.set_defaults(func=cmd_contradict)

    p_ingest = sub.add_parser(
        "ingest", help="pull external material (file/URL/YouTube/stdin) into 00-sources/")
    p_ingest.add_argument("quelle", help="path, URL, or - for stdin")
    p_ingest.add_argument("--branch", default=None,
                          help="context hint recorded in frontmatter (default: 00-sources)")
    p_ingest.add_argument("--title", default=None)
    p_ingest.add_argument("--source", dest="origin", default=None,
                          help="echte Herkunft, wenn gelesen wird aus einer "
                               "Kopie (heruntergeladenes Transkript, entpacktes "
                               "Archiv). Wird als source: vermerkt.")
    p_ingest.add_argument("--write", action="store_true",
                          help="persist the note + links + log (default: dry-run)")
    p_ingest.add_argument("--json", action="store_true")
    p_ingest.add_argument("--no-contradict", action="store_true",
                          help="skip the post-write contradiction check")
    p_ingest.set_defaults(func=cmd_ingest)

    p_stats = sub.add_parser("stats", help="vault health stats")
    p_stats.add_argument("--json", action="store_true")
    p_stats.set_defaults(func=cmd_stats)

    return p


def main(argv=None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        return args.func(args)
    except KeyboardInterrupt:
        return 130


if __name__ == "__main__":
    sys.exit(main())
