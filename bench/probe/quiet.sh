#!/bin/bash
# The precondition every probe row is read under, stated the way each row states the idle
# baseline it is read against. Exit 1 with the reading when the machine is not quiet: a reading
# taken on a busy laptop measures the laptop, which is what G23 found.
#
# `node` counts like `cargo` and `rustc`, except the chain this script hangs off: the harness that
# launched it is unavoidable and the `load1` clause already refuses it when it is burning a core,
# while any other `node` — a sibling harness, a dev server — is noise the row would swallow.
#
# Sourcing this file defines the two functions and runs nothing, so the tests beside it can read
# the walk and the filter without waiting for a machine that happens to be quiet.
set -u

# This shell's own chain of PIDs, `$$` upwards.
ancestor_pids() {
  local pid=$$
  while [ -n "$pid" ] && [ "$pid" != "0" ] && [ "$pid" != "1" ]; do
    printf '%s\n' "$pid"
    pid=$(ps -o ppid= -p "$pid" 2>/dev/null | tr -d '[:space:]')
  done
}

# `pid name` candidates on stdin, minus this shell's own ancestors and nothing else.
busy_lines() {
  local skip pid rest
  skip=" $(ancestor_pids | tr '\n' ' ')"
  while read -r pid rest; do
    case "$skip" in *" $pid "*) continue ;; esac
    printf '%s %s\n' "$pid" "$rest"
  done
}

main() {
  IDLE=$(top -l 2 -s 1 | grep 'CPU usage' | tail -1 | sed 's/.*, \([0-9.]*\)% idle.*/\1/')
  LOAD1=$(sysctl -n vm.loadavg | awk '{print $2}')
  AC=$(pmset -g batt | head -1 | grep -c "AC Power")
  BUSY_LINES=$(pgrep -l 'cargo|rustc|node|repograph' | busy_lines)
  BUSY=$(printf '%s\n' "$BUSY_LINES" | grep -c '[^[:space:]]' | tr -d ' ')
  echo "quiet: idle=${IDLE}% load1=${LOAD1} ac=${AC} busy_processes=${BUSY}"
  ok=1
  awk -v i="$IDLE" 'BEGIN{exit !(i+0 >= 85)}' || { echo "  not quiet: idle ${IDLE}% < 85%"; ok=0; }
  awk -v l="$LOAD1" 'BEGIN{exit !(l+0 < 3.0)}' || { echo "  not quiet: load1 ${LOAD1} >= 3.0"; ok=0; }
  [ "$AC" = "1" ] || { echo "  not quiet: on battery"; ok=0; }
  [ "$BUSY" = "0" ] || { echo "  not quiet: $BUSY cargo/rustc/node/repograph processes running"; printf '%s\n' "$BUSY_LINES"; ok=0; }
  [ "$ok" = "1" ]
}

if [ "${BASH_SOURCE[0]}" = "$0" ]; then main; fi
