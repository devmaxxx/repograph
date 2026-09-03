#!/usr/bin/env bash
# Assembles the npm packages from prebuilt binaries and packs them into dist/npm/.
# Publishing stays a separate, deliberate step: a version number, once published, is spent —
# GitHub Packages refuses a second upload of it even after the first is deleted. The registry
# comes from each package's publishConfig; publishing by hand needs a classic PAT with
# write:packages as //npm.pkg.github.com/:_authToken in ~/.npmrc.
#
#   scripts/npm-pack.sh [version] [darwin-arm64=<binary>] [linux-x64=<binary>]
#
# The version defaults to Cargo.toml's. On an Apple-silicon Mac, darwin-arm64 defaults to
# target/release/repograph; any platform not given is left out of this pack run, and the
# launcher's optional dependency on it simply goes unresolved until it is published.
# Plain variables rather than an associative array: macOS still ships bash 3.2.
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$root"

version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
darwin_arm64=""
linux_x64=""
if [ "$(uname -s)-$(uname -m)" = "Darwin-arm64" ] && [ -x target/release/repograph ]; then
  darwin_arm64=target/release/repograph
fi
for arg in "$@"; do
  case "$arg" in
    darwin-arm64=*) darwin_arm64=${arg#*=} ;;
    linux-x64=*) linux_x64=${arg#*=} ;;
    *=*) echo "unknown platform in $arg" >&2; exit 2 ;;
    *) version=$arg ;;
  esac
done

out=dist/npm
rm -rf "$out" && mkdir -p "$out"

pack_platform() {
  local platform=$1 src=$2 dir
  [ -x "$src" ] || { echo "$platform: $src is not an executable" >&2; exit 2; }
  dir=npm/repograph-$platform
  mkdir -p "$dir/bin"
  cp "$src" "$dir/bin/repograph" && chmod 755 "$dir/bin/repograph"
  (cd "$dir" && npm pkg set version="$version" && npm pack --silent --pack-destination "$root/$out")
}

[ -n "$darwin_arm64" ] && pack_platform darwin-arm64 "$darwin_arm64"
[ -n "$linux_x64" ] && pack_platform linux-x64 "$linux_x64"

(
  cd npm/repograph
  npm pkg set version="$version"
  for platform in darwin-arm64 linux-x64; do
    npm pkg set "optionalDependencies.@devmaxxx/repograph-$platform=$version"
  done
  npm pack --silent --pack-destination "$root/$out"
)

echo "packed $version into $out:"
ls -1 "$out"
echo
echo "publish the platform packages first, the launcher last:"
for t in "$out"/*-arm64-*.tgz "$out"/*-x64-*.tgz "$out"/devmaxxx-repograph-"$version".tgz; do
  [ -e "$t" ] && echo "  npm publish $t"
done
true
