import assert from 'node:assert/strict';
import { test } from 'node:test';
import { decidePending, PENDING_TTL_MS, type PendingAction } from '../src/pending.ts';

const now = 1_000_000;
const pending = (dir: string, age = 0): PendingAction => ({ dir, createdAt: now - age });

test('decidePending consumes the action of the window that opened the folder', () => {
  assert.equal(decidePending(pending('/AI/foo'), '/AI/foo', now), 'consume');
});

test('decidePending consumes an peer remote path (matched via uri.path)', () => {
  // Design C: the stored dir is the peer path and the remote workspace folder's
  // uri.path reports the very same string, so they match in the UI host.
  const peer = '/home/alice/AI/VoxType';
  assert.equal(decidePending(pending(peer), peer, now), 'consume');
});

test('decidePending keeps another window\'s fresh action (globalState is shared)', () => {
  assert.equal(decidePending(pending('/AI/foo'), '/AI/bar', now), 'keep');
  assert.equal(decidePending(pending('/AI/foo'), undefined, now), 'keep');
});

test('decidePending expires an action nobody picked up', () => {
  assert.equal(decidePending(pending('/AI/foo', PENDING_TTL_MS + 1), '/AI/bar', now), 'expire');
});

test('decidePending consumes on match even when stale', () => {
  assert.equal(decidePending(pending('/AI/foo', PENDING_TTL_MS + 1), '/AI/foo', now), 'consume');
});

test('decidePending does nothing without a pending action', () => {
  assert.equal(decidePending(undefined, '/AI/foo', now), 'none');
});
