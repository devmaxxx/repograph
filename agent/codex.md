# What Codex actually reads

Read from the installed CLI on 2026-09-09 — `codex --version` 0.147.0, the vendored binary at
`@openai/codex-darwin-arm64/vendor/aarch64-apple-darwin/bin/codex` — rather than from memory of a
docs page. Three facts, each with how it was established, because the first install written against
an assumption put two files where nothing would ever read them.

Two of the three are settled by running the CLI: `codex debug prompt-input` renders the exact
message list a session would be given, and costs no model call. That is the check to re-run when a
Codex release moves any of this.

## Instructions: the repository's root `AGENTS.md`

The binary's own instruction template carries an *AGENTS.md spec* section:

> Repos often contain AGENTS.md files. These files can appear anywhere within the repository. …
> The scope of an AGENTS.md file is the entire directory tree rooted at the folder that contains
> it. … The contents of the AGENTS.md file at the root of the repo and any directories from the CWD
> up to the root are included with the developer message and don't need to be re-read.

Confirmed by running it: a scratch repository whose root `AGENTS.md` held a unique marker, and
`codex debug prompt-input` in that directory printed the marker inside a message headed
`# AGENTS.md instructions for <cwd>`, under a `--- project-doc ---` separator.

So the stanza goes in `<repo>/AGENTS.md`, created when the repository has none. There is no
`.codex/AGENTS.md` in that spec, and an earlier draft of this installer wrote one — a file Codex
would never have opened. The same section is why the stanza is short: a root `AGENTS.md` is
included in **every** developer message, not fetched on demand.

## Hooks: a machine-level file, not a repository one

The config schema names hooks once, as `"hooks": "./hooks.json"` — a path relative to `CODEX_HOME`,
which is `~/.codex` unless the environment says otherwise. The live file on this machine
(`~/.codex/hooks.json`, written by GitNexus) confirms the shape, and it is the same shape Claude
Code uses:

```json
{ "hooks": { "PreToolUse": [ { "matcher": "Grep|Glob|Bash",
  "hooks": [ { "type": "command", "command": "node '/abs/path/hook.mjs'", "timeout": 10 } ] } ] } }
```

All four events this hook dispatches on are present in the binary's strings: `SessionStart`,
`SubagentStart`, `PreToolUse`, `PostToolUse`.

Whether a **repository-level** `.codex/hooks.json` is read as well is *not* established. The binary
carries the string `(Error parsing project hooks config file `, which suggests some project-scoped
hooks file exists; but a malformed `.codex/hooks.json` in a scratch repository produced no such
error, and the trusted-project probe that would have settled it could not be completed — see *What
has not been established*. The installer therefore does not depend on the answer either way.

**Nothing here writes that file.** It is machine-level: an entry added from inside one repository
would fire in every other repository on the machine, which is not a side effect a `--repo` command
gets to have. `install-agent --codex` therefore installs the repository-scoped half — the stanza and
the skill — copies the hook script into `.codex/hooks/` so there is something to point at, and
prints the JSON to add to `~/.codex/hooks.json`, for the reader to paste once if they want it.

Max, 2026-09-07: Codex gets the brief, the skill and the risk line, and **not** the Bash
interceptor — its verdict is being read on Claude Code and is not carried over blind. The printed
JSON reflects that: three events, no `PreToolUse`.

## Skills: `.codex/skills/<name>/SKILL.md`

Confirmed by running it: a scratch repository carrying `.codex/skills/probe-codex/SKILL.md` and
`.agents/skills/probe-agents/SKILL.md` had **both** listed under *Available skills* in
`codex debug prompt-input`'s output, with the roots table naming each directory. So
`.codex/skills/repo-query/SKILL.md` is read, which is where the installer puts it; `.agents/skills`
is the other repository-scoped root, and is left alone because a repository that uses it has its own
conventions there.

Only the *description* line of each skill is in the prompt — the body is fetched on demand — which
is why the skill's front matter says when to use it and the body says how.

## What has not been established

Whether Codex delivers `additionalContext` from `SubagentStart` the way Claude Code does. The event
name is in the binary; the output contract for it is not. Until a real Codex run shows the rule
arriving in a subagent, the Codex install's subagent propagation is unproven — which is why the
printed JSON is something a person opts into rather than something this command performs.

Whether a repository-level `.codex/hooks.json` fires. Testing it needs a session that reaches
`SessionStart` in a **trusted** project, and the attempt to run one under a scratch `CODEX_HOME`
spent the machine's ChatGPT refresh token — `codex login` is the repair, and no further probe of
this kind should be run against a live login. Re-check with a project that is already trusted, from
a session a person starts themselves.
