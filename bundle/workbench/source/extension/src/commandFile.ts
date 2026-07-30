// Command files — the only way to reach a RUNNING VSCode window from outside.
//
// Only an extension can create a terminal in an open window, and nothing outside
// VSCode can call an extension command. So the extension watches
// ~/.claude/workbench/commands/ and executes what is dropped there. That is how
// the orchestrator can open alice's worker tab for him, without stealing his
// focus and without him having to click anything.
//
// Format (both work, so a drop from a script cannot get it wrong):
//   1. the FILE NAME is the command   ->  touch ~/.claude/workbench/commands/open-worker-tab
//   2. the first non-empty LINE is    ->  echo open-worker-tab > ~/.claude/workbench/commands/req-123
// The content wins when it names a known command; otherwise the file name is
// tried. Unknown commands are ignored (logged, never thrown) so a stray file
// breaks nothing.
//
// TARGETING (the drop directory is global, several windows poll it): the line
// AFTER the command is the target project directory, and the line after that an
// optional sessionKey — V2 allows several sessions in one folder, so a folder
// alone does not identify a window:
//   open-worker-tab
//   /Users/alice/AI/foo
//   9f2a1c
// A file WITHOUT a target stays valid and is executed by the first window that
// sees it (unchanged behaviour). A file WITH a target is left alone by every
// other window; if no window claims it, it EXPIRES after TARGET_TTL_MS and is
// removed with a log line, so a command into the void is visible instead of
// lying around forever.
//
// A file this window is going to run is deleted BEFORE it runs — a command that
// fails must not run again on the next tick.
import { mkdir, readdir, readFile, stat, unlink } from 'node:fs/promises';
import { homedir } from 'node:os';
import { basename, extname, join } from 'node:path';

export const COMMANDS = [
  'open-worker-tab',
  'focus-orchestrator',
  'refresh-workers',
] as const;

export type WorkbenchCommand = (typeof COMMANDS)[number];

export function isWorkbenchCommand(value: string): value is WorkbenchCommand {
  return (COMMANDS as readonly string[]).includes(value);
}

export function commandsDir(): string {
  return join(homedir(), '.claude', 'workbench', 'commands');
}

/** Non-empty, non-comment lines — comments and blank lines carry nothing. */
function contentLines(content: string): string[] {
  return content
    .split('\n')
    .map((line) => line.trim())
    .filter((line) => line.length > 0 && !line.startsWith('#'));
}

/** Which window a command is meant for; both fields may be absent. */
export interface CommandTarget {
  /** Absolute project directory of the addressed window. */
  dir?: string;
  /** Session inside that directory (SPEC-V2 B); absent = any session of it. */
  sessionKey?: string;
}

export interface ParsedCommand {
  command: WorkbenchCommand;
  target: CommandTarget;
}

/** How long a targeted command waits for its window before it expires. */
export const TARGET_TTL_MS = 60_000;

/**
 * The command a dropped file asks for and who it is for, or undefined when it
 * names no command. The target lines follow the command line — or start at the
 * first line when the command came from the file name.
 */
export function parseCommandFile(fileName: string, content: string): ParsedCommand | undefined {
  const lines = contentLines(content);
  const stem = basename(fileName, extname(fileName));
  let command: WorkbenchCommand;
  let rest: string[];
  if (lines.length > 0 && isWorkbenchCommand(lines[0])) {
    command = lines[0];
    rest = lines.slice(1);
  } else if (isWorkbenchCommand(stem)) {
    command = stem;
    rest = lines;
  } else {
    return undefined;
  }
  const [dir, sessionKey] = rest;
  return {
    command,
    target: {
      dir: dir && dir.startsWith('/') ? dir : undefined,
      sessionKey: dir && dir.startsWith('/') ? sessionKey : undefined,
    },
  };
}

/** Does a target address this window? No target addresses every window. */
export function targetMatches(target: CommandTarget, window: CommandTarget): boolean {
  if (!target.dir) {
    return true;
  }
  if (target.dir !== window.dir) {
    return false;
  }
  // A folder without a key means "any session of that folder".
  return target.sessionKey === undefined || target.sessionKey === window.sessionKey;
}

export interface CommandSink {
  /** Runs one command. Rejections are caught by the caller and logged. */
  run(command: WorkbenchCommand): Promise<void>;
  log(message: string): void;
}

/** Files the drop directory should never be bothered with. */
function isIgnored(fileName: string): boolean {
  return fileName.startsWith('.');
}

export interface DrainOptions {
  /** This window's identity; a targeted command only runs where it matches. */
  window?: CommandTarget;
  now?: number;
  expireMs?: number;
}

/**
 * Executes and removes what this window is addressed by, and lets targeted
 * files for other windows lie — until they expire. Never throws: a missing
 * directory, an unreadable file or a failing command must not take the
 * extension down.
 */
export async function drainCommands(
  dir: string,
  sink: CommandSink,
  options: DrainOptions = {},
): Promise<void> {
  const window = options.window ?? {};
  const expireMs = options.expireMs ?? TARGET_TTL_MS;
  let files: string[];
  try {
    files = await readdir(dir);
  } catch {
    return; // no directory yet — nothing was dropped
  }
  for (const file of files.sort()) {
    if (isIgnored(file)) {
      continue;
    }
    const path = join(dir, file);
    let content = '';
    try {
      content = await readFile(path, 'utf8');
    } catch {
      // unreadable (binary, vanished, permissions) — the name may still say it
    }
    const parsed = parseCommandFile(file, content);
    if (parsed && !targetMatches(parsed.target, window)) {
      // Meant for another window. Leave it there — but not forever: a command
      // whose window never shows up must not rot in the directory silently.
      if (await isExpired(path, options.now ?? Date.now(), expireMs)) {
        if (await remove(path)) {
          sink.log(
            `Kommando verfallen (kein passendes Fenster): ${parsed.command} für `
            + `${parsed.target.dir}${parsed.target.sessionKey ? ` [${parsed.target.sessionKey}]` : ''}`,
          );
        }
      }
      continue;
    }
    // Delete first: a command that throws must not be retried forever.
    if (!await remove(path)) {
      continue; // already gone (another window drained it) — never run it twice
    }
    if (!parsed) {
      sink.log(`Unbekanntes Kommando ignoriert: ${file}`);
      continue;
    }
    try {
      await sink.run(parsed.command);
      sink.log(`Kommando ausgeführt: ${parsed.command}`);
    } catch (error) {
      sink.log(`Kommando fehlgeschlagen (${parsed.command}): ${(error as Error)?.message ?? error}`);
    }
  }
}

/** true when this call removed the file (and may therefore act on it). */
async function remove(path: string): Promise<boolean> {
  try {
    await unlink(path);
    return true;
  } catch {
    return false;
  }
}

async function isExpired(path: string, now: number, expireMs: number): Promise<boolean> {
  try {
    return now - (await stat(path)).mtimeMs >= expireMs;
  } catch {
    return false; // vanished — nothing to expire
  }
}

/** Makes sure the drop directory exists, so dropping into it always works. */
export async function ensureCommandsDir(dir: string = commandsDir()): Promise<void> {
  try {
    await mkdir(dir, { recursive: true });
  } catch {
    // the drain simply finds nothing — never fail activation over this
  }
}
