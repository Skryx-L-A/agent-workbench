import assert from 'node:assert/strict';
import { test } from 'node:test';
import {
  aliveSessionNames,
  buildRemoteCards,
  parseRemoteDirs,
  parseRemotePreviews,
  parseRemoteReport,
  previewRequests,
  remotePreviewScript,
  remoteReportScript,
  type RemotePreview,
  sshArgs,
} from '../src/remoteState.ts';

test('sshArgs runs batch-mode with a connect timeout', () => {
  const args = sshArgs('peer', 'echo hi');
  assert.deepEqual(args, [
    '-o', 'BatchMode=yes',
    '-o', 'ConnectTimeout=8',
    'peer', 'echo hi',
  ]);
});

test('remoteReportScript lists sessions then panes', () => {
  const script = remoteReportScript();
  assert.match(script, /\.claude\/workbench\/sessions/);
  assert.match(script, /tmux list-panes -a/);
});

const REPORT = `@@STATES@@
@@FILE@@
{"dir":"/home/alice/AI/Demo","tmuxSession":"wb-Demo-37b117","lastActive":"2026-07-19T13:05:09Z","workers":[{"name":"builder","model":"opus:xhigh"}]}
@@FILE@@
{"dir":"/home/alice/AI/Uebersetzer","lastActive":"2026-07-18T10:00:00Z","workers":[]}
@@PANES@@
wb-Demo-37b117|%0||0
wb-Demo-37b117|%5|builder|0
main|%9||0
`;

test('parseRemoteReport splits state files and keeps the pane text', () => {
  const report = parseRemoteReport(REPORT);
  assert.deepEqual(report.states.map((s) => s.dir), [
    '/home/alice/AI/Demo', // newest lastActive first
    '/home/alice/AI/Uebersetzer',
  ]);
  assert.match(report.paneText, /wb-Demo-37b117\|%5\|builder\|0/);
  assert.ok(!report.paneText.includes('@@PANES@@'));
});

const REPORT_V2 = `@@STATES@@
@@FILE@@-home-alice-AI-Demo.json
{"dir":"/home/alice/AI/Demo","tmuxSession":"wb-Demo-37b117","lastActive":"2026-07-19T13:05:09Z"}
@@FILE@@-home-alice-AI-Demo__9f2a1c.json
{"dir":"/home/alice/AI/Demo","name":"Atlas","harness":"pi","model":"ornith","lastActive":"2026-07-19T14:00:00Z"}
@@PANES@@
wb-Demo-37b117|%0||0
`;

test('parseRemoteReport takes the session key from the reported file name', () => {
  const report = parseRemoteReport(REPORT_V2);
  const [named, legacy] = report.states; // newest lastActive first
  assert.equal(named.name, 'Atlas');
  assert.equal(named.sessionKey, '9f2a1c', 'key comes from the file name');
  assert.equal(named.harness, 'pi');
  assert.equal(legacy.sessionKey, undefined, 'the default session has none');
});

test('buildRemoteCards keeps two sessions of one folder apart (SPEC-V2 B/D)', () => {
  const cards = buildRemoteCards(parseRemoteReport(REPORT_V2), new Map());
  assert.deepEqual(cards.map((c) => c.name), ['Atlas', 'Demo']);
  assert.deepEqual(cards.map((c) => c.folderName), ['Demo', 'Demo']);
  assert.equal(cards[0].tmuxSession, 'wb-Demo-37b117-9f2a1c', 'computed from dir + key');
  assert.equal(cards[0].tmuxAlive, false);
  assert.equal(cards[1].tmuxSession, 'wb-Demo-37b117', 'state file wins for the default one');
  assert.equal(cards[1].tmuxAlive, true);
  assert.equal(cards[0].model, 'ornith');
});

test('parseRemoteReport tolerates an empty sessions directory', () => {
  const report = parseRemoteReport('@@STATES@@\n@@PANES@@\n');
  assert.deepEqual(report.states, []);
});

test('aliveSessionNames collects the distinct session names', () => {
  const names = aliveSessionNames('wb-Demo-37b117|%0||0\nmain|%9||0\nwb-Demo-37b117|%5|builder|0\n');
  assert.deepEqual([...names].sort(), ['main', 'wb-Demo-37b117']);
});

test('remotePreviewScript asks per SESSION, quoting slug and name', () => {
  assert.equal(remotePreviewScript([]), 'true');
  const script = remotePreviewScript([
    { slug: '-home-alice-AI-Demo', name: 'Atlas' },
    { slug: '-home-alice-AI-Demo' },
  ]);
  // one call per request, keyed by index — no name has to survive the round trip
  assert.match(script, /wbprev '0' '-home-alice-AI-Demo' 'Atlas'/);
  assert.match(script, /wbprev '1' '-home-alice-AI-Demo' ''/);
  // the remote side matches on the title Claude Code writes, newest first
  assert.match(script, /grep -qF "\\"customTitle\\":\\"\$want\\""/);
  assert.match(script, /how=named/);
  assert.match(script, /base64/);
});

