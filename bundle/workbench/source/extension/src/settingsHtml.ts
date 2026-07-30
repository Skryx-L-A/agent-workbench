// Pure rendering for the Einstellungen webview (no vscode import — unit-testable).
import { escapeHtml } from './format.ts';
import {
  allowedEfforts,
  effectiveHarnesses,
  findHarness,
  findProvider,
  type ModelsRegistry,
  modelSupportsEffort,
  modelsForRole,
  parseModelsRegistry,
  type RegistryModel,
} from './models.ts';
import type { HookEntry } from './hooksInfo.ts';
import { modelsSectionScript, renderModelsSection } from './modelsHtml.ts';
import type { ProviderKeyStatus } from './providerSecrets.ts';
import type { LaunchdJob, PathInfo } from './systemInfo.ts';
import {
  EFFORTS,
  type Effort,
  effortLabel,
  MODEL_LABEL,
  MODELS,
  PI_MODEL_LABEL,
  PI_MODELS,
  type Settings,
  settingsFile,
  WORKER_LAYOUT_LABEL,
  WORKER_MACHINE_LABEL,
  WORKER_MACHINES,
} from './settings.ts';

/** Marks the "own Ollama id" entry of the pi model list. */
const CUSTOM_MODEL = '__custom__';

interface EffortMeta {
  efforts: Effort[];
  supportsEffort: boolean;
}

/** Per-model client-side clamp data (SPEC-V3 D item 3), covering every id any select on this page can hold. */
function effortMeta(model: RegistryModel, registry: ModelsRegistry): EffortMeta {
  const harness = findHarness(registry, model.harness);
  const provider = findProvider(registry, model.provider);
  return { efforts: allowedEfforts(model), supportsEffort: modelSupportsEffort(model, harness, provider) };
}

