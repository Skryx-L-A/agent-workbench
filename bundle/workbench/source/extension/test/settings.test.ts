import assert from 'node:assert/strict';
import { test } from 'node:test';
import {
  applyChange,
  clampEffort,
  coerceSetting,
  DEFAULT_SETTINGS,
  effortLabel,
  effortsForModel,
  expandHome,
  isModelForHarness,
  parseSettings,
} from '../src/settings.ts';

test('parseSettings falls back to the defaults for missing or broken input', () => {
  assert.deepEqual(parseSettings(undefined), DEFAULT_SETTINGS);
  assert.deepEqual(parseSettings('not json'), DEFAULT_SETTINGS);
  assert.deepEqual(parseSettings('[]'), DEFAULT_SETTINGS);
  assert.deepEqual(parseSettings('{}'), DEFAULT_SETTINGS);
});

test('parseSettings takes the stored values and rejects unusable ones', () => {
  const settings = parseSettings(JSON.stringify({
    orchestratorModel: 'claude-sonnet-5',
    orchestratorEffort: 'high',
    workerLayout: 'window',
    workerPollSeconds: 12,
    terminalStartMaximized: false,
    newSessionDefaultDir: '~/Projekte',
    unknownFutureKey: 'kept elsewhere',
  }));
  assert.equal(settings.orchestratorModel, 'claude-sonnet-5');
  assert.equal(settings.orchestratorEffort, 'high');
  assert.equal(settings.workerLayout, 'window');
  assert.equal(settings.workerPollSeconds, 12);
  assert.equal(settings.terminalStartMaximized, false);
  assert.equal(settings.newSessionDefaultDir, '~/Projekte');

  const bad = parseSettings(JSON.stringify({
    orchestratorModel: 'gpt-9',
    orchestratorEffort: 'max',
    workerLayout: 'tabs',
    workerPollSeconds: 0,
  }));
  // 'max' is above the policy ceiling and must never come back as a value
  assert.equal(bad.orchestratorEffort, DEFAULT_SETTINGS.orchestratorEffort);
  assert.equal(bad.orchestratorModel, DEFAULT_SETTINGS.orchestratorModel);
  assert.equal(bad.workerLayout, DEFAULT_SETTINGS.workerLayout);
  assert.equal(bad.workerPollSeconds, DEFAULT_SETTINGS.workerPollSeconds);
});

test('effortsForModel caps fable at medium, everything else at xhigh', () => {
  assert.deepEqual(effortsForModel('claude-fable-5'), ['low', 'medium']);
  assert.deepEqual(effortsForModel('claude-opus-5'), ['low', 'medium', 'high', 'xhigh']);
  assert.ok(!effortsForModel('claude-opus-5').includes('max' as never));
  assert.equal(clampEffort('claude-fable-5', 'xhigh'), 'medium');
  assert.equal(clampEffort('claude-opus-5', 'xhigh'), 'xhigh');
});

test('coerceSetting validates per key, harness decides the model ids (SPEC-V2 F)', () => {
  assert.equal(coerceSetting('orchestratorModel', 'claude-opus-5', 'claude'), 'claude-opus-5');
  assert.equal(coerceSetting('orchestratorModel', 'ornith', 'claude'), undefined);
  assert.equal(coerceSetting('orchestratorModel', 'ornith', 'pi'), 'ornith');
  assert.equal(coerceSetting('orchestratorModel', 'ornith:35b', 'pi'), 'ornith:35b');
  assert.equal(coerceSetting('orchestratorModel', 'claude-opus-5', 'pi'), undefined);
  assert.equal(coerceSetting('orchestratorHarness', 'pi'), 'pi');
  // SPEC-V3 D: a harness is registry DATA now, not a fixed claude|pi pair — the
  // gap the BAU-Auftrag names explicitly. Real membership is checked where the
  // registry lives (settingsView.ts/wb-code), not here.
  assert.equal(coerceSetting('orchestratorHarness', 'codex'), 'codex');
  assert.equal(coerceSetting('orchestratorHarness', 'not a valid id'), undefined, 'still rejects garbage with whitespace');
  assert.equal(coerceSetting('orchestratorEffort', 'max'), undefined);
  assert.equal(coerceSetting('workerLayout', 'window'), 'window');
  assert.equal(coerceSetting('terminalStartMaximized', 'ja'), undefined);
  assert.equal(coerceSetting('contextGuardAutostart', true), true);
  assert.equal(coerceSetting('contextGuardAutostart', 'true'), undefined, 'a string is not a boolean');
  assert.equal(coerceSetting('modelDiscoveryAuto', false), false);
  assert.equal(coerceSetting('modelDiscoveryAuto', 'false'), undefined, 'a string is not a boolean');
  assert.equal(coerceSetting('workerPollSeconds', '7'), 7);
  assert.equal(coerceSetting('workerPollSeconds', 1000), undefined);
  assert.equal(coerceSetting('newSessionDefaultDir', '  '), undefined);
  assert.equal(coerceSetting('rmRf', '/'), undefined);
});

