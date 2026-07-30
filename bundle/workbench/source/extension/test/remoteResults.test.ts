import assert from 'node:assert/strict';
import { test } from 'node:test';
import {
  parseRemoteResults,
  remoteResultsScript,
  readRemoteResults,
  RESULT_TIMEOUT_MS,
} from '../src/remoteResults.ts';

test('remoteResultsScript asks for every worker in ONE call, server-side', () => {
  assert.equal(remoteResultsScript([]), 'true');
  const script = remoteResultsScript(['SSH-builder', 'SSH-reviewer']);
  assert.match(script, /for n in 'SSH-builder' 'SSH-reviewer'; do/);
  assert.match(script, /\$HOME\/\.pi-workers\/results\/\$n\/latest\.md/);
  // the file is read over there and never copied here (same rule as wb-result)
  assert.ok(!script.includes('scp'), 'result files must not be copied');
  assert.match(script, /base64/);
});

test('remoteResultsScript quotes a name with a quote in it', () => {
  assert.match(remoteResultsScript([`SSH-it's`]), /'SSH-it'\\''s'/);
});

test('the sidebar SSH call has a shorter timeout than the state calls', () => {
  assert.ok(RESULT_TIMEOUT_MS <= 8000, 'the sidebar must not hang on a slow link');
});

function line(name: string, mtimeSeconds: number, firstLine: string): string {
  return `@@RES@@${name}\t${mtimeSeconds}\t${Buffer.from(firstLine, 'utf8').toString('base64')}`;
}

test('parseRemoteResults reads mtime in ms and the decoded first line', () => {
  const results = parseRemoteResults([
    line('SSH-builder', 1_760_000_000, 'Umbau fertig — Tests grün'),
    line('SSH-reviewer', 1_760_000_060, 'Review: 2 Findings'),
    'noise that is not ours',
  ].join('\n'));
  assert.equal(results.size, 2);
  assert.deepEqual(results.get('SSH-builder'), {
    firstLine: 'Umbau fertig — Tests grün',
    mtime: 1_760_000_000_000,
  });
  assert.equal(results.get('SSH-reviewer')?.firstLine, 'Review: 2 Findings');
});

test('parseRemoteResults survives garbage instead of guessing', () => {
  assert.equal(parseRemoteResults('').size, 0);
  const broken = parseRemoteResults('@@RES@@SSH-x\tnot-a-number\t!!!nobase64!!!');
  assert.equal(broken.get('SSH-x')?.mtime, 0);
  // a worker whose result file does not exist simply produces no line at all
  assert.equal(parseRemoteResults(line('SSH-a', 1, 'da')).get('SSH-b'), undefined);
});

test('readRemoteResults refuses to guess an SSH host for an unknown machine', async () => {
  await assert.rejects(
    () => readRemoteResults(['SSH-x'], 'irgendwas'),
    /keine SSH-Verbindung für Maschine "irgendwas"/,
  );
  // nothing to fetch: no SSH call at all, not an error
  assert.equal((await readRemoteResults([], 'peer')).size, 0);
});
