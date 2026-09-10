#!/usr/bin/env node
/**
 * The one repograph hook, dispatched on the event the harness names in its payload.
 *
 *  SessionStart   → a repository brief, so the graph's state and its five commands are in context
 *                   at start and again after a compaction, without a 7k-token CLAUDE.md.
 *  PreToolUse     → a Bash/Grep/Glob search gets the graph's top three lines beside it, under gates
 *                   written to keep an injection out rather than to put one in: an injection is
 *                   paid once as output and then on every later turn as input. Read is not a search.
 *  PostToolUse    → an Edit/Write to TypeScript gets one risk line when the touched symbol's
 *                   callers cross MEDIUM. The one call here that may refresh the store.
 *  SubagentStart  → every subagent gets the rule. Prose asking the parent to pass it on reached 19
 *                   of 146 subagent spawns in the recorded sessions, and 0 of the 140 that did not
 *                   carry it ever ran the tool.
 *
 * Contract: exit 0 always, stdout only when there is something to say, stderr only under
 * REPOGRAPH_HOOK_DEBUG. Every call is --no-dense; every call but `changes` is --stale.
 */
import { existsSync, readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { spawn, spawnSync } from 'node:child_process';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { tmpdir } from 'node:os';
import { createHash } from 'node:crypto';

const MAX_INJECTIONS = 12;   // 12 × ~107 tokens: a third of the CLAUDE.md sections this replaces
const SEEDS = 3;
const LINE_CUT = 160;
const REMINDER_EVERY = 40;
const TIMEOUT_MS = 5000;
/** What the autostarted `serve` is told: leave after half an hour unasked, forget the model after five minutes. */
const SERVE_IDLE = 1800;
const SERVE_IDLE_MODEL = 300;   // measured on the 33.5k-row copy: 0.08 s resident, 0.77 s after a drop

const HERE = dirname(fileURLToPath(import.meta.url));

/**
 * The rule handed to every subagent. Read from `rule.txt` beside this file when the installer put
 * one there, and otherwise the copy below — the two are compared by a test, so a hook installed
 * without its text still says the same thing.
 */
const RULE_FALLBACK = `This repository has a repograph index. Ask it before grepping for a concept: \`repograph ask <words>\`
answers by meaning, \`repograph impact <Symbol>\` names who calls a symbol, \`repograph changes\` maps
your diff onto the callers it reaches. Read only the \`path:line\` those answers print.
Grep is still the right tool for a literal — an env var name, a string in a test, a config key.
`;

const BUILD_NOTICE =
  'repograph: this repository has no index yet. `repograph build` writes one in a couple of minutes ' +
  'and answers by meaning afterwards; until then this notice is all it can say.';

function rule() {
  const beside = join(HERE, 'rule.txt');
  try { return existsSync(beside) ? readFileSync(beside, 'utf8') : RULE_FALLBACK; } catch { return RULE_FALLBACK; }
}

function debug(...args) {
  if (process.env.REPOGRAPH_HOOK_DEBUG) console.error('[repograph-hook]', ...args);
}

/** PATH first, the package's own bin second, and nothing else: a hook that installed things would not be a hook. */
function binary(root) {
  const local = join(root, 'node_modules', '.bin', 'repograph');
  return process.env.REPOGRAPH_BIN || (existsSync(local) ? local : 'repograph');
}

function run(root, args) {
  const r = spawnSync(binary(root), ['--repo', root, '--no-dense', ...args], {
    encoding: 'utf8', timeout: TIMEOUT_MS, stdio: ['ignore', 'pipe', 'pipe'], windowsHide: true,
  });
  if (r.status !== 0) debug('exit', r.status, args.join(' '), (r.stderr || '').slice(0, 200));
  return r.status === 0 ? r.stdout : '';
}

function built(root) { return existsSync(join(root, '.repograph', 'manifest.json')); }

/**
 * Words worth asking, or null. Four letters is the floor because a shorter token is a flag, an
 * extension or a variable name far more often than it is a concept; six words is the ceiling
 * because the seventh never changed an answer and every one costs prompt.
 */
export function queryWords(pattern) {
  if (!pattern) return null;
  if (/\//.test(pattern) && !/\s/.test(pattern)) return null;            // a path, not a question
  const bare = pattern.replace(/[\\^$.*+?()[\]{}|]/g, ' ').trim();
  if (!bare) return null;
  // A lone identifier or a document id is sent whole: the exact stage answers it in 60 ms and
  // 20–40 tokens, which is the best case this hook has.
  if (/^[A-Za-z][A-Za-z0-9_]*$/.test(bare) && bare.length >= 4) return [bare];
  if (/^[A-Z][A-Z0-9]*(?:-[A-Z0-9]+)*-\d{1,4}$/.test(bare)) return [bare];
  const words = bare.split(/[^\p{L}\p{N}_-]+/u).filter((w) => /\p{L}{4,}/u.test(w)).slice(0, 6);
  return words.length ? words : null;
}

/**
 * What a tool call searches for. Bash is the surface that matters: a pass over 38 recorded sessions
 * found Grep 0, Glob 0 and Bash 2,727 calls, 933 of them running `rg` or `grep` — the harness tells
 * the model to search through Bash, so a hook that only watched Grep watched nothing.
 */
export function searchPattern(tool, input) {
  if (!input) return null;
  if (tool === 'Grep') return input.pattern || null;
  if (tool === 'Glob') {
    const segments = String(input.pattern || '').split(/[/*]+/)
      .map((s) => s.replace(/^[^\p{L}\p{N}]+|[^\p{L}\p{N}]+$/gu, ''))
      .filter((s) => /\p{L}{4,}/u.test(s));
    return segments.sort((a, b) => b.length - a.length)[0] || null;
  }
  if (tool !== 'Bash') return null;
  const command = String(input.command || '');
  if (!/\b(rg|grep)\b/.test(command)) return null;
  // Quoted first: a phrase is one argument, and splitting on whitespace would send the graph the
  // first word of `rg -n "cancellation policy"` and dedup every later phrase starting the same way.
  const tokens = command.match(/"[^"]*"|'[^']*'|\S+/g) || [];
  const at = tokens.findIndex((t) => /^(rg|grep)$/.test(t) || /\/(rg|grep)$/.test(t));
  if (at < 0) return null;
  const takesValue = /^-(e|f|g|m|A|B|C|t|T|-glob|-type|-max-count|-regexp)$/;
  for (let i = at + 1; i < tokens.length; i += 1) {
    const t = tokens[i];
    if (takesValue.test(t)) { i += 1; continue; }
    if (t.startsWith('-')) continue;
    return t.replace(/^['"]|['"]$/g, '');
  }
  return null;
}

function stateDir(sessionKey) { return join(tmpdir(), 'repograph-hook', sessionKey); }

/** True the first time this session asks about this key; an unwritable tmp answers once and never dedups. */
function once(sessionKey, kind, key) {
  const file = join(stateDir(sessionKey), `${kind}-${createHash('sha1').update(key).digest('hex').slice(0, 16)}`);
  if (existsSync(file)) return false;
  try { mkdirSync(stateDir(sessionKey), { recursive: true }); writeFileSync(file, ''); } catch { /* answer this one */ }
  return true;
}

function count(sessionKey, name) {
  try { return Number(readFileSync(join(stateDir(sessionKey), `count-${name}`), 'utf8')) || 0; } catch { return 0; }
}

function bump(sessionKey, name) {
  const n = count(sessionKey, name) + 1;
  try { mkdirSync(stateDir(sessionKey), { recursive: true }); writeFileSync(join(stateDir(sessionKey), `count-${name}`), String(n)); } catch { /* no state, no cadence */ }
  return n;
}

function isWatchedCall(tool, input) {
  if (tool === 'Bash') return /\b(rg|grep)\b/.test(String((input || {}).command || '')) || /\.(ts|tsx|md)\b/.test(String((input || {}).command || ''));
  return tool === 'Grep' || tool === 'Glob';
}

/**
 * On by default because the harness said so, under a rule committed before the number was read:
 * interception ships when its tokens per hit are lower than reminder-only's. Sonnet, twice over the
 * twelve tasks — 137,991 and 129,243 tokens per hit with it, 194,011 and 169,036 without, the two
 * ranges not overlapping — and 11/11 hits against 11/9. `REPOGRAPH_HOOK_INTERCEPT=0` turns it off.
 * `docs/bench/2026-09-09-agent-surface-results.md` has all four runs and what they are worth.
 */
function intercept(payload, root, sessionKey) {
  if (process.env.REPOGRAPH_HOOK_INTERCEPT === '0') return null;
  const input = payload.tool_input || {};
  // A search the agent runs *with* repograph is not one to answer beside.
  if (payload.tool_name === 'Bash' && /\brepograph\b/.test(String(input.command || ''))) return null;
  if (JSON.stringify(input).includes('.repograph/')) return null;
  const words = queryWords(searchPattern(payload.tool_name, input));
  if (!words) return null;
  if (!once(sessionKey, 'ask', words.join(' ').toLowerCase())) return null;
  if (count(sessionKey, 'injections') >= MAX_INJECTIONS) return null;
  const lines = run(root, ['ask', '--stale', '--seeds', String(SEEDS), ...words])
    .split('\n').filter((l) => l && !l.startsWith('  ')).map((l) => l.slice(0, LINE_CUT));
  if (!lines.length) return null;
  bump(sessionKey, 'injections');
  return [`repograph ask ${words.join(' ')} →`, ...lines, '(read only these path:line; `repograph ask` for more)'].join('\n');
}

function reminder(payload, sessionKey) {
  if (!isWatchedCall(payload.tool_name, payload.tool_input)) return null;
  const n = bump(sessionKey, 'watched');
  return n % REMINDER_EVERY === 0
    ? 'repograph: this repository has an index — `repograph ask <words>` answers by meaning, no tokens.'
    : null;
}

function riskLine(payload, root, sessionKey) {
  const file = String((payload.tool_input || {}).file_path || '');
  if (!/\.tsx?$/.test(file)) return null;
  let r;
  try { r = JSON.parse(run(root, ['changes', '--depth', '1', '--json'])); } catch { return null; }
  if (!r || !['MEDIUM', 'HIGH', 'CRITICAL'].includes(r.risk)) return null;
  if (!once(sessionKey, 'risk', `${file}:${r.risk}`)) return null;
  const direct = (r.affected || []).filter((d) => d.depth === 1).length;
  const touched = (r.touched || []).filter((t) => t.indexed !== false)
    .map((t) => String(t.id || '').split('::').pop()).filter(Boolean).slice(0, 3).join(', ');
  const files = (r.files || []).length;
  return `repograph changes: risk ${r.risk} — ${direct} direct callers of ${touched} in ${files} files; \`repograph changes --depth 1\` lists them.`;
}

function brief(root) {
  const text = run(root, ['prime']);
  return text ? text.trimEnd() : null;
}

/**
 * Starts a resident `serve` for this repository, detached, and never waits on it. No probe first:
 * a second `serve` refuses to bind while one answers and exits by itself, so the start *is* the
 * check. What it buys is the difference between a gate that fires and one that times out — a fused
 * ask is ~0.4 s against a cold process and ~80 ms through the socket, inside a 5 s hook budget on a
 * machine that is also running a build. `--idle-model` is what keeps that cheap between questions.
 */
function autostartServe(root) {
  if (process.env.REPOGRAPH_HOOK_SERVE === '0') return;
  try {
    const child = spawn(binary(root), ['--repo', root, 'serve', '--idle', String(SERVE_IDLE), '--idle-model', String(SERVE_IDLE_MODEL)],
      { detached: true, stdio: 'ignore', windowsHide: true });
    // A spawn that fails reports it as an event, not as a throw, and an unheard `error` event ends
    // this process — which would turn "no binary on PATH" into a hook that dies on stderr at every
    // session start. The listener is what keeps the contract: exit 0, and say nothing.
    child.on('error', (e) => debug('serve', e.message));
    child.unref();
  } catch (e) { debug('serve', e.message); }
}

const handlers = {
  SessionStart: (p, root) => {
    if (!built(root)) return null;
    autostartServe(root);
    return brief(root);
  },
  SubagentStart: (p, root) => (built(root) ? rule() : null),
  PreToolUse: (p, root, key) => {
    if (p.tool_name === 'Agent') {
      if (process.env.REPOGRAPH_HOOK_AGENT_INPUT !== '1') return null;
      const prompt = String((p.tool_input || {}).prompt || '');
      return { updatedInput: { ...(p.tool_input || {}), prompt: `${prompt}\n\n${rule()}` } };
    }
    if (!built(root)) {
      return isWatchedCall(p.tool_name, p.tool_input) && once(key, 'build', 'once') ? BUILD_NOTICE : null;
    }
    return intercept(p, root, key) ?? reminder(p, key);
  },
  PostToolUse: (p, root, key) => (built(root) ? riskLine(p, root, key) : null),
};

async function readStdin() {
  const chunks = [];
  for await (const chunk of process.stdin) chunks.push(chunk);
  return Buffer.concat(chunks).toString('utf8');
}

async function main() {
  let payload;
  try { payload = JSON.parse(await readStdin()); } catch { return; }
  const event = payload.hook_event_name;
  const handler = handlers[event];
  if (!handler) return;
  const root = payload.cwd || process.env.CLAUDE_PROJECT_DIR || process.cwd();
  // The subagent's own id is in the key: a subagent shares its parent's session_id, so a counter
  // keyed by session alone finds the count already past zero on a subagent's first call and says
  // nothing — which is a second reason 140 unprimed subagents went on grepping.
  const sessionKey = `${payload.session_id || 'nosession'}-${payload.agent_id || 'main'}`;
  let out;
  try { out = handler(payload, root, sessionKey); } catch (e) { debug('handler', e); return; }
  if (!out) return;
  const body = typeof out === 'string'
    ? { hookEventName: event, additionalContext: out }
    : { hookEventName: event, ...out };
  // Written without process.exit: on Windows the pipe may still be draining when exit would cut it.
  process.stdout.write(JSON.stringify({ hookSpecificOutput: body }));
}

// Importable for the tests, which exercise the pure parts directly; only the real run reads stdin.
if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  main();
}

export { rule, RULE_FALLBACK, intercept, riskLine, handlers, MAX_INJECTIONS };
