import assert from 'node:assert/strict';
import { test } from 'node:test';
import {
  allowedEfforts,
  BUILTIN_HARNESSES,
  BUILTIN_MODELS,
  BUILTIN_PROVIDERS,
  clampEffortForModel,
  effectiveHarnesses,
  effectiveModels,
  effectiveProviders,
  findHarness,
  findProvider,
  harnessesForProvider,
  isFreeModel,
  isReservedModelId,
  modelDataStaysLocal,
  modelsForRole,
  modelSupportsEffort,
  parseModelsRegistry,
  type Provider,
  providersForHarness,
  unknownPlaceholders,
  unknownPlaceholdersInHarness,
  type RegistryHarness,
  type RegistryModel,
} from '../src/models.ts';

function baseModel(overrides: Record<string, unknown> = {}): Record<string, unknown> {
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
  };
}

function baseHarness(overrides: Record<string, unknown> = {}): Record<string, unknown> {
  return {
    id: 'codex',
    label: 'Codex CLI (OpenAI)',
    command: 'codex',
    args: ['--model', '{model}'],
    ...overrides,
  };
}

test('parseModelsRegistry falls back to an empty registry on missing/broken input', () => {
  assert.deepEqual(parseModelsRegistry(undefined), { version: 1, providers: [], harnesses: [], models: [] });
  assert.deepEqual(parseModelsRegistry('not json'), { version: 1, providers: [], harnesses: [], models: [] });
  assert.deepEqual(parseModelsRegistry('[]'), { version: 1, providers: [], harnesses: [], models: [] });
  assert.deepEqual(parseModelsRegistry('{}'), { version: 1, providers: [], harnesses: [], models: [] });
});

test('parseModelsRegistry accepts a well-formed entry of each kind', () => {
  const registry = parseModelsRegistry(JSON.stringify({
    version: 1,
    providers: [{ id: 'openai', label: 'OpenAI', kind: 'cloud', apiKeyEnv: 'OPENAI_API_KEY' }],
    harnesses: [baseHarness()],
    models: [baseModel()],
  }));
  assert.equal(registry.providers.length, 1);
  assert.equal(registry.providers[0].id, 'openai');
  assert.equal(registry.harnesses.length, 1);
  assert.equal(registry.harnesses[0].command, 'codex');
  assert.equal(registry.models.length, 1);
  assert.equal(registry.models[0].id, 'gpt-5-codex');
});

test('parseModelsRegistry drops one broken entry without losing the rest of the file', () => {
  const registry = parseModelsRegistry(JSON.stringify({
    providers: [{ id: 'openai', label: 'OpenAI', kind: 'cloud' }, { id: 'broken' /* no label/kind */ }],
    harnesses: [baseHarness(), { id: 'no-command', label: 'x' }],
    models: [baseModel(), { id: 'incomplete' }],
  }));
  assert.equal(registry.providers.length, 1);
  assert.equal(registry.harnesses.length, 1);
  assert.equal(registry.models.length, 1);
});

test('parseModelsRegistry: a model colliding with a built-in alias is rejected', () => {
  const registry = parseModelsRegistry(JSON.stringify({
    models: [
      baseModel({ id: 'claude-sonnet-5' }),
      baseModel({ id: 'ornith' }),
      baseModel({ id: 'gpt-5-codex' }),
    ],
  }));
  assert.equal(registry.models.length, 1);
  assert.equal(registry.models[0].id, 'gpt-5-codex');
  assert.ok(isReservedModelId('claude-sonnet-5'));
  assert.ok(isReservedModelId('ornith'));
  assert.ok(!isReservedModelId('gpt-5-codex'));
});

