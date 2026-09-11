//! Families as the documents write them. Nothing is configured and nothing is stored: a build
//! reads the definitions its documents carry, an update says so when that set moves, and
//! `repograph families` shows what was derived beside what was left as text.

mod common;

const FIXTURES: &[&str] = &["03-calendar.md", "06-payments.md", "BE-M01-foundation.md", "constitution.yaml"];

fn repograph(repo: &std::path::Path, args: &[&str]) -> (bool, String, String) {
    let out = common::run(repo, args);
    (out.status.success(), String::from_utf8_lossy(&out.stdout).into_owned(), String::from_utf8_lossy(&out.stderr).into_owned())
}

/// The rendered table under this heading: its rows, up to the next heading. Sections are read
/// apart because a prefix crossing the line between them — a family the documents stopped
/// defining, a mention that became one — is the whole of what the report is for, and a search
/// over the rendered output as a whole cannot see the move.
fn section<'a>(text: &'a str, head: &str) -> Vec<&'a str> {
    text.lines()
        .skip_while(|l| !l.starts_with(head))
        .skip(1)
        .take_while(|l| l.starts_with("  "))
        .collect()
}

/// The row of one section that starts with this name, indented.
fn row<'a>(text: &'a str, head: &str, name: &str) -> &'a str {
    section(text, head).into_iter()
        .find(|l| l.strip_prefix("  ").is_some_and(|l| l.split_whitespace().next() == Some(name)))
        .unwrap_or_else(|| panic!("no {name} row under {head} in:\n{text}"))
}

fn families<'a>(text: &'a str, name: &str) -> &'a str { row(text, "families", name) }
fn mention<'a>(text: &'a str, name: &str) -> &'a str { row(text, "mention-only prefixes", name) }

const REQ: &str = "# Требования\n\n**REQ-7 · MUST · Отмена визита за сутки**\n\nОтмена возможна за сутки, задача NEW-5.\n\nсм. REQ-7, даты по ISO-8601\n";

