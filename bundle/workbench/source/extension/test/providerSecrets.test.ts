// Writes real files (the secrets-sync side only), so this file redirects HOME
// into a temp directory — same pattern as settingsWrite.test.ts. The Keychain
// side (`security -i`) is NEVER invoked from a test: only its pure script
// builder (keychainAddScript) is checked here.
import assert from 'node:assert/strict';
import { chmodSync, mkdtempSync, writeFileSync } from 'node:fs';
import { readFile, stat } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { test } from 'node:test';
import {
  keychainAddScript,
  keychainService,
  providerKeyStatus,
  sanitizeApiKey,
  secretsSyncFile,
  writeSecretsSyncFile,
} from '../src/providerSecrets.ts';

test('keychainService prefixes the provider id (SPEC-V3 A.1 example: wb-openrouter)', () => {
  assert.equal(keychainService('openrouter'), 'wb-openrouter');
});

test('secretsSyncFile: ~/.secrets-sync/api-keys/<provider>', () => {
  assert.equal(secretsSyncFile('openrouter', '/Users/alice'), '/Users/alice/.secrets-sync/api-keys/openrouter');
});

test('sanitizeApiKey trims, rejects empty and rejects embedded newlines', () => {
  assert.equal(sanitizeApiKey('  sk-abc123  '), 'sk-abc123');
  assert.equal(sanitizeApiKey(''), undefined);
  assert.equal(sanitizeApiKey('   '), undefined);
  assert.equal(sanitizeApiKey('sk-abc\n123'), undefined);
  assert.equal(sanitizeApiKey('sk-abc\r\n123'), undefined);
});

test('keychainAddScript never needs the secret to appear as a bare unquoted token', () => {
  const script = keychainAddScript('wb-openrouter', 'openrouter', 'sk-super-secret');
  assert.match(script, /^add-generic-password /);
  assert.match(script, /-a "openrouter"/);
  assert.match(script, /-s "wb-openrouter"/);
  assert.match(script, /-w "sk-super-secret"/);
  assert.match(script, / -U\n$/);
});

test('keychainAddScript escapes embedded quotes and backslashes in the secret', () => {
  const script = keychainAddScript('wb-x', 'x', 'sk-"weird"\\value');
  assert.match(script, /-w "sk-\\"weird\\"\\\\value"/);
  // the script stays a single well-formed line the security -i parser can read
  assert.equal(script.split('\n').filter((l) => l.length > 0).length, 1);
});

test('keychainAddScript never leaks the secret through the account/service quoting boundary', () => {
  // a secret containing a quote must not be able to close the -w string early
  // and inject a bogus extra flag into the command line.
  const script = keychainAddScript('wb-x', 'x', '" -s evil -w pwned');
  const wMatch = /-w "([^]*)" -U\n$/.exec(script);
  assert.ok(wMatch, 'the -w value must still be a single well-formed quoted token');
});

const home = mkdtempSync(join(tmpdir(), 'wb-secrets-'));
process.env.HOME = home;
assert.ok(secretsSyncFile('x').startsWith(home), `HOME redirect failed: ${secretsSyncFile('x')}`);

test('writeSecretsSyncFile writes mode 600 and providerKeyStatus reports it without reading the value', async () => {
  const before = await providerKeyStatus({ id: 'openrouter', kind: 'cloud' });
  assert.equal(before.present, false);

  await writeSecretsSyncFile('openrouter', 'sk-abc123');
  const status = await providerKeyStatus({ id: 'openrouter', kind: 'cloud' });
  assert.equal(status.present, true);
  assert.ok(status.setAt, 'setAt must be populated once the file exists');
  assert.ok(!Number.isNaN(Date.parse(status.setAt!)));

  const info = await stat(secretsSyncFile('openrouter'));
  assert.equal(info.mode & 0o777, 0o600, 'the key file must be mode 600');

  // sanity: the file really holds the key (this test file may read it — the UI
  // code path never does), so a broken write would not slip through.
  assert.equal((await readFile(secretsSyncFile('openrouter'), 'utf8')).trim(), 'sk-abc123');
});

test('writeSecretsSyncFile overwrites on a second save (re-entering a key updates it)', async () => {
  await writeSecretsSyncFile('reentry', 'sk-first');
  await writeSecretsSyncFile('reentry', 'sk-second');
  assert.equal((await readFile(secretsSyncFile('reentry'), 'utf8')).trim(), 'sk-second');
});