// SPEC-V3 D, Reviewer-Befund M6: workerModel is harness-independent — any
// registered model id (not just the five Claude ones) is a valid default.
test('coerceSetting accepts any registered-shape workerModel id, not just the five Claude ones', () => {
  assert.equal(coerceSetting('workerModel', 'claude-sonnet-5'), 'claude-sonnet-5');
  assert.equal(coerceSetting('workerModel', 'gpt-5-codex'), 'gpt-5-codex');
  assert.equal(coerceSetting('workerModel', 'ollama/ornith:9b'), 'ollama/ornith:9b');
  assert.equal(coerceSetting('workerModel', 'rm -rf /'), undefined, 'still rejects garbage with whitespace');
  assert.equal(coerceSetting('workerModel', ''), undefined);
});

test('isModelForHarness: pi also takes a full Ollama id, claude does not', () => {
  assert.equal(isModelForHarness('pi', 'huihui_ai/model-abliterated:q6_k'), true);
  assert.equal(isModelForHarness('pi', 'rm -rf /'), false);
  assert.equal(isModelForHarness('claude', 'claude-haiku-4-5'), true);
  assert.equal(isModelForHarness('claude', 'claude-haiku'), false);
});

test('applyChange keeps unknown keys and clamps the effort (SPEC-V2 A)', () => {
  const stored = {
    orchestratorModel: 'claude-opus-5',
    orchestratorEffort: 'xhigh',
    contextGuardAutostart: true,
    somethingFromV3: { nested: 1 },
  };
  const next = applyChange(stored, 'orchestratorModel', 'claude-fable-5');
  assert.equal(next.orchestratorModel, 'claude-fable-5');
  assert.equal(next.orchestratorEffort, 'medium', 'fable must not keep xhigh');
  assert.equal(next.contextGuardAutostart, true);
  assert.deepEqual(next.somethingFromV3, { nested: 1 });
});

test('applyChange reinterprets the model when the harness switches (SPEC-V2 F)', () => {
  const toPi = applyChange(
    { orchestratorHarness: 'claude', orchestratorModel: 'claude-opus-5', orchestratorEffort: 'xhigh' },
    'orchestratorHarness',
    'pi',
  );
  assert.equal(toPi.orchestratorModel, 'ornith');
  assert.equal(toPi.orchestratorEffort, 'xhigh');

  const back = applyChange(
    { orchestratorHarness: 'pi', orchestratorModel: 'ornith' },
    'orchestratorHarness',
    'claude',
  );
  assert.equal(back.orchestratorModel, 'claude-opus-5');

  // a model that fits the new harness survives the switch
  const keeps = applyChange(
    { orchestratorHarness: 'claude', orchestratorModel: 'ornith9' },
    'orchestratorHarness',
    'pi',
  );
  assert.equal(keeps.orchestratorModel, 'ornith9');
});

// SPEC-V3 D: settings.ts stays registry-agnostic itself, but takes resolvers
// from a caller who DID load the registry (settingsView.ts) — this is that seam.
test('applyChange uses the registry hooks for a non-claude/pi harness switch', () => {
  const toCodex = applyChange(
    { orchestratorHarness: 'claude', orchestratorModel: 'claude-opus-5', orchestratorEffort: 'xhigh' },
    'orchestratorHarness',
    'codex',
    {
      defaultModelFor: (harness) => (harness === 'codex' ? 'gpt-5-codex' : 'claude-opus-5'),
      effortsForModel: (model) => (model === 'gpt-5-codex' ? ['low', 'medium', 'high'] : ['low', 'medium', 'high', 'xhigh']),
      // Without this hook, isModelForHarness's id-SHAPE check for a custom
      // harness would call ANY plausible-looking id "fitting" — a real
      // membership check is what makes the stale Claude id get reset at all.
      modelFitsHarness: (harness, model) => harness === 'codex' && model === 'gpt-5-codex',
    },
  );
  assert.equal(toCodex.orchestratorModel, 'gpt-5-codex');
  assert.equal(toCodex.orchestratorEffort, 'high', 'clamped to the registry model\'s own maxEffort, not the Claude/fable rule');

  // Without any hooks, settings.ts has no registry to check membership against
  // — isModelForHarness's shape-only check for a custom harness therefore
  // leaves a syntactically plausible id alone rather than guessing wrong
  // (deliberate, mirrors wb-state settings set's own "free but string" rule).
  const noHooks = applyChange(
    { orchestratorHarness: 'claude', orchestratorModel: 'claude-sonnet-5' },
    'orchestratorHarness',
    'codex',
  );
  assert.equal(noHooks.orchestratorModel, 'claude-sonnet-5', 'left as-is, not silently reset without a registry to check against');
});

