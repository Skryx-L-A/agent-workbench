import assert from 'node:assert/strict';
import { test } from 'node:test';
import {
  claudeProjectSlug,
  folderSessions,
  isTranscriptUncertain,
  parseState,
  pickFreeKey,
  sessionDisplayName,
  sessionIdentity,
  sessionKeyFromFileName,
  slugForDir,
  sortByLastActive,
  stateFileForDir,
  stateFileName,
  transcriptRisk,
} from '../src/state.ts';

test('slugForDir replaces slashes (state-file slug, must match wb-state)', () => {
  assert.equal(slugForDir('/Users/alice/AI/foo'), '-Users-alice-AI-foo');
  assert.equal(slugForDir('/Users/alice/.pi/agent'), '-Users-alice-.pi-agent');
});

test('claudeProjectSlug also replaces dots and underscores', () => {
  // verified on disk: /Users/alice/.pi/agent -> ~/.claude/projects/-Users-alice--pi-agent
  assert.equal(claudeProjectSlug('/Users/alice/.pi/agent'), '-Users-alice--pi-agent');
  assert.equal(claudeProjectSlug('/Users/alice/AI/my_app'), '-Users-alice-AI-my-app');
  assert.equal(claudeProjectSlug('/Users/alice/AI/foo'), '-Users-alice-AI-foo');
});

test('stateFileForDir points into ~/.claude/workbench/sessions', () => {
  const file = stateFileForDir('/Users/alice/AI/foo');
  assert.ok(file.endsWith('/.claude/workbench/sessions/-Users-alice-AI-foo.json'), file);
});

test('parseState keeps the contract fields and defaults workers', () => {
  const state = parseState(JSON.stringify({
    dir: '/Users/alice/AI/foo',
    tmuxSession: 'wb-foo',
    lastActive: '2026-07-11T14:33:02Z',
    workers: [{ name: 'builder', kind: 'claude', model: 'fable:medium' }, { kind: 'broken' }],
  }));
  assert.equal(state?.dir, '/Users/alice/AI/foo');
  assert.equal(state?.tmuxSession, 'wb-foo');
  assert.deepEqual(state?.workers?.map((w) => w.name), ['builder']);
});

test('stateFileName: default session keeps the legacy name, others get the key', () => {
  assert.equal(stateFileName('/Users/alice/AI/foo'), '-Users-alice-AI-foo.json');
  assert.equal(stateFileName('/Users/alice/AI/foo', '9f2a1c'), '-Users-alice-AI-foo__9f2a1c.json');
  // not a key: the file must not become an unreachable name
  assert.equal(stateFileName('/Users/alice/AI/foo', 'ZZZ'), '-Users-alice-AI-foo.json');
});

test('sessionKeyFromFileName reads the key back, ignoring a folder with __', () => {
  assert.equal(sessionKeyFromFileName('-Users-alice-AI-foo__9f2a1c.json'), '9f2a1c');
  assert.equal(sessionKeyFromFileName('-Users-alice-AI-foo.json'), undefined);
  assert.equal(sessionKeyFromFileName('-Users-alice-AI-my__app.json'), undefined);
  assert.equal(sessionKeyFromFileName('-Users-alice-AI-my__app__00beef.json'), '00beef');
});

test('parseState keeps the V2 fields and tolerates V1 files', () => {
  const v2 = parseState(JSON.stringify({
    dir: '/Users/alice/AI/foo',
    name: 'Refactor-Session',
    sessionKey: '9f2a1c',
    harness: 'pi',
    model: 'ornith',
    tmuxSession: 'wb-foo-1a2b3c-9f2a1c',
  }));
  assert.equal(v2?.name, 'Refactor-Session');
  assert.equal(v2?.sessionKey, '9f2a1c');
  assert.equal(v2?.harness, 'pi');
  assert.equal(v2?.model, 'ornith');

  const v1 = parseState(JSON.stringify({ dir: '/Users/alice/AI/foo', tmuxSession: 'wb-foo' }));
  assert.equal(v1?.name, undefined);
  assert.equal(v1?.sessionKey, undefined);
  assert.equal(v1?.harness, undefined);
  // a malformed key is dropped rather than believed
  assert.equal(parseState('{"dir":"/x","sessionKey":"nope"}')?.sessionKey, undefined);
});

