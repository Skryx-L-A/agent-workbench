// Decisions about the workbench terminal, kept free of vscode so they can be tested.
import { execFile } from 'node:child_process';
import { shellQuote } from './format.ts';
import type { WorkerLayout } from './settings.ts';
import { exact } from './tmux.ts';

export type TerminalPlan = 'show' | 'launch';

/** Is the tmux client (wb-code) still running under the terminal's shell? */
export type Liveness = 'alive' | 'dead' | 'unknown';

export interface ExistingTerminal {
  /** Created by this extension-host session — a restored terminal is not. */
  createdByUs: boolean;
  /** Its shell process has exited. */
  exited: boolean;
  liveness: Liveness;
}

/**
 * A terminal we launched is attached to tmux and its prompt is the orchestrator's
 * Claude chat — sending 'wb-code ...' there would submit the command as a chat
 * message, so such a terminal is only shown (review finding 6).
 *
 * But VSCode restores its own terminals after the openFolder reload: same name,
 * scrollback of the old session, fresh shell, wb-code long dead. Showing that
 * ghost leaves an empty prompt instead of the orchestrator. Only reuse a terminal
 * that we created AND whose tmux client is provably alive; anything else is
 * relaunched (a second attach is harmless — tmux is multi-client).
 */
export function terminalPlan(existing?: ExistingTerminal): TerminalPlan {
  if (!existing) {
    return 'launch';
  }
  return existing.createdByUs && !existing.exited && existing.liveness === 'alive'
    ? 'show'
    : 'launch';
}

export interface LaunchOptions {
  /** Claude session id to resume (claude harness only, SPEC-V2 F). */
  sessionId?: string;
  /** Session name alice gave it; wb-code passes it on as `claude -n`. */
  name?: string;
  /** Missing for the folder's default session (SPEC-V2 B). */
  sessionKey?: string;
}

/** wb-code [dir] [--resume <id>] [--name <n>] [--key <k>] — SPEC-V2 B. */
export function orchestratorCommand(dir: string, options: LaunchOptions = {}): string {
  const parts = ['wb-code', shellQuote(dir)];
  if (options.sessionId) {
    parts.push('--resume', shellQuote(options.sessionId));
  }
  if (options.name) {
    parts.push('--name', shellQuote(options.name));
  }
  if (options.sessionKey) {
    parts.push('--key', shellQuote(options.sessionKey));
  }
  return parts.join(' ');
}

/**
 * Restoring the orchestrator tab after a window reload ATTACHES, nothing else:
 * plain `tmux attach-session`, never wb-code. wb-code would attach too (it is
 * idempotent), but only after deciding whether to start Claude — and a second
 * Claude instance in alice's session is the one thing that must never happen
 * here. An attach cannot do it.
 */
export function orchestratorAttachCommand(session: string): string {
  return `exec tmux attach-session -t ${shellQuote(exact(session))}`;
}

export interface RestoreState {
  /** A workspace folder is open — without one there is no session to restore. */
  hasFolder: boolean;
  /** The tmux session of this folder/session key is alive. */
  sessionAlive: boolean;
  /** The pending-launch path already started the terminals in this activation. */
  handledPending: boolean;
  layout: WorkerLayout;
}

export interface RestorePlan {
  orchestrator: boolean;
  workerTab: boolean;
}

/**
 * What to re-create after an activation (window reload — which installing a new
 * build requires). Nothing is restored for a dead session: a reload must not
 * resurrect a workbench alice has closed. And nothing is restored when the
 * launch path already ran in this activation, so no tab opens twice.
 */
export function restorePlan(state: RestoreState): RestorePlan {
  const orchestrator = state.hasFolder && state.sessionAlive && !state.handledPending;
  return { orchestrator, workerTab: orchestrator && state.layout === 'window' };
}

/** How long the worker terminal waits for the orchestrator's tmux session. */
export const WORKER_ATTACH_WAIT_S = 30;

