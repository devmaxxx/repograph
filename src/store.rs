use crate::model::Graph;
use crate::walk::Manifest;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

pub struct Store { dir: PathBuf }

impl Store {
    pub fn new(repo: &Path) -> Store { Store { dir: repo.join(".repograph") } }

    pub fn load(&self) -> Result<(Graph, Manifest)> {
        let read = |name: &str| -> Result<Option<String>> {
            let p = self.dir.join(name);
            if !p.exists() { return Ok(None); }
            Ok(Some(std::fs::read_to_string(&p).with_context(|| format!("read {}", p.display()))?))
        };
        let g = match read("graph.json")? { Some(t) => serde_json::from_str(&t).context("graph.json")?, None => Graph::default() };
        let m = match read("manifest.json")? { Some(t) => serde_json::from_str(&t).context("manifest.json")?, None => Manifest::default() };
        Ok((g, m))
    }

    pub fn save(&self, g: &Graph, m: &Manifest) -> Result<()> {
        std::fs::create_dir_all(&self.dir)?;
        self.write_atomic("graph.json", &serde_json::to_vec(g)?)?;
        self.save_manifest(m)
    }

    /// The manifest alone: a walk that found no change can still have learned the stat stamps
    /// that spare the next one from hashing the tree again.
    pub fn save_manifest(&self, m: &Manifest) -> Result<()> {
        self.write_atomic("manifest.json", &serde_json::to_vec(m)?)
    }

    pub fn write_atomic(&self, name: &str, bytes: &[u8]) -> Result<()> {
        std::fs::create_dir_all(&self.dir)?;
        let tmp = self.dir.join(format!("{name}.tmp"));
        std::fs::write(&tmp, bytes).with_context(|| format!("write {}", tmp.display()))?;
        std::fs::rename(&tmp, self.dir.join(name)).with_context(|| format!("rename {name}"))?;
        Ok(())
    }

    /// Whether a store file is there with something in it — a metadata read, so a caller can
    /// decide about a 50 MB file without loading it.
    pub fn has(&self, name: &str) -> bool {
        std::fs::metadata(self.dir.join(name)).map(|m| m.len() > 0).unwrap_or(false)
    }

    /// What a `stat` says about a store file, for a poller that wants to know whether someone
    /// else has written it without reading megabytes to find out.
    pub fn stamp(&self, name: &str) -> Option<crate::walk::Stamp> {
        crate::walk::stamp_of(&std::fs::metadata(self.dir.join(name)).ok()?)
    }

    pub fn read_bytes(&self, name: &str) -> Result<Option<Vec<u8>>> {
        let p = self.dir.join(name);
        if !p.exists() { return Ok(None); }
        Ok(Some(std::fs::read(&p)?))
    }

    /// Drops what `build` recomputes and nothing else: the dense vectors are reused by
    /// content hash, and the questions cost model tokens that a rebuild must not spend twice.
    pub fn wipe(&self) -> Result<()> {
        for name in ["graph.json", "manifest.json", "graph.json.tmp", "manifest.json.tmp"] {
            let p = self.dir.join(name);
            if p.exists() { std::fs::remove_file(&p).with_context(|| format!("remove {}", p.display()))?; }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{EdgeKind, Extraction, NodeKind};

    #[test]
    fn round_trip_and_atomic_write() {
        let d = tempfile::tempdir().unwrap();
        let store = Store::new(d.path());
        let mut g = Graph::default();
        let mut e = Extraction::default();
        e.node(NodeKind::File, "file:a.md", "a.md", "", "a.md", 1);
        e.edge("file:a.md", "INV-01", EdgeKind::References, "", "a.md");
        g.apply(e);
        let m = Manifest::default();
        store.save(&g, &m).unwrap();
        assert!(d.path().join(".repograph/graph.json").exists());
        assert!(!d.path().join(".repograph/graph.json.tmp").exists());
        let (g2, _) = store.load().unwrap();
        assert_eq!(g2.nodes.len(), 1);
        assert_eq!(g2.edges.len(), 1);
    }

    #[test]
    fn corrupt_graph_json_is_an_error_naming_the_file_not_a_panic() {
        let d = tempfile::tempdir().unwrap();
        let store = Store::new(d.path());
        store.save(&Graph::default(), &Manifest::default()).unwrap();
        std::fs::write(d.path().join(".repograph/graph.json"), "{ not json").unwrap();
        let err = store.load().unwrap_err().to_string();
        assert!(err.contains("graph.json"), "{err}");
    }

    #[test]
    fn a_leftover_tmp_from_a_crashed_write_is_overwritten_not_read() {
        let d = tempfile::tempdir().unwrap();
        let store = Store::new(d.path());
        std::fs::create_dir_all(d.path().join(".repograph")).unwrap();
        std::fs::write(d.path().join(".repograph/graph.json.tmp"), "garbage").unwrap();
        store.save(&Graph::default(), &Manifest::default()).unwrap();
        let (g, _) = store.load().unwrap();
        assert!(g.nodes.is_empty());
        assert!(!d.path().join(".repograph/graph.json.tmp").exists());
    }

    // Bytes reach the store only through a rename, so a write that fails part-way — here the
    // temp path is occupied by a directory — cannot leave a half-written graph behind.
    #[test]
    fn a_failed_write_leaves_the_stored_graph_intact() {
        let d = tempfile::tempdir().unwrap();
        let store = Store::new(d.path());
        let mut g = Graph::default();
        let mut e = Extraction::default();
        e.node(NodeKind::File, "file:a.md", "a.md", "", "a.md", 1);
        g.apply(e);
        store.save(&g, &Manifest::default()).unwrap();
        std::fs::create_dir(d.path().join(".repograph/graph.json.tmp")).unwrap();
        assert!(store.save(&Graph::default(), &Manifest::default()).is_err());
        assert_eq!(store.load().unwrap().0.nodes.len(), 1);
    }

    #[test]
    fn has_means_present_and_non_empty() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path());
        assert!(!store.has("vectors.f32"));
        store.write_atomic("vectors.f32", b"").unwrap();
        assert!(!store.has("vectors.f32"));
        store.write_atomic("vectors.f32", &[0u8; 8]).unwrap();
        assert!(store.has("vectors.f32"));
    }

    #[test]
    fn wipe_keeps_the_vectors_and_the_questions() {
        let d = tempfile::tempdir().unwrap();
        let s = Store::new(d.path());
        s.save(&Graph::default(), &Manifest::default()).unwrap();
        s.write_atomic("vectors.f32", b"v").unwrap();
        s.write_atomic("questions.json", b"{}").unwrap();
        s.wipe().unwrap();
        assert!(!d.path().join(".repograph/graph.json").exists());
        assert!(!d.path().join(".repograph/manifest.json").exists());
        assert_eq!(s.read_bytes("vectors.f32").unwrap().as_deref(), Some(&b"v"[..]));
        assert_eq!(s.read_bytes("questions.json").unwrap().as_deref(), Some(&b"{}"[..]));
        s.wipe().unwrap();
    }

    #[test]
    fn load_absent_is_empty() {
        let d = tempfile::tempdir().unwrap();
        let (g, m) = Store::new(d.path()).load().unwrap();
        assert!(g.nodes.is_empty() && m.files.is_empty());
    }
}
