use crate::enrich::Questions;
use crate::model::{Graph, NodeKind};
use crate::store::Store;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Node ids best first, each with the cosine of its best row.
pub type Scored = Vec<(String, f32)>;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct DenseIndex {
    pub ids: Vec<String>,
    pub hashes: Vec<String>,
    /// True for a generated-question row; stores written before enrichment existed have none.
    #[serde(default)] pub kinds: Vec<bool>,
    pub dim: usize,
    #[serde(skip)] pub vectors: Vec<f32>,
}

/// Every text embedded for a node, e5-prefixed. The passage is the node itself; each generated
/// question is embedded as a query, since the reader's question is one too (e5's symmetric case).
fn rows(n: &crate::model::Node, questions: &Questions) -> Vec<String> {
    let mut out = vec![format!("passage: {}\n{}", n.label, n.body)];
    out.extend(questions.get(&n.id).iter().map(|q| format!("query: {q}")));
    out
}
fn normalise(v: &mut [f32]) {
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 { for x in v { *x /= norm; } }
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
        let raw = store.read_bytes("vectors.f32")?.unwrap_or_default();
        #[allow(clippy::chunks_exact_to_as_chunks)]
        { idx.vectors = raw.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect(); }
        if idx.vectors.len() != idx.ids.len() * idx.dim {
            // A torn pair of files is treated as no index at all; the next sync rebuilds it.
            return Ok(DenseIndex::default());
        }
        Ok(idx)
    }

    pub fn save(&self, store: &Store) -> Result<()> {
        let mut raw = Vec::with_capacity(self.vectors.len() * 4);
        for x in &self.vectors { raw.extend_from_slice(&x.to_le_bytes()); }
        store.write_atomic("vectors.f32", &raw)?;
        store.write_atomic("vectors.json", &serde_json::to_vec(self)?)
    }

    #[allow(clippy::type_complexity)]
    pub fn sync(&mut self, graph: &Graph, questions: &Questions, embed: &mut dyn FnMut(&[String]) -> Result<Vec<Vec<f32>>>) -> Result<usize> {
        let mut keep_ids = Vec::new();
        let mut keep_hashes = Vec::new();
        let mut keep_kinds = Vec::new();
        let mut todo_kinds = Vec::new();
        let mut keep_vecs: Vec<f32> = Vec::new();
        let mut todo_ids = Vec::new();
        let mut todo_texts = Vec::new();
        let mut todo_hashes = Vec::new();
        let old: std::collections::HashMap<(&str, &str), usize> =
            self.ids.iter().enumerate().map(|(i, id)| ((id.as_str(), self.hashes[i].as_str()), i)).collect();
        for n in graph.nodes.values().filter(|n| n.kind != NodeKind::File) {
            for text in rows(n, questions) {
                let hash = blake3::hash(text.as_bytes()).to_hex().to_string();
                let is_q = text.starts_with("query: ");
                match old.get(&(n.id.as_str(), hash.as_str())).copied() {
                    Some(i) if self.dim > 0 => {
                        keep_ids.push(n.id.clone());
                        keep_hashes.push(hash);
                        keep_kinds.push(is_q);
                        keep_vecs.extend_from_slice(&self.vectors[i * self.dim..(i + 1) * self.dim]);
                    }
                    _ => { todo_ids.push(n.id.clone()); todo_texts.push(text); todo_hashes.push(hash); todo_kinds.push(is_q); }
                }
            }
        }
        let embedded = todo_ids.len();
        if embedded > 0 {
            let mut vecs = embed(&todo_texts)?;
            for v in vecs.iter_mut() { normalise(v); }
            self.dim = vecs.first().map(|v| v.len()).unwrap_or(self.dim);
            for ((id, (hash, v)), is_q) in todo_ids.into_iter().zip(todo_hashes.into_iter().zip(vecs)).zip(todo_kinds) {
                keep_ids.push(id);
                keep_hashes.push(hash);
                keep_kinds.push(is_q);
                keep_vecs.extend_from_slice(&v);
            }
        }
        self.ids = keep_ids;
        self.hashes = keep_hashes;
        self.kinds = keep_kinds;
        self.vectors = keep_vecs;
        if self.ids.is_empty() { self.dim = 0; }
        Ok(embedded)
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
        let mut rows: Vec<(f32, &str, bool)> = self.ids.iter().enumerate()
            .filter(|(i, _)| exclude.is_none_or(|h| self.hashes.get(*i).is_none_or(|x| x != h)))
            .map(|(i, id)| {
                let v = &self.vectors[i * self.dim..(i + 1) * self.dim];
                (v.iter().zip(&q).map(|(a, b)| a * b).sum::<f32>(), id.as_str(), self.kinds.get(i).copied().unwrap_or(false))
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

    /// A stand-in embedder: a 3-d vector from the first three bytes after the e5 prefix, so
    /// tests are deterministic.
    fn fake(texts: &[String]) -> Result<Vec<Vec<f32>>> {
        Ok(texts.iter().map(|t| {
            let b = t.split_once(": ").map(|x| x.1).unwrap_or(t).as_bytes();
            vec![b[0] as f32, b.get(1).copied().unwrap_or(0) as f32, b.get(2).copied().unwrap_or(0) as f32]
        }).collect())
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
    fn search_is_cosine_descending() {
        let mut idx = DenseIndex::default();
        idx.sync(&graph("x"), &Questions::default(), &mut fake).unwrap();
        let q = fake(&["штраф".to_string()]).unwrap().remove(0);
        assert_eq!(idx.search(&q, 2).0[0], "FR-PAY-26");
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
        let mut idx = DenseIndex::default();
        idx.sync(&graph("x"), &Questions::default(), &mut fake).unwrap();
        idx.save(&store).unwrap();
        let back = DenseIndex::load(&store).unwrap();
        assert_eq!(back.ids, idx.ids);
        assert_eq!(back.vectors, idx.vectors);
        assert_eq!(back.dim, 3);
    }
}
