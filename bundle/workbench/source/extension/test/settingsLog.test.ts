import assert from 'node:assert/strict';
import { test } from 'node:test';
import {
  appendChangeLog,
  changedKeys,
  changeLines,
  changeLogPath,
  extensionActor,
  formatChangeLine,
  logTimestamp,
} from '../src/settingsLog.ts';

/**
 * Reference line produced by `wb-state settings set` (python section of
 * ~/.local/bin/wb-state): ts \t actor \t key \t <old JSON> -> <new JSON>.
 * The extension must produce exactly this shape — `tail` sees one log.
 */
const WB_STATE_LINE =
  '2026-07-25T09:12:33Z\twb-shell\tworkerLayout\t"split" -> "window"\n';

test('the line format matches wb-state character for character', () => {
  assert.equal(
    formatChangeLine('2026-07-25T09:12:33Z', 'wb-shell', 'workerLayout', 'split', 'window'),
    WB_STATE_LINE,
  );
  // tabs as separators, ' -> ' between the values, single trailing newline
  const [ts, actor, key, values] = WB_STATE_LINE.replace(/\n$/, '').split('\t');
  assert.equal(ts, '2026-07-25T09:12:33Z');
  assert.equal(actor, 'wb-shell');
  assert.equal(key, 'workerLayout');
  assert.equal(values, '"split" -> "window"');
});

test('an unset key is logged as the quoted string "<unset>", like wb-state', () => {
  // python: cfg.get(key, "<unset>") -> json.dumps -> "<unset>" WITH quotes
  assert.equal(
    formatChangeLine('2026-07-25T09:12:33Z', 'extension:foo', 'workerPollSeconds', undefined, 9),
    '2026-07-25T09:12:33Z\textension:foo\tworkerPollSeconds\t"<unset>" -> 9\n',
  );
});

test('values keep their JSON type, umlauts stay raw (ensure_ascii=False)', () => {
  assert.match(formatChangeLine('T', 'a', 'terminalStartMaximized', true, false), /true -> false$/m);
  assert.match(formatChangeLine('T', 'a', 'workerPollSeconds', 5, 12), /5 -> 12$/m);
  assert.match(
    formatChangeLine('T', 'a', 'newSessionDefaultDir', '~/AI', '~/Übungen'),
    /"~\/AI" -> "~\/Übungen"\n$/,
  );
});

test('logTimestamp is UTC without milliseconds', () => {
  assert.equal(logTimestamp(new Date('2026-07-25T09:12:33.456Z')), '2026-07-25T09:12:33Z');
  assert.match(logTimestamp(), /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$/);
});

test('the actor names the extension and the window it wrote from', () => {
  assert.equal(extensionActor('9f2a1c', '/Users/alice/AI/foo'), 'extension:9f2a1c');
  assert.equal(extensionActor(undefined, '/Users/alice/AI/foo'), 'extension:foo');
  assert.equal(extensionActor(undefined, undefined), 'extension');
});

test('only real changes are logged', () => {
  const before = { workerLayout: 'split', workerPollSeconds: 5 };
  assert.deepEqual(changedKeys(before, { ...before, workerLayout: 'window' }), ['workerLayout']);
  // a write that sets the same value again produces nothing
  assert.deepEqual(changedKeys(before, { ...before }), []);
  assert.equal(changeLines(before, { ...before }, 'extension:foo'), '');
});

test('one line PER changed key when a write touches several', () => {
  // a model change that clamps its effort, or a harness switch
  const before = { orchestratorHarness: 'claude', orchestratorModel: 'claude-opus-5', orchestratorEffort: 'xhigh' };
  const after = { orchestratorHarness: 'pi', orchestratorModel: 'ornith', orchestratorEffort: 'xhigh' };
  const lines = changeLines(before, after, 'extension:9f2a1c', '2026-07-25T09:12:33Z');
  assert.equal(
    lines,
    '2026-07-25T09:12:33Z\textension:9f2a1c\torchestratorHarness\t"claude" -> "pi"\n'
    + '2026-07-25T09:12:33Z\textension:9f2a1c\torchestratorModel\t"claude-opus-5" -> "ornith"\n',
  );
  assert.equal(lines.split('\n').filter((l) => l.length > 0).length, 2, 'the unchanged effort is not logged');
});

test('changeLogPath is the file the shell side appends to', () => {
  assert.ok(changeLogPath().endsWith('/.local/state/wb-settings-changes.log'), changeLogPath());
});

test('a log that cannot be written reports instead of throwing', async () => {
  // /dev/null/... can never be a directory — mkdir and append both fail
  const failure = await appendChangeLog('x\n', '/dev/null/nope/wb.log');
  assert.match(failure ?? '', /Settings-Änderungslog nicht schreibbar/);
  // nothing to write is not an error
  assert.equal(await appendChangeLog('', '/dev/null/nope/wb.log'), undefined);
});
