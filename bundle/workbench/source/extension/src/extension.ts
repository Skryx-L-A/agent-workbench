import { basename, relative } from 'node:path';
import * as vscode from 'vscode';
import { HomePanel, type OpenTarget } from './homeView.ts';
import {
  isMachine,
  peerRemoteUri,
  type Machine,
  MACHINE_LABEL,
  MACHINE_STATE_KEY,
  MACHINES,
} from './machine.ts';
import {
  type CommandSink,
  type CommandTarget,
  commandsDir,
  drainCommands,
  ensureCommandsDir,
} from './commandFile.ts';
import { decidePending, type PendingAction } from './pending.ts';
import { listRemoteDirs } from './remoteState.ts';
import { nextSessionKey } from './sessionKey.ts';
import {
  DEFAULT_SETTINGS,
  expandHome,
  readSettings,
  type Settings,
  workbenchDir,
  type WorkerLayout,
} from './settings.ts';
import { extensionActor } from './settingsLog.ts';
import { SettingsPanel } from './settingsView.ts';
import { readState } from './state.ts';
import {
  type LaunchOptions,
  orchestratorAttachCommand,
  orchestratorCommand,
  orchestratorLiveness,
  restorePlan,
  terminalPlan,
  workerViewCommand,
} from './terminal.ts';
import {
  findOrchestratorPane,
  focusPane,
  hasSession,
  killViewSession,
  sendText,
  sessionName,
  viewSessionName,
} from './tmux.ts';
import {
  hintMessage,
  OPEN_TAB_LABEL,
  regrid,
  shouldHintWorkerTab,
  syncLayout,
  workerTabAction,
  type WorkerTabState,
} from './workerTab.ts';
import { WorkersProvider } from './workersView.ts';

const PENDING_KEY = 'claudeWorkbench.pendingAction';
/** Which session of the open folder this window is showing (SPEC-V2 B). */
const SESSION_KEY_STATE = 'claudeWorkbench.sessionKey';
const TERMINAL_NAME = 'Claude Workbench';
const WORKER_TERMINAL_NAME = 'Claude Workbench — Worker';

/**
 * The terminals this extension host created. A terminal VSCode restored from a
 * previous window session is not in here — it only looks like ours.
 */
const ownTerminals = new Set<vscode.Terminal>();

/** Last known layout and when the missing-tab hint was last shown. */
let workerLayout: WorkerLayout = DEFAULT_SETTINGS.workerLayout;
let lastTabHintAt: number | undefined;

/** Log of what the extension did — the only trace a dropped command leaves. */
let output: vscode.OutputChannel | undefined;

