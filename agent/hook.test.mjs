// The hook, exercised the way a harness runs it: a payload on stdin, a fake `repograph` the hook
// resolves the way it resolves the real one, and a temporary root that has or lacks a store.
// Run: `node --test agent/hook.test.mjs`, on every platform the hook is installed on.
import { test } from 'node:test';
import assert from 'node:assert';
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, existsSync, chmodSync, copyFileSync, symlinkSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, dirname, delimiter } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';

import { queryWords, searchPattern, binary, rule, RULE_FALLBACK, COMMAND } from './hook.mjs';

const HOOK = join(dirname(fileURLToPath(import.meta.url)), 'hook.mjs');

const WIN = process.platform === 'win32';

/**
 * A root with a fake `repograph`; `answer` is what that fake prints on stdout.
 *
 * The fake is a Node script behind a launcher rather than a shell script, because Windows cannot
 * execute an extensionless `#!/bin/sh` file at all — the five tests that need the binary to answer
 * failed there for that reason alone. Node also keeps the answers' Cyrillic out of a batch file's
 * code page, and writes the argv line itself, so the log reads the same on both platforms whatever
 * quoting the launcher needed.
 */
function world({ store = true, answer = '', status = 0 } = {}) {
  const root = mkdtempSync(join(tmpdir(), 'repograph-hook-test-'));
  if (store) {
    mkdirSync(join(root, '.repograph'), { recursive: true });
    writeFileSync(join(root, '.repograph', 'manifest.json'), '{}');
  }
  const bin = join(root, 'bin');
  mkdirSync(bin, { recursive: true });
  const argvLog = join(root, 'argv.txt');
  const fake = join(bin, 'fake.mjs');
  writeFileSync(fake, [
    "import { appendFileSync, writeSync } from 'node:fs';",
    `appendFileSync(${JSON.stringify(argvLog)}, process.argv.slice(2).join(' ') + '\\n', 'utf8');`,
    `writeSync(1, ${JSON.stringify(answer + '\n')});`,
    `process.exit(${status});`,
  ].join('\n'), 'utf8');

  // The launcher is what the hook actually spawns: a `.cmd` on Windows, which is why the hook has
  // to know how to spawn one, and a shell script everywhere else, which is what `PATH` finds.
  const launcher = join(bin, WIN ? 'repograph.cmd' : 'repograph');
  writeFileSync(launcher, WIN
    ? `@echo off\r\n"${process.execPath}" "${fake}" %*\r\n`
    : `#!/bin/sh\nexec "${process.execPath}" "${fake}" "$@"\n`);
  if (!WIN) chmodSync(launcher, 0o755);
  return { root, bin, launcher, argvLog };
}

function fire(w, payload, env = {}, hook = HOOK) {
  const r = spawnSync(process.execPath, [hook], {
    input: JSON.stringify({ cwd: w.root, session_id: 'sess', ...payload }),
    encoding: 'utf8',
    env: {
      ...process.env,
      PATH: `${w.bin}${delimiter}${process.env.PATH}`,
      // Windows resolves a bare name through `PATHEXT` only inside a shell, so `PATH` alone would
      // never find `repograph.cmd`; the variable the hook already honours names it instead.
      ...(WIN ? { REPOGRAPH_BIN: w.launcher } : {}),
      // `tmpdir()` reads TMPDIR on POSIX and TEMP/TMP on Windows. All three, or the per-session
      // state the once-and-cadence gates keep would be shared between tests on one platform.
      TMPDIR: join(w.root, 'tmp'), TEMP: join(w.root, 'tmp'), TMP: join(w.root, 'tmp'),
      ...env,
    },
  });
  assert.equal(r.status, 0, `the hook always exits 0; stderr: ${r.stderr}`);
  return r.stdout ? JSON.parse(r.stdout) : null;
}

function context(out) { return out?.hookSpecificOutput?.additionalContext ?? null; }

