// Worker tab (SPEC-V2 C) — the rules around the second terminal tab.
//
// The failure this guards against, seen for real on 2026-07-25: settings said
// workerLayout "window", four workers were running in the tmux window `workers`,
// and the session had no worker tab (it was started before the feature existed).
// alice saw NO workers at all and had to assume none were running. Silence is
// the worst possible outcome here, so the extension either shows the tab or says
// out loud that it is missing.
import { execFile } from 'node:child_process';
import type { WorkerLayout } from './settings.ts';

export type WorkerTabAction = 'open' | 'close' | 'none';

/**
 * State of the worker tab in this window:
 *   none  — no terminal with that name
 *   ghost — a terminal VSCode restored across a reload: right name, dead shell,
 *           shows nothing. As useless as no tab at all.
 *   live  — a terminal this extension host created and whose shell is running.
 */
export type WorkerTabState = 'none' | 'ghost' | 'live';

/**
 * What the layout demands of the tab: "window" needs a LIVE one (a ghost is
 * replaced), "split" must not leave any behind — after wb-grid pulls the panes
 * back, the `workers` window is gone and a leftover tab would point at nothing.
 */
export function workerTabAction(layout: WorkerLayout, tab: WorkerTabState): WorkerTabAction {
  if (layout === 'window') {
    return tab === 'live' ? 'none' : 'open';
  }
  return tab === 'none' ? 'none' : 'close';
}

/**
 * settings.json is GLOBAL, the layout a window believes in is local. A window
 * that did not make the change would otherwise keep acting on "split" while its
 * workers move into the `workers` window on the next wb-grid run — the same
 * invisible-workers incident, one window over. So every window watches the file
 * and treats a FOREIGN change like its own.
 *
 * Applying only ever touches the window's OWN tmux session, and only when the
 * value really differs from what this window last acted on. Two windows can
 * therefore not push each other in circles: after the first application both
 * agree with the file, and the next observation is a no-op.
 */
export function syncLayout(
  known: WorkerLayout,
  fromFile: WorkerLayout,
): { apply: boolean; known: WorkerLayout } {
  return { apply: known !== fromFile, known: fromFile };
}

/** How long the hint stays quiet after it was shown once. */
export const HINT_COOLDOWN_MS = 5 * 60 * 1000;

/**
 * Warn when workers are running in a `workers` window that nothing in this
 * window shows. No workers, no tab needed yet; and never more than once per
 * cooldown, so the hint stays a hint and does not become noise.
 */
export function shouldHintWorkerTab(
  layout: WorkerLayout,
  tab: WorkerTabState,
  workerCount: number,
  lastHintAt: number | undefined,
  now: number,
  cooldownMs: number = HINT_COOLDOWN_MS,
): boolean {
  // A ghost tab counts as missing: it shows nothing, which is exactly the
  // failure this hint exists for.
  if (layout !== 'window' || tab === 'live' || workerCount < 1) {
    return false;
  }
  return lastHintAt === undefined || now - lastHintAt >= cooldownMs;
}

export function hintMessage(workerCount: number): string {
  const workers = workerCount === 1 ? '1 Worker läuft' : `${workerCount} Worker laufen`;
  return `${workers} im eigenen Worker-Tab, der hier nicht geöffnet ist.`;
}

export const OPEN_TAB_LABEL = 'Worker-Tab öffnen';

/**
 * Re-tiles the session for the CURRENT setting: wb-grid reads workerLayout
 * itself and MOVES panes (break-pane/join-pane), so a running worker survives
 * the switch. `paneRef` is any pane of the session — wb-grid derives the session
 * from it. Login shell because wb-grid lives in ~/.local/bin.
 */
export const REGRID_TIMEOUT_MS = 10000;

export function regridCommand(paneRef: string): string {
  return `wb-grid ${paneRef}`;
}

/** Never rejects: a failed re-tile must not stop the layout switch. */
export function regrid(paneRef: string): Promise<void> {
  return new Promise((resolve) => {
    execFile(
      '/bin/zsh',
      ['-lc', regridCommand(paneRef)],
      { timeout: REGRID_TIMEOUT_MS },
      () => resolve(),
    );
  });
}
