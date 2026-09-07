# More Windows CI Tests — Plan

**Ask:** "подготовь больше тестов в ci под windows". Branch `feat/windows-serve` at `3f1cf46`.

**What the Windows job proves today** (run 34066540506, 7 min 34 s): clippy (91 s); `cargo test` — 455
unit + 11 serve, 3 ignored (101 s); the 30 `enrich::`/`rerank::` tests with `Git\bin` and `Git\usr\bin`
off PATH (1 s); the dense arm on e5-small through the socket, cache restored (11 s, resident fused ask
31 ms median); a lexical `build` + `changes --base HEAD` on the checkout (207 s — 202 s of it a
`cargo run --release` compile, 5 s of judging). The release dispatch smokes `repograph.exe --version`.

**What runs nowhere:** the `bench` job is gated on `BENCH_REPO_TOKEN`, which is unset — it prints a
warning in 3 s on every run. The unix `test` job and the Windows `cargo test` are the same set apart
from three gated tests: `a_binary_replaced_under_a_live_server…` (unix; Windows will not replace a
running image), `a_non_utf8_file_name_is_skipped…` (linux), and the four `cfg(windows)` tests
(the rename retries, `plain()`, the resolver). The gap is not in `cargo test`; it is everything a
user does *around* the binary: install it, start it from PowerShell, run it under a Cyrillic path,
have Claude Code's shell find `claude`, ask it eight times at once.

**Rules for every item below.**
- A CI step names its red: an assertion on the `serve: answered by the resident process` line, an
  exit code, a `throw`. A step that runs a command and calls success "green" judges nothing — the
  fallback path makes `ask` succeed with no server at all.
- Environment goes on the child `Command` (`.env`, `.env_remove`), never `set_var`: the config
  tests' races are filed separately, and `embed.rs` already took the `CACHE_OVERRIDE` route to
  stay out of that. No Windows-only test today mutates the environment; none added here does.
- The unix path stays byte-identical: tests only, `cfg` gates only where a platform cannot make
  the case, no Cargo change (socket2 and windows-sys are already Windows deps, and `tests/serve.rs`
  already carries the transport seam).
- Every step after `cargo test` reuses `target\debug\repograph.exe` and the built test binaries:
  zero compile. Every Windows verdict is a CI run; nothing here is verifiable on the Mac.

## (i) Do now

### 1. Pay for everything: the `changes` step on the debug exe — saves 3 min 22 s
Not a test; the budget. `cargo test` has already built `target\debug\repograph.exe` (the bin is what
`CARGO_BIN_EXE_repograph` points the serve tests at). The step judges `git -C` and the stripped
path, not codegen; the release bytes are the release workflow's job (item 12). Replace the two
`cargo run --release --quiet --` with `.\target\debug\repograph.exe`. Job drops to ~4 min 10 s.

