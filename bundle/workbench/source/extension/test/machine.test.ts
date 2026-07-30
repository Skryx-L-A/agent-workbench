import assert from 'node:assert/strict';
import { test } from 'node:test';
import {
  isMachine,
  PEER_REMOTE_AUTHORITY,
  peerRemoteUri,
  MACHINE_LABEL,
  MACHINES,
  otherMachine,
} from '../src/machine.ts';

test('isMachine accepts only the two machines', () => {
  assert.equal(isMachine('mac'), true);
  assert.equal(isMachine('peer'), true);
  assert.equal(isMachine('windows'), false);
  assert.equal(isMachine(undefined), false);
});

test('MACHINES and labels stay in sync, German, no emoji', () => {
  assert.deepEqual([...MACHINES], ['mac', 'peer']);
  for (const machine of MACHINES) {
    assert.ok(MACHINE_LABEL[machine].length > 0);
  }
  const joined = Object.values(MACHINE_LABEL).join(' ');
  assert.ok(!/\p{Extended_Pictographic}/u.test(joined), 'emoji in a machine label');
});

test('otherMachine toggles', () => {
  assert.equal(otherMachine('mac'), 'peer');
  assert.equal(otherMachine('peer'), 'mac');
});

test('peerRemoteUri builds the vscode-remote parts for the absolute peer path', () => {
  assert.deepEqual(peerRemoteUri('/home/alice/AI/Demo'), {
    scheme: 'vscode-remote',
    authority: PEER_REMOTE_AUTHORITY,
    path: '/home/alice/AI/Demo',
  });
  assert.equal(PEER_REMOTE_AUTHORITY, 'ssh-remote+peer');
});