#[test]
fn a_repository_gets_its_families_from_the_documents_and_says_when_they_move() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    std::fs::create_dir_all(repo.join("docs")).unwrap();
    std::fs::write(repo.join("docs/req.md"), REQ).unwrap();

    let (ok, out, err) = repograph(repo, &["build"]);
    assert!(ok, "{out}{err}");
    assert!(err.contains("families: REQ · milestones: (none)"), "the build says what it read: {err}");
    assert!(!err.contains("ISO"), "a prefix only ever mentioned is not a family: {err}");

    // The point of all of it: a family nobody configured answers exactly.
    let (ok, out, err) = repograph(repo, &["ask", "REQ-7"]);
    assert!(ok, "{out}{err}");
    assert!(out.lines().next().unwrap().starts_with("REQ-7"), "{out}");

    let (ok, out, err) = repograph(repo, &["families"]);
    assert!(ok, "{out}{err}");
    let req: Vec<&str> = families(&out, "REQ").split_whitespace().collect();
    assert_eq!((req[1], req[2], req[3]), ("1", "1", "docs/req.md:3"), "the family, its nodes, its definitions and the line that defines it: {out}");
    assert!(families(&out, "REQ").ends_with("defined once"), "one line defines it, and the row says so: {out}");
    assert!(mention(&out, "ISO").contains("docs/req.md:7"), "and the prefix no line defines: {out}");
    assert!(out.contains("Not families"), "said in words, not left as a column to read: {out}");

    // A no-op update is a fixed point, families included: nothing is re-read and nothing is said.
    let (ok, out, err) = repograph(repo, &["update"]);
    assert!(ok, "{out}{err}");
    assert!(out.starts_with("changed 0 removed 0"), "{out}");
    assert!(!err.contains("families"), "{err}");

    // Before any line defines `NEW`, the citation is held aside: `verify` counts it and nothing
    // a reader follows reaches it.
    let (ok, out, err) = repograph(repo, &["verify"]);
    assert!(ok, "{out}{err}");
    assert!(out.contains("held aside: 2 edges to ids in 2 prefixes no line defines  ISO NEW"), "{out}");
    let (ok, out, _) = repograph(repo, &["explain", "REQ-7"]);
    assert!(ok && !out.contains("NEW-5"), "a held-aside citation is not an answer: {out}");

    // A family the corpus grows: the one file that defines it is read, and the citations the
    // graph was holding are released — `req.md` is not touched and not re-read.
    std::fs::write(repo.join("docs/new.md"), "**NEW-1 · MUST · Новое правило**\n\nтело\n").unwrap();
    let (ok, out, err) = repograph(repo, &["update"]);
    assert!(ok, "{out}{err}");
    assert!(out.starts_with("changed 1 removed 0"), "{out}");
    assert!(err.contains("families: +NEW"), "{err}");
    let (ok, out, _) = repograph(repo, &["explain", "REQ-7"]);
    assert!(ok && out.contains("NEW-5"), "released without a re-read: {out}");
    let (ok, out, _) = repograph(repo, &["verify"]);
    assert!(ok && out.contains("held aside: 1 edges to ids in 1 prefixes no line defines  ISO"), "{out}");
    let (ok, out, err) = repograph(repo, &["ask", "NEW-1"]);
    assert!(ok, "{out}{err}");
    assert!(out.lines().next().unwrap().starts_with("NEW-1"), "{out}");

    // And a file that still names the old keys: one line, and not one family different.
    std::fs::write(repo.join("repograph.toml"), "id_families = [\"FR-PAY\"]\n").unwrap();
    let (ok, out, err) = repograph(repo, &["families"]);
    assert!(ok, "{out}{err}");
    assert!(err.contains("id_families is no longer read"), "{err}");
    assert!(families(&out, "REQ").contains("docs/req.md:3") && families(&out, "NEW").contains("docs/new.md:1"), "{out}");
    assert!(!out.contains("FR-PAY"), "a key nobody reads names no family: {out}");

    // The definition edited away with no update since. The graph is the report's source, so the
    // family is still there with its node; what the report can say is that the store is behind
    // the tree, which is the one state it exists to expose.
    std::fs::write(repo.join("docs/new.md"), "тело без определения\n").unwrap();
    let (ok, out, err) = repograph(repo, &["families"]);
    assert!(ok, "{out}{err}");
    assert!(families(&out, "NEW").contains("docs/new.md:1"), "{out}");
    assert!(err.contains("families: the store is 1 file behind the tree — run `repograph update`"), "{err}");
    let (ok, json, err) = repograph(repo, &["families", "--json"]);
    assert!(ok, "{json}{err}");
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    let new = v["families"].as_array().unwrap().iter().find(|f| f["family"] == "NEW").unwrap();
    assert!(new["defined"]["file"] == "docs/new.md" && new["nodes"] == 1, "{new}");
    // And after the update the family is gone with its definition, and its citation is held again.
    let (ok, out, err) = repograph(repo, &["update"]);
    assert!(ok && err.contains("families: -NEW"), "{out}{err}");
    let (ok, out, _) = repograph(repo, &["verify"]);
    assert!(ok && out.contains("held aside: 2 edges"), "{out}");
}

/// A repository whose documents define nothing: file nodes, an empty family line, and every
/// id-shaped mention held aside where no reader follows it. The state the old never-matching
/// alternation stood in for, now a state the store carries — there is no constructor left to
/// hand a `None` to.
#[test]
fn a_corpus_that_defines_no_ids_reads_as_one() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    std::fs::create_dir_all(repo.join("docs")).unwrap();
    std::fs::write(repo.join("docs/notes.md"), "# Заметки\n\nдаты по ISO-8601, см. RFC-7231 и -M01\n").unwrap();

    let (ok, out, err) = repograph(repo, &["build"]);
    assert!(ok, "{out}{err}");
    assert!(err.contains("families: (none) · milestones: (none)"), "{err}");
    assert!(out.starts_with("changed 1 removed 0 nodes 1 edges 0"), "one file node and nothing a reader follows: {out}");

    let (ok, out, err) = repograph(repo, &["verify"]);
    assert!(ok, "{out}{err}");
    assert!(out.contains("held aside: 2 edges to ids in 2 prefixes no line defines  ISO RFC"), "{out}");
    assert!(out.contains("dangling edges: 0\n"), "{out}");

    let (ok, out, err) = repograph(repo, &["families"]);
    assert!(ok, "{out}{err}");
    assert!(out.contains("families                              nodes  defs  defined\n  (none)"), "{out}");
    assert!(mention(&out, "ISO").contains("docs/notes.md:3"), "{out}");

    // An id-shaped word that is no node is a search term, not an exact seed — and on a corpus
    // whose only node is the file itself there is nothing for the term to seat, so the answer is
    // empty rather than an answer about `ISO-8601`. The contract is both halves of that: the
    // command succeeds, and it prints nothing.
    let (ok, out, err) = repograph(repo, &["ask", "ISO-8601"]);
    assert!(ok, "{out}{err}");
    assert_eq!(out.trim(), "", "nothing to seat on a corpus of file nodes: {out}");
}

