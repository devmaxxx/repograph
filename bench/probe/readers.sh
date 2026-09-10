#!/bin/bash
# readers.sh BINARY WORKTREE LOGDIR [N]
# The reader suite, N runs per row (default 5), in the one writable worktree straight after
# reset.sh has put the pinned fixture's store there, the resident server bypassed so every row is
# a cold process. Rows are the 2026-09-07 suite less `import-legacy`, which writes the store, and
# less `ask-stale`, which is every ask row now: every command that could walk the tree is read
# `--stale`, so no row refreshes stamps, rewrites a mirror or resyncs the index under the rows
# after it — the index moving under the suite is the cause G23 named — and the suite writes
# nothing, which the plan proves with the store's checksums before and after. `changes` needs a
# diff to read, so it touches one source file for the row's duration and moves the original back
# with its stamp (`cp -p`). Writes LOGDIR/summary.txt (one line per run) and LOGDIR/medians.txt.
set -u
B=$1; F=$2; L=$3; N=${4:-5}
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
M="$HERE/measure.sh"
export REPOGRAPH_NO_SERVE=1
mkdir -p "$L"
# Truncated, not appended: `measure.sh` appends, so a second suite into the same log directory
# would pool its runs with the first one's and `medians` would take one median over both.
: > "$L/summary.txt"
# Not a pipeline: bash 3.2 has no pipefail here and a `| tee` would hand back tee's exit code.
if ! "$HERE/quiet.sh" > "$L/quiet.txt" 2>&1; then cat "$L/quiet.txt"; echo "refusing to measure on a machine that is not quiet" >&2; exit 2; fi
cat "$L/quiet.txt" | tee -a "$L/summary.txt"
cd "$F" || exit 2
Q="как отменить запись и кто платит штраф"
CN=packages/ui/src/lib/cn.ts
for i in $(seq 1 "$N"); do
  "$M" ask-fused-$i "$L" -- "$B" --repo "$F" ask --stale $Q
  "$M" ask-nodense-$i "$L" -- "$B" --repo "$F" --no-dense ask --stale $Q
  "$M" ask-rerank-local-$i "$L" -- "$B" --repo "$F" ask --stale --rerank-local $Q
  "$M" ask-exact-id-$i "$L" -- "$B" --repo "$F" ask --stale FR-PAY-22
  "$M" impact-$i "$L" -- "$B" --repo "$F" impact --stale cn --depth 3
  "$M" trace-$i "$L" -- "$B" --repo "$F" trace --stale main cn --depth 6
  # The backup lives in the log directory, not beside the file: `changes` reads every untracked
  # path `git ls-files --others --exclude-standard` names, and a `cn.ts.orig` in the worktree is
  # one — the row would measure two changed files, the second a whole copy of the first.
  cp -p "$CN" "$L/cn.ts.orig"
  printf '\n// touched for the changes measurement\n' >> "$CN"
  "$M" changes-$i "$L" -- "$B" --repo "$F" changes --stale --depth 2
  cp -p "$L/cn.ts.orig" "$CN" && rm -f "$L/cn.ts.orig"
  "$M" bench-nodense-$i "$L" -- "$B" --repo "$F" --no-dense bench
  "$M" bench-dense-$i "$L" -- "$B" --repo "$F" bench
  "$M" dump10-$i "$L" -- "$B" --repo "$F" dump --queries "$HERE/queries10.jsonl" --out "$L/dump10.json" --depth 300
done
python3 "$HERE/judge.py" medians "$L/summary.txt" | tee "$L/medians.txt"
