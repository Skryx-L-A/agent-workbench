// Reading Peer session state from the Mac home page over short, keyless SSH
// calls (no daemon). Only needed BEFORE connecting: once VSCode is in the remote
// peer window, tmux.ts / workers.ts / claudeLog.ts run natively on peer through
// the remote extension host, so no SSH is involved there.
//
// The command builders and the output parsers are pure and unit-tested; the two
// thin execFile wrappers are the only impure part.
import { basename } from 'node:path';
import { lastUserMessage } from './claudeLog.ts';
import { shellQuote } from './format.ts';
import { runSsh } from './ssh.ts';
import {
  claudeProjectSlug,
  isSessionKey,
  isTranscriptUncertain,
  parseState,
  sessionDisplayName,
  sessionKeyFromFileName,
  type SessionState,
  sortByLastActive,
  transcriptRisk,
} from './state.ts';
import { mergeWorkers } from './workers.ts';
import { parseWorkerPanes, sessionName, WORKER_FORMAT } from './tmux.ts';
import type { SessionCard } from './homeHtml.ts';

// The SSH plumbing moved to ssh.ts; re-exported so callers keep one import.
export { sshArgs, SSH_CONNECT_TIMEOUT_S, SSH_TIMEOUT_MS } from './ssh.ts';

/** Tail of each transcript pulled for the preview — enough for the last turn. */
export const PREVIEW_TAIL_BYTES = 131072;

const STATES_MARKER = '@@STATES@@';
const FILE_MARKER = '@@FILE@@';
const PANES_MARKER = '@@PANES@@';
const PREVIEW_MARKER = '@@PREV@@';
const PREVIEW_DATA_MARKER = '@@PREVB64@@';

/**
 * One remote command that returns everything the home page needs for the resume
 * cards: every session state file (delimited) plus a full pane listing (for the
 * live/dead worker badges and the tmux-alive flag).
 */
export function remoteReportScript(): string {
  return [
    `printf '${STATES_MARKER}\\n'`,
    `for f in "$HOME/.claude/workbench/sessions"/*.json; do`,
    `  [ -e "$f" ] || continue`,
    // The file NAME carries the session key of a state file written without the
    // field (SPEC-V2 B), so it is reported alongside the contents.
    `  printf '${FILE_MARKER}%s\\n' "$(basename "$f")"`,
    `  cat "$f"`,
    `  printf '\\n'`,
    `done`,
    `printf '${PANES_MARKER}\\n'`,
    `tmux list-panes -a -F '${WORKER_FORMAT}' 2>/dev/null || true`,
  ].join('\n');
}

export interface RemoteReport {
  states: SessionState[];
  paneText: string;
}

export function parseRemoteReport(out: string): RemoteReport {
  const panesAt = out.indexOf(PANES_MARKER);
  const statesPart = panesAt >= 0 ? out.slice(0, panesAt) : out;
  const paneText = panesAt >= 0 ? out.slice(panesAt + PANES_MARKER.length) : '';

  const states: SessionState[] = [];
  const start = statesPart.indexOf(STATES_MARKER);
  const body = start >= 0 ? statesPart.slice(start + STATES_MARKER.length) : statesPart;
  for (const chunk of body.split(FILE_MARKER)) {
    if (!chunk.trim()) {
      continue;
    }
    // '<file name>\n<json>' — the name line is absent in reports from an older
    // report script, in which case the whole chunk is the JSON.
    const newline = chunk.indexOf('\n');
    const head = newline < 0 ? '' : chunk.slice(0, newline).trim();
    const fileName = head.endsWith('.json') ? head : undefined;
    const state = parseState((fileName ? chunk.slice(newline + 1) : chunk).trim());
    if (state) {
      states.push({
        ...state,
        sessionKey: state.sessionKey ?? (fileName ? sessionKeyFromFileName(fileName) : undefined),
      });
    }
  }
  return { states: sortByLastActive(states), paneText: paneText.replace(/^\n/, '') };
}

/** Distinct tmux session names present in a pane listing (session_name is field 1). */
export function aliveSessionNames(paneText: string): Set<string> {
  const names = new Set<string>();
  for (const line of paneText.split('\n')) {
    const name = line.split('|')[0]?.trim();
    if (name) {
      names.add(name);
    }
  }
  return names;
}

/** One preview request: which project directory, and which session in it. */
export interface PreviewRequest {
  slug: string;
  /** Session name from the state file; missing for an unnamed (V1) session. */
  name?: string;
}

/** Transcripts examined per request before falling back to the newest one. */
export const PREVIEW_CANDIDATES = 20;

/**
 * One transcript PER SESSION, not per folder: with several workbench sessions in
 * one peer folder, the newest transcript alone would give every card the same
 * preview and the same session id to resume.
 *
 * The remote side picks the newest transcript whose head carries the session's
 * name (`claude -n` writes {"type":"custom-title","customTitle":"…"} into the
 * first lines) and falls back to the newest transcript, reporting WHICH of the
 * two happened. Answers are keyed by request index, so no name has to survive
 * the round trip. Slugs are computed on the Mac (claudeProjectSlug); the remote
 * side needs no path logic, and base64 keeps names and tails free of our markers.
 */
