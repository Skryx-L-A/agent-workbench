// The drop directory is exercised for real (temp dir): whether a command file
// actually disappears is the whole point, so it is not faked here.
import assert from 'node:assert/strict';
import { mkdtempSync, writeFileSync } from 'node:fs';
import { readdir, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { test } from 'node:test';
import {
  type CommandSink,
  commandsDir,
  drainCommands,
  ensureCommandsDir,
  parseCommandFile,
  targetMatches,
  type WorkbenchCommand,
} from '../src/commandFile.ts';

function recorder(): CommandSink & { ran: WorkbenchCommand[]; logs: string[] } {
  const ran: WorkbenchCommand[] = [];
  const logs: string[] = [];
  return {
    ran,
    logs,
    run: async (command) => {
      ran.push(command);
    },
    log: (message) => logs.push(message),
  };
}

test('commandsDir sits next to the other workbench state', () => {
  assert.ok(commandsDir().endsWith('/.claude/workbench/commands'), commandsDir());
});

test('parseCommandFile accepts the file name AND a written line', () => {
  assert.equal(parseCommandFile('open-worker-tab', '')?.command, 'open-worker-tab');
  assert.equal(parseCommandFile('req-1234', 'open-worker-tab\n')?.command, 'open-worker-tab');
  assert.equal(parseCommandFile('open-worker-tab.cmd', '')?.command, 'open-worker-tab');
  // content wins when it names a command, comments and blank lines are skipped
  assert.equal(
    parseCommandFile('refresh-workers', '# vom Orchestrator\nfocus-orchestrator')?.command,
    'focus-orchestrator',
  );
  assert.equal(parseCommandFile('irgendwas', 'kein-kommando'), undefined);
  assert.equal(parseCommandFile('irgendwas', ''), undefined);
});

test('parseCommandFile reads the target: folder and optional session key', () => {
  // command in the content: target follows it
  assert.deepEqual(
    parseCommandFile('req-1', 'open-worker-tab\n/Users/alice/AI/foo\n9f2a1c\n')?.target,
    { dir: '/Users/alice/AI/foo', sessionKey: '9f2a1c' },
  );
  // command from the file name: the target starts at the first line
  assert.deepEqual(
    parseCommandFile('open-worker-tab', '/Users/alice/AI/foo\n')?.target,
    { dir: '/Users/alice/AI/foo', sessionKey: undefined },
  );
  // no target at all — the old, still valid form
  assert.deepEqual(parseCommandFile('open-worker-tab', '')?.target, { dir: undefined, sessionKey: undefined });
  // something that is not an absolute path is no target
  assert.deepEqual(parseCommandFile('open-worker-tab', 'foo')?.target, { dir: undefined, sessionKey: undefined });
});

test('targetMatches: no target is for everyone, a key must match exactly', () => {
  const windowA = { dir: '/Users/alice/AI/foo', sessionKey: '9f2a1c' };
  const windowB = { dir: '/Users/alice/AI/foo', sessionKey: undefined };
  const windowC = { dir: '/Users/alice/AI/bar', sessionKey: undefined };
  assert.equal(targetMatches({}, windowA), true);
  assert.equal(targetMatches({ dir: '/Users/alice/AI/foo' }, windowA), true, 'folder: any session of it');
  assert.equal(targetMatches({ dir: '/Users/alice/AI/foo' }, windowB), true);
  assert.equal(targetMatches({ dir: '/Users/alice/AI/foo' }, windowC), false);
  assert.equal(targetMatches({ dir: '/Users/alice/AI/foo', sessionKey: '9f2a1c' }, windowA), true);
  assert.equal(targetMatches({ dir: '/Users/alice/AI/foo', sessionKey: '9f2a1c' }, windowB), false);
});

test('a known command runs and its file is gone afterwards', async () => {
  const dir = mkdtempSync(join(tmpdir(), 'wb-commands-'));
  writeFileSync(join(dir, 'open-worker-tab'), '');
  writeFileSync(join(dir, 'req-42'), 'refresh-workers\n');
  const sink = recorder();
  await drainCommands(dir, sink);
  assert.deepEqual(sink.ran, ['open-worker-tab', 'refresh-workers']);
  assert.deepEqual(await readdir(dir), [], 'the drop directory must be empty again');
  assert.ok(sink.logs.some((l) => l.includes('Kommando ausgeführt: open-worker-tab')));
});

test('an unknown command is ignored, logged, and cleaned up', async () => {
  const dir = mkdtempSync(join(tmpdir(), 'wb-commands-'));
  writeFileSync(join(dir, 'rm-rf-slash'), 'do something evil');
  const sink = recorder();
  await drainCommands(dir, sink);
  assert.deepEqual(sink.ran, [], 'nothing unknown may ever be executed');
  assert.deepEqual(await readdir(dir), [], 'the stray file does not pile up');
  assert.ok(sink.logs.some((l) => l.includes('Unbekanntes Kommando ignoriert: rm-rf-slash')));
});

test('a broken drop breaks nothing: binary content, dotfiles, missing directory', async () => {
  const dir = mkdtempSync(join(tmpdir(), 'wb-commands-'));
  await writeFile(join(dir, 'open-worker-tab'), Buffer.from([0xff, 0xfe, 0x00, 0x01]));
  writeFileSync(join(dir, '.DS_Store'), 'noise');
  const sink = recorder();
  await drainCommands(dir, sink);
  // the name still says what to do, even with unreadable content
  assert.deepEqual(sink.ran, ['open-worker-tab']);
  assert.deepEqual(await readdir(dir), ['.DS_Store'], 'dotfiles are left alone');
  // a directory that does not exist is simply nothing to do
  await drainCommands(join(dir, 'weg'), sink);
  assert.deepEqual(sink.ran, ['open-worker-tab']);
});

test('a failing command is logged and never retried', async () => {
  const dir = mkdtempSync(join(tmpdir(), 'wb-commands-'));
  writeFileSync(join(dir, 'focus-orchestrator'), '');
  const logs: string[] = [];
  await drainCommands(dir, {
    run: async () => {
      throw new Error('kein Terminal');
    },
    log: (message) => logs.push(message),
  });
  assert.deepEqual(await readdir(dir), [], 'the file is gone even though the command failed');
  assert.ok(logs.some((l) => l.includes('Kommando fehlgeschlagen (focus-orchestrator): kein Terminal')));
});

const FOO = '/Users/alice/AI/foo';

test('a command addressed to this window runs here', async () => {
  const dir = mkdtempSync(join(tmpdir(), 'wb-commands-'));
  writeFileSync(join(dir, 'req-1'), `open-worker-tab\n${FOO}\n9f2a1c\n`);
  const sink = recorder();
  await drainCommands(dir, sink, { window: { dir: FOO, sessionKey: '9f2a1c' } });
  assert.deepEqual(sink.ran, ['open-worker-tab']);
  assert.deepEqual(await readdir(dir), []);
});

test('a command for ANOTHER window is left lying, not executed', async () => {
  const dir = mkdtempSync(join(tmpdir(), 'wb-commands-'));
  writeFileSync(join(dir, 'req-1'), `open-worker-tab\n${FOO}\n9f2a1c\n`);
  const sink = recorder();
  // same folder, but the other session of it — and a completely different folder
  await drainCommands(dir, sink, { window: { dir: FOO, sessionKey: undefined } });
  await drainCommands(dir, sink, { window: { dir: '/Users/alice/AI/bar' } });
  assert.deepEqual(sink.ran, [], 'the wrong window must not run it');
  assert.deepEqual(await readdir(dir), ['req-1'], 'it waits for its window');
});

test('a command without a target still runs in the first window that sees it', async () => {
  const dir = mkdtempSync(join(tmpdir(), 'wb-commands-'));
  writeFileSync(join(dir, 'open-worker-tab'), '');
  const sink = recorder();
  await drainCommands(dir, sink, { window: { dir: '/Users/alice/AI/irgendwas' } });
  assert.deepEqual(sink.ran, ['open-worker-tab'], 'unchanged behaviour');
  assert.deepEqual(await readdir(dir), []);
});

test('a targeted command whose window never appears expires and is logged', async () => {
  const dir = mkdtempSync(join(tmpdir(), 'wb-commands-'));
  writeFileSync(join(dir, 'req-1'), `open-worker-tab\n${FOO}\n9f2a1c\n`);
  const sink = recorder();
  const window = { dir: '/Users/alice/AI/bar' };
  // fresh: it waits
  await drainCommands(dir, sink, { window, now: Date.now(), expireMs: 60_000 });
  assert.deepEqual(await readdir(dir), ['req-1']);
  // an hour later nobody has claimed it
  await drainCommands(dir, sink, { window, now: Date.now() + 3_600_000, expireMs: 60_000 });
  assert.deepEqual(sink.ran, [], 'expiring is not executing');
  assert.deepEqual(await readdir(dir), [], 'it does not rot in the directory');
  assert.ok(
    sink.logs.some((l) => l.includes('Kommando verfallen (kein passendes Fenster): open-worker-tab für /Users/alice/AI/foo [9f2a1c]')),
    sink.logs.join(' | '),
  );
});

test('ensureCommandsDir creates the drop directory', async () => {
  const dir = join(mkdtempSync(join(tmpdir(), 'wb-commands-')), 'neu', 'tief');
  await ensureCommandsDir(dir);
  assert.deepEqual(await readdir(dir), []);
});
