use crate::model::Graph;
use crate::walk::Manifest;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

pub struct Store { dir: PathBuf }

/// Where a load got its value. The JSON is what a store is defined by and what any other
/// binary reads; the mirror is a postcard copy of the same value written beside it, because
/// parsing 10 MB of graph JSON costs every `ask` 18 ms and decoding the same graph a few.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source { Absent, Json, Mirror }

/// A `Graph` that gained a field is a new mirror shape, and a magic that says so is one fewer
/// thing that depends on postcard failing at the right byte: `RGM1` mirrors hold the struct from
/// before `pending`, and are passed over rather than decoded into it.
const MIRROR_MAGIC: &[u8; 4] = b"RGM2";
const MIRROR_HEADER: usize = 4 + 12 + 16 + 8;

fn stamp(p: &Path) -> Option<(u128, u64)> {
    let m = std::fs::metadata(p).ok()?;
    let mtime = m.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?.as_nanos();
    Some((mtime, m.len()))
}

/// What a mirror was written from. It is trusted only while the JSON's stat stamp and the
/// release that wrote it both still match: postcard is not self-describing, so a struct that
/// changed shape between releases would otherwise decode into a wrong graph without one error.
fn mirror_header(stamp: (u128, u64)) -> [u8; MIRROR_HEADER] {
    let mut h = [0u8; MIRROR_HEADER];
    h[..4].copy_from_slice(MIRROR_MAGIC);
    let v = env!("CARGO_PKG_VERSION").as_bytes();
    let n = v.len().min(12);
    h[4..4 + n].copy_from_slice(&v[..n]);
    h[16..32].copy_from_slice(&stamp.0.to_le_bytes());
    h[32..40].copy_from_slice(&stamp.1.to_le_bytes());
    h
}

fn mirror_name(json: &str) -> String { format!("{}.bin", json.trim_end_matches(".json")) }

#[cfg(unix)]
pub(crate) fn rename_over(from: &Path, to: &Path) -> std::io::Result<()> { std::fs::rename(from, to) }

/// A rename over a file some other program has open without delete sharing — an indexer, a sync
/// client, a scanner — is refused for as long as that handle lives, which is milliseconds. A held
/// *destination* is reported as access-denied rather than as a sharing violation (measured on a
/// Windows runner, where the sharing-violation-only form of this let both tests fail with os error
/// 5); std's own retry for access-denied is a second rename with POSIX semantics, which the
/// holder's share mode refuses just as flatly, so waiting is the only thing left. Both errors are
/// retried, for about half a second in all, before the store is left as it was.
#[cfg(windows)]
pub(crate) fn rename_over(from: &Path, to: &Path) -> std::io::Result<()> {
    const HELD_BY_ANOTHER_PROCESS: [i32; 2] = [5, 32];
    let mut wait = std::time::Duration::from_millis(1);
    loop {
        match std::fs::rename(from, to) {
            Err(e) if e.raw_os_error().is_some_and(|c| HELD_BY_ANOTHER_PROCESS.contains(&c))
                && wait < std::time::Duration::from_millis(512) => {
                std::thread::sleep(wait);
                wait *= 2;
            }
            r => return r,
        }
    }
}

impl Store {
    pub fn new(repo: &Path) -> Store { Store { dir: repo.join(".repograph") } }

    pub fn load(&self) -> Result<(Graph, Manifest)> { self.load_traced().map(|(g, m, _)| (g, m)) }

    /// `load`, saying where the graph came from, so a reader that may write can leave the
    /// mirror a JSON-only store is missing.
    pub fn load_traced(&self) -> Result<(Graph, Manifest, Source)> {
        let (g, source) = self.load_mirrored::<Graph>("graph.json")?;
        let m = match self.read_bytes("manifest.json")? {
            Some(b) => serde_json::from_slice(&b).context("manifest.json")?,
            None => Manifest::default(),
        };
        Ok((g.unwrap_or_default(), m, source))
    }

