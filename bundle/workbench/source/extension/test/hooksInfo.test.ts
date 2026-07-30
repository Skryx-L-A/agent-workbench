import assert from 'node:assert/strict';
import { test } from 'node:test';
import { claudeSettingsFile, parseHooks, readHooks } from '../src/hooksInfo.ts';

test('parseHooks flattens event/matcher/hooks into rows', () => {
  const rows = parseHooks(JSON.stringify({
    hooks: {
      PreToolUse: [
        { matcher: 'Bash', hooks: [{ type: 'command', command: 'bash guard.sh', timeout: 5 }] },
        { hooks: [{ type: 'command', command: 'bash other.sh' }] },
      ],
      SessionStart: [{ hooks: [{ type: 'command', command: 'bash start.sh' }] }],
    },
  }));
  assert.equal(rows.length, 3);
  assert.deepEqual(rows[0], {
    event: 'PreToolUse', matcher: 'Bash', command: 'bash guard.sh', timeout: 5,
    isDenyHook: false, denyHookReason: undefined,
  });
  assert.equal(rows[1].matcher, undefined);
  assert.equal(rows[2].event, 'SessionStart');
});

// CLAUDE.md names these three explicitly: "Die drei Deny-Hooks (Secrets,
// Kill-Muster, Push-Gate) bekommen einen deutlichen Warnhinweis".
test('parseHooks flags the three deny-hooks by filename, with a reason', () => {
  const rows = parseHooks(JSON.stringify({
    hooks: {
      PreToolUse: [{
        matcher: 'Bash',
        hooks: [
          { type: 'command', command: 'bash "$HOME/.claude/hooks/bash-guard-secrets.sh"' },
          { type: 'command', command: 'bash "$HOME/.claude/hooks/bash-guard-kill-pattern.sh"' },
          { type: 'command', command: 'bash "$HOME/.claude/hooks/push-gate-worker.sh"' },
          { type: 'command', command: 'bash "$HOME/.claude/hooks/media-cloud-guard.sh"' },
        ],
      }],
    },
  }));
  const denyCommands = rows.filter((r) => r.isDenyHook).map((r) => r.command);
  assert.equal(denyCommands.length, 3);
  assert.ok(denyCommands[0].includes('bash-guard-secrets.sh'));
  const mediaGuard = rows.find((r) => r.command.includes('media-cloud-guard.sh'))!;
  assert.equal(mediaGuard.isDenyHook, false, 'only the three named hooks are flagged, not every guard');
  const secretsGuard = rows.find((r) => r.command.includes('bash-guard-secrets.sh'))!;
  assert.match(secretsGuard.denyHookReason!, /Secret/);
});

test('parseHooks never throws: missing/broken/empty input all become an empty list', () => {
  assert.deepEqual(parseHooks(undefined), []);
  assert.deepEqual(parseHooks('not json'), []);
  assert.deepEqual(parseHooks('[]'), []);
  assert.deepEqual(parseHooks('{}'), []);
  assert.deepEqual(parseHooks(JSON.stringify({ hooks: 'not an object' })), []);
  assert.deepEqual(parseHooks(JSON.stringify({ hooks: { X: 'not an array' } })), []);
  assert.deepEqual(parseHooks(JSON.stringify({ hooks: { X: [{ hooks: 'not an array' }] } })), []);
  assert.deepEqual(parseHooks(JSON.stringify({ hooks: { X: [{ hooks: [{ type: 'command' }] }] } })), [], 'a hook without a command is skipped');
});

test('claudeSettingsFile points at ~/.claude/settings.json', () => {
  assert.equal(claudeSettingsFile('/Users/alice'), '/Users/alice/.claude/settings.json');
});

test('readHooks against the real ~/.claude/settings.json on this machine: well-formed, never throws', async () => {
  const rows = await readHooks();
  assert.ok(Array.isArray(rows));
  for (const row of rows) {
    assert.equal(typeof row.event, 'string');
    assert.equal(typeof row.command, 'string');
    assert.equal(typeof row.isDenyHook, 'boolean');
  }
});