/**
 * Command for the "Claude Workbench — Worker" terminal (SPEC-V2 C): it attaches
 * to a GROUPED session ('<sess>-view'), which shares the windows of the
 * orchestrator's session but keeps its own current window, and selects the
 * 'workers' window there.
 *
 * Deviations from the literal command in SPEC-V2 C, all for robustness:
 * the grouped session is created detached and attached separately (so an
 * existing view session is simply attached, as the spec requires), and
 * select-window is best-effort — a failure there must not stop the attach.
 * The wait is bounded: wb-code needs a moment to create the session.
 *
 * `wb-workers-window` runs FIRST and is load-bearing, not cosmetic (measured
 * 2026-07-25): sessions of a group share their windows including their
 * destruction, so a group whose only window is the orchestrator's dies whole —
 * both sessions and the server — when that window is closed. Before the first
 * worker exists there is no 'workers' window, so the helper creates it with a
 * placeholder pane. Without it the worker tab also just showed a second copy of
 * the orchestrator's Claude pane.
 */
export function workerViewCommand(session: string, viewSession: string): string {
  const s = shellQuote(exact(session));
  const view = shellQuote(viewSession);
  const viewTarget = shellQuote(exact(viewSession));
  const workersWindow = shellQuote(`${exact(viewSession)}:workers`);
  return [
    `for i in $(seq 1 ${WORKER_ATTACH_WAIT_S}); do tmux has-session -t ${s} 2>/dev/null && break; sleep 1; done`,
    `wb-workers-window ${shellQuote(session)}`,
    `tmux has-session -t ${viewTarget} 2>/dev/null || tmux new-session -d -t ${s} -s ${view}`,
    `tmux select-window -t ${workersWindow} 2>/dev/null`,
    `exec tmux attach-session -t ${viewTarget}`,
  ].join('; ');
}

export interface Process {
  pid: number;
  ppid: number;
  command: string;
}

/** Parses `ps -axo pid=,ppid=,command=`. */
export function parseProcesses(stdout: string): Process[] {
  const processes: Process[] = [];
  for (const line of stdout.split('\n')) {
    const match = /^\s*(\d+)\s+(\d+)\s+(.*)$/.exec(line);
    if (match) {
      processes.push({ pid: Number(match[1]), ppid: Number(match[2]), command: match[3] });
    }
  }
  return processes;
}

/** wb-code execs `tmux attach`, so the live orchestrator shows up as a tmux child. */
const ORCHESTRATOR_PROCESS = /(^|\/|\s)(tmux|wb-code|claude)(\s|$)/;

/** Does any descendant of `pid` look like our attached orchestrator? */
export function hasOrchestratorDescendant(processes: Process[], pid: number): boolean {
  const children = new Map<number, Process[]>();
  for (const process of processes) {
    const siblings = children.get(process.ppid);
    if (siblings) {
      siblings.push(process);
    } else {
      children.set(process.ppid, [process]);
    }
  }
  const queue = [...(children.get(pid) ?? [])];
  while (queue.length > 0) {
    const process = queue.shift()!;
    if (ORCHESTRATOR_PROCESS.test(process.command)) {
      return true;
    }
    queue.push(...(children.get(process.pid) ?? []));
  }
  return false;
}

/**
 * 'unknown' whenever the answer is not certain (no pid, ps failed) — the caller
 * then relaunches, because an empty shell prompt is the worse failure.
 */
export async function orchestratorLiveness(pid: number | undefined): Promise<Liveness> {
  if (pid === undefined) {
    return 'unknown';
  }
  let stdout: string;
  try {
    stdout = await ps();
  } catch {
    return 'unknown';
  }
  return hasOrchestratorDescendant(parseProcesses(stdout), pid) ? 'alive' : 'dead';
}

function ps(): Promise<string> {
  return new Promise((resolve, reject) => {
    execFile('ps', ['-axo', 'pid=,ppid=,command='], { timeout: 5000, maxBuffer: 8 << 20 },
      (error, stdout) => (error ? reject(error) : resolve(stdout)));
  });
}
