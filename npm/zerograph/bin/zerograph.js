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
// failed with EBUSY. The binary runs from a copy under LOCALAPPDATA instead, and the package's own
// file is never held open. Any failure here runs the package's file as before.
//
// One slot per version and size, so a project's devDependency and a global install at different
// versions keep their own copies instead of evicting each other on every run, and a reinstall of
// the same version reuses its copy rather than adding a 95 MB one. A published version's bytes
// never change, so the size only tells unpublished packs apart.
const SLOT_TTL_MS = 30 * 24 * 3600 * 1000;
const TOUCH_MS = 24 * 3600 * 1000;

function runnable(path, { platform = process.platform, localAppData = process.env.LOCALAPPDATA, now = Date.now() } = {}) {
  if (platform !== "win32" || !localAppData) return path;
  try {
    const { version } = require("../package.json");
    const root = join(localAppData, "zerograph", "bin");
    const slot = `${version}-${fs.statSync(path).size}`;
    const dir = join(root, slot);
    const copy = join(dir, "zerograph.exe");
    const used = join(dir, "used");
    if (fs.existsSync(copy)) {
      // Marked at most once a day, so a slot in use is never the one the sweep below removes.
      try {
        if (now - fs.statSync(used).mtimeMs > TOUCH_MS) fs.utimesSync(used, new Date(now), new Date(now));
      } catch {
        fs.writeFileSync(used, "");
      }
      return copy;
    }
    fs.mkdirSync(dir, { recursive: true });
    // Left by a launcher killed mid-copy; one another launcher is still writing stays locked.
    for (const f of fs.readdirSync(dir)) {
      if (f.endsWith(".tmp")) { try { fs.rmSync(join(dir, f), { force: true }); } catch {} }
    }
    const tmp = `${copy}.${process.pid}.tmp`;
    try {
      fs.copyFileSync(path, tmp);
      fs.renameSync(tmp, copy);
    } catch (e) {
      // A full disk, or another launcher that renamed the same bytes into place first.
      fs.rmSync(tmp, { force: true });
      if (!fs.existsSync(copy)) throw e;
    }
    fs.writeFileSync(used, "");
    fs.utimesSync(used, new Date(now), new Date(now));
    // Slots nothing has launched for a month; one whose binary still runs fails the delete and
    // stays.
    for (const other of fs.readdirSync(root)) {
      if (other === slot) continue;
      const o = join(root, other);
      try {
        let last;
        try { last = fs.statSync(join(o, "used")).mtimeMs; } catch { last = fs.statSync(o).mtimeMs; }
        if (now - last > SLOT_TTL_MS) fs.rmSync(o, { recursive: true, force: true });
      } catch {}
    }
    return copy;
  } catch {
    return path;
  }
}

function main() {
  const found = binaryPath();
  if (found.error) {
    console.error(found.error);
    process.exit(2);
  }
  const args = process.argv.slice(2);
  const exe = runnable(found.path);
  let run = spawnSync(exe, args, { stdio: "inherit" });
  // A sweep in another launcher can remove an old slot between our check and the spawn.
  if (run.error && run.error.code === "ENOENT" && exe !== found.path) {
    run = spawnSync(found.path, args, { stdio: "inherit" });
  }
  if (run.error) {
    console.error(`zerograph: ${run.error.message}`);
    process.exit(2);
  }
  // A signal death has no status; report it the way a shell would.
  process.exit(run.status === null ? 128 : run.status);
}

if (require.main === module) main();
module.exports = { runnable };