test('parseModelsRegistry: maxEffort caps efforts, "max" is not a valid maxEffort', () => {
  const capped = parseModelsRegistry(JSON.stringify({
    models: [baseModel({ efforts: ['low', 'medium', 'high', 'xhigh'], maxEffort: 'medium', defaultEffort: 'xhigh' })],
  }));
  assert.deepEqual(capped.models[0].efforts, ['low', 'medium']);
  assert.equal(capped.models[0].defaultEffort, 'medium', 'requested default above the cap falls back to the top allowed');

  const badCeiling = parseModelsRegistry(JSON.stringify({
    models: [baseModel({ maxEffort: 'max' })],
  }));
  assert.equal(badCeiling.models.length, 0, 'max is above the policy ceiling and is not a valid maxEffort');

  const noMaxEffort = parseModelsRegistry(JSON.stringify({
    models: [baseModel({ maxEffort: undefined })],
  }));
  assert.equal(noMaxEffort.models.length, 0, 'maxEffort is required');
});

test('parseModelsRegistry: roles must be non-empty and drawn from worker/orchestrator', () => {
  const noRoles = parseModelsRegistry(JSON.stringify({ models: [baseModel({ roles: [] })] }));
  assert.equal(noRoles.models.length, 0);
  const badRoles = parseModelsRegistry(JSON.stringify({ models: [baseModel({ roles: ['admin'] })] }));
  assert.equal(badRoles.models.length, 0);
  const oneRole = parseModelsRegistry(JSON.stringify({ models: [baseModel({ roles: ['worker'] })] }));
  assert.deepEqual(oneRole.models[0].roles, ['worker']);
});

test('BUILTIN_MODELS matches today\'s Claude and pi lists exactly (fallback when models.json is absent)', () => {
  const ids = BUILTIN_MODELS.map((m) => m.id);
  assert.deepEqual(ids, [
    'claude-opus-5', 'claude-opus-4-8', 'claude-sonnet-5', 'claude-haiku-4-5', 'claude-fable-5',
    'ornith', 'qwen', 'ornith9',
  ]);
  const fable = BUILTIN_MODELS.find((m) => m.id === 'claude-fable-5')!;
  assert.deepEqual(fable.efforts, ['low', 'medium'], 'fable stays capped at medium (policy, CLAUDE.md)');
  assert.equal(fable.maxEffort, 'medium');
});

test('effectiveModels: an empty file registry yields exactly the built-ins', () => {
  const registry = parseModelsRegistry(undefined);
  assert.deepEqual(effectiveModels(registry).map((m) => m.id), BUILTIN_MODELS.map((m) => m.id));
});

test('effectiveModels: file models are appended after the built-ins', () => {
  const registry = parseModelsRegistry(JSON.stringify({ models: [baseModel()] }));
  const ids = effectiveModels(registry).map((m) => m.id);
  assert.equal(ids[ids.length - 1], 'gpt-5-codex');
  assert.equal(ids.length, BUILTIN_MODELS.length + 1);
});

test('modelsForRole: filters by role, harness, machine and enabled', () => {
  const registry = parseModelsRegistry(JSON.stringify({
    models: [
      baseModel({ id: 'worker-only', roles: ['worker'] }),
      baseModel({ id: 'orch-only', roles: ['orchestrator'] }),
      baseModel({ id: 'mac-only', roles: ['worker'], machines: ['mac'] }),
      baseModel({ id: 'disabled', roles: ['worker'], enabled: false }),
    ],
  }));
  const workers = modelsForRole(registry, { role: 'worker' }).map((m) => m.id);
  assert.ok(workers.includes('worker-only'));
  assert.ok(!workers.includes('orch-only'));
  assert.ok(!workers.includes('disabled'), 'disabled models are excluded by default');

  const peerWorkers = modelsForRole(registry, { role: 'worker', machine: 'peer' }).map((m) => m.id);
  assert.ok(!peerWorkers.includes('mac-only'), 'a mac-only model must not appear for peer');

  const withDisabled = modelsForRole(registry, { role: 'worker', enabledOnly: false }).map((m) => m.id);
  assert.ok(withDisabled.includes('disabled'));
});

test('allowedEfforts / clampEffortForModel respect the per-model maxEffort ceiling', () => {
  const model: RegistryModel = {
    id: 'x', label: 'X', harness: 'codex', provider: 'openai', modelRef: 'x',
    roles: ['worker'], efforts: ['low', 'medium', 'high'], maxEffort: 'medium', defaultEffort: 'medium',
    enabled: true,
  };
  assert.deepEqual(allowedEfforts(model), ['low', 'medium']);
  assert.equal(clampEffortForModel(model, 'high'), 'medium');
  assert.equal(clampEffortForModel(model, 'low'), 'low');
});

