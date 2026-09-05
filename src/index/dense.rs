use crate::enrich::Questions;
use crate::model::{Graph, NodeKind};
use crate::store::Store;
use crate::walk::Stamp;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Node ids best first, each with the cosine of its best row.
pub type Scored = Vec<(String, f32)>;

/// Holes tolerated per live row before `sync` compacts. Compaction costs the whole-file rewrite
/// the append exists to avoid, so it is worth a quarter of the file being dead weight.
const HOLE_SHARE: usize = 4;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct DenseIndex {
    /// One entry per row of `vectors.f32`, in the file's own order: a row keeps its offset for
    /// as long as it lives, which is what lets a sync append instead of rewriting 50 MB. A dead
    /// row holds an empty id until the next compaction closes the gap.
    pub ids: Vec<String>,
    pub hashes: Vec<String>,
    /// True for a generated-question row; stores written before enrichment existed have none.
    #[serde(default)] pub kinds: Vec<bool>,
    pub dim: usize,
    /// The dead rows, ascending. Left out of the file when there are none, so a store this
    /// binary wrote and never punched a hole in still reads in one that predates the field.
    #[serde(default, skip_serializing_if = "Vec::is_empty")] free: Vec<usize>,
    #[serde(skip)] pub vectors: Vec<f32>,
    /// The live rows, ascending — what `search` scans, so a dead row is not even a branch on
    /// the hot path.
    #[serde(skip)] live: Vec<usize>,
    /// Leading rows of `vectors.f32` that already hold what memory holds, with the stamp of the
    /// file they were counted in: an append may only extend a file no one else has rewritten.
    #[serde(skip)] persisted: usize,
    #[serde(skip)] stamp: Option<Stamp>,
}

/// Every text embedded for a node, e5-prefixed. The passage is the node itself; each generated
/// question is embedded as a query, since the reader's question is one too (e5's symmetric case).
/// A file has no passage row — its head comment is for the prompt, not the index — and is
/// present through its questions alone, once `enrich --code` has asked about it.
fn rows(n: &crate::model::Node, questions: &Questions) -> Vec<String> {
    let mut out = Vec::new();
    if n.kind != NodeKind::File { out.push(format!("passage: {}\n{}", n.label, n.indexed_body())); }
    out.extend(questions.get(&n.id).iter().map(|q| format!("query: {q}")));
    out
}
fn normalise(v: &mut [f32]) {
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 { for x in v { *x /= norm; } }
}
fn le_bytes(v: &[f32]) -> Vec<u8> {
    let mut raw = Vec::with_capacity(v.len() * 4);
    for x in v { raw.extend_from_slice(&x.to_le_bytes()); }
    raw
}

impl DenseIndex {
    /// Whether an index is on disk, without reading it: an exact-id or `--no-dense` answer
    /// never needs the vectors, and loading 50 MB of them cost every such `ask` 40 ms.
    pub fn present(store: &Store) -> bool {
        store.has("vectors.json") && store.has("vectors.f32")
    }

    pub fn load(store: &Store) -> Result<DenseIndex> {
        let Some(meta) = store.read_bytes("vectors.json")? else { return Ok(DenseIndex::default()) };
        let mut idx: DenseIndex = serde_json::from_slice(&meta).context("vectors.json")?;
        // Stamped before the read, never after: a rewrite racing this read then leaves a stamp
        // the next save cannot match, and it rewrites the file whole instead of appending to a
        // prefix that is no longer ours.
        idx.stamp = store.stamp("vectors.f32");
        let raw = store.read_bytes("vectors.f32")?.unwrap_or_default();
        let want = idx.ids.len() * idx.dim;
        if raw.len() / 4 < want {
            // A torn pair of files is treated as no index at all; the next sync rebuilds it.
            return Ok(DenseIndex::default());
        }
        // Anything past the last row the metadata names is what a crash between an append and
        // the metadata rename left: unreferenced, and overwritten by the next append.
        #[allow(clippy::chunks_exact_to_as_chunks)]
        { idx.vectors = raw[..want * 4].chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect(); }
        idx.persisted = idx.ids.len();
        idx.reindex();
        Ok(idx)
    }

