# Two machines

Optional. Everything works on one machine. This describes what changes when you have a second
one — say a laptop you sit at and a desktop with a bigger GPU — and want agents to use both.

The idea is not "remote control". It is that a job runs where it fits: big unified-memory work on
the Mac, CUDA work on the box with the NVIDIA card, and long independent jobs on whichever
machine is currently idle.

## Reaching the other machine

`ssh peer` has to work without a password prompt, or nothing below is usable. Two ways:

- **Tailscale SSH** — both machines join the same tailnet, SSH is handled by Tailscale, and no
  key management is left. This is the shipped assumption: `ssh peer` just works, from either side.
- **Plain SSH keys** — works fine, you maintain the keys. Put the private key somewhere the
  agent can read it and never let it near a git repository.

Add a `Host peer` entry to `~/.ssh/config` on both machines so the name is symmetric: from A,
`peer` is B; from B, `peer` is A. Every tool below uses that name and nothing else.

## Running work over there

```bash
check-resources                 # free VRAM/RAM, GPU processes, loaded models, PROTECTED list
check-resources --json          # the same for a script
run-on peer <command>           # run it there — but only if it fits
run-on peer --force <command>   # override, after you decided it is safe
```

`run-on` is the safe primitive. It reads `check-resources` **on the target** and refuses a job
that would displace a PROTECTED service (exit 3) or that obviously does not fit in free memory
(exit 4). It does not decide policy; it enforces the floor.

The policy sits in the agent roles and is deliberately conservative: check what is loaded, and if
something would have to be stopped to make room, **ask the user** — never stop it yourself. Then,
in order: run it on your own machine instead, or use a smaller model, or say plainly that it does
not fit. Killing someone else's inference job to free VRAM is the failure mode this rule exists
to prevent.

## Keeping state in sync

Two categories, two mechanisms, on purpose:

- **Code and notes**: git. The knowledge vault is a private repository; both machines clone it.
- **Secrets and machine-local state**: never git. Syncthing peer-to-peer between the two
  machines, so an API key never reaches a hosting provider. Folders: the vault's `90-secrets/`
  and a small `~/.secrets-sync/` holding API keys as mode-600 files.

`vault-sync` commits and pushes the vault; `status-freshness` tells you which project notes have
gone stale.

## Sessions on the other machine

Orchestrator sessions and their workers run in one tmux session, your own commands in another —
so that watching agents work and typing commands do not fight over the same window. The
workbench opens both in separate tabs and attaches a grouped view session, which is what makes
remote workers visible locally at all.

That last part matters more than it sounds: workers running where nobody can see them is a
failure, not a detail. If you change the layout, verify afterwards that the panes are actually
on screen.
