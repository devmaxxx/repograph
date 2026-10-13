//! Which grammar reads a file: by its name, through our overrides and then ast-grep's extension
//! table, and by its shebang when the name says nothing.
//!
//! This file names no crate module but `model`: `tests/parse_census.rs` compiles it by path,
//! because the crate is a binary and a test cannot link against it. Its cases live in
//! `src/code/cases.rs` for the same reason.

use crate::model::{Extraction, NodeKind};
use ast_grep_language::{Language as _, LanguageExt as _, SupportLang};
use tree_sitter::{Language, Parser, Tree};

/// One variant per grammar the 0.6.0 design reads, plus the two languages read by blanking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Lang { TypeScript, Tsx, Kotlin, Java, CSharp, Razor, Rust, Python, Dart, Swift, GraphQl, Sql, Bicep, Hcl, Terraform, Shell, Vue, Lua, Go, C }

/// The unit a resolver works over: languages that compile together share one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Family { TypeScript, Jvm, DotNet, Rust, Python, Dart, Swift, GraphQl, Sql, Bicep, Hcl, Shell, Lua, Go, C }

/// The extension a grammar or the files-only note keys off, lowercase-sensitive like the
/// filesystem: `Path::extension` alone, shared so the two readings of it cannot drift apart.
fn ext(rel: &str) -> Option<&str> {
    std::path::Path::new(rel).extension().and_then(|e| e.to_str())
}

/// What a file's name says about it.
enum Named {
    Read(Lang),
    /// A language we do not read, or an extension we refuse: final, so no shebang overrides it.
    Unread,
    /// Neither table knows the name; only the file's first line can tell.
    Unknown,
}

/// Our overrides, then ast-grep's table (L2). Both read the extension alone, case-sensitively.
fn by_name(rel: &str) -> Named {
    if let Some(answer) = overridden(ext(rel)) {
        return answer;
    }
    match SupportLang::from_path(rel) {
        Some(carried) => carried_lang(carried).map_or(Named::Unread, Named::Read),
        None => Named::Unknown,
    }
}

/// Extensions where our answer is not ast-grep's: grammars it does not carry, languages read by
/// blanking, JavaScript read by the TypeScript grammars, and refusals.
fn overridden(ext: Option<&str>) -> Option<Named> {
    Some(match ext? {
        "bicep" => Named::Read(Lang::Bicep),
        // A `.tf` file is a Terraform module's: its blocks are addresses that resolve across the
        // directory. Every other HCL dialect is read by block type and labels.
        "tf" => Named::Read(Lang::Terraform),
        "sql" => Named::Read(Lang::Sql),
        "gql" | "graphql" => Named::Read(Lang::GraphQl),
        // A view (`.cshtml`) is Razor too: same directives, same `@functions`, one reader.
        "razor" | "cshtml" => Named::Read(Lang::Razor),
        "vue" => Named::Read(Lang::Vue),
        // ast-grep says JavaScript, a grammar we do not link; the TypeScript grammar reads it.
        "js" | "mjs" | "cjs" => Named::Read(Lang::TypeScript),
        // JSX needs the TSX grammar; the TypeScript one reads `<div/>` as a type assertion.
        "jsx" => Named::Read(Lang::Tsx),
        // Names ast-grep gives a language we read, which we do not read as it: Gradle scripts, variable
        // files that declare nothing, and extensions that are often another language. Reading any is a
        // widening with its own case.
        "kts" | "ktm" | "tfvars" | "nomad" | "workflow" | "bats" | "cgi" | "command" | "env" | "fcgi" | "ksh" | "tmux"
        | "tool" | "zsh" | "py3" | "pyi" | "bzl" | "bazel" => Named::Unread,
        _ => return None,
    })
}

