import assert from 'node:assert/strict';
import { test } from 'node:test';
import { renderHtml, type SessionCard } from '../src/homeHtml.ts';

const card: SessionCard = {
  dir: '/Users/alice/AI/foo',
  name: 'foo',
  folderName: 'foo',
  tmuxSession: 'wb-foo-1a2b3c',
  lastActive: new Date(Date.now() - 2 * 3600 * 1000).toISOString(),
  sessionId: 'abc-123',
  lastUserMessage: 'Baue Teil A',
  workers: [
    { name: 'builder', model: 'fable:medium', status: 'running', paneId: '%3' },
    { name: 'reviewer', status: 'ended' },
  ],
  tmuxAlive: true,
};

test('renderHtml shows project, path, message, workers and the resume button', () => {
  const html = renderHtml([card], 'vscode-resource:/codicon.css', 'vscode-resource:', 'N0NCE');
  assert.match(html, /Sessions fortsetzen/);
  assert.match(html, /foo/);
  assert.match(html, /\/Users\/alice\/AI\/foo/);
  assert.match(html, /Baue Teil A/);
  assert.match(html, /vor 2 Std\./);
  assert.match(html, /builder — läuft/);
  assert.match(html, /reviewer — beendet/);
  assert.match(html, /data-dir="\/Users\/alice\/AI\/foo" data-session="abc-123"/);
  assert.match(html, /Fortsetzen/);
});

test('renderHtml tells two sessions of the same folder apart (SPEC-V2 D)', () => {
  const second: SessionCard = {
    ...card,
    name: 'Refactor-Session',
    sessionKey: '9f2a1c',
    tmuxSession: 'wb-foo-1a2b3c-9f2a1c',
    sessionId: 'def-456',
  };
  const html = renderHtml([card, second], 'c.css', 'vscode-resource:', 'N0NCE');
  // session name is the heading, the folder is shown underneath
  assert.match(html, /<span class="project">Refactor-Session<\/span>/);
  assert.match(html, /<span class="folder">foo<\/span>/);
  // the tmux session name is unique per session and distinguishes the cards
  assert.match(html, /wb-foo-1a2b3c —/);
  assert.match(html, /wb-foo-1a2b3c-9f2a1c —/);
  // resume carries the key, so the right session of the folder is started
  assert.match(html, /data-session="def-456"\s+data-key="9f2a1c" data-name="Refactor-Session"/);
  assert.match(html, /data-session="abc-123"\s+data-key="" data-name="foo"/);
});

test('renderHtml shows harness and model when the state file has them (SPEC-V2 F)', () => {
  const pi: SessionCard = { ...card, harness: 'pi', model: 'ornith' };
  assert.match(
    renderHtml([pi], 'c.css', 'vscode-resource:', 'N0NCE'),
    /<div class="harness">pi \(lokal\) · ornith<\/div>/,
  );
  // old state files carry neither field — the line is simply absent
  assert.ok(!renderHtml([card], 'c.css', 'vscode-resource:', 'N0NCE').includes('class="harness"'));
});

test('renderHtml marks a remote worker with its machine on the badge', () => {
  const remote: SessionCard = {
    ...card,
    workers: [
      { name: 'SSH-builder', model: 'sonnet5:high', status: 'running', paneId: '%3', machine: 'peer' },
      { name: 'reviewer', status: 'running', paneId: '%4' },
    ],
  };
  const html = renderHtml([remote], 'c.css', 'vscode-resource:', 'N0NCE');
  assert.match(html, /SSH-builder — läuft · peer/);
  assert.match(html, /reviewer — läuft</, 'a local worker keeps the V1 badge');
});

test('renderHtml says "Zuordnung unsicher" instead of guessing quietly', () => {
  const shaky: SessionCard = { ...card, transcriptUncertain: true };
  const html = renderHtml([shaky], 'c.css', 'vscode-resource:', 'N0NCE');
  assert.match(html, /Zuordnung unsicher/);
  assert.match(html, /class="uncertain"/);
  // the normal case stays clean (the style block always carries the class name)
  assert.ok(!renderHtml([card], 'c.css', 'vscode-resource:', 'N0NCE').includes('class="uncertain"'));
});

test('renderHtml offers the Einstellungen entry point', () => {
  const html = renderHtml([], 'c.css', 'vscode-resource:', 'N0NCE');
  assert.match(html, /id="settings"/);
  assert.match(html, /Einstellungen/);
});

test('renderHtml escapes user content', () => {
  const evil: SessionCard = { ...card, lastUserMessage: '<img src=x onerror=alert(1)>', workers: [] };
  const html = renderHtml([evil], 'c.css', 'vscode-resource:', 'N0NCE');
  assert.ok(!html.includes('<img src=x'), 'raw markup leaked into the webview');
  assert.match(html, /&lt;img src=x/);
});

test('renderHtml has an empty state and no emoji', () => {
  const html = renderHtml([], 'c.css', 'vscode-resource:', 'N0NCE');
  assert.match(html, /Noch keine Sessions/);
  assert.ok(!/\p{Extended_Pictographic}/u.test(html), 'emoji found in the Startseite');
});

test('renderHtml locks scripts to the nonce (no unsafe-inline)', () => {
  const html = renderHtml([], 'c.css', 'vscode-resource:', 'N0NCE');
  assert.match(html, /script-src 'nonce-N0NCE'/);
  assert.match(html, /<script nonce="N0NCE">/);
  assert.ok(!/script-src[^;]*unsafe-inline/.test(html), 'inline scripts still allowed');
});

test('renderHtml renders the Mac|Peer switch with the active machine marked', () => {
  const html = renderHtml([], 'c.css', 'vscode-resource:', 'N0NCE',
    { machine: 'peer', reachable: true, loading: false });
  assert.match(html, /data-machine="mac"/);
  assert.match(html, /data-machine="peer"/);
  assert.match(html, /class="machine active" data-machine="peer"/);
  assert.match(html, /Sessions fortsetzen — Peer \(Linux-PC\)/);
});

test('renderHtml shows a loading notice while Peer state is fetched', () => {
  const html = renderHtml([], 'c.css', 'vscode-resource:', 'N0NCE',
    { machine: 'peer', reachable: true, loading: true });
  assert.match(html, /über SSH geladen/);
});

test('renderHtml shows an unreachable notice for Peer, escaping the error', () => {
  const html = renderHtml([], 'c.css', 'vscode-resource:', 'N0NCE',
    { machine: 'peer', reachable: false, loading: false, error: '<timeout>' });
  assert.match(html, /Peer ist nicht erreichbar/);
  assert.match(html, /&lt;timeout&gt;/);
  assert.ok(!html.includes('<timeout>'), 'raw error leaked into the webview');
});
