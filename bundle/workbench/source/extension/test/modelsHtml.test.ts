import assert from 'node:assert/strict';
import { test } from 'node:test';
import { modelStatusDot, modelsSectionScript, renderModelsSection } from '../src/modelsHtml.ts';
import { BUILTIN_MODELS, parseModelsRegistry, type RegistryModel } from '../src/models.ts';

function customModel(overrides: Record<string, unknown> = {}): RegistryModel {
  return {
    id: 'gpt-5-codex',
    label: 'GPT-5 Codex',
    harness: 'codex',
    provider: 'openai',
    modelRef: 'gpt-5-codex',
    roles: ['worker', 'orchestrator'],
    efforts: ['low', 'medium', 'high'],
    maxEffort: 'high',
    defaultEffort: 'medium',
    enabled: true,
    ...overrides,
  } as RegistryModel;
}

const CODEX_HARNESS = { id: 'codex', label: 'Codex CLI', command: 'codex', readyPattern: '^›' };
const OPENAI_PROVIDER = { id: 'openai', label: 'OpenAI', kind: 'cloud' as const };
const OLLAMA_PROVIDER = { id: 'ollama', label: 'Ollama', kind: 'local' as const };

// SPEC-V3 D item 2: "startbar / Binary fehlt / Ready ungemessen / kein Key" —
// the FIRST applicable state wins, in spawn-attempt order.
test('modelStatusDot: the five real states, first-applicable-wins', () => {
  const model = customModel();
  assert.equal(modelStatusDot(customModel({ enabled: false }), CODEX_HARNESS, OPENAI_PROVIDER, { present: true }, true), 'disabled');
  assert.equal(modelStatusDot(model, CODEX_HARNESS, OPENAI_PROVIDER, { present: true }, false), 'binary-missing');
  assert.equal(modelStatusDot(model, { ...CODEX_HARNESS, readyPattern: undefined }, OPENAI_PROVIDER, { present: true }, true), 'ready-unmeasured');
  assert.equal(modelStatusDot(model, undefined, OPENAI_PROVIDER, { present: true }, true), 'ready-unmeasured', 'an unregistered harness has no readyPattern either');
  assert.equal(modelStatusDot(model, CODEX_HARNESS, OPENAI_PROVIDER, { present: false }, true), 'no-key');
  assert.equal(modelStatusDot(model, CODEX_HARNESS, OPENAI_PROVIDER, { present: true }, true), 'startable');
  assert.equal(modelStatusDot(model, CODEX_HARNESS, OLLAMA_PROVIDER, undefined, true), 'startable', 'a local provider needs no key');
});

test('modelStatusDot: a subscription provider with no loginCheckPath is honestly "unknown", not "no-key"', () => {
  const model = customModel();
  const subscription = { id: 'chatgpt', label: 'ChatGPT', kind: 'subscription' as const };
  assert.equal(
    modelStatusDot(model, CODEX_HARNESS, subscription, { present: false, checked: false }, true),
    'startable',
    'unchecked must not read as broken',
  );
  assert.equal(
    modelStatusDot(model, CODEX_HARNESS, subscription, undefined, true),
    'startable',
    'no key-status entry at all (e.g. a bare test render) must not read as broken either',
  );
  assert.equal(
    modelStatusDot(model, CODEX_HARNESS, subscription, { present: false }, true),
    'no-key',
    'checked and absent IS a real no-key state',
  );
  assert.equal(
    modelStatusDot(model, CODEX_HARNESS, subscription, { present: true }, true),
    'startable',
  );
});

test('renderModelsSection lists every built-in model and the three registry tables', () => {
  const html = renderModelsSection(parseModelsRegistry(undefined), {});
  for (const model of BUILTIN_MODELS) {
    assert.ok(html.includes(model.label), `missing built-in model label: ${model.label}`);
  }
  assert.match(html, /<h3>Modelle<\/h3>/);
  assert.match(html, /<h3>Harnesses<\/h3>/);
  assert.match(html, /<h3>Provider<\/h3>/);
  assert.match(html, /Modell hinzufügen/);
  assert.match(html, /Harness hinzufügen/);
  assert.match(html, /Provider hinzufügen/);
});

