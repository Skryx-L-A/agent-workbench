// Spur B (2026-07-29): `wb-state models discover` is being built in parallel by
// another worker (shell/wb-state) — this suite never calls the real binary
// (Regel 2026-07-29: no test may depend on an installed preset or this
// machine's state, e.g. whatever ~/.local/bin/wb-state happens to be today).
// Every test here fully REPLACES PATH with a throwaway bin dir holding a stub
// script, so behaviour is pinned to the Vertrag's documented `--json` shape,
// not to whichever wb-state build happens to be on disk.
import assert from 'node:assert/strict';
import { mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { test } from 'node:test';
import { discoverArgs, discoverModels, summarizeDiscoverReport, type DiscoverReport } from '../src/modelsCli.ts';

test('discoverArgs: no ids means --all, explicit ids replace it, --if-stale/--json placement', () => {
  assert.deepEqual(discoverArgs(), ['models', 'discover', '--all', '--json']);
  assert.deepEqual(discoverArgs(undefined, { ifStale: true }), ['models', 'discover', '--all', '--if-stale', '--json']);
  assert.deepEqual(discoverArgs([]), ['models', 'discover', '--all', '--json'], 'an empty array is treated like "no ids"');
  assert.deepEqual(discoverArgs(['codex']), ['models', 'discover', 'codex', '--json']);
  assert.deepEqual(
    discoverArgs(['codex', 'agy'], { ifStale: true }),
    ['models', 'discover', 'codex', 'agy', '--if-stale', '--json'],
  );
});

// Vertrag Teil 2 §5 (modelDiscoveryAuto): `network: false` is the extension-
// side assumption for skipping provider-level catalog fetches while local
// sources keep running — see modelsCli.ts's DiscoverOpts doc comment for why
// this flag name is not yet confirmed against real shell code.
test('discoverArgs: network:false appends --no-network, network:true/absent does not', () => {
  assert.deepEqual(discoverArgs(undefined, { network: false }), ['models', 'discover', '--all', '--no-network', '--json']);
  assert.deepEqual(
    discoverArgs(undefined, { ifStale: true, network: false }),
    ['models', 'discover', '--all', '--if-stale', '--no-network', '--json'],
  );
  assert.deepEqual(discoverArgs(undefined, { network: true }), ['models', 'discover', '--all', '--json']);
  assert.deepEqual(discoverArgs(), ['models', 'discover', '--all', '--json'], 'network defaults to true (unaffected)');
});

test('summarizeDiscoverReport: added/removed/updated counts, unchanged, per-harness error, empty report', () => {
  const report: DiscoverReport = {
    pi: { added: ['ollama-ornith-9b'], removed: [], updated: ['ollama-ornith-35b'], kept: 10, error: null },
    codex: { added: [], removed: [], updated: [], kept: 4, error: null },
    agy: { added: [], removed: [], updated: [], kept: 0, error: 'agy: Kommando nicht gefunden' },
  };
  assert.equal(
    summarizeDiscoverReport(report),
    'pi: +1, 1 aktualisiert · codex: unverändert · agy: Fehler (agy: Kommando nicht gefunden)',
  );
  assert.equal(summarizeDiscoverReport({}), 'Keine Harnesses geprüft.');
});

function writeStub(bin: string, body: string): void {
  writeFileSync(join(bin, 'wb-state'), `#!/bin/sh\n${body}\n`, { mode: 0o755 });
}

/** Runs `run` with PATH fully replaced by `bin` (never the real machine PATH), restores it after. */
async function withStubPath<T>(bin: string, run: () => Promise<T>): Promise<T> {
  const savedPath = process.env.PATH;
  process.env.PATH = bin;
  try {
    return await run();
  } finally {
    process.env.PATH = savedPath;
  }
}

test('discoverModels: a successful --json run is summarized, args reach the CLI unchanged', async () => {
  const bin = mkdtempSync(join(tmpdir(), 'wb-discover-bin-'));
  const argvLog = join(bin, 'argv.log');
  // `echo` only (a shell builtin) — PATH is fully replaced below, so an
  // external binary like `cat` would not resolve.
  writeStub(bin, `echo "$@" > '${argvLog}'\n`
    + `echo '{"pi":{"added":["ollama-ornith-9b"],"removed":[],"updated":[],"kept":10,"error":null}}'`);
  const result = await withStubPath(bin, () => discoverModels(undefined, { ifStale: true }));
  assert.equal(result.ok, true);
  assert.deepEqual(result.report, {
    pi: { added: ['ollama-ornith-9b'], removed: [], updated: [], kept: 10, error: null },
  });
  assert.equal(result.message, 'pi: +1');
  assert.equal(readFileSync(argvLog, 'utf8').trim(), 'models discover --all --if-stale --json');
});

test('discoverModels: an explicit harness id is passed positionally, never with --all', async () => {
  const bin = mkdtempSync(join(tmpdir(), 'wb-discover-bin-'));
  const argvLog = join(bin, 'argv.log');
  writeStub(bin, `echo "$@" > '${argvLog}'\n`
    + `echo '{"codex":{"added":[],"removed":["gpt-5-old"],"updated":[],"kept":4,"error":null}}'`);
  const result = await withStubPath(bin, () => discoverModels(['codex']));
  assert.equal(result.ok, true);
  assert.equal(result.message, 'codex: -1');
  assert.equal(readFileSync(argvLog, 'utf8').trim(), 'models discover codex --json');
});

test('discoverModels: a non-zero exit is a failure, stderr surfaces (never swallowed)', async () => {
  const bin = mkdtempSync(join(tmpdir(), 'wb-discover-bin-'));
  writeStub(bin, "echo 'wb-state: models.json ist gesperrt' >&2\nexit 1");
  const result = await withStubPath(bin, () => discoverModels());
  assert.equal(result.ok, false);
  assert.equal(result.message, 'wb-state: models.json ist gesperrt');
  assert.equal(result.report, undefined);
});

test('discoverModels: non-JSON stdout falls back to the raw trimmed text, report stays undefined', async () => {
  const bin = mkdtempSync(join(tmpdir(), 'wb-discover-bin-'));
  writeStub(bin, "echo 'wb-state: noch nicht gebaut'");
  const result = await withStubPath(bin, () => discoverModels());
  assert.equal(result.ok, true, 'the CLI itself succeeded — only the JSON parse failed');
  assert.equal(result.message, 'wb-state: noch nicht gebaut');
  assert.equal(result.report, undefined);
});

test('discoverModels: wb-state missing entirely gets the ENOENT hint, never throws', async () => {
  const emptyBin = mkdtempSync(join(tmpdir(), 'wb-discover-empty-bin-'));
  const result = await withStubPath(emptyBin, () => discoverModels());
  assert.equal(result.ok, false);
  assert.match(result.message, /wb-state nicht gefunden/);
});
