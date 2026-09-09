//! Questions a reader might ask to reach a node, written once by a language model and cached
//! by passage hash. They give every answer the reader's vocabulary as well as the author's:
//! `query::ask` fuses them as a BM25 list of their own — on the plain path only when that list
//! matched the question at least 0.85 as strongly as the passage list did, on the reranked path
//! always — and `--rerank` also pools them as dense rows. On the development corpus the
//! lexical-only arm reads paraphrase 7/30 raw against 14/30 enriched and keyword 39/40 in both;
//! before the gate an equal turn in the fusion cost that arm two keyword cases.
use crate::model::{Graph, Node, NodeKind};
use crate::store::{Source, Store};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::Write;
use std::sync::{Arc, Mutex};

pub(crate) const FILE: &str = "questions.json";
const KINDS: [NodeKind; 5] = [NodeKind::Requirement, NodeKind::Invariant, NodeKind::Adr, NodeKind::Milestone, NodeKind::Entity];
const PASSAGE_CHARS: usize = 1500;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry { pub hash: String, pub questions: Vec<String> }

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Questions { pub entries: BTreeMap<String, Entry> }

#[derive(Debug)]
pub struct Report { pub generated: usize, pub dropped: usize, pub batches: usize, pub failed: usize, pub left: usize }

/// What one `enrich` run covers: at most `limit` stale nodes of each kind, and code only on request.
#[derive(Clone, Copy, Debug, Default)]
pub struct Scope { pub limit: Option<usize>, pub code: bool }

/// One prompt's worth of nodes with their passage hashes; `retry` after the model skipped them
/// once, `code` for the code prompt.
struct Batch<'a> { nodes: Vec<(&'a Node, String)>, retry: bool, code: bool }

fn passage(n: &Node) -> String {
    let body: String = n.body.chars().take(PASSAGE_CHARS).collect();
    format!("{}\n{}", n.label, body)
}

fn hash(text: &str) -> String { blake3::hash(text.as_bytes()).to_hex().to_string() }

/// An entity that is only a name (a backticked span in some title) gives the model nothing to
/// ask about, and it says so by skipping the entry — 63 of them on the bench corpus, retried on
/// every run until they were left out.
pub fn eligible(n: &Node) -> bool { KINDS.contains(&n.kind) && !(n.kind == NodeKind::Entity && n.body.trim().is_empty()) }

/// Code a developer's question can reach: a symbol its author wrote something about or that
/// has a body of its own, and a file with a head comment. A one-line declaration or a decorator
/// name gives the model nothing to ask about. Opt-in (`enrich --code`): the doc floors are
/// measured without these, and `coverage` counts documents alone, so a store with code questions
/// is graded as the enriched store it also is.
pub fn eligible_code(n: &Node) -> bool {
    match n.kind {
        NodeKind::File => !n.body.trim().is_empty(),
        NodeKind::Symbol => !n.id.starts_with("deco:") && !n.body.trim().is_empty() && (n.body.contains('\n') || n.end > n.line + 2),
        _ => false,
    }
}

/// Code nodes that carry questions, over code nodes that could — reported beside `coverage`,
/// never part of it.
pub fn code_coverage(graph: &Graph, questions: &Questions) -> (usize, usize) {
    let nodes: Vec<&Node> = graph.nodes.values().filter(|n| eligible_code(n)).collect();
    (nodes.iter().filter(|n| !questions.get(&n.id).is_empty()).count(), nodes.len())
}

/// Eligible nodes that carry questions, over eligible nodes. `run` saves after every batch, so
/// a `--limit` run, an interrupt or a model that skipped a batch twice all leave a store with
/// some questions in it; "any entry at all" would call such a store enriched and hold it to
/// numbers only a finished run reaches. Passage freshness is left out on purpose: a question
/// written for an older wording still finds its node, so one edited requirement should not
/// reclassify the whole store.
pub fn coverage(graph: &Graph, questions: &Questions) -> (usize, usize) {
    let nodes: Vec<&Node> = graph.nodes.values().filter(|n| eligible(n)).collect();
    (nodes.iter().filter(|n| !questions.get(&n.id).is_empty()).count(), nodes.len())
}

/// How many eligible nodes have no questions, when the store has questions for some others.
/// `None` when there is nothing to say: a store nobody enriched is a state, and a store that was
/// enriched and then grew is a next step nothing else prints — a rebuild under new families adds
/// requirement-like nodes `enrich` has never seen, and the reader learns it from a bench summary
/// or not at all.
pub fn unenriched_note(graph: &Graph, questions: &Questions) -> Option<usize> {
    let (covered, eligible) = coverage(graph, questions);
    (covered > 0 && eligible > covered).then(|| eligible - covered)
}

/// Whether a `coverage` reading has earned the enriched floors. A high-water mark rather than
/// equality because the two mistakes are not symmetric: a store at 99% still measures the
/// enriched numbers, so grading it enriched risks about no false red, while grading it raw drops
/// it five paraphrase points and one keyword point onto floors it clears without trying, and
/// `bench` speaks through its exit code. Over ~2 000 nodes equality is a cliff a single node
/// walks off — one requirement added after the run, one node the model skipped past its retry,
/// one entry `clean` drops on load — and a real regression behind that cliff exits 0.
pub fn enriched(covered: usize, eligible: usize) -> bool { eligible > 0 && covered * 100 >= eligible * 99 }

