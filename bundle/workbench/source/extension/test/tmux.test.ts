import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { test } from 'node:test';
import {
  exact,
  hasSessionArgs,
  killSessionArgs,
  listPanesArgs,
  parseRolePanes,
  parseWorkerPanes,
  pickOrchestrator,
  ROLE_FORMAT,
  sessionName,
  viewSessionName,
  WORKER_FORMAT,
} from '../src/tmux.ts';

const md5 = (value: string) => createHash('md5').update(value).digest('hex').slice(0, 6);

test('sessionName: wb-<sanitized basename>-<md5-6 of the full path>', () => {
  assert.equal(sessionName('/Users/alice/AI/foo'), `wb-foo-${md5('/Users/alice/AI/foo')}`);
});

test('sessionName replaces special characters instead of dropping them', () => {
  // my_app and my-app must not collapse onto the same session (review finding 3)
  const underscore = sessionName('/Users/alice/AI/my_app');
  const hyphen = sessionName('/Users/alice/AI/my-app');
  assert.match(underscore, /^wb-my-app-[0-9a-f]{6}$/);
  assert.match(hyphen, /^wb-my-app-[0-9a-f]{6}$/);
  assert.notEqual(underscore, hyphen);
});

test('sessionName keeps same-basename projects apart', () => {
  assert.notEqual(sessionName('/Users/alice/AI/foo'), sessionName('/Users/alice/work/foo'));
});

test('exact() anchors the session name against tmux prefix matching', () => {
  assert.equal(exact('wb-Vox'), '=wb-Vox');
});

test('has-session is anchored against tmux prefix matching', () => {
  // '-t wb-Vox' would match a running 'wb-VoxType' — see review finding 1
  assert.deepEqual(hasSessionArgs('wb-Vox'), ['has-session', '-t', '=wb-Vox']);
});

test('pane listings ask the whole server and carry the session name', () => {
  // '=' does NOT anchor 'list-panes -s' (tmux 3.7b resolves it as a window
  // target and prefix-matches anyway), so the session is filtered in code.
  assert.deepEqual(listPanesArgs(WORKER_FORMAT), [
    'list-panes', '-a', '-F', '#{session_name}|#{pane_id}|#{@wb_worker}|#{pane_dead}',
  ]);
  assert.deepEqual(listPanesArgs(ROLE_FORMAT), [
    'list-panes', '-a', '-F', '#{session_name}|#{pane_id}|#{@wb_role}|#{pane_dead}',
  ]);
});

test('parseWorkerPanes reads pane_id / @wb_worker / pane_dead of the exact session', () => {
  const panes = parseWorkerPanes('wb-Vox|%3|builder|0\nwb-Vox|%7|reviewer|1\n', 'wb-Vox');
  assert.deepEqual(panes, [
    { paneId: '%3', worker: 'builder', dead: false },
    { paneId: '%7', worker: 'reviewer', dead: true },
  ]);
});

test('parseWorkerPanes ignores panes of a prefix-sharing session', () => {
  const out = 'wb-VoxType|%9|fremd|0\nwb-Vox|%3|builder|0\n';
  assert.deepEqual(parseWorkerPanes(out, 'wb-Vox'), [
    { paneId: '%3', worker: 'builder', dead: false },
  ]);
  assert.deepEqual(parseWorkerPanes(out, 'wb-VoxType'), [
    { paneId: '%9', worker: 'fremd', dead: false },
  ]);
});

test('parseWorkerPanes skips panes without @wb_worker', () => {
  assert.deepEqual(parseWorkerPanes('wb-foo|%1||0\nwb-foo|%2|builder|0\n', 'wb-foo'), [
    { paneId: '%2', worker: 'builder', dead: false },
  ]);
});

test('parseRolePanes reads pane_id / @wb_role / pane_dead of the exact session', () => {
  const out = 'wb-foolong|%9|orchestrator|0\nwb-foo|%1|orchestrator|0\nwb-foo|%2|worker|1\n\n';
  assert.deepEqual(parseRolePanes(out, 'wb-foo'), [
    { paneId: '%1', role: 'orchestrator', dead: false },
    { paneId: '%2', role: 'worker', dead: true },
  ]);
});

test('pickOrchestrator prefers the living pane and reports a dead one', () => {
  assert.deepEqual(
    pickOrchestrator([
      { paneId: '%1', role: 'orchestrator', dead: true },
      { paneId: '%4', role: 'orchestrator', dead: false },
      { paneId: '%2', role: 'worker', dead: false },
    ]),
    { status: 'ok', paneId: '%4' },
  );
  assert.deepEqual(
    pickOrchestrator([{ paneId: '%1', role: 'orchestrator', dead: true }]),
    { status: 'dead' },
  );
  assert.deepEqual(
    pickOrchestrator([{ paneId: '%2', role: 'worker', dead: false }]),
    { status: 'missing' },
  );
});

test('killSessionArgs anchors the view session (a group member, no window dies)', () => {
  assert.deepEqual(killSessionArgs(viewSessionName('wb-foo-1a2b3c')), [
    'kill-session', '-t', '=wb-foo-1a2b3c-view',
  ]);
});
