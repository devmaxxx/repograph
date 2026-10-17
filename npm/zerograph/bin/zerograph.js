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
// the same version reuses its copy rather than adding a 95 MB one. The size is not proof of the
// bytes (the linker pads a PE file to 512 bytes, so a rebuild often keeps it), so an install the
// slot has not seen is hashed once against the copy; `seen` remembers the installs that matched.
// npm writes every file with one fixed mtime, so an install is told apart by when it created it.
const SLOT_TTL_MS = 30 * 24 * 3600 * 1000;
const TOUCH_MS = 24 * 3600 * 1000;

function sha256(file) {
  const hash = require("node:crypto").createHash("sha256");
  const fd = fs.openSync(file, "r");
  try {
    const buf = Buffer.alloc(1 << 20);
    let n;
    while ((n = fs.readSync(fd, buf, 0, buf.length, null)) > 0) hash.update(buf.subarray(0, n));
  } finally {
    fs.closeSync(fd);
  }
  return hash.digest("hex");
}

function runnable(path, { platform = process.platform, localAppData = process.env.LOCALAPPDATA, now = Date.now() } = {}) {
  if (platform !== "win32" || !localAppData) return path;
  try {
    const { version } = require("../package.json");
    const root = join(localAppData, "zerograph", "bin");
    const src = fs.statSync(path);
    const slot = `${version}-${src.size}`;
    const install = `${src.size}:${Math.trunc(src.ctimeMs)}`;
    const dir = join(root, slot);
    const copy = join(dir, "zerograph.exe");
    const used = join(dir, "used");
    const seen = join(dir, "seen");
    const digest = join(dir, "sha256");
    const read = (f) => { try { return fs.readFileSync(f, "utf8"); } catch { return ""; } };
    const mark = () => {
      // At most once a day, so a slot in use is never the one the sweep below removes.
      try {
        if (now - fs.statSync(used).mtimeMs <= TOUCH_MS) return;
      } catch {
        fs.writeFileSync(used, "");
      }
      fs.utimesSync(used, new Date(now), new Date(now));
    };
    let hash;
    if (fs.existsSync(copy)) {
      if (read(seen).split("\n").includes(install)) { mark(); return copy; }
      hash = sha256(path);
      if (hash === read(digest).trim()) {
        fs.appendFileSync(seen, `${install}\n`);
        mark();
        return copy;
      }
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
      // A full disk, an older build of these bytes still running, or another launcher that renamed
      // the same bytes into place first: only the last leaves a copy that is ours to run.
      fs.rmSync(tmp, { force: true });
      if (!fs.existsSync(copy) || (hash ?? sha256(path)) !== sha256(copy)) return path;
    }
    fs.writeFileSync(digest, hash ?? sha256(path));
    fs.writeFileSync(seen, `${install}\n`);
    mark();
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

// A sweep in another launcher can remove an old slot between `runnable` and the spawn.
function spawnBinary(exe, pkg, args, spawn = spawnSync) {
  const run = spawn(exe, args, { stdio: "inherit" });
  if (run.error && run.error.code === "ENOENT" && exe !== pkg) return spawn(pkg, args, { stdio: "inherit" });
  return run;
}

function main() {
  const found = binaryPath();
  if (found.error) {
    console.error(found.error);
    process.exit(2);
  }
  const run = spawnBinary(runnable(found.path), found.path, process.argv.slice(2));
  if (run.error) {
    console.error(`zerograph: ${run.error.message}`);
    process.exit(2);
  }
  // A signal death has no status; report it the way a shell would.
  process.exit(run.status === null ? 128 : run.status);
}

if (require.main === module) main();
module.exports = { runnable, spawnBinary };
