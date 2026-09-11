#!/bin/bash
# measure.sh NAME LOGDIR -- cmd args...
# One command under /usr/bin/time -l (wall, user, sys, max RSS) with `top` sampled every second
# for instantaneous CPU% and thread count. Writes NAME.time, NAME.samples, NAME.out and appends
# one summary line to LOGDIR/summary.txt. macOS only: `-l` and `top -l` are Darwin's.
set -u
NAME=$1; LOG=$2; shift 2; [ "$1" = "--" ] && shift
mkdir -p "$LOG"
/usr/bin/time -l "$@" >"$LOG/$NAME.out" 2>"$LOG/$NAME.time" &
WRAP=$!
sleep 0.3
# `time` forks the measured command; a command that finished inside 0.3 s leaves no samples.
PID=$(pgrep -P $WRAP | head -1)
[ -z "$PID" ] && PID=$WRAP
: >"$LOG/$NAME.samples"
# The row `top` printed for this pid, not whatever its last line happened to be. The loop's final
# pass always straddles the process's exit — `kill -0` succeeded a moment before — so `top`'s
# second frame finds nothing to print and its last line is the column header (`PID %CPU MEM #TH`).
# Appended, that header counts as a sample: every row's `samples=` reads one higher than the number
# of CPU readings behind its peak, and a binary that died right after `pgrep` caught it leaves a
# file of one header rather than the empty one `embed.sh` refuses on.
while kill -0 "$PID" 2>/dev/null; do
  top -l 2 -s 1 -pid "$PID" -stats pid,cpu,mem,th 2>/dev/null |
    awk -v p="$PID" '$1 == p {l=$0} END{if (l != "") print l}' >>"$LOG/$NAME.samples"
done
wait $WRAP; RC=$?
# A command that failed still leaves `time` a full report, so its row would otherwise enter the
# median as an honestly-measured fast run — a mistyped flag reading as a reader that got 60×
# quicker. The row carries its status and the script exits with it.
#
# One non-zero exit is not that, though. `repograph bench` exits 2 for a suite that answered every
# case and missed a floor, and 1 for an empty graph, a missing case file or a dense width mismatch:
# a row that missed a floor spent its wall clock reading, and a row that never found a store spent
# it failing. The status is what separates them — it is the interface, and the sentence on stderr
# is only a message to a person, free to be reworded. A binary too old to exit 2 says nothing this
# reads, so its missed floors arrive as the failures they are indistinguishable from.
FLOORS=0
[ "$RC" = "2" ] && FLOORS=1
if [ "$RC" = "0" ]; then :
elif [ "$FLOORS" = "1" ]; then echo "measure: $NAME exited $RC on its floors — the timing stands, the floors did not" >&2
else echo "measure: $NAME exited $RC — this row measured a failure" >&2
fi
# Absent on every row where nothing happened, so a clean line is the line it has always been and
# the field's absence can never admit anything: an instrument too old to write it refuses the row.
FLOORS_FIELD=""
[ "$FLOORS" = "1" ] && FLOORS_FIELD=" floors_missed=1"
PEAK_CPU=$(awk '{gsub("%","",$2); if ($2+0>m) m=$2+0} END{print m+0}' "$LOG/$NAME.samples")
PEAK_TH=$(awk '{split($4,a,"/"); if (a[1]+0>m) m=a[1]+0} END{print m+0}' "$LOG/$NAME.samples")
# The measured command's stderr shares this file with `time`'s report, and an unanchored `/real/`
# prints one number per matching line: a single stderr line holding the word turned WALL into two
# lines and split the summary line `judge.py medians` parses into four. So each field is read off
# the report's whole shape — every word of it, in place, and nothing else on the line — and only
# the last such line is taken, which is one value however loud the command was.
TIME=$LOG/$NAME.time
RSS=$(awk 'NF == 5 && $2 == "maximum" && $3 == "resident" && $4 == "set" && $5 == "size" {r=$1} END{if (r != "") printf "%.2f", r/1073741824}' "$TIME")
REPORT=$(awk 'NF == 6 && $2 == "real" && $4 == "user" && $6 == "sys" {l=$0} END{print l}' "$TIME")
WALL=$(printf '%s\n' "$REPORT" | awk '{print $1}')
USR=$(printf '%s\n' "$REPORT" | awk '{print $3}')
SYS=$(printf '%s\n' "$REPORT" | awk '{print $5}')
N=$(wc -l <"$LOG/$NAME.samples" | tr -d ' ')
echo "$NAME  wall=${WALL}s user=${USR}s sys=${SYS}s maxrss=${RSS}GB peak_cpu=${PEAK_CPU}% peak_threads=${PEAK_TH} samples=${N} rc=${RC}${FLOORS_FIELD}" | tee -a "$LOG/summary.txt"
exit "$RC"