    /// A JSON store file, read through its mirror when the mirror still describes these bytes.
    pub fn load_mirrored<T: serde::de::DeserializeOwned>(&self, json: &str) -> Result<(Option<T>, Source)> {
        let p = self.dir.join(json);
        let Some(now) = stamp(&p) else { return Ok((None, Source::Absent)) };
        if let Some(bytes) = self.read_bytes(&mirror_name(json))? {
            // A mirror that does not decode is one a crash or another release left behind; the
            // JSON it mirrors is still there to answer from, so it is passed over, not reported.
            if bytes.starts_with(&mirror_header(now)) {
                if let Ok(v) = postcard::from_bytes::<T>(&bytes[MIRROR_HEADER..]) { return Ok((Some(v), Source::Mirror)); }
            }
        }
        let bytes = std::fs::read(&p).with_context(|| format!("read {}", p.display()))?;
        Ok((Some(serde_json::from_slice(&bytes).context(json.to_string())?), Source::Json))
    }

    /// The mirror of a JSON file as it stands on disk now. Nothing reads a mirror whose stamp
    /// disagrees with its JSON, so writing it after the JSON can never leave a reader misled.
    pub fn write_mirror<T: serde::Serialize>(&self, json: &str, value: &T) -> Result<()> {
        let Some(now) = stamp(&self.dir.join(json)) else { return Ok(()) };
        let mut bytes = mirror_header(now).to_vec();
        bytes.extend(postcard::to_stdvec(value)?);
        self.write_atomic(&mirror_name(json), &bytes)
    }

    /// Returns the stamp of the `graph.json` this call wrote, for a writer that claims vectors
    /// against it: a `stat` after the save could already be describing another writer's graph.
    pub fn save(&self, g: &Graph, m: &Manifest) -> Result<Option<crate::walk::Stamp>> {
        std::fs::create_dir_all(&self.dir)?;
        let written = self.write_atomic_stamped("graph.json", &serde_json::to_vec(g)?)?;
        self.write_mirror("graph.json", g)?;
        self.save_manifest(m)?;
        Ok(written)
    }

    /// The manifest alone: a walk that found no change can still have learned the stat stamps
    /// that spare the next one from hashing the tree again.
    pub fn save_manifest(&self, m: &Manifest) -> Result<()> {
        self.write_atomic("manifest.json", &serde_json::to_vec(m)?)
    }

    pub fn write_atomic(&self, name: &str, bytes: &[u8]) -> Result<()> {
        self.write_atomic_stamped(name, bytes).map(|_| ())
    }

    /// `write_atomic`, with the stamp of these bytes taken from the temporary file before the
    /// rename, which keeps its mtime and length: a writer renaming over the file a moment later
    /// cannot lend its stamp to them.
    fn write_atomic_stamped(&self, name: &str, bytes: &[u8]) -> Result<Option<crate::walk::Stamp>> {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        std::fs::create_dir_all(&self.dir)?;
        // One name per writer: readers that refresh at once each write a whole graph, and a
        // shared name let one rename away, or truncate, the file another was still writing.
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let tmp = self.dir.join(format!("{name}.{}.{n}.tmp", std::process::id()));
        let written = std::fs::write(&tmp, bytes).with_context(|| format!("write {}", tmp.display()))
            .and_then(|()| {
                let stamp = std::fs::metadata(&tmp).ok().as_ref().and_then(crate::walk::stamp_of);
                rename_over(&tmp, &self.dir.join(name)).with_context(|| format!("rename {name}"))?;
                Ok(stamp)
            });
        if written.is_err() { let _ = std::fs::remove_file(&tmp); }
        written
    }