test('effectiveHarnesses: built-in claude/pi are always present and overridable by id', () => {
  const withoutFile = effectiveHarnesses(parseModelsRegistry(undefined));
  assert.deepEqual(withoutFile.map((h) => h.id).sort(), ['claude', 'pi']);

  const overridden = effectiveHarnesses(parseModelsRegistry(JSON.stringify({
    harnesses: [baseHarness({ id: 'claude', label: 'Claude Code (angepasst)', command: 'claude-custom' })],
  })));
  const claude = overridden.find((h) => h.id === 'claude')!;
  assert.equal(claude.command, 'claude-custom', 'a file entry overrides the built-in claude adapter');
  assert.equal(overridden.length, 2, 'pi is still present, unaffected');
});

test('unknownPlaceholders: only the documented placeholders are accepted', () => {
  assert.deepEqual(unknownPlaceholders('{model} {effort} {workdir} {sessionDir} {systemPromptFile} {name}'), []);
  assert.deepEqual(unknownPlaceholders('{secret:openrouter} {baseUrl:openrouter}'), []);
  assert.deepEqual(unknownPlaceholders('{secret:} {totallyUnknown}'), ['secret:', 'totallyUnknown']);
  assert.deepEqual(unknownPlaceholders('no placeholders here'), []);
});

test('unknownPlaceholdersInHarness scans args, env values and the effort/autonomy/resume args', () => {
  const harness: RegistryHarness = {
    id: 'x', label: 'X', command: 'x',
    args: ['--model', '{model}', '--bogus', '{nope}'],
    env: { OPENAI_API_KEY: '{secret:openai}', BAD: '{alsoNope}' },
    effort: { style: 'arg', args: ['{effort}', '{stillNope}'] },
    autonomy: { args: ['--dangerously-bypass-approvals'] },
    resume: { args: ['{sessionDir}'] },
  };
  const bad = unknownPlaceholdersInHarness(harness);
  assert.deepEqual([...bad].sort(), ['alsoNope', 'nope', 'stillNope'].sort());
});

test('BUILTIN_PROVIDERS: the ten preconfigured providers from SPEC-V3 A.1, all inactive (no key)', () => {
  const ids = BUILTIN_PROVIDERS.map((p) => p.id).sort();
  assert.deepEqual(ids, [
    'anthropic-api', 'deepseek', 'google', 'groq', 'llamacpp',
    'mistral', 'ollama', 'openai', 'openrouter', 'xai',
  ].sort());
  assert.equal(BUILTIN_PROVIDERS.find((p) => p.id === 'ollama')!.kind, 'local');
  assert.equal(BUILTIN_PROVIDERS.find((p) => p.id === 'openai')!.kind, 'cloud');
});

test('effectiveProviders: a file entry overrides a built-in provider by id, new ones are appended', () => {
  const registry = parseModelsRegistry(JSON.stringify({
    providers: [
      { id: 'openrouter', label: 'OpenRouter (eigene Base-URL)', kind: 'cloud', baseUrl: 'https://custom.example/v1' },
      { id: 'my-cloud', label: 'Meine Cloud', kind: 'cloud' },
    ],
  }));
  const providers = effectiveProviders(registry);
  assert.equal(providers.length, BUILTIN_PROVIDERS.length + 1, 'openrouter overrides, my-cloud is new');
  assert.equal(findProvider(registry, 'openrouter')!.baseUrl, 'https://custom.example/v1');
  assert.equal(findProvider(registry, 'my-cloud')!.label, 'Meine Cloud');
  assert.equal(findProvider(registry, 'openai')!.label, 'OpenAI', 'untouched built-in survives');
  assert.equal(findProvider(registry, 'does-not-exist'), undefined);
});

