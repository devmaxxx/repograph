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

/// The generation of the grammar a file is read by — the id shapes, the definition heads, the
/// registry rows, the ids a source file cites, and which of those a settled graph admits. Bumped
/// by hand when a change to any of them would make a re-read of a file that has not moved yield a
/// different graph, and left alone by a release that does not touch them: this number is what
/// forces one whole-tree re-read on the first writer after an upgrade (see `apply_diff`), and a
/// bump nobody needed is that walk paid for nothing. `0` belongs to no generation: it is what a
/// manifest written before the stamp existed reads as, and what a writer leaves behind when a file
/// it had to read would not open — neither store was read whole, and `0` is stale against every
/// generation there is or will be.
pub const GRAMMAR: u32 = 1;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Manifest {
    pub files: BTreeMap<String, String>,
    /// A manifest written before the stat cache existed has none; every file is hashed once more
    /// and the stamps are there from the next save on.
    #[serde(default)] pub stamps: BTreeMap<String, Stamp>,
    /// The grammar generation the graph saved beside this manifest was read whole by, or `0` for
    /// none. It belongs here and not on the graph because it answers the same question the hashes
    /// and stamps do — must this file be read again — and every writer holds the manifest at the
    /// moment it asks.
    #[serde(default)] pub grammar: u32,
}

#[derive(Debug, Default)]
pub struct Diff { pub changed: Vec<Entry>, pub removed: Vec<String> }

pub(crate) fn globs(globs: &[String]) -> Result<GlobSet> {
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
    let docs = globs(&cfg.doc_globs)?;
    let code = globs(&cfg.code_globs)?;
    let skip = globs(&cfg.skip)?;
    let registries = globs(&cfg.registries)?;
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
            grammar: GRAMMAR,
        }
    }

    /// Whether the graph beside this manifest was read by a grammar other than this build's — a
    /// store every hash calls current and that is still short of what a re-read would find, or
    /// long by what a newer build put in it. Different, not older: either way it is not the store
    /// this build would have written, and one walk is what makes it one.
    pub fn stale_grammar(&self) -> bool {
        self.grammar != GRAMMAR
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
            grammar: GRAMMAR,
        }).unwrap()).unwrap();
        assert!(old.stamps.is_empty());
        let again = walk(d.path(), &Config::default(), &old).unwrap();
        assert_eq!(again, hashed);
        assert_eq!(Manifest::from_entries(&again).stamps.len(), hashed.len());
    }

    // A manifest from a release that stamped no grammar is behind whatever this build reads by,
    // and it is the only thing that says so: every hash in it still matches the tree.
    #[test]
    fn a_manifest_without_a_grammar_stamp_reads_as_behind_this_one() {
        let old: Manifest = serde_json::from_str("{\"files\":{}}").unwrap();
        assert_eq!(old.grammar, 0);
        assert!(old.stale_grammar());
        assert!(!Manifest::from_entries(&[]).stale_grammar());
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

    fn init(d: &Path) {
        std::fs::create_dir_all(d.join(".git")).unwrap();
    }

    #[test]
    fn skip_globs_win_over_doc_globs() {
        let d = tempfile::tempdir().unwrap();
        init(d.path());
        std::fs::create_dir_all(d.path().join("docs")).unwrap();
        // TRACKER.md matches the default doc glob "**/*.md" as much as it matches the skip glob.
        std::fs::write(d.path().join("docs/TRACKER.md"), "generated\n").unwrap();
        let entries = walk(d.path(), &Config::default(), &Manifest::default()).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn registry_beats_doc_for_the_same_path() {
        let d = tempfile::tempdir().unwrap();
        init(d.path());
        std::fs::create_dir_all(d.path().join("docs")).unwrap();
        std::fs::write(d.path().join("docs/constitution.yaml"), "version: 1\n").unwrap();
        let mut cfg = Config::default();
        cfg.doc_globs.push("**/*.yaml".to_string()); // now overlaps the registry glob for the same path
        let entries = walk(d.path(), &cfg, &Manifest::default()).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].kind, FileKind::Registry);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn a_non_utf8_file_name_is_skipped_without_failing() {
        use std::os::unix::ffi::OsStrExt;
        let d = tempfile::tempdir().unwrap();
        init(d.path());
        std::fs::write(d.path().join(std::ffi::OsStr::from_bytes(b"bad_\xFF\xFE.md")), "x\n").unwrap();
        std::fs::write(d.path().join("good.md"), "y\n").unwrap();
        let entries = walk(d.path(), &Config::default(), &Manifest::default()).unwrap();
        assert_eq!(entries.iter().map(|e| e.rel.as_str()).collect::<Vec<_>>(), vec!["good.md"]);
    }

    #[test]
    fn a_gitignored_file_is_not_walked() {
        let d = tempfile::tempdir().unwrap();
        init(d.path());
        std::fs::write(d.path().join(".gitignore"), "secret.md\n").unwrap();
        std::fs::write(d.path().join("secret.md"), "x\n").unwrap();
        std::fs::write(d.path().join("public.md"), "y\n").unwrap();
        let entries = walk(d.path(), &Config::default(), &Manifest::default()).unwrap();
        assert_eq!(entries.iter().map(|e| e.rel.as_str()).collect::<Vec<_>>(), vec!["public.md"]);
    }

    #[test]
    fn diff_puts_a_brand_new_file_in_changed_not_a_separate_list() {
        // `Diff` has no third "added" list: a file `self.files` has never seen fails the same
        // hash comparison as a changed one, so it surfaces through `changed` too.
        let d = repo();
        let before = walk(d.path(), &Config::default(), &Manifest::default()).unwrap();
        let manifest = Manifest::from_entries(&before);
        std::fs::write(d.path().join("docs/new.md"), "# new\n").unwrap();
        let after = walk(d.path(), &Config::default(), &manifest).unwrap();
        let diff = manifest.diff(&after);
        assert_eq!(diff.changed.iter().map(|e| e.rel.as_str()).collect::<Vec<_>>(), vec!["docs/new.md"]);
        assert!(diff.removed.is_empty());
    }

    #[test]
    fn diff_lists_a_removed_file_only_in_removed_not_changed() {
        let d = repo();
        let before = walk(d.path(), &Config::default(), &Manifest::default()).unwrap();
        let manifest = Manifest::from_entries(&before);
        std::fs::remove_file(d.path().join("docs/a.md")).unwrap();
        let after = walk(d.path(), &Config::default(), &manifest).unwrap();
        let diff = manifest.diff(&after);
        assert_eq!(diff.removed, vec!["docs/a.md".to_string()]);
        assert!(diff.changed.is_empty());
    }

    #[test]
    fn a_file_matching_no_configured_glob_is_silently_excluded() {
        let d = tempfile::tempdir().unwrap();
        init(d.path());
        std::fs::write(d.path().join("data.json"), "{}\n").unwrap();
        let entries = walk(d.path(), &Config::default(), &Manifest::default()).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn sorted_output_order_with_cyrillic_file_names() {
        let d = tempfile::tempdir().unwrap();
        init(d.path());
        std::fs::write(d.path().join("яблоко.md"), "x\n").unwrap();
        std::fs::write(d.path().join("абрикос.md"), "y\n").unwrap();
        std::fs::write(d.path().join("a.md"), "z\n").unwrap();
        let entries = walk(d.path(), &Config::default(), &Manifest::default()).unwrap();
        assert_eq!(entries.iter().map(|e| e.rel.as_str()).collect::<Vec<_>>(), vec!["a.md", "абрикос.md", "яблоко.md"]);
    }
}