function argv(w) { return existsSync(w.argvLog) ? readFileSync(w.argvLog, 'utf8').trim().split('\n') : []; }

/** The argv line matching `re`, waited for: a detached child writes after the hook has exited. */
async function waitFor(w, re, ms = 3000) {
  for (let waited = 0; waited < ms; waited += 50) {
    const hit = argv(w).find((l) => re.test(l));
    if (hit) return hit;
    await new Promise((r) => setTimeout(r, 50));
  }
  return null;
}

/**
 * The hook as `install-agent --command` writes it, alone in a directory with no `rule.txt`: its one
 * declaration replaced by the command JSON-encoded, which is what `src/install_agent.rs` does. A
 * function replacement, because a replacement string would read a `$'` in the command as a pattern.
 */
function installed(w, command) {
  const declaration = 'const COMMAND = "repograph";';
  const source = readFileSync(HOOK, 'utf8');
  assert.equal(source.split(declaration).length, 2, 'the declaration the installer replaces is in the hook once');
  const dir = join(w.root, 'installed');
  mkdirSync(dir, { recursive: true });
  const file = join(dir, 'repograph-hook.mjs');
  writeFileSync(file, source.replace(declaration, () => `const COMMAND = ${JSON.stringify(command)};`));
  return file;
}

/**
 * What `changes --depth 1 --json` prints for a diff: each part touches `symbol` in `file` and has
 * `callers` direct callers spread over `files` files.
 */
function diff(parts, risk) {
  const touched = [];
  const affected = [];
  for (const { file, symbol, callers, files = callers } of parts) {
    touched.push({ id: `sym:${file}::${symbol}`, at: `${file}:3-40`, indexed: true });
    for (let i = 0; i < callers; i += 1) {
      affected.push({ id: `sym:src/${symbol}/c${i % files}.ts::caller${i}`, at: `src/${symbol}/c${i % files}.ts:${i + 1}`,
        depth: 1, kind: 'Calls', via: `sym:${file}::${symbol}` });
    }
  }
  return JSON.stringify({ touched, affected, files: [], risk });
}

function edit(w, ...segments) {
  return { hook_event_name: 'PostToolUse', tool_name: 'Edit', tool_input: { file_path: join(w.root, ...segments) } };
}

test('a search pattern is read out of Bash, Grep and Glob, and nowhere else', () => {
  assert.equal(searchPattern('Bash', { command: 'rg -n "cancellation policy" apps/' }), 'cancellation policy',
    'a quoted phrase is one argument: splitting on whitespace would ask about its first word only');
  assert.equal(searchPattern('Bash', { command: 'grep -R withTenant .' }), 'withTenant');
  assert.equal(searchPattern('Bash', { command: 'ls -la' }), null);
  assert.equal(searchPattern('Grep', { pattern: 'tenant isolation' }), 'tenant isolation');
  assert.equal(searchPattern('Glob', { pattern: '**/tenant-*.ts' }), 'tenant');
  assert.equal(searchPattern('Glob', { pattern: '**/*.ts' }), null);
  assert.equal(searchPattern('Read', { file_path: 'a.ts' }), null, 'reading a named file is not a search');
});

test('`-e` and `--regexp` carry the pattern, not a value to skip', () => {
  assert.equal(searchPattern('Bash', { command: 'rg -n -e "cancellation policy" packages' }), 'cancellation policy',
    'skipped as a flag value, the graph was asked about the path after it');
  assert.equal(searchPattern('Bash', { command: 'grep -R --regexp withTenant .' }), 'withTenant');
});

