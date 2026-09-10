#!/bin/bash
# arms.sh <binary> <repo> <logdir> <label>  — the four arms, transcripts kept, recorded with a tag
set -u
B=$1; R=$2; L=$3; T=$4
# Off the script's own location, not this machine's checkout: a clone that cannot run the script
# in its own tree is the defect the ledger already named once.
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
H="$ROOT/bench/history"
mkdir -p "$L"
for suite in rec dev; do
  for arm in dense lexical; do
    nd=""; [ "$arm" = lexical ] && nd="--no-dense"
    cases=""; [ "$suite" = dev ] && cases="--cases $ROOT/bench/dev-cases.jsonl"
    out="$L/$T-$suite-$arm.txt"
    REPOGRAPH_NO_SERVE=1 "$B" --repo "$R" $nd bench $cases > "$out" 2>&1
    rc=$?
    tail -3 "$out"
    # Recorded whatever the status: `bench` exits non-zero when a floor fails, and a failed floor
    # is a reading the history exists to hold. G32's rebuilt arms are expected to fail one, and
    # its gate names them as recorded arms — an arm dropped for exiting non-zero is a lost row.
    [ "$rc" = "0" ] || echo "arms: $suite/$arm exited $rc — recorded anyway, see $out" >&2
    python3 "$H/track.py" record "$out" --corpus beauty-crm --corpus-path "$R" --tag "$T" --note "next-version-levers $T"
  done
done
