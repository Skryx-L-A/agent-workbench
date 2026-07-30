// Pure rendering for the "Modelle & Harnesses" section of the Einstellungen
// webview (no vscode import — unit-testable, same split as settingsHtml.ts).
import { escapeHtml } from './format.ts';
import {
  allowedEfforts,
  effectiveHarnesses,
  effectiveModels,
  effectiveProviders,
  harnessesForProvider,
  type HarnessDiscover,
  isBuiltinHarnessId,
  isBuiltinProviderId,
  isFreeModel,
  type ModelsRegistry,
  modelsFile,
  type Provider,
  providersForHarness,
  type RegistryHarness,
  type RegistryModel,
} from './models.ts';
import type { ProviderKeyStatus } from './providerSecrets.ts';
import { EFFORTS } from './settings.ts';

/**
 * Four real states plus 'disabled' (SPEC-V3 D item 2: "startbar / Binary fehlt /
 * Ready ungemessen / kein Key"), checked in the order a spawn attempt would hit
 * them — the FIRST one that applies wins, same order as `wb-state models
 * resolve`'s own gates (models.ts's blockers list).
 */
export type ModelStatus = 'startable' | 'binary-missing' | 'ready-unmeasured' | 'no-key' | 'disabled';

/**
 * `binaryPresent` comes from an actual `which`/exec-bit check on the harness
 * command (settingsView.ts, one check per unique harness — cheap), not a guess.
 * `keyStatus` is `undefined` only when the provider needs no check (local, or a
 * subscription provider with no `loginCheckPath`) — that is deliberately NOT
 * the same as "no-key": we cannot verify subscription login without a
 * configured check path, and an honest "unknown" must not read as "broken".
 */
export function modelStatusDot(
  model: RegistryModel,
  harness: RegistryHarness | undefined,
  provider: Provider | undefined,
  keyStatus: ProviderKeyStatus | undefined,
  binaryPresent: boolean,
): ModelStatus {
  if (!model.enabled) {
    return 'disabled';
  }
  if (!binaryPresent) {
    return 'binary-missing';
  }
  if (!harness?.readyPattern) {
    return 'ready-unmeasured';
  }
  // Cloud always gates (a missing map entry reads as "no key found", same as
  // before this function knew about subscriptions). Subscription only gates
  // when a REAL check actually ran (settingsView.ts always runs one in
  // production) — with no entry at all, "unknown" must not read as "no-key".
  const checkable = provider?.kind === 'cloud'
    || (provider?.kind === 'subscription' && keyStatus !== undefined && keyStatus.checked !== false);
  if (checkable && !keyStatus?.present) {
    return 'no-key';
  }
  return 'startable';
}

function dotColorVar(status: ModelStatus): string {
  switch (status) {
    case 'startable':
      return 'var(--vscode-testing-iconPassed, #3fb950)';
    case 'binary-missing':
      return 'var(--vscode-errorForeground, #f85149)';
    case 'ready-unmeasured':
      return 'var(--vscode-charts-orange, #d18616)';
    case 'no-key':
      return 'var(--vscode-editorWarning-foreground, #d29922)';
    case 'disabled':
      return 'var(--vscode-descriptionForeground, #888)';
  }
}

function dotTitle(status: ModelStatus): string {
  switch (status) {
    case 'startable':
      return 'Startbar (ungeprüft) — auf "Prüfen" klicken für eine echte Probe.';
    case 'binary-missing':
      return 'Binary des Harness fehlt (nicht im PATH).';
    case 'ready-unmeasured':
      return 'Ready-Muster des Harness ist nicht gemessen — "Ready-Muster messen" beim Harness ausführen.';
    case 'no-key':
      return 'Kein Key/keine Anmeldung hinterlegt.';
    case 'disabled':
      return 'Deaktiviert.';
  }
}

const STATUS_LABEL: Record<ModelStatus, string> = {
  startable: 'startbar',
  'binary-missing': 'Binary fehlt',
  'ready-unmeasured': 'Ready ungemessen',
  'no-key': 'kein Key',
  disabled: 'deaktiviert',
};

function statusDotHtml(status: ModelStatus): string {
  return `<span class="dot" style="background:${dotColorVar(status)}" title="${dotTitle(status)}"></span>`
    + `<span class="statusLabel">${STATUS_LABEL[status]}</span>`;
}

function formatCost(model: RegistryModel): string {
  if (!model.cost || (model.cost.inPerMTok === undefined && model.cost.outPerMTok === undefined)) {
    return '—';
  }
  const inCost = model.cost.inPerMTok !== undefined ? `$${model.cost.inPerMTok}` : '?';
  const outCost = model.cost.outPerMTok !== undefined ? `$${model.cost.outPerMTok}` : '?';
  return `${inCost} / ${outCost} pro MTok`;
}

function formatContext(model: RegistryModel): string {
  if (!model.contextWindow) {
    return '—';
  }
  return model.contextWindow >= 1000 ? `${Math.round(model.contextWindow / 1000)}K` : String(model.contextWindow);
}

function formatEffortRange(model: RegistryModel): string {
  const allowed = allowedEfforts(model);
  if (allowed.length === 0) {
    return '—';
  }
  return allowed.length === 1 ? allowed[0] : `${allowed[0]}…${allowed[allowed.length - 1]}`;
}

function formatRoles(model: RegistryModel): string {
  return model.roles.map((r) => (r === 'worker' ? 'Worker' : 'Orchestrator')).join(', ');
}

function formatDateDe(iso: string | undefined): string {
  const date = iso ? new Date(iso) : undefined;
  return date && !Number.isNaN(date.getTime())
    ? date.toLocaleDateString('de-DE', { year: 'numeric', month: '2-digit', day: '2-digit' })
    : 'unbekanntem Datum';
}

function keyStatusText(status: ProviderKeyStatus | undefined): string {
  if (!status?.present) {
    return 'nicht hinterlegt';
  }
  return `hinterlegt am ${formatDateDe(status.setAt)}`;
}

