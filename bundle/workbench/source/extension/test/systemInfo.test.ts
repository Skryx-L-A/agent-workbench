import assert from 'node:assert/strict';
import { mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { test } from 'node:test';
import {
  collectLaunchdJobs,
  collectPathInfo,
  LAUNCHD_LABELS,
  namedPaths,
  parseLaunchctlList,
} from '../src/systemInfo.ts';

test('namedPaths: the three paths CLAUDE.md names, under the given home', () => {
  const paths = namedPaths('/Users/alice');
  assert.deepEqual(paths.map((p) => p.path), [
    '/Users/alice/Knowledge',
    '/Users/alice/.pi-workers',
    '/Users/alice/.local/trash-snapshots',
  ]);
});

test('parseLaunchctlList reads PID/LastExitStatus/Label, "-" becomes undefined', () => {
  const map = parseLaunchctlList([
    '1271\t0\tagent-workbench.mcp-playwright',
    '-\t0\tagent-workbench.mcp-reaper',
    'not a real line',
    '',
  ].join('\n'));
  assert.deepEqual(map.get('agent-workbench.mcp-playwright'), { pid: 1271, lastExit: 0 });
  assert.deepEqual(map.get('agent-workbench.mcp-reaper'), { pid: undefined, lastExit: 0 });
  assert.equal(map.get('agent-workbench.does-not-exist'), undefined);
});

test('collectPathInfo against a real throwaway directory: exists, has a size, never throws for a missing one', async () => {
  const home = mkdtempSync(join(tmpdir(), 'wb-systeminfo-'));
  writeFileSync(join(home, 'placeholder'), 'x'); // just so Knowledge/ has SOME content if created below
  // Only create one of the three named dirs — the other two must report "fehlt", not throw.
  const fs = await import('node:fs/promises');
  await fs.mkdir(join(home, 'Knowledge'), { recursive: true });
  await fs.writeFile(join(home, 'Knowledge', 'note.md'), 'hello');

  const info = await collectPathInfo(home);
  assert.equal(info.length, 3);
  const vault = info.find((p) => p.label === 'Vault')!;
  assert.equal(vault.exists, true);
  assert.ok(vault.sizeHuman, 'du -sh must report a size for an existing directory');
  const workers = info.find((p) => p.label === 'Worker-Ergebnisse')!;
  assert.equal(workers.exists, false);
  assert.equal(workers.sizeHuman, undefined);
});

test('collectLaunchdJobs against the real launchctl: well-formed for all five named jobs, never throws', async () => {
  const jobs = await collectLaunchdJobs();
  assert.equal(jobs.length, LAUNCHD_LABELS.length);
  for (const job of jobs) {
    assert.ok(LAUNCHD_LABELS.includes(job.label));
    assert.equal(typeof job.loaded, 'boolean');
  }
});
