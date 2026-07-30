// Einstellungen panel — the only writer of ~/.claude/workbench/settings.json
// (SPEC-V2, section A). Changes take effect immediately: the file is written on
// every control change and re-read by the shell layer on the next start/spawn.
//
// The "Modelle & Harnesses" section (SPEC-V3 D) reads models.json directly
// (readModelsRegistry) but writes ONLY through wb-state (modelsCli.ts) — this
// panel never validates or locks the registry itself, that is wb-state's job.
// The one write path this panel DOES own end-to-end is a provider API key
// (providerSecrets.ts), because a key must never cross a process boundary as a
// CLI argument.
import { randomBytes } from 'node:crypto';
import * as vscode from 'vscode';
import {
  addHarness,
  addModel,
  addProvider,
  checkModel,
  discoverModels,
  harnessBinaryPresent,
  probeHarness,
  removeEntry,
  type RegistryKind,
  setField,
} from './modelsCli.ts';
import {
  allowedEfforts,
  effectiveHarnesses,
  effectiveModels,
  effectiveProviders,
  modelsForRole,
  readModelsRegistry,
  type RegistryHarness,
  type RegistryModel,
  unknownPlaceholdersInHarness,
  type Provider as RegistryProvider,
} from './models.ts';
import { providerKeyStatus, saveProviderKey, type ProviderKeyStatus } from './providerSecrets.ts';
import { readHooks } from './hooksInfo.ts';
import { mcpShared, type McpSharedAction } from './systemCli.ts';
import { collectLaunchdJobs, collectPathInfo } from './systemInfo.ts';
import {
  coerceSetting,
  defaultModelFor,
  effortsForModel,
  isModelForHarness,
  readSettings,
  type RegistryHooks,
  type Settings,
  writeSetting,
} from './settings.ts';
import { renderSettingsHtml } from './settingsHtml.ts';

async function collectRenderData(): Promise<{
  registry: Awaited<ReturnType<typeof readModelsRegistry>>;
  keyStatuses: Record<string, ProviderKeyStatus>;
  binaryPresence: Record<string, boolean>;
}> {
  const registry = await readModelsRegistry();
  const keyStatuses: Record<string, ProviderKeyStatus> = {};
  for (const provider of effectiveProviders(registry)) {
    if (provider.kind !== 'local') {
      keyStatuses[provider.id] = await providerKeyStatus(provider);
    }
  }
  // One check per unique harness command (SPEC-V3 D item 2's "Binary fehlt"
  // state) — a handful of exec calls at most, never per model.
  const binaryPresence: Record<string, boolean> = {};
  const commands = new Map(effectiveHarnesses(registry).map((h) => [h.id, h.command]));
  for (const [harnessId, command] of commands) {
    binaryPresence[harnessId] = await harnessBinaryPresent(command);
  }
  return { registry, keyStatuses, binaryPresence };
}

/**
 * Registry-aware resolvers for applyChange/writeSetting (SPEC-V3 D): a harness
 * switch to a non-claude/pi adapter picks its first orchestrator-role model
 * instead of falling back to a Claude id, and effort clamping for a registry
 * model uses its OWN maxEffort/efforts, not the Claude/fable-only rule.
 */
function registryHooksFor(registry: Awaited<ReturnType<typeof readModelsRegistry>>): RegistryHooks {
  return {
    defaultModelFor: (harness) => {
      if (harness === 'claude' || harness === 'pi') {
        return defaultModelFor(harness);
      }
      const first = modelsForRole(registry, { role: 'orchestrator', harness })[0];
      return first?.id ?? defaultModelFor(harness);
    },
    effortsForModel: (modelId) => {
      const model = effectiveModels(registry).find((m) => m.id === modelId);
      return model ? allowedEfforts(model) : effortsForModel(modelId);
    },
    modelFitsHarness: (harness, model) => {
      if (harness === 'claude' || harness === 'pi') {
        return isModelForHarness(harness, model);
      }
      return typeof model === 'string' && modelsForRole(registry, { role: 'orchestrator', harness }).some((m) => m.id === model);
    },
  };
}

export class SettingsPanel {
  private static current: SettingsPanel | undefined;

  /** Called after every successful write so the running extension can react. */
  static onChanged: ((settings: Settings) => void) | undefined;

  /**
   * Actor column for the change log — set by the extension so a line names the
   * window that wrote it ('extension:<sessionKey>' / 'extension:<folder>').
   */
  static actor = 'extension';

  /** Where a failing change log is reported (output channel). */
  static onLogError: ((message: string) => void) | undefined;

