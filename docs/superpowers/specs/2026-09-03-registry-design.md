# Registry — every checkout this machine has built a graph for

Decided with Max 2026-09-03 after the GitNexus replacement analysis
(`2026-09-03-replace-gitnexus.md`). Ships as repograph 0.5.0, after the 0.4.0 release
`beauty-crm` is waiting on.

## Why

GitNexus kept `~/.gitnexus/registry.json` so an MCP server and a hook could find a repository by
name. Its rows were written by hand and never checked: today it lists eleven repositories, of
which three exist as code, two share the name `bonliva-erp`, and six are empty fix-clones with
six markdown files each. The rows nobody used rotted.

repograph needs the same three things — address a repository by name from anywhere, list what is
indexed and how fresh it is, build a worktree without paying the ~100 s embedding cost again —
without a file anyone maintains.

## Principle

**The registry is a cache of facts git already knows.** Nothing is registered by hand: every
command that opens a store records the checkout it ran in, and every write drops checkouts whose
path is gone. A row is either confirmed by use or removed. There is no `register`, no
`unregister`, no import from GitNexus.

## What is stored

`~/.config/repograph/registry.json` (override: `REPOGRAPH_REGISTRY`; same convention as
`FASTEMBED_CACHE_DIR`), written atomically through a temp file and a rename.

```json
{
  "projects": {
    "bc97b44ef15e1ed3fb1c482f9112865eca6e21ff": {
      "name": "beauty-crm",
      "checkouts": [
        { "path": "/Users/max/Documents/projects/beauty-crm", "branch": "main", "main": true, "last_used": 1756900000 },
        { "path": "/Users/max/Documents/projects/beauty-crm/.claude/worktrees/tasks-coordination", "branch": "worktree-tasks-coordination", "main": false, "last_used": 1756890000 }
      ]
    }
  }
}
```

- **Project key** — the history's root commits (`git rev-list --max-parents=0 HEAD`), sorted and
  joined with `+`. A clone, a linked worktree, a nested `.claude/worktrees/x` and a directory that
  was moved all share it; `bonliva-erp` has three roots and both its checkouts list the same
  three. A directory that is not a git checkout keys on `path:<canonical path>`.
- **Name** — the last segment of `origin` (`git@github-bonliva:Bonliva/bonliva-erp.git` →
  `bonliva-erp`), else the directory's basename. A second, different project arriving with a
  name already taken gets `<name>-<first 4 hex of its key>`. A name, once assigned, does not
  change.
- **Checkout** — absolute path, branch, whether it is the main checkout (`--git-dir` equals
  `--git-common-dir`), and when it was last used.

## Hot path stays hot

`build` and `update` run git once and cache `{key, name, main}` in `.repograph/identity.json`.
Every other command reads that file, reads the branch from `.git/HEAD` (following a linked
worktree's `gitdir:` pointer), and loads the registry — three small files, no subprocess — and
writes the registry only when this checkout's `last_used` is older than ten minutes. A store
built by an older release has no identity file; such a command skips the registry until the
next `update`. A registry that cannot be read or written prints one line on stderr and never
fails the command.

## Addressing

`--repo <value>`: an existing directory is a path, as today. Otherwise `value` is `name` or
`name@branch`. `name` picks, in order: the checkout whose path contains the current directory;
the main checkout; the most recently used. `name@branch` picks the checkout on that branch and,
when there is none, fails listing the branches there are. An unknown name fails listing the
names there are.

## Seeding

A `build` (or an `update` in a directory with no store) looks for another checkout of the same
project whose store has the newest `manifest.json`, copies its seven files (`graph.json`,
`graph.bin`, `manifest.json`, `questions.json`, `questions.bin`, `vectors.f32`, `vectors.json`)
and then runs the ordinary update: the manifest diff re-reads only files whose blake3 differs,
the dense index re-embeds only rows whose `(id, text hash)` changed. A worktree on a branch a
few commits away costs seconds instead of the ~100 s of embedding. `--no-seed` skips the copy.
The command prints `seeded from <path>` so a surprising node count has an explanation.

## Nested checkouts

The walker skips any directory below the root that contains a `.git` entry — file or directory.
A worktree placed in a visible subdirectory, a vendored repository, a fix-clone dropped inside
the tree: none is indexed twice. Hidden directories (`.claude/worktrees`) were already skipped.

## `repos`

```
$ repograph repos
beauty-crm  bc97b44e
  /Users/max/Documents/projects/beauty-crm  main  main  8306 nodes / 29610 edges  fresh
  /Users/max/Documents/projects/beauty-crm/.claude/worktrees/tasks-coordination  worktree-tasks-coordination  8291 nodes / 29540 edges  stale: 3 changed, 1 removed
bonliva-erp  1d2a6306
  /Users/max/Documents/projects/bonliva-erp  main  main  no store
```

Counts come from the store as it stands; freshness is the same ~10 ms manifest diff `ask` runs.
`--json` prints the registry object with the two computed fields added per checkout, which is
what a global hook reads to map a working directory to a checkout by longest path prefix.

## Out of scope

Importing `~/.gitnexus/registry.json`; `watch --all`; matching shallow clones by remote URL
(their root commits differ — they register as a second project, named with a suffix, and that is
visible in `repos`); any MCP server.

## Acceptance

- `repograph build` in `beauty-crm`, then `repograph --repo beauty-crm ask FR-PAY-22` from `/tmp`
  answers; `repograph --repo beauty-crm@worktree-tasks-coordination verify` answers from the
  worktree's store.
- `repograph build` in a fresh `git worktree add` of `beauty-crm` prints `seeded from …`,
  finishes in under 10 s with the model cached, and `verify` shows the same counts as the parent
  plus or minus the branch's own changes.
- `repos` lists both checkouts under one `beauty-crm`; after `rm -rf` of the worktree, the next
  `repograph` command in any project drops the row.
- An `ask` on the exact-id path measures no more than 1 ms slower than 0.4.0
  (`REPOGRAPH_TIMING=1`).
- `cargo test`, `cargo clippy --all-targets` clean; bench floors unchanged.
