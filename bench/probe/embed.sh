#!/bin/bash
# embed.sh NAME BINARY WORKTREE LOGDIR
# A whole-store embed under the sampler, the store's vectors removed first, every stderr line
# stamped with the wall clock so the cadence is read from when lines arrived and not from the
# rate the line itself quotes. `caffeinate -di` keeps the machine and the display awake: the run
# that closed G19 lost its intervals to a laptop that slept, and `judge.py cadence` refuses a
# transcript with no `start` stamp.
set -u
NAME=$1; B=$2; F=$3; L=$4
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# The one script here that deletes store files, so it refuses the pinned fixture by path: nothing
# writes there, and a mistyped argument is how that rule would get broken.
case "$(cd "$F" && pwd -P)" in "$HOME/bench/beauty-crm-502e8a6d") echo "refusing: $F is the pinned fixture" >&2; exit 2;; esac
mkdir -p "$L"
if ! "$HERE/quiet.sh" > "$L/$NAME.quiet" 2>&1; then cat "$L/$NAME.quiet"; echo "refusing to measure on a machine that is not quiet" >&2; exit 2; fi
cat "$L/$NAME.quiet" | tee -a "$L/summary.txt"
rm -f "$F/.repograph/vectors.f32" "$F/.repograph/vectors.json"
STAMP='perl -MTime::HiRes=time -ne '"'"'printf "%.2f %s", time, $_'"'"''
perl -MTime::HiRes=time -e 'printf "%.2f start\n", time' > "$L/$NAME.err"
caffeinate -di /usr/bin/time -l "$B" --repo "$F" embed >"$L/$NAME.out" 2> >(eval "$STAMP" >> "$L/$NAME.err") &
WRAP=$!
sleep 0.3
PID=$(pgrep -f "^$B --repo $F embed" | head -1)
[ -z "$PID" ] && PID=$WRAP
: >"$L/$NAME.samples"
while kill -0 "$PID" 2>/dev/null; do
  top -l 2 -s 1 -pid "$PID" -stats pid,cpu,mem,th 2>/dev/null | tail -1 >>"$L/$NAME.samples"
done
wait $WRAP
sleep 0.5
PEAK_CPU=$(awk '{gsub("%","",$2); if ($2+0>m) m=$2+0} END{print m+0}' "$L/$NAME.samples")
RSS=$(awk '/maximum resident set size/{printf "%.2f", $2/1073741824}' "$L/$NAME.err")
WALL=$(awk '/real/{print $2}' "$L/$NAME.err")
USR=$(awk '/real/{print $4}' "$L/$NAME.err")
echo "$NAME  wall=${WALL}s user=${USR}s maxrss=${RSS}GB peak_cpu=${PEAK_CPU}%" | tee -a "$L/summary.txt"
python3 "$HERE/judge.py" cadence "$L/$NAME.err" | tee -a "$L/summary.txt"
