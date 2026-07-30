// Picking the right transcript for a SESSION (not just for the folder). Uses
// real files under a redirected HOME; node --test isolates this file's process.
import assert from 'node:assert/strict';
import { mkdirSync, mkdtempSync, writeFileSync, utimesSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { test } from 'node:test';
import { latestTranscript } from '../src/claudeLog.ts';

const home = mkdtempSync(join(tmpdir(), 'wb-transcripts-'));
process.env.HOME = home;

const DIR = '/tmp/x/foo';
const projectDir = join(home, '.claude', 'projects', '-tmp-x-foo');
mkdirSync(projectDir, { recursive: true });

function transcript(file: string, name: string, message: string, mtime: number): void {
  const path = join(projectDir, file);
  writeFileSync(path, [
    JSON.stringify({ type: 'custom-title', customTitle: name }),
    JSON.stringify({ type: 'user', message: { role: 'user', content: message } }),
    '',
  ].join('\n'), 'utf8');
  utimesSync(path, mtime, mtime);
}

transcript('aaa.jsonl', 'Refactor', 'alte Sitzung', 1_000_000);
transcript('bbb.jsonl', 'Haupt', 'neue Sitzung', 2_000_000);

test('latestTranscript picks the transcript of the named session (SPEC-V2 B/D)', async () => {
  // two workbench sessions in one folder must not resume the same Claude session
  assert.equal((await latestTranscript(DIR, 'Refactor'))?.sessionId, 'aaa');
  assert.equal((await latestTranscript(DIR, 'Refactor'))?.lastUserMessage, 'alte Sitzung');
  assert.equal((await latestTranscript(DIR, 'Haupt'))?.sessionId, 'bbb');
});

test('latestTranscript falls back to the newest one (V1 behaviour)', async () => {
  assert.equal((await latestTranscript(DIR))?.sessionId, 'bbb');
  assert.equal((await latestTranscript(DIR, 'Nie gestartet'))?.sessionId, 'bbb');
});

test('latestTranscript returns undefined for a folder without transcripts', async () => {
  assert.equal(await latestTranscript('/tmp/x/leer'), undefined);
});