  static async show(context: vscode.ExtensionContext): Promise<void> {
    if (SettingsPanel.current) {
      SettingsPanel.current.panel.reveal(vscode.ViewColumn.One);
      await SettingsPanel.current.render();
      void SettingsPanel.current.kickOffDiscovery();
      return;
    }
    const panel = vscode.window.createWebviewPanel(
      'claudeWorkbench.settings',
      'Claude Workbench — Einstellungen',
      vscode.ViewColumn.One,
      {
        enableScripts: true,
        retainContextWhenHidden: true,
        localResourceRoots: [vscode.Uri.joinPath(context.extensionUri, 'media')],
      },
    );
    SettingsPanel.current = new SettingsPanel(panel, context);
    await SettingsPanel.current.render();
    void SettingsPanel.current.kickOffDiscovery();
  }

  private constructor(
    private readonly panel: vscode.WebviewPanel,
    private readonly context: vscode.ExtensionContext,
  ) {
    panel.onDidDispose(() => {
      SettingsPanel.current = undefined;
    });
    panel.webview.onDidReceiveMessage(async (message: any) => {
      switch (message?.command) {
        case 'set':
          await this.handleSet(message);
          return;
        case 'models-check':
          await this.handleModelsAction(() => checkModel(message.id), `Prüfen (${message.id})`);
          return;
        case 'models-probe-harness':
          await this.handleModelsAction(
            () => probeHarness(message.id),
            `Ready-Muster messen (${message.id})`,
          );
          return;
        case 'models-discover':
          await this.handleModelsMutation(
            () => discoverModels(message.harnessId ? [message.harnessId] : undefined, { ifStale: false }),
            message.harnessId ? `Modelle neu einlesen (${message.harnessId})` : 'Kataloge jetzt aktualisieren',
          );
          return;
        case 'models-remove':
          await this.handleModelsMutation(
            () => removeEntry(message.kind as RegistryKind, message.id),
            `Entfernen (${message.id})`,
          );
          return;
        case 'models-add-model':
          await this.handleModelsMutation(
            () => addModel(message.model as RegistryModel),
            `Modell hinzufügen (${message.model?.id})`,
          );
          return;
        case 'models-add-harness': {
          const draft = message.harness as RegistryHarness;
          const unknown = unknownPlaceholdersInHarness(draft);
          if (unknown.length > 0) {
            await this.render(`Harness nicht hinzugefügt — unbekannte Platzhalter: ${unknown.join(', ')}`);
            return;
          }
          await this.handleModelsMutation(() => addHarness(draft), `Harness hinzufügen (${draft.id})`);
          return;
        }
        case 'models-add-provider':
          await this.handleModelsMutation(
            () => addProvider(message.provider as RegistryProvider),
            `Provider hinzufügen (${message.provider?.id})`,
          );
          return;
        case 'provider-key-save':
          await this.handleModelsMutation(
            () => saveProviderKey(message.providerId, message.key),
            `Key speichern (${message.providerId})`,
          );
          return;
        // Generic single-field write (SPEC-V3 D items 2+4): the model 'enabled'
        // toggle and a provider's 'loginCheckPath' both go through the same
        // `wb-state models set` path addModel/addHarness/addProvider already use.
        case 'models-set-field':
          await this.handleModelsMutation(
            () => setField(message.kind as RegistryKind, message.id, message.field, message.value),
            `${message.field} setzen (${message.id})`,
          );
          return;
        // Nachtrag priority 3: the shared MCP-server LaunchAgents. In-place
        // update (no full re-render) — this section is independent of
        // settings/registry state, and a full HTML replace would race the
        // fresh webview's own message listener the way handleModelsMutation's
        // comment already documents for the registry actions.
        case 'mcp-shared':
          await this.handleMcpShared(message.action as McpSharedAction);
          return;
        // Nachtrag priority 4: "anzeigen, öffnen können" — a standard VS Code
        // command that reveals the path in the OS file manager. Read-only
        // (Finder), the panel state itself never changes, no re-render needed.
        case 'open-path':
          if (typeof message.path === 'string') {
            await vscode.commands.executeCommand('revealFileInOS', vscode.Uri.file(message.path));
          }
          return;
      }
    });
  }

