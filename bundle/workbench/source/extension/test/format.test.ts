import assert from 'node:assert/strict';
import { test } from 'node:test';
import { escapeHtml, previewText, relativeTime, shellQuote } from '../src/format.ts';

const now = new Date('2026-07-11T16:00:00Z');
const ago = (seconds: number) => new Date(now.getTime() - seconds * 1000).toISOString();

test('relativeTime speaks German', () => {
  assert.equal(relativeTime(ago(30), now), 'gerade eben');
  assert.equal(relativeTime(ago(60), now), 'vor 1 Min.');
  assert.equal(relativeTime(ago(45 * 60), now), 'vor 45 Min.');
  assert.equal(relativeTime(ago(2 * 3600), now), 'vor 2 Std.');
  assert.equal(relativeTime(ago(26 * 3600), now), 'gestern');
  assert.equal(relativeTime(ago(3 * 86400), now), 'vor 3 Tagen');
  assert.equal(relativeTime(ago(14 * 86400), now), 'vor 2 Wochen');
  assert.equal(relativeTime(ago(60 * 86400), now), 'vor 2 Monaten');
});

test('relativeTime handles missing and broken timestamps', () => {
  assert.equal(relativeTime(undefined, now), 'unbekannt');
  assert.equal(relativeTime('kaputt', now), 'unbekannt');
});

test('previewText collapses whitespace and truncates', () => {
  assert.equal(previewText('  a \n  b  '), 'a b');
  assert.equal(previewText('abcdefghij', 5), 'abcd…');
});

test('escapeHtml neutralises markup', () => {
  assert.equal(escapeHtml('<b>&"</b>'), '&lt;b&gt;&amp;&quot;&lt;/b&gt;');
});

test('shellQuote survives quotes in paths', () => {
  assert.equal(shellQuote('/Users/alice/AI/foo'), `'/Users/alice/AI/foo'`);
  assert.equal(shellQuote(`/tmp/it's`), `'/tmp/it'\\''s'`);
});
