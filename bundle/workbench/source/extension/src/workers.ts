// Merges the (possibly stale) worker list from the state file with live tmux
// panes and the worker's latest result file.
import { open, stat } from 'node:fs/promises';
import { homedir } from 'node:os';
import { join } from 'node:path';
import { relativeTime } from './format.ts';
import { readRemoteResults } from './remoteResults.ts';
import { readState, type SessionState, type StateWorker } from './state.ts';
import { listWorkerPanes, sessionName } from './tmux.ts';

export type WorkerStatus = 'running' | 'dead' | 'ended';

export interface WorkerView {
  name: string;
  kind?: string;
  model?: string;
  status: WorkerStatus;
  paneId?: string;
  result?: WorkerResult;
  /**
   * Machine the worker runs on ('peer' for a `claude-worker --on peer` worker,
   * which is called SSH-<name> everywhere). Absent = local, as in V1.
   */
  machine?: string;
  /** Set when a remote result could not be read (SSH down, timeout). */
  resultUnreachable?: boolean;
}

export interface WorkerResult {
  firstLine: string;
  mtime: number;
}

/**
 * The extension host always runs on the Mac (extensionKind "ui", Design C), so
 * a worker entry marked 'peer' is the remote case for everything in here.
 */
export const LOCAL_MACHINE = 'mac';

export function isRemoteWorker(worker: { machine?: string }): boolean {
  return worker.machine !== undefined && worker.machine !== LOCAL_MACHINE;
}

export const STATUS_LABEL: Record<WorkerStatus, string> = {
  running: 'läuft',
  dead: 'Pane tot',
  ended: 'beendet',
};

export function resultsDir(name: string): string {
  return join(homedir(), '.pi-workers', 'results', name);
}

/** State-file workers plus any live pane not (yet) in the state file. */
export function mergeWorkers(
  stateWorkers: StateWorker[],
  panes: { paneId: string; worker: string; dead: boolean }[],
): WorkerView[] {
  const byName = new Map<string, WorkerView>();
  for (const worker of stateWorkers) {
    byName.set(worker.name, {
      name: worker.name,
      kind: worker.kind,
      model: worker.model,
      machine: worker.machine,
      status: 'ended',
    });
  }
  for (const pane of panes) {
    const existing = byName.get(pane.worker);
    const status: WorkerStatus = pane.dead ? 'dead' : 'running';
    if (existing) {
      existing.status = status;
      existing.paneId = pane.paneId;
    } else {
      byName.set(pane.worker, { name: pane.worker, status, paneId: pane.paneId });
    }
  }
  return [...byName.values()].sort((a, b) => a.name.localeCompare(b.name, 'de'));
}

/**
 * Tooltip line about the worker's result. For a remote worker it never claims
 * "kein Result": the file lives on the other machine, so it says so — including
 * when the machine could not be reached.
 */
export function resultTooltip(worker: WorkerView): string {
  const when = worker.result
    ? relativeTime(new Date(worker.result.mtime).toISOString())
    : undefined;
  if (!isRemoteWorker(worker)) {
    return worker.result
      ? `Letztes Result (${when}): ${worker.result.firstLine}`
      : 'Kein Result vorhanden.';
  }
  if (worker.result) {
    return `Letztes Result auf ${worker.machine} (${when}): ${worker.result.firstLine}`;
  }
  return worker.resultUnreachable
    ? `Result liegt auf ${worker.machine} — Maschine nicht erreichbar (ssh ${worker.machine}).`
    : `Noch kein Result auf ${worker.machine} (~/.pi-workers/results/${worker.name}/latest.md).`;
}

/** mtime + first line of ~/.pi-workers/results/<name>/latest.md */
export async function readResult(name: string): Promise<WorkerResult | undefined> {
  const file = join(resultsDir(name), 'latest.md');
  try {
    const info = await stat(file);
    const handle = await open(file, 'r');
    try {
      const buffer = Buffer.alloc(512);
      const { bytesRead } = await handle.read(buffer, 0, 512, 0);
      const firstLine = buffer
        .toString('utf8', 0, bytesRead)
        .split('\n')
        .map((line) => line.replace(/^#+\s*/, '').trim())
        .find((line) => line.length > 0);
      return { firstLine: firstLine ?? '', mtime: info.mtimeMs };
    } finally {
      await handle.close();
    }
  } catch {
    return undefined;
  }
}

/**
 * Result files of remote workers live on the TARGET machine, so they are read
 * over SSH — server-side, exactly like wb-result does, never copied here. One
 * call per machine per refresh instead of one per worker, and cached briefly so
 * a fast sidebar poll does not turn into an SSH storm.
 */
export const REMOTE_RESULT_TTL_MS = 15_000;

interface RemoteResultCache {
  fetchedAt: number;
  results: Map<string, WorkerResult>;
  reachable: boolean;
}

const remoteResults = new Map<string, RemoteResultCache>();

/** Only exported for tests — drops the cached remote results. */
export function clearRemoteResultCache(): void {
  remoteResults.clear();
}

async function remoteResultsFor(
  machine: string,
  names: string[],
  now: number,
): Promise<RemoteResultCache> {
  const cached = remoteResults.get(machine);
  if (cached && now - cached.fetchedAt < REMOTE_RESULT_TTL_MS) {
    return cached;
  }
  let fresh: RemoteResultCache;
  try {
    fresh = { fetchedAt: now, results: await readRemoteResults(names, machine), reachable: true };
  } catch {
    // SSH down or timed out: keep quiet, the sidebar says where the result is.
    fresh = { fetchedAt: now, results: new Map(), reachable: false };
  }
  remoteResults.set(machine, fresh);
  return fresh;
}

/**
 * Full worker view for one SESSION: state file + live panes + result files.
 * `sessionKey` selects which session of the folder is meant (SPEC-V2 B/D);
 * without it the folder's default session is read, as in V1.
 */
export async function loadWorkers(dir: string, sessionKey?: string): Promise<WorkerView[]> {
  const state: SessionState | undefined = await readState(dir, sessionKey);
  const session = state?.tmuxSession ?? sessionName(dir, sessionKey);
  const panes = await listWorkerPanes(session);
  const workers = mergeWorkers(state?.workers ?? [], panes);
  const remote = workers.filter(isRemoteWorker);
  if (remote.length > 0) {
    // Today every remote worker of a Mac session lives on peer; group anyway so
    // a third machine would not need a second code path.
    for (const machine of new Set(remote.map((w) => w.machine!))) {
      const names = remote.filter((w) => w.machine === machine).map((w) => w.name);
      const cache = await remoteResultsFor(machine, names, Date.now());
      for (const worker of remote.filter((w) => w.machine === machine)) {
        worker.result = cache.results.get(worker.name);
        worker.resultUnreachable = !cache.reachable;
      }
    }
  }
  for (const worker of workers) {
    if (!isRemoteWorker(worker)) {
      worker.result = await readResult(worker.name);
    }
  }
  return workers;
}
