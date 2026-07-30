import assert from 'node:assert/strict';
import { test } from 'node:test';
import { isRemoteWorker, mergeWorkers, resultTooltip } from '../src/workers.ts';

test('mergeWorkers marks state workers without a pane as ended', () => {
  const workers = mergeWorkers([{ name: 'builder', model: 'fable:medium' }], []);
  assert.deepEqual(workers, [
    { name: 'builder', kind: undefined, model: 'fable:medium', machine: undefined, status: 'ended' },
  ]);
});

test('mergeWorkers carries the machine of an SSH worker, none for a local one', () => {
  const workers = mergeWorkers(
    [
      { name: 'SSH-builder', kind: 'claude', model: 'sonnet5:high', machine: 'peer' },
      { name: 'reviewer', kind: 'claude', model: 'opus5:high' },
    ],
    [{ paneId: '%3', worker: 'SSH-builder', dead: false }],
  );
  const ssh = workers.find((w) => w.name === 'SSH-builder')!;
  // the local pane is only the mirror — the worker itself runs on peer
  assert.equal(ssh.machine, 'peer');
  assert.equal(ssh.status, 'running');
  assert.equal(isRemoteWorker(ssh), true);
  const local = workers.find((w) => w.name === 'reviewer')!;
  assert.equal(local.machine, undefined);
  assert.equal(isRemoteWorker(local), false);
  // a worker explicitly marked as running on this machine is not remote either
  assert.equal(isRemoteWorker({ machine: 'mac' }), false);
});

test('resultTooltip never claims "kein Result" for a remote worker', () => {
  const base = { name: 'SSH-builder', status: 'running' as const, machine: 'peer' };
  assert.match(resultTooltip(base), /Noch kein Result auf peer/);
  assert.match(
    resultTooltip({ ...base, resultUnreachable: true }),
    /Result liegt auf peer — Maschine nicht erreichbar \(ssh peer\)\./,
  );
  assert.match(
    resultTooltip({ ...base, result: { firstLine: 'Fertig', mtime: Date.now() } }),
    /Letztes Result auf peer \(gerade eben\): Fertig/,
  );
  // local workers keep the V1 wording
  const local = { name: 'reviewer', status: 'ended' as const };
  assert.equal(resultTooltip(local), 'Kein Result vorhanden.');
  assert.match(
    resultTooltip({ ...local, result: { firstLine: 'Review ok', mtime: Date.now() } }),
    /^Letztes Result \(gerade eben\): Review ok$/,
  );
});

test('mergeWorkers takes live and dead status from tmux', () => {
  const workers = mergeWorkers(
    [{ name: 'builder' }, { name: 'reviewer' }],
    [
      { paneId: '%3', worker: 'builder', dead: false },
      { paneId: '%7', worker: 'reviewer', dead: true },
    ],
  );
  assert.deepEqual(
    workers.map((w) => [w.name, w.status, w.paneId]),
    [['builder', 'running', '%3'], ['reviewer', 'dead', '%7']],
  );
});

test('mergeWorkers adds live panes missing from the state file', () => {
  const workers = mergeWorkers([], [{ paneId: '%9', worker: 'adhoc', dead: false }]);
  assert.deepEqual(workers.map((w) => w.name), ['adhoc']);
  assert.equal(workers[0].status, 'running');
});