/// One id defined in two documents: the family has one node and two definitions, and the report
/// says both. The two counts stand side by side in the rendered row, so a row that read them in
/// the other order — or read one of them twice — would print `2  1` here.
#[test]
fn a_family_defined_in_two_files_counts_its_nodes_apart_from_its_definitions() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    std::fs::create_dir_all(repo.join("docs")).unwrap();
    std::fs::write(repo.join("docs/req.md"), REQ).unwrap();
    std::fs::write(repo.join("docs/req2.md"),
                   "**REQ-7 · MUST · Отмена визита за сутки**\n\nто же требование, другой документ\n").unwrap();
    assert!(repograph(repo, &["build"]).0);

    let (ok, out, err) = repograph(repo, &["families"]);
    assert!(ok, "{out}{err}");
    let req: Vec<&str> = families(&out, "REQ").split_whitespace().collect();
    assert_eq!((req[1], req[2]), ("1", "2"), "one node, defined in two files: {out}");
}

/// The four shapes a definition comes in, over the corpus the extractor cases quote: a bold head,
/// a heading, a milestone document's own name, and a registry row.
#[test]
fn every_shape_the_extractor_defines_a_node_with_defines_a_family() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    std::fs::create_dir_all(repo.join("docs")).unwrap();
    let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    for name in FIXTURES {
        std::fs::copy(fixtures.join(name), repo.join("docs").join(name)).unwrap();
    }

    let (ok, _, err) = repograph(repo, &["families"]);
    assert!(!ok && err.contains("repograph build"), "before a build it should say so: {err}");
    assert!(repograph(repo, &["build"]).0);

    let (ok, out, err) = repograph(repo, &["families"]);
    assert!(ok, "{out}{err}");
    let nodes = |name: &str| families(&out, name).split_whitespace().nth(1).unwrap().parse::<usize>().unwrap();
    assert!(nodes("FR-PAY") > 0 && families(&out, "FR-PAY").contains("docs/06-payments.md:"), "a bold head: {out}");
    assert!(nodes("FR-CAL") > 0 && families(&out, "FR-CAL").contains("docs/03-calendar.md:"), "a heading: {out}");
    assert!(nodes("INV") > 0 && families(&out, "INV").contains("docs/constitution.yaml:"), "a registry row: {out}");
    assert!(row(&out, "milestones", "BE").contains("docs/BE-M01-foundation.md:1"), "a milestone document: {out}");
    // Written in a requirement's body and defined nowhere — the line the rule draws.
    assert!(mention(&out, "OQ").contains("docs/"), "a prefix only written is text: {out}");

    let (ok, json, err) = repograph(repo, &["families", "--json"]);
    assert!(ok, "{json}{err}");
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    let cal = v["families"].as_array().unwrap().iter().find(|f| f["family"] == "FR-CAL").unwrap();
    assert_eq!(cal["defined"]["file"], "docs/03-calendar.md");
    assert!(v["milestones"].as_array().unwrap().iter().any(|m| m["family"] == "BE"));
    assert!(v["mention_only"].as_array().unwrap().iter().any(|m| m["prefix"] == "OQ"));
}
