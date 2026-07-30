// `mcp-shared` control (Nachtrag priority 3): the shared MCP-server LaunchAgents
// (basic-memory/playwright, ~/.claude/CLAUDE.md "MCP-Server laufen geteilt").
// Read-only status plus the four control verbs the CLI itself exposes — this
// module only ever shells out to the ALREADY-EXISTING `mcp-shared` tool, it
// never touches a config file directly (same arm's-length pattern as
// modelsCli.ts's wb-state calls).
import { execFile } from 'node:child_process';
import { describeCliError } from './modelsCli.ts';

export type McpSharedAction = 'status' | 'start' | 'stop' | 'restart' | 'apply' | 'reap';

export interface CliResult {
  ok: boolean;
  message: string;
}

interface ExecError {
  code?: string;
  stderr?: string;
  stdout?: string;
  message?: string;
}

function run(command: string, args: string[], timeoutMs: number): Promise<string> {
  return new Promise((resolve, reject) => {
    const child = execFile(
      command,
      args,
      { timeout: timeoutMs, maxBuffer: 4 << 20 },
      (err, stdout, stderr) => {
        if (err) {
          reject(Object.assign(err, { stderr, stdout }));
          return;
        }
        resolve(stdout);
      },
    );
    child.stdin?.end();
  });
}

export async function mcpShared(action: McpSharedAction, timeoutMs = 15_000): Promise<CliResult> {
  try {
    const stdout = await run('mcp-shared', [action], timeoutMs);
    return { ok: true, message: stdout.trim() || 'OK.' };
  } catch (error) {
    return {
      ok: false,
      message: describeCliError(error as ExecError, 'mcp-shared nicht gefunden (~/.local/bin im PATH?).'),
    };
  }
}