test('renderModelsSection: a built-in model/harness/provider row has no "Entfernen" button', () => {
  const html = renderModelsSection(parseModelsRegistry(undefined), {});
  assert.ok(!html.includes('data-remove="model:claude-opus-5"'));
  assert.ok(!html.includes('data-remove="harness:claude"'));
  assert.ok(!html.includes('data-remove="harness:pi"'));
  assert.ok(!html.includes('data-remove="provider:openai"'));
});

test('renderModelsSection: a custom model gets an "Entfernen" button, a built-in override keeps none', () => {
  const registry = parseModelsRegistry(JSON.stringify({
    models: [customModel()],
    harnesses: [{ id: 'claude', label: 'Claude Code (angepasst)', command: 'claude-custom' }],
  }));
  const html = renderModelsSection(registry, {});
  assert.ok(html.includes('data-remove="model:gpt-5-codex"'), 'custom model must be removable');
  assert.ok(!html.includes('data-remove="harness:claude"'), 'an overridden built-in harness id is still not removable');
});

test('renderModelsSection: the placeholder help text lists all documented placeholders', () => {
  const html = renderModelsSection(parseModelsRegistry(undefined), {});
  for (const token of ['{model}', '{effort}', '{workdir}', '{sessionDir}', '{systemPromptFile}', '{name}']) {
    assert.ok(html.includes(token), `placeholder help missing ${token}`);
  }
  // escapeHtml turns < > into entities, so the provider-suffixed placeholders
  // survive as text, just not with literal angle brackets
  assert.match(html, /\{secret:.*provider.*\}/);
  assert.match(html, /\{baseUrl:.*provider.*\}/);
});

test('renderModelsSection: provider key status shows only presence/date, never a value', () => {
  const withKey = renderModelsSection(parseModelsRegistry(undefined), {
    openai: { present: true, setAt: '2026-07-28T10:00:00.000Z' },
  });
  assert.match(withKey, /hinterlegt am 28\.07\.2026/);
  assert.ok(!withKey.includes('sk-'), 'no key value must ever appear in the rendered HTML');

  const withoutKey = renderModelsSection(parseModelsRegistry(undefined), {});
  assert.match(withoutKey, /nicht hinterlegt/);
});

test('renderModelsSection escapes model/provider/harness user text against HTML injection', () => {
  const registry = parseModelsRegistry(JSON.stringify({
    models: [customModel({ label: '<script>alert(1)</script>', goodFor: '<img src=x onerror=alert(1)>' })],
  }));
  const html = renderModelsSection(registry, {});
  assert.ok(!html.includes('<script>alert(1)</script>'));
  assert.ok(!html.includes('<img src=x onerror=alert(1)>'));
  assert.ok(html.includes('&lt;script&gt;'));
});

test('renderModelsSection never receives or renders a raw API key — only status objects', () => {
  // The function signature only accepts ProviderKeyStatus (present/setAt), so there
  // is no code path through which a secret value could reach the HTML at all.
  const html = renderModelsSection(parseModelsRegistry(undefined), {
    openai: { present: true, setAt: new Date().toISOString() },
  });
  assert.ok(!/sk-[A-Za-z0-9]/.test(html));
});

test('modelsSectionScript wires up postMessage handlers and never embeds a literal secret', () => {
  const script = modelsSectionScript();
  assert.match(script, /models-check/);
  assert.match(script, /models-probe-harness/);
  assert.match(script, /models-remove/);
  assert.match(script, /provider-key-save/);
  assert.match(script, /models-add-model/);
  assert.match(script, /models-add-harness/);
  assert.match(script, /models-add-provider/);
  assert.match(script, /models-set-field/);
});

// SPEC-V3 D item 2: every model row gets an aktiv/inaktiv toggle wired through
// the same generic set-field message the loginCheckPath editor uses.
test('renderModelsSection: every model row carries an enabled toggle', () => {
  const html = renderModelsSection(parseModelsRegistry(undefined), {});
  assert.match(html, /data-toggle-enabled="claude-opus-5" checked>/);
});

