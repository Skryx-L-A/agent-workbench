import assert from 'node:assert/strict';
import { test } from 'node:test';
import { isSyntheticUserText, lastUserMessage, transcriptName } from '../src/claudeLog.ts';

const line = (entry: unknown) => JSON.stringify(entry);

test('lastUserMessage returns the newest real user turn', () => {
  const jsonl = [
    line({ type: 'user', message: { role: 'user', content: 'erste Frage' } }),
    line({ type: 'assistant', message: { role: 'assistant', content: 'Antwort' } }),
    line({ type: 'user', message: { role: 'user', content: 'zweite Frage' } }),
  ].join('\n');
  assert.equal(lastUserMessage(jsonl), 'zweite Frage');
});

test('lastUserMessage skips hooks, meta, sidechains and tool results', () => {
  const jsonl = [
    line({ type: 'user', message: { role: 'user', content: 'echte Frage' } }),
    line({ type: 'user', isMeta: true, message: { role: 'user', content: 'meta' } }),
    line({ type: 'user', attachment: { type: 'hook_success' }, message: { role: 'user', content: 'hook' } }),
    line({ type: 'user', isSidechain: true, message: { role: 'user', content: 'subagent' } }),
    line({ type: 'user', message: { role: 'user', content: [{ type: 'tool_result', content: 'ok' }] } }),
  ].join('\n');
  assert.equal(lastUserMessage(jsonl), 'echte Frage');
});

test('lastUserMessage strips system-reminder blocks and joins text blocks', () => {
  const jsonl = line({
    type: 'user',
    message: {
      role: 'user',
      content: [
        { type: 'text', text: '<system-reminder>ignorieren</system-reminder>Baue Teil A' },
        { type: 'text', text: 'und teste es' },
      ],
    },
  });
  assert.equal(lastUserMessage(jsonl), 'Baue Teil A\nund teste es');
});

test('lastUserMessage tolerates partial and non-JSON lines', () => {
  const jsonl = [
    '{"type":"user","message":{"role":"user","content":"tru',
    line({ type: 'user', message: { role: 'user', content: 'ganze Zeile' } }),
  ].join('\n');
  assert.equal(lastUserMessage(jsonl), 'ganze Zeile');
});

test('lastUserMessage returns undefined when there is no user turn', () => {
  assert.equal(lastUserMessage(line({ type: 'assistant', message: { content: 'x' } })), undefined);
});

test('isSyntheticUserText recognises the machinery Claude writes as a user turn', () => {
  // shapes taken from the real transcripts under ~/.claude/projects/-Users-alice-AI/
  assert.ok(isSyntheticUserText('<task-notification>\n<task-id>bc2g4difu</task-id>\n</task-notification>'));
  assert.ok(isSyntheticUserText('[SYSTEM NOTIFICATION] Background task finished'));
  assert.ok(isSyntheticUserText('<local-command-stdout>Compacted </local-command-stdout>'));
  assert.ok(isSyntheticUserText('<command-name>/clear</command-name>\n<command-message>clear</command-message>'));
  assert.ok(isSyntheticUserText('Caveat: The messages below were generated while running local commands.'));
  assert.ok(isSyntheticUserText('[Image: original 2624x1824, displayed at 2000x1390.]'));
  assert.ok(isSyntheticUserText('   '));
  assert.ok(!isSyntheticUserText('go on'));
  assert.ok(!isSyntheticUserText('der dritte opus worker soll das review machen'));
});

test('lastUserMessage skips background-task notifications back to the real turn', () => {
  const jsonl = [
    line({ type: 'user', message: { role: 'user', content: 'go on' } }),
    line({ type: 'assistant', message: { role: 'assistant', content: 'ok' } }),
    line({ type: 'user', message: { role: 'user', content: [{ type: 'tool_result', content: 'x' }] } }),
    line({ type: 'user', message: { role: 'user', content: '<task-notification>\n<task-id>bc2g4difu</task-id>\n</task-notification>' } }),
    line({ type: 'user', message: { role: 'user', content: [{ type: 'text', text: '[SYSTEM NOTIFICATION] task done' }] } }),
    line({ type: 'user', message: { role: 'user', content: '<local-command-stdout>Compacted </local-command-stdout>' } }),
    line({ type: 'user', message: { role: 'user', content: '<command-name>/clear</command-name>' } }),
  ].join('\n');
  assert.equal(lastUserMessage(jsonl), 'go on');
});

test('lastUserMessage keeps a real turn that only carries a system-reminder tail', () => {
  const jsonl = line({
    type: 'user',
    message: { role: 'user', content: 'der dritte opus worker\n<system-reminder>ignorieren</system-reminder>' },
  });
  assert.equal(lastUserMessage(jsonl), 'der dritte opus worker');
});

test('lastUserMessage returns undefined when only synthetic turns exist', () => {
  const jsonl = line({ type: 'user', message: { role: 'user', content: '<task-notification>x</task-notification>' } });
  assert.equal(lastUserMessage(jsonl), undefined);
});

test('transcriptName reads the session name Claude Code writes at the top', () => {
  // verified shape (claude -n WbNameProbe, 2026-07-25):
  const head = [
    line({ type: 'custom-title', customTitle: 'Refactor-Session', sessionId: 'abc' }),
    line({ type: 'agent-name', agentName: 'Refactor-Session', sessionId: 'abc' }),
    line({ type: 'user', message: { role: 'user', content: 'los' } }),
  ].join('\n');
  assert.equal(transcriptName(head), 'Refactor-Session');
});

test('transcriptName returns undefined for an unnamed transcript', () => {
  assert.equal(transcriptName(line({ type: 'user', message: { role: 'user', content: 'x' } })), undefined);
  assert.equal(transcriptName(''), undefined);
  // a head cut mid-line must not throw
  assert.equal(transcriptName('{"type":"custom-tit'), undefined);
});