test('unknownPlaceholdersInHarness: a clean harness template has no findings', () => {
  const harness: RegistryHarness = {
    id: 'codex', label: 'Codex', command: 'codex',
    args: ['--model', '{model}'],
    env: { OPENAI_API_KEY: '{secret:openai}' },
    effort: { style: 'arg', args: ['-c', 'model_reasoning_effort={effort}'] },
  };
  assert.deepEqual(unknownPlaceholdersInHarness(harness), []);
});

// SPEC-V3 D item 5: promptPattern/promptIgnore were validated fields on the real
// shell side but silently dropped by the extension's parser.
test('parseModelsRegistry keeps a harness\'s promptPattern and promptIgnore', () => {
  const registry = parseModelsRegistry(JSON.stringify({
    harnesses: [{
      id: 'codex', label: 'Codex CLI', command: 'codex',
      promptPattern: '^›', promptIgnore: '^› Run /review',
    }],
  }));
  const codex = findHarness(registry, 'codex')!;
  assert.equal(codex.promptPattern, '^›');
  assert.equal(codex.promptIgnore, '^› Run /review');
});

// SPEC-V3 D item 3: supportsEffort/dataStaysLocal were validated by wb-state but
// not parsed here, so the extension could never know a model's real answer.
test('parseModelsRegistry keeps a model\'s supportsEffort and dataStaysLocal, and its alias', () => {
  const registry = parseModelsRegistry(JSON.stringify({
    models: [{
      id: 'ornith-35b', label: 'Ornith 35B', alias: 'ornith', harness: 'pi', provider: 'ollama',
      modelRef: 'ornith:35b', roles: ['worker'], efforts: ['medium'], maxEffort: 'medium',
      defaultEffort: 'medium', enabled: true, supportsEffort: true, dataStaysLocal: true,
    }],
  }));
  const model = registry.models[0];
  assert.equal(model.alias, 'ornith');
  assert.equal(model.supportsEffort, true);
  assert.equal(model.dataStaysLocal, true);
});

const CLOUD_PROVIDER: Provider = { id: 'openai', label: 'OpenAI', kind: 'cloud' };
const LOCAL_PROVIDER: Provider = { id: 'ollama', label: 'Ollama', kind: 'local' };
const SUBSCRIPTION_PROVIDER: Provider = { id: 'chatgpt', label: 'ChatGPT', kind: 'subscription' };

function effortTestModel(overrides: Partial<RegistryModel> = {}): RegistryModel {
  return {
    id: 'm', label: 'M', harness: 'codex', provider: 'openai', modelRef: 'm',
    roles: ['worker'], efforts: ['medium'], maxEffort: 'medium', defaultEffort: 'medium', enabled: true,
    ...overrides,
  };
}

test('modelDataStaysLocal: explicit model field wins, then provider field, then provider.kind', () => {
  assert.equal(modelDataStaysLocal(effortTestModel(), CLOUD_PROVIDER), false);
  assert.equal(modelDataStaysLocal(effortTestModel(), LOCAL_PROVIDER), true);
  assert.equal(modelDataStaysLocal(effortTestModel({ dataStaysLocal: true }), CLOUD_PROVIDER), true, 'model field overrides a cloud provider');
  assert.equal(
    modelDataStaysLocal(effortTestModel(), { ...CLOUD_PROVIDER, dataStaysLocal: true }),
    true,
    'provider field wins over provider.kind when the model itself says nothing',
  );
  assert.equal(modelDataStaysLocal(effortTestModel(), SUBSCRIPTION_PROVIDER), false, 'subscription is never local by default');
});

test('modelSupportsEffort: harness style gates first, then the model field, then data-locality', () => {
  const argHarness: RegistryHarness = { id: 'codex', label: 'Codex', command: 'codex', effort: { style: 'arg' } };
  const noneHarness: RegistryHarness = { id: 'opencode', label: 'opencode', command: 'opencode', effort: { style: 'none' } };
  assert.equal(modelSupportsEffort(effortTestModel(), noneHarness, CLOUD_PROVIDER), false, 'harness with no effort mechanism always wins');
  assert.equal(modelSupportsEffort(effortTestModel(), argHarness, CLOUD_PROVIDER), true, 'cloud + no explicit field: not local, so supported');
  assert.equal(modelSupportsEffort(effortTestModel(), argHarness, LOCAL_PROVIDER), false, 'local + no explicit field: not supported by default');
  assert.equal(
    modelSupportsEffort(effortTestModel({ supportsEffort: true }), argHarness, LOCAL_PROVIDER),
    true,
    'explicit field overrides the local-provider default (real case: pi --thinking to ornith)',
  );
  assert.equal(modelSupportsEffort(effortTestModel(), undefined, CLOUD_PROVIDER), false, 'no registered harness: nothing to send effort through');
});