test('a stage a pipe feeds reads stdin, not the repository, and is not asked about', () => {
  for (const command of [
    'gh auth status 2>&1 | grep -E "Logged in|Active account"',
    'rg --files | rg tenancy',
    'git log --oneline |& grep cancellation',
    "ls -a packages | grep -i 'lintstaged\\|lint-staged'",
    'git log --oneline |\n  grep cancellation',
    'git log --oneline \\\n  | grep cancellation',
    'rg --files \\\n  | rg tenancy',
  ]) assert.equal(searchPattern('Bash', { command }), null, command);
  assert.equal(searchPattern('Bash', { command: 'rg -n "refund|cancellation" apps/ | head -5' }), 'refund|cancellation',
    'a `|` inside quotes is the pattern, and its stage reads the repository');
  assert.equal(searchPattern('Bash', { command: 'grep -R refund\\|cancellation apps' }), 'refund\\|cancellation',
    'an escaped `|` is not a pipe either');
  assert.equal(searchPattern('Bash', { command: 'cd apps && rg -n withTenant src' }), 'withTenant');
  assert.equal(searchPattern('Bash', { command: 'rg -n \\\n  cancellation packages' }), 'cancellation',
    'a line continuation reads as a space, so its backslash is not the pattern');
  assert.equal(searchPattern('Bash', { command: 'git diff --quiet || grep -R withTenant packages' }), 'withTenant',
    '`||` runs a second command, it does not feed one');

  const w = world({ answer: 'sym:a.ts::accountRow  a.ts:1  accountRow' });
  const payload = { hook_event_name: 'PreToolUse', tool_name: 'Bash', tool_input: { command: 'gh auth status 2>&1 | grep -E "Logged in|Active account"' } };
  assert.equal(fire(w, payload), null);
  assert.deepEqual(argv(w), [], 'the graph was not asked');
});

test('a question is words, a path is not, and an identifier goes whole', () => {
  assert.deepEqual(queryWords('cancellation policy'), ['cancellation', 'policy']);
  assert.deepEqual(queryWords('отмена записи'), ['отмена', 'записи']);
  assert.equal(queryWords('src/app/'), null);
  assert.equal(queryWords('foo'), null, 'three letters is a flag more often than a concept');
  assert.deepEqual(queryWords('withTenant'), ['withTenant']);
  assert.deepEqual(queryWords('FR-PAY-22'), ['FR-PAY-22']);
  assert.deepEqual(queryWords('a.b*'), null);
  assert.equal(queryWords('cancellation refunds deposits invoices приложение расписание календарь').length, 6,
    'six words at most; the seventh never changed an answer and every one costs prompt');
});

test('a milestone or task id goes whole, and neither an escape nor a leading dash reaches ask', () => {
  assert.deepEqual(queryWords('BE-M17'), ['BE-M17'], 'no four-letter word to fall back on: without the id nothing is asked');
  assert.deepEqual(queryWords('BE-M01-T03'), ['BE-M01-T03']);
  assert.deepEqual(queryWords('\\bwithTenant\\b'), ['withTenant'], 'dropping only the backslash asks about bwithTenant');
  assert.deepEqual(queryWords('\\bFR-CAL-40\\b'), ['FR-CAL-40']);
  assert.deepEqual(queryWords('--rerank misses'), ['rerank', 'misses']);

  const w = world({ answer: 'FR-PAY-22  docs/a.md:1  отмена' });
  fire(w, { hook_event_name: 'PreToolUse', tool_name: 'Bash', tool_input: { command: 'rg -n -- "--rerank misses" docs' } });
  assert.match(argv(w)[0], /ask --stale --seeds 3 rerank misses$/);
  assert.ok(!argv(w)[0].includes('--rerank'), `ask would read it as its paid flag: ${argv(w)[0]}`);
});

test('the local shim the hook picks is one this platform can start', () => {
  const w = world();
  const bin = join(w.root, 'node_modules', '.bin');
  mkdirSync(bin, { recursive: true });
  const named = process.env.REPOGRAPH_BIN;
  delete process.env.REPOGRAPH_BIN;                 // this is the resolution when nothing names it
  try {
    assert.equal(binary(w.root), 'repograph', 'nothing installed locally: PATH answers');

    // The trio npm, pnpm and yarn write, in the order they appear: the shell script first.
    writeFileSync(join(bin, 'repograph'), '#!/bin/sh\n');
    assert.equal(binary(w.root), WIN ? 'repograph' : join(bin, 'repograph'),
      'the extensionless shim is not a fallback on Windows: CreateProcess cannot read it, and PATH can');

    writeFileSync(join(bin, 'repograph.cmd'), '@echo off\r\n');
    assert.equal(binary(w.root), join(bin, WIN ? 'repograph.cmd' : 'repograph'));
  } finally {
    if (named !== undefined) process.env.REPOGRAPH_BIN = named;
  }
});