function formatTimeDe(iso: string | undefined): string | undefined {
  const date = iso ? new Date(iso) : undefined;
  return date && !Number.isNaN(date.getTime())
    ? date.toLocaleTimeString('de-DE', { hour: '2-digit', minute: '2-digit' })
    : undefined;
}

/** German-comma amount, e.g. 4.12 -> "4,12" — matches the task's own example "Guthaben 4,12 $". */
function formatAmountDe(amount: number): string {
  return amount.toLocaleString('de-DE', { minimumFractionDigits: 2, maximumFractionDigits: 2 });
}

/**
 * Task item 6 / Vertrag Teil 2 §1+§2: catalog size and account balance, per
 * cloud provider. Both are written by `wb-state models discover` once its
 * network side lands — until then `provider.catalogStatus`/`balanceStatus`
 * are simply absent, and this renders an honest "unbekannt" rather than a
 * made-up number (never guess — SPEC-V3 D's own rule for every other status
 * field on this page). `keyStatus` reuses the SAME presence check the key
 * field already shows, so "Key fehlt — nichts registriert" cannot drift out
 * of sync with the actual key state.
 */
function catalogAndBalanceHtml(provider: Provider, keyStatus: ProviderKeyStatus | undefined): string {
  let catalogText: string;
  if (provider.catalogStatus) {
    const when = formatTimeDe(provider.catalogStatus.fetchedAt);
    const keyNote = !keyStatus?.present ? ', Key fehlt — nichts registriert' : '';
    catalogText = `${provider.catalogStatus.count} im Katalog${when ? ` (abgerufen ${when})` : ''}${keyNote}`;
  } else {
    catalogText = 'Katalogstatus: unbekannt';
  }
  let balanceText: string;
  if (provider.balanceStatus) {
    const when = formatTimeDe(provider.balanceStatus.fetchedAt);
    const amount = provider.balanceStatus.amount === null
      ? 'unbegrenzt'
      : `${formatAmountDe(provider.balanceStatus.amount)} ${provider.balanceStatus.currency ?? '$'}`;
    balanceText = `Guthaben ${amount}${when ? ` (abgerufen ${when})` : ''}`;
  } else {
    balanceText = 'Guthaben: unbekannt';
  }
  return `<div class="hint catalogStatus">${escapeHtml(catalogText)}</div><div class="hint catalogStatus">${escapeHtml(balanceText)}</div>`;
}

/**
 * Task item 1 (Vertrag Teil 2 §4): every field the webview's own search/filter
 * bar needs to decide "does this row match", baked as `data-*` on the `<tr>`
 * so filtering never round-trips to the extension host — plain attribute
 * comparisons over up to ~500 rows, no re-render.
 */
function modelSearchAttrs(model: RegistryModel): string {
  const haystack = `${model.id} ${model.label} ${model.modelRef}`.toLowerCase();
  return ` data-search="${escapeHtml(haystack)}" data-harness="${escapeHtml(model.harness)}"`
    + ` data-provider="${escapeHtml(model.provider)}" data-free="${isFreeModel(model) ? '1' : '0'}"`
    + ` data-enabled="${model.enabled ? '1' : '0'}"`;
}

function modelRow(
  model: RegistryModel,
  registry: ModelsRegistry,
  keyStatuses: Record<string, ProviderKeyStatus>,
  binaryPresence: Record<string, boolean>,
): string {
  const harness = effectiveHarnesses(registry).find((h) => h.id === model.harness);
  const provider = effectiveProviders(registry).find((p) => p.id === model.provider);
  // Missing from the map = not checked this render (e.g. a fast re-render after
  // a mutation) — assume present rather than flash a false "Binary fehlt".
  const binaryPresent = binaryPresence[model.harness] ?? true;
  const status = modelStatusDot(model, harness, provider, keyStatuses[model.provider], binaryPresent);
  // Auto-discovered entries are selectable but never recommended — no
  // workerClass, so no row in the (hand-curated) routing table either
  // (Vertrag discover.md §3, task item 3).
  const autoBadge = model.source === 'auto'
    ? `<div class="autoBadge" title="Automatisch erkannt — auswählbar, aber nicht empfohlen (kein workerClass, keine Zeile in der Routing-Tabelle).">automatisch erkannt, ${escapeHtml(formatDateDe(model.discoveredAt))}</div>`
    : '';
  return `<tr${modelSearchAttrs(model)}>
  <td>${statusDotHtml(status)}</td>
  <td>${escapeHtml(model.label)}<div class="idHint">${escapeHtml(model.id)}${model.alias ? ` · Alias: ${escapeHtml(model.alias)}` : ''}</div>${autoBadge}</td>
  <td>${escapeHtml(harness?.label ?? model.harness)}</td>
  <td>${escapeHtml(provider?.label ?? model.provider)}</td>
  <td>${escapeHtml(formatRoles(model))}</td>
  <td>${escapeHtml(formatEffortRange(model))}</td>
  <td>${escapeHtml(formatContext(model))}</td>
  <td>${escapeHtml(formatCost(model))}</td>
  <td class="fit">${escapeHtml(model.goodFor ?? '—')}</td>
  <td class="actions">
    <label class="enabledToggle" title="Modell aktiv/deaktiviert (wb-state models set enabled)">
      <input type="checkbox" data-toggle-enabled="${escapeHtml(model.id)}"${model.enabled ? ' checked' : ''}> aktiv
    </label>
    <button type="button" data-check-model="${escapeHtml(model.id)}">Prüfen</button>
    ${model.builtin ? '' : `<button type="button" class="danger" data-remove="model:${escapeHtml(model.id)}">Entfernen</button>`}
  </td>
</tr>`;
}

