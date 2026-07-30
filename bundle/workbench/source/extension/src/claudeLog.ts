// Reads Claude Code's own transcripts (~/.claude/projects/<slug>/*.jsonl) to
// recover the session id to resume and a preview of the last user message.
import { open, readdir, readFile, stat } from 'node:fs/promises';
import { homedir } from 'node:os';
import { basename, join } from 'node:path';
import { claudeProjectSlug, slugForDir } from './state.ts';

export interface TranscriptInfo {
  sessionId: string;
  lastUserMessage?: string;
  file: string;
  mtime: number;
  /** true when this transcript carries the session's name, not just the newest mtime. */
  nameMatched: boolean;
}

const TAIL_BYTES = 2 * 1024 * 1024;

export function projectsDir(): string {
  return join(homedir(), '.claude', 'projects');
}

/**
 * Transcript directories Claude Code may have used for this project, most
 * likely first. The plain '/'-slug is kept as a fallback for older layouts.
 */
export function transcriptDirs(dir: string): string[] {
  const candidates = [claudeProjectSlug(dir), slugForDir(dir)];
  return [...new Set(candidates)].map((slug) => join(projectsDir(), slug));
}

/**
 * Session name a transcript was started with (`claude -n <name>`, which wb-code
 * passes through). Claude Code writes it as its own entry at the top of the
 * file: {"type":"custom-title","customTitle":"…"} / {"type":"agent-name",…}.
 * Verified against a probe transcript on 2026-07-25.
 */
export function transcriptName(head: string): string | undefined {
  for (const line of head.split('\n')) {
    const trimmed = line.trim();
    if (!trimmed.startsWith('{')) {
      continue;
    }
    let entry: any;
    try {
      entry = JSON.parse(trimmed);
    } catch {
      continue; // a truncated last line of the head, or a non-name entry
    }
    const name = entry?.customTitle ?? entry?.agentName;
    if (typeof name === 'string' && name.length > 0) {
      return name;
    }
  }
  return undefined;
}

/** Head of a transcript that is read to recognise its session name. */
const NAME_HEAD_BYTES = 8 * 1024;
/** Upper bound on transcripts probed for a name — the newest ones matter. */
const NAME_PROBE_FILES = 25;

/**
 * Newest transcript by mtime for a project dir. With several workbench sessions
 * in one folder, "newest" alone would hand every card the same session id, so
 * `sessionName` (if given) picks the newest transcript that was started under
 * that name; without a match the newest transcript is used, exactly as in V1.
 */
export async function latestTranscript(
  dir: string,
  sessionName?: string,
): Promise<TranscriptInfo | undefined> {
  let projectDir = '';
  let files: string[] = [];
  for (const candidate of transcriptDirs(dir)) {
    try {
      const found = (await readdir(candidate)).filter((f) => f.endsWith('.jsonl'));
      if (found.length > 0) {
        projectDir = candidate;
        files = found;
        break;
      }
    } catch {
      // directory does not exist — try the next slug
    }
  }
  if (files.length === 0) {
    return undefined;
  }
  const candidates: { file: string; mtime: number; size: number }[] = [];
  for (const file of files) {
    try {
      const info = await stat(join(projectDir, file));
      candidates.push({ file: join(projectDir, file), mtime: info.mtimeMs, size: info.size });
    } catch {
      // vanished between readdir and stat — skip
    }
  }
  candidates.sort((a, b) => b.mtime - a.mtime);
  let newest = candidates[0];
  if (!newest) {
    return undefined;
  }
  let nameMatched = false;
  if (sessionName) {
    const named = await findNamedTranscript(candidates, sessionName);
    if (named) {
      newest = named;
      nameMatched = true;
    }
  }
  // The tail is usually enough; a long stretch of tool results and background
  // notifications can push the last real turn out of it, so fall back to the
  // whole file rather than showing "Keine Nachricht gefunden".
  let message = lastUserMessage(await readTail(newest.file, TAIL_BYTES));
  if (!message && newest.size > TAIL_BYTES) {
    message = lastUserMessage(await readFile(newest.file, 'utf8'));
  }
  return {
    sessionId: basename(newest.file, '.jsonl'),
    lastUserMessage: message,
    file: newest.file,
    mtime: newest.mtime,
    nameMatched,
  };
}

