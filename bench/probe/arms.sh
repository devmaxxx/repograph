#!/bin/bash
# arms.sh <binary> <repo> <logdir> <label>  — the four arms, transcripts kept, recorded with a tag
B=$1; R=$2; L=$3; T=$4; H=/Users/max/Documents/projects/repograph/bench/history
mkdir -p "$L"
for suite in rec dev; do
  for arm in dense lexical; do
    nd=""; [ "$arm" = lexical ] && nd="--no-dense"
    cases=""; [ "$suite" = dev ] && cases="--cases /Users/max/Documents/projects/repograph/bench/dev-cases.jsonl"
    REPOGRAPH_NO_SERVE=1 "$B" --repo "$R" $nd bench $cases > "$L/$T-$suite-$arm.txt" 2>&1 || true
    tail -3 "$L/$T-$suite-$arm.txt"
    python3 "$H/track.py" record "$L/$T-$suite-$arm.txt" --corpus beauty-crm --corpus-path "$R" --tag "$T" --note "next-version-levers $T"
  done
done