impl Questions {
    pub fn load(store: &Store) -> Result<Questions> { Self::load_traced(store).map(|(q, _)| q) }

    /// `load`, saying whether the JSON had to be parsed — the moment a reader that may write
    /// leaves the mirror behind for the next one.
    pub fn load_traced(store: &Store) -> Result<(Questions, Source)> {
        let (q, source) = store.load_mirrored::<Questions>(FILE)?;
        let mut q = q.unwrap_or_default();
        // A mirror is written from entries the guard already passed; only the JSON, which may
        // predate the guard, still needs it.
        if source == Source::Json { clean(&mut q.entries); }
        Ok((q, source))
    }

    pub fn write_mirror(&self, store: &Store) -> Result<()> { store.write_mirror(FILE, self) }

    pub fn save(&self, store: &Store) -> Result<()> {
        store.write_atomic(FILE, &serde_json::to_vec_pretty(self)?)?;
        // The mirror holds what a load of this JSON would return, guard included, so the two
        // paths can never disagree about an entry.
        let mut mirrored = self.clone();
        clean(&mut mirrored.entries);
        mirrored.write_mirror(store)
    }

    pub fn get(&self, id: &str) -> &[String] {
        self.entries.get(id).map(|e| e.questions.as_slice()).unwrap_or(&[])
    }

    /// Nodes whose cached questions are missing or were written for a different passage.
    fn stale<'a>(&self, graph: &'a Graph, wanted: fn(&Node) -> bool) -> Vec<(&'a Node, String)> {
        graph.nodes.values().filter(|n| wanted(n)).filter_map(|n| {
            let h = hash(&passage(n));
            match self.entries.get(&n.id) {
                Some(e) if e.hash == h => None,
                _ => Some((n, h)),
            }
        }).collect()
    }

    /// Drops entries for nodes the graph no longer has; returns how many went.
    fn prune(&mut self, graph: &Graph) -> usize {
        let before = self.entries.len();
        // Code questions stay whether or not this run asked for them.
        self.entries.retain(|id, _| graph.nodes.get(id).is_some_and(|n| eligible(n) || eligible_code(n)));
        before - self.entries.len()
    }
}

/// Worded and measured on the development corpus (a Russian PRD); the example substitutions
/// and the four reader roles are its, and a rewording is a re-measure.
pub fn prompt(nodes: &[&Node]) -> String {
    let mut p = String::from(
        "Below are entries from a product's documentation: an id, a title line and the start of the text.\n\
         For each entry write 12 short questions (3-10 words) that a person who has never read this \
         documentation could ask to find exactly this entry. Use everyday words, not the entry's own \
         terms: replace every specialised or product-specific word with what an ordinary person would \
         say (SLA -> сроки ответа, аудит-лог -> история действий, no-show -> клиент не пришёл), and \
         vary the wording between questions so that no two share their key words. Write three \
         questions each from the point of view of a customer, a front-desk employee, the business \
         owner and a developer, every one about a different detail of the entry. Then add one line \
         `id<TAB>synonyms: ...` with 5-10 everyday synonyms or paraphrases of the entry's key terms, \
         comma-separated. Every entry gets its lines, including one that has only a title. Write \
         every question and synonym in the language the entry itself is written in (a Russian entry \
         gets Russian questions), never translated. Output exactly one question per line, in the form \
         `id<TAB>text`, with no numbering and no commentary.\n\n");
    for n in nodes {
        p.push_str(&format!("### {}\n{}\n\n", n.id, passage(n)));
    }
    p
}

/// Worded for code and measured on the development corpus's TypeScript. Its two languages are
/// the corpus's — a Russian PRD over English identifiers — and a rewording is a re-measure.
pub fn prompt_code(nodes: &[&Node]) -> String {
    let mut p = String::from(
        "Below are entries from a codebase: a key (c1, c2, ...), the id, the file path, then what the \
         author wrote about the code and the line that declares it.\n\
         For each entry write 8 short questions (4-12 words) that a developer implementing a feature \
         could ask to find exactly this code without knowing its name: where something is handled, \
         which file or function does a thing, what enforces a rule, where to add or change a \
         behaviour. Describe what the code does in everyday product words. Never repeat the \
         identifier itself; say what it does instead (TenantContextInterceptor -> где проверяется, \
         что запрос принадлежит нужному бизнесу). Write four questions in Russian and four in \
         English, every one about a different aspect of the entry. Then add one line \
         `id<TAB>synonyms: ...` with 5-10 everyday words or phrases for what the code does, in both \
         languages, comma-separated. Every entry gets its lines. Output exactly one question per \
         line, in the form `key<TAB>text` with the entry's key (`c1`, `c2`, ...) copied exactly, and \
         no other numbering and no commentary.\n\n");
    for (i, n) in nodes.iter().enumerate() {
        p.push_str(&format!("### c{}\n{}\n{}\n{}\n\n", i + 1, n.id, n.file, passage(n)));
    }
    p
}

/// What a code answer may open an entry with: the entry's `c<n>` key or its id. Asked for the
/// id, the model copied the label of a long one — `AvailabilityService` for
/// `sym:apps/api/src/…/availability.service.ts::AvailabilityService` — and half the batches of
/// the development corpus came back without a usable line; a two-character key is copied whole.
pub fn code_keys<'a>(nodes: &[&'a Node]) -> Vec<(String, &'a Node)> {
    nodes.iter().enumerate().flat_map(|(i, n)| [(format!("c{}", i + 1), *n), (n.id.clone(), *n)]).collect()
}

