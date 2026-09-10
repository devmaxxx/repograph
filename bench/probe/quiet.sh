#!/bin/bash
# The precondition every probe row is read under, stated the way each row states the idle
# baseline it is read against. Exit 1 with the reading when the machine is not quiet: a reading
# taken on a busy laptop measures the laptop, which is what G23 found.
#
# Presence is not noise — a process consuming no CPU moves no wall clock — and the arbiter of
# whether the machine spoiled a row is the control G23 prescribes, the same binary through the
# suite twice; this precondition exists to catch the obvious cases cheaply, not to certify silence.
#
# The two rules that follow. `cargo`, `rustc` and any other `repograph` refuse on presence at any
# CPU: they are this project's own work and they are bursty, so one at 0% now is compiling or
# embedding a second later. Any other `node` refuses only at or above NODE_CPU_MIN percent — a
# sibling harness burning a core is pollution a row would swallow, one asleep in a poll loop is
# not — and this script's own ancestor chain is exempt at any CPU, because the harness that
# launched the probe is unavoidable and the `load1` clause refuses it on its own when it is busy.
#
# Sourcing this file defines the functions and runs nothing, so the tests beside it can read the
# walk and both rules without waiting for a machine that happens to be quiet.
set -u

# The bar a `node` outside the chain has to clear to count, frozen with the rest of this clause.
NODE_CPU_MIN=5.0

# This shell's own chain of PIDs, `$$` upwards.
ancestor_pids() {
  local pid=$$
  while [ -n "$pid" ] && [ "$pid" != "0" ] && [ "$pid" != "1" ]; do
    printf '%s\n' "$pid"
    pid=$(ps -o ppid= -p "$pid" 2>/dev/null | tr -d '[:space:]')
  done
}

# `pid …` rows on stdin, minus this shell's own ancestors and nothing else.
busy_lines() {
  local skip pid rest
  skip=" $(ancestor_pids | tr '\n' ' ')"
  while read -r pid rest; do
    case "$skip" in *" $pid "*) continue ;; esac
    printf '%s %s\n' "$pid" "$rest"
  done
}

# `pid pcpu comm` rows on stdin, the offending ones out, one rule per kind. `comm` is the
# executable's path and may hold spaces, so the name compared is the basename of everything past
# the first two fields rather than a fixed field.
busy_rows() {
  awk -v kind="$1" -v bar="$NODE_CPU_MIN" '
    {
      name = $0
      sub(/^[ \t]*[^ \t]+[ \t]+[^ \t]+[ \t]+/, "", name)
      sub(/.*\//, "", name)
      if (kind == "project" && name ~ /cargo|rustc|repograph/) print
      if (kind == "node" && name ~ /node/ && $2 + 0 >= bar) print
    }'
}

count_rows() {
  printf '%s\n' "$1" | grep -c '[^[:space:]]' | tr -d ' '
}

main() {
  IDLE=$(top -l 2 -s 1 | grep 'CPU usage' | tail -1 | sed 's/.*, \([0-9.]*\)% idle.*/\1/')
  LOAD1=$(sysctl -n vm.loadavg | awk '{print $2}')
  AC=$(pmset -g batt | head -1 | grep -c "AC Power")
  ROWS=$(ps -Ao pid=,pcpu=,comm= | busy_lines)
  PROJECT_LINES=$(printf '%s\n' "$ROWS" | busy_rows project)
  NODE_LINES=$(printf '%s\n' "$ROWS" | busy_rows node)
  PROJECT=$(count_rows "$PROJECT_LINES")
  NODE=$(count_rows "$NODE_LINES")
  echo "quiet: idle=${IDLE}% load1=${LOAD1} ac=${AC} busy=$((PROJECT + NODE)) (cargo/rustc/repograph: ${PROJECT}, node ≥${NODE_CPU_MIN}%: ${NODE})"
  ok=1
  awk -v i="$IDLE" 'BEGIN{exit !(i+0 >= 85)}' || { echo "  not quiet: idle ${IDLE}% < 85%"; ok=0; }
  awk -v l="$LOAD1" 'BEGIN{exit !(l+0 < 3.0)}' || { echo "  not quiet: load1 ${LOAD1} >= 3.0"; ok=0; }
  [ "$AC" = "1" ] || { echo "  not quiet: on battery"; ok=0; }
  [ "$PROJECT" = "0" ] || { echo "  not quiet: $PROJECT cargo/rustc/repograph processes running"; printf '%s\n' "$PROJECT_LINES"; ok=0; }
  [ "$NODE" = "0" ] || { echo "  not quiet: $NODE node processes at ${NODE_CPU_MIN}% CPU or above"; printf '%s\n' "$NODE_LINES"; ok=0; }
  [ "$ok" = "1" ]
}

if [ "${BASH_SOURCE[0]}" = "$0" ]; then main; fi
