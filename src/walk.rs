use crate::config::Config;
use anyhow::{Context, Result};
use globset::{Glob, GlobSet, GlobSetBuilder};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileKind { Doc, Code, Registry }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry { pub rel: String, pub kind: FileKind, pub hash: String }

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Manifest { pub files: BTreeMap<String, String> }

#[derive(Debug, Default)]
pub struct Diff { pub changed: Vec<Entry>, pub removed: Vec<String> }

fn set(globs: &[String]) -> Result<GlobSet> {
    let mut b = GlobSetBuilder::new();
    for g in globs {
        b.add(Glob::new(g).with_context(|| format!("glob {g}"))?);
    }
    Ok(b.build()?)
}

pub fn walk(repo: &Path, cfg: &Config) -> Result<Vec<Entry>> {
    let docs = set(&cfg.doc_globs)?;
    let code = set(&cfg.code_globs)?;
    let skip = set(&cfg.skip)?;
    let registries = set(&cfg.registries)?;
    let mut out = Vec::new();
    for dent in ignore::WalkBuilder::new(repo).hidden(true).git_ignore(true).build() {
        let dent = match dent {
            Ok(d) => d,
            Err(e) => { eprintln!("walk: {e}"); continue; }
        };
        if !dent.file_type().map(|t| t.is_file()).unwrap_or(false) {
            continue;
        }
        let rel_path = dent.path().strip_prefix(repo).unwrap_or(dent.path());
        let Some(rel) = rel_path.to_str().map(|s| s.replace('\\', "/")) else {
            eprintln!("walk: skipping non-UTF-8 path {}", rel_path.display());
            continue;
        };
        if skip.is_match(&rel) {
            continue;
        }
        let kind = if registries.is_match(&rel) { FileKind::Registry }
            else if docs.is_match(&rel) { FileKind::Doc }
            else if code.is_match(&rel) { FileKind::Code }
            else { continue };
        let bytes = match std::fs::read(dent.path()) {
            Ok(b) => b,
            Err(e) => { eprintln!("walk: skipping {rel}: {e}"); continue; }
        };
        out.push(Entry { rel, kind, hash: blake3::hash(&bytes).to_hex().to_string() });
    }
    out.sort_by(|a, b| a.rel.cmp(&b.rel));
    Ok(out)
}

impl Manifest {
    pub fn from_entries(entries: &[Entry]) -> Manifest {
        Manifest { files: entries.iter().map(|e| (e.rel.clone(), e.hash.clone())).collect() }
    }

    pub fn diff(&self, now: &[Entry]) -> Diff {
        let mut d = Diff::default();
        for e in now {
            if self.files.get(&e.rel) != Some(&e.hash) {
                d.changed.push(e.clone());
            }
        }
        let present: std::collections::BTreeSet<&str> = now.iter().map(|e| e.rel.as_str()).collect();
        d.removed = self.files.keys().filter(|k| !present.contains(k.as_str())).cloned().collect();
        d
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo() -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(d.path().join(".git")).unwrap();
        std::fs::create_dir_all(d.path().join("docs")).unwrap();
        std::fs::create_dir_all(d.path().join("node_modules/x")).unwrap();
        std::fs::write(d.path().join("docs/a.md"), "# a\n").unwrap();
        std::fs::write(d.path().join("docs/TRACKER.md"), "generated\n").unwrap();
        std::fs::write(d.path().join("docs/constitution.yaml"), "version: 1\n").unwrap();
        std::fs::write(d.path().join("b.ts"), "export const b = 1;\n").unwrap();
        std::fs::write(d.path().join("node_modules/x/c.ts"), "export const c = 1;\n").unwrap();
        std::fs::write(d.path().join(".gitignore"), "ignored.md\n").unwrap();
        std::fs::write(d.path().join("ignored.md"), "# hidden\n").unwrap();
        d
    }

    #[test]
    fn walk_classifies_and_skips() {
        let d = repo();
        let entries = walk(d.path(), &Config::default()).unwrap();
        let rels: Vec<_> = entries.iter().map(|e| (e.rel.as_str(), e.kind)).collect();
        assert_eq!(rels, vec![
            ("b.ts", FileKind::Code),
            ("docs/a.md", FileKind::Doc),
            ("docs/constitution.yaml", FileKind::Registry),
        ]);
        assert_eq!(entries[0].hash, blake3::hash(b"export const b = 1;\n").to_hex().to_string());
    }

    #[test]
    fn diff_reports_changed_and_removed() {
        let d = repo();
        let before = walk(d.path(), &Config::default()).unwrap();
        let manifest = Manifest::from_entries(&before);
        std::fs::write(d.path().join("docs/a.md"), "# a changed\n").unwrap();
        std::fs::remove_file(d.path().join("b.ts")).unwrap();
        let after = walk(d.path(), &Config::default()).unwrap();
        let diff = manifest.diff(&after);
        assert_eq!(diff.changed.iter().map(|e| e.rel.as_str()).collect::<Vec<_>>(), vec!["docs/a.md"]);
        assert_eq!(diff.removed, vec!["b.ts".to_string()]);
    }

    #[test]
    fn unchanged_tree_diffs_empty() {
        let d = repo();
        let before = walk(d.path(), &Config::default()).unwrap();
        let diff = Manifest::from_entries(&before).diff(&before);
        assert!(diff.changed.is_empty() && diff.removed.is_empty());
    }
}
