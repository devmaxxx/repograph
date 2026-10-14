//! Edge cases of the code extractor, each on an inline source so the grammar's shape for the
//! construct is pinned by the assertion rather than by a shared fixture.

mod bicep;
mod c;
mod cpp;
mod csharp;
mod dart;
mod elixir;
mod go;
mod graphql;
mod hcl;
mod java;
mod jvm;
mod kotlin;
mod lua;
mod ocaml;
mod php;
mod python;
mod razor;
mod ruby;
mod rust;
mod scala;
mod shell;
mod sql;
mod swift;
mod typescript;
mod vue;

use super::CodeExtractor;
use crate::code::imports::Resolver;
use crate::model::{EdgeKind, Extraction, Extractor, NodeKind};

struct Repo {
    dir: tempfile::TempDir,
}

impl Repo {
    fn new(files: &[(&str, &str)]) -> Repo {
        let dir = tempfile::tempdir().unwrap();
        for (p, c) in files {
            let full = dir.path().join(p);
            std::fs::create_dir_all(full.parent().unwrap()).unwrap();
            std::fs::write(full, c).unwrap();
        }
        Repo { dir }
    }

    fn extract(&self, rel: &str, src: &str) -> Extraction {
        CodeExtractor::new(Resolver::new(self.dir.path(), &crate::config::Config::default()).unwrap()).extract(rel, src)
    }
}

fn extract(rel: &str, src: &str) -> Extraction {
    Repo::new(&[]).extract(rel, src)
}

fn ids(ex: &Extraction) -> Vec<&str> {
    ex.nodes.iter().map(|n| n.id.as_str()).collect()
}

fn edges(ex: &Extraction, kind: EdgeKind) -> Vec<(&str, &str, &str)> {
    ex.edges.iter().filter(|e| e.kind == kind).map(|e| (e.source.as_str(), e.target.as_str(), e.context.as_str())).collect()
}

fn declares(ex: &Extraction, name: &str) -> Option<String> {
    ex.edges.iter().find(|e| e.kind == EdgeKind::Declares && e.target.ends_with(&format!("::{name}"))).map(|e| e.context.clone())
}
// ---- the import resolver ----

fn resolver(files: &[(&str, &str)]) -> (Repo, Resolver) {
    let repo = Repo::new(files);
    let r = Resolver::new(repo.dir.path(), &crate::config::Config::default()).unwrap();
    (repo, r)
}

