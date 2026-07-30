// A1 — Startseite: resume an earlier session or start a new one, on the Mac or
// (fully remote via Remote-SSH) on Peer.
import { randomBytes } from 'node:crypto';
import { basename } from 'node:path';
import * as vscode from 'vscode';
import { latestTranscript } from './claudeLog.ts';
import { type HomeView, renderHtml, type SessionCard } from './homeHtml.ts';
import { isMachine, type Machine, MACHINE_STATE_KEY } from './machine.ts';
import { collectRemoteCards } from './remoteState.ts';
import {
  isTranscriptUncertain,
  readAllStates,
  sessionDisplayName,
  transcriptRisk,
} from './state.ts';
import { hasSession, listWorkerPanes, sessionName } from './tmux.ts';
import { mergeWorkers } from './workers.ts';

/** Local (Mac) resume cards — one per SESSION state file (SPEC-V2 B/D). */
export async function collectCards(): Promise<SessionCard[]> {
  const cards: SessionCard[] = [];
  const states = await readAllStates();
  const risk = transcriptRisk(states);
  for (const state of states) {
    const session = state.tmuxSession ?? sessionName(state.dir, state.sessionKey);
    const name = sessionDisplayName(state);
    const [transcript, alive, panes] = await Promise.all([
      latestTranscript(state.dir, state.name),
      hasSession(session),
      listWorkerPanes(session),
    ]);
    cards.push({
      dir: state.dir,
      name,
      folderName: basename(state.dir),
      sessionKey: state.sessionKey,
      harness: state.harness,
      model: state.model,
      tmuxSession: session,
      lastActive: state.lastActive,
      sessionId: state.claudeSessionId ?? transcript?.sessionId,
      lastUserMessage: transcript?.lastUserMessage,
      transcriptUncertain: transcript !== undefined
        && isTranscriptUncertain(state, risk, transcript.nameMatched),
      workers: mergeWorkers(state.workers ?? [], panes),
      tmuxAlive: alive,
    });
  }
  return cards;
}

/** Which session of a folder to resume, plus what to resume it with. */
export interface OpenTarget {
  sessionId?: string;
  /** Missing for the folder's default session (SPEC-V2 B). */
  sessionKey?: string;
  name?: string;
}

export interface HomeDeps {
  /** Open a session on `machine` (Mac local, or Peer fully remote). */
  open: (dir: string, target: OpenTarget, machine: Machine) => Promise<void>;
  /** Run the new-session picker for `machine` and open the chosen folder. */
  pickNew: (machine: Machine) => Promise<void>;
  /** Open the Einstellungen panel. */
  openSettings: () => Promise<void>;
}

export class HomePanel {
  private static current: HomePanel | undefined;

  static async show(context: vscode.ExtensionContext, deps: HomeDeps): Promise<void> {
    if (HomePanel.current) {
      HomePanel.current.panel.reveal(vscode.ViewColumn.One);
      await HomePanel.current.render();
      return;
    }
    const panel = vscode.window.createWebviewPanel(
      'claudeWorkbench.home',
      'Claude Workbench',
      vscode.ViewColumn.One,
      {
        enableScripts: true,
        retainContextWhenHidden: true,
        localResourceRoots: [vscode.Uri.joinPath(context.extensionUri, 'media')],
      },
    );
    HomePanel.current = new HomePanel(panel, context, deps);
    await HomePanel.current.render();
  }

  private constructor(
    private readonly panel: vscode.WebviewPanel,
    private readonly context: vscode.ExtensionContext,
    private readonly deps: HomeDeps,
  ) {
    panel.onDidDispose(() => {
      HomePanel.current = undefined;
    });
    panel.webview.onDidReceiveMessage(async (message: any) => {
      switch (message?.command) {
        case 'resume':
          await this.deps.open(
            message.dir,
            {
              sessionId: message.sessionId,
              sessionKey: message.sessionKey,
              name: message.name,
            },
            this.machine(),
          );
          break;
        case 'new':
          await this.deps.pickNew(this.machine());
          break;
        case 'settings':
          await this.deps.openSettings();
          break;
        case 'switchMachine':
          if (isMachine(message.machine)) {
            await this.context.globalState.update(MACHINE_STATE_KEY, message.machine);
            await this.render();
          }
          break;
        case 'refresh':
          await this.render();
          break;
      }
    });
  }

  private machine(): Machine {
    const stored = this.context.globalState.get(MACHINE_STATE_KEY);
    return isMachine(stored) ? stored : 'mac';
  }

  private async render(): Promise<void> {
    const machine = this.machine();
    if (machine === 'peer') {
      // The SSH round-trip is not instant — show a loading page first.
      this.paint([], { machine, reachable: true, loading: true });
      const result = await collectRemoteCards();
      if (this.machine() !== 'peer') {
        return; // user switched back while we were loading
      }
      this.paint(result.cards, {
        machine,
        reachable: result.reachable,
        loading: false,
        error: result.error,
      });
      return;
    }
    this.paint(await collectCards(), { machine, reachable: true, loading: false });
  }

  private paint(cards: SessionCard[], view: HomeView): void {
    const codiconUri = this.panel.webview.asWebviewUri(
      vscode.Uri.joinPath(this.context.extensionUri, 'media', 'codicon.css'),
    );
    const nonce = randomBytes(16).toString('base64');
    this.panel.webview.html = renderHtml(
      cards,
      codiconUri.toString(),
      this.panel.webview.cspSource,
      nonce,
      view,
    );
  }
}
