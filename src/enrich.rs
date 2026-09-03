//! Questions a reader might ask to reach a node, written once by a language model and cached
//! by passage hash. They give `ask --rerank`'s candidate pool the reader's vocabulary as well
//! as the author's: measured on the development corpus they move no seed on their own, but
//! carry every reachable paraphrase target into the pool the model picks from.
use crate::model::{Graph, Node, NodeKind};
use crate::store::{Source, Store};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::Write;
use std::sync::{Arc, Mutex};

const FILE: &str = "questions.json";
const KINDS: [NodeKind; 5] = [NodeKind::Requirement, NodeKind::Invariant, NodeKind::Adr, NodeKind::Milestone, NodeKind::Entity];
const PASSAGE_CHARS: usize = 1500;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry { pub hash: String, pub questions: Vec<String> }

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Questions { pub entries: BTreeMap<String, Entry> }

pub struct Report { pub generated: usize, pub dropped: usize, pub batches: usize, pub failed: usize, pub left: usize }

fn passage(n: &Node) -> String {
    let body: String = n.body.chars().take(PASSAGE_CHARS).collect();
    format!("{}\n{}", n.label, body)
}

fn hash(text: &str) -> String { blake3::hash(text.as_bytes()).to_hex().to_string() }

/// An entity that is only a name (a backticked span in some title) gives the model nothing to
/// ask about, and it says so by skipping the entry — 63 of them on the bench corpus, retried on
/// every run until they were left out.
pub fn eligible(n: &Node) -> bool { KINDS.contains(&n.kind) && !(n.kind == NodeKind::Entity && n.body.trim().is_empty()) }

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
    fn stale<'a>(&self, graph: &'a Graph) -> Vec<(&'a Node, String)> {
        graph.nodes.values().filter(|n| eligible(n)).filter_map(|n| {
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
        self.entries.retain(|id, _| graph.nodes.get(id).is_some_and(eligible));
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

/// Lines of `id<TAB>question` for ids in the batch; anything else is ignored. A line may carry
/// several questions tab-joined, an id before each — the bench corpus had 40 such lines, each
/// stored whole with its own id inside, which the exact stage then answered for free.
pub fn parse(output: &str, batch: &[&Node]) -> BTreeMap<String, Vec<String>> {
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for line in output.lines() {
        let mut current: Option<&str> = None;
        for part in line.split('\t').map(str::trim).filter(|p| !p.is_empty()) {
            if let Some(n) = batch.iter().find(|n| n.id == part) {
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

pub fn run_command(command: &str, input: &str) -> Result<String> {
    let mut child = std::process::Command::new("sh").arg("-c").arg(command)
        .stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped())
        .spawn().with_context(|| format!("spawn `{command}`"))?;
    child.stdin.take().context("stdin")?.write_all(input.as_bytes())?;
    let out = child.wait_with_output()?;
    if !out.status.success() {
        anyhow::bail!("`{command}` exited {}: {}", out.status, String::from_utf8_lossy(&out.stderr).trim());
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Generates questions for every stale node through `command` (prompt on stdin, lines on
/// stdout), `parallel` batches at a time, saving after each batch so an interrupted run keeps
/// what it paid for.
pub fn run(store: &Store, graph: &Graph, questions: Questions, command: &str, batch: usize, parallel: usize, limit: Option<usize>) -> Result<Report> {
    let mut questions = questions;
    let dropped = questions.prune(graph);
    let mut stale = questions.stale(graph);
    if let Some(l) = limit { stale.truncate(l); }
    // A batch the model answers in prose instead of `id<TAB>text` leaves its nodes without
    // questions and its exit status green; measured once on 172 batches, 7 came back that way
    // and 195 nodes silently stayed unsearchable. The nodes an answer skipped go round once more.
    let batches: Vec<(Vec<(&Node, String)>, bool)> = stale.chunks(batch.max(1)).map(|c| (c.to_vec(), false)).collect();
    let total = batches.len();
    let queue = Arc::new(Mutex::new(batches));
    let shared = Arc::new(Mutex::new((questions, 0usize, 0usize)));
    std::thread::scope(|s| {
        for _ in 0..parallel.max(1) {
            let (queue, shared) = (Arc::clone(&queue), Arc::clone(&shared));
            s.spawn(move || loop {
                let Some((b, retry)) = queue.lock().unwrap().pop() else { break };
                let nodes: Vec<&Node> = b.iter().map(|(n, _)| *n).collect();
                match run_command(command, &prompt(&nodes)) {
                    Ok(out) => {
                        let parsed = parse(&out, &nodes);
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
                        let mut queue = queue.lock().unwrap();
                        if !skipped.is_empty() && !retry {
                            eprintln!("enrich: {} of {} nodes skipped by the model, retrying them", skipped.len(), b.len());
                            queue.push((skipped, true));
                        }
                        eprintln!("enrich: batch done, {} of {} left", queue.len(), total);
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
    let left = questions.stale(graph).len();
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

    // The generator is a shell command, so the test's generator is `awk` echoing the ids it was
    // given; a real model is never needed to prove the plumbing.
    #[test]
    fn run_generates_for_stale_nodes_only_and_prunes_the_gone() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path());
        let g = graph();
        let cmd = r#"awk '/^### /{printf "%s\tq for %s\n", $2, $2}'"#;
        let r = run(&store, &g, Questions::default(), cmd, 1, 2, None).unwrap();
        assert_eq!((r.generated, r.dropped, r.batches, r.failed), (2, 0, 2, 0));
        let q = Questions::load(&store).unwrap();
        assert_eq!(q.get("FR-PAY-22"), ["q for FR-PAY-22"]);
        assert!(q.get("BE-M01-T1").is_empty(), "tasks are not enriched");
        assert!(q.get("entity:Money").is_empty(), "a bare entity name is not enriched");
        // Nothing changed: nothing is generated again.
        let r = run(&store, &g, q, cmd, 8, 1, None).unwrap();
        assert_eq!((r.generated, r.batches), (0, 0));
        // A body edit regenerates that node; a removed node is pruned.
        let mut g2 = Graph::default();
        let mut e = Extraction::default();
        e.node(NodeKind::Requirement, "FR-PAY-22", "отмена", "другое тело", "a.md", 1);
        g2.apply(e);
        let r = run(&store, &g2, Questions::load(&store).unwrap(), cmd, 8, 1, None).unwrap();
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
        let r = run(&store, &graph(), Questions::default(), &cmd, 8, 1, None).unwrap();
        assert_eq!((r.generated, r.batches, r.failed, r.left), (2, 1, 0, 0));
        let q = Questions::load(&store).unwrap();
        assert_eq!(q.get("FR-PAY-22"), ["q1 for FR-PAY-22"]);
        assert_eq!(q.get("FR-PAY-26"), ["q2 for FR-PAY-26"]);
        assert_eq!(std::fs::read_to_string(&calls).unwrap().lines().count(), 2);

        let silent = format!(r#"echo x >> "{}"; awk '/^### /{{ exit }}'"#, calls.display());
        let r = run(&store, &graph(), Questions::default(), &silent, 8, 1, None).unwrap();
        assert_eq!((r.generated, r.left), (0, 2));
        assert_eq!(std::fs::read_to_string(&calls).unwrap().lines().count(), 4, "an answer that skips everything is retried once, not forever");
    }

    #[test]
    fn a_failing_command_is_counted_not_fatal() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path());
        let r = run(&store, &graph(), Questions::default(), "exit 3", 8, 1, None).unwrap();
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
    fn run_respects_the_limit_argument() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path());
        let cmd = r#"awk '/^### /{printf "%s\tq for %s\n", $2, $2}'"#;
        let r = run(&store, &graph(), Questions::default(), cmd, 1, 1, Some(1)).unwrap();
        assert_eq!((r.generated, r.batches), (1, 1));
    }

    #[test]
    fn a_batch_or_parallel_count_of_zero_does_not_panic_and_still_processes_everything() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path());
        let cmd = r#"awk '/^### /{printf "%s\tq for %s\n", $2, $2}'"#;
        let r = run(&store, &graph(), Questions::default(), cmd, 0, 0, None).unwrap();
        assert_eq!(r.generated, 2);
        assert!(Questions::load(&store).unwrap().get("FR-PAY-22").len() == 1);
    }
}