#[test]
fn package_exports_with_a_wildcard_subpath() {
    let (_repo, r) = resolver(&[
        ("packages/db/package.json", r#"{ "name": "@x/db", "exports": { ".": "./dist/index.js", "./*": { "types": "./dist/*.d.ts", "default": "./dist/*.js" } } }"#),
        ("packages/db/src/index.ts", ""),
        ("packages/db/src/seed/permissions.ts", ""),
        ("packages/ui/package.json", r#"{ "name": "@x/ui", "exports": { "./components/*": "./dist/components/ui/*.js" } }"#),
        ("packages/ui/src/components/ui/button.tsx", ""),
    ]);
    assert_eq!(r.resolve("apps/a.ts", "@x/db/seed/permissions").as_deref(), Some("packages/db/src/seed/permissions.ts"));
    assert_eq!(r.resolve("apps/a.ts", "@x/db").as_deref(), Some("packages/db/src/index.ts"));
    assert_eq!(r.resolve("apps/a.ts", "@x/ui/components/button").as_deref(), Some("packages/ui/src/components/ui/button.tsx"));
    assert_eq!(r.resolve("apps/a.ts", "@x/db/missing"), None);
}

#[test]
fn an_exact_export_wins_over_the_wildcard() {
    let (_repo, r) = resolver(&[
        ("packages/d/package.json", r#"{ "name": "@x/d", "exports": { "./time": "./dist/time/index.js", "./*": "./dist/*.js" } }"#),
        ("packages/d/src/time/index.ts", ""),
        ("packages/d/src/time.ts", ""),
    ]);
    assert_eq!(r.resolve("apps/a.ts", "@x/d/time").as_deref(), Some("packages/d/src/time/index.ts"));
}

#[test]
fn a_package_name_that_is_a_prefix_of_another_does_not_capture_it() {
    let (_repo, r) = resolver(&[
        ("packages/ui/package.json", r#"{ "name": "@x/ui", "exports": { ".": "./src/index.ts", "./*": "./src/*.ts" } }"#),
        ("packages/ui/src/index.ts", ""),
        ("packages/ui/src/kit.ts", ""),
        ("packages/ui-kit/package.json", r#"{ "name": "@x/ui-kit", "exports": { ".": "./src/index.ts" } }"#),
        ("packages/ui-kit/src/index.ts", ""),
    ]);
    assert_eq!(r.resolve("apps/a.ts", "@x/ui-kit").as_deref(), Some("packages/ui-kit/src/index.ts"));
    assert_eq!(r.resolve("apps/a.ts", "@x/ui/kit").as_deref(), Some("packages/ui/src/kit.ts"));
}

#[test]
fn jsonc_tsconfig_with_comments_urls_and_trailing_commas() {
    let (_repo, r) = resolver(&[
        ("tsconfig.json", "{\n  // comment\n  \"$schema\": \"https://json.schemastore.org/tsconfig\",\n  /* block\n  comment */\n  \"compilerOptions\": {\n    \"paths\": {\n      \"@/*\": [\"./src/*\",],\n    },\n  },\n}\n"),
        ("src/a.ts", ""),
    ]);
    assert_eq!(r.resolve("src/b.ts", "@/a").as_deref(), Some("src/a.ts"));
}

#[test]
fn tsconfig_paths_honour_base_url() {
    let (_repo, r) = resolver(&[
        ("tsconfig.json", r#"{ "compilerOptions": { "baseUrl": "./src", "paths": { "~/*": ["lib/*"] } } }"#),
        ("src/lib/util.ts", ""),
    ]);
    assert_eq!(r.resolve("src/app.ts", "~/util").as_deref(), Some("src/lib/util.ts"));
}

#[test]
fn a_file_beats_a_directory_index_of_the_same_name() {
    let (_repo, r) = resolver(&[("src/foo.ts", ""), ("src/foo/index.ts", ""), ("src/bar/index.tsx", "")]);
    assert_eq!(r.resolve("src/a.ts", "./foo").as_deref(), Some("src/foo.ts"));
    assert_eq!(r.resolve("src/a.ts", "./foo/index").as_deref(), Some("src/foo/index.ts"));
    assert_eq!(r.resolve("src/a.ts", "./bar").as_deref(), Some("src/bar/index.tsx"));
}

#[test]
fn non_source_specs_and_paths_outside_the_repo_are_none() {
    let (_repo, r) = resolver(&[("src/a.ts", ""), ("src/data.json", ""), ("src/s.css", ""), ("src/dist/x.ts", "")]);
    assert_eq!(r.resolve("src/a.ts", "./data.json"), None);
    assert_eq!(r.resolve("src/a.ts", "./s.css"), None);
    assert_eq!(r.resolve("src/a.ts", "../../../../etc/passwd"), None);
    assert_eq!(r.resolve("src/a.ts", "./dist/x"), None, "build output is not indexed");
    assert_eq!(r.resolve("src/a.ts", "./a.ts").as_deref(), Some("src/a.ts"), "an explicit extension still resolves");
}

#[test]
fn a_package_inside_node_modules_is_not_a_workspace_package() {
    let (_repo, r) = resolver(&[
        ("node_modules/zod/package.json", r#"{ "name": "zod", "exports": { ".": "./index.ts" } }"#),
        ("node_modules/zod/index.ts", ""),
        ("src/a.ts", ""),
    ]);
    assert_eq!(r.resolve("src/a.ts", "zod"), None);
}

#[test]
fn a_package_without_exports_falls_back_to_main_and_types() {
    let (_repo, r) = resolver(&[
        ("packages/m/package.json", r#"{ "name": "@x/m", "main": "./dist/index.js" }"#),
        ("packages/m/src/index.ts", ""),
        ("packages/t/package.json", r#"{ "name": "@x/t", "types": "./src/index.ts" }"#),
        ("packages/t/src/index.ts", ""),
    ]);
    assert_eq!(r.resolve("apps/a.ts", "@x/m").as_deref(), Some("packages/m/src/index.ts"));
    assert_eq!(r.resolve("apps/a.ts", "@x/t").as_deref(), Some("packages/t/src/index.ts"));
}

#[test]
fn a_malformed_package_json_or_tsconfig_does_not_abort_the_walk() {
    let (_repo, r) = resolver(&[
        ("packages/bad/package.json", "{ not json"),
        ("packages/bad/tsconfig.json", "{ also not json"),
        ("packages/ok/package.json", r#"{ "name": "@x/ok", "exports": { ".": "./src/index.ts" } }"#),
        ("packages/ok/src/index.ts", ""),
    ]);
    assert_eq!(r.resolve("apps/a.ts", "@x/ok").as_deref(), Some("packages/ok/src/index.ts"));
}

/// A development aid, not a test: prints the tree-sitter S-expression of `ZEROGRAPH_DUMP`, so a
/// case can be written against a grammar's real shape. `ZEROGRAPH_DUMP` is a path to a file, whose
/// extension picks the grammar; or inline source, named by `ZEROGRAPH_DUMP_AS` (`dump.ts` unless
/// set, `dump.tsx` under `ZEROGRAPH_DUMP_TSX`).
/// `ZEROGRAPH_DUMP='export * as ns from "./lib";' cargo test dump_tree -- --ignored --nocapture`
/// `ZEROGRAPH_DUMP=path/to/Order.cs cargo test dump_tree -- --ignored --nocapture`
/// With `ZEROGRAPH_DUMP_CAPTURES` set it also prints each capture of the language's two queries, so a
/// query is written against what it matches rather than against what it was meant to.
#[test]
#[ignore]
fn dump_tree() {
    let arg = crate::env("DUMP").expect("set ZEROGRAPH_DUMP to a file, or to the source to parse");
    let (rel, src) = match std::fs::read(&arg) {
        Ok(bytes) => (arg, bytes),
        Err(_) => {
            let named = crate::env("DUMP_AS");
            let rel = named.unwrap_or_else(|| if crate::env_os("DUMP_TSX").is_some() { "dump.tsx".into() } else { "dump.ts".into() });
            (rel, arg.into_bytes())
        }
    };
    let lang = crate::code::lang::Lang::of(&rel).unwrap_or_else(|| panic!("no grammar reads {rel}"));
    let tree = lang.parse(&src).unwrap_or_else(|| panic!("{lang:?} is read by blanking, not by a grammar of its own"));
    println!("{}", tree.root_node().to_sexp());
    if crate::env_os("DUMP_CAPTURES").is_some() {
        use streaming_iterator::StreamingIterator;
        for (part, compiled) in [("decl", crate::code::queries::decl(lang)), ("refs", crate::code::queries::refs(lang))] {
            let Some(c) = compiled else {
                println!("{lang:?} has no {part} query");
                continue;
            };
            let mut cursor = tree_sitter::QueryCursor::new();
            let mut matches = cursor.matches(&c.query, tree.root_node(), &src[..]);
            while let Some(m) = matches.next() {
                for cap in m.captures() {
                    let at = cap.node.start_position();
                    let text = cap.node.utf8_text(&src).unwrap_or("");
                    println!("{part} #{} @{} {}:{} {text:?}", m.pattern_index, c.query.capture_names()[cap.index as usize], at.row + 1, at.column);
                }
            }
        }
    }
}

// ---- the grammar table (L1) ----
// `lang.rs` holds no test module of its own: `tests/parse_census.rs` compiles that file by path,
// where these cases would need the crate's model.

#[test]
fn bicep_terraform_bake_and_shell_each_pick_their_grammar() {
    use crate::code::lang::{Family, Lang};
    for (rel, lang) in [
        ("sample/stack.bicep", Lang::Bicep),
        ("infra/staging/main.tf", Lang::Terraform),
        ("docker-bake.hcl", Lang::Hcl),
        ("bench/probe/reset.sh", Lang::Shell),
        ("tools/x.bash", Lang::Shell),
    ] {
        assert_eq!(Lang::of(rel), Some(lang), "{rel}");
        assert!(lang.parse(b"").is_some(), "{rel} has a grammar");
    }
    // A variable-values file assigns and declares nothing an address names.
    assert_eq!(Lang::of("infra/staging/terraform.tfvars"), None);
    assert_eq!((Lang::Bicep.family(), Lang::Hcl.family(), Lang::Shell.family()), (Family::Bicep, Family::Hcl, Family::Shell));
    assert_eq!(Lang::Terraform.family(), Family::Hcl);
}

#[test]
fn a_globbed_file_no_grammar_reads_is_a_file_and_nothing_else() {
    // A log is no family's language, so no 0.6.0 grammar can turn this fixture into a real one.
    let ex = extract("svc/boot.log", "boot ok\nRun() called\n");
    assert_eq!(ids(&ex), vec!["file:svc/boot.log"]);
    assert!(ex.nodes.iter().all(|n| n.kind == NodeKind::File));
    assert!(ex.edges.is_empty(), "{:?}", ex.edges);
}

#[test]
fn javascript_keeps_the_typescript_grammar() {
    let ex = extract("web/a.js", "export function refund(id) { return id; }\n");
    assert!(ids(&ex).contains(&"sym:web/a.js::refund"), "{:?}", ids(&ex));
}

#[test]
fn each_typescript_grammar_reaches_the_typescript_extractor_through_the_table() {
    for (rel, src) in [("a.ts", "export function f() {}\n"), ("b.tsx", "export const V = () => <p/>;\n"), ("c.mjs", "export function g() {}\n")] {
        let ex = extract(rel, src);
        assert!(ids(&ex).iter().any(|i| i.starts_with(&format!("sym:{rel}::"))), "{rel}: {:?}", ids(&ex));
    }
}

#[test]
fn the_typescript_family_is_read_by_its_two_grammars_and_an_unknown_extension_by_none() {
    use crate::code::lang::{Family, Lang};
    for rel in ["a.ts", "a.mts", "a.cts", "a.js", "a.mjs", "a.cjs", "types/a.d.ts"] {
        assert_eq!(Lang::of(rel), Some(Lang::TypeScript), "{rel}");
    }
    for rel in ["ui/App.tsx", "ui/App.jsx"] {
        assert_eq!(Lang::of(rel), Some(Lang::Tsx), "{rel}");
    }
    for rel in ["Makefile", "docs/a.md", "ci.yaml", "ts"] {
        assert_eq!(Lang::of(rel), None, "{rel}");
    }
    assert_eq!((Lang::TypeScript.family(), Lang::Tsx.family()), (Family::TypeScript, Family::TypeScript));
    assert!(Lang::Tsx.parse(b"export const V = () => <div/>;\n").is_some_and(|t| !t.root_node().has_error()));
}

#[test]
fn the_file_node_every_language_writes_is_the_one_an_unread_file_always_got() {
    let mut ex = Extraction::default();
    crate::code::lang::file_node("svc/Program.cs", &mut ex);
    assert_eq!(ex.nodes.len(), 1);
    let n = &ex.nodes[0];
    assert_eq!((n.id.as_str(), n.label.as_str(), n.body.as_str(), n.file.as_str(), n.line),
        ("file:svc/Program.cs", "svc/Program.cs", "", "svc/Program.cs", 1));
    assert_eq!(n.kind, NodeKind::File);
    assert!(ex.edges.is_empty());
}

#[test]
fn the_note_names_each_extension_once_with_its_count() {
    use crate::code::lang::files_only_note;
    // An empty root: the note reads a shebang under it, never under the process's directory.
    let root = tempfile::tempdir().unwrap();
    let rels = ["a/B.log", "a/C.log", "b/x.ts", "Makefile"];
    assert_eq!(files_only_note(root.path(), rels.into_iter()).as_deref(),
        Some("code: no grammar reads 2 .log, 1 (no extension) — indexed as files only"));
    assert_eq!(files_only_note(root.path(), ["b/x.ts", "c/y.tsx", "d/z.mjs"].into_iter()), None);
}

#[test]
fn kotlin_and_java_are_one_family_and_a_gradle_script_is_not_read() {
    use crate::code::lang::{Family, Lang};
    assert_eq!(Lang::of("mobile/shared/src/commonMain/kotlin/a/Tokens.kt"), Some(Lang::Kotlin));
    assert_eq!(Lang::of("android/app/src/main/java/a/Checkout.java"), Some(Lang::Java));
    // Every .kts the census found is a Gradle script, and spec "### Kotlin" does not read them.
    assert_eq!(Lang::of("mobile/build.gradle.kts"), None);
    assert_eq!(Lang::of("tools/release.main.kts"), None);
    assert_eq!(Lang::Kotlin.family(), Family::Jvm);
    assert_eq!(Lang::Java.family(), Family::Jvm);
}

#[test]
fn both_jvm_grammars_parse_a_clean_file_without_an_error_node() {
    use crate::code::lang::Lang;
    for (lang, src) in [
        (Lang::Kotlin, "package a\n\nclass A(private val b: B) {\n    fun f() = b.g()\n}\n"),
        (Lang::Java, "package a;\n\npublic class A {\n    private B b;\n    int f() { return b.g(); }\n}\n"),
    ] {
        assert!(lang.grammar().is_some(), "{lang:?} has a grammar");
        let tree = lang.parse(src.as_bytes()).expect("a tree");
        assert!(!tree.root_node().has_error(), "{lang:?}: {}", tree.root_node().to_sexp());
    }
}

#[test]
fn a_gradle_script_under_a_glob_is_a_file_and_nothing_else() {
    let ex = extract("mobile/build.gradle.kts", "plugins {\n    kotlin(\"multiplatform\")\n}\nfun helper() = 1\n");
    assert_eq!(ids(&ex), vec!["file:mobile/build.gradle.kts"]);
    assert!(ex.edges.is_empty(), "{:?}", ex.edges);
}

// `ext` is case-sensitive by L1's design: an uppercase extension is a file, and the note says so.
#[test]
fn an_uppercase_extension_is_a_file_named_by_the_note() {
    use crate::code::lang::{files_only_note, Lang};
    assert_eq!(Lang::of("a/Legacy.JAVA"), None);
    let ex = extract("a/Legacy.JAVA", "package a;\n\npublic class Legacy {}\n");
    assert_eq!(ids(&ex), vec!["file:a/Legacy.JAVA"]);
    let root = tempfile::tempdir().unwrap();
    assert!(files_only_note(root.path(), ["a/Legacy.JAVA"].into_iter()).is_some_and(|n| n.contains(".JAVA")));
}

// ---- detection: the name, then ast-grep's table, then the shebang (L2) ----

#[test]
fn a_file_no_table_names_is_read_by_the_interpreter_its_shebang_names() {
    use crate::code::lang::{detect, Lang};
    for (head, lang) in [
        ("#!/bin/sh\n", Lang::Shell),
        ("#!/bin/bash\n", Lang::Shell),
        ("#!/bin/dash\n", Lang::Shell),
        ("#!/bin/zsh\n", Lang::Shell),
        ("#! /bin/bash -eu\n", Lang::Shell),
        ("#!/usr/bin/env bash\n", Lang::Shell),
        ("#!/usr/bin/env -S bash -euo pipefail\n", Lang::Shell),
        ("#!/usr/bin/python\n", Lang::Python),
        ("#!/usr/bin/python2\n", Lang::Python),
        ("#!/usr/local/bin/python3\n", Lang::Python),
        ("#!/usr/bin/env python3\n", Lang::Python),
        ("#!/usr/bin/python3.11\n", Lang::Python),
        ("#!/usr/bin/env python3.9 -u\n", Lang::Python),
        ("#!/usr/bin/env PYTHONPATH=src python3 -u\n", Lang::Python),
        ("#!/usr/bin/env node\n", Lang::TypeScript),
        ("#!/usr/bin/env -S deno run --allow-read\n", Lang::TypeScript),
        ("#!/usr/bin/env bun\n", Lang::TypeScript),
        ("#!/usr/bin/env ts-node\n", Lang::TypeScript),
    ] {
        assert_eq!(detect("bin/deploy", Some(head)), Some(lang), "{head:?}");
    }
}

#[test]
fn a_shebang_names_no_language_when_it_is_absent_late_or_an_interpreter_we_do_not_read() {
    use crate::code::lang::detect;
    for head in [
        "",
        "echo hi\n",
        "\n#!/bin/bash\n",
        "# !/bin/bash\n",
        "#!/usr/bin/env ruby\n",
        "#!/usr/bin/perl -w\n",
        "#!/usr/bin/env\n",
        "#!/usr/bin/env -S\n",
        "#!\n",
    ] {
        assert_eq!(detect("bin/deploy", Some(head)), None, "{head:?}");
    }
    // A caller without the text, as a deleted file's path is read, has only the name to go by.
    assert_eq!(detect("bin/deploy", None), None);
}

#[test]
fn a_name_either_table_knows_decides_whatever_the_shebang_says() {
    use crate::code::lang::{detect, Lang};
    assert_eq!(detect("tools/x.py", Some("#!/bin/bash\n")), Some(Lang::Python));
    assert_eq!(detect("tools/x.sh", Some("#!/usr/bin/env python3\n")), Some(Lang::Shell));
    // ast-grep reads these as languages we leave as files, and that answer is final.
    for rel in ["ci.yaml", "README.md", "a.hs", "page.html"] {
        assert_eq!(detect(rel, Some("#!/bin/bash\n")), None, "{rel}");
    }
}

#[test]
fn every_refused_extension_is_a_file_even_under_a_shebang() {
    use crate::code::lang::{detect, Lang};
    for ext in ["kts", "ktm", "tfvars", "nomad", "workflow", "bats", "cgi", "command", "env", "fcgi", "ksh", "tmux", "tool", "zsh", "py3", "pyi", "bzl", "bazel"] {
        let rel = format!("a/b.{ext}");
        assert_eq!(Lang::of(&rel), None, "{rel}");
        assert_eq!(detect(&rel, Some("#!/bin/bash\n")), None, "{rel}");
    }
}

#[test]
fn javascript_is_read_by_the_typescript_grammars_and_not_by_the_one_ast_grep_names() {
    use crate::code::lang::{detect, Lang};
    for rel in ["a.js", "a.mjs", "a.cjs"] {
        assert_eq!(detect(rel, None), Some(Lang::TypeScript), "{rel}");
    }
    assert_eq!(detect("a.jsx", None), Some(Lang::Tsx));
}

#[test]
fn a_script_with_no_extension_is_read_by_its_shebang_from_the_extractor() {
    let ex = extract("bin/deploy", "#!/usr/bin/env bash\ngreet() {\n  echo hi\n}\n");
    assert_eq!(ids(&ex), vec!["file:bin/deploy", "sym:bin/deploy::greet"]);
    let ex = extract("bin/notes", "greet() {\n  echo hi\n}\n");
    assert_eq!(ids(&ex), vec!["file:bin/notes"]);
}

#[test]
fn every_language_names_its_grammar_once_and_the_two_embedded_ones_none() {
    use crate::code::lang::Lang;
    for lang in Lang::ALL {
        match lang {
            Lang::Razor | Lang::Vue => assert!(lang.grammar().is_none(), "{lang:?}"),
            _ => assert!(lang.parse(b"").is_some(), "{lang:?}"),
        }
    }
}

#[test]
fn a_local_hides_a_path_through_it_whichever_separator_joins_the_path() {
    use crate::code::engine::{facts, File};
    use crate::code::lang::Lang;
    use crate::code::queries;
    // A dotted name and a slashed one start at the same segment, so a local `a` hides `a.b.C` and
    // `a/b` alike in every language, and hides no name that only starts with the same letters.
    let src = "local function f(a)\n  return a\nend\n";
    let tree = Lang::Lua.parse(src.as_bytes()).unwrap();
    let file = File { lang: Lang::Lua, rel: "f.lua", text: src, src, tree: Some(&tree) };
    let spec = crate::code::spec::spec(Lang::Lua);
    let facts = facts(spec, queries::decl(Lang::Lua).unwrap(), queries::refs(Lang::Lua), &file);
    let inside = src.find("return").unwrap();
    for name in ["a", "a.b.C", "a/b", "a.b/c"] {
        assert!(facts.locals.shadows(name, inside, None), "{name}");
    }
    for name in ["ab.C", "b.a", "C"] {
        assert!(!facts.locals.shadows(name, inside, None), "{name}");
    }
    assert!(!facts.locals.shadows("a.b.C", src.len(), None), "past the function's body");
}