  /**
   * Vertrag discover.md §5: the settings page itself triggers one `discover
   * --all --if-stale` per open, in the background — never blocks the page and
   * never surfaces an error (a missing/not-yet-built `wb-state discover`
   * subcommand must stay invisible here, unlike the explicit "Modelle neu
   * einlesen" button, which does report failures via handleModelsMutation).
   * On success the panel is rebuilt so newly discovered models show up
   * without the user having to do anything.
   */
  private async kickOffDiscovery(): Promise<void> {
    // Vertrag Teil 2 §5: `modelDiscoveryAuto` (default true) gates the
    // network-catalog part of this AUTOMATIC trigger only — an explicit
    // click on "Kataloge jetzt aktualisieren" always runs the full thing,
    // untouched by this flag (see the models-discover message case below).
    const settings = await readSettings().catch(() => undefined);
    const network = settings?.modelDiscoveryAuto !== false;
    const result = await discoverModels(undefined, { ifStale: true, network }).catch(() => undefined);
    if (!result?.ok || SettingsPanel.current !== this) {
      return;
    }
    await this.render(result.message);
  }

  private async handleMcpShared(action: McpSharedAction): Promise<void> {
    const result = await mcpShared(action);
    if (!result.ok) {
      SettingsPanel.onLogError?.(`mcp-shared ${action}: ${result.message}`);
    }
    // status itself is the fresh state; every other verb needs a follow-up
    // status call to show its effect (the CLI's own action output is not the
    // status table).
    const statusResult = action === 'status' ? result : await mcpShared('status');
    this.panel.webview.postMessage({
      command: 'mcp-shared-status',
      text: statusResult.message,
      actionText: action === 'status' ? '' : `${action}: ${result.message}`,
    });
  }

  private async handleSet(message: { key: string; value: unknown }): Promise<void> {
    // The stored harness decides which model ids are valid (SPEC-V2 F).
    const stored = await readSettings();
    const value = coerceSetting(message.key, message.value, stored.orchestratorHarness);
    if (value === undefined) {
      this.panel.webview.postMessage({ command: 'saved', text: 'Wert nicht übernommen.' });
      return;
    }
    try {
      const registry = await readModelsRegistry();
      const settings = await writeSetting(message.key, value, {
        actor: SettingsPanel.actor,
        onLogError: (text) => SettingsPanel.onLogError?.(text),
        registryHooks: registryHooksFor(registry),
      });
      SettingsPanel.onChanged?.(settings);
      // A harness switch changes the whole model control — repaint it.
      if (message.key === 'orchestratorHarness') {
        await this.render();
        return;
      }
      this.panel.webview.postMessage({ command: 'saved', text: 'Gespeichert.' });
    } catch (error) {
      this.panel.webview.postMessage({
        command: 'saved',
        text: `Nicht gespeichert: ${(error as Error)?.message ?? error}`,
      });
    }
  }

  /** Read-only registry actions (check/probe): no data changed, so the live panel is left alone. */
  private async handleModelsAction(
    run: () => Promise<{ ok: boolean; message: string }>,
    label: string,
  ): Promise<void> {
    const result = await this.runSafely(run, label);
    this.panel.webview.postMessage({ command: 'models-status', text: `${label}: ${result}` });
  }

  /**
   * A registry-mutating action: the result is baked into the NEXT render's
   * #modelsStatus (renderSettingsHtml's modelsStatusText) instead of a
   * postMessage sent after replacing panel.webview.html — a postMessage right
   * after a full HTML replace can race the fresh script's message listener.
   */
  private async handleModelsMutation(
    run: () => Promise<{ ok: boolean; message: string }>,
    label: string,
  ): Promise<void> {
    const result = await this.runSafely(run, label);
    await this.render(`${label}: ${result}`);
  }

  /** Never lets a command failure vanish — SPEC-V3 D: "nie verschluckt". */
  private async runSafely(run: () => Promise<{ ok: boolean; message: string }>, label: string): Promise<string> {
    try {
      const result = await run();
      return result.message;
    } catch (error) {
      const text = `Fehlgeschlagen: ${(error as Error)?.message ?? error}`;
      SettingsPanel.onLogError?.(`${label}: ${text}`);
      return text;
    }
  }

  private async render(modelsStatusText?: string): Promise<void> {
    const codiconUri = this.panel.webview.asWebviewUri(
      vscode.Uri.joinPath(this.context.extensionUri, 'media', 'codicon.css'),
    );
    const nonce = randomBytes(16).toString('base64');
    const [settings, { registry, keyStatuses, binaryPresence }, mcpSharedStatus, hooks, pathInfo, launchdJobs] = await Promise.all([
      readSettings(),
      collectRenderData(),
      mcpShared('status'),
      readHooks(),
      collectPathInfo(),
      collectLaunchdJobs(),
    ]);
    this.panel.webview.html = renderSettingsHtml(
      settings,
      codiconUri.toString(),
      this.panel.webview.cspSource,
      nonce,
      registry,
      keyStatuses,
      modelsStatusText,
      binaryPresence,
      mcpSharedStatus.message,
      hooks,
      pathInfo,
      launchdJobs,
    );
  }
}
