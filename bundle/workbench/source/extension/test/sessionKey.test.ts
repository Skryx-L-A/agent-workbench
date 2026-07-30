import assert from 'node:assert/strict';
import { test } from 'node:test';
import { needsKey, parseNewKeyOutput } from '../src/sessionKey.ts';

test('parseNewKeyOutput accepts a 6-hex key and nothing else', () => {
  assert.equal(parseNewKeyOutput('9f2a1c\n'), '9f2a1c');
  assert.equal(parseNewKeyOutput('  00beef  '), '00beef');
  // wb-state missing / an error message must not become a session key
  assert.equal(parseNewKeyOutput('wb-state: unbekanntes Kommando'), undefined);
  assert.equal(parseNewKeyOutput(''), undefined);
  assert.equal(parseNewKeyOutput('9F2A1C'), undefined);
  assert.equal(parseNewKeyOutput('9f2a1c9f'), undefined);
});

test('needsKey: the first session of a folder stays the default session', () => {
  assert.equal(needsKey({ hasDefault: false, keys: [] }), false);
  assert.equal(needsKey({ hasDefault: true, keys: [] }), true);
  // a folder whose default file was deleted but which still has keyed sessions
  assert.equal(needsKey({ hasDefault: false, keys: ['9f2a1c'] }), true);
});
