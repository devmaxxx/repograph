# Install

The packages live in GitHub Packages, which serves no anonymous reads — every consumer
authenticates, public package or not. Once per machine, in `~/.npmrc`:

```
@devmaxxx:registry=https://npm.pkg.github.com
//npm.pkg.github.com/:_authToken=<classic PAT with read:packages>
```

The token must be a **classic** personal access token; the npm registry does not accept
fine-grained ones. A project-level `.npmrc` can carry the first line, but not the second: pnpm
stopped expanding `${ENV_VAR}` in project files in 11.5.3, so a committed `_authToken` would
either be a literal secret or silently ignored.

Then:

```bash
pnpm add -D @devmaxxx/zerograph    # npm: prebuilt binary for macOS arm64, Linux x64 and Windows x64
cargo install --path .             # from source; Rust 1.98, pinned in rust-toolchain.toml
```

The npm package is a launcher: the binary comes from `@devmaxxx/zerograph-darwin-arm64`,
`@devmaxxx/zerograph-linux-x64` or `@devmaxxx/zerograph-win32-x64`, pulled in as an optional
dependency, so a lockfile written on one platform installs on the others. All three are under the
same scope, so the one registry line above covers them. Tagged releases (`v*`) build all three
binaries as GitHub release assets (a tarball for the unix ones, a zip holding `zerograph.exe` for
Windows) and cut the npm packages from those same files (`.github/workflows/release.yml`,
publishing with the repository's own `GITHUB_TOKEN`; `scripts/npm-pack.sh` does the same by
hand).

## Coming from repograph

zerograph was called repograph until 0.6.0. Swap the package (`pnpm remove @devmaxxx/repograph`,
then `pnpm add -D @devmaxxx/zerograph`); the launcher still answers to `repograph` as well, so a
script that calls the old name keeps running. Nothing needs rebuilding or re-downloading, and
nothing is copied or deleted:

- **The store.** A `.repograph/` directory is read where it stands. The first write — `build`,
  `update`, a reader's inline refresh, `serve` starting — renames it to `.zerograph/` in one step,
  `.gitignore` inside it included. When both exist, `.zerograph/` is the store and `.repograph/` is
  left alone for you to remove. A symlink at either name is refused, as it always was.
- **The project file.** `zerograph.toml` is read, else `repograph.toml`; `zerograph model` rewrites
  whichever one it found. With both present only `zerograph.toml` is read, and a notice says so.
  `git mv repograph.toml zerograph.toml` when convenient.
- **The machine.** `~/.config/repograph/config.toml` is read while there is no
  `~/.config/zerograph/config.toml`. A model trusted under either directory stays trusted; new
  trusts are written to the new one. Each model cache under `~/.cache/repograph/` is used in place
  until one under `~/.cache/zerograph/` exists.
- **The environment.** Every `REPOGRAPH_*` variable is read while its `ZEROGRAPH_*` name is unset;
  a `ZEROGRAPH_*` that is set, even to nothing, wins.
- **Agents and git hooks.** Re-run `zerograph install-agent` with the flags you used before. It
  replaces the old instructions stanza, settings entries and git hook blocks rather than adding
  second ones, deletes `.claude/hooks/repograph-hook.mjs`, and removes a Claude Code MCP server
  registered as `repograph`. Codex's old script stays until `~/.codex/hooks.json` points at
  `zerograph-hook.mjs`; the install prints the block to paste.

## On Windows

Windows 10 1903, Windows 11 or Server 2022, CPU inference, an unsigned binary, and a handful of
platform-specific traps — SmartScreen on a browser-fetched zip, Defender scanning the model as it
lands, `Start-Process` instead of `&`, PowerShell's code page on a *piped* answer, and keeping the
repository out of a OneDrive tree. All of it is in [running on Windows](windows.md).
