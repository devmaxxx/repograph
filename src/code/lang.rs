//! Which grammar reads a file, decided by its name alone.
//!
//! This file names no crate module but `model`: `tests/parse_census.rs` compiles it by path,
//! because the crate is a binary and a test cannot link against it. Its cases live in
//! `src/code/cases.rs` for the same reason.

use crate::model::{Extraction, NodeKind};
use tree_sitter::{Language, Parser, Tree};

/// One variant per grammar the 0.6.0 design reads, plus the two languages read by blanking.
// Declared ahead of the family plans that construct them, so that no plan edits the declaration.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Lang { TypeScript, Tsx, Kotlin, Java, CSharp, Razor, Rust, Python, Dart, Swift, GraphQl, Sql, Bicep, Hcl, Shell, Vue }

/// The unit a resolver works over: languages that compile together share one.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Family { TypeScript, Jvm, DotNet, Rust, Python, Dart, Swift, GraphQl, Sql, Bicep, Hcl, Shell }

impl Lang {
    /// `None`: no grammar and no embedding reads this extension (L1). Such a file is indexed as a
    /// file and never parsed: every grammar returns a tree for any input, and a tree of the wrong
    /// language looks right.
    pub fn of(rel: &str) -> Option<Lang> {
        match std::path::Path::new(rel).extension().and_then(|e| e.to_str()) {
            Some("ts" | "mts" | "cts" | "js" | "mjs" | "cjs") => Some(Lang::TypeScript),
            // JSX needs the TSX grammar; the TypeScript one reads `<div/>` as a type assertion.
            Some("tsx" | "jsx") => Some(Lang::Tsx),
            _ => None,
        }
    }

    /// `None` for the embedded languages (Razor, Vue), which are read by blanking through another grammar.
    pub fn grammar(self) -> Option<Language> {
        match self {
            Lang::TypeScript => Some(Language::new(tree_sitter_typescript::LANGUAGE_TYPESCRIPT)),
            Lang::Tsx => Some(Language::new(tree_sitter_typescript::LANGUAGE_TSX)),
            // A language whose family plan has not landed has no crate in this build, and `of`
            // never returns it. Once every plan has landed the arm is unreachable, and harmless.
            #[allow(unreachable_patterns)]
            _ => None,
        }
    }

    pub fn parse(self, src: &[u8]) -> Option<Tree> {
        let mut parser = Parser::new();
        parser.set_language(&self.grammar()?).ok()?;
        parser.parse(src, None)
    }

    // Only the cases read this until the resolvers do.
    #[allow(dead_code)]
    pub fn family(self) -> Family {
        match self {
            Lang::TypeScript | Lang::Tsx => Family::TypeScript,
            // Reached only by a variant `of` cannot return yet; see `grammar`.
            #[allow(unreachable_patterns)]
            other => unreachable!("{other:?} has no family arm: its plan has not landed"),
        }
    }
}

/// The `File` node every extractor writes for `file:<rel>`, and all that a file no grammar reads gets.
pub fn file_node(rel: &str, ex: &mut Extraction) {
    ex.node(NodeKind::File, &format!("file:{rel}"), rel, "", rel, 1);
}

/// One stderr line naming what was indexed without being read, so a glob that reaches past the
/// grammars says so instead of producing a graph that is silently thinner than the tree.
pub fn files_only_note<'a>(rels: impl Iterator<Item = &'a str>) -> Option<String> {
    let mut by_ext = std::collections::BTreeMap::<String, usize>::new();
    for rel in rels.filter(|r| Lang::of(r).is_none()) {
        let ext = std::path::Path::new(rel).extension().and_then(|e| e.to_str())
            .map(|e| format!(".{e}")).unwrap_or_else(|| "(no extension)".to_string());
        *by_ext.entry(ext).or_default() += 1;
    }
    if by_ext.is_empty() {
        return None;
    }
    let mut parts: Vec<(String, usize)> = by_ext.into_iter().collect();
    parts.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let list = parts.iter().map(|(e, n)| format!("{n} {e}")).collect::<Vec<_>>().join(", ");
    Some(format!("code: no grammar reads {list} — indexed as files only"))
}