export async function activate(context: vscode.ExtensionContext): Promise<void> {
  const workers = new WorkersProvider(context.workspaceState.get<string>(SESSION_KEY_STATE));
  const settings = await readSettings();
  workers.setPollSeconds(settings.workerPollSeconds);
  workerLayout = settings.workerLayout;
  // Settings take effect immediately, without restarting the extension and
  // without waiting for the next session start — a layout switch moves the panes
  // and opens or closes the worker tab right away (SPEC-V2 C).
  // Every settings write of this window is traceable in the change log with the
  // window it came from — the shell side logs its writes the same way.
  SettingsPanel.actor = extensionActor(
    context.workspaceState.get<string>(SESSION_KEY_STATE),
    vscode.workspace.workspaceFolders?.[0]?.uri.path,
  );
  SettingsPanel.onLogError = log;
  SettingsPanel.onChanged = (changed: Settings) => {
    workers.setPollSeconds(changed.workerPollSeconds);
    void applyWorkerLayout(context, changed.workerLayout);
  };
  // Every refresh of the sidebar also answers: can alice SEE these workers?
  // Only LIVE ones count — a stale entry of a finished worker is nothing to
  // chase a tab for.
  workers.onWorkers = (list) =>
    void checkWorkerTabVisible(list.filter((w) => w.status === 'running').length);
  context.subscriptions.push(
    workers,
    vscode.window.onDidCloseTerminal((terminal) => ownTerminals.delete(terminal)),
    vscode.window.registerTreeDataProvider('claude-workbench.workers', workers),
    vscode.commands.registerCommand('claude-workbench.start', () => openHome(context)),
    vscode.commands.registerCommand('claude-workbench.newSession', () => newSession(context)),
    vscode.commands.registerCommand('claude-workbench.settings', () => SettingsPanel.show(context)),
    vscode.commands.registerCommand('claude-workbench.openWorkerTab', () => openWorkerTab(context)),
    vscode.commands.registerCommand('claude-workbench.refreshWorkers', () => workers.refresh()),
    vscode.commands.registerCommand('claude-workbench.focusWorker', focusWorker),
    vscode.commands.registerCommand(
      'claude-workbench.sendPathToOrchestrator',
      (uri?: vscode.Uri) => sendPath(context, uri),
    ),
    vscode.commands.registerCommand(
      'claude-workbench.sendSelectionToOrchestrator',
      () => sendSelection(context),
    ),
  );

  // Reachable from outside BEFORE anything else can fail: a dropped command file
  // is the orchestrator's only way into this running window.
  await startCommandWatcher(context, workers);
  // …and a layout switch in ANY window must reach this one (settings.json is global).
  startSettingsWatcher(context, workers);

  // Design C: this extension is extensionKind "ui", so it runs in the LOCAL
  // (Mac) extension host even inside a Remote-SSH (peer) window. globalState is
  // therefore the SAME store before and after the openFolder reload for both
  // machines, so the launch handover is a single globalState action for all —
  // no file-based handover on peer is needed. Match on uri.path so a remote
  // workspace folder (vscode-remote://…/home/alice/…) matches the peer path
  // stored in the pending action (on POSIX .path == .fsPath for local folders).
  const folder = vscode.workspace.workspaceFolders?.[0]?.uri.path;
  const pending = context.globalState.get<PendingAction>(PENDING_KEY);
  let handledPending = false;
  switch (decidePending(pending, folder, Date.now())) {
    case 'consume':
      await context.globalState.update(PENDING_KEY, undefined);
      await context.workspaceState.update(SESSION_KEY_STATE, pending!.sessionKey);
      workers.setSessionKey(pending!.sessionKey);
      SettingsPanel.actor = extensionActor(pending!.sessionKey, folder);
      await startOrchestratorTerminal(folder!, {
        sessionId: pending!.sessionId,
        name: pending!.name,
        sessionKey: pending!.sessionKey,
      });
      workers.refresh();
      handledPending = true;
      break;
    case 'expire':
      await context.globalState.update(PENDING_KEY, undefined);
      vscode.window.showWarningMessage(
        `Session in "${pending!.dir}" wurde nicht gestartet — bitte erneut versuchen.`,
      );
      break;
    case 'keep':
      // belongs to another window of this profile — do not touch it
      break;
  }
  await restoreTerminals(context, folder, handledPending, settings.workerLayout);
  if (!folder) {
    await openHome(context);
  }
}

/**
 * After a window reload — which installing a new build requires — the window
 * must not sit there without its terminals. If this window has a folder whose
 * tmux session is still alive, the tabs are re-created: the orchestrator tab
 * ATTACHES to the running session (never wb-code, so no second Claude can
 * start), and with workerLayout "window" the worker tab comes back with it.
 * A dead session restores nothing.
 */
async function restoreTerminals(
  context: vscode.ExtensionContext,
  folder: string | undefined,
  handledPending: boolean,
  layout: WorkerLayout,
): Promise<void> {
  const sessionKey = context.workspaceState.get<string>(SESSION_KEY_STATE);
  const session = folder
    ? (await readState(folder, sessionKey))?.tmuxSession ?? sessionName(folder, sessionKey)
    : undefined;
  const plan = restorePlan({
    hasFolder: folder !== undefined,
    sessionAlive: session !== undefined && await hasSession(session),
    handledPending,
    layout,
  });
  if (!plan.orchestrator) {
    return;
  }
  await attachOrchestratorTerminal(session!);
  if (plan.workerTab) {
    await showWorkerTerminal(folder!, sessionKey);
  }
  log(`Terminals wiederhergestellt (tmux-Session ${session}${plan.workerTab ? ', inkl. Worker-Tab' : ''}).`);
}

