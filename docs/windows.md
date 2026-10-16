# Running zerograph on Windows

Moved out of the README on 2026-09-10; nothing here changed in the move.

## What the Windows build needs, and what it trips over

The floor is Windows 10 1903, Windows 11 or Server 2022. The ONNX Runtime the Windows binary links
is pyke's DirectML build, which imports DirectML and DirectX 12 at load; those libraries are inbox
from 1903 on, and an older system fails at load rather than at a query. No GPU is used: no execution
provider is registered, so inference runs on the CPU — a GPU-less runner builds a store with
vectors and answers a fused question through a resident `serve`
([run](https://github.com/devmaxxx/zerograph/actions/runs/34064881516)).

Install through npm: `npm install -g zerograph` or `pnpm add -g zerograph`. The binary is not
Authenticode-signed, and SmartScreen judges only files that carry the mark of the web, which a
browser writes and npm, pnpm, `gh release download` and `curl` do not — so the npm route never shows
the "Windows protected your PC" screen. A zip fetched with a browser and opened through Explorer
does; `Unblock-File .\zerograph.exe` clears the mark for good, and running it from a terminal does
not trigger the screen at all. Unsigned is not unverifiable: every release binary carries a build
provenance attestation and a line in `SHA256SUMS`, and [verifying a download](install.md#verifying-a-download)
shows both checks. An unsigned Rust binary can still draw a heuristic false positive from Defender:
restore it from Protection History and add an exclusion.

Defender scans the model as it lands — about 1.2 GB for the default `embeddinggemma-300m` — and
every `graph.json` and `vectors.f32` rewrite on close. The first download prints the
`Add-MpPreference -ExclusionPath` line for the model cache; it and one for the repository's
`.zerograph` are an optional speed-up, not a requirement.

`zerograph serve --detach` starts a resident server with no console window and returns once it
listens; its output goes to `.zerograph/serve.log`. `zerograph stop` ends it through the same exit
an idle timeout takes, which removes the socket file — `Stop-Process` does not, and leaves the file
for the next `serve` to sweep. Upgrading while a server runs is safe through npm: the launcher runs
a per-version copy of the binary from `%LOCALAPPDATA%\zerograph\bin`, so `npm i -g` can replace
the package's file under a live server, and the old copy is removed by the first launch after the
server that ran it has gone. `cargo install` and `Expand-Archive -Force` still replace the running
file itself, so stop the server before either.

A reader over its refresh budget, and the git hooks `install-agent` writes, start `zerograph update`
detached: no console, a process group of its own, and out of the caller's job object where the job
allows breakaway. A job that forbids it — some CI runners and agent harnesses — keeps the refresh
inside, and closing that job ends it partway; the vectors it embedded are checkpointed and the next
reader starts another for the rest. Git for Windows runs the `#!/bin/sh` hooks through its own
shell, so they need no executable bit.

Keep the repository out of a OneDrive, Dropbox or Google Drive tree. `.zerograph` is per machine
and worth nothing to another one, every store write is a rename the client sees as a new file to
upload, and `serve.sock` is a reparse point of a tag no sync client knows — what a given client
does with one is not verified here. A rename over a store file another program holds open is
waited out for about half a second before it is reported, which is what a scanner or an
indexer costs.

Console output is UTF-8. Windows Terminal renders it, and so does Claude Code's Bash tool; but
PowerShell decodes a *piped or captured* native command's output with `[Console]::OutputEncoding`,
which on a Russian-locale system is code page 866 unless the system UTF-8 option or a profile line
sets it otherwise — `zerograph ask … | Out-File` and `$x = zerograph ask …` are where a Cyrillic
answer turns to mojibake, not the screen. Environment paths must be Windows-form even when they
are set inside Git Bash: `FASTEMBED_CACHE_DIR`, `XDG_CONFIG_HOME` and `ZEROGRAPH_CONFIG` reach a
native exe as written, and MSYS converts arguments, not arbitrary values. For a tree deeper than
260 characters, git itself needs `core.longpaths=true` to check it out; zerograph follows it from
there.