export function renderSettingsHtml(
  settings: Settings,
  codiconHref: string,
  cspSource: string,
  nonce: string,
  modelsRegistry: ModelsRegistry = parseModelsRegistry(undefined),
  providerKeyStatus: Record<string, ProviderKeyStatus> = {},
  modelsStatusText?: string,
  /** One `which`/exec-bit check per unique harness command — see modelsHtml.ts. */
  harnessBinaryPresence: Record<string, boolean> = {},
  /** Raw `mcp-shared status` output (Nachtrag priority 3) — undefined before the first fetch. */
  mcpSharedStatusText?: string,
  /** ~/.claude/settings.json's hooks, flattened — read-only, see hooksInfo.ts. */
  hooks: HookEntry[] = [],
  /** Vault/Worker-Ergebnisse/Snapshot-Ordner existence+size — read-only, see systemInfo.ts. */
  pathInfo: PathInfo[] = [],
  /** The five launchd jobs CLAUDE.md names — read-only, see systemInfo.ts. */
  launchdJobs: LaunchdJob[] = [],
): string {
  const pi = settings.orchestratorHarness === 'pi';
  const claude = settings.orchestratorHarness === 'claude';
  const customModel = pi && !(PI_MODELS as readonly string[]).includes(settings.orchestratorModel);
  // SPEC-V3 D: the orchestrator harness is no longer a fixed claude|pi choice —
  // every harness in the registry (built-in claude/pi plus every registered
  // adapter: codex, agy, aider, opencode, …) is a valid orchestrator pane.
  const harnessChoices = effectiveHarnesses(modelsRegistry);
  // Orchestrator/Worker-Modellauswahl ziehen ihre Optionen aus der Registry
  // (SPEC-V3 D). `claude` stays filtered to `.builtin` because orchestratorModel
  // for harness 'claude' is validated against exactly the five canonical ids
  // (settings.ts isModelForHarness) — the policy caps (fable's medium ceiling,
  // effort clamping) hang off those five ids specifically. Any OTHER harness has
  // no such fixed set: its options are simply every registered model for that
  // harness with the 'orchestrator' role.
  const claudeModels = modelsForRole(modelsRegistry, { role: 'orchestrator', harness: 'claude' })
    .filter((m) => m.builtin);
  const claudeModelOptions = claudeModels.map((m) => ({ id: m.id, label: m.label }));
  const registryOrchestratorModels = claude || pi
    ? []
    : modelsForRole(modelsRegistry, { role: 'orchestrator', harness: settings.orchestratorHarness });
  // Worker default model is harness-INDEPENDENT (SPEC-V3 D, Reviewer-Befund M6):
  // `claude-worker <name> default …` resolves whatever is stored here through
  // the registry regardless of what the orchestrator itself runs, so the list is
  // every worker-role model across every harness, not scoped to one.
  const workerModels = modelsForRole(modelsRegistry, { role: 'worker' });
  const workerModelOptions = workerModels.map((m) => {
    const h = findHarness(modelsRegistry, m.harness);
    return { id: m.id, label: `${m.label} — ${m.id} (${h?.label ?? m.harness})` };
  });
  // The webview clamps the effort choice to the model's ceiling, and hides the
  // whole effort row for a model that takes no effort flag at all (item 3) —
  // both without a round trip. Covers every id any select on this page can hold.
  const effortMetaMap: Record<string, EffortMeta> = {};
  for (const model of [...claudeModels, ...registryOrchestratorModels, ...workerModels]) {
    effortMetaMap[model.id] = effortMeta(model, modelsRegistry);
  }

  return `<!DOCTYPE html>
<html lang="de">
<head>
<meta charset="utf-8">
<meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src ${cspSource} 'unsafe-inline'; font-src ${cspSource}; script-src 'nonce-${nonce}';">
<link rel="stylesheet" href="${codiconHref}">
<title>Claude Workbench — Einstellungen</title>
<style>
  body {
    font-family: var(--vscode-font-family);
    font-size: var(--vscode-font-size);
    color: var(--vscode-foreground);
    background: var(--vscode-editor-background);
    margin: 0;
    padding: 32px 40px 48px;
    line-height: 1.5;
  }
  h1 { font-size: 1.5rem; font-weight: 600; margin: 0 0 4px; letter-spacing: -0.01em; }
  h2 { font-size: 0.8rem; font-weight: 600; text-transform: uppercase; letter-spacing: 0.06em;
       color: var(--vscode-descriptionForeground); margin: 32px 0 12px; }
  .file { color: var(--vscode-descriptionForeground); font-size: 0.8rem;
          font-family: var(--vscode-editor-font-family); word-break: break-all; }
  .field { display: grid; grid-template-columns: minmax(200px, 280px) 1fr; gap: 16px;
           align-items: start; padding: 10px 0;
           border-bottom: 1px solid var(--vscode-panel-border, rgba(128,128,128,0.18)); }
  .field:last-of-type { border-bottom: none; }
  label.title { font-weight: 600; }
  .hint { color: var(--vscode-descriptionForeground); font-size: 0.8rem; }
  select, input[type="text"], input[type="number"], input[type="password"] {
    font-family: inherit; font-size: 0.85rem;
    background: var(--vscode-input-background, transparent);
    color: var(--vscode-input-foreground, inherit);
    border: 1px solid var(--vscode-input-border, rgba(128,128,128,0.35));
    border-radius: 4px; padding: 5px 8px; min-width: 220px;
  }
  select:focus, input:focus { outline: 1px solid var(--vscode-focusBorder); outline-offset: -1px; }
  input[type="checkbox"] { accent-color: var(--vscode-button-background); width: 16px; height: 16px; }
  .segmented { display: inline-flex; border: 1px solid var(--vscode-panel-border, rgba(128,128,128,0.3));
               border-radius: 6px; overflow: hidden; }
  .segmented button {
    font-family: inherit; font-size: 0.8rem; border: none; border-radius: 0; cursor: pointer;
    background: transparent; color: var(--vscode-foreground); padding: 6px 14px;
  }
  .segmented button.active { background: var(--vscode-button-background); color: var(--vscode-button-foreground); }
  .segmented button:not(.active):hover { background: var(--vscode-toolbar-hoverBackground, rgba(128,128,128,0.15)); }
  #customModelRow { margin-top: 8px; }
  .layout-help { margin-top: 8px; display: flex; flex-direction: column; gap: 3px; max-width: 60ch; }
  .status { margin-top: 24px; color: var(--vscode-descriptionForeground); font-size: 0.8rem;
            min-height: 1.2em; display: flex; align-items: center; gap: 6px; }
  .codicon { font-size: 14px; }
  h3 { font-size: 0.95rem; font-weight: 600; margin: 24px 0 8px; }
  table.registry { width: 100%; border-collapse: collapse; font-size: 0.82rem; margin-bottom: 8px; }
  table.registry th, table.registry td {
    text-align: left; padding: 6px 8px; vertical-align: top;
    border-bottom: 1px solid var(--vscode-panel-border, rgba(128,128,128,0.18));
  }
  table.registry th { color: var(--vscode-descriptionForeground); font-weight: 600; font-size: 0.75rem;
                       text-transform: uppercase; letter-spacing: 0.04em; }
  table.registry td.fit { max-width: 32ch; }
  table.registry td.actions { white-space: nowrap; }
  .idHint { color: var(--vscode-descriptionForeground); font-size: 0.72rem; font-family: var(--vscode-editor-font-family); }
  .dot { display: inline-block; width: 8px; height: 8px; border-radius: 50%; margin-right: 4px; }
  .autoBadge { color: var(--vscode-descriptionForeground); font-size: 0.72rem; margin-top: 2px; }
  .discoverFields { margin: 4px 0 8px; padding-left: 8px;
                     border-left: 2px solid var(--vscode-panel-border, rgba(128,128,128,0.25)); }
  .statusLabel { font-size: 0.78rem; color: var(--vscode-descriptionForeground); }
  .enabledToggle { display: inline-flex; align-items: center; gap: 4px; font-size: 0.8rem; margin-right: 8px; }
  .enabledToggle input { width: auto; }
  .mcpStatus { background: var(--vscode-textCodeBlock-background, rgba(128,128,128,0.1));
               padding: 8px 10px; border-radius: 4px; font-family: var(--vscode-editor-font-family);
               font-size: 0.8rem; white-space: pre-wrap; max-width: 60ch; }
  tr.denyHook { background: color-mix(in srgb, var(--vscode-editorWarning-foreground, #d29922) 10%, transparent); }
  .denyHookWarning { color: var(--vscode-editorWarning-foreground, #d29922); font-size: 0.78rem; }
  button.danger { color: var(--vscode-errorForeground, #f85149); }
  .keyInput { min-width: 140px; margin-right: 4px; }
  .keyPair { display: inline-flex; align-items: center; gap: 4px; }
  details.addForm { margin: 8px 0 20px; border: 1px solid var(--vscode-panel-border, rgba(128,128,128,0.25));
                     border-radius: 6px; padding: 4px 12px; }
  details.addForm summary { cursor: pointer; padding: 8px 0; font-weight: 600; }
  details.addForm .field { grid-template-columns: minmax(140px, 220px) 1fr; }
  .catalogStatus { margin-top: 2px; }
  /* Task item 2: type-to-filter overlay over a (visually hidden) <select> — see modelPickerMarkup/wireModelPicker. */
  select.modelPickerSelect { display: none; }
  .modelPicker { position: relative; display: inline-block; }
  .modelPicker .modelPickerSearch { min-width: 320px; }
  .modelPickerList {
    position: absolute; z-index: 20; top: 100%; left: 0; margin-top: 2px;
    min-width: 320px; max-height: 260px; overflow-y: auto;
    background: var(--vscode-dropdown-background, var(--vscode-editor-background));
    border: 1px solid var(--vscode-dropdown-border, var(--vscode-panel-border, rgba(128,128,128,0.35)));
    border-radius: 4px; box-shadow: 0 2px 8px rgba(0,0,0,0.25);
  }
  .modelPickerItem { padding: 5px 10px; font-size: 0.85rem; cursor: pointer; }
  .modelPickerItem:hover { background: var(--vscode-list-hoverBackground, rgba(128,128,128,0.15)); }
  .modelPickerItem.current { font-weight: 600; }
  /* Task item 1: model-table search/filter bar and pager. */
  .modelSearchBar { display: flex; flex-wrap: wrap; gap: 8px; margin: 8px 0; }
  .modelSearchBar input[type="text"] { min-width: 260px; flex: 1 1 260px; }
  .modelPager { display: flex; align-items: center; gap: 10px; margin: 4px 0 10px;
                font-size: 0.8rem; color: var(--vscode-descriptionForeground); }
</style>
</head>
<body>
<h1>Einstellungen</h1>
<div class="file">${escapeHtml(settingsFile())}</div>

<h2>Orchestrator</h2>
<div class="field">
  <div>
    <label class="title">Harness</label>
    <div class="hint">Was im Orchestrator-Pane läuft — jeder in „Modelle & Harnesses" registrierte Adapter, nicht nur Claude Code und pi.</div>
  </div>
  <div class="segmented">
${harnessChoices.map((h) => `    <button data-harness="${escapeHtml(h.id)}" class="${h.id === settings.orchestratorHarness ? 'active' : ''}">${escapeHtml(h.label)}</button>`).join('\n')}
  </div>
</div>
${orchestratorModelSection(settings, claudeModelOptions, registryOrchestratorModels, pi, claude, customModel, modelsRegistry)}

<h2>Worker</h2>
<div class="field">
  <div>
    <label class="title">Layout</label>
    <div class="hint">Wo die Worker-Panes liegen. Die Umstellung greift sofort, laufende Worker bleiben am Leben.</div>
  </div>
  <div>
    <div class="segmented">
${(['split', 'window'] as const).map((layout) => `      <button data-layout="${layout}" class="${layout === settings.workerLayout ? 'active' : ''}">${escapeHtml(WORKER_LAYOUT_LABEL[layout])}</button>`).join('\n')}
    </div>
    <div class="hint layout-help">
      <div>Panes neben dem Orchestrator: alles in einem Terminal, Worker unter dem Orchestrator.</div>
      <div>Eigener Tab, alle Worker zusammen: zweiter Terminal-Tab „Claude Workbench — Worker" im selben Fenster, umschaltbar in der Terminal-Liste rechts.</div>
    </div>
  </div>
</div>
${workerModelSection(settings, workerModelOptions, workerModels, modelsRegistry)}
<div class="field">
  <div>
    <label class="title" for="workerPollSeconds">Sidebar-Poll-Intervall</label>
    <div class="hint">Sekunden zwischen zwei Worker-Abfragen (1–300).</div>
  </div>
  <div><input type="number" id="workerPollSeconds" min="1" max="300" step="1" value="${settings.workerPollSeconds}"></div>
</div>
<div class="field">
  <div>
    <label class="title" for="maxWorkers">Maximale Worker je Session</label>
    <div class="hint">claude-worker/pi-worker lehnen einen weiteren Spawn ab, sobald eine Session so viele Worker-Panes hält (mindestens 1). Bestehende Panes werden weiter wiederverwendet.</div>
  </div>
  <div><input type="number" id="maxWorkers" min="1" step="1" value="${settings.maxWorkers}"></div>
</div>
<div class="field">
  <div>
    <label class="title">Default-Maschine für Worker</label>
    <div class="hint">Wohin ein claude-worker-Aufruf ohne explizite Maschine (--on) und ohne SSH-Namen geht.</div>
  </div>
  <div class="segmented">
${WORKER_MACHINES.map((machine) => `    <button data-worker-machine="${machine}" class="${machine === settings.defaultWorkerMachine ? 'active' : ''}">${escapeHtml(WORKER_MACHINE_LABEL[machine])}</button>`).join('\n')}
  </div>
</div>

<h2>Kontext-Guard</h2>
<div class="field">
  <div>
    <label class="title" for="guardOrchWarnPct">Orchestrator-Warnschwelle</label>
    <div class="hint">Kontext-Prozent (1–99), ab dem context-guard den Orchestrator zum Sichern von Wissen auffordert.</div>
  </div>
  <div><input type="number" id="guardOrchWarnPct" min="1" max="99" step="1" value="${settings.guardOrchWarnPct}"></div>
</div>
<div class="field">
  <div>
    <label class="title" for="guardWorkerWarnPct">Worker-Warnschwelle</label>
    <div class="hint">Kontext-Prozent (1–99), ab dem context-guard einen Worker zur Übergabe + /compact auffordert.</div>
  </div>
  <div><input type="number" id="guardWorkerWarnPct" min="1" max="99" step="1" value="${settings.guardWorkerWarnPct}"></div>
</div>
<div class="field">
  <div>
    <label class="title" for="minWorkerPaneWidth">Mindestbreite Worker-Pane</label>
    <div class="hint">Spaltenbreite (mindestens 20), unter die wb-grid einen Worker-Pane nicht drückt — sonst schneidet tmux die Statuszeile ab und der Guard wird blind. wb-grid warnt auf stderr, wenn eine Zeile enger würde.</div>
  </div>
  <div><input type="number" id="minWorkerPaneWidth" min="20" step="1" value="${settings.minWorkerPaneWidth}"></div>
</div>
<div class="field">
  <div>
    <label class="title" for="contextGuardAutostart">context-guard beim Start automatisch starten</label>
    <div class="hint">Standardmäßig AUS (Entscheidung des Nutzers 2026-07-27): der Orchestrator startet seinen
      eigenen Guard, sobald Worker existieren — ein zusätzlicher automatisch gestarteter Guard würde
      <code>/compact</code> doppelt tippen. Nur einschalten, wenn wirklich kein Guard mehr manuell gestartet wird.</div>
  </div>
  <div><input type="checkbox" id="contextGuardAutostart"${settings.contextGuardAutostart ? ' checked' : ''}></div>
</div>

<h2>Modell-Erkennung</h2>
<div class="field">
  <div>
    <label class="title" for="modelDiscoveryAuto">Automatisch beim Start/Öffnen aktualisieren</label>
    <div class="hint">An (Vorgabe): <code>wb-code</code>-Start und das Öffnen dieser Seite stoßen
      <code>wb-state models discover --all --if-stale</code> an, inklusive Netzabruf bei
      Anbietern mit hinterlegtem Key (OpenRouter, OpenAI, …). Aus: nur noch lokale Quellen
      (installierte Ollama-Modelle, agy/codex/opencode) laufen automatisch weiter — sie kosten
      nichts und verlassen die Maschine nicht. Der Knopf „Kataloge jetzt aktualisieren" unten bei
      „Modelle & Harnesses" fragt unabhängig von diesem Schalter immer alles ab.</div>
  </div>
  <div><input type="checkbox" id="modelDiscoveryAuto"${settings.modelDiscoveryAuto ? ' checked' : ''}></div>
</div>

<h2>Start</h2>
<div class="field">
  <div>
    <label class="title" for="terminalStartMaximized">Terminal maximiert starten</label>
    <div class="hint">Das Orchestrator-Panel nimmt beim Start die volle Höhe ein.</div>
  </div>
  <div><input type="checkbox" id="terminalStartMaximized"${settings.terminalStartMaximized ? ' checked' : ''}></div>
</div>
<div class="field">
  <div>
    <label class="title" for="newSessionDefaultDir">Default-Ordner für neue Sessions</label>
    <div class="hint">Startpunkt des Ordner-Dialogs, z. B. ~/AI.</div>
  </div>
  <div><input type="text" id="newSessionDefaultDir" value="${escapeHtml(settings.newSessionDefaultDir)}" spellcheck="false"></div>
</div>

<div class="status" id="status"></div>

<h2>System</h2>
<div class="field">
  <div>
    <label class="title">Geteilte MCP-Server</label>
    <div class="hint">basic-memory/playwright laufen als LaunchAgents statt pro Session (~/.claude/CLAUDE.md).
      Nach einem Plugin-Update „Konfiguration erneut anwenden", sonst fällt Playwright auf stdio zurück.</div>
  </div>
  <div>
    <pre class="mcpStatus" id="mcpSharedStatus">${escapeHtml(mcpSharedStatusText ?? 'Noch nicht geladen.')}</pre>
    <button type="button" id="mcpSharedRefresh">Status aktualisieren</button>
    <button type="button" id="mcpSharedRestart">Neustart</button>
    <button type="button" id="mcpSharedApply">Konfiguration erneut anwenden</button>
    <button type="button" id="mcpSharedReap">Verwaiste Helferprozesse aufräumen</button>
    <div class="status" id="mcpSharedActionStatus"></div>
  </div>
</div>
${renderHooksSection(hooks)}
${renderPathsAndAutomations(pathInfo, launchdJobs)}

${renderModelsSection(modelsRegistry, providerKeyStatus, modelsStatusText, harnessBinaryPresence)}

<script nonce="${nonce}">
  const vscode = acquireVsCodeApi();
  // Per-model {efforts, supportsEffort} (SPEC-V3 D item 3) — ids not in this map
  // (pi aliases, a custom Ollama id) fall back to "supports everything", the
  // legacy behaviour every existing pi/claude flow already relied on.
  const EFFORT_META = ${JSON.stringify(effortMetaMap)};
  const status = document.getElementById('status');

  function set(key, value) {
    vscode.postMessage({ command: 'set', key, value });
  }

  function syncEfforts(modelId, effortId) {
    const modelEl = document.getElementById(modelId);
    const effortEl = document.getElementById(effortId);
    const row = document.getElementById(effortId + 'Row');
    const rowHint = document.getElementById(effortId + 'RowHint');
    if (!modelEl || !effortEl) {
      return;
    }
    const meta = EFFORT_META[modelEl.value];
    const supportsEffort = meta ? meta.supportsEffort : true;
    if (row) row.hidden = !supportsEffort;
    if (rowHint) rowHint.hidden = supportsEffort;
    if (!supportsEffort) {
      return;
    }
    const allowed = meta ? meta.efforts : ${JSON.stringify([...EFFORTS])};
    for (const option of effortEl.options) {
      option.hidden = !allowed.includes(option.value);
      option.disabled = option.hidden;
    }
    if (!allowed.includes(effortEl.value)) {
      effortEl.value = allowed[allowed.length - 1];
      set(effortId, effortEl.value);
    }
  }

  const customRow = document.getElementById('customModelRow');
  const customModel = document.getElementById('customModel');

  for (const [modelId, effortId] of [['orchestratorModel', 'orchestratorEffort'], ['workerModel', 'workerEffort']]) {
    // A harness with no orchestrator-role model registered renders no select at
    // all (orchestratorModelSection) — nothing to wire up.
    const modelEl = document.getElementById(modelId);
    if (!modelEl) {
      continue;
    }
    modelEl.addEventListener('change', (event) => {
      // pi's "own Ollama id" entry is not a model itself — the text field is.
      if (event.target.value === '${CUSTOM_MODEL}') {
        if (customRow) { customRow.hidden = false; }
        customModel?.focus();
        return;
      }
      if (customRow) {
        customRow.hidden = true;
      }
      set(modelId, event.target.value);
      syncEfforts(modelId, effortId);
    });
    document.getElementById(effortId)?.addEventListener('change', (event) => set(effortId, event.target.value));
    syncEfforts(modelId, effortId);
  }
  customModel?.addEventListener('change', (event) => set('orchestratorModel', event.target.value.trim()));

  // Task item 2: the search-to-filter overlay for the (now hidden)
  // orchestratorModel/workerModel <select>s. The select stays authoritative —
  // this only sets select.value and dispatches 'change' on pick, so every
  // listener above (syncEfforts, the custom-Ollama-id toggle, set()) fires
  // exactly as if the user had picked a native <option>. Blur without a pick
  // restores the CURRENT selection's label — "die bisherige Auswahl darf
  // dabei nicht verloren gehen" — and that same current selection is always
  // sorted first in the result list — "steht immer oben".
  function wireModelPicker(selectId) {
    const select = document.getElementById(selectId);
    const input = document.querySelector('input[data-picker-for="' + selectId + '"]');
    const list = document.querySelector('div[data-picker-list-for="' + selectId + '"]');
    if (!select || !input || !list) return;
    function labelOf(value) {
      const opt = Array.from(select.options).find((o) => o.value === value);
      return opt ? opt.textContent : value;
    }
    function render(filterText) {
      const q = filterText.trim().toLowerCase();
      const currentValue = select.value;
      const all = Array.from(select.options).map((o) => ({ value: o.value, label: o.textContent }));
      const matches = all.filter((o) => !q || o.label.toLowerCase().includes(q) || o.value.toLowerCase().includes(q));
      matches.sort((a, b) => (a.value === currentValue ? -1 : b.value === currentValue ? 1 : 0));
      list.innerHTML = '';
      for (const o of matches.slice(0, 200)) {
        const item = document.createElement('div');
        item.className = 'modelPickerItem' + (o.value === currentValue ? ' current' : '');
        item.textContent = o.label;
        item.addEventListener('mousedown', (e) => {
          e.preventDefault();
          select.value = o.value;
          input.value = o.label;
          list.hidden = true;
          select.dispatchEvent(new Event('change'));
        });
        list.appendChild(item);
      }
      list.hidden = matches.length === 0;
    }
    input.value = labelOf(select.value);
    input.addEventListener('focus', () => render(''));
    input.addEventListener('input', () => render(input.value));
    input.addEventListener('blur', () => {
      setTimeout(() => { list.hidden = true; input.value = labelOf(select.value); }, 150);
    });
    select.addEventListener('change', () => { input.value = labelOf(select.value); });
  }
  wireModelPicker('orchestratorModel');
  wireModelPicker('workerModel');

  for (const button of document.querySelectorAll('button[data-harness]')) {
    button.addEventListener('click', () => set('orchestratorHarness', button.dataset.harness));
  }
  for (const button of document.querySelectorAll('button[data-layout]')) {
    button.addEventListener('click', () => {
      for (const other of document.querySelectorAll('button[data-layout]')) {
        other.classList.toggle('active', other === button);
      }
      set('workerLayout', button.dataset.layout);
    });
  }
  document.getElementById('terminalStartMaximized')
    .addEventListener('change', (event) => set('terminalStartMaximized', event.target.checked));
  document.getElementById('newSessionDefaultDir')
    .addEventListener('change', (event) => set('newSessionDefaultDir', event.target.value));
  document.getElementById('workerPollSeconds')
    .addEventListener('change', (event) => set('workerPollSeconds', Number(event.target.value)));
  document.getElementById('maxWorkers')
    .addEventListener('change', (event) => set('maxWorkers', Number(event.target.value)));
  document.getElementById('guardOrchWarnPct')
    .addEventListener('change', (event) => set('guardOrchWarnPct', Number(event.target.value)));
  document.getElementById('guardWorkerWarnPct')
    .addEventListener('change', (event) => set('guardWorkerWarnPct', Number(event.target.value)));
  document.getElementById('minWorkerPaneWidth')
    .addEventListener('change', (event) => set('minWorkerPaneWidth', Number(event.target.value)));
  document.getElementById('contextGuardAutostart').addEventListener('change', (event) => {
    // alice's explicit 2026-07-27 decision (off) — flipping it on is a real
    // behaviour change (a second guard could double-type /compact), so it asks
    // once instead of silently taking effect.
    if (event.target.checked && !confirm('context-guard automatisch starten? Der Orchestrator startet seinen eigenen Guard bereits, sobald Worker existieren — ein zusätzlicher automatischer Start kann /compact doppelt auslösen.')) {
      event.target.checked = false;
      return;
    }
    set('contextGuardAutostart', event.target.checked);
  });
  document.getElementById('modelDiscoveryAuto')
    .addEventListener('change', (event) => set('modelDiscoveryAuto', event.target.checked));

  const mcpSharedAction = document.getElementById('mcpSharedActionStatus');
  function mcpSharedReport(text) { if (mcpSharedAction) mcpSharedAction.textContent = text; }
  document.getElementById('mcpSharedRefresh')?.addEventListener('click', () => {
    mcpSharedReport('Lade Status …');
    vscode.postMessage({ command: 'mcp-shared', action: 'status' });
  });
  document.getElementById('mcpSharedRestart')?.addEventListener('click', () => {
    mcpSharedReport('Starte neu …');
    vscode.postMessage({ command: 'mcp-shared', action: 'restart' });
  });
  document.getElementById('mcpSharedApply')?.addEventListener('click', () => {
    mcpSharedReport('Wende Konfiguration an …');
    vscode.postMessage({ command: 'mcp-shared', action: 'apply' });
  });
  document.getElementById('mcpSharedReap')?.addEventListener('click', () => {
    mcpSharedReport('Räume verwaiste Helferprozesse auf …');
    vscode.postMessage({ command: 'mcp-shared', action: 'reap' });
  });
  for (const button of document.querySelectorAll('button[data-open-path]')) {
    button.addEventListener('click', () => {
      vscode.postMessage({ command: 'open-path', path: button.dataset.openPath });
    });
  }

  window.addEventListener('message', (event) => {
    const message = event.data;
    if (message?.command === 'mcp-shared-status') {
      const pre = document.getElementById('mcpSharedStatus');
      if (pre) pre.textContent = message.text;
      mcpSharedReport(message.actionText || '');
    }
  });
  for (const button of document.querySelectorAll('button[data-worker-machine]')) {
    button.addEventListener('click', () => {
      for (const other of document.querySelectorAll('button[data-worker-machine]')) {
        other.classList.toggle('active', other === button);
      }
      set('defaultWorkerMachine', button.dataset.workerMachine);
    });
  }

  window.addEventListener('message', (event) => {
    const message = event.data;
    if (message?.command === 'saved') {
      status.textContent = message.text;
    }
  });
${modelsSectionScript()}
</script>
</body>
</html>`;
}