    /// The per-row arrays lined up with the ids and the live scan order rebuilt. The resize is
    /// for stores older than the `kinds` field: without it the first appended row would land at
    /// index 0 of a short array and answer for someone else's row.
    fn reindex(&mut self) {
        self.hashes.resize(self.ids.len(), String::new());
        self.kinds.resize(self.ids.len(), false);
        let mut dead = vec![false; self.ids.len()];
        for &i in &self.free { if let Some(d) = dead.get_mut(i) { *d = true; } }
        self.free = dead.iter().enumerate().filter(|(_, d)| **d).map(|(i, _)| i).collect();
        self.live = dead.iter().enumerate().filter(|(_, d)| !**d).map(|(i, _)| i).collect();
    }

    /// The new rows appended and the metadata renamed over the old, in that order: a crash
    /// between the two leaves rows nothing points at, never metadata pointing at rows that are
    /// not there. The whole file is rewritten instead when what is in hand is no longer an
    /// extension of what is on disk — after a compaction, or when another process rewrote it.
    pub fn save(&mut self, store: &Store) -> Result<()> {
        let kept = self.persisted * self.dim;
        let extends = self.persisted > 0 && kept <= self.vectors.len()
            && self.stamp.is_some() && self.stamp == store.stamp("vectors.f32");
        if extends {
            store.append_after("vectors.f32", kept as u64 * 4, &le_bytes(&self.vectors[kept..]))?;
        } else {
            store.write_atomic("vectors.f32", &le_bytes(&self.vectors))?;
        }
        self.persisted = self.ids.len();
        self.stamp = store.stamp("vectors.f32");
        store.write_atomic("vectors.json", &serde_json::to_vec(self)?)
    }

    #[allow(clippy::type_complexity)]
    pub fn sync(&mut self, graph: &Graph, questions: &Questions, embed: &mut dyn FnMut(&[String]) -> Result<Vec<Vec<f32>>>) -> Result<usize> {
        let mut alive = vec![false; self.ids.len()];
        let mut todo_ids = Vec::new();
        let mut todo_texts = Vec::new();
        let mut todo_hashes = Vec::new();
        let mut todo_kinds = Vec::new();
        let old: std::collections::HashMap<(&str, &str), usize> =
            self.live.iter().map(|&i| ((self.ids[i].as_str(), self.hashes[i].as_str()), i)).collect();
        for n in graph.nodes.values() {
            for text in rows(n, questions) {
                let hash = blake3::hash(text.as_bytes()).to_hex().to_string();
                let is_q = text.starts_with("query: ");
                match old.get(&(n.id.as_str(), hash.as_str())).copied() {
                    Some(i) if self.dim > 0 => alive[i] = true,
                    _ => { todo_ids.push(n.id.clone()); todo_texts.push(text); todo_hashes.push(hash); todo_kinds.push(is_q); }
                }
            }
        }
        let embedded = todo_ids.len();
        if embedded > 0 {
            let mut vecs = embed(&todo_texts)?;
            for v in vecs.iter_mut() { normalise(v); }
            let dim = vecs.first().map(|v| v.len()).unwrap_or(self.dim);
            // A model of another width invalidates every stored offset, so the old rows cannot
            // be appended to — they go, and the file is rewritten from the new ones alone.
            if self.dim > 0 && dim != self.dim {
                self.ids.clear(); self.hashes.clear(); self.kinds.clear(); self.vectors.clear();
                alive.clear();
                self.persisted = 0;
            }
            self.dim = dim;
            for ((id, (hash, v)), is_q) in todo_ids.into_iter().zip(todo_hashes.into_iter().zip(vecs)).zip(todo_kinds) {
                self.ids.push(id);
                self.hashes.push(hash);
                self.kinds.push(is_q);
                self.vectors.extend_from_slice(&v);
                alive.push(true);
            }
        }
        self.free = alive.iter().enumerate().filter(|(_, a)| !**a).map(|(i, _)| i).collect();
        // A hole keeps its row's floats — that is what holds the offsets still — but not its
        // identity: nothing may match it again, and the metadata is rewritten on every save.
        for &i in &self.free { self.ids[i].clear(); self.hashes[i].clear(); self.kinds[i] = false; }
        self.reindex();
        if self.free.len() * HOLE_SHARE > self.live.len() { self.compact(); }
        Ok(embedded)
    }