/** Re-attaches the orchestrator tab; a ghost terminal from the reload is replaced. */
async function attachOrchestratorTerminal(session: string): Promise<void> {
  const existing = vscode.window.terminals.find((t) => t.name === TERMINAL_NAME);
  const createdByUs = existing !== undefined && ownTerminals.has(existing);
  const plan = terminalPlan(
    existing && {
      createdByUs,
      exited: existing.exitStatus !== undefined,
      liveness: createdByUs ? await orchestratorLiveness(await existing.processId) : 'unknown',
    },
  );
  if (plan === 'show') {
    existing!.show(true);
    return;
  }
  // A terminal VSCode restored across the reload carries the name but a dead
  // shell — it must go, otherwise two tabs claim to be the orchestrator.
  existing?.dispose();
  const cwd = vscode.workspace.workspaceFolders?.[0]?.uri;
  const terminal = vscode.window.createTerminal({
    name: TERMINAL_NAME,
    cwd,
    location: vscode.TerminalLocation.Panel,
  });
  ownTerminals.add(terminal);
  terminal.sendText(orchestratorAttachCommand(session));
  terminal.show(true);
}

/**
 * The drop directory (~/.claude/workbench/commands/) is watched AND polled: the
 * watcher reacts instantly, the poll guarantees delivery even where file
 * watching outside the workspace is unreliable.
 */
const COMMAND_POLL_MS = 3000;

async function startCommandWatcher(
  context: vscode.ExtensionContext,
  workers: WorkersProvider,
): Promise<void> {
  const dir = commandsDir();
  await ensureCommandsDir(dir);
  const sink: CommandSink = {
    run: async (command) => {
      switch (command) {
        case 'open-worker-tab':
          await vscode.commands.executeCommand('claude-workbench.openWorkerTab');
          break;
        case 'focus-orchestrator':
          vscode.window.terminals.find((t) => t.name === TERMINAL_NAME)?.show(true);
          break;
        case 'refresh-workers':
          workers.refresh();
          break;
      }
    },
    log,
  };
  // The drop directory is global and every window polls it, so a command may
  // name the window it is meant for (folder + optional sessionKey).
  const drain = () => void drainCommands(dir, sink, { window: windowTarget(context) });
  const watcher = vscode.workspace.createFileSystemWatcher(
    new vscode.RelativePattern(vscode.Uri.file(dir), '*'),
  );
  const timer = setInterval(drain, COMMAND_POLL_MS);
  context.subscriptions.push(
    watcher,
    watcher.onDidCreate(drain),
    watcher.onDidChange(drain),
    { dispose: () => clearInterval(timer) },
  );
  log(`Kommando-Verzeichnis wird beobachtet: ${dir}`);
  drain();
}

/** Identity of this window for addressed command files (SPEC-V2 B). */
function windowTarget(context: vscode.ExtensionContext): CommandTarget {
  return {
    dir: vscode.workspace.workspaceFolders?.[0]?.uri.path,
    sessionKey: context.workspaceState.get<string>(SESSION_KEY_STATE),
  };
}

/**
 * settings.json is shared by every window, so a layout switch made ANYWHERE has
 * to reach this window too: re-read the file, and apply a changed layout exactly
 * as if this window had made the change (own session re-tiled, tab opened or
 * closed, visibility hint as usual). Watched and polled, like the command
 * directory — an unnoticed layout switch is what made four workers invisible.
 */
function startSettingsWatcher(context: vscode.ExtensionContext, workers: WorkersProvider): void {
  const check = async () => {
    const current = await readSettings();
    workers.setPollSeconds(current.workerPollSeconds);
    const { apply } = syncLayout(workerLayout, current.workerLayout);
    if (apply) {
      log(`Layout-Wechsel aus einem anderen Fenster übernommen: ${current.workerLayout}`);
      await applyWorkerLayout(context, current.workerLayout);
    }
  };
  const run = () => void check();
  const watcher = vscode.workspace.createFileSystemWatcher(
    new vscode.RelativePattern(vscode.Uri.file(workbenchDir()), 'settings.json'),
  );
  const timer = setInterval(run, SETTINGS_POLL_MS);
  context.subscriptions.push(
    watcher,
    watcher.onDidCreate(run),
    watcher.onDidChange(run),
    { dispose: () => clearInterval(timer) },
  );
}