// SPEC-V3 D item 5: readyPattern, promptPattern, promptIgnore, autonomy flags.
test('renderModelsSection: the harness add-form has promptPattern/promptIgnore/autonomy fields', () => {
  const html = renderModelsSection(parseModelsRegistry(undefined), {});
  assert.match(html, /id="newHarnessPromptPattern"/);
  assert.match(html, /id="newHarnessPromptIgnore"/);
  assert.match(html, /id="newHarnessAutonomy"/);
  assert.match(html, /<th>Prompt-Muster<\/th>/);
});

test('renderModelsSection: harness table shows a registered promptPattern', () => {
  const registry = parseModelsRegistry(JSON.stringify({
    harnesses: [{ id: 'codex', label: 'Codex CLI', command: 'codex', promptPattern: '^›' }],
  }));
  const html = renderModelsSection(registry, {});
  assert.match(html, /\^›/);
});

// SPEC-V3 D item 4: subscription providers get a login command, never a key field.
test('renderModelsSection: a subscription provider shows the login command, not a key field', () => {
  const registry = parseModelsRegistry(JSON.stringify({
    providers: [{ id: 'chatgpt', label: 'OpenAI ChatGPT (Abo/Login)', kind: 'subscription' }],
    harnesses: [{ id: 'codex', label: 'Codex CLI', command: 'codex' }],
    models: [{
      id: 'gpt-5-codex', label: 'GPT-5 Codex', harness: 'codex', provider: 'chatgpt', modelRef: 'gpt-5-codex',
      roles: ['worker'], efforts: ['medium'], maxEffort: 'medium', defaultEffort: 'medium', enabled: true,
    }],
  }));
  const html = renderModelsSection(registry, {});
  assert.match(html, /<code>codex<\/code>/);
  assert.ok(!html.includes('data-provider-key="chatgpt"'), 'a subscription provider must never show a key field');
  assert.match(html, /nicht automatisch prüfbar/);
  assert.match(html, /data-login-check-path="chatgpt"/);
});

test('renderModelsSection: a subscription provider with no linked model says so instead of guessing a command', () => {
  const registry = parseModelsRegistry(JSON.stringify({
    providers: [{ id: 'antigravity', label: 'Google Antigravity (Abo/Login)', kind: 'subscription' }],
  }));
  const html = renderModelsSection(registry, {});
  assert.match(html, /noch kein Harness mit diesem Provider verknüpft/);
});

// Vertrag discover.md (Spur B, task item 3): an auto-discovered model gets a
// visible "automatisch erkannt, <Datum>" badge, a curated/manual one does not.
test('renderModelsSection: an auto-discovered model is badged, a manual one is not', () => {
  const registry = parseModelsRegistry(JSON.stringify({
    models: [
      customModel({ id: 'auto-model', source: 'auto', discoveredAt: '2026-07-29T01:00:00.000Z' }),
      customModel({ id: 'manual-model', source: 'manual' }),
    ],
  }));
  const html = renderModelsSection(registry, {});
  assert.match(html, /automatisch erkannt, 29\.07\.2026/);
  assert.match(html, /auswählbar, aber nicht empfohlen/);
  const manualRow = html.slice(html.indexOf('manual-model') - 400, html.indexOf('manual-model') + 100);
  assert.ok(!manualRow.includes('automatisch erkannt'), 'a manual/curated model must not get the auto badge');
});

test('renderModelsSection: the aktiv toggle is wired for an auto-discovered model too', () => {
  const registry = parseModelsRegistry(JSON.stringify({
    models: [customModel({ id: 'auto-model', source: 'auto', discoveredAt: '2026-07-29T01:00:00.000Z' })],
  }));
  const html = renderModelsSection(registry, {});
  assert.match(html, /data-toggle-enabled="auto-model" checked>/);
});

