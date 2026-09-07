# Windows Edge Cases and a Downloadable Build — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Max's target is the beauty-crm repository on a Windows 11 machine, indexed and asked with a resident `serve`, at the speed the unix binary gives. Branch `feat/windows-serve` (tip `7dd7011`, plan `docs/superpowers/plans/2026-09-06-windows-serve-transport.md`) made the crate compile on `x86_64-pc-windows-msvc`, put the socket behind a seam, added a green `windows-latest` CI job and a green three-target release dry run, and wired the npm `win32-x64` package. PR #15 (`olkp1`, `fix/file-id-on-stable`, being rebased as this is written) restores the socket file's identity on Windows through `GetFileInformationByHandle`. What is still missing is in two parts. (A) Nothing that run produced can be downloaded: the dry run compiles `repograph.exe` and uploads it nowhere, because the upload is gated on a tag. (B) The CI job is green on a runner that is not a user's machine: it carries Git for Windows' `usr\bin` on `PATH`, an ASCII path, admin rights, no sync client, and it never opens the embedding model. This plan produces the download and then walks every Windows edge that was asked about, reading the branch's code before saying anything about it, and sorts each edge into must-fix, should-fix or documented-only.

**Architecture:** unchanged from the transport plan. `serve` (`src/serve.rs`) binds `.repograph/serve.sock` through `mod sys` — std's Unix socket on unix, socket2's `AF_UNIX` on Windows — and `ask` (`src/main.rs:396–425`) tries the socket first. `main.rs:353` canonicalizes `--repo` before anything reads it, which on Windows means every path in the process, the socket's included, carries the `\\?\` verbatim prefix. `enrich`/`ask --rerank` shell out through `sh -c` (`src/enrich.rs:252`). The store writes through `write_atomic` (`src/store.rs:92`, a temp file and a rename). The embedder fetches through `hf-hub` into `cache_root()` (`src/index/mod.rs:12`) and opens an `ort` session with no execution provider named (`src/index/embed.rs:127`). This plan adds no module; it adds one function to `enrich.rs`, one to `main.rs`, one to `store.rs`, tests beside each, two CI steps, one release step, and words.