const SETTINGS_POLL_MS = 5000;

function log(message: string): void {
  output ??= vscode.window.createOutputChannel('Claude Workbench');
  output.appendLine(`[${new Date().toISOString()}] ${message}`);
}

export function deactivate(): void {
  // nothing to clean up beyond context.subscriptions
}

function currentMachine(context: vscode.ExtensionContext): Machine {
  const stored = context.globalState.get(MACHINE_STATE_KEY);
  return isMachine(stored) ? stored : 'mac';
}

/** Native QuickPick at start-up; the choice is persisted and pre-selected. */
async function chooseMachine(context: vscode.ExtensionContext): Promise<void> {
  const current = currentMachine(context);
  const items = MACHINES.map((machine) => ({
    label: MACHINE_LABEL[machine],
    description: machine === current ? 'zuletzt gewählt' : undefined,
    machine,
  }));
  const pick = await vscode.window.showQuickPick(items, {
    title: 'Claude Workbench — Maschine wählen',
    placeHolder: `Aktuell: ${MACHINE_LABEL[current]}`,
  });
  if (pick) {
    await context.globalState.update(MACHINE_STATE_KEY, pick.machine);
  }
}

async function openHome(context: vscode.ExtensionContext): Promise<void> {
  await chooseMachine(context);
  await HomePanel.show(context, {
    open: (dir, target, machine) => openSession(context, dir, target, machine),
    pickNew: (machine) => pickNewFolder(context, machine),
    openSettings: () => SettingsPanel.show(context),
  });
}

/** Palette command "Neue Session": uses the currently selected machine. */
async function newSession(context: vscode.ExtensionContext): Promise<void> {
  await pickNewFolder(context, currentMachine(context));
}

/**
 * Folder picker for a new session: local dialog on Mac, remote QuickPick on
 * Peer. Afterwards alice names the session (SPEC-V2 D); a folder that
 * already has a session gets a key for the new one, the first one stays the
 * folder's default session.
 */
async function pickNewFolder(context: vscode.ExtensionContext, machine: Machine): Promise<void> {
  const dir = machine === 'peer' ? await pickRemoteFolder() : await pickLocalFolder();
  if (!dir) {
    return;
  }
  const name = await askSessionName(dir);
  if (name === undefined) {
    return; // dialog cancelled — no session, no state file
  }
  const sessionKey = await nextSessionKey(dir, machine === 'peer');
  await openSession(context, dir, { name, sessionKey }, machine);
}

/** Empty input means "no name" — the Startseite then shows basename(dir). */
async function askSessionName(dir: string): Promise<string | undefined> {
  const name = await vscode.window.showInputBox({
    title: 'Neue Session — Name',
    prompt: 'Name dieser Session (erscheint auf der Startseite und in Claude)',
    value: basename(dir),
    placeHolder: basename(dir),
  });
  return name === undefined ? undefined : name.trim();
}

async function pickLocalFolder(): Promise<string | undefined> {
  const settings = await readSettings();
  const picked = await vscode.window.showOpenDialog({
    canSelectFolders: true,
    canSelectFiles: false,
    canSelectMany: false,
    defaultUri: vscode.Uri.file(expandHome(settings.newSessionDefaultDir)),
    openLabel: 'Session hier starten',
    title: 'Neue Session — Projektordner wählen',
  });
  return picked?.[0]?.fsPath;
}

