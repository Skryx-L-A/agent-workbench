import assert from 'node:assert/strict';
import { test } from 'node:test';
import { parseModelsRegistry } from '../src/models.ts';
import { DEFAULT_SETTINGS, type Settings } from '../src/settings.ts';
import { renderSettingsHtml } from '../src/settingsHtml.ts';

function render(
  settings: Partial<Settings> = {},
  registryJson?: Record<string, unknown>,
  mcpSharedStatusText?: string,
  hooks?: import('../src/hooksInfo.ts').HookEntry[],
  pathInfo?: import('../src/systemInfo.ts').PathInfo[],
  launchdJobs?: import('../src/systemInfo.ts').LaunchdJob[],
): string {
  return renderSettingsHtml(
    { ...DEFAULT_SETTINGS, ...settings },
    'vscode-resource:/codicon.css',
    'vscode-resource:',
    'N0NCE',
    registryJson ? parseModelsRegistry(JSON.stringify(registryJson)) : undefined,
    {},
    undefined,
    {},
    mcpSharedStatusText,
    hooks,
    pathInfo,
    launchdJobs,
  );
}

test('renderSettingsHtml offers exactly the five Claude models', () => {
  const html = render();
  for (const model of ['claude-opus-5', 'claude-opus-4-8', 'claude-sonnet-5', 'claude-haiku-4-5', 'claude-fable-5']) {
    assert.match(html, new RegExp(`<option value="${model}"`));
  }
  assert.match(html, /<option value="claude-opus-5" selected>/);
});

test('renderSettingsHtml never offers effort max', () => {
  const html = render();
  assert.match(html, /<option value="xhigh"/);
  assert.ok(!/<option value="max"/.test(html), 'max is above the policy ceiling');
});

test('renderSettingsHtml carries the fable cap into the webview', () => {
  const html = render();
  assert.match(html, /"claude-fable-5":\{"efforts":\["low","medium"\],"supportsEffort":true\}/);
});

test('renderSettingsHtml shows both worker layouts, the stored one active', () => {
  const html = render({ workerLayout: 'window' });
  assert.match(html, /data-layout="split" class=""/);
  assert.match(html, /data-layout="window" class="active"/);
  assert.match(html, /Panes neben dem Orchestrator/);
  assert.match(html, /Eigener Tab, alle Worker zusammen/);
});

/**
 * The <select> of the orchestrator model — the worker select is Claude-only.
 * Matches `<select id="orchestratorModel"` with ANY trailing attributes (task
 * item 2 added `class="modelPickerSelect" hidden` to it) rather than the
 * exact opening tag text, so this helper does not silently start matching
 * nothing — and every assertion built on it does not silently start passing
 * vacuously — the next time the tag grows an attribute.
 */
function orchestratorModelSelect(html: string): string {
  const match = /<select id="orchestratorModel"[^>]*>/.exec(html);
  if (!match) {
    return '';
  }
  const start = match.index;
  return html.slice(start, html.indexOf('</select>', start));
}

