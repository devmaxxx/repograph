//! Questions a reader might ask to reach a node, written once by a language model and cached
//! by passage hash. They give `ask --rerank`'s candidate pool the reader's vocabulary as well
//! as the author's: measured on the development corpus they move no seed on their own, but
//! carry every reachable paraphrase target into the pool the model picks from.
use crate::model::{Graph, Node, NodeKind};
use crate::store::Store;
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

pub struct Report { pub generated: usize, pub dropped: usize, pub batches: usize, pub failed: usize }

fn passage(n: &Node) -> String {
    let body: String = n.body.chars().take(PASSAGE_CHARS).collect();
    format!("{}\n{}", n.label, body)
}

fn hash(text: &str) -> String { blake3::hash(text.as_bytes()).to_hex().to_string() }

pub fn eligible(n: &Node) -> bool { KINDS.contains(&n.kind) }

impl Questions {
    pub fn load(store: &Store) -> Result<Questions> {
        match store.read_bytes(FILE)? {
            Some(b) => serde_json::from_slice(&b).context(FILE),
            None => Ok(Questions::default()),
        }
    }

    pub fn save(&self, store: &Store) -> Result<()> {
        store.write_atomic(FILE, &serde_json::to_vec_pretty(self)?)
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

/// Lines of `id<TAB>question` for ids in the batch; anything else is ignored.
pub fn parse(output: &str, batch: &[&Node]) -> BTreeMap<String, Vec<String>> {
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for line in output.lines() {
        let Some((id, q)) = line.split_once('\t') else { continue };
        let (id, q) = (id.trim(), q.trim());
        if q.is_empty() || !batch.iter().any(|n| n.id == id) { continue; }
        out.entry(id.to_string()).or_default().extend(split_joined(q));
    }
    out
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
    let batches: Vec<Vec<(&Node, String)>> = stale.chunks(batch.max(1)).map(|c| c.to_vec()).collect();
    let total = batches.len();
    let queue = Arc::new(Mutex::new(batches));
    let shared = Arc::new(Mutex::new((questions, 0usize, 0usize)));
    std::thread::scope(|s| {
        for _ in 0..parallel.max(1) {
            let (queue, shared) = (Arc::clone(&queue), Arc::clone(&shared));
            s.spawn(move || loop {
                let Some(b) = queue.lock().unwrap().pop() else { break };
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
                        eprintln!("enrich: batch done, {} of {} left", queue.lock().unwrap().len(), total);
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
    Ok(Report { generated, dropped, batches: total, failed })
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

    #[test]
    fn a_failing_command_is_counted_not_fatal() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path());
        let r = run(&store, &graph(), Questions::default(), "exit 3", 8, 1, None).unwrap();
        assert_eq!((r.generated, r.failed), (0, 1));
    }
}