export function remotePreviewScript(requests: PreviewRequest[]): string {
  if (requests.length === 0) {
    return 'true';
  }
  const calls = requests.map((request, index) =>
    `wbprev ${shellQuote(String(index))} ${shellQuote(request.slug)} ${shellQuote(request.name ?? '')}`
  );
  return [
    `wbprev() {`,
    `  idx="$1"; s="$2"; want="$3"; f=""; how=newest`,
    `  d="$HOME/.claude/projects/$s"`,
    `  if [ -n "$want" ]; then`,
    `    for c in $(ls -t "$d"/*.jsonl 2>/dev/null | head -${PREVIEW_CANDIDATES}); do`,
    `      if head -c 8192 "$c" | grep -qF "\\"customTitle\\":\\"$want\\""; then f="$c"; how=named; break; fi`,
    `    done`,
    `  fi`,
    `  [ -n "$f" ] || f=$(ls -t "$d"/*.jsonl 2>/dev/null | head -1)`,
    `  [ -n "$f" ] || return 0`,
    `  printf '${PREVIEW_MARKER}%s\\t%s\\t%s\\n' "$idx" "$(basename "$f" .jsonl)" "$how"`,
    `  printf '${PREVIEW_DATA_MARKER}%s\\n' "$(tail -c ${PREVIEW_TAIL_BYTES} "$f" | base64 | tr -d '\\n')"`,
    `}`,
    ...calls,
  ].join('\n');
}

export interface RemotePreview {
  sessionId: string;
  lastUserMessage?: string;
  /** true when the transcript was matched by session name, not just by mtime. */
  nameMatched: boolean;
}

/** Parses the preview script output into a request-index -> preview map. */
export function parseRemotePreviews(out: string): Map<number, RemotePreview> {
  const previews = new Map<number, RemotePreview>();
  let index: number | undefined;
  let sessionId: string | undefined;
  let nameMatched = false;
  for (const line of out.split('\n')) {
    if (line.startsWith(PREVIEW_MARKER)) {
      const [i, id, how] = line.slice(PREVIEW_MARKER.length).split('\t');
      const parsed = Number(i);
      index = Number.isInteger(parsed) ? parsed : undefined;
      sessionId = id;
      nameMatched = how?.trim() === 'named';
    } else if (line.startsWith(PREVIEW_DATA_MARKER) && index !== undefined && sessionId) {
      const b64 = line.slice(PREVIEW_DATA_MARKER.length);
      let text: string | undefined;
      try {
        text = lastUserMessage(Buffer.from(b64, 'base64').toString('utf8'));
      } catch {
        text = undefined;
      }
      previews.set(index, { sessionId, lastUserMessage: text, nameMatched });
      index = undefined;
      sessionId = undefined;
      nameMatched = false;
    }
  }
  return previews;
}

/** Preview requests for a report, in the order of its states (index = key). */
export function previewRequests(states: SessionState[]): PreviewRequest[] {
  return states.map((state) => ({ slug: claudeProjectSlug(state.dir), name: state.name }));
}

/** Turns the two remote reports into resume cards, mirroring collectCards (local). */
export function buildRemoteCards(
  report: RemoteReport,
  previews: Map<number, RemotePreview>,
): SessionCard[] {
  const alive = aliveSessionNames(report.paneText);
  const risk = transcriptRisk(report.states);
  return report.states.map((state, index) => {
    const session = state.tmuxSession ?? sessionName(state.dir, state.sessionKey);
    const preview = previews.get(index);
    return {
      dir: state.dir,
      name: sessionDisplayName(state),
      folderName: basename(state.dir),
      sessionKey: state.sessionKey,
      harness: state.harness,
      model: state.model,
      tmuxSession: session,
      lastActive: state.lastActive,
      sessionId: state.claudeSessionId ?? preview?.sessionId,
      lastUserMessage: preview?.lastUserMessage,
      transcriptUncertain: preview !== undefined
        && isTranscriptUncertain(state, risk, preview.nameMatched),
      workers: mergeWorkers(state.workers ?? [], parseWorkerPanes(report.paneText, session)),
      tmuxAlive: alive.has(session),
    };
  });
}

export interface RemoteCardsResult {
  reachable: boolean;
  cards: SessionCard[];
  error?: string;
}

/** Reads peer state over SSH and returns resume cards, or a reachability error. */
export async function collectRemoteCards(): Promise<RemoteCardsResult> {
  let report: RemoteReport;
  try {
    report = parseRemoteReport(await runSsh(remoteReportScript()));
  } catch (error) {
    return { reachable: false, cards: [], error: String((error as Error)?.message ?? error) };
  }
  let previews = new Map<number, RemotePreview>();
  try {
    previews = parseRemotePreviews(
      await runSsh(remotePreviewScript(previewRequests(report.states))),
    );
  } catch {
    // Previews are best-effort: show the cards without a message rather than fail.
  }
  return { reachable: true, cards: buildRemoteCards(report, previews) };
}

/** File names in peer's state directory — the key space of a folder (SPEC-V2 B). */
export function remoteSessionFilesScript(): string {
  return `ls -1 "$HOME/.claude/workbench/sessions" 2>/dev/null || true`;
}

export async function listRemoteSessionFiles(): Promise<string[]> {
  return parseRemoteDirs(await runSsh(remoteSessionFilesScript()));
}

/** `wb-state new-key` on peer; undefined when the command is not (yet) there. */
export async function remoteNewKey(dir: string): Promise<string | undefined> {
  try {
    const key = (await runSsh(`wb-state new-key ${shellQuote(dir)}`)).trim().split(/\s+/)[0];
    return isSessionKey(key) ? key : undefined;
  } catch {
    return undefined;
  }
}

/** peer project folders under ~/AI for the remote new-session picker. */
export function remoteListDirsScript(): string {
  return `find "$HOME/AI" -maxdepth 1 -mindepth 1 -type d 2>/dev/null | sort`;
}

export function parseRemoteDirs(out: string): string[] {
  return out.split('\n').map((l) => l.trim()).filter((l) => l.length > 0);
}

export async function listRemoteDirs(): Promise<string[]> {
  return parseRemoteDirs(await runSsh(remoteListDirsScript()));
}
