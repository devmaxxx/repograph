#!/usr/bin/env bash
# Builds the overlays from their sources rather than keeping copies in this repository: B is the
# consuming project's own `.claude/`, which belongs to that project and not to this one, and C is
# whatever `install-agent` writes today — a checked-in copy of either would be a second version of
# something that already has one.
set -euo pipefail
F=${FIXTURE:-/Users/max/bench/beauty-crm-502e8a6d}
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$HERE/../../.." && pwd)"
BIN=${BIN:-$REPO/target/release/repograph}

mkdir -p "$HERE/A" "$HERE/B/hooks" "$HERE/B/skills"
# A: the consuming project's instructions with the repograph sections removed — everything from
# '## Скиллы' on, under the original H1.
python3 - "$F/.claude/CLAUDE.md" "$HERE/A/CLAUDE.md" <<'PY'
import sys
t = open(sys.argv[1]).read(); i = t.find('## Скиллы')
# `find` answers -1 for a heading that is not there, and `t[-1:]` is one character: overlay A
# would then be an H1 and a full stop, and A is the arm nobody watches. Fail instead.
if i < 0:
    raise SystemExit(f"{sys.argv[1]}: no '## Скиллы' heading — the fixture's CLAUDE.md changed shape")
open(sys.argv[2], 'w').write(t.splitlines()[0] + '\n\n' + t[i:])
PY

# B: today's repograph surface, verbatim.
cp "$F/.claude/CLAUDE.md" "$F/.claude/settings.json" "$HERE/B/"
cp "$F/.claude/hooks/repograph-notice.mjs" "$F/.claude/hooks/precompact-brief.mjs" \
   "$F/.claude/hooks/compact-state.mjs" "$HERE/B/hooks/"
rm -rf "$HERE/B/skills/repo-query"; cp -R "$F/.claude/skills/repo-query" "$HERE/B/skills/"

# C: whatever the installer writes today, never hand-edited.
C=$(mktemp -d); "$BIN" --repo "$C" install-agent --claude >/dev/null
rm -rf "$HERE/C"; cp -R "$C/.claude" "$HERE/C"; rm -rf "$C"

wc -c "$HERE/A/CLAUDE.md" "$HERE/B/CLAUDE.md" "$HERE/C/CLAUDE.md"