/// Lines of `id<TAB>question` for ids in the batch; anything else is ignored. A line may carry
/// several questions tab-joined, an id before each — the bench corpus had 40 such lines, each
/// stored whole with its own id inside, which the exact stage then answered for free.
pub fn parse(output: &str, batch: &[&Node]) -> BTreeMap<String, Vec<String>> {
    let keys: Vec<(String, &Node)> = batch.iter().map(|n| (n.id.clone(), *n)).collect();
    parse_keyed(output, &keys)
}

/// `parse` over any set of keys, several of which may open the same entry.
pub fn parse_keyed(output: &str, keys: &[(String, &Node)]) -> BTreeMap<String, Vec<String>> {
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for line in output.lines() {
        let mut current: Option<&str> = None;
        for part in line.split('\t').map(str::trim).filter(|p| !p.is_empty()) {
            if let Some((_, n)) = keys.iter().find(|(k, _)| k == part) {
                current = Some(&n.id);
            } else if let Some(id) = current.filter(|_| readable(part)) {
                out.entry(id.to_string()).or_default().extend(split_joined(part));
            }
        }
    }
    out
}

/// A question is searchable only in a script the readers write: the generator drifted into
/// Urdu on 12 ADR nodes of the bench corpus, 144 lines that ranked for nobody and sat in both
/// question indexes. Letters outside Cyrillic and Latin may not be the majority.
fn readable(q: &str) -> bool {
    let (mut letters, mut known) = (0usize, 0usize);
    for c in q.chars().filter(|c| c.is_alphabetic()) {
        letters += 1;
        if c.is_ascii_alphabetic() || matches!(c, '\u{00C0}'..='\u{024F}' | '\u{0400}'..='\u{04FF}') { known += 1; }
    }
    known * 2 >= letters
}

/// Stored questions written before `parse` learned the two rules above: tab-joined lines are
/// split and the entry's own id dropped, unreadable lines go, and an entry left without
/// questions is forgotten so the next `enrich` asks for it again.
fn clean(entries: &mut BTreeMap<String, Entry>) {
    entries.retain(|id, e| {
        e.questions = e.questions.iter()
            .flat_map(|q| q.split('\t').map(str::trim).filter(|p| !p.is_empty() && p != id && readable(p)).map(String::from).collect::<Vec<_>>())
            .collect();
        !e.questions.is_empty()
    });
}

/// The model sometimes packs a whole entry's questions into one comma-separated line without
/// question marks; three or more comma parts of three words each are that case, a synonym list
/// or a single question with a comma in it is not.
fn split_joined(line: &str) -> Vec<String> {
    let parts: Vec<&str> = line.split(", ").map(str::trim).collect();
    if !line.contains('?') && parts.len() >= 3 && parts.iter().all(|p| p.split_whitespace().count() >= 3) {
        parts.into_iter().map(String::from).collect()
    } else {
        vec![line.to_string()]
    }
}

/// The shell the configured command runs under: `sh`, on every platform. The commands in
/// `repograph.toml` are written in its syntax — a `VAR=value` prefix, `""` for an empty argument —
/// and a `cmd /C` or PowerShell rendering on Windows would make one key mean two things.
#[cfg(unix)]
fn shell() -> Result<std::process::Command> { Ok(std::process::Command::new("sh")) }

/// The same `sh`, which on Windows is Git for Windows'. Its default install puts `git` on PATH and
/// not `sh` — the Unix tools are an opt-in — so a `sh` PATH does not resolve is looked for beside
/// `git`, at the bash Claude Code names for its own Bash tool, and where the installer puts it;
/// found that way, Git's `usr\bin` goes on the child's PATH too, since that is where `awk` and the
/// rest of what a command may call live.
#[cfg(windows)]
fn shell() -> Result<std::process::Command> {
    use std::path::PathBuf;
    let path_dirs: Vec<PathBuf> = std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect()).unwrap_or_default();
    if path_dirs.iter().any(|d| d.join("sh.exe").is_file()) { return Ok(std::process::Command::new("sh")); }
    let bash = std::env::var_os("CLAUDE_CODE_GIT_BASH_PATH").map(PathBuf::from);
    let program_dirs: Vec<PathBuf> = [("ProgramFiles", ""), ("ProgramW6432", ""), ("LOCALAPPDATA", "Programs")].iter()
        .filter_map(|(k, sub)| std::env::var_os(k).map(|v| PathBuf::from(v).join(sub))).collect();
    let sh = git_sh(&path_dirs, bash.as_deref(), &program_dirs)
        .context("`sh` is not on PATH and no Git for Windows was found beside `git` or under Program Files: enrich and rerank run their command under sh, which Git for Windows provides")?;
    let mut cmd = std::process::Command::new(&sh);
    let root = sh.parent().and_then(std::path::Path::parent).map(std::path::Path::to_path_buf).unwrap_or_default();
    cmd.env("PATH", std::env::join_paths(std::iter::once(root.join("usr").join("bin")).chain(path_dirs))?);
    Ok(cmd)
}

