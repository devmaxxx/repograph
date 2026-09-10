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
while kill -0 "$PID" 2>/dev/null; do
  top -l 2 -s 1 -pid "$PID" -stats pid,cpu,mem,th 2>/dev/null | tail -1 >>"$LOG/$NAME.samples"
done
wait $WRAP; RC=$?
# A command that failed still leaves `time` a full report, so its row would otherwise enter the
# median as an honestly-measured fast run — a mistyped flag reading as a reader that got 60×
# quicker. The row carries its status and the script exits with it.
#
# One non-zero exit is not that, though. `repograph bench` returns 1 for a missed floor and 1 for an
# empty graph, a missing case file or a dense width mismatch, so the status alone cannot say whether
# the reader did its work: a row that answered every case and then failed a floor spent its wall
# clock reading, and a row that never found a store spent it failing. Only the wording separates
# them, and it is read here rather than in `judge.py` because the transcript is what holds it. The
# field says which of the two it was; the row is still refused unless `judge.py` also recognises the
# row as one that runs `bench`. A wording that changes upstream is caught by `test_measure.py`.
FLOORS=0
if [ "$RC" != "0" ] && grep -q "bench floors not met" "$LOG/$NAME.time"; then FLOORS=1; fi
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
