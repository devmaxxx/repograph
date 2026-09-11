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
import { existsSync, readFileSync, writeFileSync, mkdirSync, realpathSync } from 'node:fs';
import { spawn, spawnSync } from 'node:child_process';
import { join, dirname, relative } from 'node:path';
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

/**
 * How this repository runs the binary, in every hint the hook prints. `install-agent --command`
 * rewrites this one line with the command JSON-encoded, so nothing in it reaches the script as code.
 * A workspace that installs repograph as a dependency has no bare `repograph` on PATH, and a hint
 * naming a program the shell cannot find is one the agent runs, fails, and pays for twice.
 */
const COMMAND = "repograph";

const HERE = dirname(fileURLToPath(import.meta.url));

/**
 * The rule handed to every subagent. Read from `rule.txt` beside this file when the installer put
 * one there, and otherwise the copy below — the two are compared by a test, so a hook installed
 * without its text still says the same thing.
 */
const RULE_FALLBACK = `This repository has a repograph index. Ask it before grepping for a concept: \`${COMMAND} ask <words>\`
answers by meaning, \`${COMMAND} impact <Symbol>\` names who calls a symbol, \`${COMMAND} changes\` maps
your diff onto the callers it reaches. Read only the \`path:line\` those answers print.
Grep is still the right tool for a literal — an env var name, a string in a test, a config key.
`;

const BUILD_NOTICE =
  `repograph: this repository has no index yet. \`${COMMAND} build\` writes one in a couple of minutes ` +
  'and answers by meaning afterwards; until then this notice is all it can say.';

function rule() {
  const beside = join(HERE, 'rule.txt');
  try { return existsSync(beside) ? readFileSync(beside, 'utf8') : RULE_FALLBACK; } catch { return RULE_FALLBACK; }
}

function debug(...args) {
  if (process.env.REPOGRAPH_HOOK_DEBUG) console.error('[repograph-hook]', ...args);
}

/**
 * PATH first, the package's own bin second, and nothing else: a hook that installed things would
 * not be a hook. npm, pnpm and yarn all write the same trio under `node_modules/.bin` — an
 * extensionless shell script, a `.cmd` and a `.ps1` — and on Windows the `.cmd` is the one that
 * starts. The extensionless shim is worse than nothing there: returning it spawns a file
 * `CreateProcess` cannot read, where falling through finds whatever `repograph.exe` an installer
 * put on PATH.
 */
function binary(root) {
  if (process.env.REPOGRAPH_BIN) return process.env.REPOGRAPH_BIN;
  const local = join(root, 'node_modules', '.bin', 'repograph');
  const shim = process.platform === 'win32' ? `${local}.cmd` : local;
  return existsSync(shim) ? shim : 'repograph';
}

/**
 * How to hand one command to `spawn`. A `.cmd` or `.bat` is read by the command interpreter rather
 * than by `CreateProcess`, and Node has refused to spawn one without a shell since CVE-2024-27980.
 * The shell joins the file and its arguments into one command line without quoting them, so they
 * are quoted here — and nothing that reaches this can close a quote: a Windows path may not contain
 * one, and the words come from `queryWords`, which keeps letters, digits, `_` and `-` and nothing
 * else. Everything that is not a batch file is spawned as it always was.
 *
 * Two things the quoting does not buy, both only on that batch path. `cmd` expands `%NAME%` inside
 * quotes and the command line has no escape for it — only a batch file's own `%%` — so a repository
 * under a directory that pairs two `%` around an environment variable's name is asked about by the
 * expanded path. And `spawnSync`'s timeout kills the interpreter rather than its child, so a wedged
 * `repograph` outlives the five seconds this hook waits. Both need a local install on Windows to
 * reach at all; neither is worth the shell it would take to avoid, and neither is silent about it.
 */
function spawnable(bin, args) {
  // Windows and a batch file, both: the extension alone would open the shell on a POSIX machine
  // whose `REPOGRAPH_BIN` happens to end in `.cmd`, where a repository path may contain a quote
  // and the sentence above stops being true.
  if (process.platform !== 'win32' || !/\.(cmd|bat)$/i.test(bin)) return { file: bin, args, opts: {} };
  return { file: `"${bin}"`, args: args.map((a) => `"${a}"`), opts: { shell: true } };
}