/**
 * Task item 2 (Vertrag Teil 2 §4): with the registry now running into the
 * hundreds of entries, a plain `<select>` stopped being a usable control. The
 * `<select>` STAYS the source of truth — every existing change listener
 * (syncEfforts, the pi custom-field toggle, `set()`) keeps working verbatim —
 * this only adds a type-to-filter search input + result list on top of it,
 * wired client-side in wireModelPicker() below. `hidden` here is the SSR
 * default so the page never flashes a raw multi-hundred-row select before the
 * script runs; the CSS class also collapses it permanently once JS is live.
 */
function modelPickerMarkup(selectId: string): string {
  return `<div class="modelPicker">
      <input type="text" class="modelPickerSearch" data-picker-for="${escapeHtml(selectId)}"
             placeholder="Modell suchen (tippen zum Filtern) …" autocomplete="off" spellcheck="false">
      <div class="modelPickerList" data-picker-list-for="${escapeHtml(selectId)}" hidden></div>
    </div>`;
}

/**
 * pi models: the three local aliases, plus a free-text field for any other
 * Ollama id (SPEC-V2 F). Both write the same key, orchestratorModel.
 */
function piModelField(value: string, custom: boolean): string {
  const options = [
    ...PI_MODELS.map((model) =>
      `      <option value="${model}"${!custom && model === value ? ' selected' : ''}>${escapeHtml(PI_MODEL_LABEL[model])}</option>`
    ),
    `      <option value="${CUSTOM_MODEL}"${custom ? ' selected' : ''}>Eigene Ollama-ID …</option>`,
  ].join('\n');
  return `<div class="field">
  <div>
    <label class="title" for="orchestratorModel">Modell</label>
    <div class="hint">Lokaler Alias oder eine volle Ollama-ID.</div>
  </div>
  <div>
    <select id="orchestratorModel" class="modelPickerSelect" hidden>
${options}
    </select>
    ${modelPickerMarkup('orchestratorModel')}
    <div id="customModelRow"${custom ? '' : ' hidden'}>
      <input type="text" id="customModel" value="${escapeHtml(custom ? value : '')}"
             placeholder="z. B. ornith:35b" spellcheck="false">
    </div>
  </div>
</div>`;
}