test('remotePreviewScript survives a session name with a quote', () => {
  const script = remotePreviewScript([{ slug: '-home-alice-AI-foo', name: `Alex's Session` }]);
  assert.match(script, /'Alex'\\''s Session'/);
});

test('previewRequests keeps the order of the states (index is the key)', () => {
  const requests = previewRequests([
    { dir: '/home/alice/AI/foo', name: 'Atlas' },
    { dir: '/home/alice/AI/foo' },
  ]);
  assert.deepEqual(requests, [
    { slug: '-home-alice-AI-foo', name: 'Atlas' },
    { slug: '-home-alice-AI-foo', name: undefined },
  ]);
});

test('parseRemotePreviews decodes the tail and reports how the file was chosen', () => {
  const jsonl = JSON.stringify({ type: 'user', message: { content: 'Starte den Backtest' } });
  const b64 = Buffer.from(jsonl + '\n', 'utf8').toString('base64');
  const out = `@@PREV@@0\tabc-123\tnamed\n@@PREVB64@@${b64}\n`
    + `@@PREV@@1\tdef-456\tnewest\n@@PREVB64@@${b64}\n`;
  const previews = parseRemotePreviews(out);
  assert.equal(previews.get(0)?.sessionId, 'abc-123');
  assert.equal(previews.get(0)?.lastUserMessage, 'Starte den Backtest');
  assert.equal(previews.get(0)?.nameMatched, true);
  assert.equal(previews.get(1)?.sessionId, 'def-456');
  assert.equal(previews.get(1)?.nameMatched, false, 'fell back to the newest transcript');
});

test('buildRemoteCards mirrors the local cards: preview, live worker, tmux flag', () => {
  const report = parseRemoteReport(REPORT);
  const trading0 = report.states.findIndex((s) => s.dir === '/home/alice/AI/Demo');
  const previews = new Map<number, RemotePreview>([
    [trading0, { sessionId: 'abc-123', lastUserMessage: 'Weiter', nameMatched: false }],
  ]);
  const cards = buildRemoteCards(report, previews);
  const trading = cards.find((c) => c.name === 'Demo')!;
  assert.equal(trading.dir, '/home/alice/AI/Demo');
  assert.equal(trading.sessionId, 'abc-123');
  assert.equal(trading.lastUserMessage, 'Weiter');
  assert.equal(trading.tmuxAlive, true);
  assert.equal(trading.workers.find((w) => w.name === 'builder')?.status, 'running');

  const ueber = cards.find((c) => c.name === 'Uebersetzer')!;
  assert.equal(ueber.tmuxAlive, false, 'no pane for its session');
  assert.equal(ueber.sessionId, undefined);
  // one session per folder: the newest transcript is the right one, no warning
  assert.equal(trading.transcriptUncertain, false);
});

test('buildRemoteCards gives two sessions of one folder their OWN preview', () => {
  const report = parseRemoteReport(REPORT_V2);
  const previews = new Map<number, RemotePreview>([
    [0, { sessionId: 'atlas-1', lastUserMessage: 'Atlas weiter', nameMatched: true }],
    [1, { sessionId: 'haupt-1', lastUserMessage: 'Hauptsession weiter', nameMatched: true }],
  ]);
  const [named, legacy] = buildRemoteCards(report, previews);
  assert.equal(named.sessionId, 'atlas-1');
  assert.equal(named.lastUserMessage, 'Atlas weiter');
  assert.equal(legacy.sessionId, 'haupt-1');
  assert.equal(legacy.lastUserMessage, 'Hauptsession weiter');
  assert.equal(named.transcriptUncertain, false);
});

test('buildRemoteCards flags an unresolved transcript instead of guessing quietly', () => {
  const report = parseRemoteReport(REPORT_V2);
  const previews = new Map<number, RemotePreview>([
    [0, { sessionId: 'x-1', nameMatched: false }],
    [1, { sessionId: 'x-1', nameMatched: false }],
  ]);
  const cards = buildRemoteCards(report, previews);
  assert.equal(cards[0].transcriptUncertain, true, 'two sessions, no name match');
  assert.equal(cards[1].transcriptUncertain, true);
});

test('parseRemoteDirs trims and drops blanks', () => {
  assert.deepEqual(
    parseRemoteDirs('/home/alice/AI/Demo\n/home/alice/AI/Memory system\n\n'),
    ['/home/alice/AI/Demo', '/home/alice/AI/Memory system'],
  );
});
