import assert from 'node:assert/strict';
import { mkdtempSync, readFileSync, symlinkSync } from 'node:fs';
import { mkdir } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { test } from 'node:test';
import {
  addArgs,
  addHarness,
  addModel,
  addProvider,
  checkModelArgs,
  describeCliError,
  harnessBinaryPresent,
  probeHarnessArgs,
  removeArgs,
  removeEntry,
  setField,
  setFieldArgs,
} from '../src/modelsCli.ts';
import type { RegistryModel } from '../src/models.ts';

test('addArgs builds the wb-state models add line for each kind, JSON always positional (never stdin)', () => {
  assert.deepEqual(addArgs('model', '{"id":"x"}'), ['models', 'add', '--kind', 'model', '{"id":"x"}']);
  assert.deepEqual(addArgs('harness', '{"id":"y"}'), ['models', 'add', '--kind', 'harness', '{"id":"y"}']);
  assert.deepEqual(addArgs('provider', '{"id":"z"}'), ['models', 'add', '--kind', 'provider', '{"id":"z"}']);
});

test('setFieldArgs names kind, id, field and JSON-encoded value, all positional', () => {
  assert.deepEqual(
    setFieldArgs('model', 'gpt-5-codex', 'enabled', true),
    ['models', 'set', '--kind', 'model', 'gpt-5-codex', 'enabled', 'true'],
  );
  assert.deepEqual(
    setFieldArgs('model', 'gpt-5-codex', 'label', 'GPT-5 Codex'),
    ['models', 'set', '--kind', 'model', 'gpt-5-codex', 'label', '"GPT-5 Codex"'],
  );
});

test('removeArgs / checkModelArgs / probeHarnessArgs', () => {
  assert.deepEqual(removeArgs('harness', 'codex'), ['models', 'remove', '--kind', 'harness', 'codex']);
  assert.deepEqual(checkModelArgs('gpt-5-codex'), ['models', 'check', 'gpt-5-codex']);
  assert.deepEqual(probeHarnessArgs('codex'), ['codex']);
});

test('describeCliError: ENOENT gets the caller-provided missing-command hint', () => {
  assert.equal(
    describeCliError({ code: 'ENOENT' }, 'wb-state nicht gefunden.'),
    'wb-state nicht gefunden.',
  );
});

test('describeCliError: a real failure surfaces stderr, never a swallowed generic message', () => {
  assert.equal(
    describeCliError({ stderr: 'models.json ist gesperrt\n' }, 'hint'),
    'models.json ist gesperrt',
  );
});

test('describeCliError: falls back to stdout, then to a generic line, but never throws', () => {
  assert.equal(describeCliError({ stdout: 'unbekanntes Feld "foo"' }, 'hint'), 'unbekanntes Feld "foo"');
  assert.equal(describeCliError({ message: 'timeout' }, 'hint'), 'timeout');
  assert.equal(describeCliError({}, 'hint'), 'Kommando fehlgeschlagen (keine weitere Ausgabe).');
});

// Reviewer-Befund K1/M9 (2026-07-28): the builder tests above only checked what
// this module returns, never whether `wb-state` accepts it — 217 green tests
// coexisted with a write path that failed on every real call. This block runs
// the actual `shell/wb-state` script against a throwaway HOME/models.json, the
// contract test the reviewer asked for. Never the real configuration: own HOME
// (mktemp), own PATH entry, TMUX/TMUX_PANE unset so the ACTOR lookup inside
// wb-state cannot reach a live tmux server.
delete process.env.TMUX;
delete process.env.TMUX_PANE;
const home = mkdtempSync(join(tmpdir(), 'wb-modelscli-home-'));
const bin = mkdtempSync(join(tmpdir(), 'wb-modelscli-bin-'));
// `npm test` (package.json) always runs with cwd = extension/, the only
// documented way this suite is invoked — no root package.json wraps it.
const repoRoot = join(process.cwd(), '..');
symlinkSync(join(repoRoot, 'shell', 'wb-state'), join(bin, 'wb-state'));
process.env.HOME = home;
process.env.PATH = `${bin}:${process.env.PATH ?? ''}`;
const modelsFile = join(home, '.claude', 'workbench', 'models.json');

function readModelsFile(): any {
  return JSON.parse(readFileSync(modelsFile, 'utf8'));
}

const testModel: RegistryModel = {
  id: 'contract-test-model',
  label: 'Contract Test Model',
  harness: 'claude',
  provider: 'claude-subscription',
  modelRef: 'claude-sonnet-5',
  roles: ['worker'],
  efforts: ['medium'],
  maxEffort: 'medium',
  defaultEffort: 'medium',
  enabled: true,
};