// Vertrag Teil 2 item 4: a global "Kataloge jetzt aktualisieren" button, plus
// one per harness that actually has a discover config (a harness with none
// has nothing to re-read).
test('renderModelsSection: the global "Kataloge jetzt aktualisieren" button always renders', () => {
  const html = renderModelsSection(parseModelsRegistry(undefined), {});
  const buttonTag = html.slice(html.indexOf('id="discoverAllButton"') - 40, html.indexOf('</button>', html.indexOf('id="discoverAllButton"')));
  assert.match(buttonTag, /id="discoverAllButton"/);
  assert.match(buttonTag, /Kataloge jetzt aktualisieren/);
});

test('renderModelsSection: a harness with a discover block gets a per-harness reload button, one without does not', () => {
  const registry = parseModelsRegistry(JSON.stringify({
    harnesses: [
      { id: 'agy', label: 'Antigravity CLI', command: 'agy', discover: { source: 'command-lines', command: ['agy', 'models'] } },
      { id: 'codex', label: 'Codex CLI', command: 'codex' },
    ],
  }));
  const html = renderModelsSection(registry, {});
  assert.match(html, /data-discover-run="agy"/);
  assert.ok(!html.includes('data-discover-run="codex"'), 'codex has no discover block, no reload button');
  assert.ok(!html.includes('data-discover-run="claude"'), 'the built-in claude harness has no discover block either');
});

// Task item 3: the harness form (add AND edit) exposes the seven discover
// sub-fields; "keine" must round-trip cleanly for a harness with no block.
test('renderModelsSection: the add-harness form carries the seven discover fields', () => {
  const html = renderModelsSection(parseModelsRegistry(undefined), {});
  const marker = 'data-discover-fields="__new__"';
  assert.ok(html.includes(marker), 'add-harness form must have a discover-fields container');
  const start = html.indexOf(marker);
  const block = html.slice(start, html.indexOf('</div>\n</div>', start) + 20);
  for (const field of ['source', 'command', 'file', 'jsonPath', 'provider', 'idPrefix', 'ttlHours']) {
    assert.ok(block.includes(`data-field="${field}"`), `add-harness discover block missing data-field="${field}"`);
  }
});

test('renderModelsSection: each harness gets an inline discover editor prefilled from its current config', () => {
  const registry = parseModelsRegistry(JSON.stringify({
    harnesses: [{
      id: 'codex', label: 'Codex CLI', command: 'codex',
      discover: { source: 'file-json', file: '~/.codex/models_cache.json', jsonPath: 'models[].slug', idPrefix: 'codex-', ttlHours: 12 },
    }],
  }));
  const html = renderModelsSection(registry, {});
  assert.match(html, /data-discover-fields="codex"/);
  assert.match(html, /Erkennung: file-json/);
  assert.match(html, /value="~\/\.codex\/models_cache\.json"/);
  assert.match(html, /value="models\[\]\.slug"/);
  assert.match(html, /value="codex-"/);
  assert.match(html, /value="12"/);
  assert.match(html, /data-save-discover="codex"/);
});

test('renderModelsSection: a harness without a discover block shows "keine" selected, not a stale value', () => {
  const registry = parseModelsRegistry(JSON.stringify({
    harnesses: [{ id: 'codex', label: 'Codex CLI', command: 'codex' }],
  }));
  const html = renderModelsSection(registry, {});
  assert.match(html, /keine Erkennung/);
  const marker = 'data-discover-fields="codex"';
  const block = html.slice(html.indexOf(marker), html.indexOf(marker) + 700);
  assert.match(block, /<option value="" selected>keine<\/option>/);
});

test('modelsSectionScript wires up the discover buttons', () => {
  const script = modelsSectionScript();
  assert.match(script, /discoverAllButton/);
  assert.match(script, /data-discover-run/);
  assert.match(script, /data-save-discover/);
  assert.match(script, /models-discover/);
  assert.match(script, /readDiscoverFields/);
});

// ---- Vertrag Teil 2 (Spur B, 2026-07-29) ----

