//! Kotlin extraction on inline sources, so the grammar's shape is pinned by the assertion.

use std::collections::BTreeSet;

use crate::code::jvm::fixture::{edges, ids, one, Repo};
use crate::model::EdgeKind;

const TOKENS: &str = "package app

public class SessionTokens(private val store: Store, name: String, val onDone: () -> Unit) {
    val cached: Store? = null
    fun read(): String = store.get()
    companion object {
        fun empty(): SessionTokens = TODO()
    }
    class Pair(val a: String) {
        fun swap() {}
    }
}

object Registry {
    val size = 1
}

fun interface Source {
    fun fetch(): String
}

enum class Kind {
    A, B;
    fun flip() {}
}

fun String.shout(): String = uppercase()

val String.loud: String get() = this

private fun helper() = 1

internal val limit = 2

typealias Key = String
";

#[test]
fn classes_objects_members_and_top_level_names_are_declared() {
    let ex = one("app/Tokens.kt", TOKENS);
    for id in [
        "file:app/Tokens.kt",
        "sym:app/Tokens.kt::SessionTokens",
        "sym:app/Tokens.kt::SessionTokens.store",
        "sym:app/Tokens.kt::SessionTokens.onDone",
        "sym:app/Tokens.kt::SessionTokens.cached",
        "sym:app/Tokens.kt::SessionTokens.read",
        "sym:app/Tokens.kt::SessionTokens.empty",
        "sym:app/Tokens.kt::SessionTokens.Pair",
        "sym:app/Tokens.kt::SessionTokens.Pair.a",
        "sym:app/Tokens.kt::SessionTokens.Pair.swap",
        "sym:app/Tokens.kt::Registry",
        "sym:app/Tokens.kt::Registry.size",
        "sym:app/Tokens.kt::Source",
        "sym:app/Tokens.kt::Source.fetch",
        "sym:app/Tokens.kt::Kind",
        "sym:app/Tokens.kt::Kind.flip",
        "sym:app/Tokens.kt::shout",
        "sym:app/Tokens.kt::loud",
        "sym:app/Tokens.kt::helper",
        "sym:app/Tokens.kt::limit",
        "sym:app/Tokens.kt::Key",
    ] {
        assert!(ids(&ex).contains(&id), "{id} missing from {:?}", ids(&ex));
    }
    // A plain constructor parameter is not a property, an enum entry is not a declaration, and a
    // companion's members are the class's own.
    for absent in ["SessionTokens.name", "Kind.A", "Companion"] {
        assert!(!ids(&ex).iter().any(|i| i.ends_with(absent) || i.contains(&format!("{absent}."))), "{absent} in {:?}", ids(&ex));
    }
}

#[test]
fn members_are_declared_by_their_class_and_top_level_names_by_the_file() {
    let ex = one("app/Tokens.kt", TOKENS);
    let declares = edges(&ex, EdgeKind::Declares);
    assert!(declares.contains(&("file:app/Tokens.kt", "sym:app/Tokens.kt::SessionTokens", "export")));
    assert!(declares.contains(&("sym:app/Tokens.kt::SessionTokens", "sym:app/Tokens.kt::SessionTokens.read", "export")));
    assert!(declares.contains(&("sym:app/Tokens.kt::SessionTokens", "sym:app/Tokens.kt::SessionTokens.empty", "export")));
    assert!(declares.contains(&("sym:app/Tokens.kt::SessionTokens", "sym:app/Tokens.kt::SessionTokens.store", "")));
    assert!(declares.contains(&("sym:app/Tokens.kt::SessionTokens.Pair", "sym:app/Tokens.kt::SessionTokens.Pair.swap", "export")));
}

#[test]
fn private_and_protected_are_not_exported_and_internal_is() {
    let ex = one("app/V.kt", "package app\n\ninternal class Open {\n    protected fun p() {}\n}\n\nprivate class Hidden\n\nprivate fun helper() = 1\n");
    let declares = edges(&ex, EdgeKind::Declares);
    assert!(declares.contains(&("file:app/V.kt", "sym:app/V.kt::Open", "export")));
    assert!(declares.contains(&("sym:app/V.kt::Open", "sym:app/V.kt::Open.p", "")));
    assert!(declares.contains(&("file:app/V.kt", "sym:app/V.kt::Hidden", "")));
    assert!(declares.contains(&("file:app/V.kt", "sym:app/V.kt::helper", "")));
}