/** No native dialog on the Mac would reach peer, so offer its ~/AI folders over SSH. */
async function pickRemoteFolder(): Promise<string | undefined> {
  let dirs: string[];
  try {
    dirs = await listRemoteDirs();
  } catch (error) {
    vscode.window.showWarningMessage(`Peer nicht erreichbar (ssh peer): ${error}`);
    return undefined;
  }
  const custom = { label: 'Eigenen Pfad eingeben …', dir: undefined as string | undefined };
  const items = [
    ...dirs.map((dir) => ({ label: basename(dir), description: dir, dir })),
    custom,
  ];
  const pick = await vscode.window.showQuickPick(items, {
    title: 'Neue Peer-Session — Projektordner wählen',
    placeHolder: '~/AI auf Peer',
  });
  if (!pick) {
    return undefined;
  }
  if (pick.dir) {
    return pick.dir;
  }
  return vscode.window.showInputBox({
    title: 'Peer-Pfad',
    prompt: 'Absoluter Pfad auf Peer',
    value: '/home/alice/AI/',
  });
}

/**
 * Opens the folder in this window. That reloads the window (and, for a UI
 * extension, re-activates it in the same local host), so the terminal launch is
 * handed to the next activation via globalState. On Peer the folder is opened
 * fully remote (vscode-remote://ssh-remote+peer<path>): the window becomes a
 * remote window, but the extension stays on the Mac (extensionKind "ui"), so the
 * same globalState handover works. `dir` is the absolute path ON the target
 * machine (peer path for peer), which is exactly what the remote workspace
 * folder's uri.path reports after the reload.
 */
async function openSession(
  context: vscode.ExtensionContext,
  dir: string,
  target: OpenTarget,
  machine: Machine,
): Promise<void> {
  const current = vscode.workspace.workspaceFolders?.[0]?.uri.path;
  if (current === dir) {
    await context.workspaceState.update(SESSION_KEY_STATE, target.sessionKey);
    await startOrchestratorTerminal(dir, target);
    return;
  }
  const action: PendingAction = {
    dir,
    sessionId: target.sessionId,
    name: target.name,
    sessionKey: target.sessionKey,
    machine,
    createdAt: Date.now(),
  };
  await context.globalState.update(PENDING_KEY, action);
  const folderUri = machine === 'peer'
    ? vscode.Uri.from(peerRemoteUri(dir))
    : vscode.Uri.file(dir);
  await vscode.commands.executeCommand('vscode.openFolder', folderUri, false);
}

/**
 * A live workbench terminal is attached to the tmux session — its prompt belongs
 * to the orchestrator, so typing 'wb-code ...' into it would land in the Claude
 * chat; it is only shown. A terminal VSCode restored across the openFolder reload
 * carries the same name but a dead wb-code, so it is disposed and relaunched.
 */
async function startOrchestratorTerminal(dir: string, options: LaunchOptions = {}): Promise<void> {
  const settings = await readSettings();
  const existing = vscode.window.terminals.find((t) => t.name === TERMINAL_NAME);
  const createdByUs = existing !== undefined && ownTerminals.has(existing);
  const plan = terminalPlan(
    existing && {
      createdByUs,
      exited: existing.exitStatus !== undefined,
      liveness: createdByUs ? await orchestratorLiveness(await existing.processId) : 'unknown',
    },
  );
  if (plan === 'show') {
    existing!.show(true);
    await maximizePanel(settings.terminalStartMaximized);
    return;
  }
  existing?.dispose();
  // Use the workspace folder Uri as cwd so a remote (peer) window opens the
  // terminal on peer; fall back to the plain path for the same-folder case.
  const cwd = vscode.workspace.workspaceFolders?.[0]?.uri ?? dir;
  const terminal = vscode.window.createTerminal({
    name: TERMINAL_NAME,
    cwd,
    location: vscode.TerminalLocation.Panel,
  });
  ownTerminals.add(terminal);
  terminal.sendText(orchestratorCommand(dir, options));
  terminal.show(true);
  await maximizePanel(settings.terminalStartMaximized);
  if (settings.workerLayout === 'window') {
    await showWorkerTerminal(dir, options.sessionKey);
  }
}

/**
 * Second terminal for workerLayout 'window' (SPEC-V2 C): all workers live in the
 * 'workers' window of the same tmux session, and this terminal attaches to a
 * grouped session so it can show that window while the orchestrator terminal
 * stays on its own. An existing worker terminal of this extension host is only
 * revealed — re-attaching would drop its current window.
 */