test('effortLabel follows the harness wording', () => {
  assert.equal(effortLabel('claude'), 'Effort');
  assert.equal(effortLabel('pi'), 'Thinking-Level');
});

test('expandHome resolves the leading ~ only', () => {
  assert.equal(expandHome('~/AI', '/Users/alice'), '/Users/alice/AI');
  assert.equal(expandHome('~', '/Users/alice'), '/Users/alice');
  assert.equal(expandHome('/Users/alice/AI', '/Users/alice'), '/Users/alice/AI');
  assert.equal(expandHome('foo/~/bar', '/Users/alice'), 'foo/~/bar');
});

// SPEC-V2 A, four new keys (Reviewer-Befund F5: every key needs a real consumer,
// a test, and a contract line — this is the test).
test('parseSettings takes valid guard/grid/machine keys', () => {
  const settings = parseSettings(JSON.stringify({
    guardOrchWarnPct: 60,
    guardWorkerWarnPct: 90,
    minWorkerPaneWidth: 80,
    maxWorkers: 3,
    defaultWorkerMachine: 'peer',
  }));
  assert.equal(settings.guardOrchWarnPct, 60);
  assert.equal(settings.guardWorkerWarnPct, 90);
  assert.equal(settings.minWorkerPaneWidth, 80);
  assert.equal(settings.maxWorkers, 3);
  assert.equal(settings.defaultWorkerMachine, 'peer');
});

test('parseSettings falls back to defaults for guard/grid/machine keys (SPEC-V2 A)', () => {
  assert.deepEqual(
    parseSettings(undefined),
    DEFAULT_SETTINGS,
  );
  assert.equal(DEFAULT_SETTINGS.guardOrchWarnPct, 75, 'default must match the old ORCH_PCT constant');
  assert.equal(DEFAULT_SETTINGS.guardWorkerWarnPct, 80, 'default must match the old WARN_PCT constant');
  assert.equal(DEFAULT_SETTINGS.minWorkerPaneWidth, 60);
  assert.equal(DEFAULT_SETTINGS.maxWorkers, 8);
  assert.equal(DEFAULT_SETTINGS.defaultWorkerMachine, 'local');

  const bad = parseSettings(JSON.stringify({
    guardOrchWarnPct: 0,
    guardWorkerWarnPct: 100,
    minWorkerPaneWidth: 19,
    maxWorkers: 0,
    defaultWorkerMachine: 'moon',
  }));
  assert.equal(bad.guardOrchWarnPct, DEFAULT_SETTINGS.guardOrchWarnPct, '0 is out of the 1..99 range');
  assert.equal(bad.guardWorkerWarnPct, DEFAULT_SETTINGS.guardWorkerWarnPct, '100 is out of the 1..99 range');
  assert.equal(bad.minWorkerPaneWidth, DEFAULT_SETTINGS.minWorkerPaneWidth, '19 is below the floor of 20');
  assert.equal(bad.maxWorkers, DEFAULT_SETTINGS.maxWorkers, '0 workers makes no sense');
  assert.equal(bad.defaultWorkerMachine, DEFAULT_SETTINGS.defaultWorkerMachine, 'unknown machine name');
});

// SPEC-V2 A: contextGuardAutostart existed in the file/shell defaults but had
// no UI control (Nachtrag item 1) — this is the read-side contract for it.
test('parseSettings takes a stored contextGuardAutostart and defaults to false', () => {
  assert.equal(DEFAULT_SETTINGS.contextGuardAutostart, false, 'off by default (alice 2026-07-27)');
  assert.equal(parseSettings(JSON.stringify({ contextGuardAutostart: true })).contextGuardAutostart, true);
  assert.equal(parseSettings(JSON.stringify({ contextGuardAutostart: 'true' })).contextGuardAutostart, false, 'a non-boolean falls back');
});

// Vertrag Teil 2 §5: on by default (opposite of contextGuardAutostart — the
// AUTOMATIC catalog fetch is opt-out, not opt-in), off only stops the
// automatic NETWORK path (settingsView.ts's kickOffDiscovery), never local sources.
test('parseSettings takes a stored modelDiscoveryAuto and defaults to true', () => {
  assert.equal(DEFAULT_SETTINGS.modelDiscoveryAuto, true, 'on by default (Vertrag Teil 2 §5)');
  assert.equal(parseSettings(JSON.stringify({ modelDiscoveryAuto: false })).modelDiscoveryAuto, false);
  assert.equal(parseSettings(JSON.stringify({ modelDiscoveryAuto: 'false' })).modelDiscoveryAuto, true, 'a non-boolean falls back to the default');
  assert.equal(parseSettings(undefined).modelDiscoveryAuto, true);
});