/// One arm per language ast-grep carries; `None` for the ones we leave as files (L1).
fn carried_lang(carried: SupportLang) -> Option<Lang> {
    match carried {
        SupportLang::Bash => Some(Lang::Shell),
        SupportLang::CSharp => Some(Lang::CSharp),
        SupportLang::Dart => Some(Lang::Dart),
        SupportLang::Hcl => Some(Lang::Hcl),
        SupportLang::Java => Some(Lang::Java),
        SupportLang::Kotlin => Some(Lang::Kotlin),
        SupportLang::Python => Some(Lang::Python),
        SupportLang::Rust => Some(Lang::Rust),
        SupportLang::Swift => Some(Lang::Swift),
        SupportLang::Tsx => Some(Lang::Tsx),
        SupportLang::TypeScript => Some(Lang::TypeScript),
        SupportLang::Lua => Some(Lang::Lua),
        SupportLang::Go => Some(Lang::Go),
        SupportLang::C => Some(Lang::C),
        SupportLang::Cpp | SupportLang::Elixir | SupportLang::Php | SupportLang::Ruby
        | SupportLang::Scala => None,
        SupportLang::Css | SupportLang::Haskell | SupportLang::Html | SupportLang::JavaScript | SupportLang::Json
        | SupportLang::Markdown | SupportLang::Nix | SupportLang::Solidity | SupportLang::Yaml => None,
    }
}

/// The language that reads `rel`: its name first, then, when neither table knows the name and the
/// caller holds the text, the interpreter its shebang names.
pub fn detect(rel: &str, head: Option<&str>) -> Option<Lang> {
    match by_name(rel) {
        Named::Read(lang) => Some(lang),
        Named::Unread => None,
        Named::Unknown => head.and_then(shebang),
    }
}

/// How much of a file its shebang is looked for in: Linux reads no more of the line than this.
const SHEBANG_BYTES: u64 = 256;

/// `detect` for a file on disk: its name, and only for a name neither table knows, the shebang in
/// its first bytes, so a file is never read whole to learn what reads it.
pub fn detect_path(rel: &str, path: &std::path::Path) -> Option<Lang> {
    use std::io::Read as _;
    match by_name(rel) {
        Named::Read(lang) => Some(lang),
        Named::Unread => None,
        Named::Unknown => {
            let mut head = Vec::new();
            std::fs::File::open(path).ok()?.take(SHEBANG_BYTES).read_to_end(&mut head).ok()?;
            shebang(&String::from_utf8_lossy(&head))
        }
    }
}

/// `python3.11` is as common as `python3`, and `lua5.4` as `lua`: the version is digits and dots after the name.
fn versioned(program: &str, name: &str) -> bool {
    program.strip_prefix(name).is_some_and(|v| v.chars().all(|c| c.is_ascii_digit() || c == '.'))
}

/// `#!/bin/bash` and `#!/usr/bin/env -S python3` alike: the interpreter's file name, after `env`
/// and its flags when the line goes through `env`.
fn shebang(text: &str) -> Option<Lang> {
    let line = text.lines().next()?.strip_prefix("#!")?;
    let mut words = line.split_whitespace();
    let mut program = words.next()?.rsplit('/').next()?;
    if program == "env" {
        program = words.find(|w| !w.starts_with('-') && !w.contains('='))?;
    }
    match program {
        "sh" | "bash" | "dash" | "zsh" => Some(Lang::Shell),
        p if versioned(p, "python") => Some(Lang::Python),
        p if versioned(p, "lua") => Some(Lang::Lua),
        "node" | "deno" | "bun" | "ts-node" => Some(Lang::TypeScript),
        _ => None,
    }
}

impl Lang {
    pub const ALL: [Lang; 20] = [
        Lang::TypeScript, Lang::Tsx, Lang::Kotlin, Lang::Java, Lang::CSharp, Lang::Razor, Lang::Rust, Lang::Python,
        Lang::Dart, Lang::Swift, Lang::GraphQl, Lang::Sql, Lang::Bicep, Lang::Hcl, Lang::Terraform, Lang::Shell,
        Lang::Vue, Lang::Lua, Lang::Go, Lang::C,
    ];

    /// `None`: no grammar and no embedding reads this extension (L1). Such a file is indexed as a
    /// file and never parsed: every grammar returns a tree for any input, and a tree of the wrong
    /// language looks right. The name alone decides, so a deleted file reads as it did; `detect`
    /// adds the shebang for a caller holding the text.
    pub fn of(rel: &str) -> Option<Lang> {
        match by_name(rel) {
            Named::Read(lang) => Some(lang),
            Named::Unread | Named::Unknown => None,
        }
    }