**Tech Stack:** Rust 1.98.0 pinned, every dependency `=`-pinned (`windows-sys =0.61.2` arrives with PR #15 under `[target.'cfg(windows)'.dependencies]`; this plan adds nothing to `Cargo.toml`). GitHub Actions: `windows-latest` is the `windows-2025-vs2026` image (read from the job log), with Git for Windows' `bin` and `usr\bin` on `PATH`. No Windows toolchain on the Mac, none installed; every Windows verdict is a CI run whose URL the step records. `gh 2.92.0` on the Mac.

**Spec:** this document. The edge-case findings are the spec; each one names the code it read.

## Global Constraints

- The worktree is `/Users/max/Documents/projects/repograph/.worktrees/windows-serve`, branch `feat/windows-serve`. Another agent is rebasing PR #15 there now: **Task 0 waits for `gh pr view 15` to say `MERGED`** and only then touches the worktree. Never `cd` into the main checkout; use `git -C` or run from the worktree.
- No step builds for Windows on this machine. Every Windows-affecting task ends with a push and a watched CI run (`gh auth switch --user devmaxxx && gh run watch …`), and the run's verdict is the step's verdict.
- The unix path stays byte-identical: every Windows change is behind `#[cfg(windows)]` or an `if cfg!(windows)`, and the unix half of each seam is the line that is there today. The wire format does not change. The bench floors are untouched because no retrieval code is.
- Cargo dependencies stay `=`-pinned. Nothing is added: PR #15's `windows-sys` carries `Win32_Storage_FileSystem`, which holds every constant this plan's Windows tests need (`FILE_SHARE_READ` and friends).
- The record directory is `$M=/Users/max/bench/windows-edge-2026-09-06`; every CI run URL a step produces is written there and the PR body cites them.
- Any `gh` call runs `gh auth switch --user devmaxxx && gh …` in the same shell command.
- The commit hook rejects AI attribution trailers and session links, and rejects a single shell command holding both a heredoc and `git commit` unless the heredoc's first line is a Conventional Commits subject: `git commit -F - <<'MSG'` with the subject first is the form. Edits and commits go in separate commands.
- Comments say why, never what; no ticket ids in comments; tool directives stay.
- Every "works on Windows" in a commit, the README or the PR cites a run URL. Nothing about Windows is asserted from memory.
- Implementers never dispatch subagents.

---

## What was read, and what it says

Facts this plan stands on, each from a file on this machine or a log fetched here. Later sections cite these by number.

1. `src/serve.rs:69` (branch tip): `sys::id` returns `None` on Windows; the `Unlink` guard (`serve.rs:320–325`) therefore removes nothing there, and the identity test is `#[cfg(unix)]` (`serve.rs:350`). PR #15's diff touches only `id` and `Cargo.{toml,lock}`: after it lands the identity exists on Windows and **no test on the Windows job reads it** until the `cfg` comes off.
2. `src/main.rs:353`: `let repo = cli.repo.canonicalize()?;` — on Windows, std's `canonicalize` opens the directory and asks `GetFinalPathNameByHandleW` (`std/src/sys/fs/windows.rs:1593`), which answers in the `\\?\C:\…` form. Every path in the process is derived from it: the socket (`serve::socket_path`), the store, the walk root, and `git -C` (`src/changes.rs:144`).
3. Windows CI job `101506740337` (run `34040679754`, green): `446 passed; 2 ignored` unit, `11 passed` in `tests/serve.rs`; the twenty `enrich::tests` and `rerank::tests::a_successful_commands_output_is_parsed_into_picks` passed — the runner resolves `sh` and `awk`. The whole job is under three minutes (clippy 1m08s, test build 1m12s, tests 5 s).
4. Release dry run `34040652089` (green, three build jobs, `npm` skipped): the Windows job printed `repograph 0.5.0` from `.\target\x86_64-pc-windows-msvc\release\repograph.exe --version` and packed the zip; no `upload-artifact` step exists, so nothing was kept.
5. `src/enrich.rs:252`: `Command::new("sh").arg("-c").arg(command)`. The only program repograph ever spawns by name is `sh` (and `git`, `changes.rs:144`); `awk`/`printf` appear only inside test command strings (`enrich.rs:421,466,494,503,514,565,589,598`; `rerank.rs:143`) and reach the OS through that `sh`. The default commands (`src/config.rs:43–44`) are `MAX_THINKING_TOKENS=0 claude -p --model {model} --output-format text --tools "" --setting-sources "" --no-session-persistence` — POSIX syntax (a `VAR=value` prefix, `""` quoting) on both keys.
6. Claude Code's setup page (fetched): on native Windows, Git for Windows is optional and is what gives Claude Code its Bash tool; the path to it is `CLAUDE_CODE_GIT_BASH_PATH`, default `C:\Program Files\Git\bin\bash.exe`.
7. `socket2-0.6.5/src/sys/windows.rs:1023–1057`: the address builder requires the path to be UTF-8 (`to_str`, error `path must be valid UTF-8`), refuses `bytes.len() >= 108` (error `path must be shorter than SUN_LEN`, so 107 bytes is the most), and its own comment says Windows expects UTF-8 in `sun_path`.
8. `std/src/sys/paths/windows.rs:174–179` (1.98.0): `home_dir()` reads `USERPROFILE` (non-empty) and otherwise the profile API. `HOME` is never consulted on Windows.
9. `std/src/sys/fs/windows.rs:206`: every std file open shares `READ | WRITE | DELETE`. `:1321–1357`: `rename` is `MoveFileExW(REPLACE_EXISTING)`, retried with `FileRenameInfoEx` + `POSIX_SEMANTICS` only when the error is `ACCESS_DENIED`; a sharing violation (os error 32) is returned as is. `:1298–1319`: `remove_file` is `DeleteFileW`, with a posix-delete retry on `ACCESS_DENIED` only. `:339,1229,1246`: open, stat and readdir go through `maybe_verbatim`, so std handles paths past 260 characters on its own.
10. `src/store.rs`: reads are whole-file (`read_bytes`, `std::fs::read`); `write_atomic` is write-temp-then-rename; `append_after` opens for write briefly; nothing in `ask.rs`, `main.rs`, `index/lexical.rs`, `index/dense.rs` holds a store file open between requests (grep: no `File::open`, no `OpenOptions` outside `append_after`). No `memmap` in `Cargo.lock`.
11. `src/walk.rs:61`: `rel` is `to_str().replace('\\', "/")` — every id, glob match and registry match keys on a `/`-joined string. `walk_classifies_and_skips` asserts `docs/a.md` and passed on Windows (fact 3). globset normalizes separators itself (`globset-0.4.20/src/pathutil.rs:67–71`); `ignore` does the same for `.gitignore` (`ignore-0.4.33/src/pathutil.rs:25,62`).
12. CRLF: `src/doc/requirements.rs:50` splits with `.lines()` (which strips a trailing `\r`), joins bodies with `"\n"` and trims; `src/code/symbols.rs:34,45,262,347` take every body through `.lines()`/`flatten`/`comment_text`; `src/index/lexical.rs:30` splits tokens on anything not alphanumeric or `-`; `src/enrich.rs:36–41` hashes `label\nbody`. Nothing slices a body out of the raw text by byte offset.
13. `hf-hub-0.5.0/src/api/sync.rs:435–449`: on Windows the snapshot pointer is a symlink if `symlink_file` succeeds and otherwise the blob is **renamed** into place; no privilege is needed for the second path.
14. `ort-sys-2.0.0-rc.13/build/download/dist.tsv:9`: the only feature-less `x86_64-pc-windows-msvc` row is the `directml` build; `build/static_link/mod.rs:60–65` links `dxguid DXCORE DXGI D3D12 DirectML` — load-time imports of the exe. `src/index/embed.rs:127–129` and `cross.rs:76–78` name no execution provider, so ORT runs its CPU provider. `ort-2.0.0-rc.13/src/util/mod.rs:80–88` passes the model path as UTF-16 on Windows.
15. `npm/repograph/bin/repograph.js`: `spawnSync(found.path, argv, {stdio: "inherit"})` on an absolute `.exe` path from `require.resolve` — no shell in between; exit `128` when `status` is `null`.
16. Byte counts (UTF-8, computed here) for `<repo>\.repograph\serve.sock`: `C:\Users\maxim\Documents\projects\beauty-crm` 66; `C:\Users\Максим\Documents\projects\beauty-crm` 73; `C:\Users\Максим Иванов\OneDrive\Documents\projects\beauty-crm` 95; `C:\Users\Максим\OneDrive - Efisco Sp. z o.o\Documents\projects\beauty-crm` 101; a `.worktrees\<branch>` under the Cyrillic Documents case 99. Add 4 for the `\\?\` prefix fact 2 puts on all of them: the worst case here is 105 of the 107 allowed.
17. The runner's temp directory is spelled with an 8.3 component (`C:\Users\RUNNER~1\…`); tests bind through that spelling and `ask` connects through the canonical `\\?\C:\Users\runneradmin\…` spelling; `a_reply_from_another_version_is_ignored` passed (fact 3). So the kernel resolves `sun_path` as a file path — `\\?\` accepted, short and long names one file.
18. The "566 ms" figure in the brief is in no file read here; the only judged bound on the stale-socket fallback is `tests/serve.rs:207`'s `< 5 s` over the whole `ask` process (spawn, store load, connect, answer).

---

## Part A — the working build

### The change

`.github/workflows/release.yml`, `build` job, one step between the archive steps and the release upload, after line 34:

```yaml
      # A dispatch run has no release to put the archive on. It is kept as a run artifact instead,
      # so the file a runner just built and smoked can be fetched with `gh run download` and tried
      # on a real machine before a tag is spent; a tag run skips this and publishes as it did.
      - if: github.event_name == 'workflow_dispatch'
        uses: actions/upload-artifact@v4
        with:
          name: repograph-${{ matrix.target }}
          path: repograph-${{ matrix.target }}.${{ matrix.ext }}
          if-no-files-found: error
          retention-days: 14
```

- Workflow: `release.yml`, not `ci.yml`. The release job builds `--release` with `lto = "thin"` and runs the load smoke; the CI job builds the dev profile and never packs. What Max downloads must be the bytes a tag would publish.
- Which run: `workflow_dispatch` only. A tag run has `event_name == 'push'` and is unchanged line for line.
- Artifact name: `repograph-x86_64-pc-windows-msvc` (the matrix produces the two unix ones too, which costs nothing and gives a Mac build of the same commit). Contents: the one file `repograph-x86_64-pc-windows-msvc.zip`, byte-identical to the release asset a tag would upload.
- Retention: 14 days — long enough to fetch after a weekend, short enough that a stale branch build cannot be mistaken for a release months later.
- `actions/upload-artifact@v4` matches the file's `checkout@v4` era; the current major is v7.0.1 (read from the release list) with the same inputs (`name`, `path`, `if-no-files-found`, `retention-days`), and either is acceptable.

### Fetching it

From the Mac (dispatch, wait, download):

```bash
gh auth switch --user devmaxxx && gh workflow run release.yml --repo devmaxxx/repograph --ref feat/windows-serve
sleep 20; gh auth switch --user devmaxxx && RUN=$(gh run list --repo devmaxxx/repograph --workflow release --event workflow_dispatch --branch feat/windows-serve --limit 1 --json databaseId --jq '.[0].databaseId') && gh run watch --repo devmaxxx/repograph --exit-status "$RUN" | tail -4 && echo "$RUN"
gh auth switch --user devmaxxx && gh run download "$RUN" --repo devmaxxx/repograph -n repograph-x86_64-pc-windows-msvc -D ~/bench/windows-edge-2026-09-06/win
ls -l ~/bench/windows-edge-2026-09-06/win   # repograph-x86_64-pc-windows-msvc.zip
```

On the Windows box (PowerShell; `gh` once via `winget install --id GitHub.cli`, then `gh auth login` — the artifacts API needs a token even on this public repository):

```powershell
gh run download <RUN> --repo devmaxxx/repograph -n repograph-x86_64-pc-windows-msvc -D $env:USERPROFILE\Downloads\repograph
Expand-Archive $env:USERPROFILE\Downloads\repograph\repograph-x86_64-pc-windows-msvc.zip -DestinationPath $env:USERPROFILE\.local\bin -Force
repograph --version
```

`$env:USERPROFILE\.local\bin` is where Claude Code's native installer already put `claude.exe`, so it is on `PATH`. A file that arrives through `gh` carries no mark-of-the-web (Go's HTTP client writes no `Zone.Identifier` stream), so no SmartScreen dialog; see edge 10 for the browser route.

### Artifact or `cargo install --git`?

`cargo install --git https://github.com/devmaxxx/repograph --branch feat/windows-serve --locked` on the Windows box would work — PR #15's author built and tested the branch on Windows 11 with 1.98.0 — and it has one advantage: a rebuild after every push without waiting for a runner. Against it: it needs rustup and the MSVC Build Tools with the Windows SDK (several gigabytes, an interactive installer) to compile oniguruma, tree-sitter and blake3's assembly; it downloads the pyke ONNX Runtime archive during the build; `cargo install --git` builds in a temporary checkout, so the repository's `rust-toolchain.toml` is not what selects the compiler — the user's default toolchain is, which merely has to be ≥ 1.98; and the binary it produces is one nobody but that machine has seen. The artifact is the bytes a runner built, smoked with `--version`, and (after Task 4) drove through the dense arm; it needs nothing installed but `gh`. **Recommend the artifact** for the first working build and for every check that follows; `cargo install --git` only if Max wants to change Windows code on the box itself, and then the CI job is still the judge of what gets merged. The npm route cannot deliver a branch build at all — GitHub Packages spends a version number per publish, and `0.5.0`'s `win32-x64` package does not exist until a tag.

---

## Part B — the edge cases

Each: what breaks, how a user sees it, the fix, what judges it, and what the branch already handles. Tier in brackets: **[must]**, **[should]**, **[doc]**.

### 1. `sh -c` for enrich and rerank — **[must]**

*What breaks.* Fact 5: the program spawned is `sh`; `awk` and `printf` are never spawned by repograph. What PR #15's author hit is `sh` not resolving on a real box: Git for Windows' installer default ("Git from the command line and also from 3rd-party software") puts `<Git>\cmd` on `PATH` — `git.exe` and nothing else — and only the opt-in "Use Git and optional Unix tools" puts `<Git>\bin` and `<Git>\usr\bin` there, which is what the runner image has (fact 3). The error is `spawn \`…\`: program not found` from `enrich.rs:254`'s context, and the author read it as awk missing because the failing tests' commands are awk one-liners.

*How a user sees it.* A beauty-crm user in PowerShell or cmd with the default Git install: `repograph enrich` prints `enrich: spawn \`MAX_THINKING_TOKENS=0 claude -p …\`: program not found` once per batch, generates nothing, and reports every node still left; `ask --rerank` prints `rerank: spawn …: program not found; answering from the fused order` and answers unreranked. A user with no Git for Windows at all sees the same. A user *inside* Git Bash sees neither, because that shell's `PATH` holds `/usr/bin`.

*Options.* (a) Require Git Bash on `PATH` and say so — the branch's README sentence at line 486 — leaves the default install broken and puts the fix on the user. (b) A `cfg(windows)` `cmd /C` or PowerShell rendering: the default command (fact 5) is POSIX — `MAX_THINKING_TOKENS=0` as a prefix is a program name to `cmd`, `--tools ""` survives `cmd`'s parsing only by luck, and every user-written `enrich_command` in a `repograph.toml` would mean one thing on unix and another on Windows; that forks the meaning of a config key by platform, which is worse than a missing shell. Rejected. (c) Make the tests not need awk — they do not, once `sh` is found, since Git for Windows' `usr\bin` carries gawk and `printf` is a builtin; and the tests are not what a user runs. (d) **Find `sh` the way the platform makes it findable**: on `PATH` if it is there (the runner, a Git Bash user — unchanged behaviour), otherwise beside `git` (the default install has `<Git>\cmd\git.exe` on `PATH`, and `<Git>\bin\sh.exe` is two directories over), otherwise where Claude Code says its Git Bash is (`CLAUDE_CODE_GIT_BASH_PATH`, fact 6), otherwise the installer's default locations (`%ProgramFiles%\Git`, `%ProgramW6432%\Git`, `%LOCALAPPDATA%\Programs\Git`); when found by discovery rather than `PATH`, put `<Git>\usr\bin` at the front of the child's `PATH` so `awk`, `wc`, `tr` and the rest resolve inside `sh -c` as they do in Git Bash. A machine with none of these gets one line: `sh is not on PATH and no Git for Windows was found beside git or under Program Files: enrich and rerank run their command under sh, which Git for Windows provides`. Chosen: (d), with the README sentence rewritten. The unix side keeps `Command::new("sh")` verbatim.

*What judges it.* Two things. A unit test of the resolver over a mock tree (`<root>\cmd\git.exe`, `<root>\bin\sh.exe`, `<root>\usr\bin\` as empty files in a tempdir, a synthetic `PATH` holding only `<root>\cmd`) — Windows-only, deterministic. And a CI step in the Windows job that strips `\Git\bin` and `\Git\usr\bin` from `PATH`, asserts `sh` no longer resolves, and runs `cargo test -- enrich:: rerank::` — red on the branch as it stands (the author's failure, reproduced on the runner), green after the change. Task 2.

*Already handled on the branch:* nothing beyond the README sentence, which is replaced.

### 2. `sun_path` and Windows-length paths — **[should]** (the prefix), **[doc]** (the limit)

*What breaks.* Facts 7 and 16: 107 bytes of UTF-8, and the canonicalized path (fact 2) spends four of them on `\\?\`. The realistic beauty-crm cases are 66–105 bytes with the prefix: under the limit, but the OneDrive-with-company-name case has two bytes to spare, and one more `.worktrees\<branch>` level takes it over. Cyrillic is two bytes a letter in UTF-8, which is why the user name matters; socket2 requires UTF-8 (fact 7) and Windows paths are UTF-16, so any real user name converts — the failure mode is length, never encoding.

*How a user sees it.* `serve` exits at once with `Error: bind \\?\C:\Users\…\.repograph\serve.sock` and, beneath it, `path must be shorter than SUN_LEN` (socket2's text, fact 7; std's unix text is the same words). `ask` sees no socket file (`sys::present` false, `serve.rs:137`) and answers in its own process, printing nothing about it — clean, and silent about why there is no server.

*The fix.* Take the prefix off after `canonicalize` on Windows only (`main.rs:353`): std re-applies `\\?\` inside every call that needs it (fact 9), so nothing is lost for long paths, and four bytes come back to the socket, every message reads `C:\…` rather than `\\?\C:\…`, and `git -C` gets the form it is documented to take (edge 12). The relative-path trick (`set_current_dir` then bind `.repograph/serve.sock`) is still rejected: it would change where `enrich`'s command runs from. 8.3 short names were considered and rejected: not generated on every volume, and a socket name that differs by who computed it is a socket nobody finds. The limit itself stays documented; the README sentence gains the byte arithmetic so a user can count their own path.

*What judges it.* Unit tests on the prefix function (Windows-only: `\\?\C:\a\b` → `C:\a\b`, `\\?\UNC\srv\share\x` → `\\srv\share\x`, a plain path untouched, and `fs::metadata` on the stripped canonical temp dir succeeds). A unit test in `serve.rs` that binds under a tempdir joined with `Максим` (UTF-8 in `sun_path`, judged on the Windows job — the runner's own path is ASCII and proves nothing about it). Task 5 and Task 3.

*Already handled:* the README states the limit (line 210); the error names the path.

### 3. OneDrive / Dropbox / Google Drive — **[doc]**

*What is known.* `Documents` under a Microsoft account is frequently redirected into `OneDrive\Documents`, so a repository cloned there has `.repograph\` inside a synced tree. Everything in `.repograph\` is a regular file except `serve.sock`, which `bind` creates as an NTFS reparse point with tag `IO_REPARSE_TAG_AF_UNIX`. A sync client uploads regular files as it sees them close; the store files are rewritten by rename (fact 10), so the client sees a new file each time and uploads it again — for beauty-crm's `vectors.f32` and `graph.json` that is tens of megabytes per `build`, nothing per `ask`. While the client holds a file open for upload, a rename over it can fail (edge 5). Files On-Demand can dehydrate a store file to a placeholder; a `std::fs::read` hydrates it on the next `ask`, at network speed, once.

*What is unverified.* What OneDrive does with a reparse point of a tag it does not know: it cannot upload it, and whether it merely badges the folder with an error, logs and skips it, or attempts to delete it is not something a runner can show — GitHub's Windows image has no signed-in sync client. Nothing was found here that says a client deletes foreign reparse points, and the `serve` code survives deletion anyway (`serve` re-creates on start; a running server whose file vanished is unreachable until restarted — `ask` sees no file and answers itself).

*The fix.* Documented: keep the repository out of a synced tree (`C:\src\…`, or `%USERPROFILE%\projects` when Documents is redirected); if it must live there, expect the store to be synced and the socket file to show as an error in the client. The rename retry (edge 5) is the one code change that helps here. Task 8.

### 4. Ctrl-C and the stale socket — **[must]** (the tests), otherwise handled

*What happens.* A console Ctrl-C ends the process from the control handler; no `Drop` runs (`Unlink`, `serve.rs:320`), the file stays. Same for `Stop-Process`, `taskkill`, and the test harness's `kill()`. The next `serve` (`serve.rs:180–182`): `present && connect ok` → `bail!("another serve answers …")`; otherwise `remove_file` (error ignored) then `bind`. The branch's unit test removes a socket file after its listener is dropped and binds again under the name, and passed on Windows (fact 3) — that is exactly the sweep path with a dead predecessor. A client in between: `present` true → `connect` → refused → `None` → answers here (`serve.rs:137–138`).

*What is not judged.* (a) What a *bare* connect to a stale file costs: fact 18 — the 5 s bound covers a whole process. (b) `remove_file` while this process's own listener is still bound: `run` returns with the listener alive in the accept thread, and `Unlink::drop` then removes the file — after PR #15 that removal has an identity to compare and will actually run on Windows, where the branch's `None` made it a no-op. If `DeleteFileW` refuses a bound socket file, the cost is a stale file the next `serve` sweeps, never a wrong answer; but it should be known, not guessed.

*What judges it.* Two unit tests in `serve.rs`: a timed `sys::connect` against a bound-then-dropped socket (`ConnectionRefused` in well under a second — `WSAECONNREFUSED` maps to that kind), and `remove_file` on a socket file whose listener is alive. Task 3.

### 5. File sharing and locking — **[should]**

*What breaks.* Fact 9: std shares `READ|WRITE|DELETE` on every open, so repograph's own processes never block one another — a concurrent `ask` mid-read of `graph.json` does not stop `write_atomic`'s rename (the `ACCESS_DENIED` it would get is retried with POSIX rename semantics, and a handle with `FILE_SHARE_DELETE` permits that). `serve` holds no store file open between requests and there is no mmap (fact 10). The hole is *other* programs: Windows Search, a sync client mid-upload, an editor with the file open, a backup agent — anything that opens `graph.json` without `FILE_SHARE_DELETE`. Then `MoveFileExW` fails with `ERROR_SHARING_VIOLATION` (os error 32), std returns it as is, and `write_atomic` fails.

*How a user sees it.* `Error: rename graph.json` … `The process cannot access the file because it is being used by another process. (os error 32)` from `build`/`update`/`enrich`, or `serve: … os error 32` in the server's log from a poll's refresh. The store is intact (the temp file was written first, `store.rs:95`), so the next command simply tries again — but a `build` that ran for a minute and lost its save at the last step is a minute lost.

*The fix.* A Windows-only retry in `write_atomic` on os error 32 with exponential back-off to about half a second total — what cargo, rustup and git themselves do on Windows for the same reason — and the unix line untouched. `remove_file` in `wipe` and the stale-socket sweep are left alone: a wipe that fails names its file, and the sweep's failure surfaces as `bind`'s `WSAEADDRINUSE`, both rare and both recoverable by retrying the command.

*What judges it.* A Windows-only unit test: a thread opens the destination with `share_mode(FILE_SHARE_READ)` and holds it 100 ms; `write_atomic` on the main thread succeeds and the new bytes are there. A second test holds it past the budget and asserts the error is os error 32 and the old bytes survive. Task 6.

### 6. CRLF — **[should]** (a pinning test; no code change expected)

*What happens.* Git for Windows' installer default is `core.autocrlf=true`, so a fresh beauty-crm checkout has CRLF in every text file. Fact 12: the doc extractor splits with `.lines()`, which strips a trailing `\r`, and builds bodies by joining with `\n`; the code extractor takes every body through `.lines()`/`flatten`/`comment_text`; tree-sitter counts `\r\n` as one newline, so rows — the `line` fields — agree; BM25 tokens split on anything not alphanumeric; the passage hash is over `label\nbody`. So node ids, labels, bodies, line numbers, BM25 documents and `questions.json`'s hashes are identical to a LF checkout's, and a `questions.json` copied from the Mac still hits on a CRLF tree (no tokens re-spent). What differs is the manifest's blake3 over raw bytes (`walk.rs:85`), and that is per machine: `.repograph/` is gitignored and nothing compares it against a committed expectation. The `\u{feff}` BOM is handled at `requirements.rs:50`. The CI job sets `core.autocrlf false` because the *test fixtures* are compared by bytes — that protects the tests, and says nothing about a CRLF corpus.

*What is not judged.* The property above is by construction, not by test: one future `text[a..b]` slice in an extractor would break it silently.

*The fix.* None to the code. A test in `src/doc/cases.rs` and one in `src/code/cases.rs` that extract the same source with `\n` and with `\r\n` and assert equal nodes and edges. Deterministic, every platform. Task 7. If either fails, that is a real defect the test found, fixed at the one consumer that sliced raw text — and the fix must be the same on unix, since a CRLF file on a Mac is the same bytes.

### 7. Case-insensitive filesystem and separators — handled

Fact 11: `rel` is `/`-joined at the one place it is made, so doc ids (`file:docs/a.md`), symbol ids (`sym:apps/a.ts::x`), `doc_globs`/`skip`/`registries` matches and cross-references are the strings unix makes, and `walk_classifies_and_skips` proved it on the Windows job. `.gitignore` matching is normalized by `ignore`. Case: git checks out the committed case, and two paths differing only by case cannot coexist on NTFS — exactly the situation on the Mac's default APFS, so nothing is Windows-specific. `Path::strip_prefix(repo)` works because `ignore` yields paths under the root it was given, verbatim prefix included. Nothing to do; the README does not need a sentence.

### 8. `cache_root()` and `HOME` — handled, one caveat **[doc]**

Fact 8: on Windows std reads `USERPROFILE`, never `HOME`. Git Bash sets `HOME=/c/Users/x` and inherits `USERPROFILE=C:\Users\x` from Windows, so a native `repograph.exe` started from Git Bash, PowerShell or cmd lands on one cache, `C:\Users\x\.cache\repograph`, and one machine config. `the_cache_root_is_under_the_home_the_platform_names` passed on the Windows job (fact 3). The caveat: `FASTEMBED_CACHE_DIR` and `XDG_CONFIG_HOME`, if a user sets them *inside* Git Bash in MSYS form (`/c/Users/x/…`), reach a native exe unconverted — MSYS converts arguments that look like paths, not arbitrary environment values — and `PathBuf::from("/c/Users/x")` is a path on the current drive. One README line. Task 8.

### 9. Long paths past `MAX_PATH` — **[doc]**

Fact 9: std prefixes `\\?\` on open, stat and readdir when the path needs it, and after `canonicalize` every path the process derives is already in that form (fact 2), so `ignore`/walk, the store, `hf-hub`'s cache (about 130 characters under the profile) and blake3 reads are safe on a deep monorepo. socket2's bind cannot reach 260 — the socket is capped at 107 bytes. `std::process::Command` looks `sh` up on `PATH` and inherits the current directory, so the repository's depth does not enter into it. What is not ours: git itself needs `core.longpaths=true` to check such a tree out, and after Task 5 strips the prefix, `git -C <plain path>` on a >260 path is git's problem in the same way. Documented in one sentence. Task 8.

### 10. Unsigned binary — **[doc]**

The release zip is not Authenticode-signed (a certificate is out of scope). A browser download carries the mark-of-the-web on the zip; Explorer's "Extract All" propagates it to the exe; **double-clicking** such an exe shows SmartScreen's "Windows protected your PC"; running it from a terminal does not — SmartScreen is invoked by the shell, not by `CreateProcess`. `Unblock-File .\repograph.exe` clears the mark. `gh run download`, `curl` and npm write no mark. Defender: an unsigned Rust binary can draw a heuristic false positive (a quarantine, the file "vanishes"); the documented remedy is to restore it from Protection History and add an exclusion. Real-time scanning also touches every write: the 2.2 GB `model.onnx_data` (e5-large, fact from the hub headers: 2 235 363 328 bytes plus a 546 KB graph; the small model is one 470 MB file) is scanned once as it lands, and each `graph.json`/`vectors.f32` rewrite is scanned on close; `Add-MpPreference -ExclusionPath` on `%USERPROFILE%\.cache\repograph` and on the repository's `.repograph` is the optional speed-up. All documented, nothing fixed. Task 8.

### 11. The npm launcher on Windows — handled, one line **[doc]**

Fact 15. `require.resolve` returns `C:\…\node_modules\@devmaxxx\repograph-win32-x64\bin\repograph.exe` (a drive-letter path is a path); `spawnSync` of an absolute `.exe` with `stdio: "inherit"` involves no shell, so spaces and Cyrillic in the install path are inert, and the child inherits the console handle so its UTF-8 output is rendered through `WriteConsoleW` regardless of code page. npm's `repograph.cmd` shim passes `%*` through `cmd` once, which mangles unbalanced quotes and `%` — the same for every npm CLI and not ours to fix. `run.status` is `null` on Windows only when `run.error` is set (already handled above it); a console Ctrl-C ends the child with `STATUS_CONTROL_C_EXIT` (`0xC000013A`), which the launcher passes to `process.exit` verbatim — a large exit code, harmless. The one thing worth a line: PowerShell decodes a *piped or captured* native command's output with `[Console]::OutputEncoding`, which on a Russian-locale Windows is code page 866 unless the system UTF-8 option or a profile line sets UTF-8 — Cyrillic answers piped into `Out-File` or `$x = repograph ask …` come out as mojibake, while Windows Terminal and Claude Code's Bash tool (bytes, UTF-8) are fine. Task 8.

### 12. Everything else found while reading

- **`\\?\` and `git -C`** (fact 2, `changes.rs:144`): `repograph changes` runs `git -C \\?\C:\… diff …`. Whether Git for Windows accepts the verbatim form is not something read here can settle; the CI job runs no `changes` integration. Task 5 adds a CI step that builds the checkout and runs `changes` — red-first on the branch to *see*, then green after the prefix strip. **[should]**
- **`current_exe` and the build stamp** (`serve.rs:102–104`): `GetModuleFileNameW`; the file cannot change under a running image, so the stamp is stable, which is why the replaced-binary test is unix-only. The flip side: `npm i -g`, `cargo install` and `Expand-Archive -Force` over a running `serve` fail with a sharing error — stop `serve` first. **[doc]**
- **`Path::exists` on the reparse point**: avoided (`serve.rs:55` uses `symlink_metadata`); the presence test passed on the Windows job. Handled.
- **Timeouts**: `SO_RCVTIMEO`/`SO_SNDTIMEO` are honoured by winsock and surface as `TimedOut`, which `hello_line` (`serve.rs:267`) already treats with `WouldBlock`. Not judged by any test on any platform (a thirty-second test); left.
- **Threads and the rendezvous channel**: std, platform-neutral; the twelve serve tests passed.
- **ORT's DirectML build without a GPU** (fact 14): no execution provider is registered, so ORT runs the CPU provider; DirectML is a link-time fact, not a runtime one. What it does cost: `DirectML.dll`, `dxcore.dll`, `d3d12.dll`, `dxgi.dll` are load-time imports, inbox since Windows 10 1903 — so the floor is **Windows 10 1903 / Windows 11 / Server 2022+**, above Rust's 1809 (Server 2019 and 10 LTSC 2019 fail at load with `STATUS_DLL_NOT_FOUND`), and Server Core is out. The `--version` smoke on a GPU-less Azure runner proves loading; only an actual session proves inference on the CPU provider — Task 4's dense smoke on that same GPU-less runner is that proof. **[must]** (the proof), **[doc]** (the floor)
- **Windows Defender Firewall on `AF_UNIX`**: the firewall filters IP traffic at WFP's ALE layers; `afunix.sys` is not an IP transport and registers no WFP callouts, so there is neither a prompt nor a block — consistent with the runner (firewall on, no interaction possible, eleven socket tests green). Reasoned, not proven; nothing to do. **[doc]** (one clause)
- **`serve` in the background from PowerShell**: `&` is not a job operator there; `Start-Process repograph -ArgumentList 'serve','--idle','86400' -WindowStyle Hidden` is, and `--idle` or `Stop-Process -Name repograph` ends it (no `Drop`, so the file stays for the next sweep — edge 4). **[doc]**
- **`--repo` from Git Bash**: MSYS converts a `/c/Users/…` argument to `C:\Users\…` before the exe sees it; `canonicalize` then resolves 8.3 spellings and symlinks so server and client name one socket however each was started (fact 17). Handled.
- **hf-hub on a box without Developer Mode** (fact 13): the symlink attempt fails and the blob is renamed into the snapshot; ORT resolves `model.onnx_data` beside `model.onnx` in that same directory. The runner (admin) very likely takes the symlink branch and Max's box the rename branch; Task 4 states which branch it exercised. **[doc]**
- **The README after PR #15**: lines 205–208 say "Windows gives std no identity to read there" — false once #15 lands. Task 8.

### The tiers

| Tier | Item | Task |
|---|---|---|
| must | the downloadable build | 1 |
| must | `sh` found on a default Git for Windows install; the CI step that reproduces the user's box | 2 |
| must | the identity from PR #15 judged on Windows; remove-while-bound; refused-connect timing; a non-ASCII socket directory | 3 |
| must | the dense arm opened and driven through the socket on a Windows runner | 4 |
| should | the `\\?\` prefix off the repository path; `changes` on Windows judged | 5 |
| should | rename retried on a sharing violation | 6 |
| should | CRLF pinned by a test | 7 |
| doc | sync folders, Ctrl-C, SmartScreen/Defender, console encoding, the OS floor, long paths, stopping `serve` before an update, MSYS environment values, the identity sentence | 8 |

## Verification ledger

| Claim | Verified how | Or judged by |
|---|---|---|
| Only `sh` (and `git`) are spawned; the default commands are POSIX syntax | read `enrich.rs:252`, `changes.rs:144`, `config.rs:43–44` | — |
| The runner resolves `sh`/`awk`; a default Git install does not put them on `PATH` | fact 3 (log); Git for Windows installer behaviour is not readable here | Task 2 Step 3's stripped-`PATH` step, red then green |
| `sh` found beside `git` finds `awk` when `usr\bin` is prepended | not verifiable here | Task 2 Step 3 |
| `canonicalize` returns `\\?\…` on Windows | std source (`fs/windows.rs:1593`) | Task 5's unit test on the stripped form |
| `\\?\` and 8.3 spellings both bind/connect the same socket file | fact 17 (tests passed through both spellings) | — |
| 107-byte cap, UTF-8 required and expected | socket2 source (fact 7) | Task 3's Cyrillic-directory test on Windows |
| Byte counts of the beauty-crm paths | computed here (fact 16) | — |
| `home_dir` reads `USERPROFILE` only | std source (fact 8) | `the_cache_root_is_under_the_home_the_platform_names`, already green |
| std shares `READ\|WRITE\|DELETE`; rename retries only on `ACCESS_DENIED`; delete is `DeleteFileW` | std source (fact 9) | Task 6's held-open tests on Windows |
| No store file is held open between requests; no mmap | grep of `store.rs`, `ask.rs`, `dense.rs`; `Cargo.lock` | — |
| CRLF-neutral extraction, tokens and passage hashes | read `requirements.rs:50`, `symbols.rs`, `lexical.rs:30`, `enrich.rs:36–41` | Task 7's tests, every platform |
| `rel` is `/`-joined; globs and ids match unix | `walk.rs:61`; `walk_classifies_and_skips` green on Windows | — |
| hf-hub falls back to rename without symlink privilege | hf-hub source (fact 13) | which branch the runner took: Task 4 Step 4 reads it off the cache tree |
| The Windows ORT build is DirectML, linked with DX libs, no EP registered | ort-sys `dist.tsv`, `static_link/mod.rs`; `embed.rs:127` | Task 4's session on a GPU-less runner |
| A bare connect to a stale socket file is refused promptly on Windows | not verified (fact 18) | Task 3's timed unit test |
| `remove_file` succeeds on a socket file whose listener is alive | not verified | Task 3's unit test |
| `git -C` accepts `\\?\` | not verified | Task 5 Step 1, red-first |
| Claude Code names its Git Bash via `CLAUDE_CODE_GIT_BASH_PATH` | its setup page (fetched) | — |
| SmartScreen, Defender, OneDrive, firewall behaviour | reasoned from platform documentation; nothing here can run them | documented only |
| The artifact is downloadable with `gh run download -n …` | `gh run download --help` (a single named artifact extracts into `-D`) | Task 1 Step 3 |

---

## File Structure

| File | Responsibility | Tasks |
|---|---|---|
| `.github/workflows/release.yml` | the dispatch-only artifact step | 1 |
| `src/enrich.rs` | `shell()`: `sh` on unix; on Windows, `sh` from `PATH`, beside `git`, from `CLAUDE_CODE_GIT_BASH_PATH`, or under Program Files, with `usr\bin` on the child's `PATH`; the resolver's unit test | 2 |
| `.github/workflows/ci.yml` | the stripped-`PATH` enrich step; the model cache and the dense smoke; the `changes` step | 2, 4, 5 |
| `src/serve.rs` | four unit tests: identity on every platform, remove-while-bound, refused-connect timing, a non-ASCII directory | 3 |
| `tests/serve.rs` | the ignored dense smoke through the socket on the small model | 4 |
| `src/main.rs` | `plain()`: the verbatim prefix off the canonical repository path, Windows only | 5 |
| `src/store.rs` | `rename_over()`: the sharing-violation retry, Windows only; two tests | 6 |
| `src/doc/cases.rs`, `src/code/cases.rs` | the CRLF pinning cases | 7 |
| `README.md` | the Windows section; the identity sentence; the `sh` sentence; the socket arithmetic | 8 |

---

### Task 0: After PR #15, the baseline

**Files:** none in the repo; `$M/task0.txt`.

- [ ] **Step 1: Wait for #15, then fast-forward the worktree**

```bash
gh auth switch --user devmaxxx && gh pr view 15 --repo devmaxxx/repograph --json state,mergeCommit --jq '"\(.state) \(.mergeCommit.oid // "")"'
```

Expected: `MERGED <sha>`. Anything else: stop; the rebase is still in flight and the worktree is not this plan's to touch.

```bash
W=/Users/max/Documents/projects/repograph/.worktrees/windows-serve
git -C "$W" status --porcelain; git -C "$W" fetch origin && git -C "$W" pull --ff-only origin feat/windows-serve && git -C "$W" log --oneline -3
grep -n 'GetFileInformationByHandle' "$W/src/serve.rs" | head -2
grep -n 'cfg(unix)' "$W/src/serve.rs"
```

Expected: a clean tree; the tip is #15's merge; `GetFileInformationByHandle` is in `serve.rs`. Note whether `#[cfg(unix)]` still sits above `a_bound_socket_file_carries_an_identity_and_a_free_name_does_not` — Task 3 removes it if so.

- [ ] **Step 2: The unix baseline and the plan documents on the branch**

```bash
M=/Users/max/bench/windows-edge-2026-09-06; mkdir -p "$M"
cd "$W" && cargo clippy --all-targets -- -D warnings 2>&1 | tail -1 && cargo test --release 2>&1 | grep 'test result' | tee "$M/task0.txt"
cp /Users/max/Documents/projects/repograph/docs/superpowers/plans/2026-09-06-windows-serve-transport.md "$W/docs/superpowers/plans/" 2>/dev/null; cp /Users/max/Documents/projects/repograph/docs/superpowers/plans/2026-09-06-windows-edge-cases-and-build.md "$W/docs/superpowers/plans/"
git -C "$W" add docs/superpowers/plans/2026-09-06-windows-serve-transport.md docs/superpowers/plans/2026-09-06-windows-edge-cases-and-build.md
git -C "$W" commit -F - <<'MSG'
docs(plans): the Windows transport plan and the edge-case plan on the branch
MSG
```

Expected: clippy clean; `446 passed` unit (the count fact 3 recorded on Windows; unix may differ by the linux-only walk test — record what it says) and `12 passed` for `tests/serve.rs`. Both counts are what every later task is held to, plus its own additions.

---

### Task 1: The downloadable build

**Files:**
- Modify: `.github/workflows/release.yml`

- [ ] **Step 1: The step**

Insert after line 34 (the `Compress-Archive` line's step) and before `- if: startsWith(github.ref, 'refs/tags/')`, the YAML block under *Part A — The change* above, verbatim.

- [ ] **Step 2: Commit, push, dispatch, watch**

```bash
git -C "$W" add .github/workflows/release.yml
git -C "$W" commit -F - <<'MSG'
build(release): a dispatch run keeps its archives as run artifacts

The dry run compiled and smoked repograph.exe and kept nothing, because the upload is a release
step and a branch has no release. The archive is now an artifact of the run for fourteen days, so
the file a runner proved can be fetched and tried on a real machine; a tag run is unchanged.
MSG
git -C "$W" push
gh auth switch --user devmaxxx && gh workflow run release.yml --repo devmaxxx/repograph --ref feat/windows-serve
sleep 20; gh auth switch --user devmaxxx && RUN=$(gh run list --repo devmaxxx/repograph --workflow release --event workflow_dispatch --branch feat/windows-serve --limit 1 --json databaseId --jq '.[0].databaseId') && gh run watch --repo devmaxxx/repograph --exit-status "$RUN" | tail -6 | tee "$M/task1-release.txt" && echo "$RUN" >> "$M/task1-release.txt"
```

Expected: three `build` jobs green, `npm` skipped, and each build job's log ends with `Artifact repograph-<target> has been successfully uploaded`.

- [ ] **Step 3: Fetch it here, and prove the artifact is the archive**

```bash
gh auth switch --user devmaxxx && gh run download "$RUN" --repo devmaxxx/repograph -n repograph-x86_64-pc-windows-msvc -D "$M/win" && ls -l "$M/win"
unzip -l "$M/win/repograph-x86_64-pc-windows-msvc.zip"
gh auth switch --user devmaxxx && gh run view "$RUN" --repo devmaxxx/repograph --json url --jq .url | tee -a "$M/task1-release.txt"
```

Expected: one file, `repograph-x86_64-pc-windows-msvc.zip`, listing `repograph.exe` at its root. Record the run URL. Report DONE with the PowerShell fetch block from *Part A — Fetching it* and the run id, so Max can pull this first build onto the box now — the later tasks change the exe and a second dispatch (Task 8 Step 4) replaces it.

---

### Task 2: `sh` on a default Git for Windows install

**Files:**
- Modify: `src/enrich.rs`, `.github/workflows/ci.yml`

**Interfaces:**
- Produces: `fn shell() -> Result<Command>` in `enrich.rs`, used by `run_command` (and through it by `rerank.rs`); on Windows also `fn git_sh(path_dirs: &[PathBuf], bash_hint: Option<&Path>, program_dirs: &[PathBuf]) -> Option<PathBuf>`, the pure resolver the test drives.

- [ ] **Step 1: The CI step that reproduces the user's box, red first**

Append to the `windows` job in `.github/workflows/ci.yml`, after `- run: cargo test`:

```yaml
      # A default Git for Windows install puts `git` on PATH and not `sh`; the runner image has
      # both. This is a user's box: Git's shell directories are taken off PATH, and the tests that
      # spawn the shell are run again.
      - shell: pwsh
        run: |
          $env:PATH = ($env:PATH -split ';' | Where-Object { $_ -notmatch '\\Git\\(usr\\)?bin\\?$' }) -join ';'
          if (Get-Command sh -ErrorAction SilentlyContinue) { throw "sh is still on PATH: $((Get-Command sh).Source)" }
          if (-not (Get-Command git -ErrorAction SilentlyContinue)) { throw "git left PATH too; the pattern is wrong" }
          cargo test -- enrich:: rerank::
```

```bash
git -C "$W" add .github/workflows/ci.yml
git -C "$W" commit -F - <<'MSG'
ci: enrich and rerank tests with Git's shell directories off PATH

Red on purpose: this is the PATH a default Git for Windows install leaves, and the shell those
tests spawn is not on it. The commit that finds the shell beside git turns it green.
MSG
git -C "$W" push
gh auth switch --user devmaxxx && gh run watch --repo devmaxxx/repograph --exit-status $(gh run list --repo devmaxxx/repograph --branch feat/windows-serve --workflow ci --limit 1 --json databaseId --jq '.[0].databaseId'); true
gh auth switch --user devmaxxx && gh run view --repo devmaxxx/repograph --log-failed $(gh run list --repo devmaxxx/repograph --branch feat/windows-serve --workflow ci --limit 1 --json databaseId --jq '.[0].databaseId') | grep -m5 'program not found\|FAILED\|panicked' | tee "$M/task2-red.txt"
```

Expected: `test` green, `windows` red at the new step with `spawn \`awk …\`: program not found` (or the OS's own words for it) in the failed tests — the author's eight, reproduced on a runner. If the step is *green*, the pattern did not strip the directory that holds `sh.exe`: print `$env:PATH` in the step, fix the pattern, and only then continue — a step that cannot go red judges nothing.

- [ ] **Step 2: The resolver's failing unit test**

Append to `src/enrich.rs`'s `mod tests`:

```rust
    /// The shell a default Git for Windows install leaves findable: `git` on PATH under `cmd\`,
    /// `sh` two directories over. A synthetic tree and a synthetic PATH, so the test is about the
    /// lookup and not about what this machine has installed.
    #[cfg(windows)]
    #[test]
    fn a_shell_that_is_not_on_path_is_found_beside_git_and_then_under_program_files() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("Git");
        for f in ["cmd\\git.exe", "bin\\sh.exe", "usr\\bin\\awk.exe"] {
            let p = root.join(f);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(&p, b"").unwrap();
        }
        let elsewhere = dir.path().join("elsewhere");
        std::fs::create_dir_all(&elsewhere).unwrap();
        assert_eq!(git_sh(&[elsewhere.clone(), root.join("cmd")], None, &[]), Some(root.join("bin").join("sh.exe")), "beside git");
        assert_eq!(git_sh(&[elsewhere.clone()], None, &[dir.path().to_path_buf()]), Some(root.join("bin").join("sh.exe")), "under a program directory");
        assert_eq!(git_sh(&[elsewhere.clone()], Some(&root.join("bin").join("bash.exe")), &[]), Some(root.join("bin").join("sh.exe")), "from the bash Claude Code names");
        assert_eq!(git_sh(&[elsewhere], None, &[]), None, "nothing to find");
    }
```

- [ ] **Step 3: `shell()` and the call site**

In `src/enrich.rs`, replace `std::process::Command::new("sh")` at line 252 with `shell()?`, and add above `run_command`:

```rust
/// The shell the configured command runs under: `sh`, on every platform. The commands in
/// `repograph.toml` are written in its syntax — a `VAR=value` prefix, `""` for an empty argument —
/// and a `cmd /C` or PowerShell rendering on Windows would make one key mean two things.
#[cfg(unix)]
fn shell() -> Result<std::process::Command> { Ok(std::process::Command::new("sh")) }

/// The same `sh`, which on Windows is Git for Windows'. Its default install puts `git` on PATH and
/// not `sh` — the Unix tools are an opt-in — so a `sh` PATH does not resolve is looked for beside
/// `git`, at the bash Claude Code names for its own Bash tool, and where the installer puts it;
/// found that way, Git's `usr\bin` goes on the child's PATH too, since that is where `awk` and the
/// rest of what a command may call live.
#[cfg(windows)]
fn shell() -> Result<std::process::Command> {
    use std::path::PathBuf;
    let path_dirs: Vec<PathBuf> = std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect()).unwrap_or_default();
    if path_dirs.iter().any(|d| d.join("sh.exe").is_file()) { return Ok(std::process::Command::new("sh")); }
    let bash = std::env::var_os("CLAUDE_CODE_GIT_BASH_PATH").map(PathBuf::from);
    let program_dirs: Vec<PathBuf> = [("ProgramFiles", ""), ("ProgramW6432", ""), ("LOCALAPPDATA", "Programs")].iter()
        .filter_map(|(k, sub)| std::env::var_os(k).map(|v| PathBuf::from(v).join(sub))).collect();
    let sh = git_sh(&path_dirs, bash.as_deref(), &program_dirs)
        .context("`sh` is not on PATH and no Git for Windows was found beside `git` or under Program Files: enrich and rerank run their command under sh, which Git for Windows provides")?;
    let mut cmd = std::process::Command::new(&sh);
    let root = sh.parent().and_then(std::path::Path::parent).map(std::path::Path::to_path_buf).unwrap_or_default();
    cmd.env("PATH", std::env::join_paths(std::iter::once(root.join("usr").join("bin")).chain(path_dirs))?);
    Ok(cmd)
}

/// `<Git>\bin\sh.exe` for the first `<Git>` that has one: the parent or grandparent of a directory
/// on `path_dirs` holding `git.exe` (`cmd\` and `mingw64\bin\` are both one install), the
/// grandparent of `bash_hint`, then `<dir>\Git` for each of `program_dirs`.
#[cfg(windows)]
fn git_sh(path_dirs: &[std::path::PathBuf], bash_hint: Option<&std::path::Path>, program_dirs: &[std::path::PathBuf]) -> Option<std::path::PathBuf> {
    let sh_in = |root: &std::path::Path| { let p = root.join("bin").join("sh.exe"); p.is_file().then_some(p) };
    let beside_git = path_dirs.iter().filter(|d| d.join("git.exe").is_file())
        .flat_map(|d| d.ancestors().skip(1).take(2).map(std::path::Path::to_path_buf).collect::<Vec<_>>());
    let named = bash_hint.and_then(|b| b.parent()?.parent()).map(std::path::Path::to_path_buf);
    let installed = program_dirs.iter().map(|d| d.join("Git"));
    beside_git.chain(named).chain(installed).find_map(|root| sh_in(&root))
}
```

`anyhow::Context` is already imported at the top of the file. `rerank.rs` calls `run_command` and needs no change.

- [ ] **Step 4: Unix green, pushed, Windows green**

```bash
cd "$W" && cargo clippy --all-targets -- -D warnings 2>&1 | tail -1 && cargo test --release 2>&1 | grep 'test result'
git diff src/enrich.rs | grep '^[-+]' | grep -v '^[-+][-+]' | grep -c 'Command::new("sh")'
git -C "$W" add src/enrich.rs
git -C "$W" commit -F - <<'MSG'
fix(enrich): the shell is found where a default Git for Windows install leaves it

The commands run under sh on every platform, and on Windows that is Git for Windows' — which the
installer does not put on PATH by default, only git. A sh PATH cannot resolve is looked for beside
git, at the bash Claude Code names, and under Program Files, with Git's usr\bin put on the
command's PATH so awk and the rest resolve as they do in Git Bash. Unix spawns the same sh it did.
MSG
git -C "$W" push
gh auth switch --user devmaxxx && gh run watch --repo devmaxxx/repograph --exit-status $(gh run list --repo devmaxxx/repograph --branch feat/windows-serve --workflow ci --limit 1 --json databaseId --jq '.[0].databaseId') | tail -4 | tee "$M/task2-ci.txt"
```

Expected: unix counts unchanged (the new test is Windows-only; the grep prints `2` — one removed line, one added, both the unix `Command::new("sh")`); both jobs green, the stripped-`PATH` step showing the enrich and rerank tests passing. Record the run URL.

Triage if the stripped step is still red: the log's `spawn` context names the command. If it says `program not found` for `sh` itself, `git_sh` found nothing — print `$env:PATH` and `$env:ProgramFiles` from the step, and check which of the three candidate roots the runner's Git is at (`C:\Program Files\Git`). If it says `awk: not found` from `sh`, `usr\bin` did not reach the child: the `PATH` env is set on the `Command`, and `sh -c` inherits it — verify the join and that `root` is the grandparent of `sh.exe`, not its parent.

---

### Task 3: The socket file's identity and its costs, judged on Windows

**Files:**
- Modify: `src/serve.rs` (`mod tests` only)

**Interfaces:** none new; four tests against `sys`.

- [ ] **Step 1: The tests**

In `src/serve.rs` `mod tests`: if `#[cfg(unix)]` (and its "Unix only, because `sys::id` is `None` on Windows" sentence) is still above `a_bound_socket_file_carries_an_identity_and_a_free_name_does_not` after #15, remove both and extend the test; then add three tests. The four, in full:

```rust
    /// The identity `Unlink` compares before it removes anything. On Windows it is read off a
    /// handle, and a name unlinked and bound again is a new file by construction — NTFS bumps the
    /// record's sequence number — so that is asserted there; unix may hand the inode straight back.
    #[test]
    fn a_bound_socket_file_carries_an_identity_and_a_free_name_does_not() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("serve.sock");
        assert!(sys::id(&path).is_none());
        let first = sys::bind(&path).unwrap();
        let id = sys::id(&path).expect("a bound socket file has an identity");
        drop(first);
        std::fs::remove_file(&path).unwrap();
        let _second = sys::bind(&path).unwrap();
        if cfg!(windows) { assert_ne!(sys::id(&path), Some(id), "bound again under the name, another file"); }
    }

    /// What `run` does on the way out: the listener is still alive on its thread when the guard
    /// removes the file. If the platform refused this, the cost would be a stale file for the
    /// next `serve` to sweep — worth knowing rather than guessing.
    #[test]
    fn the_socket_file_is_removed_while_its_listener_is_still_bound() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("serve.sock");
        let _listener = sys::bind(&path).unwrap();
        std::fs::remove_file(&path).unwrap();
        assert!(!sys::present(&path));
    }

    /// The whole reason for a socket file over a port: a name nobody listens on is refused, at
    /// once, not accepted by a stranger and waited out.
    #[test]
    fn a_connect_to_a_socket_nobody_listens_on_is_refused_at_once() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("serve.sock");
        drop(sys::bind(&path).unwrap());
        let started = std::time::Instant::now();
        let err = sys::connect(&path).expect_err("nobody listens");
        assert!(started.elapsed() < std::time::Duration::from_secs(1), "refused after {:?}", started.elapsed());
        assert_eq!(err.kind(), std::io::ErrorKind::ConnectionRefused, "{err}");
    }

    /// A user's name is in the socket's path, and on Windows the path crosses into `sun_path` as
    /// UTF-8: two bytes a Cyrillic letter, and a conversion the runner's own ASCII path never makes.
    #[test]
    fn a_socket_binds_and_connects_under_a_directory_that_is_not_ascii() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("Максим");
        std::fs::create_dir(&home).unwrap();
        let path = home.join("serve.sock");
        let _listener = sys::bind(&path).unwrap();
        assert!(sys::present(&path));
        sys::connect(&path).unwrap();
    }
```

- [ ] **Step 2: Unix green, pushed, Windows green**

```bash
cd "$W" && cargo clippy --all-targets -- -D warnings 2>&1 | tail -1 && cargo test --release serve::tests 2>&1 | grep 'test result'
git -C "$W" add src/serve.rs
git -C "$W" commit -F - <<'MSG'
test(serve): the socket file's identity and its costs, on every platform

The identity test runs on Windows now that there is one to read; a socket file is removed while
its listener is alive, which is the order run's exit has; a connect nobody accepts is refused in
under a second; and a socket binds under a Cyrillic directory name, which a runner's ASCII path
never checks.
MSG
git -C "$W" push
gh auth switch --user devmaxxx && gh run watch --repo devmaxxx/repograph --exit-status $(gh run list --repo devmaxxx/repograph --branch feat/windows-serve --workflow ci --limit 1 --json databaseId --jq '.[0].databaseId') | tail -4 | tee "$M/task3-ci.txt"
```

Expected: `5 passed` in `serve::tests` on unix; both jobs green; the Windows log lists all five `serve::tests` as `ok`. Record the run URL.

Triage, each a separate commit: the refused-connect test failing on its `kind` — record the kind the log shows (`Other` with `WSAECONNREFUSED` in the message is possible) and relax to `is_err()` plus a message check, keeping the timing assertion, which is the property; failing on *timing* reopens the transport decision and is reported, not patched. `remove_file` while bound failing — leave the test in as `#[cfg(unix)]` with the finding in its comment, and add one README sentence: a server's own exit leaves the file on Windows and the next one sweeps it. The Cyrillic test failing on `bind` with `InvalidInput` — socket2 refused the path; record the message and report.

---

### Task 4: The dense arm on a Windows runner

**Files:**
- Modify: `tests/serve.rs`, `.github/workflows/ci.yml`

**Interfaces:**
- Produces: an `#[ignore]`d integration test, run explicitly by the Windows job with the small model in `FASTEMBED_CACHE_DIR` (`embed.rs:73` honours it), the cache kept across runs by `actions/cache`.

Background the brief cannot know: no job on any platform opens the embedding model — `bench` is `--no-dense`, the unit tests that touch `ort` are ignored without an exported model, and the twelve serve tests build with `--no-dense`. For Max's target the dense arm is the whole point: e5-large through ORT's CPU provider on Windows, the tokenizer with its 16 MB vocabulary, `hf-hub`'s cache on NTFS, and the resident answer to a fused question. The small model (470 MB, one file) stands in for the large one (2.2 GB): the same code path, a quarter of the download. The runner has no GPU, which is what makes it the right judge of "no GPU is needed".

- [ ] **Step 1: The test, on the Mac first**

Append to `tests/serve.rs`:

```rust
/// The dense arm end to end: a store built with vectors, a fused answer in this process, and
/// the same answer from a resident server — on the small model, in whatever cache the environment
/// names. Ignored by default because it wants 470 MB on disk; the Windows CI job runs it with the
/// cache restored between runs, and it is the one place the ONNX Runtime build the Windows
/// binary links opens a session and embeds.
#[test]
#[ignore = "needs the small model in FASTEMBED_CACHE_DIR or ~/.cache/repograph/fastembed; the Windows CI job runs it"]
fn a_fused_answer_is_resident_on_the_small_model() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("docs")).unwrap();
    std::fs::write(dir.path().join("docs/pay.md"), "**FR-PAY-1 · MUST · Штраф за отмену**\n\nШтраф списывается сам (INV-1).\n\n**INV-1 · MUST · Деньги не сгорают**\n\nОтмена не сжигает деньги.\n").unwrap();
    std::fs::write(dir.path().join("docs/cal.md"), "**FR-CAL-1 · MUST · Перенос визита**\n\nПеренос не считается отменой.\n").unwrap();
    std::fs::write(dir.path().join("repograph.toml"), "id_families = [\"FR-PAY\", \"FR-CAL\", \"INV\"]\nembed_model = \"intfloat/multilingual-e5-small\"\n").unwrap();
    let out = repograph().arg("--repo").arg(dir.path()).arg("build").output().unwrap();
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "{err}");
    assert!(!err.contains("dense: model unavailable"), "the model did not open: {err}");
    assert!(std::fs::metadata(dir.path().join(".repograph/vectors.f32")).map(|m| m.len() > 0).unwrap_or(false), "no vectors were written");
    let (want, err) = ask_in(FUSED, dir.path(), &["--no-serve"], &["штраф"]);
    assert!(want.contains("FR-PAY-1"), "{want}");
    assert!(!err.contains("dense: model unavailable"), "{err}");
    let mut server = serve_in(FUSED, dir.path(), &["--every", "3600", "--idle", "120"]);
    let start = Instant::now();
    let (through, _) = loop {
        let (out, err) = ask_in(FUSED, dir.path(), &[], &["штраф"]);
        if err.contains("serve: answered by the resident process") { break (out, err); }
        assert!(start.elapsed() < Duration::from_secs(120), "the fused server never answered over the socket: {err}");
        std::thread::sleep(Duration::from_millis(250));
    };
    assert_eq!(through, want, "the resident fused answer is the process's fused answer");
    // The number the whole exercise is for, on the record in the log: eleven resident asks, the
    // median, against whatever the unix run of this test prints.
    let mut times: Vec<u128> = (0..11).map(|_| { let t = Instant::now(); ask_in(FUSED, dir.path(), &[], &["штраф"]); t.elapsed().as_millis() }).collect();
    times.sort();
    eprintln!("resident fused ask, median of 11: {} ms", times[5]);
    server.kill().unwrap();
    let _ = server.wait();
}
```

Run on the Mac, where the small model is cached (the bench notes say the campaign copies were embedded with it):

```bash
cd "$W" && cargo test --release --test serve -- --ignored --nocapture a_fused_answer_is_resident_on_the_small_model 2>&1 | grep -E 'median|test result|panicked' | tee "$M/task4-mac.txt"
```

Expected: `1 passed` and a `median … ms` line. If the model is not cached, this downloads it once into `~/.cache/repograph/fastembed`; that is fine.

- [ ] **Step 2: The job**

In `.github/workflows/ci.yml`, the `windows` job gains a job-level `env` and two steps at its end:

```yaml
  windows:
    runs-on: windows-latest
    env:
      # The small model, kept between runs under the workspace: the dense arm is opened here and
      # nowhere else in CI, on a runner with no GPU.
      FASTEMBED_CACHE_DIR: ${{ github.workspace }}\.model-cache
    steps:
      …existing steps…
      - uses: actions/cache@v4
        with:
          path: ${{ github.workspace }}\.model-cache
          key: e5-small-onnx-1
      - run: cargo test --test serve -- --ignored --nocapture a_fused_answer_is_resident_on_the_small_model
```

`actions/cache@v4` — the current major is v6.1.0 with the same `path`/`key` inputs; either. The key is a hand-bumped constant: the model does not move, and bumping it is how a broken cache is thrown away.

- [ ] **Step 3: Commit, push, watch — twice**

```bash
git -C "$W" add tests/serve.rs .github/workflows/ci.yml
git -C "$W" commit -F - <<'MSG'
test(serve): the dense arm through the socket on the small model, run by the Windows job

No job opened the embedding model on any platform. This test builds with vectors, asks fused in
its own process and through a resident server, asserts the same bytes, and prints the resident
median; it is ignored by default and the Windows job runs it with the model cached between runs,
on a runner with no GPU.
MSG
git -C "$W" push
gh auth switch --user devmaxxx && gh run watch --repo devmaxxx/repograph --exit-status $(gh run list --repo devmaxxx/repograph --branch feat/windows-serve --workflow ci --limit 1 --json databaseId --jq '.[0].databaseId') | tail -4 | tee "$M/task4-ci-1.txt"
gh auth switch --user devmaxxx && gh run view --repo devmaxxx/repograph --log $(gh run list --repo devmaxxx/repograph --branch feat/windows-serve --workflow ci --limit 1 --json databaseId --jq '.[0].databaseId') | grep -E 'median of 11|Cache (saved|restored|not found)' | tee -a "$M/task4-ci-1.txt"
```

Expected on the first run: `Cache not found`, the model downloaded (the `hf-hub` progress lines), `1 passed`, a `median` line, and `Cache saved` at the end. Then re-run the workflow (`gh run rerun <id>` or an empty commit) and expect `Cache restored` and the same `1 passed` — the second run is the one that judges the cache's handling of whatever `hf-hub` wrote (a symlink tree on an admin runner, or renamed blobs). Record both URLs and both medians.

- [ ] **Step 4: Which branch of `hf-hub` the runner took**

Add, temporarily or permanently, a step after the test: `Get-ChildItem -Recurse $env:FASTEMBED_CACHE_DIR | Where-Object { $_.Attributes -match 'ReparsePoint' } | Select-Object FullName` — a listing means symlinks (the admin path); an empty one means renamed blobs (the path Max's box will take without Developer Mode). Record which in `$M/task4-ci-1.txt`; the README sentence in Task 8 says which one the runner proved.

Contingency, by the log: the session failing to open (`ort` error text) on the runner is the finding this task exists to make — stop and report with the log lines; the `--version` smoke proved loading only. A second run failing on a restored cache while the first passed: bump the key to `e5-small-onnx-2` and add `enableCrossOsArchive: false` is not the issue (same OS); if `hf-hub` errors on a dangling pointer, replace the cache step with nothing and let each run download — 470 MB at runner bandwidth is under a minute — and say so in the step's comment.

---

### Task 5: The repository path without its verbatim prefix; `changes` on Windows

**Files:**
- Modify: `.github/workflows/ci.yml`, `src/main.rs`

- [ ] **Step 1: The `changes` step, to see**

Append to the `windows` job:

```yaml
      # `changes` hands the repository path to `git -C`. The checkout is the corpus: built once
      # lexically, then diffed against HEAD, which a fresh clone can always do.
      - shell: pwsh
        run: |
          cargo run --release --quiet -- --no-dense --repo "$env:GITHUB_WORKSPACE" build
          cargo run --release --quiet -- --no-dense --repo "$env:GITHUB_WORKSPACE" changes --base HEAD
```

Commit (`ci: changes on the Windows checkout` — with a body saying it is there to find out whether git takes the verbatim path), push, watch. Record in `$M/task5-red.txt` whether it went red with `fatal:` from git (then the prefix is the cause) or green (git took `\\?\`, and Step 2 is still worth its four bytes). Either way, continue.

- [ ] **Step 2: The failing unit test**

`src/main.rs` already has a `mod tests` (line 490 on the branch tip); append inside it, the `cfg` on the test rather than on the module:

```rust
    #[cfg(windows)]
    #[test]
    fn a_canonical_path_is_stated_without_its_verbatim_prefix() {
        use std::path::PathBuf;
        assert_eq!(super::plain(PathBuf::from(r"\\?\C:\a\b")), PathBuf::from(r"C:\a\b"));
        assert_eq!(super::plain(PathBuf::from(r"\\?\UNC\srv\share\x")), PathBuf::from(r"\\srv\share\x"));
        assert_eq!(super::plain(PathBuf::from(r"C:\a\b")), PathBuf::from(r"C:\a\b"));
        let temp = super::plain(std::env::temp_dir().canonicalize().unwrap());
        assert!(!temp.to_string_lossy().starts_with(r"\\?\"), "{}", temp.display());
        assert!(std::fs::metadata(&temp).unwrap().is_dir(), "the stripped path still names the directory");
    }
```

- [ ] **Step 3: `plain()` at the one site**

`src/main.rs:353`: `let repo = plain(cli.repo.canonicalize()?);`, and above `fn main`:

```rust
/// The path as a user would write it. `canonicalize` on Windows answers in the `\\?\C:\…` form,
/// which std puts back itself wherever a call needs it; carried around instead it is four bytes
/// of the socket name's budget, a prefix in every message, and a spelling `git -C` is not
/// promised to take.
#[cfg(windows)]
fn plain(p: PathBuf) -> PathBuf {
    let Some(s) = p.to_str() else { return p };
    match s.strip_prefix(r"\\?\") {
        Some(rest) if rest.starts_with(r"UNC\") => PathBuf::from(format!(r"\\{}", &rest[4..])),
        Some(rest) => PathBuf::from(rest),
        None => p,
    }
}

#[cfg(unix)]
fn plain(p: PathBuf) -> PathBuf { p }
```

- [ ] **Step 4: Unix green, pushed, Windows green**

```bash
cd "$W" && cargo clippy --all-targets -- -D warnings 2>&1 | tail -1 && cargo test --release 2>&1 | grep 'test result'
git -C "$W" add src/main.rs
git -C "$W" commit -F - <<'MSG'
fix(main): the repository path is carried without the verbatim prefix on Windows

canonicalize answers \\?\C:\… there; std re-applies that prefix itself wherever a call needs it,
so carrying it bought nothing and cost four bytes of the socket name, a prefix in every message,
and a spelling git -C is not promised to take. Unix is untouched.
MSG
git -C "$W" push
gh auth switch --user devmaxxx && gh run watch --repo devmaxxx/repograph --exit-status $(gh run list --repo devmaxxx/repograph --branch feat/windows-serve --workflow ci --limit 1 --json databaseId --jq '.[0].databaseId') | tail -4 | tee "$M/task5-ci.txt"
```

Expected: unix counts unchanged (the test is Windows-only, the unix `plain` is the identity); both jobs green, including the `changes` step and all serve tests (the socket path now `C:\…`). Record the URL.

---

### Task 6: A rename retried on a sharing violation

**Files:**
- Modify: `src/store.rs`

- [ ] **Step 1: The failing tests**

Append to `src/store.rs`'s `mod tests`:

```rust
    /// A reader that took no delete share — an indexer, a sync client, an editor — holds the
    /// destination for a moment; the rename waits it out rather than failing the save.
    #[cfg(windows)]
    fn hold_open_for(path: &std::path::Path, ms: u64) -> std::thread::JoinHandle<()> {
        use std::os::windows::fs::OpenOptionsExt;
        use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ;
        let f = std::fs::OpenOptions::new().read(true).share_mode(FILE_SHARE_READ).open(path).unwrap();
        std::thread::spawn(move || { std::thread::sleep(std::time::Duration::from_millis(ms)); drop(f); })
    }

    #[cfg(windows)]
    #[test]
    fn a_rename_over_a_briefly_held_file_waits_and_lands() {
        let d = tempfile::tempdir().unwrap();
        let store = Store::new(d.path());
        store.write_atomic("graph.json", b"old").unwrap();
        let holder = hold_open_for(&d.path().join(".repograph/graph.json"), 100);
        store.write_atomic("graph.json", b"new").unwrap();
        holder.join().unwrap();
        assert_eq!(store.read_bytes("graph.json").unwrap().as_deref(), Some(&b"new"[..]));
    }

    #[cfg(windows)]
    #[test]
    fn a_rename_over_a_file_held_past_the_budget_fails_with_the_sharing_error_and_keeps_the_old_bytes() {
        let d = tempfile::tempdir().unwrap();
        let store = Store::new(d.path());
        store.write_atomic("graph.json", b"old").unwrap();
        let holder = hold_open_for(&d.path().join(".repograph/graph.json"), 1500);
        let err = store.write_atomic("graph.json", b"new").unwrap_err();
        assert!(format!("{err:#}").contains("os error 32"), "{err:#}");
        holder.join().unwrap();
        assert_eq!(store.read_bytes("graph.json").unwrap().as_deref(), Some(&b"old"[..]));
    }
```

- [ ] **Step 2: `rename_over()`**

`store.rs:96`: `std::fs::rename(&tmp, self.dir.join(name))` → `rename_over(&tmp, &self.dir.join(name))`, and above `impl Store`:

```rust
#[cfg(unix)]
fn rename_over(from: &Path, to: &Path) -> std::io::Result<()> { std::fs::rename(from, to) }

/// A rename over a file some other program has open without delete sharing — an indexer, a sync
/// client, a scanner — is refused with a sharing violation for as long as that handle lives, which
/// is milliseconds. std retries nothing for that error, so this does, for about half a second in
/// all, before the store is left as it was.
#[cfg(windows)]
fn rename_over(from: &Path, to: &Path) -> std::io::Result<()> {
    const SHARING_VIOLATION: i32 = 32;
    let mut wait = std::time::Duration::from_millis(1);
    loop {
        match std::fs::rename(from, to) {
            Err(e) if e.raw_os_error() == Some(SHARING_VIOLATION) && wait < std::time::Duration::from_millis(512) => {
                std::thread::sleep(wait);
                wait *= 2;
            }
            r => return r,
        }
    }
}
```

- [ ] **Step 3: Unix green, pushed, Windows green**

```bash
cd "$W" && cargo clippy --all-targets -- -D warnings 2>&1 | tail -1 && cargo test --release store:: 2>&1 | grep 'test result'
git -C "$W" add src/store.rs
git -C "$W" commit -F - <<'MSG'
fix(store): a rename over a briefly held file waits it out on Windows

Every std open shares delete, so repograph's own processes never block a save; other programs —
an indexer, a sync client, a scanner — open without it, and the rename fails with a sharing
violation for the milliseconds their handle lives. std retries only access-denied; this retries
that one error with back-off to about half a second. Unix renames as it did.
MSG
git -C "$W" push
gh auth switch --user devmaxxx && gh run watch --repo devmaxxx/repograph --exit-status $(gh run list --repo devmaxxx/repograph --branch feat/windows-serve --workflow ci --limit 1 --json databaseId --jq '.[0].databaseId') | tail -4 | tee "$M/task6-ci.txt"
```

Expected: unix counts unchanged; both jobs green with both new `store::tests` `ok` on Windows. If the "held past the budget" test reports a different os error, the log says which — `ERROR_ACCESS_DENIED` (5) means std's own POSIX-rename retry ran and the handle's share mode was not what the test set; adjust the test's share mode, not the retry.

---

### Task 7: CRLF pinned

**Files:**
- Modify: `src/doc/cases.rs`, `src/code/cases.rs`

- [ ] **Step 1: The cases**

Both files are inline case suites; follow their existing helper names (`extract`/`node(&ex, id)` in `doc/cases.rs`, the `ex`/`node` helpers in `code/cases.rs` — read the first thirty lines of each before writing). The shape:

```rust
/// A checkout with core.autocrlf=true — Git for Windows' installer default — is the same corpus:
/// every id, label, body and line number, and so every passage hash a questions.json was written
/// under. By construction today, since the extractors split with `lines()`; pinned so a slice of
/// raw text in some future extractor cannot quietly make it false.
#[test]
fn a_crlf_document_extracts_the_same_nodes_and_edges_as_its_lf_twin() {
    let lf = "**FR-PAY-1 · MUST · Штраф за отмену**\n\nШтраф списывается сам (INV-1).\n\n```\n**FR-X-9 · MUST · not a head**\n```\n\n**INV-1 · MUST · Деньги не сгорают**\n\nОтмена не сжигает деньги.\n";
    let crlf = lf.replace('\n', "\r\n");
    let (a, b) = (extract("docs/pay.md", lf), extract("docs/pay.md", &crlf));
    assert_eq!(a.nodes, b.nodes);
    assert_eq!(a.edges, b.edges);
    assert!(a.nodes.iter().all(|n| !n.body.contains('\r') && !n.label.contains('\r')));
}
```

and for code a TypeScript source with a doc comment, a decorated class with two members, an exported function and an import, extracted twice the same way with the same three assertions.

- [ ] **Step 2: Green, pushed**

```bash
cd "$W" && cargo test --release cases 2>&1 | grep 'test result'
git -C "$W" add src/doc/cases.rs src/code/cases.rs
git -C "$W" commit -F - <<'MSG'
test: a CRLF checkout extracts the same graph as an LF one

Git for Windows checks out with CRLF by default. The extractors already split with lines(), so
ids, bodies, line numbers and passage hashes agree with a unix checkout; this pins it.
MSG
git -C "$W" push
```

Expected: both cases pass on the first run — if one does not, it found a raw-text slice; fix at that consumer (the same fix on every platform, since a CRLF file on a Mac is the same bytes) and say so in the commit. CI green on both jobs.

---

### Task 8: The words, and the second dispatch

**Files:**
- Modify: `README.md`

Anchors are text on the branch tip; re-find by text.

- [ ] **Step 1: `README.md`**

- The serve paragraph, the sentence beginning `On unix it removes the one it bound itself, told from a replacement's by the socket file's device and inode; Windows gives std no identity to read there, so a \`serve\` exiting leaves the file and the next one removes it before binding, which is what both platforms already do after a Ctrl-C.` → `It removes the one it bound itself, told from a replacement's by the socket file's identity — device and inode on unix, the volume serial and NTFS file reference number on Windows, read off a handle there because std's accessors for them are unstable. A Ctrl-C, a \`Stop-Process\` or a \`kill\` runs no such removal on any platform: the file stays, and the next \`serve\` removes it before binding.`
- The limit sentence: after `104 on macOS;` insert `the bytes are UTF-8, so a Cyrillic user name costs two a letter, and \`C:\Users\Максим\OneDrive - <company>\Documents\projects\beauty-crm\.repograph\serve.sock\` is about 100 of the 107 allowed;`.
- The `sh` sentence: `Both run under \`sh -c\`; on Windows that is Git for Windows' \`sh\`, which has to be on \`PATH\` — nothing else in repograph needs a shell.` → `Both run under \`sh -c\`; on Windows that is Git for Windows' \`sh\`, taken from \`PATH\` when it is there and otherwise found beside \`git\`, at the bash Claude Code names in \`CLAUDE_CODE_GIT_BASH_PATH\`, or under Program Files, with Git's \`usr\bin\` put on the command's own \`PATH\` — nothing else in repograph needs a shell. Without Git for Windows, \`enrich\` and \`ask --rerank\` refuse with a line that says so, and everything else runs.`
- A new subsection after the install block's launcher paragraph, `### On Windows`, in this order, one short paragraph each: the floor (Windows 10 1903 / Windows 11 / Server 2022 — the ONNX Runtime build imports DirectML and DirectX 12 at load; no GPU is used, inference runs on the CPU, proven on a GPU-less runner: cite `$M/task4-ci-1.txt`'s URL); an unsigned binary (SmartScreen on an Explorer-launched copy from a browser download, `Unblock-File`, none from a terminal or from `gh`/npm; Defender's scan of the 2.2 GB model download and of `.repograph`, and `Add-MpPreference -ExclusionPath` as the optional speed-up); running `serve` in the background (`Start-Process repograph -ArgumentList 'serve','--idle','86400' -WindowStyle Hidden`; `--idle` or `Stop-Process -Name repograph` ends it); stop `serve` before replacing the binary; keep the repository out of a OneDrive/Dropbox-synced tree (the store is per machine and the socket file is a reparse point a sync client cannot upload; what it does with one is unverified); console output is UTF-8 — Windows Terminal renders it, and PowerShell decodes a piped or captured answer with `[Console]::OutputEncoding`, which on a Russian-locale system is code page 866 unless set to UTF-8; environment paths (`FASTEMBED_CACHE_DIR`, `XDG_CONFIG_HOME`, `REPOGRAPH_CONFIG`) must be Windows-form even when set inside Git Bash; git needs `core.longpaths=true` for a tree deeper than 260 characters, and repograph then follows it.
- The embeddings paragraph: `(\`%USERPROFILE%\.cache\repograph\fastembed\` on Windows; the layout is the hub client's, so a cache populated by an earlier release is reused as is)` gains `— where the client links each file into its snapshot, or on a Windows account without symlink rights moves it there`.

- [ ] **Step 2: Read back, commit, push**

```bash
cd "$W" && grep -n 'gives std no identity\|has to be on `PATH`' README.md; echo "(expected: nothing)"
git -C "$W" add README.md
git -C "$W" commit -F - <<'MSG'
docs: the Windows section — the floor, the unsigned binary, serve in the background, sync folders, the console
MSG
git -C "$W" push
```

- [ ] **Step 3: The second dispatch — the build Max keeps**

```bash
gh auth switch --user devmaxxx && gh workflow run release.yml --repo devmaxxx/repograph --ref feat/windows-serve
sleep 20; gh auth switch --user devmaxxx && RUN=$(gh run list --repo devmaxxx/repograph --workflow release --event workflow_dispatch --branch feat/windows-serve --limit 1 --json databaseId --jq '.[0].databaseId') && gh run watch --repo devmaxxx/repograph --exit-status "$RUN" | tail -6 | tee "$M/task8-release.txt" && echo "$RUN" >> "$M/task8-release.txt"
```

Expected: three green build jobs, three artifacts. Report DONE with the run id and the PowerShell fetch block.

- [ ] **Step 4: What to run on the Windows box, for the record**

Not a step this machine runs; the block Max runs, whose numbers go into the PR body under *Measured on the box*:

```powershell
cd C:\...\beauty-crm
repograph --version
repograph build                                      # first run downloads the model the project's repograph.toml names
Start-Process repograph -ArgumentList 'serve','--idle','86400' -WindowStyle Hidden
Start-Sleep 5; repograph ask штраф за отмену          # expect "serve: answered by the resident process" on stderr
1..11 | ForEach-Object { (Measure-Command { repograph ask штраф за отмену | Out-Null }).TotalMilliseconds } | Sort-Object | Select-Object -Index 5
repograph enrich --limit 2                           # exercises sh discovery against the real claude
```

The median is compared with the README's `serve` row and the Mac's `$M/task4-mac.txt`; "unix-equivalent" is a number on the same corpus, not a feeling.

---

### Task 9: The pull request

**Files:** none.

- [ ] **Step 1: Update the branch's PR (or open it)**

If the transport plan's Task 7 opened a PR, `gh pr edit` its body; otherwise `gh pr create` with the transport plan's body and this addition:

```
## Windows edges, and the build

- The archive of every dispatch run is a run artifact: <URL from $M/task8-release.txt>; fetch with
  `gh run download <id> -n repograph-x86_64-pc-windows-msvc`.
- `sh` is found where a default Git for Windows install leaves it; judged by a CI step that strips
  Git's shell directories from PATH, red at <$M/task2-red.txt's run> and green at <$M/task2-ci.txt>.
- The socket file's identity from #15 is tested on Windows; a stale socket's connect is refused in
  under a second; a socket binds under a Cyrillic directory: <$M/task3-ci.txt>.
- The dense arm opens and answers through the socket on a GPU-less Windows runner, small model,
  cached between runs: <$M/task4-ci-1.txt> (first, downloading) and the re-run (restored). Median
  resident fused ask on the runner: <ms>; on the Mac: <$M/task4-mac.txt>.
- The repository path is carried without `\\?\` on Windows; `changes` runs on the Windows checkout:
  <$M/task5-ci.txt>.
- A rename over a briefly held file waits it out on Windows: <$M/task6-ci.txt>.
- A CRLF checkout extracts the same graph: pinned by two cases.
- Documented, not fixed: sync folders, SmartScreen/Defender, the console encoding, the OS floor,
  long paths, stopping serve before an update.

## Measured on the box

<Max's numbers from Task 8 Step 4, or "not yet">
```

Report DONE with the PR URL and the paths under `$M`.

---

## Placeholder check, done when the plan was written

Every `$M/...` file is written by a step before a later step reads it. Every line number cited is from the branch tip `7dd7011` read through a throwaway worktree (removed), from the 1.98.0 `rust-src` under `~/.rustup/toolchains/stable-aarch64-apple-darwin`, or from the registry sources of `socket2-0.6.5`, `hf-hub-0.5.0`, `ort-sys-2.0.0-rc.13`, `ort-2.0.0-rc.13`, `globset-0.4.20` and `ignore-0.4.33`; anchors in the README and in test files are re-found by text. The Rust in Tasks 2, 3, 5, 6 and 7 is written against the files as they stand; Task 0 Step 1 confirms #15's shape before Task 3 assumes it. The two CI steps that must go red first (Task 2 Step 1, Task 5 Step 1) say what red must look like, and a green there is treated as a step that judged nothing. No step builds for Windows on the Mac; every Windows verdict is a run URL in `$M`.