function harnessRow(harness: RegistryHarness): string {
  return `<tr>
  <td>${escapeHtml(harness.label)}<div class="idHint">${escapeHtml(harness.id)}</div></td>
  <td>${escapeHtml(harness.command)}</td>
  <td>${harness.readyPattern ? escapeHtml(harness.readyPattern) : '<span class="hint">nicht gemessen</span>'}</td>
  <td>${harness.promptPattern ? escapeHtml(harness.promptPattern) : '<span class="hint">—</span>'}</td>
  <td>${harness.compactCommand === null ? 'nur Auto-Kompaktierung' : escapeHtml(harness.compactCommand ?? '/compact')}</td>
  <td class="actions">
    <button type="button" data-probe-harness="${escapeHtml(harness.id)}">Ready-Muster messen</button>
    ${harness.discover ? `<button type="button" data-discover-run="${escapeHtml(harness.id)}">Modelle neu einlesen</button>` : ''}
    ${isBuiltinHarnessId(harness.id) ? '' : `<button type="button" class="danger" data-remove="harness:${escapeHtml(harness.id)}">Entfernen</button>`}
  </td>
</tr>`;
}

const DISCOVER_SOURCE_OPTIONS: { id: string; label: string }[] = [
  { id: '', label: 'keine' },
  { id: 'ollama', label: 'ollama (installierte lokale Modelle)' },
  { id: 'command-lines', label: 'command-lines (ein Kommando, eine Referenz pro Zeile)' },
  { id: 'file-json', label: 'file-json (JSON-Datei)' },
];

/**
 * The seven discover sub-fields the task asks for (source/command/file/
 * jsonPath/provider/idPrefix/ttlHours) — `filter`/`refTemplate`/
 * `providerByPrefix`/`roles` stay hand-JSON-only, not exposed here. Shared by
 * the "Harness hinzufügen" form and each harness's inline discover editor
 * (`marker` scopes the `data-field` lookup in modelsSectionScript, `current`
 * prefills for an edit, stays undefined for the add form).
 */
function discoverFieldsHtml(marker: string, current?: HarnessDiscover): string {
  const sourceOptions = DISCOVER_SOURCE_OPTIONS
    .map((o) => `      <option value="${escapeHtml(o.id)}"${o.id === (current?.source ?? '') ? ' selected' : ''}>${escapeHtml(o.label)}</option>`)
    .join('\n');
  return `<div class="discoverFields" data-discover-fields="${escapeHtml(marker)}">
  <div class="field"><label>Automatische Erkennung: Quelle</label><select data-field="source">
${sourceOptions}
  </select></div>
  <div class="field"><label>Kommando (command-lines)</label><input type="text" data-field="command" placeholder="z. B. agy models" value="${escapeHtml(current?.command?.join(' ') ?? '')}" spellcheck="false"></div>
  <div class="field"><label>Datei (file-json)</label><input type="text" data-field="file" placeholder="~/.codex/models_cache.json" value="${escapeHtml(current?.file ?? '')}" spellcheck="false"></div>
  <div class="field"><label>JSON-Pfad (file-json)</label><input type="text" data-field="jsonPath" placeholder="models[].slug" value="${escapeHtml(current?.jsonPath ?? '')}" spellcheck="false"></div>
  <div class="field"><label>Provider-Fallback</label><input type="text" data-field="provider" placeholder="z. B. antigravity" value="${escapeHtml(current?.provider ?? '')}" spellcheck="false"></div>
  <div class="field"><label>ID-Präfix</label><input type="text" data-field="idPrefix" placeholder="z. B. agy-" value="${escapeHtml(current?.idPrefix ?? '')}" spellcheck="false"></div>
  <div class="field"><label>TTL Stunden</label><input type="number" data-field="ttlHours" min="1" step="1" placeholder="12" value="${current?.ttlHours ?? ''}"></div>
  <div class="hint">„keine" entfernt die Erkennung für diesen Harness. Bei „ollama" installierte lokale Modelle
    (pi/aider), bei „command-lines" eine nichtleere Zeile Stdout = eine Modell-Referenz (agy), bei „file-json"
    eine JSON-Datei mit Pfad der Form <code>a[].b</code> (codex).</div>
</div>`;
}

/**
 * Vertrag discover.md §5: harnesses that already carry a `discover` block get
 * an editor here (task item 3's "Harness-Formular … bearbeiten"), one
 * `<details>` per harness so nothing is hidden without at least a label to
 * find it by.
 */
function harnessDiscoverEditor(harness: RegistryHarness): string {
  return `<details class="addForm">
  <summary>${escapeHtml(harness.label)} — ${harness.discover ? `Erkennung: ${escapeHtml(harness.discover.source)}` : 'keine Erkennung'}</summary>
  ${discoverFieldsHtml(harness.id, harness.discover)}
  <button type="button" data-save-discover="${escapeHtml(harness.id)}">Speichern</button>
</details>`;
}

/**
 * Task item 3 / Vertrag Teil 2 §0.2: alice's decision — the key is TYPED at
 * the harness, but STORED exactly once per provider (same `provider-key-save`
 * path providerRow already uses, same keychain entry — no second secret copy,
 * no new write path). The hint under the field names every OTHER harness that
 * shares the same provider, so it is never a surprise which harnesses a saved
 * key already unlocks.
 */
function harnessProviderKeyField(
  harnessId: string,
  provider: Provider,
  status: ProviderKeyStatus | undefined,
  registry: ModelsRegistry,
): string {
  const others = harnessesForProvider(registry, provider.id)
    .filter((h) => h.id !== harnessId)
    .map((h) => h.label);
  const sharedHint = others.length > 0
    ? `gespeichert als Provider ${provider.label}, gilt auch für: ${others.join(', ')}`
    : `gespeichert als Provider ${provider.label}`;
  return `<div class="field">
  <div>
    <label>${escapeHtml(provider.label)}</label>
    <div class="hint">${escapeHtml(keyStatusText(status))} — ${escapeHtml(sharedHint)}</div>
  </div>
  <div>
    <span class="keyPair">
      <input type="password" class="keyInput" data-provider-key="${escapeHtml(provider.id)}"
             placeholder="API-Key" autocomplete="off" spellcheck="false">
      <button type="button" data-save-key="${escapeHtml(provider.id)}">Speichern</button>
    </span>
  </div>
</div>`;
}