    /// Extends a store file whose first `keep` bytes the caller still stands behind, dropping
    /// whatever follows them: bytes past that point are what an interrupted write left, and no
    /// metadata names them. Durable before it returns, so the metadata written next never
    /// points at rows a crash could still lose.
    pub fn append_after(&self, name: &str, keep: u64, bytes: &[u8]) -> Result<()> {
        use std::io::{Seek, SeekFrom, Write};
        std::fs::create_dir_all(&self.dir)?;
        let p = self.dir.join(name);
        let mut f = std::fs::OpenOptions::new().write(true).create(true).truncate(false).open(&p)
            .with_context(|| format!("open {}", p.display()))?;
        f.set_len(keep).with_context(|| format!("truncate {}", p.display()))?;
        f.seek(SeekFrom::Start(keep))?;
        f.write_all(bytes).with_context(|| format!("append {}", p.display()))?;
        f.sync_all()?;
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

    /// A record that has emptied leaves no file behind, so a store never holds one it would not write.
    pub fn remove(&self, name: &str) -> Result<()> {
        let p = self.dir.join(name);
        if p.exists() { std::fs::remove_file(&p).with_context(|| format!("remove {}", p.display()))?; }
        Ok(())
    }

    /// What a writer killed between its write and its rename left behind, and the header record.
    /// The graph and the manifest stay: `build` replaces them by rename when it saves, so a build
    /// that never gets there leaves the previous store to answer from rather than none. A temp file
    /// written in the last few minutes may be another process's save in flight, so only an older
    /// one goes. The headers stay with the graph they were read beside, for the same reason.
    pub fn drop_leftovers(&self) -> Result<()> {
        for e in std::fs::read_dir(&self.dir).into_iter().flatten().flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            let stale = e.metadata().and_then(|m| m.modified()).ok()
                .and_then(|t| t.elapsed().ok()).is_some_and(|age| age > LEFTOVER_AGE);
            if ["graph.json.", "manifest.json.", "headers.json."].iter().any(|p| name.starts_with(p)) && name.ends_with(".tmp") && stale {
                std::fs::remove_file(e.path()).with_context(|| format!("remove {}", e.path().display()))?;
            }
        }
        Ok(())
    }

    /// The store's writer lock, or `None` when another process holds it. A reader asks this way
    /// and answers from the store as it stands when it is held: the holder is a refresh already
    /// under way, and a second one would only race it to the same rename.
    pub fn try_lock_writer(&self) -> Result<Option<WriterLock>> {
        let f = self.lock_file()?;
        match f.try_lock() {
            Ok(()) => Ok(Some(WriterLock(f))),
            Err(std::fs::TryLockError::WouldBlock) => Ok(None),
            Err(std::fs::TryLockError::Error(e)) => Err(e).context("lock .repograph/writer.lock"),
        }
    }

    /// The writer lock, waited for. For a foreground `update` or `build`, whose caller asked for
    /// the store to be brought in line and would rather wait for a running refresh than skip it.
    pub fn lock_writer(&self) -> Result<WriterLock> {
        let f = self.lock_file()?;
        f.lock().context("lock .repograph/writer.lock")?;
        Ok(WriterLock(f))
    }

    /// Never removed: a lock file deleted while one process holds it lets the next process lock
    /// a new file of the same name, and both believe they are the only writer.
    fn lock_file(&self) -> Result<std::fs::File> {
        std::fs::create_dir_all(&self.dir)?;
        let p = self.dir.join("writer.lock");
        std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(&p)
            .with_context(|| format!("open {}", p.display()))
    }
}