/// `<Git>\bin\sh.exe` for the first `<Git>` that has one: the parent or grandparent of a directory
/// on `path_dirs` holding `git.exe` (`cmd\` and `mingw64\bin\` are both one install), the
/// grandparent of `bash_hint`, then `<dir>\Git` for each of `program_dirs`.
#[cfg(windows)]
fn git_sh(path_dirs: &[std::path::PathBuf], bash_hint: Option<&std::path::Path>, program_dirs: &[std::path::PathBuf]) -> Option<std::path::PathBuf> {
    let sh_in = |root: &std::path::Path| { let p = root.join("bin").join("sh.exe"); p.is_file().then_some(p) };
    let beside_git = path_dirs.iter().filter(|d| d.join("git.exe").is_file())
        .flat_map(|d| d.ancestors().skip(1).take(2).map(std::path::Path::to_path_buf).collect::<Vec<_>>());
    let named = bash_hint.and_then(|b| b.parent()?.parent()).map(std::path::Path::to_path_buf);
    let installed = program_dirs.iter().map(|d| d.join("Git"));
    beside_git.chain(named).chain(installed).find_map(|root| sh_in(&root))
}

/// `sh -c 'a | b'` returns b's status, so a generator that dies into a `tee` looks like success —
/// which is how one run reported 167 batches, 0 failed and nothing written. Shells that have
/// `pipefail` are asked for it; those that do not fall back to the empty-answer check in `run`,
/// which is the load-bearing half anyway: a pipeline's exit status is the operator's to get right,
/// an empty answer is nobody's to mistake for one.
fn pipefail_prefix() -> &'static str {
    static P: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    let ok = *P.get_or_init(|| {
        shell()
            .and_then(|mut c| Ok(c.arg("-c").arg("set -o pipefail")
                .stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status()?))
            .is_ok_and(|s| s.success())
    });
    if ok { "set -o pipefail; " } else { "" }
}