/**
 * One `<details>` per harness that actually runs models from at least one
 * cloud provider — a harness with none (pi/ollama-only, claude's own
 * subscription) gets no block at all, nothing to type here.
 */
function harnessProviderKeysEditor(
  harness: RegistryHarness,
  registry: ModelsRegistry,
  keyStatuses: Record<string, ProviderKeyStatus>,
): string | undefined {
  const providers = providersForHarness(registry, harness.id);
  if (providers.length === 0) {
    return undefined;
  }
  return `<details class="addForm">
  <summary>${escapeHtml(harness.label)} — API-Keys (${providers.length})</summary>
  ${providers.map((p) => harnessProviderKeyField(harness.id, p, keyStatuses[p.id], registry)).join('\n')}
</details>`;
}

/**
 * `cloud` shows the existing key field. `local` needs nothing. `subscription`
 * (codex/agy: login via account, SPEC-V3 H0) shows NO key field — instead the
 * exact login command to copy (the harness's own CLI, run bare, drops into an
 * interactive login) plus an honest, opt-in login-check: without a configured
 * `loginCheckPath` the UI says so instead of guessing (item 4).
 */
function providerRow(provider: Provider, keyStatuses: Record<string, ProviderKeyStatus>, registry: ModelsRegistry): string {
  const status = keyStatuses[provider.id];
  let authCell: string;
  let actionsCell: string;
  if (provider.kind === 'local') {
    authCell = '<span class="hint">kein Key nötig</span>';
    actionsCell = '';
  } else if (provider.kind === 'subscription') {
    const commands = [...new Set(harnessesForProvider(registry, provider.id).map((h) => h.command))];
    const loginHint = commands.length > 0
      ? commands.map((c) => `<code>${escapeHtml(c)}</code>`).join(', ')
      : '<span class="hint">noch kein Harness mit diesem Provider verknüpft</span>';
    let statusText: string;
    if (!provider.loginCheckPath) {
      statusText = 'Anmeldestatus nicht automatisch prüfbar (kein Prüfpfad hinterlegt).';
    } else if (status?.present) {
      statusText = `Anmeldung erkannt (${escapeHtml(provider.loginCheckPath)}, Stand ${formatDateDe(status.setAt)}).`;
    } else {
      statusText = `Keine Anmeldung erkannt (${escapeHtml(provider.loginCheckPath)} fehlt).`;
    }
    authCell = `${loginHint}<div class="hint">${statusText}</div>`;
    actionsCell = `
    ${commands.length > 0 ? `<button type="button" data-copy-text="${escapeHtml(commands[0])}">Kommando kopieren</button>` : ''}
    <input type="text" class="keyInput" data-login-check-path="${escapeHtml(provider.id)}"
           placeholder="~/.codex/auth.json" value="${escapeHtml(provider.loginCheckPath ?? '')}" spellcheck="false">
    <button type="button" data-save-login-check="${escapeHtml(provider.id)}">Prüfpfad speichern</button>`;
  } else {
    authCell = escapeHtml(keyStatusText(status)) + catalogAndBalanceHtml(provider, status);
    actionsCell = `
    <span class="keyPair">
      <input type="password" class="keyInput" data-provider-key="${escapeHtml(provider.id)}"
             placeholder="API-Key" autocomplete="off" spellcheck="false">
      <button type="button" data-save-key="${escapeHtml(provider.id)}">Speichern</button>
    </span>`;
  }
  return `<tr>
  <td>${escapeHtml(provider.label)}<div class="idHint">${escapeHtml(provider.id)}</div></td>
  <td>${escapeHtml(provider.kind)}</td>
  <td>${provider.baseUrl ? escapeHtml(provider.baseUrl) : '—'}</td>
  <td>${authCell}</td>
  <td class="actions">
    ${actionsCell}
    ${isBuiltinProviderId(provider.id) ? '' : `<button type="button" class="danger" data-remove="provider:${escapeHtml(provider.id)}">Entfernen</button>`}
  </td>
</tr>`;
}

function optionList(options: { id: string; label: string }[], selected?: string): string {
  return options
    .map((o) => `      <option value="${escapeHtml(o.id)}"${o.id === selected ? ' selected' : ''}>${escapeHtml(o.label)}</option>`)
    .join('\n');
}

const PLACEHOLDER_HELP = '{model} {effort} {workdir} {sessionDir} {systemPromptFile} {name} '
  + '{secret:<provider>} {baseUrl:<provider>}';