test('the rule the hook hands a subagent is the file the installer ships', () => {
  const shipped = readFileSync(join(dirname(HOOK), 'rule.txt'), 'utf8');
  assert.equal(rule(), shipped);
  assert.equal(RULE_FALLBACK.trim(), shipped.split('{{command}}').join(COMMAND).trim(),
    'the built-in copy and the file cannot drift');
});

test('SessionStart prints the brief, and nothing where there is no store', () => {
  const w = world({ answer: 'repograph: 3076 doc nodes, 5240 code nodes\nenriched=true (1996/1996 nodes)' });
  const c = context(fire(w, { hook_event_name: 'SessionStart', source: 'startup' }));
  assert.ok(c.startsWith('repograph:'), c);
  assert.ok(Buffer.byteLength(c) <= 700, `${Buffer.byteLength(c)} B`);
  assert.ok(argv(w).some((l) => /--no-dense prime/.test(l)), argv(w).join(' | '));

  const bare = world({ store: false });
  assert.equal(fire(bare, { hook_event_name: 'SessionStart', source: 'startup' }), null);
});

test('a session started in a subdirectory finds the index above it, and a nested checkout does not borrow it', () => {
  const w = world({ answer: 'repograph: 3076 doc nodes, 5240 code nodes' });
  const env = { REPOGRAPH_HOOK_SERVE: '0' };
  const sub = join(w.root, 'packages', 'db');
  mkdirSync(sub, { recursive: true });
  const c = context(fire(w, { hook_event_name: 'SessionStart', source: 'startup', cwd: sub }, env));
  assert.ok(c?.startsWith('repograph:'), `a cd into a subdirectory lost the index: ${c}`);
  assert.ok(argv(w).some((l) => l.startsWith(`--repo ${w.root} --no-dense prime`)), argv(w).join(' | '));

  const nested = join(w.root, 'worktree');
  mkdirSync(join(nested, 'src'), { recursive: true });
  writeFileSync(join(nested, '.git'), 'gitdir: ../.git/worktrees/worktree\n');
  assert.equal(fire(w, { hook_event_name: 'SessionStart', source: 'startup', cwd: join(nested, 'src') }, env), null,
    'a checkout of its own ends the walk: its parent\'s index is not its index');
});

test('a hook run through a symlinked path still answers', () => {
  const w = world({ answer: 'repograph: 3076 doc nodes, 5240 code nodes' });
  const real = join(w.root, 'real');
  mkdirSync(real);
  copyFileSync(HOOK, join(real, 'hook.mjs'));
  const link = join(w.root, 'link');
  // A junction on Windows, which needs no symlink privilege; the type is ignored everywhere else.
  symlinkSync(real, link, 'junction');
  const c = context(fire(w, { hook_event_name: 'SessionStart', source: 'startup' }, { REPOGRAPH_HOOK_SERVE: '0' }, join(link, 'hook.mjs')));
  assert.ok(c?.startsWith('repograph:'), `argv[1] keeps the link and import.meta.url resolves it: ${c}`);
});