/**
 * `choices` comes from the models registry (SPEC-V3 D) — when models.json is
 * missing or empty that is exactly the built-in MODELS/MODEL_LABEL list, so the
 * fallback below only ever fires for a registry read that somehow came back
 * empty (defensive, not a normal path).
 */
function modelField(
  id: string,
  title: string,
  hint: string,
  value: string,
  choices: { id: string; label: string }[] = MODELS.map((m) => ({ id: m, label: MODEL_LABEL[m] })),
): string {
  const options = choices.map(({ id: modelId, label }) =>
    `      <option value="${escapeHtml(modelId)}"${modelId === value ? ' selected' : ''}>${escapeHtml(label)} — ${escapeHtml(modelId)}</option>`
  ).join('\n');
  return `<div class="field">
  <div>
    <label class="title" for="${id}">${escapeHtml(title)}</label>
    <div class="hint">${escapeHtml(hint)}</div>
  </div>
  <div>
    <select id="${id}" class="modelPickerSelect" hidden>
${options}
    </select>
    ${modelPickerMarkup(id)}
  </div>
</div>`;
}

/**
 * `hidden`: this model/harness takes no effort flag at all (SPEC-V3 D item 3) —
 * the row starts hidden and a sibling hint takes its place; the client-side
 * script toggles both live when the model selection changes (same pattern as
 * pi's customModelRow).
 */