test('renderSettingsHtml explains what each worker layout does', () => {
  const html = render();
  assert.match(html, /Die Umstellung greift sofort, laufende Worker bleiben am Leben\./);
  assert.match(html, /Panes neben dem Orchestrator: alles in einem Terminal/);
  assert.match(html, /zweiter Terminal-Tab „Claude Workbench — Worker" im selben Fenster/);
  assert.match(html, /umschaltbar in der Terminal-Liste/);
  assert.ok(!/\p{Extended_Pictographic}/u.test(html), 'no emoji in the explanation');
});

test('renderSettingsHtml switches the model list with the harness (SPEC-V2 F)', () => {
  const claude = render();
  assert.match(claude, /data-harness="claude" class="active"/);
  assert.ok(
    !orchestratorModelSelect(claude).includes('value="ornith"'),
    'pi aliases must not show up for Claude Code',
  );
  assert.match(claude, />Effort</);

  const pi = render({ orchestratorHarness: 'pi', orchestratorModel: 'ornith' });
  assert.match(pi, /data-harness="pi" class="active"/);
  assert.match(pi, /<option value="ornith" selected>/);
  assert.match(pi, /<option value="qwen"/);
  assert.match(pi, /<option value="ornith9"/);
  assert.ok(
    !orchestratorModelSelect(pi).includes('value="claude-opus-5"'),
    'Claude ids must not show up for pi',
  );
  assert.match(pi, />Thinking-Level</);
});

test('renderSettingsHtml opens the free-text field for a full Ollama id', () => {
  const custom = render({ orchestratorHarness: 'pi', orchestratorModel: 'ornith:35b' });
  assert.match(custom, /<option value="__custom__" selected>/);
  assert.match(custom, /id="customModel" value="ornith:35b"/);
  // with an alias selected the field stays hidden
  assert.match(render({ orchestratorHarness: 'pi', orchestratorModel: 'qwen' }), /id="customModelRow" hidden/);
});

// Vertrag discover.md / task item 4: the orchestrator picker for a non-claude/
// pi harness already filters by role 'orchestrator' — this locks in that an
// auto-discovered model (roles from the discover default) makes the picker
// show real options, and the "kein Orchestrator-Modell registriert" hint
// appears ONLY when a harness truly has none, not just because it was never
// hand-curated.
test('renderSettingsHtml: orchestrator picker shows an auto-discovered model; the empty hint only fires when truly empty', () => {
  const withAuto = render(
    { orchestratorHarness: 'codex', orchestratorModel: 'codex-gpt-5' },
    {
      providers: [{ id: 'chatgpt', label: 'ChatGPT', kind: 'subscription' }],
      harnesses: [{ id: 'codex', label: 'Codex CLI', command: 'codex' }],
      models: [{
        id: 'codex-gpt-5', label: 'GPT-5 (codex)', harness: 'codex', provider: 'chatgpt', modelRef: 'gpt-5',
        roles: ['worker', 'orchestrator'], efforts: ['low', 'medium', 'high'], maxEffort: 'high', defaultEffort: 'medium',
        enabled: true, source: 'auto', discoveredAt: '2026-07-29T01:00:00Z',
      }],
    },
  );
  assert.match(withAuto, /<option value="codex-gpt-5"/);
  assert.ok(!withAuto.includes('für diesen Harness registriert'), 'a real auto-discovered option must suppress the empty hint');

  const withoutModels = render(
    { orchestratorHarness: 'codex' },
    { harnesses: [{ id: 'codex', label: 'Codex CLI', command: 'codex' }] },
  );
  assert.match(withoutModels, /Kein Modell mit Rolle „Orchestrator" für diesen Harness registriert\./);
});

test('renderSettingsHtml has no emoji and locks scripts to the nonce', () => {
  const html = render();
  assert.ok(!/\p{Extended_Pictographic}/u.test(html), 'emoji found in the settings panel');
  assert.match(html, /script-src 'nonce-N0NCE'/);
  assert.ok(!/script-src[^;]*unsafe-inline/.test(html), 'inline scripts still allowed');
});

test('renderSettingsHtml escapes the stored directory', () => {
  const html = render({ newSessionDefaultDir: '~/AI"><script>alert(1)</script>' });
  assert.ok(!html.includes('"><script>alert(1)'), 'raw markup leaked into the webview');
  assert.match(html, /&quot;&gt;&lt;script&gt;/);
});

// SPEC-V2 A, four new keys (Reviewer-Befund F5).
test('renderSettingsHtml shows the guard thresholds and the pane-width floor', () => {
  const html = render({ guardOrchWarnPct: 70, guardWorkerWarnPct: 85, minWorkerPaneWidth: 72 });
  assert.match(html, /id="guardOrchWarnPct" min="1" max="99" step="1" value="70"/);
  assert.match(html, /id="guardWorkerWarnPct" min="1" max="99" step="1" value="85"/);
  assert.match(html, /id="minWorkerPaneWidth" min="20" step="1" value="72"/);
});

test('renderSettingsHtml shows maxWorkers and the default-machine switch, the stored one active', () => {
  const html = render({ maxWorkers: 5, defaultWorkerMachine: 'mac' });
  assert.match(html, /id="maxWorkers" min="1" step="1" value="5"/);
  assert.match(html, /data-worker-machine="local" class=""/);
  assert.match(html, /data-worker-machine="mac" class="active"/);
  assert.match(html, /data-worker-machine="peer" class=""/);
});

test('renderSettingsHtml wires change handlers for the four new fields', () => {
  const html = render();
  assert.match(html, /set\('guardOrchWarnPct', Number\(event\.target\.value\)\)/);
  assert.match(html, /set\('guardWorkerWarnPct', Number\(event\.target\.value\)\)/);
  assert.match(html, /set\('minWorkerPaneWidth', Number\(event\.target\.value\)\)/);
  assert.match(html, /set\('maxWorkers', Number\(event\.target\.value\)\)/);
  assert.match(html, /set\('defaultWorkerMachine', button\.dataset\.workerMachine\)/);
});

// SPEC-V3 D item 1: the orchestrator harness picker draws from the FULL
// registry now, not the fixed claude|pi list — this is the gap the BAU-Auftrag
// named explicitly.
const CODEX_REGISTRY = {
  harnesses: [{ id: 'codex', label: 'Codex CLI', command: 'codex', readyPattern: '^›' }],
  models: [{
    id: 'gpt-5-codex', label: 'GPT-5 Codex', harness: 'codex', provider: 'openai', modelRef: 'gpt-5-codex',
    roles: ['worker', 'orchestrator'], efforts: ['low', 'medium', 'high'], maxEffort: 'high',
    defaultEffort: 'medium', enabled: true,
  }],
};

test('renderSettingsHtml offers every registered harness as an orchestrator choice, not just claude/pi', () => {
  const html = render({}, CODEX_REGISTRY);
  assert.match(html, /data-harness="codex"/);
  assert.match(html, /data-harness="claude"/);
  assert.match(html, /data-harness="pi"/);
});

test('renderSettingsHtml: switching to a registered harness lists its own registered models', () => {
  const html = render({ orchestratorHarness: 'codex', orchestratorModel: 'gpt-5-codex' }, CODEX_REGISTRY);
  assert.match(html, /data-harness="codex" class="active"/);
  assert.match(html, /<option value="gpt-5-codex" selected>/);
  assert.ok(!orchestratorModelSelect(html).includes('claude-opus-5'), 'a Claude id must not leak into a codex model list');
});

test('renderSettingsHtml: a harness with zero orchestrator-role models shows a message, not a broken select', () => {
  const html = render({ orchestratorHarness: 'aider' }, {
    harnesses: [{ id: 'aider', label: 'aider-chat', command: 'aider' }],
    models: [{
      id: 'aider-ornith', label: 'aider + Ornith', harness: 'aider', provider: 'ollama', modelRef: 'ollama/ornith:9b',
      roles: ['worker'], efforts: ['low'], maxEffort: 'low', defaultEffort: 'low', enabled: true,
    }],
  });
  assert.match(html, /Kein Modell mit Rolle „Orchestrator" für diesen Harness registriert/);
  assert.ok(!/<select id="orchestratorModel"[^>]*>/.test(html), 'no select for a harness with nothing to choose');
});

// SPEC-V3 D item 3: opencode's real registered shape (effort.style: 'none') —
// the effort row must not offer a control that does nothing on spawn.
test('renderSettingsHtml hides the effort row for a model whose harness takes no effort flag', () => {
  const html = render({ orchestratorHarness: 'opencodeish', orchestratorModel: 'oc-model' }, {
    harnesses: [{ id: 'opencodeish', label: 'opencodeish', command: 'oc', readyPattern: 'ok', effort: { style: 'none' } }],
    models: [{
      id: 'oc-model', label: 'OC Model', harness: 'opencodeish', provider: 'ollama', modelRef: 'x',
      roles: ['orchestrator'], efforts: ['low'], maxEffort: 'low', defaultEffort: 'low', enabled: true,
    }],
  });
  assert.match(html, /id="orchestratorEffortRow" hidden/);
  assert.match(html, /id="orchestratorEffortRowHint">/);
  assert.ok(!/id="orchestratorEffortRowHint" hidden/.test(html), 'the explanatory hint must be visible when the row is hidden');
});

test('renderSettingsHtml shows the contextGuardAutostart checkbox, unchecked by default, with a confirm on enabling', () => {
  const off = render();
  assert.match(off, /id="contextGuardAutostart">/);
  assert.ok(!/id="contextGuardAutostart" checked/.test(off));
  const on = render({ contextGuardAutostart: true });
  assert.match(on, /id="contextGuardAutostart" checked>/);
  assert.match(on, /confirm\(/);
});

// Vertrag Teil 2 §5 (task item 5): on by default (opposite polarity from
// contextGuardAutostart, which is off by default), no confirm dialog — this
// switch has no cross-guard double-typing risk to warn about.
test('renderSettingsHtml shows the modelDiscoveryAuto checkbox, checked by default', () => {
  const on = render();
  assert.match(on, /id="modelDiscoveryAuto" checked>/);
  const off = render({ modelDiscoveryAuto: false });
  assert.match(off, /id="modelDiscoveryAuto">/);
  assert.ok(!/id="modelDiscoveryAuto" checked/.test(off));
  assert.match(on, /set\('modelDiscoveryAuto', event\.target\.checked\)/);
});

// Task item 2: the orchestrator/worker <select> gets a type-to-filter overlay
// (Vertrag Teil 2 §4 — "wird ebenfalls ein Suchfeld statt eines reinen
// Dropdowns"). The select itself stays in the DOM (hidden) as the value
// holder every existing listener already targets.
test('renderSettingsHtml: orchestratorModel and workerModel each get a search-picker overlay, the select stays hidden', () => {
  const html = render();
  assert.match(html, /<select id="orchestratorModel" class="modelPickerSelect" hidden>/);
  assert.match(html, /<select id="workerModel" class="modelPickerSelect" hidden>/);
  assert.match(html, /data-picker-for="orchestratorModel"/);
  assert.match(html, /data-picker-list-for="orchestratorModel"/);
  assert.match(html, /data-picker-for="workerModel"/);
  assert.match(html, /data-picker-list-for="workerModel"/);
});

test('renderSettingsHtml: pi\'s orchestrator select also gets the search-picker overlay', () => {
  const html = render({ orchestratorHarness: 'pi', orchestratorModel: 'ornith' });
  assert.match(html, /<select id="orchestratorModel" class="modelPickerSelect" hidden>/);
  assert.match(html, /data-picker-for="orchestratorModel"/);
});

// The <input> itself has no server-rendered value — wireModelPicker fills it
// from the select's (server-rendered, `selected`-marked) current option the
// moment the script runs, before any user interaction, so the visible label
// always starts as the CURRENT selection ("Auswahl nicht verloren").
test('renderSettingsHtml: wireModelPicker prefills the search input from the select\'s current value on load', () => {
  const html = render({ orchestratorHarness: 'claude', orchestratorModel: 'claude-sonnet-5' });
  assert.match(html, /input\.value = labelOf\(select\.value\);/);
  assert.match(html, /<option value="claude-sonnet-5" selected>/, 'the select itself still carries the real selection server-side');
});

test('renderSettingsHtml wires wireModelPicker for both selects, sorting the current value first ("steht immer oben")', () => {
  const html = render();
  assert.match(html, /function wireModelPicker\(selectId\)/);
  assert.match(html, /wireModelPicker\('orchestratorModel'\)/);
  assert.match(html, /wireModelPicker\('workerModel'\)/);
  assert.match(html, /a\.value === currentValue \? -1/, 'the current selection must sort first in the filtered list');
});

// Nachtrag priority 3: shared MCP-server status/control.
test('renderSettingsHtml shows the mcp-shared status text and control buttons', () => {
  const html = render({}, undefined, 'SERVER  PORT  ZUSTAND\nmcp-basic-memory  8766  läuft');
  assert.match(html, /id="mcpSharedStatus">SERVER  PORT  ZUSTAND\nmcp-basic-memory  8766  läuft</);
  assert.match(html, /id="mcpSharedRestart"/);
  assert.match(html, /id="mcpSharedApply"/);
  assert.match(html, /id="mcpSharedReap"/);
});

test('renderSettingsHtml degrades gracefully when mcp-shared has not answered yet', () => {
  const html = render();
  assert.match(html, /id="mcpSharedStatus">Noch nicht geladen\.</);
});

test('renderSettingsHtml escapes the mcp-shared status text against HTML injection', () => {
  const html = render({}, undefined, '<script>alert(1)</script>');
  assert.ok(!html.includes('<script>alert(1)</script>'));
  assert.ok(html.includes('&lt;script&gt;'));
});

// Nachtrag priority 3: read-only hooks list, deny-hooks flagged, no crash on empty.
test('renderSettingsHtml lists hooks and flags the three deny-hooks', () => {
  const html = render({}, undefined, undefined, [
    { event: 'PreToolUse', matcher: 'Bash', command: 'bash guard-secrets.sh', isDenyHook: true, denyHookReason: 'Secrets.' },
    { event: 'SessionStart', command: 'bash start.sh', isDenyHook: false },
  ]);
  assert.match(html, /<h2>Hooks<\/h2>/);
  assert.match(html, /PreToolUse/);
  assert.match(html, /guard-secrets\.sh/);
  assert.match(html, /class="denyHook"/);
  assert.match(html, /Deny-Hook — Secrets\./);
  assert.match(html, /<tr>\s*<td>SessionStart<\/td>/, 'the non-deny row stays a plain <tr>, no class attribute');
});

test('renderSettingsHtml: an empty hooks list shows a hint, never crashes', () => {
  const html = render({}, undefined, undefined, []);
  assert.match(html, /Keine Hooks gefunden/);
});

// Nachtrag priority 4: paths (existence+size, öffnen), launchd jobs, pi aliases.
test('renderSettingsHtml shows path status/size and an Öffnen button only when it exists', () => {
  const html = render({}, undefined, undefined, undefined, [
    { label: 'Vault', path: '/Users/alice/Knowledge', exists: true, sizeHuman: '94M' },
    { label: 'Worker-Ergebnisse', path: '/Users/alice/.pi-workers', exists: false },
  ]);
  assert.match(html, /Vault/);
  assert.match(html, /94M/);
  assert.match(html, /data-open-path="\/Users\/alice\/Knowledge"/);
  assert.ok(!html.includes('data-open-path="/Users/alice/.pi-workers"'), 'no Öffnen button for a missing path');
  assert.match(html, />fehlt</);
});

test('renderSettingsHtml shows the five launchd jobs with PID/exit status', () => {
  const html = render({}, undefined, undefined, undefined, undefined, [
    { label: 'agent-workbench.mcp-playwright', loaded: true, pid: 1271, lastExit: 0 },
    { label: 'agent-workbench.mcp-reaper', loaded: true, lastExit: 0 },
  ]);
  assert.match(html, /agent-workbench\.mcp-playwright/);
  assert.match(html, />1271</);
  assert.match(html, /agent-workbench\.mcp-reaper/);
});

test('renderSettingsHtml lists the pi model aliases, read-only (no input fields)', () => {
  const html = render();
  assert.match(html, /<h2>Lokale Modell-Aliase \(pi\)<\/h2>/);
  assert.match(html, />ornith</);
  assert.match(html, /ornith:35b/);
  assert.match(html, /nur Anzeige|Nur Anzeige/);
});
