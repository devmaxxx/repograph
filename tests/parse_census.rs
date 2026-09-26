//! §0's parse census, taken on any tree through the grammar table the extractor uses.
//!
//! The ignored test prints the table. `cargo test` still compiles the file, so a grammar table that
//! stops compiling here fails CI rather than a measurement day. The crate is a binary, so the
//! table is compiled into this test by path rather than linked.

#[path = "../src/code/lang.rs"]
#[allow(dead_code)]
mod lang;

/// `lang.rs` writes its file node into the crate's model. The census writes no node, so this is
/// the one shape `file_node` needs and nothing more.
#[allow(dead_code)]
mod model {
    pub enum NodeKind {
        File,
    }

    #[derive(Default)]
    pub struct Extraction;

    impl Extraction {
        pub fn node(&mut self, _kind: NodeKind, _id: &str, _label: &str, _body: &str, _file: &str, _line: u32) {}
    }
}

use lang::Lang;
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Debug, Default)]
struct Row {
    files: usize,
    bad: usize,
    error: usize,
    missing: usize,
    unparsed: usize,
}

/// The default `skip` globs as path segments: a census of vendored or built output counts someone
/// else's code.
fn skipped(rel: &str) -> bool {
    let base = rel.rsplit('/').next().unwrap_or(rel);
    rel.split('/').any(|s| s == "node_modules" || s == "dist" || s == ".yarn")
        || base.ends_with(".min.js") || base.starts_with(".pnp.")
        || rel.starts_with("graphify-out/") || rel.starts_with(".repograph/")
}

fn census(roots: &[PathBuf]) -> BTreeMap<Lang, Row> {
    let mut rows: BTreeMap<Lang, Row> = BTreeMap::new();
    for root in roots {
        // Dotted directories are walked as `walk` walks them, `.git` aside. A test's temporary tree
        // has no `.git`, and its `.gitignore` still says what is not source.
        let walker = ignore::WalkBuilder::new(root).hidden(false).filter_entry(|e| e.file_name() != ".git")
            .git_ignore(true).require_git(false).build();
        for dent in walker.flatten() {
            if !dent.file_type().is_some_and(|t| t.is_file()) {
                continue;
            }
            let rel = dent.path().strip_prefix(root).unwrap_or(dent.path()).to_string_lossy().replace('\\', "/");
            if skipped(&rel) {
                continue;
            }
            let Some(lang) = Lang::of(&rel) else { continue };
            let Ok(src) = std::fs::read(dent.path()) else { continue };
            let row = rows.entry(lang).or_default();
            row.files += 1;
            let Some(tree) = lang.parse(&src) else {
                row.unparsed += 1;
                continue;
            };
            let (mut error, mut missing) = (0, 0);
            let mut cursor = tree.walk();
            'walk: loop {
                let n = cursor.node();
                error += usize::from(n.is_error());
                missing += usize::from(n.is_missing());
                if cursor.goto_first_child() {
                    continue;
                }
                while !cursor.goto_next_sibling() {
                    if !cursor.goto_parent() {
                        break 'walk;
                    }
                }
            }
            row.error += error;
            row.missing += missing;
            row.bad += usize::from(error + missing > 0);
        }
    }
    rows
}

#[test]
#[ignore]
fn parse_census() {
    let roots: Vec<PathBuf> = std::env::var("REPOGRAPH_CENSUS_ROOTS")
        .expect("set REPOGRAPH_CENSUS_ROOTS to one or more `:`-separated roots")
        .split(':')
        .filter(|r| !r.is_empty())
        .map(PathBuf::from)
        .collect();
    println!("{:<11} {:>6} {:>6} {:>7} {:>7}", "lang", "files", "bad", "ERROR", "MISSING");
    for (lang, r) in census(&roots) {
        let name = format!("{lang:?}");
        // A language read by blanking has no grammar of its own to count errors with.
        if r.unparsed == r.files {
            println!("{name:<11} {:>6} {:>6} {:>7} {:>7}", r.files, "-", "-", "-");
        } else {
            println!("{name:<11} {:>6} {:>6} {:>7} {:>7}", r.files, r.bad, r.error, r.missing);
        }
    }
}

#[test]
fn a_file_with_error_nodes_is_counted_once_however_many_it_holds() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::write(root.join("good.ts"), "export const a = 1;\n").unwrap();
    std::fs::write(root.join("bad.ts"), "export class {\nexport class {\n").unwrap();
    std::fs::write(root.join("view.tsx"), "export const V = () => <p/>;\n").unwrap();
    std::fs::write(root.join("notes.md"), "# not code\n").unwrap();
    let rows = census(&[root.to_path_buf()]);
    let ts = &rows[&Lang::TypeScript];
    assert_eq!((ts.files, ts.bad), (2, 1));
    assert!(ts.error + ts.missing >= 1);
    assert_eq!((rows[&Lang::Tsx].files, rows[&Lang::Tsx].bad), (1, 0));
    assert_eq!(rows.len(), 2, "a file no grammar reads is not a row: {rows:?}");
}

#[test]
fn vendored_and_built_output_stays_out_of_the_count() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    for rel in ["src/a.ts", "node_modules/x/index.ts", "dist/a.ts", "packages/p/dist/b.ts"] {
        std::fs::create_dir_all(root.join(rel).parent().unwrap()).unwrap();
        std::fs::write(root.join(rel), "export const a = 1;\n").unwrap();
    }
    assert_eq!(census(&[root.to_path_buf()])[&Lang::TypeScript].files, 1);
}