    /// The live rows closed up. Every offset moves, so the next save rewrites both files whole,
    /// exactly as every save did before the rows became append-only.
    fn compact(&mut self) {
        let live = std::mem::take(&mut self.live);
        let mut vectors = Vec::with_capacity(live.len() * self.dim);
        let mut ids = Vec::with_capacity(live.len());
        let mut hashes = Vec::with_capacity(live.len());
        let mut kinds = Vec::with_capacity(live.len());
        for &i in &live {
            vectors.extend_from_slice(&self.vectors[i * self.dim..(i + 1) * self.dim]);
            ids.push(std::mem::take(&mut self.ids[i]));
            hashes.push(std::mem::take(&mut self.hashes[i]));
            kinds.push(self.kinds[i]);
        }
        self.ids = ids;
        self.hashes = hashes;
        self.kinds = kinds;
        self.vectors = vectors;
        self.free.clear();
        self.live = (0..self.ids.len()).collect();
        self.persisted = 0;
        if self.ids.is_empty() { self.dim = 0; }
    }

    /// The passage rows and the question rows ranked separately, a node once per list by its
    /// best row. Pooling both kinds into one list buries targets: a passage at rank 2 fell to 87
    /// behind other nodes' question rows.
    pub fn search(&self, query: &[f32], k: usize) -> (Vec<String>, Vec<String>) {
        let (passages, generated) = self.search_scored(query, k, None);
        (passages.into_iter().map(|(id, _)| id).collect(), generated.into_iter().map(|(id, _)| id).collect())
    }