test('SessionStart starts a resident serve, and the switch turns it off', async () => {
  const w = world({ answer: 'repograph: 3076 doc nodes, 5240 code nodes' });
  fire(w, { hook_event_name: 'SessionStart', source: 'startup' });
  // Detached and never waited on, so the fake's line lands after the hook has already exited.
  const line = await waitFor(w, /serve --idle 1800 --idle-model 300/);
  assert.ok(line, `no serve was started: ${argv(w).join(' | ')}`);
  assert.ok(!line.includes('--no-dense'), `the resident answer is the fused one: ${line}`);

  const off = world({ answer: 'repograph: 1 doc node' });
  fire(off, { hook_event_name: 'SessionStart', source: 'startup' }, { REPOGRAPH_HOOK_SERVE: '0' });
  await new Promise((r) => setTimeout(r, 300));
  assert.equal(argv(off).filter((l) => l.includes('serve')).length, 0, argv(off).join(' | '));
});

test('SubagentStart carries the rule', () => {
  const w = world();
  const c = context(fire(w, { hook_event_name: 'SubagentStart', agent_id: 'a1' }));
  assert.equal(c.trim(), readFileSync(join(dirname(HOOK), 'rule.txt'), 'utf8').trim());
  assert.deepEqual(argv(w), [], 'the rule costs no call');
});

test('a Bash rg with a word asks once, with three seeds, and keeps only the seed lines', () => {
  const w = world({ answer: 'FR-PAY-22  docs/a.md:1  отмена\nFR-PAY-26  docs/a.md:9  возврат\n  BE-M10  docs/b.md:3  neighbour' });
  const payload = { hook_event_name: 'PreToolUse', tool_name: 'Bash', tool_input: { command: 'rg -n "отмена записи" apps/' } };
  const c = context(fire(w, payload));
  assert.match(argv(w)[0], /ask --stale --seeds 3/);
  assert.ok(!c.includes('BE-M10'), `the indented neighbour line is dropped: ${c}`);
  assert.equal(c.split('\n').filter((l) => l.startsWith('FR-')).length, 2, c);
  assert.equal(fire(w, payload), null, 'the same query is asked once a session');
});

test('a search that touches repograph itself, or the store, is left alone', () => {
  const w = world({ answer: 'FR-PAY-22  docs/a.md:1  отмена' });
  for (const command of ['repograph ask отмена | rg FR', 'rg -l TODO .repograph/']) {
    assert.equal(fire(w, { hook_event_name: 'PreToolUse', tool_name: 'Bash', tool_input: { command } }), null, command);
  }
  assert.deepEqual(argv(w), [], 'the fake never ran');
});

test('an answer that is empty, or a command that fails, is silence', () => {
  for (const w of [world({ answer: '' }), world({ answer: 'x', status: 1 })]) {
    const out = fire(w, { hook_event_name: 'PreToolUse', tool_name: 'Bash', tool_input: { command: 'rg "cancellation policy"' } });
    assert.equal(out, null);
  }
});

test('the twelfth injection is the last', () => {
  const w = world({ answer: 'FR-PAY-22  docs/a.md:1  отмена' });
  let seen = 0;
  for (let i = 0; i < 15; i += 1) {
    const out = fire(w, { hook_event_name: 'PreToolUse', tool_name: 'Bash', tool_input: { command: `rg "cancellation policy number${i}part"` } });
    if (out) seen += 1;
  }
  assert.equal(seen, 12, 'a bounded worst case is what makes this a bet rather than a tax');
});

test('with the interceptor off, the reminder still keeps its cadence, per agent', () => {
  const w = world({ answer: 'FR-PAY-22  docs/a.md:1  отмена' });
  const env = { REPOGRAPH_HOOK_INTERCEPT: '0' };
  let reminders = 0;
  for (let i = 0; i < 40; i += 1) {
    const out = fire(w, { hook_event_name: 'PreToolUse', tool_name: 'Bash', tool_input: { command: `rg "thing${i}"` } }, env);
    if (out) reminders += 1;
  }
  assert.equal(reminders, 1, 'one reminder in forty watched calls');
  assert.deepEqual(argv(w), [], 'and no ask was run');

  // A subagent shares its parent's session id and must still count from zero.
  const out = fire(w, { hook_event_name: 'PreToolUse', agent_id: 'sub1', tool_name: 'Bash', tool_input: { command: 'rg "thing0"' } }, env);
  assert.equal(out, null, 'the subagent is at call one of its own forty, not at forty-one');
});

