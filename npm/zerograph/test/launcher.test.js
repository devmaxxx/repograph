"use strict";

// The Windows copy logic runs on every platform here: `runnable` takes the platform and
// LOCALAPPDATA as arguments, so the slot rules are pinned without a Windows box.
const test = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const os = require("node:os");
const { join } = require("node:path");
const { runnable, spawnBinary } = require("../bin/zerograph.js");
const { version } = require("../package.json");

const DAY = 24 * 3600 * 1000;

function sandbox() {
  const base = fs.mkdtempSync(join(os.tmpdir(), "zerograph-launcher-"));
  const pkg = join(base, "pkg.exe");
  fs.writeFileSync(pkg, "x".repeat(1000));
  const app = join(base, "app");
  const root = join(app, "zerograph", "bin");
  return { pkg, app, root, win: (now = Date.now()) => ({ platform: "win32", localAppData: app, now }) };
}

function slot(root, name, lastUsed) {
  const dir = join(root, name);
  fs.mkdirSync(dir, { recursive: true });
  fs.writeFileSync(join(dir, "zerograph.exe"), "old");
  fs.writeFileSync(join(dir, "used"), "");
  fs.utimesSync(join(dir, "used"), new Date(lastUsed), new Date(lastUsed));
  return dir;
}

test("off Windows the package's own binary runs", () => {
  const s = sandbox();
  assert.strictEqual(runnable(s.pkg, { platform: "linux", localAppData: s.app }), s.pkg);
  assert.strictEqual(fs.existsSync(s.root), false);
});

test("the binary runs from a copy, and a second launch reuses it", () => {
  const s = sandbox();
  const first = runnable(s.pkg, s.win());
  assert.notStrictEqual(first, s.pkg);
  assert.strictEqual(fs.readFileSync(first, "utf8"), fs.readFileSync(s.pkg, "utf8"));
  assert.strictEqual(runnable(s.pkg, s.win()), first);
});

test("a reinstall of the same version reuses its copy instead of adding one", () => {
  const s = sandbox();
  const first = runnable(s.pkg, s.win());
  fs.rmSync(s.pkg);
  fs.writeFileSync(s.pkg, "x".repeat(1000));
  assert.strictEqual(runnable(s.pkg, s.win()), first);
  assert.deepStrictEqual(fs.readdirSync(s.root), [`${version}-1000`]);
});

test("another version's copy used within the month stays", () => {
  const s = sandbox();
  const now = Date.now();
  const other = slot(s.root, "0.6.0-999", now - 5 * DAY);
  runnable(s.pkg, s.win(now));
  assert.ok(fs.existsSync(join(other, "zerograph.exe")));
});

test("a copy nothing launched for a month is swept", () => {
  const s = sandbox();
  const now = Date.now();
  const stale = slot(s.root, "0.6.0-999", now - 31 * DAY);
  runnable(s.pkg, s.win(now));
  assert.strictEqual(fs.existsSync(stale), false);
});

test("launching an old copy marks it, so the next sweep keeps it", () => {
  const s = sandbox();
  const now = Date.now();
  const mine = runnable(s.pkg, s.win(now - 29 * DAY));
  assert.strictEqual(runnable(s.pkg, s.win(now)), mine);
  const used = fs.statSync(join(mine, "..", "used")).mtimeMs;
  assert.ok(now - used < DAY, "the launch did not mark the slot");
});

test("a rebuild of the same version and size runs the new bytes, not the old copy", () => {
  const s = sandbox();
  runnable(s.pkg, s.win());
  // Linux stamps ctime from a coarse clock, so a rewrite within one tick would keep the install's
  // key; on Windows the stamp is 100 ns and two installs never share it.
  Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, 50);
  fs.rmSync(s.pkg);
  fs.writeFileSync(s.pkg, "y".repeat(1000));
  const copy = runnable(s.pkg, s.win());
  assert.strictEqual(fs.readFileSync(copy, "utf8"), "y".repeat(1000));
});

test("a copy swept before the spawn falls back to the package's binary", () => {
  const calls = [];
  const spawn = (exe) => {
    calls.push(exe);
    return exe === "copy.exe" ? { error: Object.assign(new Error("gone"), { code: "ENOENT" }) } : { status: 0 };
  };
  assert.strictEqual(spawnBinary("copy.exe", "pkg.exe", [], spawn).status, 0);
  assert.deepStrictEqual(calls, ["copy.exe", "pkg.exe"]);
});

test("a binary that fails for another reason is not run twice", () => {
  const calls = [];
  const spawn = (exe) => {
    calls.push(exe);
    return { error: Object.assign(new Error("denied"), { code: "EACCES" }) };
  };
  assert.strictEqual(spawnBinary("copy.exe", "pkg.exe", [], spawn).error.code, "EACCES");
  assert.deepStrictEqual(calls, ["copy.exe"]);
});