test('BUILTIN_HARNESSES: claude and pi both carry a real readyPattern/promptPattern/effort (not "nicht gemessen")', () => {
  for (const id of ['claude', 'pi']) {
    const harness = BUILTIN_HARNESSES.find((h) => h.id === id)!;
    assert.ok(harness.readyPattern, `${id}: readyPattern must be populated`);
    assert.ok(harness.promptPattern, `${id}: promptPattern must be populated`);
    assert.equal(harness.effort?.style, 'arg', `${id}: both take an effort flag`);
  }
});

test('BUILTIN_MODELS: the five Claude models resolve to the real claude-subscription provider id, pi models to ollama', () => {
  for (const model of BUILTIN_MODELS) {
    if (model.harness === 'claude') {
      assert.equal(model.provider, 'claude-subscription');
    }
    if (model.harness === 'pi') {
      assert.equal(model.provider, 'ollama');
      assert.equal(model.supportsEffort, true, 'pi forwards --thinking even to a local model');
    }
  }
});

test('harnessesForProvider: finds harnesses via a registered model, empty when none reference it', () => {
  const registry = parseModelsRegistry(JSON.stringify({
    providers: [{ id: 'chatgpt', label: 'ChatGPT', kind: 'subscription' }],
    harnesses: [{ id: 'codex', label: 'Codex CLI', command: 'codex' }],
    models: [{
      id: 'gpt-5-codex', label: 'GPT-5 Codex', harness: 'codex', provider: 'chatgpt', modelRef: 'gpt-5-codex',
      roles: ['worker'], efforts: ['medium'], maxEffort: 'medium', defaultEffort: 'medium', enabled: true,
    }],
  }));
  assert.deepEqual(harnessesForProvider(registry, 'chatgpt').map((h) => h.id), ['codex']);
  assert.deepEqual(harnessesForProvider(registry, 'antigravity'), [], 'no model references it yet');
});

// Vertrag discover.md §2/§3 (Spur B, 2026-07-29): the `discover` block on a
// harness, and the `source`/`discoveredAt` fields on a model it produces.
test('validateHarness: a well-formed discover block round-trips every field', () => {
  const registry = parseModelsRegistry(JSON.stringify({
    harnesses: [baseHarness({
      discover: {
        source: 'file-json',
        file: '~/.codex/models_cache.json',
        jsonPath: 'models[].slug',
        filter: { field: 'visibility', equals: 'list' },
        refTemplate: 'ollama/{name}',
        provider: 'antigravity',
        providerByPrefix: [['ollama/', 'ollama']],
        idPrefix: 'agy-',
        roles: ['worker', 'orchestrator'],
        supportsEffort: false,
        ttlHours: 24,
      },
    })],
  }));
  const discover = registry.harnesses[0].discover;
  assert.ok(discover, 'discover block must survive parsing');
  assert.equal(discover!.source, 'file-json');
  assert.equal(discover!.file, '~/.codex/models_cache.json');
  assert.equal(discover!.jsonPath, 'models[].slug');
  assert.deepEqual(discover!.filter, { field: 'visibility', equals: 'list' });
  assert.equal(discover!.refTemplate, 'ollama/{name}');
  assert.equal(discover!.provider, 'antigravity');
  assert.deepEqual(discover!.providerByPrefix, [['ollama/', 'ollama']]);
  assert.equal(discover!.idPrefix, 'agy-');
  assert.deepEqual(discover!.roles, ['worker', 'orchestrator']);
  assert.equal(discover!.supportsEffort, false);
  assert.equal(discover!.ttlHours, 24);
});