async function showWorkerTerminal(dir: string, sessionKey?: string): Promise<void> {
  const existing = vscode.window.terminals.find((t) => t.name === WORKER_TERMINAL_NAME);
  if (existing && ownTerminals.has(existing) && existing.exitStatus === undefined) {
    existing.show(false);
    return;
  }
  existing?.dispose();
  const state = await readState(dir, sessionKey);
  const session = state?.tmuxSession ?? sessionName(dir, sessionKey);
  const cwd = vscode.workspace.workspaceFolders?.[0]?.uri ?? dir;
  const terminal = vscode.window.createTerminal({
    name: WORKER_TERMINAL_NAME,
    cwd,
    location: vscode.TerminalLocation.Panel,
  });
  ownTerminals.add(terminal);
  terminal.sendText(workerViewCommand(session, viewSessionName(session)));
}

/**
 * Palette command: opens the worker tab for the session of this window — needed
 * after switching workerLayout to 'window' while a session is already running.
 */
async function openWorkerTab(context: vscode.ExtensionContext): Promise<void> {
  const dir = vscode.workspace.workspaceFolders?.[0]?.uri.path;
  if (!dir) {
    vscode.window.showWarningMessage('Kein Projektordner geöffnet.');
    return;
  }
  await showWorkerTerminal(dir, context.workspaceState.get<string>(SESSION_KEY_STATE));
  vscode.window.terminals.find((t) => t.name === WORKER_TERMINAL_NAME)?.show(true);
}

function workerTerminal(): vscode.Terminal | undefined {
  return vscode.window.terminals.find((t) => t.name === WORKER_TERMINAL_NAME);
}

/** 'live' only for a tab THIS host created and whose shell still runs. */
function workerTabState(): WorkerTabState {
  const terminal = workerTerminal();
  if (!terminal) {
    return 'none';
  }
  return ownTerminals.has(terminal) && terminal.exitStatus === undefined ? 'live' : 'ghost';
}

/**
 * Applies a layout switch to the RUNNING session (SPEC-V2 C). wb-grid moves the
 * panes for the new setting — it never kills a worker — and only then does the
 * tab follow: opened for "window", closed for "split" AFTER the panes are back,
 * so no tab is ever left pointing at a `workers` window that no longer exists.
 */
async function applyWorkerLayout(
  context: vscode.ExtensionContext,
  layout: WorkerLayout,
): Promise<void> {
  const previous = workerLayout;
  workerLayout = layout;
  const dir = vscode.workspace.workspaceFolders?.[0]?.uri.path;
  if (!dir || layout === previous) {
    return;
  }
  const sessionKey = context.workspaceState.get<string>(SESSION_KEY_STATE);
  const state = await readState(dir, sessionKey);
  const session = state?.tmuxSession ?? sessionName(dir, sessionKey);
  const orchestrator = await findOrchestratorPane(session);
  if (orchestrator.status === 'ok') {
    await regrid(orchestrator.paneId);
  }
  switch (workerTabAction(layout, workerTabState())) {
    case 'open':
      await showWorkerTerminal(dir, sessionKey);
      workerTerminal()?.show(true);
      vscode.window.setStatusBarMessage(
        'Worker liegen jetzt im Tab "Claude Workbench — Worker".', 6000,
      );
      break;
    case 'close':
      workerTerminal()?.dispose();
      // The grouped session only existed for that tab; its windows belong to the
      // orchestrator session and survive.
      await killViewSession(session);
      lastTabHintAt = undefined;
      break;
    case 'none':
      break;
  }
}

/**
 * The visibility guarantee: with layout "window" the workers live in their own
 * tmux window. If this VSCode window has no tab showing it, alice sees an
 * empty grid and would assume nothing is running — so say it, with the button
 * that fixes it. Rate-limited, so it stays a hint and does not nag.
 */
