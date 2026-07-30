import assert from 'node:assert/strict';
import { test } from 'node:test';
import { workerViewCommand } from '../src/terminal.ts';
import { viewSessionName } from '../src/tmux.ts';
import {
  HINT_COOLDOWN_MS,
  hintMessage,
  regridCommand,
  shouldHintWorkerTab,
  syncLayout,
  workerTabAction,
} from '../src/workerTab.ts';

test('switching to "window" opens the tab, switching back closes it', () => {
  assert.equal(workerTabAction('window', 'none'), 'open');
  assert.equal(workerTabAction('window', 'live'), 'none', 'a live tab stays as it is');
  // a tab VSCode restored across a reload shows nothing — it gets replaced
  assert.equal(workerTabAction('window', 'ghost'), 'open');
  // after wb-grid pulled the panes back, the workers window is gone — no tab
  // pointing at it may survive, not even a dead one
  assert.equal(workerTabAction('split', 'live'), 'close');
  assert.equal(workerTabAction('split', 'ghost'), 'close');
  assert.equal(workerTabAction('split', 'none'), 'none');
});

test('a missing tab while workers run leads to the hint', () => {
  const now = 1_000_000;
  // the real incident: four workers in the workers window, no tab, no sign of them
  assert.equal(shouldHintWorkerTab('window', 'none', 4, undefined, now), true);
  assert.equal(shouldHintWorkerTab('window', 'ghost', 4, undefined, now), true, 'a ghost shows nothing');
  assert.equal(shouldHintWorkerTab('window', 'live', 4, undefined, now), false, 'tab is there');
  assert.equal(shouldHintWorkerTab('split', 'none', 4, undefined, now), false, 'split needs no tab');
  assert.equal(shouldHintWorkerTab('window', 'none', 0, undefined, now), false, 'nothing running');
});

test('the hint does not nag: once per cooldown', () => {
  const shown = 1_000_000;
  assert.equal(shouldHintWorkerTab('window', 'none', 2, shown, shown + 1000), false);
  assert.equal(shouldHintWorkerTab('window', 'none', 2, shown, shown + HINT_COOLDOWN_MS), true);
});

test('hintMessage names how many workers are hidden, without emoji', () => {
  assert.match(hintMessage(1), /^1 Worker läuft im eigenen Worker-Tab/);
  assert.match(hintMessage(4), /^4 Worker laufen im eigenen Worker-Tab/);
  assert.ok(!/\p{Extended_Pictographic}/u.test(hintMessage(4)));
});

test('the workers window target is session-qualified and anchored', () => {
  const session = 'wb-foo-1a2b3c';
  const command = workerViewCommand(session, viewSessionName(session));
  // '=' anchors the session so a prefix match cannot hit another session, and
  // the window is addressed through ITS session — never as a bare 'workers'
  assert.match(command, /tmux select-window -t '=wb-foo-1a2b3c-view:workers'/);
  assert.ok(!/select-window -t 'workers'/.test(command), 'bare window target would be ambiguous');
  assert.match(command, /tmux new-session -d -t '=wb-foo-1a2b3c' -s 'wb-foo-1a2b3c-view'/);
});

test('regridCommand hands wb-grid a pane of the session', () => {
  // wb-grid derives the session from the pane and moves panes for the CURRENT
  // setting — running workers survive the switch
  assert.equal(regridCommand('%12'), 'wb-grid %12');
});

test('a foreign settings change is adopted, and only once', () => {
  // window B still believes "split" while the file says "window"
  const first = syncLayout('split', 'window');
  assert.deepEqual(first, { apply: true, known: 'window' });
  // seeing the same value again does nothing — no regrid ping-pong
  assert.deepEqual(syncLayout(first.known, 'window'), { apply: false, known: 'window' });
});

test('two windows observing each other settle after one application each', () => {
  // both believe "split", the file says "window" (somebody switched it)
  let knownA: 'split' | 'window' = 'split';
  let knownB: 'split' | 'window' = 'split';
  const applied: string[] = [];
  for (let tick = 0; tick < 5; tick++) {
    const a = syncLayout(knownA, 'window');
    knownA = a.known;
    if (a.apply) applied.push('A');
    const b = syncLayout(knownB, 'window');
    knownB = b.known;
    if (b.apply) applied.push('B');
  }
  assert.deepEqual(applied, ['A', 'B'], 'each window regrids exactly once, then both are quiet');
});