    /// `search` with each node's best cosine, skipping the rows whose hash is `exclude`: a
    /// query that is itself a stored question must not be answered by its own row.
    pub fn search_scored(&self, query: &[f32], k: usize, exclude: Option<&str>) -> (Scored, Scored) {
        if self.dim == 0 || query.len() != self.dim { return (Vec::new(), Vec::new()); }
        let mut q = query.to_vec();
        normalise(&mut q);
        let mut rows: Vec<(f32, &str, bool)> = self.live.iter().copied()
            .filter(|&i| exclude.is_none_or(|h| self.hashes[i] != *h))
            .map(|i| {
                let v = &self.vectors[i * self.dim..(i + 1) * self.dim];
                (v.iter().zip(&q).map(|(a, b)| a * b).sum::<f32>(), self.ids[i].as_str(), self.kinds[i])
            }).collect();
        rows.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap().then(a.1.cmp(b.1)));
        let pick = |want: bool| -> Scored {
            let mut seen = std::collections::HashSet::new();
            rows.iter().filter(|r| r.2 == want).filter(|r| seen.insert(r.1)).take(k).map(|r| (r.1.to_string(), r.0)).collect()
        };
        (pick(false), pick(true))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Extraction, NodeKind};

    fn graph(body22: &str) -> Graph {
        let mut g = Graph::default();
        let mut e = Extraction::default();
        e.node(NodeKind::Requirement, "FR-PAY-22", "отмена", body22, "a.md", 1);
        e.node(NodeKind::Requirement, "FR-PAY-26", "штраф", "списание", "a.md", 9);
        e.node(NodeKind::File, "file:a.md", "a.md", "", "a.md", 1);
        g.apply(e);
        g
    }

    /// Ten nodes with distinct three-byte labels, `edited` of them carrying a body that differs
    /// from the default one. Wide enough that a handful of holes stays under the compaction
    /// share, which is what the append path needs to be observable.
    fn wide(edited: u32) -> Graph {
        let mut g = Graph::default();
        let mut e = Extraction::default();
        for i in 0..10u32 {
            let body = if i < edited { "изменённое" } else { "тело" };
            e.node(NodeKind::Requirement, &format!("FR-W-{i}"), &format!("n{i}x"), body, "w.md", i + 1);
        }
        g.apply(e);
        g
    }

    /// A stand-in embedder: a 3-d vector from the first three bytes after the e5 prefix, so
    /// tests are deterministic.
    fn fake(texts: &[String]) -> Result<Vec<Vec<f32>>> {
        Ok(texts.iter().map(|t| {
            let b = t.split_once(": ").map(|x| x.1).unwrap_or(t).as_bytes();
            vec![b[0] as f32, b.get(1).copied().unwrap_or(0) as f32, b.get(2).copied().unwrap_or(0) as f32]
        }).collect())
    }

    fn synced(g: &Graph) -> DenseIndex {
        let mut idx = DenseIndex::default();
        idx.sync(g, &Questions::default(), &mut fake).unwrap();
        idx
    }

    #[test]
    fn sync_embeds_only_changed_nodes_and_drops_removed_ones() {
        let mut idx = DenseIndex::default();
        assert_eq!(idx.sync(&graph("политика"), &Questions::default(), &mut fake).unwrap(), 2);
        assert_eq!(idx.ids.len(), 2);
        assert_eq!(idx.sync(&graph("политика"), &Questions::default(), &mut fake).unwrap(), 0);
        assert_eq!(idx.sync(&graph("другое"), &Questions::default(), &mut fake).unwrap(), 1);
        let mut g = graph("другое");
        g.remove_file("a.md");
        assert_eq!(idx.sync(&g, &Questions::default(), &mut fake).unwrap(), 0);
        assert!(idx.ids.is_empty() && idx.vectors.is_empty());
    }

    #[test]
    fn a_file_has_no_passage_row_and_is_present_through_its_questions() {
        let mut idx = DenseIndex::default();
        assert_eq!(idx.sync(&graph("x"), &Questions::default(), &mut fake).unwrap(), 2, "two requirements, no row for the file");
        let mut q = Questions::default();
        q.entries.insert("file:a.md".into(), crate::enrich::Entry { hash: String::new(), questions: vec!["где список".into()] });
        assert_eq!(idx.sync(&graph("x"), &q, &mut fake).unwrap(), 1);
        let v = fake(&["query: где список".to_string()]).unwrap().remove(0);
        let (passages, questions) = idx.search(&v, 3);
        assert_eq!(questions[0], "file:a.md");
        assert!(!passages.contains(&"file:a.md".to_string()));
    }

    #[test]
    fn search_is_cosine_descending() {
        let idx = synced(&graph("x"));
        let q = fake(&["штраф".to_string()]).unwrap().remove(0);
        assert_eq!(idx.search(&q, 2).0[0], "FR-PAY-26");
    }

    #[test]
    fn an_append_leaves_the_surviving_rows_at_their_offsets() {
        let mut idx = synced(&wide(0));
        let before = idx.vectors.clone();
        assert_eq!(idx.sync(&wide(1), &Questions::default(), &mut fake).unwrap(), 1);
        assert_eq!(idx.free, vec![0]);
        assert_eq!(idx.ids.len(), 11);
        assert_eq!(idx.vectors[idx.dim..before.len()], before[idx.dim..]);
        assert_eq!(idx.ids[10], "FR-W-0");
        assert_eq!(idx.live, (1..11).collect::<Vec<_>>());
    }

    #[test]
    fn a_dead_row_is_never_returned() {
        let mut idx = synced(&wide(0));
        let stale = fake(&["passage: n0x\nтело".to_string()]).unwrap().remove(0);
        idx.sync(&wide(1), &Questions::default(), &mut fake).unwrap();
        let (passages, _) = idx.search(&stale, 20);
        assert_eq!(passages.len(), 10, "one row per live node, the hole scanned by nobody");
        assert!(!passages.iter().any(|id| id.is_empty()));
    }

    #[test]
    fn compaction_fires_past_the_hole_share_and_searches_identically() {
        let mut idx = synced(&wide(0));
        // Three of ten rows die at once: past a quarter of the live rows, so the holes close.
        idx.sync(&wide(3), &Questions::default(), &mut fake).unwrap();
        assert!(idx.free.is_empty());
        assert_eq!(idx.ids.len(), 10);
        let fresh = synced(&wide(3));
        let q = fake(&["passage: n7x\nтело".to_string()]).unwrap().remove(0);
        assert_eq!(idx.search_scored(&q, 10, None), fresh.search_scored(&q, 10, None));
    }

    #[test]
    fn an_append_saves_without_rewriting_the_rows_already_on_disk() {
        let d = tempfile::tempdir().unwrap();
        let store = Store::new(d.path());
        let mut idx = synced(&wide(0));
        idx.save(&store).unwrap();
        let rows = d.path().join(".repograph/vectors.f32");
        let before = std::fs::read(&rows).unwrap();
        idx.sync(&wide(1), &Questions::default(), &mut fake).unwrap();
        idx.save(&store).unwrap();
        let after = std::fs::read(&rows).unwrap();
        assert_eq!(after.len(), before.len() + idx.dim * 4, "one row longer, nothing rewritten");
        assert_eq!(after[..before.len()], before[..]);
        let back = DenseIndex::load(&store).unwrap();
        let q = fake(&["passage: n4x\nтело".to_string()]).unwrap().remove(0);
        assert_eq!(back.free, vec![0]);
        assert_eq!(back.search_scored(&q, 10, None), idx.search_scored(&q, 10, None));
    }

    #[test]
    fn a_store_written_before_the_holes_field_loads_unchanged() {
        let d = tempfile::tempdir().unwrap();
        let store = Store::new(d.path());
        let idx = synced(&graph("x"));
        // Neither `kinds` nor `free`, as the first release wrote it.
        let meta = serde_json::json!({ "ids": idx.ids, "hashes": idx.hashes, "dim": idx.dim });
        store.write_atomic("vectors.json", &serde_json::to_vec(&meta).unwrap()).unwrap();
        store.write_atomic("vectors.f32", &le_bytes(&idx.vectors)).unwrap();
        let back = DenseIndex::load(&store).unwrap();
        assert_eq!(back.kinds, vec![false, false]);
        let q = fake(&["штраф".to_string()]).unwrap().remove(0);
        assert_eq!(back.search(&q, 2).0[0], "FR-PAY-26");
    }

    #[test]
    fn a_store_with_no_holes_serialises_without_the_holes_field() {
        let idx = synced(&graph("x"));
        let json = String::from_utf8(serde_json::to_vec(&idx).unwrap()).unwrap();
        assert!(!json.contains("free"), "{json}");
    }

    #[test]
    fn rows_left_by_a_crash_before_the_metadata_rename_are_ignored_then_overwritten() {
        let d = tempfile::tempdir().unwrap();
        let store = Store::new(d.path());
        let mut idx = synced(&wide(0));
        idx.save(&store).unwrap();
        let rows = d.path().join(".repograph/vectors.f32");
        let good = std::fs::read(&rows).unwrap();
        // What an append interrupted before its metadata reached disk leaves behind.
        std::fs::write(&rows, [good.clone(), le_bytes(&[9.0, 9.0, 9.0])].concat()).unwrap();
        let mut back = DenseIndex::load(&store).unwrap();
        assert_eq!(back.vectors.len(), 10 * back.dim);
        let q = fake(&["passage: n4x\nтело".to_string()]).unwrap().remove(0);
        assert_eq!(back.search_scored(&q, 10, None), idx.search_scored(&q, 10, None));
        back.sync(&wide(1), &Questions::default(), &mut fake).unwrap();
        back.save(&store).unwrap();
        assert_eq!(std::fs::metadata(&rows).unwrap().len() as usize, good.len() + back.dim * 4);
        assert_eq!(DenseIndex::load(&store).unwrap().ids, back.ids);
    }

    // The fake embeds the first three bytes, so a question row starting with "query: " lands far
    // from a passage row; a query shaped like the question reaches the node through the question
    // list alone, and once however many of its rows match.
    #[test]
    fn a_question_row_answers_for_its_node_once() {
        let mut idx = DenseIndex::default();
        let mut q = Questions::default();
        q.entries.insert("FR-PAY-22".into(), crate::enrich::Entry { hash: String::new(), questions: vec!["a".into(), "b".into()] });
        assert_eq!(idx.sync(&graph("x"), &q, &mut fake).unwrap(), 4);
        let probe = fake(&["query: z".to_string()]).unwrap().remove(0);
        let (passages, generated) = idx.search(&probe, 5);
        assert_eq!(generated, vec!["FR-PAY-22".to_string()]);
        assert_eq!(passages.len(), 2);
        // Dropping the questions re-embeds nothing and forgets the question rows.
        assert_eq!(idx.sync(&graph("x"), &Questions::default(), &mut fake).unwrap(), 0);
        assert_eq!(idx.ids.len(), 2);
    }

    // The probe is the stored question "a" itself; excluded by its hash, the node is reached
    // only through its other question row, and the passage list does not move.
    #[test]
    fn excluding_a_row_by_hash_leaves_the_node_to_its_other_rows() {
        let mut idx = DenseIndex::default();
        let mut q = Questions::default();
        q.entries.insert("FR-PAY-22".into(), crate::enrich::Entry { hash: String::new(), questions: vec!["a".into(), "bz".into()] });
        idx.sync(&graph("x"), &q, &mut fake).unwrap();
        let probe = fake(&["query: a".to_string()]).unwrap().remove(0);
        let own = blake3::hash(b"query: a").to_hex().to_string();
        let (with, _) = idx.search_scored(&probe, 5, None);
        let (passages, generated) = idx.search_scored(&probe, 5, Some(&own));
        assert_eq!(generated.iter().map(|(id, _)| id.as_str()).collect::<Vec<_>>(), vec!["FR-PAY-22"]);
        assert!(generated[0].1 < 1.0 - 1e-6, "the surviving row is not the probe itself");
        assert_eq!(passages.iter().map(|(id, _)| id.as_str()).collect::<Vec<_>>(), with.iter().map(|(id, _)| id.as_str()).collect::<Vec<_>>());
        q.entries.get_mut("FR-PAY-22").unwrap().questions.truncate(1);
        idx.sync(&graph("x"), &q, &mut fake).unwrap();
        assert!(idx.search_scored(&probe, 5, Some(&own)).1.is_empty());
    }

    #[test]
    fn round_trips_through_the_store() {
        let d = tempfile::tempdir().unwrap();
        let store = Store::new(d.path());
        let mut idx = synced(&graph("x"));
        idx.save(&store).unwrap();
        let back = DenseIndex::load(&store).unwrap();
        assert_eq!(back.ids, idx.ids);
        assert_eq!(back.vectors, idx.vectors);
        assert_eq!(back.dim, 3);
    }

    #[test]
    fn loading_with_nothing_on_disk_yields_a_default_index() {
        let d = tempfile::tempdir().unwrap();
        let idx = DenseIndex::load(&Store::new(d.path())).unwrap();
        assert!(idx.ids.is_empty() && idx.vectors.is_empty() && idx.dim == 0);
    }

    #[test]
    fn a_torn_pair_missing_its_vectors_file_is_treated_as_no_index() {
        let d = tempfile::tempdir().unwrap();
        let store = Store::new(d.path());
        let idx = synced(&graph("x"));
        // Only the metadata half is written, simulating a write interrupted between the two files.
        store.write_atomic("vectors.json", &serde_json::to_vec(&idx).unwrap()).unwrap();
        let back = DenseIndex::load(&store).unwrap();
        assert!(back.ids.is_empty() && back.vectors.is_empty());
    }

    #[test]
    fn present_requires_both_files_to_be_non_empty() {
        let d = tempfile::tempdir().unwrap();
        let store = Store::new(d.path());
        assert!(!DenseIndex::present(&store));
        let mut idx = synced(&graph("x"));
        idx.save(&store).unwrap();
        assert!(DenseIndex::present(&store));
    }

    #[test]
    fn a_query_of_the_wrong_dimension_yields_empty_lists_not_a_panic() {
        let idx = synced(&graph("x"));
        let (passages, generated) = idx.search(&[1.0, 2.0], 5);
        assert!(passages.is_empty() && generated.is_empty());
    }

    #[test]
    fn k_larger_than_the_collection_returns_every_row_without_padding() {
        let idx = synced(&graph("x"));
        let q = fake(&["штраф".to_string()]).unwrap().remove(0);
        let (passages, _) = idx.search(&q, 1000);
        assert_eq!(passages.len(), 2);
    }

    #[test]
    fn equal_cosine_scores_break_ties_by_id_ascending() {
        let mut g = Graph::default();
        let mut e = Extraction::default();
        // Same label and body embed to the exact same fake vector, so their cosine ties.
        e.node(NodeKind::Requirement, "FR-PAY-99", "same", "z", "a.md", 1);
        e.node(NodeKind::Requirement, "FR-PAY-11", "same", "z", "a.md", 2);
        g.apply(e);
        let idx = synced(&g);
        let q = fake(&["z".to_string()]).unwrap().remove(0);
        let (passages, _) = idx.search(&q, 5);
        assert_eq!(passages, vec!["FR-PAY-11".to_string(), "FR-PAY-99".to_string()]);
    }
}