// SPEC-V3 D, Reviewer-Befund M6: workerModel is `string` now, any registered id.
test('parseSettings takes any plausible workerModel id, not just the five Claude ones', () => {
  assert.equal(parseSettings(JSON.stringify({ workerModel: 'gpt-5-codex' })).workerModel, 'gpt-5-codex');
  assert.equal(parseSettings(JSON.stringify({ workerModel: 'rm -rf /' })).workerModel, DEFAULT_SETTINGS.workerModel);
});

test('coerceSetting validates the guard/grid/machine keys', () => {
  assert.equal(coerceSetting('guardOrchWarnPct', 50), 50);
  assert.equal(coerceSetting('guardOrchWarnPct', 0), undefined);
  assert.equal(coerceSetting('guardOrchWarnPct', 100), undefined);
  assert.equal(coerceSetting('guardWorkerWarnPct', '77'), 77);
  assert.equal(coerceSetting('guardWorkerWarnPct', 'viel'), undefined);
  assert.equal(coerceSetting('minWorkerPaneWidth', 20), 20);
  assert.equal(coerceSetting('minWorkerPaneWidth', 19), undefined);
  assert.equal(coerceSetting('maxWorkers', 1), 1);
  assert.equal(coerceSetting('maxWorkers', 0), undefined);
  assert.equal(coerceSetting('maxWorkers', 2.7), 3);
  assert.equal(coerceSetting('defaultWorkerMachine', 'mac'), 'mac');
  assert.equal(coerceSetting('defaultWorkerMachine', 'peer'), 'peer');
  assert.equal(coerceSetting('defaultWorkerMachine', 'local'), 'local');
  assert.equal(coerceSetting('defaultWorkerMachine', 'moon'), undefined);
});

// Reviewer-Befund 7: Number(true) === 1 and Number([5]) === 5 would otherwise let a
// boolean or single-element array through as a valid percentage/width/count.
test('coerceSetting rejects non-numeric types for the four numeric guard/grid keys', () => {
  for (const key of ['guardOrchWarnPct', 'guardWorkerWarnPct', 'minWorkerPaneWidth', 'maxWorkers']) {
    assert.equal(coerceSetting(key, true), undefined, `${key}: boolean true must not become 1`);
    assert.equal(coerceSetting(key, false), undefined, `${key}: boolean false must not become 0`);
    assert.equal(coerceSetting(key, [50]), undefined, `${key}: a single-element array must not become its element`);
    assert.equal(coerceSetting(key, null), undefined, `${key}: null must not become 0`);
    assert.equal(coerceSetting(key, {}), undefined, `${key}: a plain object must be rejected`);
  }
});

// Reviewer-Befund 7, rest (round 3): Number(" 50") === 50 in JS, but the shell side's
// `^[0-9]+$` (context-guard's validate_pct, wb-grid/pi-worker's `[ -ge ]` guards) rejects
// a value with surrounding whitespace -- the UI and the guard used to disagree on the
// same stored value. Extension now matches the shell's strictness exactly.
test('coerceSetting rejects a numeric string with whitespace, matching shell strictness', () => {
  for (const key of ['guardOrchWarnPct', 'guardWorkerWarnPct', 'minWorkerPaneWidth', 'maxWorkers']) {
    assert.equal(coerceSetting(key, ' 50'), undefined, `${key}: leading whitespace must be rejected`);
    assert.equal(coerceSetting(key, '50 '), undefined, `${key}: trailing whitespace must be rejected`);
    assert.equal(coerceSetting(key, '+50'), undefined, `${key}: a leading sign must be rejected`);
  }
  assert.equal(coerceSetting('guardOrchWarnPct', '50'), 50, 'a clean digit string still works');
});

test('parseSettings rejects non-numeric types for the four numeric guard/grid keys (Befund 7)', () => {
  const settings = parseSettings(JSON.stringify({
    guardOrchWarnPct: true,
    guardWorkerWarnPct: [80],
    minWorkerPaneWidth: false,
    maxWorkers: [8],
  }));
  assert.equal(settings.guardOrchWarnPct, DEFAULT_SETTINGS.guardOrchWarnPct);
  assert.equal(settings.guardWorkerWarnPct, DEFAULT_SETTINGS.guardWorkerWarnPct);
  assert.equal(settings.minWorkerPaneWidth, DEFAULT_SETTINGS.minWorkerPaneWidth);
  assert.equal(settings.maxWorkers, DEFAULT_SETTINGS.maxWorkers);
});
