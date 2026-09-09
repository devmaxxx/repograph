// The hook, exercised the way a harness runs it: a payload on stdin, a fake `repograph` on PATH
// that records its argv, and a temporary root that has or lacks a store. Run: `node --test agent/`.
import { test } from 'node:test';
import assert from 'node:assert';
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, existsSync, chmodSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';

import { queryWords, searchPattern, rule, RULE_FALLBACK } from './hook.mjs';

const HOOK = join(dirname(fileURLToPath(import.meta.url)), 'hook.mjs');

/** A root with a fake `repograph` on its PATH; `answer` is what that fake prints on stdout. */
function world({ store = true, answer = '', status = 0 } = {}) {
  const root = mkdtempSync(join(tmpdir(), 'repograph-hook-test-'));
  if (store) {
    mkdirSync(join(root, '.repograph'), { recursive: true });
    writeFileSync(join(root, '.repograph', 'manifest.json'), '{}');
  }
  const bin = join(root, 'bin');
  mkdirSync(bin, { recursive: true });
  const argvLog = join(root, 'argv.txt');
  writeFileSync(join(bin, 'repograph'),
    `#!/bin/sh\nprintf '%s\\n' "$*" >> ${JSON.stringify(argvLog)}\ncat <<'EOF'\n${answer}\nEOF\nexit ${status}\n`);
  chmodSync(join(bin, 'repograph'), 0o755);
  return { root, bin, argvLog, state: join(tmpdir(), 'repograph-hook') };
}

function fire(w, payload, env = {}) {
  const r = spawnSync(process.execPath, [HOOK], {
    input: JSON.stringify({ cwd: w.root, session_id: 'sess', ...payload }),
    encoding: 'utf8',
    env: { ...process.env, PATH: `${w.bin}:${process.env.PATH}`, TMPDIR: join(w.root, 'tmp'), ...env },
  });
  assert.equal(r.status, 0, `the hook always exits 0; stderr: ${r.stderr}`);
  return r.stdout ? JSON.parse(r.stdout) : null;
}

function context(out) { return out?.hookSpecificOutput?.additionalContext ?? null; }

function argv(w) { return existsSync(w.argvLog) ? readFileSync(w.argvLog, 'utf8').trim().split('\n') : []; }

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

test('the rule the hook hands a subagent is the file the installer ships', () => {
  const shipped = readFileSync(join(dirname(HOOK), 'rule.txt'), 'utf8');
  assert.equal(rule(), shipped);
  assert.equal(RULE_FALLBACK.trim(), shipped.trim(), 'the built-in copy and the file cannot drift');
});

test('SessionStart prints the brief, and nothing where there is no store', () => {
  const w = world({ answer: 'repograph: 3076 doc nodes, 5240 code nodes\nenriched=true (1996/1996 nodes)' });
  const c = context(fire(w, { hook_event_name: 'SessionStart', source: 'startup' }));
  assert.ok(c.startsWith('repograph:'), c);
  assert.ok(Buffer.byteLength(c) <= 700, `${Buffer.byteLength(c)} B`);
  assert.match(argv(w)[0], /--no-dense prime/);

  const bare = world({ store: false });
  assert.equal(fire(bare, { hook_event_name: 'SessionStart', source: 'startup' }), null);
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
  const changes = JSON.stringify({
    risk: 'CRITICAL',
    touched: [{ id: 'sym:a.ts::DatabaseService.withTenant' }],
    affected: Array.from({ length: 61 }, () => ({ depth: 1 })),
    files: Array.from({ length: 48 }, (_, i) => `f${i}.ts`),
  });
  const w = world({ answer: changes });
  const payload = { hook_event_name: 'PostToolUse', tool_name: 'Edit', tool_input: { file_path: 'x.ts' } };
  const c = context(fire(w, payload));
  assert.match(c, /risk CRITICAL — 61 direct callers of DatabaseService\.withTenant in 48 files/);
  assert.match(argv(w)[0], /changes --depth 1 --json/);
  assert.equal(fire(w, payload), null, 'once per file and risk');

  const low = world({ answer: JSON.stringify({ risk: 'LOW', touched: [], affected: [], files: [] }) });
  assert.equal(fire(low, payload), null);

  const doc = world({ answer: changes });
  assert.equal(fire(doc, { ...payload, tool_input: { file_path: 'x.md' } }), null);
  assert.deepEqual(argv(doc), [], 'a document edit does not even ask');
});

test('the Agent prompt is extended only when the fallback is asked for', () => {
  const w = world();
  const payload = { hook_event_name: 'PreToolUse', tool_name: 'Agent', tool_input: { prompt: 'find the tenant guard' } };
  assert.equal(fire(w, payload), null, 'off by default: SubagentStart is the primary and this would double it');
  const out = fire(w, payload, { REPOGRAPH_HOOK_AGENT_INPUT: '1' });
  assert.ok(out.hookSpecificOutput.updatedInput.prompt.startsWith('find the tenant guard'), JSON.stringify(out));
  assert.ok(out.hookSpecificOutput.updatedInput.prompt.includes('repograph ask'), JSON.stringify(out));
  assert.equal(out.hookSpecificOutput.permissionDecision, undefined, 'no decision: the call proceeds as it would have');
});

test('a repository with no store gets one build notice and then silence', () => {
  const w = world({ store: false });
  const payload = { hook_event_name: 'PreToolUse', tool_name: 'Bash', tool_input: { command: 'rg "cancellation policy"' } };
  assert.match(context(fire(w, payload)), /repograph build/);
  assert.equal(fire(w, { ...payload, tool_input: { command: 'rg "another question here"' } }), null, 'once a session');
});

test('malformed input is silence with exit zero', () => {
  const w = world();
  const r = spawnSync(process.execPath, [HOOK], { input: 'not json at all', encoding: 'utf8', env: { ...process.env, TMPDIR: join(w.root, 'tmp') } });
  assert.equal(r.status, 0);
  assert.equal(r.stdout, '');
});