function effortField(id: string, title: string, hint: string, value: Effort, hidden = false): string {
  const options = EFFORTS.map((effort) =>
    `      <option value="${effort}"${effort === value ? ' selected' : ''}>${effort}</option>`
  ).join('\n');
  return `<div class="field" id="${id}Row"${hidden ? ' hidden' : ''}>
  <div>
    <label class="title" for="${id}">${escapeHtml(title)}</label>
    <div class="hint">${escapeHtml(hint)}</div>
  </div>
  <div>
    <select id="${id}">
${options}
    </select>
  </div>
</div>
<div class="field" id="${id}RowHint"${hidden ? '' : ' hidden'}>
  <div><label class="title">${escapeHtml(title)}</label></div>
  <div class="hint">Dieses Modell/dieser Harness kennt kein Effort-/Reasoning-Flag — nichts zu wählen.</div>
</div>`;
}

/**
 * Orchestrator model + effort, per harness (SPEC-V3 D item 1/3):
 * - pi keeps its existing alias-or-custom-Ollama-id field, effort always shown
 *   ("Thinking-Level" — pi always forwards --thinking).
 * - claude keeps the five canonical ids (policy caps hang off them by id).
 * - any OTHER harness draws its options straight from the registry; if it has
 *   no model registered with the 'orchestrator' role yet, no select is shown at
 *   all — a dropdown with nothing real in it would just fail on "set".
 */
