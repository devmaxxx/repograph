#!/usr/bin/env bash
# Publishes zerograph@0.0.0, an empty placeholder, so trusted publishing can be set up on the name.
#   scripts/npm-placeholder.sh <otp>   — the code your authenticator shows now; works without a terminal
#   scripts/npm-placeholder.sh         — in a real terminal, npm asks for the code itself
# Run once, by the package owner, before the first tag (docs/bench/runbook.md, step 5).
set -euo pipefail

existing=$(npm view zerograph version 2>/dev/null || true)
if [ -n "$existing" ]; then
  echo "zerograph is on npm already ($existing); nothing to do"
  exit 0
fi
echo "publishing as: $(npm whoami)"

d=$(mktemp -d)
trap 'rm -rf "$d"' EXIT
cd "$d"
npm init -y --init-version 0.0.0 >/dev/null
npm pkg set name=zerograph description="Placeholder; the first release is 0.6.0"

if [ $# -ge 1 ]; then
  npm publish --access public --otp="$1"
else
  npm publish --access public
fi

echo
# npm processes a new package for a few minutes before `npm view` lists it.
listed=$(npm view zerograph version 2>/dev/null || true)
echo "on npm: ${listed:-not listed yet; a new package can take a few minutes}"
echo "next: https://www.npmjs.com/package/zerograph/access → Trusted publishing → GitHub Actions,"
echo "      user devmaxxx, repository zerograph, workflow release.yml"
