//! Families as the documents write them. Nothing is configured and nothing is stored: a build
//! reads the definitions its documents carry, an update says so when that set moves, and
//! `repograph families` shows what was derived beside what was left as text.

use std::process::Command;

const FIXTURES: &[&str] = &["03-calendar.md", "06-payments.md", "BE-M01-foundation.md", "constitution.yaml"];

fn repograph(repo: &std::path::Path, args: &[&str]) -> (bool, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_repograph"))
        .arg("--no-dense").arg("--repo").arg(repo).args(args).output().unwrap();
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

const REQ: &str = "# Требования\n\n**REQ-7 · MUST · Отмена визита за сутки**\n\nОтмена возможна за сутки.\n\nсм. REQ-7, даты по ISO-8601\n";

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
    assert_eq!((req[1], req[2]), ("1", "docs/req.md:3"), "the family, its nodes and the line that defines it: {out}");
    assert!(mention(&out, "ISO").contains("docs/req.md:7"), "and the prefix no line defines: {out}");
    assert!(out.contains("Not families"), "said in words, not left as a column to read: {out}");

    // A no-op update is a fixed point, families included: what the documents define is what the
    // graph already declares, so nothing is re-read and nothing is said.
    let (ok, out, err) = repograph(repo, &["update"]);
    assert!(ok, "{out}{err}");
    assert!(out.starts_with("changed 0 removed 0"), "{out}");
    assert!(!err.contains("families"), "{err}");

    // A family the corpus grows: every document is re-read under it, not only the edited one.
    std::fs::write(repo.join("docs/new.md"), "**NEW-1 · MUST · Новое правило**\n\nтело\n").unwrap();
    let (ok, out, err) = repograph(repo, &["update"]);
    assert!(ok, "{out}{err}");
    assert!(err.contains("families: +NEW"), "{err}");
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

    // The definition edited away with no update since. `ask` still answers NEW-1 — the nodes are
    // in the store and the read matcher comes off them — so the report says the family is there
    // and nothing defines it, rather than leaving out the one state it exists to expose.
    std::fs::write(repo.join("docs/new.md"), "тело без определения\n").unwrap();
    let (ok, out, err) = repograph(repo, &["families"]);
    assert!(ok, "{out}{err}");
    assert!(families(&out, "NEW").contains("nothing defines it any more"), "{out}");
    let (ok, json, err) = repograph(repo, &["families", "--json"]);
    assert!(ok, "{json}{err}");
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    let new = v["families"].as_array().unwrap().iter().find(|f| f["family"] == "NEW").unwrap();
    assert!(new["defined"].is_null() && new["nodes"] == 1, "{new}");
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
