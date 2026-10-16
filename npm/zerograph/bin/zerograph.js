#!/usr/bin/env node
"use strict";

// Every prebuilt binary ships inside this one package, under bin/<platform>-<arch>/, so an
// install needs no second package, no install script and no network beyond the tarball.
const { spawnSync } = require("node:child_process");
const fs = require("node:fs");
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
  if (!fs.existsSync(path)) {
    return {
      error: `zerograph: this package was packed without the ${key} binary; reinstall a published version, or \`${FALLBACK}\``,
    };
  }
  return { path };
}

// Windows refuses to replace an image a process is running, so an `npm i -g` over a live `serve`
// failed with EBUSY. The binary runs from a copy kept per version under LOCALAPPDATA instead, and
// the package's own file is never held open. Any failure here runs the package's file as before.
function runnable(path) {
  if (process.platform !== "win32" || !process.env.LOCALAPPDATA) return path;
  try {
    const { version } = require("../package.json");
    const root = join(process.env.LOCALAPPDATA, "zerograph", "bin");
    const dir = join(root, version);
    const copy = join(dir, "zerograph.exe");
    // The size tells a finished copy from one a killed launcher left, and two packs of one version
    // apart; npm writes every file with the same fixed mtime, so the time cannot.
    if (fs.existsSync(copy) && fs.statSync(copy).size === fs.statSync(path).size) return copy;
    fs.mkdirSync(dir, { recursive: true });
    const tmp = `${copy}.${process.pid}.tmp`;
    fs.copyFileSync(path, tmp);
    try {
      fs.renameSync(tmp, copy);
    } catch {
      // Another launcher won the race, or the old copy is running; neither copy is ours to keep.
      fs.rmSync(tmp, { force: true });
      if (!fs.existsSync(copy) || fs.statSync(copy).size !== fs.statSync(path).size) return path;
    }
    // Copies of other versions go once nothing runs them; one still running stays for next time.
    for (const other of fs.readdirSync(root)) {
      if (other !== version) {
        try { fs.rmSync(join(root, other), { recursive: true, force: true }); } catch {}
      }
    }
    return copy;
  } catch {
    return path;
  }
}

const found = binaryPath();
if (found.error) {
  console.error(found.error);
  process.exit(2);
}

const run = spawnSync(runnable(found.path), process.argv.slice(2), { stdio: "inherit" });
if (run.error) {
  console.error(`zerograph: ${run.error.message}`);
  process.exit(2);
}
// A signal death has no status; report it the way a shell would.
process.exit(run.status === null ? 128 : run.status);
