# Install

```bash
pnpm add -D zerograph              # npm: prebuilt binary for macOS arm64, Linux x64 and Windows x64
cargo install --path .             # from source; Rust 1.98, pinned in rust-toolchain.toml
```

The npm package carries all three binaries and a launcher that runs the one for this platform,
so a lockfile written on one platform installs on the others, nothing runs at install time, and
nothing is fetched beyond the package itself (about 40 MB). It is on the public npm registry and
needs no `.npmrc`. Tagged releases (`v*`) build all three binaries as GitHub release assets (a tarball
for the unix ones, a zip holding `zerograph.exe` for Windows) and cut the npm package from those
same files (`.github/workflows/release.yml`, publishing through npm's trusted publishing, so each
version carries a provenance attestation; `scripts/npm-pack.sh` packs the same tarball by hand).

## Coming from repograph

zerograph was called repograph until 0.6.0. Nothing needs rebuilding or re-downloading, and
nothing is copied or deleted, but whatever still runs as `repograph` has to stop first: the first
write by zerograph moves the store (below) out from under it.

1. **Stop what runs as `repograph`.** A `repograph serve` (an agent session starts one in the
   background) or a `repograph watch` that outlives the move reads the store as empty and builds a
   second `.repograph/` of its own. `pkill -f 'repograph.* (serve|watch)'`, and end the agent
   sessions that use the old MCP server.
2. **Swap the package.** `pnpm remove @devmaxxx/repograph`, then `pnpm add -D zerograph`, and
   change scripts that call `repograph` to call `zerograph`: there is no command under the old name,
   and `@devmaxxx/repograph` gets no release after 0.5. Installed globally, `npm uninstall -g
   @devmaxxx/repograph` or `pnpm remove -g @devmaxxx/repograph`. Installed from source, run `cargo
   uninstall repograph`, or the old binary stays on `PATH` and goes on writing a `.repograph/` of
   its own. zerograph comes from the public npm registry, so the `@devmaxxx` lines in `~/.npmrc` can
   go.
3. **Re-run `zerograph install-agent`** with the flags you used before, straight away. The old hook
   script looks for `.repograph/` only, so once the store has moved it says nothing, and the
   install is what replaces it.

What zerograph reads from the old install:

- **The store.** A `.repograph/` directory is read where it stands. The first write — `build`,
  `update`, a reader's inline refresh, `serve` starting — renames it to `.zerograph/` in one step,
  `.gitignore` inside it included. When both exist, `.zerograph/` is the store and `.repograph/` is
  left alone for you to remove; one that comes back after the move was made by something from step
  1 or 2. A symlink at either name is refused, as it always was.
- **The project file.** `zerograph.toml` is read, else `repograph.toml`; `zerograph model` rewrites
  whichever one it found. With both present only `zerograph.toml` is read, and a notice says so.
  `git mv repograph.toml zerograph.toml` when convenient.
- **The machine.** `~/.config/repograph/config.toml` is read while there is no
  `~/.config/zerograph/config.toml`; with both, only the new one is read and a notice says so, so
  move what the old one set before creating the new. A model trusted under either directory stays
  trusted; new trusts are written to the new one. Each model cache under `~/.cache/repograph/` is
  used in place until one under `~/.cache/zerograph/` exists.
- **The environment.** Every `REPOGRAPH_*` variable is read while its `ZEROGRAPH_*` name is unset;
  a `ZEROGRAPH_*` that is set, even to nothing, wins.
- **Agents and git hooks.** The install in step 3 replaces the old instructions stanza, settings
  entries and git hook blocks rather than adding second ones, deletes
  `.claude/hooks/repograph-hook.mjs`, and removes the local-scope Claude Code MCP server registered
  as `repograph` once `zerograph` is registered (one in another scope is named, with the command
  that removes it). Codex's old script stays until `~/.codex/hooks.json` points at
  `zerograph-hook.mjs`; the install prints the block to paste.

## On Windows

Windows 10 1903, Windows 11 or Server 2022, CPU inference, an unsigned binary, and a handful of
platform-specific traps — SmartScreen on a browser-fetched zip, Defender scanning the model as it
lands, `Start-Process` instead of `&`, PowerShell's code page on a *piped* answer, and keeping the
repository out of a OneDrive tree. All of it is in [running on Windows](windows.md).