function orchestratorModelSection(
  settings: Settings,
  claudeModelOptions: { id: string; label: string }[],
  registryModels: RegistryModel[],
  pi: boolean,
  claude: boolean,
  customModel: boolean,
  registry: ModelsRegistry,
): string {
  if (pi) {
    return piModelField(settings.orchestratorModel, customModel)
      + effortField('orchestratorEffort', effortLabel('pi'), 'Thinking-Level des lokalen Modells (pi --thinking).', settings.orchestratorEffort);
  }
  if (claude) {
    return modelField('orchestratorModel', 'Modell', 'Modell, mit dem wb-code den Orchestrator startet.', settings.orchestratorModel, claudeModelOptions)
      + effortField('orchestratorEffort', 'Effort', 'Denktiefe des Orchestrators.', settings.orchestratorEffort);
  }
  if (registryModels.length === 0) {
    return `<div class="field">
  <div><label class="title">Modell</label><div class="hint">Kein Modell mit Rolle „Orchestrator" für diesen Harness registriert.</div></div>
  <div class="hint">In der Tabelle „Modelle & Harnesses" unten ein Modell mit diesem Harness anlegen und die Rolle „Orchestrator" ankreuzen.</div>
</div>`;
  }
  const options = registryModels.map((m) => ({ id: m.id, label: `${m.label} — ${m.id}` }));
  const selected = registryModels.find((m) => m.id === settings.orchestratorModel) ?? registryModels[0];
  const supportsEffort = modelSupportsEffort(selected, findHarness(registry, selected.harness), findProvider(registry, selected.provider));
  return modelField('orchestratorModel', 'Modell', 'Modell, mit dem wb-code den Orchestrator startet.', settings.orchestratorModel, options)
    + effortField('orchestratorEffort', 'Effort', 'Denktiefe des Orchestrators.', settings.orchestratorEffort, !supportsEffort);
}