export function renderModelsSection(
  registry: ModelsRegistry,
  keyStatuses: Record<string, ProviderKeyStatus>,
  /** Baked into #modelsStatus at render time — a post-render postMessage would
   *  race the fresh webview script that only just re-registered its listener. */
  statusText?: string,
  /** One `which`/exec-bit check per unique harness command (settingsView.ts); missing entries render as present (item 2). */
  binaryPresence: Record<string, boolean> = {},
): string {
  const models = effectiveModels(registry);
  const harnesses = effectiveHarnesses(registry);
  const providers = effectiveProviders(registry);
  const harnessOptions = harnesses.map((h) => ({ id: h.id, label: h.label }));
  const providerOptions = providers.map((p) => ({ id: p.id, label: p.label }));

  return `<h2>Modelle & Harnesses</h2>
<div class="file">${escapeHtml(modelsFile())}</div>

<div class="field">
  <div>
    <label class="title">Kataloge</label>
    <div class="hint">Vertrag Teil 2: jedes verfügbare Modell — lokale CLIs UND, sobald ein API-Key
      hinterlegt ist, die Kataloge von OpenRouter/OpenAI/Anthropic/Google/… — landet automatisch in
      dieser Tabelle, durchsuchbar statt in einem Dropdown. Dieser Knopf fragt ALLES erneut ab
      (<code>wb-state models discover --all</code>, inklusive Netzabruf), unabhängig davon, ob die
      automatische Erkennung unten in den Einstellungen ein- oder ausgeschaltet ist.</div>
  </div>
  <div><button type="button" id="discoverAllButton">Kataloge jetzt aktualisieren</button></div>
</div>

<h3>Modelle</h3>
<div class="modelSearchBar">
  <input type="text" id="modelSearchText" placeholder="Suche (id, Label, Modell-Referenz) …" spellcheck="false">
  <select id="modelSearchHarness"><option value="">Alle Harnesses</option>${optionList(harnessOptions)}</select>
  <select id="modelSearchProvider"><option value="">Alle Provider</option>${optionList(providerOptions)}</select>
  <select id="modelSearchFree">
    <option value="">kostenlos/kostenpflichtig: alle</option>
    <option value="free">kostenlos</option>
    <option value="paid">kostenpflichtig</option>
  </select>
  <select id="modelSearchEnabled">
    <option value="">aktiv/inaktiv: alle</option>
    <option value="enabled">aktiv</option>
    <option value="disabled">inaktiv</option>
  </select>
</div>
<div class="modelPager">
  <span id="modelPagerCount"></span>
  <button type="button" id="modelPagerPrev">&lt; zurück</button>
  <button type="button" id="modelPagerNext">weiter &gt;</button>
</div>
<table class="registry" id="modelsTable">
<thead><tr>
  <th></th><th>Label</th><th>Harness</th><th>Provider</th><th>Rollen</th>
  <th>Effort</th><th>Kontext</th><th>Kosten</th><th>Eignung</th><th>Aktionen</th>
</tr></thead>
<tbody>
${models.map((m) => modelRow(m, registry, keyStatuses, binaryPresence)).join('\n')}
</tbody>
</table>

<details class="addForm">
  <summary>Modell hinzufügen</summary>
  <div class="field"><label>ID</label><input type="text" id="newModelId" placeholder="z. B. gpt-5-codex" spellcheck="false"></div>
  <div class="field"><label>Label</label><input type="text" id="newModelLabel" placeholder="z. B. GPT-5 Codex" spellcheck="false"></div>
  <div class="field"><label>Harness</label><select id="newModelHarness">
${optionList(harnessOptions)}
  </select></div>
  <div class="field"><label>Provider</label><select id="newModelProvider">
${optionList(providerOptions)}
  </select></div>
  <div class="field"><label>Modell-Referenz</label><input type="text" id="newModelRef" placeholder="wie der Provider das Modell nennt" spellcheck="false"></div>
  <div class="field"><label>Rollen</label>
    <label><input type="checkbox" id="newModelRoleWorker" checked> Worker</label>
    <label><input type="checkbox" id="newModelRoleOrchestrator" checked> Orchestrator</label>
  </div>
  <div class="field"><label>Max. Effort</label><select id="newModelMaxEffort">
${optionList(EFFORTS.map((e) => ({ id: e, label: e })), 'high')}
  </select></div>
  <div class="field"><label>Kontextfenster</label><input type="number" id="newModelContext" min="1" placeholder="z. B. 200000"></div>
  <div class="field"><label>Kosten $/MTok (in / out)</label>
    <input type="number" id="newModelCostIn" min="0" step="0.01" style="width:90px">
    <input type="number" id="newModelCostOut" min="0" step="0.01" style="width:90px">
  </div>
  <div class="field"><label>Eignung (gut für)</label><input type="text" id="newModelGoodFor" spellcheck="false"></div>
  <div class="field"><label>Nicht geeignet für</label><input type="text" id="newModelNotFor" spellcheck="false"></div>
  <button type="button" id="addModelButton">Modell hinzufügen</button>
</details>

<h3>Harnesses</h3>
<table class="registry">
<thead><tr><th>Label</th><th>Kommando</th><th>Ready-Muster</th><th>Prompt-Muster</th><th>Kompaktieren</th><th>Aktionen</th></tr></thead>
<tbody>
${harnesses.map(harnessRow).join('\n')}
</tbody>
</table>

<details class="addForm">
  <summary>Harness hinzufügen</summary>
  <div class="field"><label>ID</label><input type="text" id="newHarnessId" placeholder="z. B. codex" spellcheck="false"></div>
  <div class="field"><label>Label</label><input type="text" id="newHarnessLabel" placeholder="z. B. Codex CLI (OpenAI)" spellcheck="false"></div>
  <div class="field"><label>Kommando</label><input type="text" id="newHarnessCommand" placeholder="z. B. codex" spellcheck="false"></div>
  <div class="field"><label>Argumente</label><input type="text" id="newHarnessArgs" placeholder="--model {model} --dir {workdir}" spellcheck="false"></div>
  <div class="hint">Platzhalter: ${escapeHtml(PLACEHOLDER_HELP)}</div>
  <div class="field"><label>Umgebungsvariablen</label><input type="text" id="newHarnessEnv" placeholder="OPENAI_API_KEY={secret:openai}" spellcheck="false"></div>
  <div class="field"><label>Autonomie-Flags</label><input type="text" id="newHarnessAutonomy" placeholder="--dangerously-skip-permissions" spellcheck="false">
    <div class="hint">Flags für „darf ohne Rückfrage arbeiten" — fehlen sie, hängt der Worker beim ersten Permission-Prompt.</div>
  </div>
  <div class="field"><label>Ready-Muster (Regex)</label><input type="text" id="newHarnessReadyPattern" placeholder="mit wb-harness-probe messen" spellcheck="false">
    <div class="hint">Muster, auf das gewartet wird, bevor ein Task eingefügt wird. Ohne gemessenes Muster: kein Start.</div>
  </div>
  <div class="field"><label>Prompt-Muster (Regex)</label><input type="text" id="newHarnessPromptPattern" placeholder="z. B. ^&gt; oder ^›" spellcheck="false">
    <div class="hint">Muster der EINGABEZEILE, für die Absende-Verifikation nach Paste+Enter — getrennt vom Ready-Muster.</div>
  </div>
  <div class="field"><label>Prompt-Ignore (Regex)</label><input type="text" id="newHarnessPromptIgnore" placeholder="z. B. Platzhaltertext einer leeren Zeile" spellcheck="false">
    <div class="hint">Text, den eine LEERE Eingabezeile trotzdem zeigt (z. B. ein Vorschlag) — sonst hält die Absende-Verifikation ihn für hängengebliebenen Text.</div>
  </div>
  <div class="field"><label>Kompaktier-Kommando</label><input type="text" id="newHarnessCompact" placeholder="/compact (leer = nur Auto-Kompaktierung)" spellcheck="false"></div>
  <div class="field"><label>Kontext-Muster (Regex, 1 Gruppe)</label><input type="text" id="newHarnessContextPattern" spellcheck="false"></div>
  <div class="field"><label>Notizen</label><input type="text" id="newHarnessNotes" spellcheck="false"></div>
  ${discoverFieldsHtml('__new__')}
  <button type="button" id="addHarnessButton">Harness hinzufügen</button>
  <div class="hint">Nach dem Speichern: unten in der Harness-Tabelle „Ready-Muster messen" — startet den Harness
    in einem Wegwerf-Pane auf eigenem tmux-Socket (die Live-Session bleibt unangetastet), zeigt die Ausgabe hier.
    Ohne mindestens ein registriertes Modell für diesen Harness kann wb-harness-probe nichts starten — erst ein
    Modell dafür anlegen.</div>
</details>

<h3>Automatische Erkennung je Harness</h3>
<div class="hint">Woher „Modelle neu einlesen" pro Harness liest — „keine" schaltet die automatische Erkennung
  für diesen Harness ab (Feld wird entfernt).</div>
${harnesses.map(harnessDiscoverEditor).join('\n')}

<h3>API-Keys je Harness</h3>
<div class="hint">Eingegeben wird hier, gespeichert bleibt jeder Key genau einmal pro Provider im
  Schlüsselbund (derselbe Weg wie unten bei „Provider") — ein Harness mit mehreren
  Cloud-Providern zeigt mehrere Felder, ein Harness ohne Cloud-Provider (lokal/Abo) zeigt hier
  gar nichts.</div>
${harnesses.map((h) => harnessProviderKeysEditor(h, registry, keyStatuses)).filter((b) => b !== undefined).join('\n')
  || '<div class="hint">Kein Harness mit einem Cloud-Provider registriert.</div>'}

<h3>Provider</h3>
<table class="registry">
<thead><tr><th>Label</th><th>Art</th><th>Base-URL</th><th>Key</th><th>Aktionen</th></tr></thead>
<tbody>
${providers.map((p) => providerRow(p, keyStatuses, registry)).join('\n')}
</tbody>
</table>

<details class="addForm">
  <summary>Provider hinzufügen</summary>
  <div class="field"><label>ID</label><input type="text" id="newProviderId" placeholder="z. B. together" spellcheck="false"></div>
  <div class="field"><label>Label</label><input type="text" id="newProviderLabel" spellcheck="false"></div>
  <div class="field"><label>Art</label><select id="newProviderKind">
    <option value="cloud" selected>cloud</option>
    <option value="local">local</option>
    <option value="subscription">subscription</option>
  </select></div>
  <div class="field"><label>Base-URL</label><input type="text" id="newProviderBaseUrl" spellcheck="false"></div>
  <div class="field"><label>API-Key</label><input type="password" id="newProviderKey" autocomplete="off" spellcheck="false"></div>
  <button type="button" id="addProviderButton">Provider hinzufügen</button>
</details>

<div class="status" id="modelsStatus">${statusText ? escapeHtml(statusText) : ''}</div>`;
}

