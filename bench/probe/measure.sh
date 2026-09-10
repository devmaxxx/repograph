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
[ "$RC" = "0" ] || echo "measure: $NAME exited $RC — this row measured a failure" >&2
PEAK_CPU=$(awk '{gsub("%","",$2); if ($2+0>m) m=$2+0} END{print m+0}' "$LOG/$NAME.samples")
PEAK_TH=$(awk '{split($4,a,"/"); if (a[1]+0>m) m=a[1]+0} END{print m+0}' "$LOG/$NAME.samples")
RSS=$(awk '/maximum resident set size/{printf "%.2f", $1/1073741824}' "$LOG/$NAME.time")
WALL=$(awk '/real/{print $1}' "$LOG/$NAME.time")
USR=$(awk '/real/{print $3}' "$LOG/$NAME.time")
SYS=$(awk '/real/{print $5}' "$LOG/$NAME.time")
N=$(wc -l <"$LOG/$NAME.samples" | tr -d ' ')
echo "$NAME  wall=${WALL}s user=${USR}s sys=${SYS}s maxrss=${RSS}GB peak_cpu=${PEAK_CPU}% peak_threads=${PEAK_TH} samples=${N} rc=${RC}" | tee -a "$LOG/summary.txt"
exit "$RC"
