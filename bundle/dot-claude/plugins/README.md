# Plugins

Two small files here, and deliberately nothing else:

- `known_marketplaces.json` — the plugin marketplaces this setup pulls from.
- `installed_plugins.json` — which plugins were installed from them.

The marketplaces themselves are **not** shipped. They are git repositories owned by other
people. Vendoring them would put a stale copy of someone else's code under this repository's
license, and because they are repositories in their own right, git turns them into empty
directories in every clone. Claude Code clones them itself on first use.

To get the same set: start Claude Code, run `/plugin`, add the marketplaces listed in
`known_marketplaces.json`, then install the plugins listed in `installed_plugins.json`. Skip any
you do not want — nothing else in this setup depends on them.