test('an edit to a hub injects one risk line, once, and only above LOW', () => {
  const changes = diff([{ file: 'src/db.ts', symbol: 'DatabaseService.withTenant', callers: 18 }], 'HIGH');
  const w = world({ answer: changes });
  const payload = edit(w, 'src', 'db.ts');
  const c = context(fire(w, payload));
  assert.match(c, /risk HIGH — 18 direct callers of DatabaseService\.withTenant in 18 files/);
  assert.match(argv(w)[0], /changes --depth 1 --json/);
  assert.equal(fire(w, payload), null, 'once per file and risk');

  const low = world({ answer: JSON.stringify({ risk: 'LOW', touched: [], affected: [], files: [] }) });
  assert.equal(fire(low, edit(low, 'src', 'db.ts')), null);

  const doc = world({ answer: changes });
  assert.equal(fire(doc, edit(doc, 'x.md')), null);
  assert.deepEqual(argv(doc), [], 'a document edit does not even ask');
});

test('an edit is scored on its own file\'s callers, not on the rest of the diff', () => {
  const w = world({ answer: diff([
    { file: 'src/db.ts', symbol: 'DatabaseService.withTenant', callers: 40 },
    { file: 'src/claims.ts', symbol: 'staleClaims', callers: 1 },
    { file: 'src/slots.ts', symbol: 'freeSlots', callers: 6, files: 2 },
  ], 'CRITICAL') });
  assert.equal(fire(w, edit(w, 'src', 'claims.ts')), null, 'one caller is silence, whatever else the diff holds');
  assert.match(context(fire(w, edit(w, 'src', 'slots.ts'))), /risk MEDIUM — 6 direct callers of freeSlots in 2 files/,
    'the level is the file\'s own, not the CRITICAL the whole diff scored');
});

test('a caller that imports through a barrel is still a caller of the edited file', () => {
  const via = 'sym:src/index.ts::DatabaseService.withTenant';
  const affected = Array.from({ length: 6 }, (_, i) => ({ id: `sym:src/c${i}.ts::caller`, at: `src/c${i}.ts:1`, depth: 1, via }));
  const touched = [{ id: 'sym:src/db.ts::DatabaseService', at: 'src/db.ts:3-40', indexed: true }];
  const w = world({ answer: JSON.stringify({ touched, affected, files: [], risk: 'HIGH' }) });
  assert.match(context(fire(w, edit(w, 'src', 'db.ts'))), /risk MEDIUM — 6 direct callers of DatabaseService\.withTenant in 6 files/,
    'the barrel alias names the member, not the file it was declared in');
  const clash = world({ answer: JSON.stringify({ touched: [...touched, { id: 'sym:src/other.ts::DatabaseService', at: 'src/other.ts:1-9', indexed: true }], affected, files: [], risk: 'HIGH' }) });
  assert.equal(fire(clash, edit(clash, 'src', 'db.ts')), null, 'a name two files in the diff share is not attributed by name');
});

test('the level is the binary\'s: MEDIUM at 5 callers or 3 files, HIGH at 15 or 10, CRITICAL at 30 or 25', () => {
  for (const [callers, files, level] of [
    [4, 2, null], [5, 1, 'MEDIUM'], [4, 3, 'MEDIUM'],
    [15, 1, 'HIGH'], [10, 10, 'HIGH'], [30, 1, 'CRITICAL'], [25, 25, 'CRITICAL'],
  ]) {
    const w = world({ answer: diff([{ file: 'src/hub.ts', symbol: 'hub', callers, files }], 'CRITICAL') });
    const c = context(fire(w, edit(w, 'src', 'hub.ts')));
    assert.equal(c?.match(/risk (\w+)/)?.[1] ?? null, level, `${callers} callers in ${files} files: ${c}`);
  }
});

