# Source of the VS Code workbench

The installer uses the packaged `claude-workbench-0.1.0.vsix` one directory up. This folder is
the source it was built from, so the extension can be read, changed and rebuilt rather than only
installed.

```
extension/     the VS Code extension (TypeScript, no framework, no bundled runtime deps)
shell-tests/   the test suites for the wb-* shell tools in ../../bin
```

## Build it

```bash
cd extension
npm install
npm run check      # typecheck + 319 tests + build
npm run package    # produces the .vsix
code --install-extension claude-workbench-0.1.0.vsix --force
```

`npm run check` is the honest gate: it typechecks, runs the whole suite and builds. If it is
green, the extension loads.

## Test the shell tools

The `wb-*` tools in `bundle/bin/` have their own suites:

```bash
bash shell-tests/test-registry.sh          # 126 cases: registry, resolve, caps, validation
bash shell-tests/test-models-discover.sh   #  45 cases: local model discovery
bash shell-tests/test-model-catalog.sh     #  58 cases: provider catalogs, balance, key safety
```

Every case builds its own throwaway fixture. None of them touches your real configuration, your
keychain or a running tmux session — a test that needs the live environment is a broken test.
The catalog suite runs against a local stub HTTP server, never a real provider, and it greps
every file and log of the run for its fixture key to prove the key did not leak.

## What differs from the machine this came from

The setup this was extracted from uses a private name for its second machine; here it is simply `peer`
throughout, and the example user in fixtures is `alice`. The launchd labels use the
`agent-workbench.` prefix. Functionally identical, just without one particular machine's
vocabulary baked in.