/// Held for as long as this value lives. The operating system releases it with the handle, so a
/// writer that is killed leaves no lock behind.
pub struct WriterLock(#[allow(dead_code)] std::fs::File);

/// Far longer than any save takes: a temp file this old has no writer left.
const LEFTOVER_AGE: std::time::Duration = std::time::Duration::from_secs(600);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{EdgeKind, Extraction, NodeKind};

    /// Two handles in one process contend exactly as two processes do: the lock belongs to the
    /// open file, not to the process, on Unix and on Windows alike.
    #[test]
    fn a_held_writer_lock_is_reported_busy_until_it_is_dropped() {
        let d = tempfile::tempdir().unwrap();
        let store = Store::new(d.path());
        let held = store.try_lock_writer().unwrap().expect("nobody holds it yet");
        assert!(store.try_lock_writer().unwrap().is_none(), "a second writer is told it is held");
        drop(held);
        assert!(store.try_lock_writer().unwrap().is_some(), "dropping the first frees it");
    }

    fn graph(label: &str) -> Graph {
        let mut g = Graph::default();
        let mut e = Extraction::default();
        e.node(NodeKind::Requirement, "FR-WEB-01", label, "тело требования", "docs/a.md", 3);
        e.edge("FR-WEB-01", "INV-01", EdgeKind::References, "ctx", "docs/a.md");
        g.apply(e);
        g
    }

    #[test]
    fn a_saved_graph_loads_from_its_mirror_and_matches_the_json() {
        let d = tempfile::tempdir().unwrap();
        let store = Store::new(d.path());
        let g = graph("Заголовок");
        store.save(&g, &Manifest::default()).unwrap();
        assert!(d.path().join(".repograph/graph.bin").exists());
        let (g2, _, source) = store.load_traced().unwrap();
        assert_eq!(source, Source::Mirror);
        assert_eq!(g2.nodes, g.nodes);
        assert_eq!(g2.edges, g.edges);
    }

    // The JSON is rewritten to a different length, so the mirror's stamp disagrees even on a
    // file system with coarse mtimes.
    #[test]
    fn a_mirror_behind_a_rewritten_json_is_passed_over() {
        let d = tempfile::tempdir().unwrap();
        let store = Store::new(d.path());
        store.save(&graph("old"), &Manifest::default()).unwrap();
        store.write_atomic("graph.json", &serde_json::to_vec(&graph("a much longer new label")).unwrap()).unwrap();
        let (g, _, source) = store.load_traced().unwrap();
        assert_eq!(source, Source::Json);
        assert_eq!(g.nodes["FR-WEB-01"].label, "a much longer new label");
    }

    #[test]
    fn a_mirror_that_does_not_decode_falls_back_to_the_json() {
        let d = tempfile::tempdir().unwrap();
        let store = Store::new(d.path());
        store.save(&graph("x"), &Manifest::default()).unwrap();
        let p = d.path().join(".repograph/graph.bin");
        let mut bytes = std::fs::read(&p).unwrap();
        bytes.truncate(MIRROR_HEADER + 3);
        std::fs::write(&p, bytes).unwrap();
        let (g, _, source) = store.load_traced().unwrap();
        assert_eq!(source, Source::Json);
        assert_eq!(g.nodes.len(), 1);
    }

    #[test]
    fn a_mirror_from_another_release_is_passed_over() {
        let d = tempfile::tempdir().unwrap();
        let store = Store::new(d.path());
        store.save(&graph("x"), &Manifest::default()).unwrap();
        let p = d.path().join(".repograph/graph.bin");
        let mut bytes = std::fs::read(&p).unwrap();
        bytes[4..10].copy_from_slice(b"9.9.9\0");
        std::fs::write(&p, bytes).unwrap();
        let (_, _, source) = store.load_traced().unwrap();
        assert_eq!(source, Source::Json);
    }

    /// A store written by a release whose `Graph` had no `pending`: its JSON reads with none
    /// held aside, and its mirror — a postcard image of the older struct under the older magic
    /// — is passed over rather than decoded into the wrong shape.
    #[test]
    fn a_store_written_before_pending_existed_reads_whole() {
        #[derive(serde::Serialize)]
        struct OldGraph { nodes: std::collections::BTreeMap<String, crate::model::Node>, edges: std::collections::BTreeSet<crate::model::Edge> }
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path());
        let mut g = Graph::default();
        let mut e = crate::model::Extraction::default();
        e.node(crate::model::NodeKind::Requirement, "FR-1", "x", "", "docs/a.md", 1);
        e.edge("FR-1", "FR-2", crate::model::EdgeKind::References, "body", "docs/a.md");
        g.apply(e);
        let old = OldGraph { nodes: g.nodes.clone(), edges: g.edges.clone() };
        std::fs::create_dir_all(dir.path().join(".repograph")).unwrap();
        store.write_atomic("graph.json", &serde_json::to_vec(&old).unwrap()).unwrap();
        store.write_atomic("manifest.json", b"{\"files\":{}}").unwrap();
        // The older release's mirror: its magic, its stamp, its struct.
        let now = stamp(&dir.path().join(".repograph/graph.json")).unwrap();
        let mut bytes = mirror_header(now).to_vec();
        bytes[..4].copy_from_slice(b"RGM1");
        bytes.extend(postcard::to_stdvec(&old).unwrap());
        store.write_atomic("graph.bin", &bytes).unwrap();

        let (read, _, source) = store.load_traced().unwrap();
        assert_eq!(source, Source::Json, "the old mirror is passed over, not misread");
        assert_eq!((read.nodes, read.edges), (g.nodes, g.edges));
        assert!(read.pending.is_empty());
    }

    #[test]
    fn a_store_without_a_mirror_reads_the_json_and_can_be_given_one() {
        let d = tempfile::tempdir().unwrap();
        let store = Store::new(d.path());
        let g = graph("x");
        store.save(&g, &Manifest::default()).unwrap();
        std::fs::remove_file(d.path().join(".repograph/graph.bin")).unwrap();
        let (_, _, source) = store.load_traced().unwrap();
        assert_eq!(source, Source::Json);
        store.write_mirror("graph.json", &g).unwrap();
        let (g2, _, source) = store.load_traced().unwrap();
        assert_eq!(source, Source::Mirror);
        assert_eq!(g2.nodes, g.nodes);
    }

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
        assert_eq!(tmps(d.path()), Vec::<String>::new());
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

    fn tmps(repo: &Path) -> Vec<String> {
        std::fs::read_dir(repo.join(".repograph")).unwrap().flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.ends_with(".tmp")).collect()
    }

    #[test]
    fn a_leftover_tmp_from_a_crashed_write_is_not_read_and_is_dropped() {
        let d = tempfile::tempdir().unwrap();
        let store = Store::new(d.path());
        std::fs::create_dir_all(d.path().join(".repograph")).unwrap();
        let old = d.path().join(".repograph/graph.json.1.0.tmp");
        std::fs::write(&old, "garbage").unwrap();
        let hour_ago = std::time::SystemTime::now() - std::time::Duration::from_secs(3600);
        std::fs::File::options().write(true).open(&old).unwrap().set_modified(hour_ago).unwrap();
        store.save(&Graph::default(), &Manifest::default()).unwrap();
        let (g, _) = store.load().unwrap();
        assert!(g.nodes.is_empty());
        store.drop_leftovers().unwrap();
        assert_eq!(tmps(d.path()), Vec::<String>::new());
    }

    #[test]
    fn a_temp_file_another_writer_is_saving_right_now_is_kept() {
        let d = tempfile::tempdir().unwrap();
        let store = Store::new(d.path());
        std::fs::create_dir_all(d.path().join(".repograph")).unwrap();
        std::fs::write(d.path().join(".repograph/graph.json.2.0.tmp"), "half a graph").unwrap();
        store.drop_leftovers().unwrap();
        assert_eq!(tmps(d.path()), vec!["graph.json.2.0.tmp".to_string()]);
    }

    // Bytes reach the store only through a rename, so a write that fails part-way — here the
    // store directory refuses new files — cannot leave a half-written graph behind.
    #[cfg(unix)]
    #[test]
    fn a_failed_write_leaves_the_stored_graph_intact() {
        use std::os::unix::fs::PermissionsExt;
        let d = tempfile::tempdir().unwrap();
        let store = Store::new(d.path());
        let mut g = Graph::default();
        let mut e = Extraction::default();
        e.node(NodeKind::File, "file:a.md", "a.md", "", "a.md", 1);
        g.apply(e);
        store.save(&g, &Manifest::default()).unwrap();
        let dir = d.path().join(".repograph");
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).unwrap();
        let failed = store.save(&Graph::default(), &Manifest::default()).is_err();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(failed);
        assert_eq!(store.load().unwrap().0.nodes.len(), 1);
    }

    /// Readers that refresh at once each write a whole graph and each rename it into place:
    /// none is refused its rename, and what stands afterwards is one writer's bytes entire.
    #[test]
    fn concurrent_writers_each_publish_a_whole_file() {
        let d = tempfile::tempdir().unwrap();
        let writers: Vec<_> = (0u8..8).map(|w| {
            let repo = d.path().to_path_buf();
            std::thread::spawn(move || {
                let store = Store::new(&repo);
                let bytes = vec![b'a' + w; 256 * 1024];
                for _ in 0..40 { store.write_atomic("graph.json", &bytes).unwrap(); }
            })
        }).collect();
        for w in writers { w.join().unwrap(); }
        let got = Store::new(d.path()).read_bytes("graph.json").unwrap().unwrap();
        assert_eq!(got.len(), 256 * 1024);
        assert!(got.iter().all(|&b| b == got[0]), "a file of two writers' bytes");
        assert_eq!(tmps(d.path()), Vec::<String>::new());
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
    fn dropping_leftovers_keeps_every_store_file() {
        let d = tempfile::tempdir().unwrap();
        let s = Store::new(d.path());
        s.save(&Graph::default(), &Manifest::default()).unwrap();
        s.write_atomic("vectors.f32", b"v").unwrap();
        s.write_atomic("questions.json", b"{}").unwrap();
        s.drop_leftovers().unwrap();
        assert!(d.path().join(".repograph/graph.json").exists());
        assert!(d.path().join(".repograph/manifest.json").exists());
        assert_eq!(s.read_bytes("vectors.f32").unwrap().as_deref(), Some(&b"v"[..]));
        assert_eq!(s.read_bytes("questions.json").unwrap().as_deref(), Some(&b"{}"[..]));
    }

    #[test]
    fn append_after_cuts_the_file_back_to_the_kept_bytes_before_extending_it() {
        let d = tempfile::tempdir().unwrap();
        let store = Store::new(d.path());
        store.write_atomic("vectors.f32", b"keepGARBAGE").unwrap();
        store.append_after("vectors.f32", 4, b"more").unwrap();
        assert_eq!(store.read_bytes("vectors.f32").unwrap().as_deref(), Some(&b"keepmore"[..]));
    }

    #[test]
    fn load_absent_is_empty() {
        let d = tempfile::tempdir().unwrap();
        let (g, m) = Store::new(d.path()).load().unwrap();
        assert!(g.nodes.is_empty() && m.files.is_empty());
    }

    /// A reader that took no delete share — an indexer, a sync client, an editor — holds the
    /// destination for a moment; the rename waits it out rather than failing the save.
    #[cfg(windows)]
    fn hold_open_for(path: &std::path::Path, ms: u64) -> std::thread::JoinHandle<()> {
        use std::os::windows::fs::OpenOptionsExt;
        use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ;
        let f = std::fs::OpenOptions::new().read(true).share_mode(FILE_SHARE_READ).open(path).unwrap();
        std::thread::spawn(move || { std::thread::sleep(std::time::Duration::from_millis(ms)); drop(f); })
    }

    #[cfg(windows)]
    #[test]
    fn a_rename_over_a_briefly_held_file_waits_and_lands() {
        let d = tempfile::tempdir().unwrap();
        let store = Store::new(d.path());
        store.write_atomic("graph.json", b"old").unwrap();
        let holder = hold_open_for(&d.path().join(".repograph/graph.json"), 100);
        store.write_atomic("graph.json", b"new").unwrap();
        holder.join().unwrap();
        assert_eq!(store.read_bytes("graph.json").unwrap().as_deref(), Some(&b"new"[..]));
    }

    /// The error a held destination gives, pinned: 5, not the 32 a sharing violation would be —
    /// the rename is refused where it deletes the file it replaces, and that is reported as
    /// access denied.
    #[cfg(windows)]
    #[test]
    fn a_rename_over_a_file_held_past_the_budget_fails_and_keeps_the_old_bytes() {
        let d = tempfile::tempdir().unwrap();
        let store = Store::new(d.path());
        store.write_atomic("graph.json", b"old").unwrap();
        let holder = hold_open_for(&d.path().join(".repograph/graph.json"), 1500);
        let err = store.write_atomic("graph.json", b"new").unwrap_err();
        assert!(format!("{err:#}").contains("os error 5"), "{err:#}");
        holder.join().unwrap();
        assert_eq!(store.read_bytes("graph.json").unwrap().as_deref(), Some(&b"old"[..]));
    }
}