#[test]
fn a_supertype_declared_in_the_same_file_is_extended_and_a_nested_one_first() {
    let src = "package app\n\nopen class Base\n\ninterface Wipe\n\nclass Handler : Base(), Wipe\n\nclass Outer {\n    open class Base\n    class Inner : Base()\n}\n";
    let ex = one("app/W.kt", src);
    let extends = edges(&ex, EdgeKind::Extends);
    assert!(extends.contains(&("sym:app/W.kt::Handler", "sym:app/W.kt::Base", "")), "{extends:?}");
    assert!(extends.contains(&("sym:app/W.kt::Handler", "sym:app/W.kt::Wipe", "")), "{extends:?}");
    assert!(extends.contains(&("sym:app/W.kt::Outer.Inner", "sym:app/W.kt::Outer.Base", "")), "{extends:?}");
    assert!(!extends.contains(&("sym:app/W.kt::Outer.Inner", "sym:app/W.kt::Base", "")), "{extends:?}");
}

#[test]
fn an_expect_class_and_its_actual_each_keep_their_own_id() {
    let common = one("common/Clock.kt", "package app\n\nexpect class Clock {\n    fun now(): Long\n}\n");
    let jvm = one("jvm/Clock.jvm.kt", "package app\n\nactual class Clock {\n    actual fun now(): Long = 0\n}\n");
    assert!(ids(&common).contains(&"sym:common/Clock.kt::Clock.now"));
    assert!(ids(&jvm).contains(&"sym:jvm/Clock.jvm.kt::Clock.now"));
}

#[test]
fn a_declaration_spans_its_lines_and_its_body_is_its_doc_and_its_name_line() {
    let ex = one("app/Tokens.kt", TOKENS);
    let n = ex.nodes.iter().find(|n| n.id == "sym:app/Tokens.kt::SessionTokens").unwrap();
    assert_eq!((n.line, n.end), (3, 12));
    let src = "package app\n\n/** Signs the envelope. FR-VIS-47 */\n@Serializable\nclass Signer\n";
    let ex = one("app/Signer.kt", src);
    let n = ex.nodes.iter().find(|n| n.id == "sym:app/Signer.kt::Signer").unwrap();
    assert_eq!(n.body, "Signs the envelope. FR-VIS-47\nclass Signer");
}

#[test]
fn the_header_names_exactly_the_top_level_names_the_file_declares() {
    let ex = one("app/Tokens.kt", TOKENS);
    let declared: BTreeSet<String> = edges(&ex, EdgeKind::Declares).into_iter()
        .filter(|(s, _, _)| *s == "file:app/Tokens.kt")
        .filter_map(|(_, t, _)| t.strip_prefix("sym:app/Tokens.kt::"))
        .map(str::to_string)
        .collect();
    let h = super::header(TOKENS);
    assert_eq!(h.scope, vec!["app".to_string()]);
    // The index resolves by `top`; a name in it that no `Declares` edge holds would resolve to an id no node has.
    assert_eq!(h.top, declared);
    assert_eq!(super::header("class NoPackage\n").scope, Vec::<String>::new());
}

#[test]
fn a_bom_and_crlf_kotlin_file_keeps_its_package() {
    let src = "\u{feff}package app\r\n\r\nclass Crlf {\r\n    fun go() {}\r\n}\r\n";
    assert_eq!(super::header(src).scope, vec!["app".to_string()]);
    let ex = one("app/Crlf.kt", src);
    let n = ex.nodes.iter().find(|n| n.id == "sym:app/Crlf.kt::Crlf.go").expect("Crlf.go declared");
    assert!(!n.body.contains('\r'), "{:?}", n.body);
}