test('validateHarness: command-lines / ollama discover sources with minimal fields', () => {
  const registry = parseModelsRegistry(JSON.stringify({
    harnesses: [
      baseHarness({ id: 'agy', discover: { source: 'command-lines', command: ['agy', 'models'] } }),
      baseHarness({ id: 'pi', discover: { source: 'ollama' } }),
    ],
  }));
  const agy = findHarness(registry, 'agy')!;
  assert.equal(agy.discover?.source, 'command-lines');
  assert.deepEqual(agy.discover?.command, ['agy', 'models']);
  const pi = findHarness(registry, 'pi')!;
  assert.equal(pi.discover?.source, 'ollama');
  assert.equal(pi.discover?.command, undefined);
});

test('validateHarness: an unknown discover.source drops the block, keeps the harness', () => {
  const registry = parseModelsRegistry(JSON.stringify({
    harnesses: [baseHarness({ discover: { source: 'ftp', file: 'x' } })],
  }));
  assert.equal(registry.harnesses.length, 1, 'the harness itself must survive an unrecognised discover.source');
  assert.equal(registry.harnesses[0].discover, undefined);
});

test('validateHarness: a discover block with no source at all is dropped, not a crash', () => {
  const registry = parseModelsRegistry(JSON.stringify({
    harnesses: [baseHarness({ discover: { file: 'x' } })],
  }));
  assert.equal(registry.harnesses[0].discover, undefined);
  const nonObject = parseModelsRegistry(JSON.stringify({ harnesses: [baseHarness({ discover: 'nonsense' })] }));
  assert.equal(nonObject.harnesses[0].discover, undefined);
});

test('validateHarness: discover.filter needs both field and equals, else dropped', () => {
  const missingEquals = parseModelsRegistry(JSON.stringify({
    harnesses: [baseHarness({ discover: { source: 'ollama', filter: { field: 'visibility' } } })],
  }));
  assert.equal(missingEquals.harnesses[0].discover?.filter, undefined);
  assert.equal(missingEquals.harnesses[0].discover?.source, 'ollama', 'the rest of the block survives');
});

test('validateModel: source/discoveredAt round-trip for an auto-discovered entry', () => {
  const registry = parseModelsRegistry(JSON.stringify({
    models: [baseModel({ source: 'auto', discoveredAt: '2026-07-29T01:00:00Z' })],
  }));
  assert.equal(registry.models[0].source, 'auto');
  assert.equal(registry.models[0].discoveredAt, '2026-07-29T01:00:00Z');
});

test('validateModel: an unknown source value is dropped, never crashes the entry', () => {
  const registry = parseModelsRegistry(JSON.stringify({
    models: [baseModel({ source: 'bogus' })],
  }));
  assert.equal(registry.models.length, 1, 'the model itself still parses');
  assert.equal(registry.models[0].source, undefined);
});

test('validateModel: a discovered model with no effort concept (opencode: effort.style "none") '
  + 'omits maxEffort/efforts/defaultEffort entirely and still parses (Vertrag §3)', () => {
  const registry = parseModelsRegistry(JSON.stringify({
    models: [{
      id: 'opencode-gpt-5', label: 'GPT-5 (opencode)', harness: 'opencode', provider: 'openai',
      modelRef: 'gpt-5', roles: ['worker', 'orchestrator'], enabled: true,
      supportsEffort: false, source: 'auto', discoveredAt: '2026-07-29T01:00:00Z',
      // maxEffort/efforts/defaultEffort deliberately absent
    }],
  }));
  assert.equal(registry.models.length, 1, 'missing maxEffort must not drop the whole entry when supportsEffort is false');
  const model = registry.models[0];
  assert.equal(model.supportsEffort, false);
  assert.ok(model.maxEffort, 'a synthetic ceiling is filled in — never read anywhere supportsEffort is false');
  assert.equal(model.source, 'auto');
});

test('validateModel: without an explicit supportsEffort:false, a missing maxEffort is still rejected (regression guard)', () => {
  const registry = parseModelsRegistry(JSON.stringify({
    models: [{
      id: 'no-max-effort', label: 'X', harness: 'codex', provider: 'openai', modelRef: 'x',
      roles: ['worker'], enabled: true,
      // no maxEffort, no supportsEffort — must still be rejected as before
    }],
  }));
  assert.equal(registry.models.length, 0);
});