test('the Agent prompt is extended only when the fallback is asked for', () => {
  const w = world();
  const payload = { hook_event_name: 'PreToolUse', tool_name: 'Agent', tool_input: { prompt: 'find the tenant guard' } };
  assert.equal(fire(w, payload), null, 'off by default: SubagentStart is the primary and this would double it');
  const out = fire(w, payload, { REPOGRAPH_HOOK_AGENT_INPUT: '1' });
  assert.ok(out.hookSpecificOutput.updatedInput.prompt.startsWith('find the tenant guard'), JSON.stringify(out));
  assert.ok(out.hookSpecificOutput.updatedInput.prompt.endsWith(rule()), JSON.stringify(out));
  assert.equal(out.hookSpecificOutput.permissionDecision, undefined, 'no decision: the call proceeds as it would have');
});

test('a repository with no store gets one build notice and then silence', () => {
  const w = world({ store: false });
  const payload = { hook_event_name: 'PreToolUse', tool_name: 'Bash', tool_input: { command: 'rg "cancellation policy"' } };
  assert.match(context(fire(w, payload)), /repograph build/);
  assert.equal(fire(w, { ...payload, tool_input: { command: 'rg "another question here"' } }), null, 'once a session');
});

test('every hint names the command install-agent was given, and no command can break the script', () => {
  const env = { REPOGRAPH_HOOK_SERVE: '0' };
  const command = 'pnpm exec repograph';
  const search = { hook_event_name: 'PreToolUse', tool_name: 'Bash', tool_input: { command: 'rg "cancellation policy"' } };

  const bare = world({ store: false });
  assert.match(context(fire(bare, search, env, installed(bare, command))), /`pnpm exec repograph build`/);

  const w = world({ answer: 'FR-PAY-22  docs/a.md:1  отмена' });
  const hook = installed(w, command);
  const given = context(fire(w, { hook_event_name: 'SubagentStart', agent_id: 'a1' }, env, hook));
  assert.match(given, /`pnpm exec repograph ask <words>`/, 'no rule.txt beside it: the built-in copy answers');
  assert.ok(!given.includes('`repograph '), given);
  const injected = context(fire(w, search, env, hook));
  assert.match(injected, /^pnpm exec repograph ask cancellation policy →/, 'the header is a command an agent re-runs');
  assert.match(injected, /`pnpm exec repograph ask` for more\)$/);
  let reminder = null;
  for (let i = 0; i < 40; i += 1) {
    const payload = { hook_event_name: 'PreToolUse', agent_id: 'r', tool_name: 'Bash', tool_input: { command: `rg "thing${i}"` } };
    reminder = context(fire(w, payload, { ...env, REPOGRAPH_HOOK_INTERCEPT: '0' }, hook)) ?? reminder;
  }
  assert.match(reminder, /`pnpm exec repograph ask <words>`/);

  const hub = world({ answer: diff([{ file: 'src/db.ts', symbol: 'withTenant', callers: 18 }], 'HIGH') });
  assert.match(context(fire(hub, edit(hub, 'src', 'db.ts'), env, installed(hub, command))), /`pnpm exec repograph changes --depth 1`/);

  const hostile = 'pnpm exec "re\'po\\graph" `x` ${y} $\'';
  const odd = world();
  const said = context(fire(odd, { hook_event_name: 'SubagentStart', agent_id: 'a1' }, env, installed(odd, hostile)));
  assert.ok(said.includes(`\`${hostile} ask <words>\``), `a JSON string is a JavaScript string literal: ${said}`);
});

test('malformed input is silence with exit zero', () => {
  const w = world();
  const r = spawnSync(process.execPath, [HOOK], { input: 'not json at all', encoding: 'utf8', env: { ...process.env, TMPDIR: join(w.root, 'tmp') } });
  assert.equal(r.status, 0);
  assert.equal(r.stdout, '');
});