    /// `None` for the embedded languages (Razor, Vue), which are read by blanking through another grammar.
    pub fn grammar(self) -> Option<Language> {
        let carried = match self {
            Lang::TypeScript => SupportLang::TypeScript,
            Lang::Tsx => SupportLang::Tsx,
            Lang::CSharp => SupportLang::CSharp,
            Lang::Kotlin => SupportLang::Kotlin,
            Lang::Java => SupportLang::Java,
            Lang::Rust => SupportLang::Rust,
            Lang::Python => SupportLang::Python,
            Lang::Hcl | Lang::Terraform => SupportLang::Hcl,
            Lang::Shell => SupportLang::Bash,
            Lang::Dart => SupportLang::Dart,
            Lang::Swift => SupportLang::Swift,
            Lang::Lua => SupportLang::Lua,
            Lang::Go => SupportLang::Go,
            Lang::C => SupportLang::C,
            // ast-grep carries none of these three; each keeps its own pinned crate.
            Lang::Sql => return Some(Language::new(tree_sitter_postgres::LANGUAGE)),
            Lang::GraphQl => return Some(Language::new(tree_sitter_graphql::LANGUAGE)),
            Lang::Bicep => return Some(Language::new(tree_sitter_bicep::LANGUAGE)),
            Lang::Razor | Lang::Vue => return None,
        };
        Some(carried.get_ts_language())
    }

    pub fn parse(self, src: &[u8]) -> Option<Tree> {
        let mut parser = Parser::new();
        parser.set_language(&self.grammar()?).ok()?;
        parser.parse(src, None)
    }

    pub fn family(self) -> Family {
        match self {
            Lang::TypeScript | Lang::Tsx => Family::TypeScript,
            Lang::CSharp | Lang::Razor => Family::DotNet,
            Lang::Kotlin | Lang::Java => Family::Jvm,
            Lang::Sql => Family::Sql,
            Lang::GraphQl => Family::GraphQl,
            Lang::Rust => Family::Rust,
            Lang::Python => Family::Python,
            Lang::Bicep => Family::Bicep,
            Lang::Hcl | Lang::Terraform => Family::Hcl,
            Lang::Shell => Family::Shell,
            Lang::Dart => Family::Dart,
            Lang::Swift => Family::Swift,
            Lang::Lua => Family::Lua,
            Lang::Go => Family::Go,
            Lang::C => Family::C,
            // A Vue script resolves names through the TypeScript resolver, and L10 puts no edge between families.
            Lang::Vue => Family::TypeScript,
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
/// grammars says so instead of producing a graph that is silently thinner than the tree. A script
/// whose shebang names a language is read, so it is not named.
pub fn files_only_note<'r>(root: &std::path::Path, rels: impl Iterator<Item = &'r str>) -> Option<String> {
    let mut by_ext = std::collections::BTreeMap::<String, usize>::new();
    for rel in rels {
        if detect_path(rel, &root.join(rel)).is_some() {
            continue;
        }
        let label = ext(rel).map(|e| format!(".{e}")).unwrap_or_else(|| "(no extension)".to_string());
        *by_ext.entry(label).or_default() += 1;
    }
    if by_ext.is_empty() {
        return None;
    }
    let mut parts: Vec<(String, usize)> = by_ext.into_iter().collect();
    parts.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let list = parts.iter().map(|(e, n)| format!("{n} {e}")).collect::<Vec<_>>().join(", ");
    Some(format!("code: no grammar reads {list} — indexed as files only"))
}

#[cfg(test)]
mod all_tests {
    use super::Lang;

    /// `ALL` feeds the reader table's state and manifest lookups, so a language missing from it is
    /// silently never registered. The exhaustive match fails to compile for a new variant.
    #[test]
    fn all_lists_every_language_once() {
        let slot = |l: Lang| match l {
            Lang::TypeScript => 0, Lang::Tsx => 1, Lang::Kotlin => 2, Lang::Java => 3, Lang::CSharp => 4, Lang::Razor => 5,
            Lang::Rust => 6, Lang::Python => 7, Lang::Dart => 8, Lang::Swift => 9, Lang::GraphQl => 10, Lang::Sql => 11,
            Lang::Bicep => 12, Lang::Hcl => 13, Lang::Terraform => 14, Lang::Shell => 15, Lang::Vue => 16, Lang::Lua => 17,
            Lang::Go => 18, Lang::C => 19,
        };
        let mut seen = [false; 20];
        for l in Lang::ALL {
            assert!(!std::mem::replace(&mut seen[slot(l)], true), "{l:?} listed twice");
        }
        assert!(seen.iter().all(|s| *s), "a language is missing from Lang::ALL");
    }
}