// Vertrag Teil 2 (Spur B, 2026-07-29): catalogs, keys at the harness, balance.
test('isFreeModel: cost 0/0 is free, any nonzero side is paid, no cost at all is unknown (not free)', () => {
  const free = { cost: { inPerMTok: 0, outPerMTok: 0 } } as RegistryModel;
  assert.equal(isFreeModel(free), true);
  const freeOneSideImplicit = { cost: { inPerMTok: 0 } } as RegistryModel;
  assert.equal(isFreeModel(freeOneSideImplicit), true, 'validateCost guarantees the other side is a real number whenever cost is present, so ?? 0 only fills a genuinely absent side');
  const paid = { cost: { inPerMTok: 2, outPerMTok: 8 } } as RegistryModel;
  assert.equal(isFreeModel(paid), false);
  const paidOneSide = { cost: { inPerMTok: 0, outPerMTok: 1.5 } } as RegistryModel;
  assert.equal(isFreeModel(paidOneSide), false);
  const noCost = { } as RegistryModel;
  assert.equal(isFreeModel(noCost), false, 'unknown must not default to free — that would misclassify an unpriced paid model');
});

test('providersForHarness: only cloud providers a harness actually runs models from, never local/subscription', () => {
  const registry = parseModelsRegistry(JSON.stringify({
    providers: [
      { id: 'openrouter', label: 'OpenRouter', kind: 'cloud' },
      { id: 'chatgpt', label: 'ChatGPT', kind: 'subscription' },
    ],
    harnesses: [baseHarness({ id: 'aider' })],
    models: [
      baseModel({ id: 'aider-openrouter-x', harness: 'aider', provider: 'openrouter' }),
      baseModel({ id: 'aider-ollama-x', harness: 'aider', provider: 'ollama' }),
    ],
  }));
  const providers = providersForHarness(registry, 'aider').map((p) => p.id);
  assert.deepEqual(providers, ['openrouter'], 'ollama is local, must not show up as a key-needing provider');
  assert.deepEqual(providersForHarness(registry, 'pi'), [], 'pi only ever runs ollama models — no cloud provider, no block');
});

test('validateProvider: catalogStatus/balanceStatus round-trip, including amount:null (unbegrenzt)', () => {
  const registry = parseModelsRegistry(JSON.stringify({
    providers: [{
      id: 'openrouter', label: 'OpenRouter', kind: 'cloud',
      catalogStatus: { count: 367, fetchedAt: '2026-07-29T10:22:00Z' },
      balanceStatus: { amount: 4.12, currency: '$', fetchedAt: '2026-07-29T10:22:00Z' },
    }],
  }));
  const p = registry.providers[0];
  assert.deepEqual(p.catalogStatus, { count: 367, fetchedAt: '2026-07-29T10:22:00Z' });
  assert.deepEqual(p.balanceStatus, { amount: 4.12, currency: '$', fetchedAt: '2026-07-29T10:22:00Z' });

  const unlimited = parseModelsRegistry(JSON.stringify({
    providers: [{ id: 'x', label: 'X', kind: 'cloud', balanceStatus: { amount: null } }],
  }));
  assert.deepEqual(unlimited.providers[0].balanceStatus, { amount: null, currency: undefined, fetchedAt: undefined });
});

test('validateProvider: a malformed catalogStatus/balanceStatus is dropped, the provider still parses', () => {
  const registry = parseModelsRegistry(JSON.stringify({
    providers: [{
      id: 'openrouter', label: 'OpenRouter', kind: 'cloud',
      catalogStatus: { count: 'not-a-number' },
      balanceStatus: { amount: 'four dollars' },
    }],
  }));
  assert.equal(registry.providers.length, 1, 'the provider itself must survive malformed status sub-objects');
  assert.equal(registry.providers[0].catalogStatus, undefined);
  assert.equal(registry.providers[0].balanceStatus, undefined);
});
