use crate::model::{Graph, NodeKind};
use crate::store::Store;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct DenseIndex {
    pub ids: Vec<String>,
    pub hashes: Vec<String>,
    pub dim: usize,
    #[serde(skip)] pub vectors: Vec<f32>,
}

pub struct Embedder { model: fastembed::TextEmbedding }

fn passage(n: &crate::model::Node) -> String { format!("{}\n{}", n.label, n.body) }

fn normalise(v: &mut [f32]) {
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 { for x in v { *x /= norm; } }
}

impl DenseIndex {
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
    pub fn sync(&mut self, graph: &Graph, embed: &mut dyn FnMut(&[String]) -> Result<Vec<Vec<f32>>>) -> Result<usize> {
        let mut keep_ids = Vec::new();
        let mut keep_hashes = Vec::new();
        let mut keep_vecs: Vec<f32> = Vec::new();
        let mut todo_ids = Vec::new();
        let mut todo_texts = Vec::new();
        let mut todo_hashes = Vec::new();
        let old: std::collections::HashMap<&str, (usize, &str)> =
            self.ids.iter().enumerate().map(|(i, id)| (id.as_str(), (i, self.hashes[i].as_str()))).collect();
        for n in graph.nodes.values().filter(|n| n.kind != NodeKind::File) {
            let text = passage(n);
            let hash = blake3::hash(text.as_bytes()).to_hex().to_string();
            match old.get(n.id.as_str()) {
                Some((i, h)) if *h == hash && self.dim > 0 => {
                    keep_ids.push(n.id.clone());
                    keep_hashes.push(hash);
                    keep_vecs.extend_from_slice(&self.vectors[i * self.dim..(i + 1) * self.dim]);
                }
                _ => { todo_ids.push(n.id.clone()); todo_texts.push(text); todo_hashes.push(hash); }
            }
        }
        let embedded = todo_ids.len();
        if embedded > 0 {
            let mut vecs = embed(&todo_texts)?;
            for v in vecs.iter_mut() { normalise(v); }
            self.dim = vecs.first().map(|v| v.len()).unwrap_or(self.dim);
            for (id, (hash, v)) in todo_ids.into_iter().zip(todo_hashes.into_iter().zip(vecs)) {
                keep_ids.push(id);
                keep_hashes.push(hash);
                keep_vecs.extend_from_slice(&v);
            }
        }
        self.ids = keep_ids;
        self.hashes = keep_hashes;
        self.vectors = keep_vecs;
        if self.ids.is_empty() { self.dim = 0; }
        Ok(embedded)
    }

    pub fn search(&self, query: &[f32], k: usize) -> Vec<String> {
        if self.dim == 0 || query.len() != self.dim { return Vec::new(); }
        let mut q = query.to_vec();
        normalise(&mut q);
        let mut scored: Vec<(f32, &str)> = self.ids.iter().enumerate().map(|(i, id)| {
            let v = &self.vectors[i * self.dim..(i + 1) * self.dim];
            (v.iter().zip(&q).map(|(a, b)| a * b).sum::<f32>(), id.as_str())
        }).collect();
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap().then(a.1.cmp(b.1)));
        scored.into_iter().take(k).map(|(_, id)| id.to_string()).collect()
    }
}

/// fastembed's own default is `.fastembed_cache` under the current directory, which re-downloads
/// 470 MB per directory `repograph` is run from and fails outright on a read-only one.
fn cache_dir() -> Result<PathBuf> {
    if let Some(dir) = std::env::var_os("FASTEMBED_CACHE_DIR") {
        return Ok(PathBuf::from(dir));
    }
    let home = std::env::var_os("HOME").context("neither FASTEMBED_CACHE_DIR nor HOME is set")?;
    Ok(PathBuf::from(home).join(".cache").join("repograph").join("fastembed"))
}

impl Embedder {
    pub fn open() -> Result<Embedder> {
        use fastembed::{EmbeddingModel, TextEmbedding, TextInitOptions};
        let opts = TextInitOptions::new(EmbeddingModel::MultilingualE5Small)
            .with_cache_dir(cache_dir()?)
            .with_show_download_progress(true)
            .with_max_length(256);
        Ok(Embedder { model: TextEmbedding::try_new(opts).context("open embedding model")? })
    }

    pub fn passages(&mut self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        let prefixed: Vec<String> = texts.iter().map(|t| format!("passage: {t}")).collect();
        Ok(self.model.embed(&prefixed, Some(64))?)
    }

    pub fn query(&mut self, text: &str) -> Result<Vec<f32>> {
        Ok(self.model.embed(&[format!("query: {text}")], None)?.remove(0))
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

    /// A stand-in embedder: a 3-d vector from the first three bytes, so tests are deterministic.
    fn fake(texts: &[String]) -> Result<Vec<Vec<f32>>> {
        Ok(texts.iter().map(|t| {
            let b = t.as_bytes();
            vec![b[0] as f32, b.get(1).copied().unwrap_or(0) as f32, b.get(2).copied().unwrap_or(0) as f32]
        }).collect())
    }

    #[test]
    fn sync_embeds_only_changed_nodes_and_drops_removed_ones() {
        let mut idx = DenseIndex::default();
        assert_eq!(idx.sync(&graph("политика"), &mut fake).unwrap(), 2);
        assert_eq!(idx.ids.len(), 2);
        assert_eq!(idx.sync(&graph("политика"), &mut fake).unwrap(), 0);
        assert_eq!(idx.sync(&graph("другое"), &mut fake).unwrap(), 1);
        let mut g = graph("другое");
        g.remove_file("a.md");
        assert_eq!(idx.sync(&g, &mut fake).unwrap(), 0);
        assert!(idx.ids.is_empty() && idx.vectors.is_empty());
    }

    #[test]
    fn search_is_cosine_descending() {
        let mut idx = DenseIndex::default();
        idx.sync(&graph("x"), &mut fake).unwrap();
        let q = fake(&["штраф".to_string()]).unwrap().remove(0);
        assert_eq!(idx.search(&q, 2)[0], "FR-PAY-26");
    }

    #[test]
    fn round_trips_through_the_store() {
        let d = tempfile::tempdir().unwrap();
        let store = Store::new(d.path());
        let mut idx = DenseIndex::default();
        idx.sync(&graph("x"), &mut fake).unwrap();
        idx.save(&store).unwrap();
        let back = DenseIndex::load(&store).unwrap();
        assert_eq!(back.ids, idx.ids);
        assert_eq!(back.vectors, idx.vectors);
        assert_eq!(back.dim, 3);
    }
}