test('sessionDisplayName falls back to the folder name (V1 state files)', () => {
  assert.equal(sessionDisplayName({ dir: '/Users/alice/AI/foo', name: 'Bau' }), 'Bau');
  assert.equal(sessionDisplayName({ dir: '/Users/alice/AI/foo' }), 'foo');
  assert.equal(sessionDisplayName({ dir: '/Users/alice/AI/foo', name: '  ' }), 'foo');
});

test('folderSessions separates the default session from the keyed ones', () => {
  const files = [
    '-Users-alice-AI-foo.json',
    '-Users-alice-AI-foo__9f2a1c.json',
    '-Users-alice-AI-foo__00beef.json',
    '-Users-alice-AI-foobar.json',
    'not-json.txt',
  ];
  const sessions = folderSessions(files, '/Users/alice/AI/foo');
  assert.equal(sessions.hasDefault, true);
  assert.deepEqual(sessions.keys.sort(), ['00beef', '9f2a1c']);
  // a folder without any session file
  assert.deepEqual(folderSessions(files, '/Users/alice/AI/new'), { hasDefault: false, keys: [] });
});

test('transcriptRisk: one session per folder is never at risk', () => {
  const risk = transcriptRisk([
    { dir: '/Users/alice/AI/foo', name: 'Haupt' },
    { dir: '/Users/alice/AI/bar' },
  ]);
  assert.equal(risk.contested.size, 0);
  assert.equal(risk.duplicateName.size, 0);
});

test('transcriptRisk marks the sessions of a shared folder, worst case same name', () => {
  const states = [
    { dir: '/Users/alice/AI/foo', name: 'Haupt' },
    { dir: '/Users/alice/AI/foo', name: 'Refactor', sessionKey: '9f2a1c' },
    { dir: '/Users/alice/AI/bar', name: 'Bau', sessionKey: '00beef' },
    { dir: '/Users/alice/AI/bar', name: 'Bau', sessionKey: 'aa11bb' },
  ];
  const risk = transcriptRisk(states);
  assert.equal(risk.contested.size, 4, 'both folders hold two sessions');
  assert.deepEqual([...risk.duplicateName].sort(), [
    sessionIdentity({ dir: '/Users/alice/AI/bar', sessionKey: '00beef' }),
    sessionIdentity({ dir: '/Users/alice/AI/bar', sessionKey: 'aa11bb' }),
  ]);
});

test('isTranscriptUncertain: only when the folder is shared and the name did not settle it', () => {
  const foo = { dir: '/Users/alice/AI/foo', name: 'Haupt' };
  const fooTwo = { dir: '/Users/alice/AI/foo', name: 'Refactor', sessionKey: '9f2a1c' };
  const bar = { dir: '/Users/alice/AI/bar', name: 'Bau', sessionKey: '00beef' };
  const barTwin = { dir: '/Users/alice/AI/bar', name: 'Bau', sessionKey: 'aa11bb' };
  const risk = transcriptRisk([foo, fooTwo, bar, barTwin]);

  assert.equal(isTranscriptUncertain(foo, risk, true), false, 'matched by name');
  assert.equal(isTranscriptUncertain(foo, risk, false), true, 'newest transcript is a guess');
  // same name twice: even a "match" cannot tell the two apart
  assert.equal(isTranscriptUncertain(bar, risk, true), true);
  // a single session in its folder never warns
  const alone = { dir: '/Users/alice/AI/allein' };
  assert.equal(isTranscriptUncertain(alone, transcriptRisk([alone]), false), false);
  // a recorded claude session id would settle it (proposed contract extension)
  assert.equal(isTranscriptUncertain({ ...foo, claudeSessionId: 'abc-123' }, risk, false), false);
});

test('pickFreeKey skips keys already in use', () => {
  const queue = ['9f2a1c', '00beef', 'aa11bb'];
  const key = pickFreeKey(['9f2a1c', '00beef'], () => queue.shift()!);
  assert.equal(key, 'aa11bb');
});

test('parseState rejects malformed input', () => {
  assert.equal(parseState('not json'), undefined);
  assert.equal(parseState('{"tmuxSession":"wb-foo"}'), undefined);
  assert.equal(parseState('[]'), undefined);
});

test('sortByLastActive: newest first, undefined last', () => {
  const order = sortByLastActive([
    { dir: 'a', lastActive: '2026-07-01T00:00:00Z' },
    { dir: 'b' },
    { dir: 'c', lastActive: '2026-07-11T00:00:00Z' },
  ]).map((s) => s.dir);
  assert.deepEqual(order, ['c', 'a', 'b']);
});
