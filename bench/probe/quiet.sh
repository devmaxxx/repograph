#!/bin/bash
# The precondition every probe row is read under, stated the way each row states the idle
# baseline it is read against. Exit 1 with the reading when the machine is not quiet: a reading
# taken on a busy laptop measures the laptop, which is what G23 found.
set -u
IDLE=$(top -l 2 -s 1 | grep 'CPU usage' | tail -1 | sed 's/.*, \([0-9.]*\)% idle.*/\1/')
LOAD1=$(sysctl -n vm.loadavg | awk '{print $2}')
AC=$(pmset -g batt | head -1 | grep -c "AC Power")
# A build or another store's run is what moves a reading; the agent harness that launched this
# script is a `node` process and is not, so it is not in the pattern.
BUSY=$(pgrep -l 'cargo|rustc|repograph' | wc -l | tr -d ' ')
echo "quiet: idle=${IDLE}% load1=${LOAD1} ac=${AC} busy_processes=${BUSY}"
ok=1
awk -v i="$IDLE" 'BEGIN{exit !(i+0 >= 85)}' || { echo "  not quiet: idle ${IDLE}% < 85%"; ok=0; }
awk -v l="$LOAD1" 'BEGIN{exit !(l+0 < 3.0)}' || { echo "  not quiet: load1 ${LOAD1} >= 3.0"; ok=0; }
[ "$AC" = "1" ] || { echo "  not quiet: on battery"; ok=0; }
[ "$BUSY" = "0" ] || { echo "  not quiet: $BUSY cargo/rustc/repograph processes running"; pgrep -l 'cargo|rustc|repograph'; ok=0; }
[ "$ok" = "1" ]
