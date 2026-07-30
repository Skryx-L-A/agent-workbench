// A2 — Worker sidebar: state file + live tmux panes, polled on the interval from
// the settings file (workerPollSeconds). V2: the sidebar shows the workers of the
// session that is open in this window, not just of the folder.
import * as vscode from 'vscode';
import { DEFAULT_SETTINGS } from './settings.ts';
import { loadWorkers, resultTooltip, STATUS_LABEL, type WorkerView } from './workers.ts';

export class WorkerTreeItem extends vscode.TreeItem {
  constructor(public readonly worker: WorkerView) {
    super(worker.name, vscode.TreeItemCollapsibleState.None);
    const model = worker.model ?? worker.kind ?? 'unbekannt';
    // A worker spawned with `claude-worker --on <machine>` runs over there; the
    // pane here is only its mirror, so the machine belongs on the entry.
    const machine = worker.machine ? ` · ${worker.machine}` : '';
    this.description = `${model} — ${STATUS_LABEL[worker.status]}${machine}`;
    this.iconPath = new vscode.ThemeIcon(
      worker.status === 'running' ? 'debug-start'
        : worker.status === 'dead' ? 'error'
          : 'circle-outline',
      worker.status === 'running'
        ? new vscode.ThemeColor('testing.iconPassed')
        : worker.status === 'dead'
          ? new vscode.ThemeColor('testing.iconFailed')
          : undefined,
    );
    this.tooltip = new vscode.MarkdownString(
      [
        `**${worker.name}**`,
        `Modell: ${model}`,
        `Status: ${STATUS_LABEL[worker.status]}`,
        worker.machine ? `Maschine: ${worker.machine}` : undefined,
        resultTooltip(worker),
      ].filter((line): line is string => line !== undefined).join('\n\n'),
    );
    this.contextValue = worker.paneId ? 'worker-live' : 'worker';
    if (worker.paneId) {
      this.command = {
        command: 'claude-workbench.focusWorker',
        title: 'Worker-Pane fokussieren',
        arguments: [worker.paneId],
      };
    }
  }
}

export class WorkersProvider implements vscode.TreeDataProvider<WorkerTreeItem>, vscode.Disposable {
  private readonly changed = new vscode.EventEmitter<void>();
  readonly onDidChangeTreeData = this.changed.event;
  private timer: NodeJS.Timeout;
  private signature = '';
  private pollSeconds = DEFAULT_SETTINGS.workerPollSeconds;
  /** Session of the folder this window shows; undefined = the default session. */
  private sessionKey: string | undefined;
  /**
   * Called with every fresh worker list. The extension uses it to check that
   * running workers are actually VISIBLE in this window (worker tab, SPEC-V2 C).
   */
  onWorkers: ((workers: WorkerView[]) => void) | undefined;

  constructor(sessionKey?: string) {
    this.sessionKey = sessionKey;
    this.timer = setInterval(() => void this.poll(), this.pollSeconds * 1000);
  }

  dispose(): void {
    clearInterval(this.timer);
    this.changed.dispose();
  }

  refresh(): void {
    this.signature = '';
    this.changed.fire();
  }

  /** Which session of the folder the sidebar reports on. */
  setSessionKey(sessionKey: string | undefined): void {
    this.sessionKey = sessionKey;
    this.refresh();
  }

  /** Takes the poll interval from the settings; a no-op when it is unchanged. */
  setPollSeconds(seconds: number): void {
    if (seconds === this.pollSeconds) {
      return;
    }
    this.pollSeconds = seconds;
    clearInterval(this.timer);
    this.timer = setInterval(() => void this.poll(), this.pollSeconds * 1000);
  }

  getTreeItem(item: WorkerTreeItem): vscode.TreeItem {
    return item;
  }

  async getChildren(): Promise<WorkerTreeItem[]> {
    const workers = await this.currentWorkers();
    this.signature = signatureOf(workers);
    return workers.map((worker) => new WorkerTreeItem(worker));
  }

  private async currentWorkers(): Promise<WorkerView[]> {
    const dir = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
    const workers = dir ? await loadWorkers(dir, this.sessionKey) : [];
    this.onWorkers?.(workers);
    return workers;
  }

  /** Only fires a redraw when something actually changed. Never rejects. */
  private async poll(): Promise<void> {
    try {
      const workers = await this.currentWorkers();
      const next = signatureOf(workers);
      if (next !== this.signature) {
        this.signature = next;
        this.changed.fire();
      }
    } catch {
      // transient fs/tmux error — the next tick tries again
    }
  }
}

function signatureOf(workers: WorkerView[]): string {
  return workers
    .map((w) => [
      w.name,
      w.status,
      w.paneId ?? '',
      w.result?.mtime ?? 0,
      w.machine ?? '',
      w.resultUnreachable ? 'unreachable' : '',
    ].join(':'))
    .join('|');
}