/**
 * Worker default model + effort (SPEC-V3 D, Reviewer-Befund M6): harness-
 * independent, one flat list across every registered harness. Always has at
 * least the five Claude ids (BUILTIN_MODELS), so — unlike the orchestrator
 * case — there is no "zero options" branch to handle.
 */
function workerModelSection(
  settings: Settings,
  options: { id: string; label: string }[],
  workerModels: RegistryModel[],
  registry: ModelsRegistry,
): string {
  const selected = workerModels.find((m) => m.id === settings.workerModel);
  const supportsEffort = !selected
    || modelSupportsEffort(selected, findHarness(registry, selected.harness), findProvider(registry, selected.provider));
  return modelField('workerModel', 'Standard-Modell', 'Vorgabe für neu gestartete Worker (claude-worker <name> default …), über alle Harnesses hinweg.', settings.workerModel, options)
    + effortField('workerEffort', 'Standard-Effort', 'Vorgabe für neu gestartete Worker.', settings.workerEffort, !supportsEffort);
}

/**
 * Nachtrag priority 3, read-only: ~/.claude/settings.json's `hooks` section.
 * No toggle — see hooksInfo.ts's file comment for why a write path was left
 * out (an honest gap beats a switch that risks corrupting a config the whole
 * setup depends on, per the user's own instruction to skip what cannot be
 * verified).
 */