// Reviewer-Befund M2 (2026-07-28): providerKeyStatus used to stat() only the
// secrets-sync file, so a key that only lives in the macOS Keychain (the
// PRIMARY location on this machine, SPEC-V3 A.1) showed as "nicht hinterlegt".
// A fake `security` binary stands in for the real Keychain here — never call
// the real one from a test.
const fakeSecurityBin = mkdtempSync(join(tmpdir(), 'wb-secrets-security-bin-'));
writeFileSync(
  join(fakeSecurityBin, 'security'),
  '#!/bin/sh\n'
    + '# Fake "security" for providerKeyStatus tests — never touches the real Keychain.\n'
    + 'if [ "$1" = "find-generic-password" ] && [ "$2" = "-s" ] && [ "$3" = "wb-keychain-provider" ]; then\n'
    + '  exit 0\n'
    + 'fi\n'
    + 'exit 44\n',
  { mode: 0o755 },
);
chmodSync(join(fakeSecurityBin, 'security'), 0o755);
process.env.PATH = `${fakeSecurityBin}:${process.env.PATH ?? ''}`;

test('providerKeyStatus checks the Keychain first (SPEC-V3 A.1 order: Keychain -> file -> env)', async () => {
  const status = await providerKeyStatus({ id: 'keychain-provider', kind: 'cloud', keychainService: 'wb-keychain-provider' });
  assert.equal(status.present, true);
  assert.equal(status.setAt, undefined, 'Keychain presence carries no date, unlike the file source');
});

test('providerKeyStatus falls through to the file when the Keychain has nothing for this service', async () => {
  const missingKeychain = await providerKeyStatus({ id: 'no-keychain-hit', kind: 'cloud', keychainService: 'wb-does-not-exist' });
  assert.equal(missingKeychain.present, false);

  await writeSecretsSyncFile('no-keychain-hit', 'sk-file-only');
  const status = await providerKeyStatus({ id: 'no-keychain-hit', kind: 'cloud', keychainService: 'wb-does-not-exist' });
  assert.equal(status.present, true);
  assert.ok(status.setAt, 'file source still reports its mtime');
});

test('providerKeyStatus falls through to the env var when neither Keychain nor file has it', async () => {
  process.env.WB_TEST_PROVIDER_KEY = 'sk-env-only';
  const status = await providerKeyStatus({ id: 'env-only-provider', kind: 'cloud', apiKeyEnv: 'WB_TEST_PROVIDER_KEY' });
  assert.equal(status.present, true);
  assert.equal(status.setAt, undefined, 'env presence carries no date either');
  delete process.env.WB_TEST_PROVIDER_KEY;
});

test('providerKeyStatus: the Keychain wins even when a file also exists for the same provider', async () => {
  await writeSecretsSyncFile('keychain-provider', 'sk-file-too');
  const status = await providerKeyStatus({ id: 'keychain-provider', kind: 'cloud', keychainService: 'wb-keychain-provider' });
  assert.equal(status.present, true);
  assert.equal(status.setAt, undefined, 'Keychain resolves first, so the file mtime never gets consulted');
});

// SPEC-V3 D item 4: a subscription provider (codex/agy: login via account, not
// a key) checks a configured loginCheckPath instead — never Keychain/file/env.
test('providerKeyStatus: subscription with no loginCheckPath reports "not checked", not "absent"', async () => {
  const status = await providerKeyStatus({ id: 'chatgpt', kind: 'subscription' });
  assert.equal(status.present, false);
  assert.equal(status.checked, false, 'nothing was actually checked — the UI must not read this as "fehlt"');
});

test('providerKeyStatus: subscription WITH a loginCheckPath checks that file, not Keychain/secrets-sync/env', async () => {
  const missing = await providerKeyStatus({ id: 'chatgpt', kind: 'subscription', loginCheckPath: join(home, 'no-such-auth.json') });
  assert.equal(missing.present, false);
  assert.notEqual(missing.checked, false, 'a configured path WAS actually checked');

  const authPath = join(home, 'fake-auth.json');
  writeFileSync(authPath, '{}');
  const present = await providerKeyStatus({ id: 'chatgpt', kind: 'subscription', loginCheckPath: authPath });
  assert.equal(present.present, true);
  assert.ok(present.setAt, 'the login file\'s mtime is reported, same shape as the cloud-key file source');
});
