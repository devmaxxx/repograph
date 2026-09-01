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
        self.write_atomic("manifest.json", &serde_json::to_vec(m)?)
    }

    pub fn write_atomic(&self, name: &str, bytes: &[u8]) -> Result<()> {
        std::fs::create_dir_all(&self.dir)?;
        let tmp = self.dir.join(format!("{name}.tmp"));
        std::fs::write(&tmp, bytes).with_context(|| format!("write {}", tmp.display()))?;
        std::fs::rename(&tmp, self.dir.join(name)).with_context(|| format!("rename {name}"))?;
        Ok(())
    }

    pub fn read_bytes(&self, name: &str) -> Result<Option<Vec<u8>>> {
        let p = self.dir.join(name);
        if !p.exists() { return Ok(None); }
        Ok(Some(std::fs::read(&p)?))
    }

    pub fn wipe(&self) -> Result<()> {
        if self.dir.exists() { std::fs::remove_dir_all(&self.dir)?; }
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
    fn load_absent_is_empty() {
        let d = tempfile::tempdir().unwrap();
        let (g, m) = Store::new(d.path()).load().unwrap();
        assert!(g.nodes.is_empty() && m.files.is_empty());
    }
}