test('addModel against the real wb-state CLI: write succeeds and the entry lands in models.json', async () => {
  await mkdir(dirname(modelsFile), { recursive: true });
  const result = await addModel(testModel);
  assert.equal(result.ok, true, `expected ok, got: ${result.message}`);
  const written = readModelsFile();
  const entry = written.models.find((m: any) => m.id === 'contract-test-model');
  assert.ok(entry, 'model missing from models.json after addModel');
  assert.equal(entry.harness, 'claude');
});

test('setField against the real wb-state CLI: the id/field/value positionals are understood', async () => {
  const result = await setField('model', 'contract-test-model', 'enabled', false);
  assert.equal(result.ok, true, `expected ok, got: ${result.message}`);
  const entry = readModelsFile().models.find((m: any) => m.id === 'contract-test-model');
  assert.equal(entry.enabled, false, 'wb-state did not apply the field change');
});

test('removeEntry against the real wb-state CLI: the entry disappears', async () => {
  const result = await removeEntry('model', 'contract-test-model');
  assert.equal(result.ok, true, `expected ok, got: ${result.message}`);
  const still = readModelsFile().models.find((m: any) => m.id === 'contract-test-model');
  assert.equal(still, undefined, 'model still present after removeEntry');
});

// SPEC-V3 D item 4: a subscription provider (codex/agy) and its new
// loginCheckPath field — against the real CLI, not just this module's own
// construction (the reason the contract block above exists at all).
test('addProvider + setField(loginCheckPath) against the real wb-state CLI', async () => {
  const added = await addProvider({ id: 'contract-test-chatgpt', label: 'ChatGPT (Test)', kind: 'subscription' });
  assert.equal(added.ok, true, `expected ok, got: ${added.message}`);
  const providerEntry = readModelsFile().providers.find((p: any) => p.id === 'contract-test-chatgpt');
  assert.ok(providerEntry, 'provider missing from models.json after addProvider');
  assert.equal(providerEntry.kind, 'subscription');

  const result = await setField('provider', 'contract-test-chatgpt', 'loginCheckPath', '~/.codex/auth.json');
  assert.equal(result.ok, true, `expected ok, got: ${result.message}`);
  const updated = readModelsFile().providers.find((p: any) => p.id === 'contract-test-chatgpt');
  assert.equal(updated.loginCheckPath, '~/.codex/auth.json');

  const removed = await removeEntry('provider', 'contract-test-chatgpt');
  assert.equal(removed.ok, true, `expected ok, got: ${removed.message}`);
});

// SPEC-V3 D item 5: promptPattern/promptIgnore/autonomy through addHarness.
test('addHarness with promptPattern/promptIgnore/autonomy against the real wb-state CLI', async () => {
  const draft = {
    id: 'contract-test-harness',
    label: 'Contract Test Harness',
    command: 'true',
    promptPattern: '^>',
    promptIgnore: '^> suggestion',
    autonomy: { args: ['--yes'] },
  };
  const result = await addHarness(draft as any);
  assert.equal(result.ok, true, `expected ok, got: ${result.message}`);
  const entry = readModelsFile().harnesses.find((h: any) => h.id === 'contract-test-harness');
  assert.ok(entry, 'harness missing from models.json after addHarness');
  assert.equal(entry.promptPattern, '^>');
  assert.equal(entry.promptIgnore, '^> suggestion');
  assert.deepEqual(entry.autonomy, { args: ['--yes'] });

  const removed = await removeEntry('harness', 'contract-test-harness');
  assert.equal(removed.ok, true, `expected ok, got: ${removed.message}`);
});

// SPEC-V3 D item 2's "Binary fehlt" status state — against the REAL filesystem/
// PATH, not a mock: 'node' (this test runner itself) is always on PATH, and a
// nonsense command name is guaranteed absent.
test('harnessBinaryPresent: a bare command resolves against the real PATH', async () => {
  assert.equal(await harnessBinaryPresent('node'), true);
  assert.equal(await harnessBinaryPresent('wb-this-command-does-not-exist-anywhere'), false);
});

test('harnessBinaryPresent: an absolute/tilde path is checked for its executable bit directly', async () => {
  assert.equal(await harnessBinaryPresent(process.execPath), true, 'the running node binary itself is executable');
  assert.equal(await harnessBinaryPresent('/no/such/path/at/all'), false);
});
