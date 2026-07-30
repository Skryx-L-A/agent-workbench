// Handing out session keys for a folder's 2nd, 3rd, … session (SPEC-V2 B).
//
// The shell layer owns the key space (`wb-state new-key <dir>` picks one that is
// free in the state directory), so it is asked first. Only if that call is not
// available or answers with something unusable does the extension pick a key
// itself, from the keys it can see — a workbench that cannot start a second
// session at all would be the worse failure.
import { execFile } from 'node:child_process';
import { randomBytes } from 'node:crypto';
import { shellQuote } from './format.ts';
import { listRemoteSessionFiles, remoteNewKey } from './remoteState.ts';
import {
  type FolderSessions,
  folderSessions,
  isSessionKey,
  pickFreeKey,
  readFolderSessions,
} from './state.ts';

export const NEW_KEY_TIMEOUT_MS = 5000;

/** First 6-hex token of `wb-state new-key` output, if it produced one. */
export function parseNewKeyOutput(out: string): string | undefined {
  const key = out.trim().split(/\s+/)[0];
  return isSessionKey(key) ? key : undefined;
}

/** A folder's first session stays the default one and gets no key (SPEC-V2 B). */
export function needsKey(sessions: FolderSessions): boolean {
  return sessions.hasDefault || sessions.keys.length > 0;
}

function localNewKey(dir: string): Promise<string | undefined> {
  return new Promise((resolve) => {
    // Login shell: ~/.local/bin (where wb-state lives) is not necessarily in the
    // extension host's PATH.
    execFile(
      '/bin/zsh',
      ['-lc', `wb-state new-key ${shellQuote(dir)}`],
      { timeout: NEW_KEY_TIMEOUT_MS },
      (error, stdout) => resolve(error ? undefined : parseNewKeyOutput(stdout)),
    );
  });
}

/**
 * Key for the next session in `dir`, or undefined when this is the folder's
 * first session (which stays the default session).
 */
export async function nextSessionKey(dir: string, remote: boolean): Promise<string | undefined> {
  const sessions = remote
    ? folderSessions(await listRemoteSessionFiles().catch(() => []), dir)
    : await readFolderSessions(dir);
  if (!needsKey(sessions)) {
    return undefined;
  }
  const fromShell = remote
    ? await remoteNewKey(dir).catch(() => undefined)
    : await localNewKey(dir);
  return fromShell ?? pickFreeKey(sessions.keys, () => randomBytes(3).toString('hex'));
}
