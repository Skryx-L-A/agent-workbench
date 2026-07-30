import assert from 'node:assert/strict';
import { test } from 'node:test';
import { mcpShared } from '../src/systemCli.ts';

// `status` is read-only (no LaunchAgent is started/stopped/restarted) — safe to
// run against the REAL `mcp-shared` on this machine, same contract-test
// reasoning as modelsCli.test.ts's real wb-state calls. start/stop/restart/
// apply/reap are deliberately NOT exercised here: they would touch alice's
// actually-running shared MCP-server LaunchAgents, which no test may do.
test('mcpShared("status") against the real CLI: a well-formed result, whether or not mcp-shared is installed', async () => {
  const result = await mcpShared('status');
  assert.equal(typeof result.ok, 'boolean');
  assert.equal(typeof result.message, 'string');
  assert.ok(result.message.length > 0, 'never an empty message either way');
});