/** Raw JS body (no <script> wrapper) — spliced into the settings webview's single nonce'd script tag. */
export function modelsSectionScript(): string {
  return `
  const modelsStatus = document.getElementById('modelsStatus');
  function modelsReport(text) { if (modelsStatus) modelsStatus.textContent = text; }

  // Task item 1 (Vertrag Teil 2 §4): search/filter/pagination over the model
  // table entirely in this script — no round-trip to the extension host, so
  // it stays fluid at hundreds of rows. Every row already carries its own
  // data-search/-harness/-provider/-free/-enabled attributes (modelRow); this
  // only ever toggles the "hidden" attribute, it never re-renders a row.
  (function setupModelSearch() {
    const table = document.getElementById('modelsTable');
    if (!table) return;
    const rows = Array.from(table.querySelectorAll('tbody tr[data-search]'));
    const PAGE_SIZE = 50;
    let page = 0;
    const textInput = document.getElementById('modelSearchText');
    const harnessSelect = document.getElementById('modelSearchHarness');
    const providerSelect = document.getElementById('modelSearchProvider');
    const freeSelect = document.getElementById('modelSearchFree');
    const enabledSelect = document.getElementById('modelSearchEnabled');
    const countEl = document.getElementById('modelPagerCount');
    const prevBtn = document.getElementById('modelPagerPrev');
    const nextBtn = document.getElementById('modelPagerNext');

    function apply() {
      const q = (textInput?.value || '').trim().toLowerCase();
      const harness = harnessSelect?.value || '';
      const provider = providerSelect?.value || '';
      const free = freeSelect?.value || '';
      const enabled = enabledSelect?.value || '';
      const matches = rows.filter((tr) => {
        if (q && !tr.dataset.search.includes(q)) return false;
        if (harness && tr.dataset.harness !== harness) return false;
        if (provider && tr.dataset.provider !== provider) return false;
        if (free === 'free' && tr.dataset.free !== '1') return false;
        if (free === 'paid' && tr.dataset.free !== '0') return false;
        if (enabled === 'enabled' && tr.dataset.enabled !== '1') return false;
        if (enabled === 'disabled' && tr.dataset.enabled !== '0') return false;
        return true;
      });
      const totalPages = Math.max(1, Math.ceil(matches.length / PAGE_SIZE));
      if (page >= totalPages) page = totalPages - 1;
      if (page < 0) page = 0;
      const start = page * PAGE_SIZE;
      const shown = new Set(matches.slice(start, start + PAGE_SIZE));
      for (const tr of rows) {
        tr.hidden = !shown.has(tr);
      }
      if (countEl) {
        countEl.textContent = matches.length === 0
          ? '0 von ' + rows.length
          : (start + 1) + '–' + Math.min(start + PAGE_SIZE, matches.length) + ' von ' + matches.length;
      }
      if (prevBtn) prevBtn.disabled = page <= 0;
      if (nextBtn) nextBtn.disabled = page >= totalPages - 1;
    }
    function applyFromStart() { page = 0; apply(); }

    textInput?.addEventListener('input', applyFromStart);
    harnessSelect?.addEventListener('change', applyFromStart);
    providerSelect?.addEventListener('change', applyFromStart);
    freeSelect?.addEventListener('change', applyFromStart);
    enabledSelect?.addEventListener('change', applyFromStart);
    prevBtn?.addEventListener('click', () => { page -= 1; apply(); });
    nextBtn?.addEventListener('click', () => { page += 1; apply(); });
    apply();
  })();

  // Reads the seven discover sub-fields out of the container tagged
  // data-discover-fields="<marker>" (one per harness's editor, plus '__new__'
  // for the add-harness form) — an empty "Quelle" means "keine", which sends
  // null so wb-state REMOVES the field rather than storing an empty shell.
  function readDiscoverFields(marker) {
    const container = document.querySelector('[data-discover-fields="' + marker + '"]');
    if (!container) return null;
    const get = (field) => container.querySelector('[data-field="' + field + '"]');
    const source = (get('source')?.value || '').trim();
    if (!source) return null;
    const discover = { source };
    const command = (get('command')?.value || '').trim();
    if (command) discover.command = command.split(/\\s+/);
    const file = (get('file')?.value || '').trim();
    if (file) discover.file = file;
    const jsonPath = (get('jsonPath')?.value || '').trim();
    if (jsonPath) discover.jsonPath = jsonPath;
    const provider = (get('provider')?.value || '').trim();
    if (provider) discover.provider = provider;
    const idPrefix = (get('idPrefix')?.value || '').trim();
    if (idPrefix) discover.idPrefix = idPrefix;
    const ttlRaw = (get('ttlHours')?.value || '').trim();
    if (ttlRaw) discover.ttlHours = Number(ttlRaw);
    return discover;
  }

  document.getElementById('discoverAllButton')?.addEventListener('click', () => {
    modelsReport('Lese Modelle neu ein …');
    vscode.postMessage({ command: 'models-discover' });
  });
  document.querySelectorAll('button[data-discover-run]').forEach((btn) => {
    btn.addEventListener('click', () => {
      const harnessId = btn.dataset.discoverRun;
      modelsReport('Lese Modelle neu ein (' + harnessId + ') …');
      vscode.postMessage({ command: 'models-discover', harnessId });
    });
  });
  document.querySelectorAll('button[data-save-discover]').forEach((btn) => {
    btn.addEventListener('click', () => {
      const id = btn.dataset.saveDiscover;
      const value = readDiscoverFields(id);
      modelsReport('Speichere Erkennung für ' + id + ' …');
      vscode.postMessage({ command: 'models-set-field', kind: 'harness', id, field: 'discover', value });
    });
  });

  document.querySelectorAll('button[data-check-model]').forEach((btn) => {
    btn.addEventListener('click', () => {
      modelsReport('Prüfe ' + btn.dataset.checkModel + ' …');
      vscode.postMessage({ command: 'models-check', id: btn.dataset.checkModel });
    });
  });
  document.querySelectorAll('button[data-probe-harness]').forEach((btn) => {
    btn.addEventListener('click', () => {
      modelsReport('Messe Ready-Muster für ' + btn.dataset.probeHarness + ' …');
      vscode.postMessage({ command: 'models-probe-harness', id: btn.dataset.probeHarness });
    });
  });
  document.querySelectorAll('button[data-remove]').forEach((btn) => {
    btn.addEventListener('click', () => {
      const [kind, id] = btn.dataset.remove.split(':');
      if (!confirm('"' + id + '" wirklich entfernen?')) return;
      vscode.postMessage({ command: 'models-remove', kind, id });
    });
  });
  // Task item 3: the SAME provider can now have a key field in two places —
  // the Provider table AND, for a cloud provider, one per harness that uses
  // it. A lookup keyed only by providerId would always hit the FIRST match in
  // the document, silently reading the wrong field if the user typed into a
  // harness-level one. Scoping to the button's own .keyPair fixes that; the
  // input carries its own providerId (data-provider-key), the button no
  // longer needs to.
  document.querySelectorAll('button[data-save-key]').forEach((btn) => {
    btn.addEventListener('click', () => {
      const wrap = btn.closest('.keyPair');
      const input = wrap ? wrap.querySelector('input[data-provider-key]') : null;
      const providerId = input ? input.dataset.providerKey : btn.dataset.saveKey;
      const key = input ? input.value : '';
      if (!key.trim()) { modelsReport('Kein Key eingegeben.'); return; }
      modelsReport('Speichere Key für ' + providerId + ' …');
      vscode.postMessage({ command: 'provider-key-save', providerId, key });
      if (input) input.value = '';
    });
  });
  document.querySelectorAll('input[data-toggle-enabled]').forEach((cb) => {
    cb.addEventListener('change', () => {
      const id = cb.dataset.toggleEnabled;
      modelsReport((cb.checked ? 'Aktiviere ' : 'Deaktiviere ') + id + ' …');
      vscode.postMessage({ command: 'models-set-field', kind: 'model', id, field: 'enabled', value: cb.checked });
    });
  });
  document.querySelectorAll('button[data-copy-text]').forEach((btn) => {
    btn.addEventListener('click', async () => {
      const text = btn.dataset.copyText;
      try {
        await navigator.clipboard.writeText(text);
      } catch {
        const ta = document.createElement('textarea');
        ta.value = text; ta.style.position = 'fixed'; ta.style.opacity = '0';
        document.body.appendChild(ta); ta.focus(); ta.select();
        try { document.execCommand('copy'); } catch {}
        document.body.removeChild(ta);
      }
      modelsReport('Kommando "' + text + '" kopiert.');
    });
  });
  document.querySelectorAll('button[data-save-login-check]').forEach((btn) => {
    btn.addEventListener('click', () => {
      const providerId = btn.dataset.saveLoginCheck;
      const input = document.querySelector('input[data-login-check-path="' + providerId + '"]');
      const value = input ? input.value.trim() : '';
      modelsReport('Speichere Prüfpfad für ' + providerId + ' …');
      vscode.postMessage({ command: 'models-set-field', kind: 'provider', id: providerId, field: 'loginCheckPath', value });
    });
  });

  document.getElementById('addModelButton')?.addEventListener('click', () => {
    const id = document.getElementById('newModelId').value.trim();
    const label = document.getElementById('newModelLabel').value.trim();
    const harness = document.getElementById('newModelHarness').value;
    const provider = document.getElementById('newModelProvider').value;
    const modelRef = document.getElementById('newModelRef').value.trim();
    if (!id || !label || !modelRef) { modelsReport('ID, Label und Modell-Referenz sind Pflicht.'); return; }
    const roles = [];
    if (document.getElementById('newModelRoleWorker').checked) roles.push('worker');
    if (document.getElementById('newModelRoleOrchestrator').checked) roles.push('orchestrator');
    if (roles.length === 0) { modelsReport('Mindestens eine Rolle wählen.'); return; }
    const maxEffort = document.getElementById('newModelMaxEffort').value;
    const contextRaw = document.getElementById('newModelContext').value;
    const costInRaw = document.getElementById('newModelCostIn').value;
    const costOutRaw = document.getElementById('newModelCostOut').value;
    const draft = {
      id, label, harness, provider, modelRef, roles,
      efforts: ['low', 'medium', 'high', 'xhigh'], maxEffort, defaultEffort: maxEffort,
      contextWindow: contextRaw ? Number(contextRaw) : undefined,
      cost: (costInRaw || costOutRaw)
        ? { inPerMTok: costInRaw ? Number(costInRaw) : undefined, outPerMTok: costOutRaw ? Number(costOutRaw) : undefined }
        : undefined,
      goodFor: document.getElementById('newModelGoodFor').value.trim() || undefined,
      notFor: document.getElementById('newModelNotFor').value.trim() || undefined,
      enabled: true,
    };
    modelsReport('Füge Modell "' + id + '" hinzu …');
    vscode.postMessage({ command: 'models-add-model', model: draft });
  });

  document.getElementById('addHarnessButton')?.addEventListener('click', () => {
    const id = document.getElementById('newHarnessId').value.trim();
    const label = document.getElementById('newHarnessLabel').value.trim();
    const command = document.getElementById('newHarnessCommand').value.trim();
    if (!id || !label || !command) { modelsReport('ID, Label und Kommando sind Pflicht.'); return; }
    const argsRaw = document.getElementById('newHarnessArgs').value.trim();
    const envRaw = document.getElementById('newHarnessEnv').value.trim();
    const env = {};
    for (const line of envRaw.split(/[\\n,]/).map((s) => s.trim()).filter(Boolean)) {
      const eq = line.indexOf('=');
      if (eq > 0) env[line.slice(0, eq).trim()] = line.slice(eq + 1).trim();
    }
    const autonomyRaw = document.getElementById('newHarnessAutonomy').value.trim();
    const draft = {
      id, label, command,
      args: argsRaw ? argsRaw.split(/\\s+/) : undefined,
      env: Object.keys(env).length ? env : undefined,
      autonomy: autonomyRaw ? { args: autonomyRaw.split(/\\s+/) } : undefined,
      readyPattern: document.getElementById('newHarnessReadyPattern').value.trim() || undefined,
      promptPattern: document.getElementById('newHarnessPromptPattern').value.trim() || undefined,
      promptIgnore: document.getElementById('newHarnessPromptIgnore').value.trim() || undefined,
      compactCommand: document.getElementById('newHarnessCompact').value.trim() || null,
      contextPattern: document.getElementById('newHarnessContextPattern').value.trim() || undefined,
      notes: document.getElementById('newHarnessNotes').value.trim() || undefined,
      discover: readDiscoverFields('__new__') || undefined,
    };
    modelsReport('Füge Harness "' + id + '" hinzu …');
    vscode.postMessage({ command: 'models-add-harness', harness: draft });
  });

  document.getElementById('addProviderButton')?.addEventListener('click', () => {
    const id = document.getElementById('newProviderId').value.trim();
    const label = document.getElementById('newProviderLabel').value.trim();
    if (!id || !label) { modelsReport('ID und Label sind Pflicht.'); return; }
    const kind = document.getElementById('newProviderKind').value;
    const baseUrl = document.getElementById('newProviderBaseUrl').value.trim() || undefined;
    const key = document.getElementById('newProviderKey').value;
    modelsReport('Füge Provider "' + id + '" hinzu …');
    vscode.postMessage({ command: 'models-add-provider', provider: { id, label, kind, baseUrl } });
    if (key.trim()) {
      vscode.postMessage({ command: 'provider-key-save', providerId: id, key });
    }
    document.getElementById('newProviderKey').value = '';
  });

  window.addEventListener('message', (event) => {
    const message = event.data;
    if (message?.command === 'models-status') {
      modelsReport(message.text);
    }
  });
`;
}