/**
 * Nachtrag priority 4, read-only: named paths (existence + size, „öffnen
 * können" via revealFileInOS), the five launchd jobs, and the pi model alias
 * mapping. The alias mapping is DISPLAY ONLY — it lives hardcoded in
 * `shell/pi-worker`'s case block, not in this registry, so editing it here
 * would change nothing real; see "Was fehlt" in the result for why that stays
 * out of scope.
 */
function renderPathsAndAutomations(pathInfo: PathInfo[], launchdJobs: LaunchdJob[]): string {
  const pathRows = pathInfo.map((p) => `<tr>
  <td>${escapeHtml(p.label)}<div class="idHint">${escapeHtml(p.path)}</div></td>
  <td>${p.exists ? 'vorhanden' : '<span class="hint">fehlt</span>'}</td>
  <td>${p.sizeHuman ? escapeHtml(p.sizeHuman) : '<span class="hint">—</span>'}</td>
  <td class="actions">${p.exists ? `<button type="button" data-open-path="${escapeHtml(p.path)}">Öffnen</button>` : ''}</td>
</tr>`).join('\n');
  const launchdRows = launchdJobs.map((j) => `<tr>
  <td>${escapeHtml(j.label)}</td>
  <td>${j.loaded ? 'geladen' : '<span class="hint">nicht geladen</span>'}</td>
  <td>${j.pid !== undefined ? String(j.pid) : '<span class="hint">—</span>'}</td>
  <td>${j.lastExit !== undefined ? String(j.lastExit) : '<span class="hint">—</span>'}</td>
</tr>`).join('\n');
  const aliasRows = Object.entries(PI_MODEL_LABEL).map(([alias, label]) => `<tr>
  <td>${escapeHtml(alias)}</td>
  <td>${escapeHtml(label)}</td>
</tr>`).join('\n');

  return `<h2>Pfade</h2>
<table class="registry">
<thead><tr><th>Pfad</th><th>Status</th><th>Größe</th><th>Aktionen</th></tr></thead>
<tbody>
${pathRows}
</tbody>
</table>

<h2>Automationen (launchd)</h2>
<table class="registry">
<thead><tr><th>Job</th><th>Status</th><th>PID</th><th>Letzter Exit-Code</th></tr></thead>
<tbody>
${launchdRows}
</tbody>
</table>

<h2>Lokale Modell-Aliase (pi)</h2>
<div class="hint">Nur Anzeige — die Zuordnung steht fest im Code von shell/pi-worker, ein neues Modell mit
  eigener ID kommt stattdessen über „Modelle & Harnesses" unten in die Registry.</div>
<table class="registry">
<thead><tr><th>Alias</th><th>Ollama-ID</th></tr></thead>
<tbody>
${aliasRows}
</tbody>
</table>`;
}

function renderHooksSection(hooks: HookEntry[]): string {
  if (hooks.length === 0) {
    return `<h2>Hooks</h2>
<div class="hint">Keine Hooks gefunden (oder ~/.claude/settings.json nicht lesbar) — nur Anzeige, kein Absturz.</div>`;
  }
  const rows = hooks.map((h) => `<tr${h.isDenyHook ? ' class="denyHook"' : ''}>
  <td>${escapeHtml(h.event)}</td>
  <td>${h.matcher ? escapeHtml(h.matcher) : '<span class="hint">—</span>'}</td>
  <td>${escapeHtml(h.command)}</td>
  <td>${h.timeout !== undefined ? `${h.timeout}s` : '<span class="hint">—</span>'}</td>
  <td>${h.isDenyHook ? `<span class="denyHookWarning">Deny-Hook — ${escapeHtml(h.denyHookReason ?? '')}</span>` : ''}</td>
</tr>`).join('\n');
  return `<h2>Hooks</h2>
<div class="hint">Nur Anzeige (~/.claude/settings.json), kein Schalter — siehe „Was fehlt" im Ergebnis. Die drei
  markierten Deny-Hooks existieren wegen echter Vorfälle; sie hier abzuschalten braucht bewusstes Handeln in der Datei selbst.</div>
<table class="registry">
<thead><tr><th>Ereignis</th><th>Matcher</th><th>Kommando</th><th>Timeout</th><th>Hinweis</th></tr></thead>
<tbody>
${rows}
</tbody>
</table>`;
}