/** Newest transcript (of the already mtime-sorted list) started under `name`. */
async function findNamedTranscript<T extends { file: string }>(
  candidates: T[],
  name: string,
): Promise<T | undefined> {
  for (const candidate of candidates.slice(0, NAME_PROBE_FILES)) {
    try {
      if (transcriptName(await readHead(candidate.file, NAME_HEAD_BYTES)) === name) {
        return candidate;
      }
    } catch {
      // unreadable transcript — try the next one
    }
  }
  return undefined;
}

async function readHead(file: string, maxBytes: number): Promise<string> {
  const handle = await open(file, 'r');
  try {
    const buffer = Buffer.alloc(maxBytes);
    const { bytesRead } = await handle.read(buffer, 0, maxBytes, 0);
    return buffer.toString('utf8', 0, bytesRead);
  } finally {
    await handle.close();
  }
}

async function readTail(file: string, maxBytes: number): Promise<string> {
  const handle = await open(file, 'r');
  try {
    const { size } = await handle.stat();
    const length = Math.min(size, maxBytes);
    const position = size - length;
    const buffer = Buffer.alloc(length);
    await handle.read(buffer, 0, length, position);
    const text = buffer.toString('utf8');
    // A partial first line would not parse as JSON anyway; drop it explicitly.
    return position > 0 ? text.slice(text.indexOf('\n') + 1) : text;
  } finally {
    await handle.close();
  }
}

/**
 * Claude Code writes a lot of machinery into the transcript as `type: "user"`:
 * background-task events, slash-command bookkeeping, command output, the
 * local-command caveat. None of those is something alice typed, so none of
 * them may show up as the card's preview.
 */
export function isSyntheticUserText(text: string): boolean {
  const t = text.trim();
  return (
    t.length === 0
    || t.startsWith('<task-notification')
    || t.includes('[SYSTEM NOTIFICATION')
    || t.includes('<local-command-stdout>')
    || t.includes('<local-command-stderr>')
    || t.includes('<command-name>')
    || t.includes('<command-message>')
    || t.startsWith('Caveat:')
    // meta lines the TUI injects for attachments, e.g. '[Image: 2624x1824 ...]'
    || /^\[(Image|Request interrupted|Pasted text)\b/.test(t)
  );
}

/** Last REAL user turn — hooks, tool results and synthetic events are not turns. */
export function lastUserMessage(jsonl: string): string | undefined {
  const lines = jsonl.split('\n');
  for (let i = lines.length - 1; i >= 0; i--) {
    const line = lines[i].trim();
    if (!line.startsWith('{')) {
      continue;
    }
    let entry: any;
    try {
      entry = JSON.parse(line);
    } catch {
      continue;
    }
    if (entry?.type !== 'user' || entry.isMeta || entry.attachment || entry.isSidechain) {
      continue;
    }
    const text = userText(entry.message?.content);
    if (text) {
      return text;
    }
  }
  return undefined;
}

function userText(content: unknown): string | undefined {
  let raw: string;
  if (typeof content === 'string') {
    raw = content;
  } else if (Array.isArray(content)) {
    raw = content
      .filter((block: any) => block?.type === 'text' && typeof block.text === 'string')
      .map((block: any) => block.text)
      .join('\n');
  } else {
    return undefined;
  }
  if (isSyntheticUserText(raw)) {
    return undefined;
  }
  // A real turn can still carry an appended system-reminder block.
  const cleaned = raw.replace(/<system-reminder>[\s\S]*?<\/system-reminder>/g, '').trim();
  return isSyntheticUserText(cleaned) ? undefined : cleaned;
}