pub fn run_command(command: &str, input: &str) -> Result<String> {
    let mut child = shell()?.arg("-c").arg(format!("{}{command}", pipefail_prefix()))
        .stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped())
        .spawn().with_context(|| format!("spawn `{command}`"))?;
    // A command that answers without reading its whole prompt closes the pipe early — `claude -p`
    // never does, a wrapper or a stub may — and the write then fails with EPIPE while the answer is
    // already on stdout. The exit status and the output judge the run, not the write.
    let mut stdin = child.stdin.take().context("stdin")?;
    match stdin.write_all(input.as_bytes()) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => {}
        Err(e) => return Err(e).with_context(|| format!("write the prompt to `{command}`")),
    }
    drop(stdin);
    let out = child.wait_with_output()?;
    if !out.status.success() {
        anyhow::bail!("`{command}` exited {}: {}", out.status, String::from_utf8_lossy(&out.stderr).trim());
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Generates questions for every stale node through `command` (prompt on stdin, lines on
/// stdout), `parallel` batches at a time, saving after each batch so an interrupted run keeps
/// what it paid for.
pub fn run(store: &Store, graph: &Graph, questions: Questions, command: &str, batch: usize, parallel: usize, scope: Scope) -> Result<Report> {
    let Scope { limit, code } = scope;
    let mut questions = questions;
    let dropped = questions.prune(graph);
    let mut stale = questions.stale(graph, eligible);
    let mut stale_code = if code { questions.stale(graph, eligible_code) } else { Vec::new() };
    if let Some(l) = limit { stale.truncate(l); stale_code.truncate(l); }
    // A batch the model answers in prose instead of `id<TAB>text` leaves its nodes without
    // questions and its exit status green; measured once on 172 batches, 7 came back that way
    // and 195 nodes silently stayed unsearchable. The nodes an answer skipped go round once more.
    // Documents and code never share a batch: each kind has its own prompt.
    let batches: Vec<Batch> = stale.chunks(batch.max(1)).map(|c| Batch { nodes: c.to_vec(), retry: false, code: false })
        .chain(stale_code.chunks(batch.max(1)).map(|c| Batch { nodes: c.to_vec(), retry: false, code: true })).collect();
    let total = batches.len();
    let queue = Arc::new(Mutex::new(batches));
    let shared = Arc::new(Mutex::new((questions, 0usize, 0usize)));
    std::thread::scope(|s| {
        for _ in 0..parallel.max(1) {
            let (queue, shared) = (Arc::clone(&queue), Arc::clone(&shared));
            s.spawn(move || loop {
                let Some(Batch { nodes: b, retry, code: is_code }) = queue.lock().unwrap().pop() else { break };
                let nodes: Vec<&Node> = b.iter().map(|(n, _)| *n).collect();
                let p = if is_code { prompt_code(&nodes) } else { prompt(&nodes) };
                match run_command(command, &p) {
                    Ok(out) => {
                        let parsed = if is_code { parse_keyed(&out, &code_keys(&nodes)) } else { parse(&out, &nodes) };
                        let mut g = shared.lock().unwrap();
                        for (n, h) in &b {
                            if let Some(qs) = parsed.get(&n.id) {
                                g.0.entries.insert(n.id.clone(), Entry { hash: h.clone(), questions: qs.clone() });
                                g.1 += 1;
                            }
                        }
                        if let Err(e) = g.0.save(store) { eprintln!("enrich: save: {e:#}"); }
                        drop(g);
                        let skipped: Vec<(&Node, String)> = b.iter().filter(|(n, _)| !parsed.contains_key(&n.id)).cloned().collect();
                        // Asked twice and answered for nobody: the generator is not declining these
                        // nodes, it is not answering. Counting that as coverage is what let a whole
                        // run report `0 failed` and exit green having written nothing. Decided
                        // before the queue lock and acted on after it, so no path holds both locks
                        // and the two can never be taken in opposite orders.
                        let answered_for_nobody = skipped.len() == b.len() && (retry || skipped.is_empty());
                        let left = {
                            let mut queue = queue.lock().unwrap();
                            if !skipped.is_empty() && !retry {
                                eprintln!("enrich: {} of {} nodes skipped by the model, retrying them", skipped.len(), b.len());
                                queue.push(Batch { nodes: skipped, retry: true, code: is_code });
                            }
                            queue.len()
                        };
                        if answered_for_nobody {
                            eprintln!("enrich: a batch of {} answered for nobody twice", b.len());
                            shared.lock().unwrap().2 += 1;
                        }
                        eprintln!("enrich: batch done, {left} of {total} left");
                    }
                    Err(e) => { eprintln!("enrich: {e:#}"); shared.lock().unwrap().2 += 1; }
                }
            });
        }
    });
    let (questions, generated, failed) = match Arc::try_unwrap(shared) {
        Ok(m) => m.into_inner().unwrap(),
        Err(_) => unreachable!("every worker has joined"),
    };
    questions.save(store)?;
    let left = questions.stale(graph, eligible).len() + if code { questions.stale(graph, eligible_code).len() } else { 0 };
    Ok(Report { generated, dropped, batches: total, failed, left })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Extraction;

    fn graph() -> Graph {
        let mut g = Graph::default();
        let mut e = Extraction::default();
        e.node(NodeKind::Requirement, "FR-PAY-22", "отмена", "штраф по политике", "a.md", 1);
        e.node(NodeKind::Requirement, "FR-PAY-26", "штраф", "списание", "a.md", 9);
        e.node(NodeKind::Task, "BE-M01-T1", "task", "", "a.md", 12);
        e.node(NodeKind::Entity, "entity:Money", "Money", "", "a.md", 1);
        e.node(NodeKind::File, "file:a.md", "a.md", "", "a.md", 1);
        g.apply(e);
        g
    }

    #[test]
    fn parse_keeps_only_tab_lines_for_ids_in_the_batch() {
        let g = graph();
        let batch: Vec<&Node> = vec![&g.nodes["FR-PAY-22"]];
        let out = "FR-PAY-22\tкак отменить запись\nFR-PAY-26\tчужой\nnoise\nFR-PAY-22\t  \nFR-PAY-22\tштраф за неявку\n";
        let p = parse(out, &batch);
        assert_eq!(p["FR-PAY-22"], vec!["как отменить запись", "штраф за неявку"]);
        assert!(!p.contains_key("FR-PAY-26"));
    }

    #[test]
    fn parse_splits_tab_joined_questions_and_never_stores_the_id() {
        let g = graph();
        let batch: Vec<&Node> = vec![&g.nodes["FR-PAY-22"], &g.nodes["FR-PAY-26"]];
        let out = "FR-PAY-22\tкак отменить запись\tFR-PAY-22\tштраф за неявку\tкто платит\tFR-PAY-26\tсколько спишут\n";
        let p = parse(out, &batch);
        assert_eq!(p["FR-PAY-22"], vec!["как отменить запись", "штраф за неявку", "кто платит"]);
        assert_eq!(p["FR-PAY-26"], vec!["сколько спишут"]);
        assert!(p.values().flatten().all(|q| !q.contains("FR-PAY")));
    }

    #[test]
    fn parse_drops_a_line_in_a_script_nobody_searches_in() {
        let g = graph();
        let batch: Vec<&Node> = vec![&g.nodes["FR-PAY-22"]];
        let out = "FR-PAY-22\tکیا میں بکنگ منسوخ کر سکتا ہوں؟\nFR-PAY-22\tчто делает asGrosze при отмене?\nFR-PAY-22\tCancellationPolicy — 24h?\n";
        let p = parse(out, &batch);
        assert_eq!(p["FR-PAY-22"], vec!["что делает asGrosze при отмене?", "CancellationPolicy — 24h?"]);
    }

    #[test]
    fn load_cleans_questions_stored_before_the_guard_and_forgets_the_emptied() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path());
        let mut qs = Questions::default();
        qs.entries.insert("FR-PAY-22".into(), Entry { hash: "h".into(), questions: vec!["как отменить\tFR-PAY-22\tштраф".into()] });
        qs.entries.insert("ADR-003".into(), Entry { hash: "h".into(), questions: vec!["کیا میں بکنگ منسوخ کر سکتا ہوں؟".into()] });
        qs.save(&store).unwrap();
        let loaded = Questions::load(&store).unwrap();
        assert_eq!(loaded.get("FR-PAY-22"), ["как отменить", "штраф"]);
        assert!(!loaded.entries.contains_key("ADR-003"));
    }

    #[test]
    fn code_is_eligible_when_its_author_wrote_something_or_it_has_a_body() {
        let mut e = Extraction::default();
        e.node(NodeKind::File, "file:a.ts", "a.ts", "The auth surface's decisions.", "a.ts", 1);
        e.node(NodeKind::File, "file:b.ts", "b.ts", "", "b.ts", 1);
        e.node_span(NodeKind::Symbol, "sym:a.ts::doc", "doc", "Ends every session.\nrevoke() {}", "a.ts", (3, 3));
        e.node_span(NodeKind::Symbol, "sym:a.ts::long", "long", "export function f() {", "a.ts", (5, 12));
        e.node_span(NodeKind::Symbol, "sym:a.ts::one", "one", "export const k = 1;", "a.ts", (14, 14));
        e.node(NodeKind::Symbol, "deco:Injectable", "Injectable", "", "a.ts", 3);
        e.node(NodeKind::Requirement, "FR-X-1", "t", "body", "d.md", 1);
        let by = |id: &str| eligible_code(e.nodes.iter().find(|n| n.id == id).unwrap());
        assert!(by("file:a.ts") && by("sym:a.ts::doc") && by("sym:a.ts::long"));
        assert!(!by("file:b.ts") && !by("sym:a.ts::one") && !by("deco:Injectable") && !by("FR-X-1"));
        let mut g = Graph::default();
        g.apply(e);
        // Documents alone decide `coverage`; code has a count of its own.
        assert_eq!(coverage(&g, &Questions::default()), (0, 1));
        assert_eq!(code_coverage(&g, &Questions::default()), (0, 3));
    }

    #[test]
    fn code_questions_are_generated_only_when_asked_for_and_kept_either_way() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path());
        let mut e = Extraction::default();
        e.node(NodeKind::Requirement, "FR-X-1", "t", "body", "d.md", 1);
        e.node(NodeKind::File, "file:a.ts", "a.ts", "The auth surface's decisions.", "a.ts", 1);
        let mut g = Graph::default();
        g.apply(e);
        let cmd = r#"awk '/^### /{printf "%s\tq for %s\n", $2, $2}'"#;
        let r = run(&store, &g, Questions::default(), cmd, 8, 1, Scope::default()).unwrap();
        assert_eq!((r.generated, r.batches), (1, 1), "without --code the file is not asked about");
        assert!(Questions::load(&store).unwrap().get("file:a.ts").is_empty());
        let r = run(&store, &g, Questions::load(&store).unwrap(), cmd, 8, 1, Scope { limit: None, code: true }).unwrap();
        assert_eq!((r.generated, r.batches, r.left), (1, 1, 0));
        assert_eq!(Questions::load(&store).unwrap().get("file:a.ts"), ["q for c1"]);
        // A later run without the flag neither regenerates nor prunes the code questions.
        let r = run(&store, &g, Questions::load(&store).unwrap(), cmd, 8, 1, Scope::default()).unwrap();
        assert_eq!((r.generated, r.dropped), (0, 0));
        assert_eq!(Questions::load(&store).unwrap().get("file:a.ts"), ["q for c1"]);
        assert_eq!(code_coverage(&g, &Questions::load(&store).unwrap()), (1, 1));
    }

    #[test]
    fn a_code_answer_opens_its_entry_by_key_or_by_id_and_never_by_label() {
        let mut e = Extraction::default();
        e.node_span(NodeKind::Symbol, "sym:apps/a.ts::revoke", "revoke", "Ends every session.\nrevoke() {}", "apps/a.ts", (3, 3));
        e.node_span(NodeKind::Symbol, "sym:apps/b.ts::index", "index", "Barrel.\nexport * from './x'", "apps/b.ts", (1, 2));
        let nodes: Vec<&Node> = e.nodes.iter().collect();
        let out = "c1\tкак выйти отовсюду\nsym:apps/b.ts::index\twhere is the barrel\nrevoke\tlabel only\nc3\tno such entry\n";
        let p = parse_keyed(out, &code_keys(&nodes));
        assert_eq!(p["sym:apps/a.ts::revoke"], vec!["как выйти отовсюду"]);
        assert_eq!(p["sym:apps/b.ts::index"], vec!["where is the barrel"]);
        assert_eq!(p.len(), 2);
    }

    #[test]
    fn the_code_prompt_shows_the_path_and_asks_in_both_languages() {
        let mut e = Extraction::default();
        e.node_span(NodeKind::Symbol, "sym:apps/a.ts::revoke", "revoke", "Ends every session.\nrevoke() {}", "apps/a.ts", (3, 3));
        let p = prompt_code(&[&e.nodes[0]]);
        assert!(p.contains("### c1\nsym:apps/a.ts::revoke\napps/a.ts\nrevoke\nEnds every session."), "{p}");
        assert!(p.contains("in the form `key<TAB>text`"));
        assert!(p.contains("four questions in Russian and four in English"));
        assert!(p.contains("Never repeat the identifier"));
    }

    // The generator is a shell command, so the test's generator is `awk` echoing the ids it was
    // given; a real model is never needed to prove the plumbing.
    #[test]
    fn run_generates_for_stale_nodes_only_and_prunes_the_gone() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path());
        let g = graph();
        let cmd = r#"awk '/^### /{printf "%s\tq for %s\n", $2, $2}'"#;
        let r = run(&store, &g, Questions::default(), cmd, 1, 2, Scope::default()).unwrap();
        assert_eq!((r.generated, r.dropped, r.batches, r.failed), (2, 0, 2, 0));
        let q = Questions::load(&store).unwrap();
        assert_eq!(q.get("FR-PAY-22"), ["q for FR-PAY-22"]);
        assert!(q.get("BE-M01-T1").is_empty(), "tasks are not enriched");
        assert!(q.get("entity:Money").is_empty(), "a bare entity name is not enriched");
        // Nothing changed: nothing is generated again.
        let r = run(&store, &g, q, cmd, 8, 1, Scope::default()).unwrap();
        assert_eq!((r.generated, r.batches), (0, 0));
        // A body edit regenerates that node; a removed node is pruned.
        let mut g2 = Graph::default();
        let mut e = Extraction::default();
        e.node(NodeKind::Requirement, "FR-PAY-22", "отмена", "другое тело", "a.md", 1);
        g2.apply(e);
        let r = run(&store, &g2, Questions::load(&store).unwrap(), cmd, 8, 1, Scope::default()).unwrap();
        assert_eq!((r.generated, r.dropped), (1, 1));
    }

    // The generator answers for one id of two on the first call and for every id on the second,
    // counting its calls in a file: the skipped node is asked again alone, and a second skip is
    // not retried.
    #[test]
    fn nodes_a_model_answer_skipped_are_asked_once_more() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path());
        let calls = dir.path().join("calls");
        let cmd = format!(
            r#"echo x >> "{c}"; n=$(wc -l < "{c}" | tr -d " "); awk -v n="$n" '/^### /{{ if (n > 1 || $2 == "FR-PAY-22") printf "%s\tq%s for %s\n", $2, n, $2 }}'"#,
            c = calls.display());
        let r = run(&store, &graph(), Questions::default(), &cmd, 8, 1, Scope::default()).unwrap();
        assert_eq!((r.generated, r.batches, r.failed, r.left), (2, 1, 0, 0));
        let q = Questions::load(&store).unwrap();
        assert_eq!(q.get("FR-PAY-22"), ["q1 for FR-PAY-22"]);
        assert_eq!(q.get("FR-PAY-26"), ["q2 for FR-PAY-26"]);
        assert_eq!(std::fs::read_to_string(&calls).unwrap().lines().count(), 2);

        let silent = format!(r#"echo x >> "{}"; awk '/^### /{{ exit }}'"#, calls.display());
        let r = run(&store, &graph(), Questions::default(), &silent, 8, 1, Scope::default()).unwrap();
        assert_eq!((r.generated, r.left), (0, 2));
        assert_eq!(r.failed, 1, "a batch that answered for nobody twice is failed, not done");
        assert_eq!(std::fs::read_to_string(&calls).unwrap().lines().count(), 4, "an answer that skips everything is retried once, not forever");
    }

    /// A store with no questions is a store nobody enriched, and saying so on every build would be
    /// noise on a corpus that never runs `enrich`. A store that has some and is missing others is
    /// a rebuild that moved the corpus, and that is the line worth printing.
    #[test]
    fn the_line_is_printed_only_when_the_store_has_questions_and_is_missing_some() {
        let mut e = Extraction::default();
        e.node(NodeKind::Requirement, "FR-X-1", "t", "body", "d.md", 1);
        e.node(NodeKind::Requirement, "FR-X-2", "t2", "body2", "d.md", 5);
        let mut g = Graph::default();
        g.apply(e);
        let mut q = Questions::default();
        assert_eq!(unenriched_note(&g, &q), None, "a store nobody enriched says nothing");
        let entry = |id: &str| Entry { hash: hash(&passage(&g.nodes[id])), questions: vec!["q".to_string()] };
        q.entries.insert("FR-X-1".into(), entry("FR-X-1"));
        assert_eq!(unenriched_note(&g, &q), Some(1));
        q.entries.insert("FR-X-2".into(), entry("FR-X-2"));
        assert_eq!(unenriched_note(&g, &q), None, "a complete store says nothing either");
    }

    /// `sh -c` returns its pipeline's last stage, so a generator that dies into a `tee` exits 0
    /// with nothing on stdout. That is what happened on 167 batches once, and it was read as
    /// coverage: 0 nodes written, 0 failed, exit 0.
    #[test]
    fn a_pipeline_whose_generator_died_is_a_failed_batch() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path());
        let r = run(&store, &graph(), Questions::default(), "false | cat", 8, 1, Scope::default()).unwrap();
        assert_eq!(r.generated, 0);
        assert!(r.failed > 0, "a pipeline that produced nothing is not coverage: {r:?}");
        assert!(r.left > 0);
    }

    /// The load-bearing half, and the one that holds on a shell without `pipefail`: an answer that
    /// names nobody is the generator not answering, whatever its exit status said.
    #[test]
    fn an_empty_answer_is_a_failed_batch_after_its_retry() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path());
        let r = run(&store, &graph(), Questions::default(), "cat > /dev/null", 8, 1, Scope::default()).unwrap();
        assert_eq!((r.generated, r.batches), (0, 1));
        assert_eq!(r.failed, 1, "one batch, asked twice, answered nothing twice");
    }

    #[test]
    fn a_command_that_answers_without_reading_its_prompt_still_answers() {
        // Larger than any pipe buffer, so the write blocks until the child closes its end of the
        // pipe — the race the CI runner lost on the reranker's stub command, made deterministic.
        let prompt = "x".repeat(1 << 20);
        let out = run_command("exec 0<&-; printf 'answered\\n'", &prompt).unwrap();
        assert_eq!(out, "answered\n");
    }

    #[test]
    fn a_failing_command_is_counted_not_fatal() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path());
        let r = run(&store, &graph(), Questions::default(), "exit 3", 8, 1, Scope::default()).unwrap();
        assert_eq!((r.generated, r.failed), (0, 1));
    }

    #[test]
    fn split_joined_splits_a_bare_comma_list_but_keeps_a_single_question_with_a_comma() {
        assert_eq!(
            split_joined("как отменить бронь, кто платит штраф, когда деньги спишут"),
            vec!["как отменить бронь", "кто платит штраф", "когда деньги спишут"]
        );
        assert_eq!(split_joined("а если это оплата, ну как быть?"), vec!["а если это оплата, ну как быть?"]);
    }

    #[test]
    fn readable_is_exactly_half_known_letters_at_the_boundary() {
        // 2 Latin + 2 Urdu letters: known*2 (4) >= letters (4), so the `>=` boundary passes.
        assert!(readable("ab کی"));
        // 1 Latin + 2 Urdu: known*2 (2) < letters (3), just under the boundary.
        assert!(!readable("a کی"));
    }

    #[test]
    fn passage_truncates_the_body_to_the_documented_character_limit() {
        let mut e = Extraction::default();
        e.node(NodeKind::Requirement, "FR-PAY-22", "title", &"щ".repeat(PASSAGE_CHARS + 200), "a.md", 1);
        let n = e.nodes.remove(0);
        let p = passage(&n);
        assert_eq!(p.chars().count(), "title".len() + 1 + PASSAGE_CHARS);
    }

    #[test]
    fn an_entity_with_only_whitespace_body_is_not_eligible() {
        let mut e = Extraction::default();
        e.node(NodeKind::Entity, "entity:Money", "Money", "   \n\t", "a.md", 1);
        assert!(!eligible(&e.nodes[0]));
    }

    #[test]
    fn coverage_counts_eligible_nodes_only_and_reads_full_after_a_whole_run() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path());
        let g = graph();
        assert_eq!(coverage(&g, &Questions::default()), (0, 2));
        let cmd = r#"awk '/^### /{printf "%s\tq for %s\n", $2, $2}'"#;
        run(&store, &g, Questions::default(), cmd, 1, 1, Scope { limit: Some(1), code: false }).unwrap();
        assert_eq!(coverage(&g, &Questions::load(&store).unwrap()), (1, 2), "a run stopped early covers part of the graph");
        assert!(!enriched(1, 2), "half a two-node graph is nowhere near the mark");
        run(&store, &g, Questions::load(&store).unwrap(), cmd, 1, 1, Scope::default()).unwrap();
        assert_eq!(coverage(&g, &Questions::load(&store).unwrap()), (2, 2));
        assert!(enriched(2, 2));
    }

    #[test]
    fn enriched_grades_at_the_high_water_mark_not_at_every_node() {
        // The development corpus's own denominator, so the counts read as the summary line does.
        assert!(enriched(1996, 1996));
        assert!(enriched(1977, 1996), "the first count at or above 99 % — the mark is 1976.04");
        assert!(!enriched(1976, 1996), "98.998 %, one node short of the mark rather than over it");
        assert!(!enriched(1900, 1996));
        // A graph with nothing to enrich cannot be told from one nobody has enriched.
        assert!(!enriched(0, 0));
    }

    #[test]
    fn run_respects_the_limit_argument() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path());
        let cmd = r#"awk '/^### /{printf "%s\tq for %s\n", $2, $2}'"#;
        let r = run(&store, &graph(), Questions::default(), cmd, 1, 1, Scope { limit: Some(1), code: false }).unwrap();
        assert_eq!((r.generated, r.batches), (1, 1));
    }

    #[test]
    fn a_batch_or_parallel_count_of_zero_does_not_panic_and_still_processes_everything() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path());
        let cmd = r#"awk '/^### /{printf "%s\tq for %s\n", $2, $2}'"#;
        let r = run(&store, &graph(), Questions::default(), cmd, 0, 0, Scope::default()).unwrap();
        assert_eq!(r.generated, 2);
        assert!(Questions::load(&store).unwrap().get("FR-PAY-22").len() == 1);
    }

    /// The shell a default Git for Windows install leaves findable: `git` on PATH under `cmd\`,
    /// `sh` two directories over. A synthetic tree and a synthetic PATH, so the test is about the
    /// lookup and not about what this machine has installed.
    #[cfg(windows)]
    #[test]
    fn a_shell_that_is_not_on_path_is_found_beside_git_and_then_under_program_files() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("Git");
        for f in ["cmd\\git.exe", "bin\\sh.exe", "usr\\bin\\awk.exe"] {
            let p = root.join(f);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(&p, b"").unwrap();
        }
        let elsewhere = dir.path().join("elsewhere");
        std::fs::create_dir_all(&elsewhere).unwrap();
        assert_eq!(git_sh(&[elsewhere.clone(), root.join("cmd")], None, &[]), Some(root.join("bin").join("sh.exe")), "beside git");
        assert_eq!(git_sh(std::slice::from_ref(&elsewhere), None, &[dir.path().to_path_buf()]), Some(root.join("bin").join("sh.exe")), "under a program directory");
        assert_eq!(git_sh(std::slice::from_ref(&elsewhere), Some(&root.join("bin").join("bash.exe")), &[]), Some(root.join("bin").join("sh.exe")), "from the bash Claude Code names");
        assert_eq!(git_sh(&[elsewhere], None, &[]), None, "nothing to find");
    }
}