### 2. The user's day, in one pwsh step: a Cyrillic path with a space, the README's own recipe
*Proves:* the `### On Windows` section verbatim on a runner, and the branch's path work end to
end — canonicalize/`plain()`, the walk, the store's renames, the socket under `sun_path`, `git -C`,
and `sh` spawned from a Cyrillic working directory — none of which any test runs together, and
none under a non-ASCII repository path (the runner's is `D:\a\repograph\repograph`).
*Where:* `.github/workflows/ci.yml`, `windows` job, after the `changes` step.
*Shape* (`$R = "$env:RUNNER_TEMP\Проект beauty crm"`, `$X = target\debug\repograph.exe`):
1. `Copy-Item docs, README.md` into `$R`; `git init`, `git add`, `git commit` there.
2. Write `repograph.toml` with **Windows PowerShell 5.1** — `powershell.exe -c "... | Out-File
   -Encoding utf8"` — which writes BOM + CRLF, the bytes a user's editor writes. Keys:
   `id_families` and `embed_model = "intfloat/multilingual-e5-small"` (the cache is restored).
   toml_parser 1.1.3 strips the BOM in its lexer (`lexer/mod.rs:35`), so this is expected green;
   it is here because it costs nothing and is what the file on a user's disk looks like.
3. `& $X --repo $R build` — assert `nodes` > 0 on the summary line.
4. The recipe as written: `Start-Process $X -ArgumentList '--repo',$R,'serve','--idle','86400'
   -WindowStyle Hidden`. Then `& $X --repo $R ask штраф 2>&1` in a loop until stderr carries
   `answered by the resident process` (20 s cap, then `throw`). Fused, from the cache: the README's
   recipe, not a `--no-dense` stand-in.
5. `enrich` with the **default** `enrich_command` and a fake `claude`: a directory prepended to
   PATH holding `claude` (no extension, `#!/bin/sh`, body `awk '/^### /{printf "%s\tq for %s\n",
   $2, $2}'` — the shape npm's own shim has, which is why it is a sh script and not a `.cmd`).
   Assert `.repograph\questions.json` has entries. This is the line a beauty-crm user actually
   runs: `MAX_THINKING_TOKENS=0 claude -p --model haiku … --tools ""` through Git's `sh`, which no
   test spawns today (they pass an awk command, never the default).
6. Append a line to `$R\README.md`; `update` — assert `changed 1`; `changes --base HEAD` — assert
   the output names `README.md`.
7. `Stop-Process -Name repograph`; assert `serve.sock` is still there (`Get-Item -Force`; the
   README says so and nothing pins it); `& $X --repo $R --no-dense serve --idle 1` — assert exit 0
   within 10 s and the file gone: the sweep over a killed predecessor's socket, then the guard.
*Cost:* ~30 s. *Red when:* the resident line never comes (serve failed under the path, or the
recipe's `Start-Process` form broke), `sh` cannot start in a Cyrillic cwd, the default command does
not reach `claude`, git refuses the path, the sweep or the guard misses.

### 3. The `sh` resolver: each of the three routes alone, against the real Git for Windows
*Proves:* today's stripped step keeps `C:\Program Files\Git\cmd` on PATH, so the "beside `git`"
route answers first and the other two are never reached on a runner. A box with no Git on PATH at
all — Claude Code installed its own bash, or a portable Git — takes the routes nobody has seen.
*Where:* three pwsh steps beside the existing one (which becomes route 1 by name), each running
`cargo test -- enrich:: rerank::` on the already-built test binary.
*Shape:* strip every entry matching `\\Git\\` from PATH and assert neither `sh` nor `git` resolves;
then (a) route 3, Program Files: nothing else changed; (b) route 2, the hint: `ProgramFiles`,
`ProgramW6432`, `LOCALAPPDATA` pointed at an empty directory, `CLAUDE_CODE_GIT_BASH_PATH=C:\Program
Files\Git\bin\bash.exe`; (c) none: the three pointed at the empty directory and no hint — the run
must **fail**, and its output must contain `sh is not on PATH and no Git for Windows was found`
(invert the exit code; a green here is a resolver that found a shell it should not have).
*Cost:* ~2 s each. *Red when:* the image's Git moves, an env key is misspelled, `usr\bin` is not on
the child's PATH (awk fails inside `sh -c`), or (c) passes.

### 4. Four integration tests in `tests/serve.rs` — every platform, `cargo test`
Each ~1–3 s, on unix too (the unix job gains them; nothing it had changes).
- `several_asks_at_once_are_all_answered_by_the_one_resident_process`: spawn eight `ask`s without
  waiting, then wait all; every stderr carries the resident line, every stdout equals the direct
  answer. *Red when:* Windows AF_UNIX refuses a concurrent connect under the backlog (fallback
  keeps stdout right and the stderr line absent — the assertion that matters), or the rendezvous
  handoff drops a stream.
- `a_second_serve_on_the_same_repository_bails_and_leaves_the_first_answering`: second `serve`
  exits non-zero with `another serve answers`; the first still answers resident afterwards. The
  probe's failure mode is the cascade `serve.rs` describes — sweep a live socket, bind over it,
  strand the first — and no test pins the probe on any platform.
- `an_idle_server_exits_clean_and_takes_its_socket_with_it`: `--idle 1`; status 0 within 10 s,
  stderr `idle for 1s`, socket absent by `symlink_metadata`. The `Unlink` guard through a real
  process on Windows: today only its primitive is unit-tested, and an `id()` that returned `None`
  would make the guard a silent no-op.
- `no_serve_and_its_environment_variable_both_answer_here_under_a_live_server`: `--no-serve` and
  `REPOGRAPH_NO_SERVE=1` (on the child) each answer with no `serve:` line, same stdout. The
  variable is untested anywhere.
- `a_repository_under_a_cyrillic_directory_with_a_space_is_built_served_and_asked`: `tempdir` →
  `Проект beauty crm` → build, `git init` + commit, serve, resident ask, edit, `changes --base
  HEAD`. `#[cfg(not(target_os = "macos"))]` with the arithmetic in the comment: macOS's temp is
  ~66 bytes, plus 23 for the name and 22 for `\.repograph\serve.sock` passes its 103-byte
  `sun_path`; Linux and Windows do not. Item 2 is the recipe on the checkout; this is the same
  path property, reproducible on Linux and readable in Rust.

### 5. The npm install path a beauty-crm user takes, on the runner
*Proves:* `bin/repograph.js` resolving `bin/repograph.exe`, npm's `.cmd` shim, the `files`
whitelist, exit-code passthrough, a Cyrillic argument through `%*`, and `scripts/npm-pack.sh`
under Git Bash — none of which any test or step touches (the transport plan's launcher check was
a `node -e` with `process.platform` stubbed, on the Mac).
*Where:* `ci.yml`, `windows` job, last (the pack rewrites `npm/*/package.json` versions in the
checkout; `git checkout -- npm/` after, and it must run after `changes`).
*Shape:* `bash scripts/npm-pack.sh 0.0.0-ci win32-x64=target/debug/repograph.exe` (node and npm
are on the image; no `setup-node`). In `$env:RUNNER_TEMP\проект node`: `npm init -y`, then
`npm i <win32 tgz> <launcher tgz>` — the launcher's darwin/linux optionals point at GitHub
Packages, which answers 401 anonymously; npm skips a failed optional, and if this image's npm
aborts instead, `--omit=optional` with the win32 tgz named explicitly resolves the same
`require.resolve` walk. Then, with `[Console]::OutputEncoding = [Text.Encoding]::UTF8`:
`npx repograph --version` → `repograph 0.5.0`; `npx repograph --repo $R --no-dense ask штраф` →
output contains `Штраф за отмену` (bytes through `stdio: inherit` and the shim, decoded once);
`npx repograph --repo $R explain nope; $LASTEXITCODE` → 1; the launcher alone with
`--omit=optional` → exit 2 and `is not installed` on stderr.
*Cost:* ~30 s. *Red when:* the `.exe` resolution, the pack script's Windows arm, the shim's
argument passing, or the exit code breaks.

### 6. The model cache under a Cyrillic profile
*Proves:* hf-hub's cache walk, tokenizers' file read and ORT's `commit_from_file` (its own
UTF-16 conversion on Windows) under `C:\Users\Максим\.cache\…` — the box's actual path. The job's
cache is `D:\a\repograph\repograph\.model-cache`, ASCII; the branch's Cyrillic test covers the
socket only.
*Where:* `ci.yml`, after the fused smoke. *Shape:* `Copy-Item -Recurse .model-cache
"$env:RUNNER_TEMP\Максим\.cache\fastembed"` (470 MB; a junction would resolve back to the ASCII
path and prove nothing), then the ignored fused test again with `FASTEMBED_CACHE_DIR` set there.
*Cost:* ~45 s (the copy dominates). *Red when:* the test's `dense: model unavailable` assertion
fires — a path mangled in any of the three libraries.

## (ii) Worth it

### 7. The socket name at the boundary — `tests/serve.rs`, every platform
Repository directory padded so the socket path is exactly `SUN_LEN − 1` bytes (108 − 1 on Linux
and Windows, 104 − 1 on macOS; socket2 refuses `>= sun_path.len()`, `sys/windows.rs:1044`, std's
check is the same shape): serve binds, ask is resident. At `SUN_LEN`: serve exits non-zero with the
path in its message; ask answers itself with no `serve:` line, in under 5 s. ~1 s. *Red when:* the
bind moves to a relative path or cwd trick, the message loses the path, or the fallback waits out
a timeout.

### 8. An 8.3 spelling and the long one name one socket — Windows only
The runner's `TEMP` is `RUNNER~1`, so every serve test already passes an 8.3 path — to *both*
sides. Serve on `dir.path()`, ask on `plain(dir.path().canonicalize())`; precondition
`assert_ne!` on the two spellings so the test says when the image stops giving short names
rather than passing vacuously. ~1 s. *Red when:* `--repo` stops being canonicalized on one side.

### 9. The machine file at `%USERPROFILE%\.config\repograph\config.toml`
Integration test spawning the binary with `USERPROFILE` (Windows) / `HOME` (unix) set to a tempdir
holding a machine file whose `enrich_command` is the awk line, `XDG_CONFIG_HOME` and
`REPOGRAPH_CONFIG` removed on the child; `enrich` writes `questions.json` ⇒ the file was read from
the documented place. Today only the `REPOGRAPH_CONFIG` route is tested, in-process. ~1 s. *Red
when:* home resolution moves to `HOME`, which a Windows console never sets.

### 10. Unix and Windows answer the docs tree byte for byte
Both the `test` and the `windows` jobs run a fixed list of `ask --json` questions on the checkout
(the `changes` step already builds it on Windows; the unix job would build it too, ~5 s) and upload
`answers.json`; a third ubuntu job, `needs: [test, windows]`, ~15 s, diffs them. The strongest
"Windows == unix" statement available without the corpus. *Red when:* a separator or path leaks,
float formatting differs, or any output depends on HashMap order — a flake here is itself a
finding. Modest value; the cheapest form of the bench question (item 13).

### 11. A `.ts` with a BOM — `src/code/cases.rs`, every platform
Visual Studio writes BOMs; the doc extractor strips one (`requirements.rs:50`), the code extractor
does not. Checked on the Mac through `dump_tree`: tree-sitter parses a BOM-prefixed source with no
ERROR node, so symbols survive — what may not is `file_head`'s body or a span. Twin test like
`a_crlf_source_extracts_the_same_nodes_and_edges_as_its_lf_twin`. ~0 s. *May go red now* on a body
byte; the fix is at the one consumer, same on every platform.

### 12. The release bytes, exercised — `release.yml`, Windows build job
After the `--version` smoke: `build` (lexical) on the checkout, `Start-Process … serve`, a resident
`ask`, `Stop-Process` — the LTO exe a user downloads, ~20 s. Restore the same `e5-small-onnx-1`
cache key and the fused smoke joins for ~15 s more. Dispatch and tag only, so it costs no push.

## (iii) Not worth it, and why

- **A Windows bench on beauty-crm.** The bench runs nowhere today (token unset). On Windows it
  would add the corpus checkout, a 3.5-min release build and a CPU embed of ~2 000 nodes for an
  arm whose lexical half is deterministic by construction and whose dense floors are integer hit
  counts on the same model bytes — ~10 min to learn what item 10 says for 15 s. Revisit when the
  token exists, as a parallel job.
- **Ctrl-C through the launcher.** No console to send `CTRL_C_EVENT` into on a runner step; the
  `status === null → 128` branch is two lines to read. Documented.
- **A socket file deleted from under a running `serve`.** The client then takes the no-file path
  every test takes before its server starts; the guard's `None` compare is unit-tested. A test
  asserting silence.
- **`serve` polling while `update` rewrites the store, as a loop.** One cross-process write under a
  live server is pinned (`a_stale_socket_answer…`); std's share modes make the rename succeed by
  construction; a loop's failures would be timing.
- **Console encoding.** `[Console]::OutputEncoding` is PowerShell's decoding; the bytes are UTF-8
  through a pipe in every serve test on Windows, and item 5 asserts them once more under UTF-8.
  The code-page-866 mojibake case tests PowerShell.
- **A BOM in `repograph.toml` as a unit test.** toml_parser strips it; item 2 writes one through
  PowerShell 5.1 for free; a pin would judge a `=`-pinned dependency.
- **`--rerank-local` on Windows.** `cross.rs`'s ignored test needs an exported model nobody
  publishes; nothing for a runner to fetch.
- **A Windows twin of the linux-only non-UTF-8 name test.** `walk.rs`'s `to_str()` skip is one
  line on every platform; a lone-surrogate name proves std's `to_str` returns `None`, by definition.
- **A second Windows job.** Not needed: after item 1 the job is ~4 min and items 2–6 add ~2 min.
  If e2e ever passes ~2 min on its own, split it as a *parallel* `windows-e2e` (own debug build,
  ~2.5 min; the repository is public, so runner minutes are free) — never `needs: windows`, which
  serializes and adds four minutes to every red.

## Job time

| Step | Today | After |
|---|---|---|
| setup, clippy, `cargo test` | 227 s | 227 s (+ ~10 s for item 4's tests) |
| stripped PATH (route 1) | 1 s | 1 s |
| routes 2, 3, none (item 3) | — | ~6 s |
| cache, fused smoke, reparse listing | 14 s | 14 s |
| Cyrillic cache (item 6) | — | ~45 s |
| `changes` on the checkout | 207 s | ~5 s (item 1) |
| the user's day (item 2) | — | ~30 s |
| npm install path (item 5) | — | ~30 s |
| **job** | **7 min 34 s** | **~6 min** |

## Found while reading, not Windows-specific
- `changes` drops every hunk of a non-ASCII file name: git's default `core.quotepath=true` prints
  `+++ "b/\321\217…"`, `parse` finds no `b/` after the quote, and `ls-files --others` lines come
  back quoted (verified in a scratch repository). Same on every platform; flagged as its own task
  (`-c core.quotepath=false` on both git calls, plus a test). Item 2 uses ASCII file names under
  a Cyrillic directory on purpose, so it judges the path and not this.
- The config tests that call `Config::load` without the `ENV` lock race `with_machine`'s erroring
  machine file (`the_machine_file_may_not_set_a_corpus_key`); filed separately. No Windows-only
  test shares that shape.

## Order
1 (the budget), 4 and 2 (the resident line is the assertion everything else leans on), 3, 5, 6;
then 7–12 as time allows. Each CI step lands with its run URL in the PR, red first where a red is
cheap to make (item 3's route (c) is red by design; item 2's step 4 goes red by starting `serve`
with a wrong `--repo` once).
