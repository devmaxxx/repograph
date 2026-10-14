#!/usr/bin/env node
"use strict";

// The binary ships in a per-platform package pulled in as an optional dependency, so a
// platform without a prebuilt binary installs cleanly and fails only when it actually runs.
const { spawnSync } = require("node:child_process");

const PACKAGES = {
  "darwin-arm64": "@devmaxxx/zerograph-darwin-arm64",
  "linux-x64": "@devmaxxx/zerograph-linux-x64",
  "win32-x64": "@devmaxxx/zerograph-win32-x64",
};

// Cargo names the Windows binary with its extension, and `require.resolve` tries none of its own
// for a file that is not a module.
const BINARY = process.platform === "win32" ? "bin/zerograph.exe" : "bin/zerograph";

const FALLBACK =
  "cargo install --git https://github.com/devmaxxx/zerograph --locked";

function binaryPath() {
  const key = `${process.platform}-${process.arch}`;
  const pkg = PACKAGES[key];
  if (!pkg) {
    return {
      error: `zerograph: no prebuilt binary for ${key}; build one with \`${FALLBACK}\``,
    };
  }
  try {
    return { path: require.resolve(`${pkg}/${BINARY}`) };
  } catch {
    return {
      error:
        `zerograph: ${pkg} is not installed. Optional dependencies were skipped (\`--no-optional\`, ` +
        `or a lockfile written on another platform); reinstall with them enabled, or \`${FALLBACK}\``,
    };
  }
}

const found = binaryPath();
if (found.error) {
  console.error(found.error);
  process.exit(2);
}

const run = spawnSync(found.path, process.argv.slice(2), { stdio: "inherit" });
if (run.error) {
  console.error(`zerograph: ${run.error.message}`);
  process.exit(2);
}
// A signal death has no status; report it the way a shell would.
process.exit(run.status === null ? 128 : run.status);
