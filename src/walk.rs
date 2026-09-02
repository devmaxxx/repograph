use crate::config::Config;
use anyhow::{Context, Result};
use globset::{Glob, GlobSet, GlobSetBuilder};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileKind { Doc, Code, Registry }

/// What a `stat` says about a file: enough to decide that re-reading it would be wasted work.
/// The blake3 hash stays the truth — a stamp only ever skips recomputing one, never declares a
/// file changed, so a touched-but-identical file is still hashed and still diffs as unchanged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stamp { pub mtime_ns: u64, pub len: u64 }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry { pub rel: String, pub kind: FileKind, pub hash: String, pub stamp: Option<Stamp> }

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Manifest {
    pub files: BTreeMap<String, String>,
    /// A manifest written before the stat cache existed has none; every file is hashed once more
    /// and the stamps are there from the next save on.
    #[serde(default)] pub stamps: BTreeMap<String, Stamp>,
}

#[derive(Debug, Default)]
pub struct Diff { pub changed: Vec<Entry>, pub removed: Vec<String> }

fn set(globs: &[String]) -> Result<GlobSet> {
    let mut b = GlobSetBuilder::new();
    for g in globs {
        b.add(Glob::new(g).with_context(|| format!("glob {g}"))?);
    }
    Ok(b.build()?)
}

pub(crate) fn stamp_of(meta: &std::fs::Metadata) -> Option<Stamp> {
    let ns = meta.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?.as_nanos();
    Some(Stamp { mtime_ns: u64::try_from(ns).ok()?, len: meta.len() })
}

/// `prev` is the manifest of the last walk; a file whose stamp still matches keeps its recorded
/// hash instead of being read. Pass `&Manifest::default()` to hash everything.
pub fn walk(repo: &Path, cfg: &Config, prev: &Manifest) -> Result<Vec<Entry>> {
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
        // Stamped before the read, never after: a write racing this walk then leaves a stamp
        // older than the file, and the next walk hashes it again rather than trusting the hash.
        let stamp = dent.metadata().ok().as_ref().and_then(stamp_of);
        if let (Some(s), Some(hash)) = (stamp, prev.files.get(&rel)) {
            if prev.stamps.get(&rel) == Some(&s) {
                out.push(Entry { rel, kind, hash: hash.clone(), stamp });
                continue;
            }
        }
        let bytes = match std::fs::read(dent.path()) {
            Ok(b) => b,
            Err(e) => { eprintln!("walk: skipping {rel}: {e}"); continue; }
        };
        out.push(Entry { rel, kind, hash: blake3::hash(&bytes).to_hex().to_string(), stamp });
    }
    out.sort_by(|a, b| a.rel.cmp(&b.rel));
    Ok(out)
}

impl Manifest {
    pub fn from_entries(entries: &[Entry]) -> Manifest {
        Manifest {
            files: entries.iter().map(|e| (e.rel.clone(), e.hash.clone())).collect(),
            stamps: entries.iter().filter_map(|e| Some((e.rel.clone(), e.stamp?))).collect(),
        }
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
        let entries = walk(d.path(), &Config::default(), &Manifest::default()).unwrap();
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
        let before = walk(d.path(), &Config::default(), &Manifest::default()).unwrap();
        let manifest = Manifest::from_entries(&before);
        std::fs::write(d.path().join("docs/a.md"), "# a changed\n").unwrap();
        std::fs::remove_file(d.path().join("b.ts")).unwrap();
        let after = walk(d.path(), &Config::default(), &manifest).unwrap();
        let diff = manifest.diff(&after);
        assert_eq!(diff.changed.iter().map(|e| e.rel.as_str()).collect::<Vec<_>>(), vec!["docs/a.md"]);
        assert_eq!(diff.removed, vec!["b.ts".to_string()]);
    }

    #[test]
    fn unchanged_tree_diffs_empty() {
        let d = repo();
        let before = walk(d.path(), &Config::default(), &Manifest::default()).unwrap();
        let diff = Manifest::from_entries(&before).diff(&before);
        assert!(diff.changed.is_empty() && diff.removed.is_empty());
    }

    // Stores written before the stat cache have no stamps; they must load and cost one more
    // hashing pass, after which the walk is stamp-driven again.
    #[test]
    fn a_manifest_without_stamps_loads_and_rehashes_once() {
        let d = repo();
        let hashed = walk(d.path(), &Config::default(), &Manifest::default()).unwrap();
        let old: Manifest = serde_json::from_str(&serde_json::to_string(&Manifest {
            files: Manifest::from_entries(&hashed).files,
            stamps: BTreeMap::new(),
        }).unwrap()).unwrap();
        assert!(old.stamps.is_empty());
        let again = walk(d.path(), &Config::default(), &old).unwrap();
        assert_eq!(again, hashed);
        assert_eq!(Manifest::from_entries(&again).stamps.len(), hashed.len());
    }

    // The stamp is what decides whether a file is read at all: with the recorded mtime and length
    // put back, the walk hands back the hash it stored rather than the one on disk.
    #[test]
    fn a_file_whose_stamp_is_unchanged_is_not_read_again() {
        let d = repo();
        let before = walk(d.path(), &Config::default(), &Manifest::default()).unwrap();
        let manifest = Manifest::from_entries(&before);
        let path = d.path().join("docs/a.md");
        let was = std::fs::metadata(&path).unwrap().modified().unwrap();
        std::fs::write(&path, "# A\n").unwrap();
        std::fs::File::options().write(true).open(&path).unwrap()
            .set_times(std::fs::FileTimes::new().set_modified(was)).unwrap();
        let after = walk(d.path(), &Config::default(), &manifest).unwrap();
        assert!(manifest.diff(&after).changed.is_empty());
    }

    // A rewrite with identical bytes moves the mtime, so the file is hashed again — and the hash
    // is what the diff answers on, so nothing is reported as changed.
    #[test]
    fn a_touched_but_identical_file_is_not_reported_as_changed() {
        let d = repo();
        let before = walk(d.path(), &Config::default(), &Manifest::default()).unwrap();
        let manifest = Manifest::from_entries(&before);
        std::fs::write(d.path().join("docs/a.md"), "# a\n").unwrap();
        let after = walk(d.path(), &Config::default(), &manifest).unwrap();
        assert_ne!(after.iter().find(|e| e.rel == "docs/a.md").unwrap().stamp, manifest.stamps.get("docs/a.md").copied());
        let diff = manifest.diff(&after);
        assert!(diff.changed.is_empty() && diff.removed.is_empty());
    }
}