function run(root, args) {
  const { file, args: argv, opts } = spawnable(binary(root), ['--repo', root, '--no-dense', ...args]);
  const r = spawnSync(file, argv, {
    encoding: 'utf8', timeout: TIMEOUT_MS, stdio: ['ignore', 'pipe', 'pipe'], windowsHide: true, ...opts,
  });
  if (r.status !== 0) debug('exit', r.status, args.join(' '), (r.stderr || '').slice(0, 200));
  return r.status === 0 ? r.stdout : '';
}

function built(root) { return existsSync(join(root, '.repograph', 'manifest.json')); }

/**
 * The nearest directory at or above `start` that carries an index. The harness's `cwd` follows a
 * `cd`, and a subdirectory has no `.repograph/` of its own; the walk stops at a `.git`, file or
 * directory, so a worktree nested inside a checkout never borrows the checkout's index.
 */
function indexRoot(start) {
  for (let d = start; ; d = dirname(d)) {
    if (built(d)) return d;
    if (existsSync(join(d, '.git')) || dirname(d) === d) return null;
  }
}

/**
 * Words worth asking, or null. Four letters is the floor because a shorter token is a flag, an
 * extension or a variable name far more often than it is a concept; six words is the ceiling
 * because the seventh never changed an answer and every one costs prompt.
 */