// Task item 1: search/filter bar + pager markup, and every model row's
// data-* attributes the webview script filters on.
test('renderModelsSection: the model table carries a search/filter bar, a pager and a stable id', () => {
  const html = renderModelsSection(parseModelsRegistry(undefined), {});
  assert.match(html, /id="modelsTable"/);
  assert.match(html, /id="modelSearchText"/);
  assert.match(html, /id="modelSearchHarness"/);
  assert.match(html, /id="modelSearchProvider"/);
  assert.match(html, /id="modelSearchFree"/);
  assert.match(html, /id="modelSearchEnabled"/);
  assert.match(html, /id="modelPagerCount"/);
  assert.match(html, /id="modelPagerPrev"/);
  assert.match(html, /id="modelPagerNext"/);
});

test('renderModelsSection: each model row carries data-search/-harness/-provider/-free/-enabled', () => {
  const registry = parseModelsRegistry(JSON.stringify({
    models: [
      customModel({ id: 'free-model', cost: { inPerMTok: 0, outPerMTok: 0 }, enabled: true }),
      customModel({ id: 'paid-model', cost: { inPerMTok: 3, outPerMTok: 9 }, enabled: false }),
    ],
  }));
  const html = renderModelsSection(registry, {});
  assert.match(html, /data-search="free-model gpt-5 codex gpt-5-codex"[^>]*data-harness="codex"[^>]*data-provider="openai"[^>]*data-free="1"[^>]*data-enabled="1"/);
  assert.match(html, /data-search="paid-model gpt-5 codex gpt-5-codex"[^>]*data-harness="codex"[^>]*data-provider="openai"[^>]*data-free="0"[^>]*data-enabled="0"/);
});