async function checkWorkerTabVisible(workerCount: number): Promise<void> {
  const now = Date.now();
  if (!shouldHintWorkerTab(workerLayout, workerTabState(), workerCount, lastTabHintAt, now)) {
    return;
  }
  lastTabHintAt = now;
  const choice = await vscode.window.showWarningMessage(
    hintMessage(workerCount),
    OPEN_TAB_LABEL,
  );
  if (choice === OPEN_TAB_LABEL) {
    await vscode.commands.executeCommand('claude-workbench.openWorkerTab');
  }
}

/**
 * The orchestrator starts full-height; the editor takes room back only when
 * alice un-maximizes the panel himself. Can be turned off in the settings
 * (terminalStartMaximized).
 *
 * 'workbench.panel.opensMaximized: always' does not fire for a panel that is
 * already open, and toggleMaximizedPanel alone would shrink an already maximized
 * panel. On a HIDDEN panel that command is not a toggle: VSCode shows the panel
 * and maximizes it only if it is not maximized yet (workbench source, 1.9x).
 * Closing first therefore makes the sequence idempotent.
 */
async function maximizePanel(enabled: boolean): Promise<void> {
  if (!enabled) {
    return;
  }
  try {
    await vscode.commands.executeCommand('workbench.action.closePanel');
    await vscode.commands.executeCommand('workbench.action.toggleMaximizedPanel');
  } catch {
    // layout nicety only — never fail the session start over it
  }
}

async function focusWorker(paneId: string): Promise<void> {
  try {
    await focusPane(paneId);
    const terminal = vscode.window.terminals.find((t) => t.name === TERMINAL_NAME);
    terminal?.show(true);
  } catch {
    vscode.window.showWarningMessage(`Pane ${paneId} konnte nicht fokussiert werden.`);
  }
}

async function sendPath(context: vscode.ExtensionContext, uri?: vscode.Uri): Promise<void> {
  const target = uri ?? vscode.window.activeTextEditor?.document.uri;
  if (!target) {
    vscode.window.showWarningMessage('Keine Datei ausgewählt.');
    return;
  }
  await sendToOrchestrator(context, relativeToWorkspace(target.fsPath) + ' ');
}

async function sendSelection(context: vscode.ExtensionContext): Promise<void> {
  const editor = vscode.window.activeTextEditor;
  if (!editor || editor.selection.isEmpty) {
    vscode.window.showWarningMessage('Keine Auswahl im Editor.');
    return;
  }
  const path = relativeToWorkspace(editor.document.uri.fsPath);
  const line = editor.selection.start.line + 1;
  const text = editor.document.getText(editor.selection);
  await sendToOrchestrator(context, `${path}:${line}\n${text}\n`);
}

/** Types the text into the orchestrator pane of THIS window's session. */
async function sendToOrchestrator(context: vscode.ExtensionContext, text: string): Promise<void> {
  const dir = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
  if (!dir) {
    vscode.window.showWarningMessage('Kein Projektordner geöffnet.');
    return;
  }
  // The state file's tmuxSession is the truth; the computed name is only a
  // fallback for a project that has never been started (SPEC V1.1).
  const sessionKey = context.workspaceState.get<string>(SESSION_KEY_STATE);
  const state = await readState(dir, sessionKey);
  const session = state?.tmuxSession ?? sessionName(dir, sessionKey);
  const orchestrator = await findOrchestratorPane(session);
  if (orchestrator.status === 'missing') {
    vscode.window.showWarningMessage(
      `Kein Orchestrator-Pane in tmux-Session "${session}" gefunden.`,
    );
    return;
  }
  if (orchestrator.status === 'dead') {
    vscode.window.showWarningMessage(
      `Orchestrator-Pane in "${session}" ist tot — mit wb-revive neu starten.`,
    );
    return;
  }
  try {
    await sendText(orchestrator.paneId, text);
    vscode.window.setStatusBarMessage('An Orchestrator gesendet — Anweisung ergänzen und abschicken.', 4000);
  } catch (error) {
    vscode.window.showErrorMessage(`Senden an Orchestrator fehlgeschlagen: ${error}`);
  }
}

function relativeToWorkspace(fsPath: string): string {
  const root = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
  if (!root) {
    return fsPath;
  }
  const rel = relative(root, fsPath);
  return rel.startsWith('..') ? fsPath : rel;
}