#[test]
fn a_file_annotation_does_not_hide_the_package() {
    let src = "@file:JvmName(\"Tokens\")\npackage app\n\nclass A\n";
    assert_eq!(super::header(src).scope, vec!["app".to_string()]);
    assert!(ids(&one("app/A.kt", src)).contains(&"sym:app/A.kt::A"));
}

const NETWORK: &str = "package pl.crm.network\n\nopen class SessionTokens {\n    fun read(): String = \"\"\n}\n";

#[test]
fn an_import_names_the_declaring_file_and_its_supertype_resolves() {
    let repo = Repo::new(&[
        ("network/Tokens.kt", NETWORK),
        ("sync/Use.kt", "package pl.crm.sync\n\nimport pl.crm.network.SessionTokens\n\nclass Use : SessionTokens()\n"),
    ]);
    let ex = repo.extract("sync/Use.kt");
    assert!(edges(&ex, EdgeKind::Imports).contains(&("file:sync/Use.kt", "file:network/Tokens.kt", "SessionTokens")), "{:?}", ex.edges);
    assert!(edges(&ex, EdgeKind::Extends).contains(&("sym:sync/Use.kt::Use", "sym:network/Tokens.kt::SessionTokens", "")), "{:?}", ex.edges);
}

#[test]
fn the_same_package_needs_no_import_and_a_star_resolves_at_its_use_without_an_imports_edge() {
    let repo = Repo::new(&[
        ("network/Tokens.kt", NETWORK),
        ("network/Same.kt", "package pl.crm.network\n\nclass Same : SessionTokens()\n"),
        ("sync/Star.kt", "package pl.crm.sync\n\nimport pl.crm.network.*\n\nclass Star : SessionTokens()\n"),
    ]);
    let same = repo.extract("network/Same.kt");
    assert!(edges(&same, EdgeKind::Extends).contains(&("sym:network/Same.kt::Same", "sym:network/Tokens.kt::SessionTokens", "")), "{:?}", same.edges);
    let star = repo.extract("sync/Star.kt");
    assert!(edges(&star, EdgeKind::Extends).contains(&("sym:sync/Star.kt::Star", "sym:network/Tokens.kt::SessionTokens", "")), "{:?}", star.edges);
    assert!(edges(&star, EdgeKind::Imports).is_empty(), "{:?}", star.edges);
}

#[test]
fn an_alias_import_resolves_by_the_alias_and_names_the_declared_type() {
    let repo = Repo::new(&[
        ("network/Tokens.kt", NETWORK),
        ("sync/Alias.kt", "package pl.crm.sync\n\nimport pl.crm.network.SessionTokens as Tokens\n\nclass Alias : Tokens()\n"),
    ]);
    let ex = repo.extract("sync/Alias.kt");
    assert!(edges(&ex, EdgeKind::Imports).contains(&("file:sync/Alias.kt", "file:network/Tokens.kt", "SessionTokens")), "{:?}", ex.edges);
    assert!(edges(&ex, EdgeKind::Extends).contains(&("sym:sync/Alias.kt::Alias", "sym:network/Tokens.kt::SessionTokens", "")), "{:?}", ex.edges);
}

#[test]
fn an_expect_class_resolves_to_every_file_that_declares_it() {
    let repo = Repo::new(&[
        ("common/Files.kt", "package app.storage\n\nexpect open class ReplicaFiles\n"),
        ("jvm/Files.jvm.kt", "package app.storage\n\nactual open class ReplicaFiles\n"),
        ("sync/Local.kt", "package app.sync\n\nimport app.storage.ReplicaFiles\n\nclass Local : ReplicaFiles()\n"),
    ]);
    let ex = repo.extract("sync/Local.kt");
    let imports = edges(&ex, EdgeKind::Imports);
    let extends = edges(&ex, EdgeKind::Extends);
    for file in ["common/Files.kt", "jvm/Files.jvm.kt"] {
        assert!(imports.contains(&("file:sync/Local.kt", format!("file:{file}").as_str(), "ReplicaFiles")), "{imports:?}");
        assert!(extends.contains(&("sym:sync/Local.kt::Local", format!("sym:{file}::ReplicaFiles").as_str(), "")), "{extends:?}");
    }
}