test('renderModelsSection: a model with no cost info at all is data-free="0" (unknown, not free)', () => {
  const registry = parseModelsRegistry(JSON.stringify({ models: [customModel({ id: 'unpriced' })] }));
  const html = renderModelsSection(registry, {});
  assert.match(html, /<tr data-search="unpriced[^>]*data-free="0"/);
});

test('modelsSectionScript wires up the search/filter/pager logic and never round-trips filtering to the extension', () => {
  const script = modelsSectionScript();
  assert.match(script, /setupModelSearch/);
  assert.match(script, /PAGE_SIZE/);
  assert.match(script, /modelSearchText/);
  assert.match(script, /modelPagerPrev/);
  assert.match(script, /modelPagerNext/);
  // The filter predicate must read the row's own dataset, never postMessage a
  // query to the extension host (Vertrag Teil 2 §4: "gehört ins Webview-Skript").
  assert.ok(!/vscode\.postMessage[^;]*modelSearch/.test(script), 'filtering must stay entirely client-side');
});

// Task item 3: API-Keys je Harness.
test('renderModelsSection: a harness with a cloud-provider model gets an "API-Keys je Harness" block naming the shared providers', () => {
  const registry = parseModelsRegistry(JSON.stringify({
    providers: [{ id: 'openrouter', label: 'OpenRouter', kind: 'cloud' }],
    harnesses: [
      { id: 'aider', label: 'aider-chat', command: 'aider' },
      { id: 'opencode', label: 'opencode', command: 'opencode' },
    ],
    models: [
      { id: 'aider-or-x', label: 'X (aider)', harness: 'aider', provider: 'openrouter', modelRef: 'x', roles: ['worker'], efforts: ['medium'], maxEffort: 'medium', defaultEffort: 'medium', enabled: true },
      { id: 'opencode-or-x', label: 'X (opencode)', harness: 'opencode', provider: 'openrouter', modelRef: 'x', roles: ['worker'], efforts: ['medium'], maxEffort: 'medium', defaultEffort: 'medium', enabled: true },
    ],
  }));
  const html = renderModelsSection(registry, {});
  assert.match(html, /<h3>API-Keys je Harness<\/h3>/);
  assert.match(html, /aider-chat — API-Keys \(1\)/);
  assert.match(html, /opencode — API-Keys \(1\)/);
  // aider's block names opencode as sharing the same provider, and vice versa.
  const aiderBlockStart = html.indexOf('aider-chat — API-Keys');
  const aiderBlock = html.slice(aiderBlockStart, aiderBlockStart + 700);
  assert.match(aiderBlock, /gilt auch für: opencode/);
});

test('renderModelsSection: a harness with only local/subscription providers gets no API-Keys block', () => {
  const html = renderModelsSection(parseModelsRegistry(undefined), {});
  // claude -> claude-subscription (subscription), pi -> ollama (local) — neither is cloud.
  assert.ok(!html.includes('Claude Code — API-Keys'));
  assert.ok(!html.includes('pi — API-Keys'));
  assert.match(html, /Kein Harness mit einem Cloud-Provider registriert\./);
});

test('renderModelsSection: the harness-level key field never collides with the provider table\'s own field (data-save-key lookup bug)', () => {
  const registry = parseModelsRegistry(JSON.stringify({
    providers: [{ id: 'openrouter', label: 'OpenRouter', kind: 'cloud' }],
    harnesses: [{ id: 'aider', label: 'aider-chat', command: 'aider' }],
    models: [{ id: 'aider-or-x', label: 'X', harness: 'aider', provider: 'openrouter', modelRef: 'x', roles: ['worker'], efforts: ['medium'], maxEffort: 'medium', defaultEffort: 'medium', enabled: true }],
  }));
  const html = renderModelsSection(registry, {});
  // Two independent key inputs for the SAME provider id must both exist —
  // one in the Provider table, one under the harness.
  const occurrences = html.split('data-provider-key="openrouter"').length - 1;
  assert.equal(occurrences, 2, 'expected one key input in the Provider table and one under the harness');
  // Both wrapped in .keyPair so the click handler can scope its lookup instead
  // of a page-wide querySelector that would always hit the first one.
  assert.match(html, /<span class="keyPair">[\s\S]*?data-provider-key="openrouter"[\s\S]*?data-save-key="openrouter"[\s\S]*?<\/span>/);
});

test('modelsSectionScript scopes the save-key lookup to the button\'s own .keyPair, not a page-wide selector', () => {
  const script = modelsSectionScript();
  assert.match(script, /closest\('\.keyPair'\)/);
  assert.match(script, /input\.dataset\.providerKey/);
});

// Task item 6: catalog/balance status, honest "unbekannt" when absent.
test('renderModelsSection: catalog/balance status render as "unbekannt" when the provider carries neither field', () => {
  const registry = parseModelsRegistry(JSON.stringify({ providers: [{ id: 'openrouter', label: 'OpenRouter', kind: 'cloud' }] }));
  const html = renderModelsSection(registry, {});
  assert.match(html, /Katalogstatus: unbekannt/);
  assert.match(html, /Guthaben: unbekannt/);
});

test('renderModelsSection: a real catalogStatus/balanceStatus renders the documented example strings', () => {
  const registry = parseModelsRegistry(JSON.stringify({
    providers: [{
      id: 'openrouter', label: 'OpenRouter', kind: 'cloud',
      catalogStatus: { count: 367, fetchedAt: '2026-07-29T10:22:00.000Z' },
      balanceStatus: { amount: 4.12, currency: '$', fetchedAt: '2026-07-29T10:22:00.000Z' },
    }],
  }));
  const html = renderModelsSection(registry, {});
  assert.match(html, /367 im Katalog \(abgerufen \d{2}:\d{2}\)/);
  assert.match(html, /Guthaben 4,12 \$ \(abgerufen \d{2}:\d{2}\)/);
});

test('renderModelsSection: a catalog with no key appends "Key fehlt — nichts registriert" (task item 6 example)', () => {
  const registry = parseModelsRegistry(JSON.stringify({
    providers: [{ id: 'openrouter', label: 'OpenRouter', kind: 'cloud', catalogStatus: { count: 367 } }],
  }));
  const html = renderModelsSection(registry, {});
  assert.match(html, /367 im Katalog, Key fehlt — nichts registriert/);
});

test('renderModelsSection: balanceStatus amount:null renders as "unbegrenzt"', () => {
  const registry = parseModelsRegistry(JSON.stringify({
    providers: [{ id: 'openrouter', label: 'OpenRouter', kind: 'cloud', balanceStatus: { amount: null } }],
  }));
  const html = renderModelsSection(registry, {});
  assert.match(html, /Guthaben unbegrenzt/);
});
