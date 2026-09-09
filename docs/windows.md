# Running repograph on Windows

Moved out of the README on 2026-09-10; nothing here changed in the move.

## What the Windows build needs, and what it trips over

The floor is Windows 10 1903, Windows 11 or Server 2022. The ONNX Runtime the Windows binary links
is pyke's DirectML build, which imports DirectML and DirectX 12 at load; those libraries are inbox
from 1903 on, and an older system fails at load rather than at a query. No GPU is used: no execution
provider is registered, so inference runs on the CPU — a GPU-less runner builds a store with
vectors and answers a fused question through a resident `serve`
([run](https://github.com/devmaxxx/repograph/actions/runs/34064881516)).

The binary is not signed. A zip fetched with a browser carries the mark of the web, Explorer's
"Extract All" passes it to the exe, and double-clicking such a copy shows SmartScreen; running it
from a terminal does not, and `Unblock-File .\repograph.exe` clears the mark for good. `gh run
download`, `curl` and npm write no mark at all. Defender scans the model as it lands — 470 MB for
the default `multilingual-e5-small`, 2.2 GB where `embed_model` names the large one — and every
`graph.json` and `vectors.f32` rewrite on close;
`Add-MpPreference -ExclusionPath` on `%USERPROFILE%\.cache\repograph` and on the repository's
`.repograph` is an optional speed-up, not a requirement. An unsigned Rust binary can also draw a
heuristic false positive: restore it from Protection History and add an exclusion.

PowerShell has no `&` job operator, so a resident server is started with
`Start-Process repograph -ArgumentList 'serve','--idle','86400' -WindowStyle Hidden`; `--idle` ends
it, and so does `Stop-Process -Name repograph`. Neither runs the exit that removes the socket file,
which is what the next `serve` sweeps before binding. Stop the server before replacing the binary:
`npm i -g`, `cargo install` and `Expand-Archive -Force` all fail with a sharing error against a
running image.

Keep the repository out of a OneDrive, Dropbox or Google Drive tree. `.repograph` is per machine
and worth nothing to another one, every store write is a rename the client sees as a new file to
upload, and `serve.sock` is a reparse point of a tag no sync client knows — what a given client
does with one is not verified here. A rename over a store file another program holds open is
waited out for about half a second before it is reported, which is what a scanner or an
indexer costs.

Console output is UTF-8. Windows Terminal renders it, and so does Claude Code's Bash tool; but
PowerShell decodes a *piped or captured* native command's output with `[Console]::OutputEncoding`,
which on a Russian-locale system is code page 866 unless the system UTF-8 option or a profile line
sets it otherwise — `repograph ask … | Out-File` and `$x = repograph ask …` are where a Cyrillic
answer turns to mojibake, not the screen. Environment paths must be Windows-form even when they
are set inside Git Bash: `FASTEMBED_CACHE_DIR`, `XDG_CONFIG_HOME` and `REPOGRAPH_CONFIG` reach a
native exe as written, and MSYS converts arguments, not arbitrary values. For a tree deeper than
260 characters, git itself needs `core.longpaths=true` to check it out; repograph follows it from
there.