#[test]
fn a_supertype_outside_the_repository_never_walks_into_a_star_or_a_package() {
    let repo = Repo::new(&[
        ("shop/Color.kt", "package shop\n\nenum class Color { RED }\n"),
        ("app/Err.kt", "package app\n\nimport shop.Color.*\n\nclass Err : Exception()\n"),
        ("app/Di.kt", "package app\n\nval network = 1\n"),
        ("app/network/Fail.kt", "package app.network\n\nclass Fail : Exception()\n"),
    ]);
    for rel in ["app/Err.kt", "app/network/Fail.kt"] {
        let ex = repo.extract(rel);
        assert!(edges(&ex, EdgeKind::Extends).is_empty(), "{rel}: {:?}", ex.edges);
    }
}

#[test]
fn an_import_under_a_top_level_value_named_like_its_package_names_no_file() {
    let repo = Repo::new(&[
        ("app/Di.kt", "package app\n\nval network = 1\n"),
        ("app/network/Use.kt", "package app.network\n\nimport app.network.databinding.MainBinding\n\nclass Use : MainBinding()\n"),
    ]);
    let ex = repo.extract("app/network/Use.kt");
    assert!(edges(&ex, EdgeKind::Imports).is_empty(), "{:?}", ex.edges);
    assert!(edges(&ex, EdgeKind::Extends).is_empty(), "{:?}", ex.edges);
}

#[test]
fn a_class_imported_through_its_companion_is_not_guessed_at_the_companion_path() {
    let repo = Repo::new(&[
        ("shop/Outer.kt", "package shop\n\nclass Outer {\n    companion object {\n        open class Builder\n    }\n}\n"),
        ("app/B.kt", "package app\n\nimport shop.Outer.Companion.Builder\n\nclass B : Builder()\n"),
    ]);
    let ex = repo.extract("app/B.kt");
    assert!(!ex.edges.iter().any(|e| e.target.contains("Companion")), "{:?}", ex.edges);
}

#[test]
fn a_member_and_a_top_level_function_import_each_name_their_file() {
    let repo = Repo::new(&[
        ("shop/Keys.kt", "package shop\n\nobject Keys {\n    const val TOKEN = \"t\"\n}\n"),
        ("shop/Format.kt", "package shop\n\nfun format(s: String) = s\n"),
        ("app/Use.kt", "package app\n\nimport shop.Keys.TOKEN\nimport shop.format\n\nclass Use\n"),
    ]);
    let ex = repo.extract("app/Use.kt");
    let imports = edges(&ex, EdgeKind::Imports);
    assert!(imports.contains(&("file:app/Use.kt", "file:shop/Keys.kt", "Keys")), "{imports:?}");
    assert!(imports.contains(&("file:app/Use.kt", "file:shop/Format.kt", "format")), "{imports:?}");
}

#[test]
fn two_imports_binding_one_name_resolve_it_to_nothing() {
    let repo = Repo::new(&[
        ("a/Base.kt", "package a\n\nopen class Base\n"),
        ("b/Base.kt", "package b\n\nopen class Base\n"),
        ("app/X.kt", "package app\n\nimport a.Base\nimport b.Base\n\nclass X : Base()\n"),
    ]);
    let ex = repo.extract("app/X.kt");
    assert!(edges(&ex, EdgeKind::Extends).is_empty(), "{:?}", ex.edges);
}

#[test]
fn a_one_line_object_declares_itself_and_its_members() {
    let src = "package app\n\nobject Keys { fun token() = 1 }\n\nobject Names { val TOKEN = 1 }\n\nclass Outer {\n    object Inner { fun size() = 2 }\n}\n";
    let ex = one("app/K.kt", src);
    for id in ["sym:app/K.kt::Keys", "sym:app/K.kt::Keys.token", "sym:app/K.kt::Names", "sym:app/K.kt::Names.TOKEN", "sym:app/K.kt::Outer.Inner", "sym:app/K.kt::Outer.Inner.size"] {
        assert!(ids(&ex).contains(&id), "{id} missing from {:?}", ids(&ex));
    }
}