export function queryWords(pattern) {
  if (!pattern) return null;
  if (/\//.test(pattern) && !/\s/.test(pattern)) return null;            // a path, not a question
  // A class escape (`\b`, `\w`, `\s`) goes whole: dropping only the backslash glues its letter to
  // the word beside it, and `\bwithTenant\b` would ask about `bwithTenant`.
  const bare = pattern.replace(/\\[A-Za-z]/g, ' ').replace(/[\\^$.*+?()[\]{}|]/g, ' ').trim();
  if (!bare) return null;
  // A lone identifier or a document id is sent whole: the exact stage answers it in 60 ms and
  // 20–40 tokens, which is the best case this hook has. A milestone or task id (`BE-M17`,
  // `BE-M01-T03`) ends in a capital and digits, and has no four-letter word to fall back on.
  if (/^[A-Za-z][A-Za-z0-9_]*$/.test(bare) && bare.length >= 4) return [bare];
  if (/^[A-Z][A-Z0-9]*(?:-[A-Z0-9]+)*-[A-Z]?\d{1,4}$/.test(bare)) return [bare];
  // A leading dash is dropped: `--rerank` sent as a word is parsed by `ask` as its paid flag.
  const words = bare.split(/[^\p{L}\p{N}_-]+/u).map((w) => w.replace(/^-+/, ''))
    .filter((w) => /\p{L}{4,}/u.test(w)).slice(0, 6);
  return words.length ? words : null;
}

/**
 * A shell line cut into its stages, each marked by whether a pipe feeds it. Quoted strings and
 * backslash escapes are kept whole, so the `|` in `grep -E "a|b"` or `grep a\|b` is a pattern and
 * not a pipe.
 */
function stages(command) {
  const out = [{ piped: false, text: '' }];
  for (const [part] of command.matchAll(/(?:"[^"]*"|'[^']*'|\\.|[^"'\\|;&\n])+|\|&|\|\||&&|[|;&\n]/g)) {
    if (/^(?:\|&|\|\||&&|[|;&\n])$/.test(part)) out.push({ piped: part === '|' || part === '|&', text: '' });
    else out[out.length - 1].text += part;
  }
  return out;
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
  const takesValue = /^-(f|g|m|A|B|C|t|T|-glob|-type|-max-count)$/;
  for (const stage of stages(command)) {
    // A stage a pipe feeds searches what the stage before it printed, not the repository:
    // `gh auth status 2>&1 | grep -E "Logged in|Active account"` came back with three graph lines
    // about accounts. `| xargs grep` is skipped too — a missed injection costs nothing, and a wrong
    // one is paid on every later turn.
    if (stage.piped) continue;
    // Quoted first: a phrase is one argument, and splitting on whitespace would send the graph the
    // first word of `rg -n "cancellation policy"` and dedup every later phrase starting the same way.
    const tokens = stage.text.match(/"[^"]*"|'[^']*'|\S+/g) || [];
    const at = tokens.findIndex((t) => /^(rg|grep)$/.test(t) || /\/(rg|grep)$/.test(t));
    if (at < 0) continue;
    for (let i = at + 1; i < tokens.length; i += 1) {
      const t = tokens[i];
      // `-e` and `--regexp` carry the pattern itself; skipping their value would ask about the path after it.
      if (/^-(e|-regexp)$/.test(t)) return (tokens[i + 1] || '').replace(/^['"]|['"]$/g, '') || null;
      if (takesValue.test(t)) { i += 1; continue; }
      if (t.startsWith('-')) continue;
      return t.replace(/^['"]|['"]$/g, '');
    }
  }
  return null;
}

/**
 * The per-session state directory. The key is built from ids the harness supplies, so it is
 * reduced to word characters before it becomes a path segment: an id carrying a slash or a `..`
 * would otherwise put this hook's empty marker files anywhere the process can write.
 */
function stateDir(sessionKey) { return join(tmpdir(), 'repograph-hook', sessionKey.replace(/\W+/g, '-')); }

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
  return [`repograph ask ${words.join(' ')} →`, ...lines, `(read only these path:line; \`${COMMAND} ask\` for more)`].join('\n');
}

function reminder(payload, sessionKey) {
  if (!isWatchedCall(payload.tool_name, payload.tool_input)) return null;
  const n = bump(sessionKey, 'watched');
  return n % REMINDER_EVERY === 0
    ? `repograph: this repository has an index — \`${COMMAND} ask <words>\` answers by meaning, no tokens.`
    : null;
}

function riskLine(payload, root, sessionKey) {
  const file = String((payload.tool_input || {}).file_path || '');
  if (!/\.tsx?$/.test(file)) return null;
  let r;
  try { r = JSON.parse(run(root, ['changes', '--depth', '1', '--json'])); } catch { return null; }
  if (!r || !['MEDIUM', 'HIGH', 'CRITICAL'].includes(r.risk)) return null;
  // `changes` covers the whole working diff: keep only what the edited file contributes, or an edit to
  // a leaf file is told about the callers of symbols some other file in the diff touched.
  const rel = relative(root, file).split('\\').join('/');
  const mine = (r.touched || []).filter((t) => t.indexed !== false && String(t.at || '').startsWith(`${rel}:`));
  const direct = (r.affected || []).filter((d) => d.depth === 1 && String(d.via || '').startsWith(`sym:${rel}::`));
  if (!mine.length || !direct.length) return null;
  const files = new Set(direct.map((d) => String(d.at || '').split(':')[0])).size;
  // `r.risk` scores the whole diff; the level is this file's own direct callers, on the thresholds
  // `impact::risk` prints beside its counts.
  const n = direct.length;
  const risk = n >= 30 || files >= 25 ? 'CRITICAL' : n >= 15 || files >= 10 ? 'HIGH' : n >= 5 || files >= 3 ? 'MEDIUM' : null;
  if (!risk) return null;
  if (!once(sessionKey, 'risk', `${file}:${risk}`)) return null;
  const touched = [...new Set(mine.map((t) => String(t.id || '').split('::').pop()).filter(Boolean))].slice(0, 3).join(', ');
  return `repograph changes: risk ${risk} — ${n} direct callers of ${touched} in ${files} files; \`${COMMAND} changes --depth 1\` lists them.`;
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
    const { file, args, opts } = spawnable(binary(root),
      ['--repo', root, 'serve', '--idle', String(SERVE_IDLE), '--idle-model', String(SERVE_IDLE_MODEL)]);
    const child = spawn(file, args, { detached: true, stdio: 'ignore', windowsHide: true, ...opts });
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
  // The harness's cwd follows a `cd` into a subdirectory, which has no `.repograph/` of its own.
  const root = (payload.cwd && indexRoot(payload.cwd)) || payload.cwd || process.env.CLAUDE_PROJECT_DIR || process.cwd();
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
// `import.meta.url` is the resolved path and argv[1] keeps a symlink in the project path, so both are
// resolved: compared as given, a project reached through a link ran the hook and it said nothing.
function invokedDirectly() {
  try { return Boolean(process.argv[1]) && fileURLToPath(import.meta.url) === realpathSync(process.argv[1]); } catch { return false; }
}
if (invokedDirectly()) {
  main();
}

export { rule, RULE_FALLBACK, COMMAND, binary, intercept, riskLine, handlers, MAX_INJECTIONS };
