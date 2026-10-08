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
pnpm add -D @devmaxxx/repograph    # npm: prebuilt binary for macOS arm64, Linux x64 and Windows x64
cargo install --path .             # from source; Rust 1.98, pinned in rust-toolchain.toml
```

The npm package is a launcher: the binary comes from `@devmaxxx/repograph-darwin-arm64`,
`@devmaxxx/repograph-linux-x64` or `@devmaxxx/repograph-win32-x64`, pulled in as an optional
dependency, so a lockfile written on one platform installs on the others. All three are under the
same scope, so the one registry line above covers them. Tagged releases (`v*`) build all three
binaries as GitHub release assets (a tarball for the unix ones, a zip holding `repograph.exe` for
Windows) and cut the npm packages from those same files (`.github/workflows/release.yml`,
publishing with the repository's own `GITHUB_TOKEN`; `scripts/npm-pack.sh` does the same by
hand).

## On Windows

Windows 10 1903, Windows 11 or Server 2022, CPU inference, an unsigned binary, and a handful of
platform-specific traps — SmartScreen on a browser-fetched zip, Defender scanning the model as it
lands, `Start-Process` instead of `&`, PowerShell's code page on a *piped* answer, and keeping the
repository out of a OneDrive tree. All of it is in [running on Windows](windows.md).
