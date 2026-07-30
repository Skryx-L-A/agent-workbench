import assert from 'node:assert/strict';
import { test } from 'node:test';
import {
  hasOrchestratorDescendant,
  orchestratorAttachCommand,
  orchestratorCommand,
  parseProcesses,
  restorePlan,
  terminalPlan,
  workerViewCommand,
} from '../src/terminal.ts';

test('terminalPlan never launches wb-code in a live workbench terminal', () => {
  // that terminal sits in the orchestrator's Claude prompt — see review finding 6
  assert.equal(terminalPlan({ createdByUs: true, exited: false, liveness: 'alive' }), 'show');
  assert.equal(terminalPlan(undefined), 'launch');
});

test('terminalPlan relaunches for a terminal VSCode restored after the reload', () => {
  // same name, but not created by this extension host: wb-code is dead in there
  assert.equal(terminalPlan({ createdByUs: false, exited: false, liveness: 'alive' }), 'launch');
});

test('terminalPlan relaunches whenever liveness is not certain', () => {
  // an empty shell prompt is worse than a second tmux client (tmux is multi-client)
  assert.equal(terminalPlan({ createdByUs: true, exited: false, liveness: 'unknown' }), 'launch');
  assert.equal(terminalPlan({ createdByUs: true, exited: false, liveness: 'dead' }), 'launch');
  assert.equal(terminalPlan({ createdByUs: true, exited: true, liveness: 'alive' }), 'launch');
});

test('orchestratorCommand quotes dir and session id', () => {
  assert.equal(
    orchestratorCommand('/Users/alice/AI/foo'),
    `wb-code '/Users/alice/AI/foo'`,
  );
  assert.equal(
    orchestratorCommand('/Users/alice/AI/foo', { sessionId: 'abc-123' }),
    `wb-code '/Users/alice/AI/foo' --resume 'abc-123'`,
  );
  assert.equal(
    orchestratorCommand(`/tmp/it's`),
    `wb-code '/tmp/it'\\''s'`,
  );
});

test('orchestratorCommand passes session name and key (SPEC-V2 B)', () => {
  assert.equal(
    orchestratorCommand('/Users/alice/AI/foo', { name: 'Refactor', sessionKey: '9f2a1c' }),
    `wb-code '/Users/alice/AI/foo' --name 'Refactor' --key '9f2a1c'`,
  );
  assert.equal(
    orchestratorCommand('/Users/alice/AI/foo', {
      sessionId: 'abc-123',
      name: `Alex's Session`,
      sessionKey: '00beef',
    }),
    `wb-code '/Users/alice/AI/foo' --resume 'abc-123' --name 'Alex'\\''s Session' --key '00beef'`,
  );
  // the folder's default session has no key, and an empty name is left out
  assert.equal(
    orchestratorCommand('/Users/alice/AI/foo', { name: '' }),
    `wb-code '/Users/alice/AI/foo'`,
  );
});

test('workerViewCommand attaches to a grouped view session, anchored (SPEC-V2 C)', () => {
  const command = workerViewCommand('wb-foo-1a2b3c', 'wb-foo-1a2b3c-view');
  // waits for the orchestrator session — bounded, never forever
  assert.match(command, /for i in \$\(seq 1 \d+\); do tmux has-session -t '=wb-foo-1a2b3c'/);
  // existing view session: attach only, no second new-session
  assert.match(
    command,
    /tmux has-session -t '=wb-foo-1a2b3c-view' 2>\/dev\/null \|\| tmux new-session -d -t '=wb-foo-1a2b3c' -s 'wb-foo-1a2b3c-view'/,
  );
  // the workers window may not exist yet — selecting it must not abort the attach
  assert.match(command, /tmux select-window -t '=wb-foo-1a2b3c-view:workers' 2>\/dev\/null/);
  assert.match(command, /exec tmux attach-session -t '=wb-foo-1a2b3c-view'$/);
});

test('workerViewCommand creates the workers window BEFORE the view session', () => {
  // A group whose only window is the orchestrator's dies whole when that window
  // is closed (measured 2026-07-25) — so the workers window must exist first.
  const command = workerViewCommand('wb-foo-1a2b3c', 'wb-foo-1a2b3c-view');
  assert.match(command, /wb-workers-window 'wb-foo-1a2b3c'/);
  assert.ok(
    command.indexOf('wb-workers-window') < command.indexOf('new-session'),
    'the workers window has to exist before the grouped session is created',
  );
});

const PS = `
    1     0 /sbin/launchd
  500     1 /bin/zsh -il
  600   500 tmux attach -t =wb-scratch-abc123
  700     1 /bin/zsh -il
  800   700 /usr/bin/less README.md
`;

test('parseProcesses reads the ps columns', () => {
  const processes = parseProcesses(PS);
  assert.equal(processes.length, 5);
  assert.deepEqual(processes[2], {
    pid: 600,
    ppid: 500,
    command: 'tmux attach -t =wb-scratch-abc123',
  });
});

test('hasOrchestratorDescendant finds the tmux client under the shell', () => {
  const processes = parseProcesses(PS);
  assert.equal(hasOrchestratorDescendant(processes, 500), true);
  // the restored ghost terminal: a shell with nothing of ours below it
  assert.equal(hasOrchestratorDescendant(processes, 700), false);
});

test('hasOrchestratorDescendant looks deeper than the direct children', () => {
  const processes = parseProcesses(`
  900     1 /bin/zsh -il
  910   900 /bin/bash /Users/alice/.local/bin/wb-code /Users/alice/AI/x
  920   910 tmux attach -t =wb-x-1
`);
  assert.equal(hasOrchestratorDescendant(processes, 900), true);
});

test('hasOrchestratorDescendant does not match the shell itself', () => {
  // the terminal's own shell never counts as an attached orchestrator
  const processes = parseProcesses(`
  930     1 /bin/zsh -il claude
`);
  assert.equal(hasOrchestratorDescendant(processes, 930), false);
});

test('restore: a live session brings the terminals back, a dead one does not', () => {
  const base = { hasFolder: true, sessionAlive: true, handledPending: false, layout: 'split' as const };
  assert.deepEqual(restorePlan(base), { orchestrator: true, workerTab: false });
  // workerLayout "window" restores BOTH tabs
  assert.deepEqual(restorePlan({ ...base, layout: 'window' }), { orchestrator: true, workerTab: true });
  // a session alice ended must not be resurrected by a reload
  assert.deepEqual(restorePlan({ ...base, sessionAlive: false }), { orchestrator: false, workerTab: false });
  assert.deepEqual(restorePlan({ ...base, hasFolder: false }), { orchestrator: false, workerTab: false });
});

test('restore does nothing when the launch path already ran (no double tab)', () => {
  assert.deepEqual(
    restorePlan({ hasFolder: true, sessionAlive: true, handledPending: true, layout: 'window' }),
    { orchestrator: false, workerTab: false },
  );
});

test('the restored orchestrator tab ATTACHES — it can never start a second Claude', () => {
  const command = orchestratorAttachCommand('wb-foo-1a2b3c');
  assert.equal(command, `exec tmux attach-session -t '=wb-foo-1a2b3c'`);
  assert.ok(!command.includes('wb-code'), 'wb-code could decide to start Claude');
  assert.ok(!command.includes('claude'), 'nothing here may launch an agent');
});
