// Writes real files, so this test file redirects HOME into a temp directory.
// node --test runs every test file in its own process, which keeps that safe.
import assert from 'node:assert/strict';
import { mkdtempSync } from 'node:fs';
import { mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { test } from 'node:test';
import { readRawSettings, readSettings, settingsFile, writeSetting } from '../src/settings.ts';

// The settings paths are derived from homedir() at call time, so redirecting
// HOME here is enough — no test ever touches the real settings file.
const home = mkdtempSync(join(tmpdir(), 'wb-settings-'));
process.env.HOME = home;

// Guard: stop rather than write into alice's home if the redirect did not take.
assert.ok(settingsFile().startsWith(home), `HOME redirect failed: ${settingsFile()}`);

test('writeSetting creates the file and reads back through the defaults', async () => {
  const settings = await writeSetting('workerLayout', 'window');
  assert.equal(settings.workerLayout, 'window');
  assert.equal(settings.orchestratorModel, 'claude-opus-5', 'unset keys keep their default');
  assert.deepEqual(JSON.parse(await readFile(settingsFile(), 'utf8')), { workerLayout: 'window' });
});

test('writeSetting preserves keys this version does not know (SPEC-V2 A)', async () => {
  await mkdir(join(home, '.claude', 'workbench'), { recursive: true });
  await writeFile(
    settingsFile(),
    JSON.stringify({ contextGuardAutostart: true, futureKey: [1, 2], workerEffort: 'high' }),
    'utf8',
  );
  await writeSetting('orchestratorEffort', 'low');
  const raw = await readRawSettings();
  assert.equal(raw.contextGuardAutostart, true);
  assert.deepEqual(raw.futureKey, [1, 2]);
  assert.equal(raw.workerEffort, 'high');
  assert.equal(raw.orchestratorEffort, 'low');
});

// SPEC-V2 A, four new keys (Reviewer-Befund F5): a real write/read round trip,
// not just the pure parseSettings/coerceSetting unit tests.
test('writeSetting round-trips the four new keys through the real file', async () => {
  await writeFile(settingsFile(), '{}', 'utf8');
  await writeSetting('guardOrchWarnPct', 65);
  await writeSetting('guardWorkerWarnPct', 88);
  await writeSetting('minWorkerPaneWidth', 90);
  await writeSetting('maxWorkers', 4);
  const settings = await writeSetting('defaultWorkerMachine', 'peer');
  assert.equal(settings.guardOrchWarnPct, 65);
  assert.equal(settings.guardWorkerWarnPct, 88);
  assert.equal(settings.minWorkerPaneWidth, 90);
  assert.equal(settings.maxWorkers, 4);
  assert.equal(settings.defaultWorkerMachine, 'peer');
  assert.deepEqual(JSON.parse(await readFile(settingsFile(), 'utf8')), {
    guardOrchWarnPct: 65,
    guardWorkerWarnPct: 88,
    minWorkerPaneWidth: 90,
    maxWorkers: 4,
    defaultWorkerMachine: 'peer',
  });
});

// Vertrag Teil 2 §5: a real write/read round trip for the new switch.
test('writeSetting round-trips modelDiscoveryAuto through the real file', async () => {
  await writeFile(settingsFile(), '{}', 'utf8');
  const settings = await writeSetting('modelDiscoveryAuto', false);
  assert.equal(settings.modelDiscoveryAuto, false);
  assert.deepEqual(JSON.parse(await readFile(settingsFile(), 'utf8')), { modelDiscoveryAuto: false });
  const readBack = await readSettings();
  assert.equal(readBack.modelDiscoveryAuto, false, 'the value survives a fresh read, not just the in-memory return');
});

test('concurrent writes do not lose each other', async () => {
  await writeFile(settingsFile(), '{}', 'utf8');
  await Promise.all([
    writeSetting('workerPollSeconds', 9),
    writeSetting('newSessionDefaultDir', '~/Projekte'),
    writeSetting('terminalStartMaximized', false),
  ]);
  const settings = await readSettings();
  assert.equal(settings.workerPollSeconds, 9);
  assert.equal(settings.newSessionDefaultDir, '~/Projekte');
  assert.equal(settings.terminalStartMaximized, false);
});

const changeLog = join(home, '.local', 'state', 'wb-settings-changes.log');

async function logLines(): Promise<string[]> {
  try {
    return (await readFile(changeLog, 'utf8')).split('\n').filter((l) => l.length > 0);
  } catch {
    return [];
  }
}

test('every write of the extension lands in the change log', async () => {
  await writeFile(settingsFile(), JSON.stringify({ workerLayout: 'split' }), 'utf8');
  await writeFile(changeLog, '', 'utf8');
  await writeSetting('workerLayout', 'window', { actor: 'extension:9f2a1c' });
  const lines = await logLines();
  assert.equal(lines.length, 1);
  assert.match(
    lines[0],
    /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z\textension:9f2a1c\tworkerLayout\t"split" -> "window"$/,
  );
});

test('a write that changes nothing logs nothing', async () => {
  await writeFile(settingsFile(), JSON.stringify({ workerLayout: 'window' }), 'utf8');
  await writeFile(changeLog, '', 'utf8');
  await writeSetting('workerLayout', 'window', { actor: 'extension:foo' });
  assert.deepEqual(await logLines(), [], 'setting a value to itself is no change');
});

test('a write touching several keys logs one line per key', async () => {
  await writeFile(
    settingsFile(),
    JSON.stringify({ orchestratorModel: 'claude-opus-5', orchestratorEffort: 'xhigh' }),
    'utf8',
  );
  await writeFile(changeLog, '', 'utf8');
  // fable caps the effort at medium, so model AND effort change in one write
  await writeSetting('orchestratorModel', 'claude-fable-5', { actor: 'extension:foo' });
  const lines = await logLines();
  assert.equal(lines.length, 2);
  assert.match(lines[0], /\torchestratorEffort\t"xhigh" -> "medium"$/);
  assert.match(lines[1], /\torchestratorModel\t"claude-opus-5" -> "claude-fable-5"$/);
});

test('the log is appended, never rewritten', async () => {
  await writeFile(changeLog, 'ALTE ZEILE\n', 'utf8');
  await writeFile(settingsFile(), '{}', 'utf8');
  await writeSetting('workerPollSeconds', 11, { actor: 'extension:foo' });
  const lines = await logLines();
  assert.equal(lines[0], 'ALTE ZEILE');
  assert.match(lines[1], /\tworkerPollSeconds\t"<unset>" -> 11$/);
});

test('a failing log does NOT stop the settings write', async () => {
  await writeFile(settingsFile(), '{}', 'utf8');
  // force a real failure: a DIRECTORY where the log file belongs (EISDIR on append)
  await rm(changeLog, { force: true });
  await mkdir(changeLog, { recursive: true });
  const problems: string[] = [];
  const settings = await writeSetting('terminalStartMaximized', false, {
    actor: 'extension:foo',
    onLogError: (message) => problems.push(message),
  });
  // the write itself stands …
  assert.equal(settings.terminalStartMaximized, false);
  assert.equal(JSON.parse(await readFile(settingsFile(), 'utf8')).terminalStartMaximized, false);
  // … and the log problem is reported, not thrown
  assert.equal(problems.length, 1);
  assert.match(problems[0], /Settings-Änderungslog nicht schreibbar/);
  await rm(changeLog, { recursive: true, force: true });
});

test('a broken settings file never breaks reading', async () => {
  await writeFile(settingsFile(), '{ this is not json', 'utf8');
  const settings = await readSettings();
  assert.equal(settings.orchestratorModel, 'claude-opus-5');
  assert.deepEqual(await readRawSettings(), {});
});
