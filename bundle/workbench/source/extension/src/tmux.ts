// All tmux interaction lives here. Commands are exactly the ones from SPEC.md.
import { execFile } from 'node:child_process';
import { createHash } from 'node:crypto';
import { basename } from 'node:path';

export interface WorkerPane {
  paneId: string;
  worker: string;
  dead: boolean;
}

export interface RolePane {
  paneId: string;
  role: string;
  dead: boolean;
}

export type OrchestratorLookup =
  | { status: 'ok'; paneId: string }
  | { status: 'dead' }
  | { status: 'missing' };

/**
 * Session name for a project dir when no state file exists yet:
 * wb-<sanitized basename>-<md5-6 of the full path>, plus '-<sessionKey>' for
 * every session beyond the folder's default one (SPEC-V2 B). Special characters
 * are REPLACED (not dropped), and the path hash keeps two projects with the same
 * basename apart. Must stay identical to wb-code — see SPEC.md, V1.1.
 *
 * The state file's `tmuxSession` always wins over this; callers pass it in.
 */
export function sessionName(dir: string, sessionKey?: string): string {
  const name = basename(dir).replace(/[^a-zA-Z0-9-]/g, '-');
  const hash = createHash('md5').update(dir).digest('hex').slice(0, 6);
  const key = sessionKey ? `-${sessionKey}` : '';
  return `wb-${name}-${hash}${key}`;
}

/**
 * Grouped session the worker terminal attaches to (SPEC-V2 C). Clients of the
 * SAME tmux session always see the same window, so a second view needs its own
 * session that shares the windows.
 */
export function viewSessionName(session: string): string {
  return `${session}-view`;
}

/**
 * tmux falls back to PREFIX matching when a target does not exist verbatim, so
 * '-t wb-Vox' would happily hit a running 'wb-VoxType'. '=' anchors the name.
 *
 * It anchors reliably for has-session. For 'list-panes -s' it does NOT (tmux
 * 3.7b resolves the target as a WINDOW there and still prefix-matches the
 * session, verified on this machine) — that is why the pane listings below go
 * through 'list-panes -a' and filter on #{session_name} instead of a target.
 */
export function exact(session: string): string {
  return '=' + session;
}

function run(args: string[], stdin?: string): Promise<string> {
  return new Promise((resolve, reject) => {
    const child = execFile('tmux', args, { timeout: 5000 }, (err, stdout) => {
      if (err) {
        reject(err);
        return;
      }
      resolve(stdout);
    });
    if (stdin !== undefined) {
      child.stdin?.end(stdin);
    }
  });
}

/** Runs tmux, returning '' instead of throwing (missing session, tmux not running, ...). */
async function runQuiet(args: string[], stdin?: string): Promise<string> {
  try {
    return await run(args, stdin);
  } catch {
    return '';
  }
}

export function hasSessionArgs(session: string): string[] {
  return ['has-session', '-t', exact(session)];
}

/** Pane options are empty when unset, so fields are pipe-separated, not space-separated. */
export const WORKER_FORMAT = '#{session_name}|#{pane_id}|#{@wb_worker}|#{pane_dead}';
export const ROLE_FORMAT = '#{session_name}|#{pane_id}|#{@wb_role}|#{pane_dead}';

export function listPanesArgs(format: string): string[] {
  return ['list-panes', '-a', '-F', format];
}

export async function hasSession(session: string): Promise<boolean> {
  try {
    await run(hasSessionArgs(session));
    return true;
  } catch {
    return false;
  }
}

export async function listWorkerPanes(session: string): Promise<WorkerPane[]> {
  return parseWorkerPanes(await runQuiet(listPanesArgs(WORKER_FORMAT)), session);
}

export function parseWorkerPanes(out: string, session: string): WorkerPane[] {
  const panes: WorkerPane[] = [];
  for (const line of out.split('\n')) {
    const [name, paneId, worker, dead] = line.replace(/\r?$/, '').split('|');
    if (name !== session || !paneId || !worker) {
      continue;
    }
    panes.push({ paneId, worker, dead: dead === '1' });
  }
  return panes;
}

export async function listRolePanes(session: string): Promise<RolePane[]> {
  return parseRolePanes(await runQuiet(listPanesArgs(ROLE_FORMAT)), session);
}

export function parseRolePanes(out: string, session: string): RolePane[] {
  const panes: RolePane[] = [];
  for (const line of out.split('\n')) {
    const [name, paneId, role, dead] = line.replace(/\r?$/, '').split('|');
    if (name !== session || !paneId || !role) {
      continue;
    }
    panes.push({ paneId, role, dead: dead === '1' });
  }
  return panes;
}

/** wb-code sets remain-on-exit, so a crashed orchestrator stays listed as a dead pane. */
export function pickOrchestrator(panes: RolePane[]): OrchestratorLookup {
  const orchestrators = panes.filter((p) => p.role === 'orchestrator');
  if (orchestrators.length === 0) {
    return { status: 'missing' };
  }
  const alive = orchestrators.find((p) => !p.dead);
  return alive ? { status: 'ok', paneId: alive.paneId } : { status: 'dead' };
}

export async function findOrchestratorPane(session: string): Promise<OrchestratorLookup> {
  return pickOrchestrator(await listRolePanes(session));
}

export function killSessionArgs(session: string): string[] {
  return ['kill-session', '-t', exact(session)];
}

/**
 * Drops the grouped view session behind the worker tab (SPEC-V2 C). Sessions of
 * a group SHARE their windows, so killing this one destroys no window and no
 * pane — the orchestrator session keeps them. Quiet: if it is already gone,
 * there is nothing to report.
 */
export async function killViewSession(session: string): Promise<void> {
  await runQuiet(killSessionArgs(viewSessionName(session)));
}

/** Brings a pane into view: select its window, then the pane itself. */
export async function focusPane(paneId: string): Promise<void> {
  await run(['select-window', '-t', paneId]);
  await run(['select-pane', '-t', paneId]);
}

/**
 * Types text into a pane WITHOUT pressing Enter — the user completes the
 * prompt and submits it. Multi-line text goes through a paste buffer with
 * bracketed paste so the receiving app does not treat newlines as submits.
 */
export async function sendText(paneId: string, text: string): Promise<void> {
  if (text.includes('\n')) {
    // Unique buffer: a second send must not overwrite ours before we paste it.
    const buffer = `wb-send-${process.pid}-${Date.now()}`;
    await run(['load-buffer', '-b', buffer, '-'], text);
    await run(['paste-buffer', '-d', '-p', '-b', buffer, '-t', paneId]);
    return;
  }
  await run(['send-keys', '-t', paneId, '-l', '--', text]);
}
