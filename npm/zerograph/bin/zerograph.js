#!/usr/bin/env node
"use strict";

// Every prebuilt binary ships inside this one package, under bin/<platform>-<arch>/, so an
// install needs no second package, no install script and no network beyond the tarball.
const { spawnSync } = require("node:child_process");
const { existsSync } = require("node:fs");
const { join } = require("node:path");

const PLATFORMS = new Set(["darwin-arm64", "linux-x64", "win32-x64"]);

const FALLBACK =
  "cargo install --git https://github.com/devmaxxx/zerograph --locked";

function binaryPath() {
  const key = `${process.platform}-${process.arch}`;
  if (!PLATFORMS.has(key)) {
    return {
      error: `zerograph: no prebuilt binary for ${key}; build one with \`${FALLBACK}\``,
    };
  }
  const path = join(__dirname, key, process.platform === "win32" ? "zerograph.exe" : "zerograph");
  // Only a pack run that was not given this platform's binary leaves it out.
  if (!existsSync(path)) {
    return {
      error: `zerograph: this package was packed without the ${key} binary; reinstall a published version, or \`${FALLBACK}\``,
    };
  }
  return { path };
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
