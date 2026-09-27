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

const WIPE: &str = "package app

class ReplicaFiles {
    fun delete() {}
}

object Registry {
    fun reset() {}
}

fun helper(n: Int) = n

class RemoteWipeHandler(private val files: ReplicaFiles) {
    fun execute() {
        files.delete()
        this.files.delete()
        Registry.reset()
        helper(1)
        execute()
    }

    fun locals(given: ReplicaFiles) {
        val typed: ReplicaFiles = given
        val built = ReplicaFiles()
        given.delete()
        typed.delete()
        built.delete()
        fun inner() { helper(2) }
    }

    companion object {
        fun make(): RemoteWipeHandler = TODO()
    }
}

class Caller {
    fun go() { RemoteWipeHandler.make() }
}
";

fn calls_from<'a>(ex: &'a crate::model::Extraction, from: &str) -> Vec<&'a str> {
    edges(ex, EdgeKind::Calls).into_iter().filter(|(s, _, _)| *s == from).map(|(_, t, _)| t).collect()
}

#[test]
fn calls_through_a_typed_property_an_object_and_a_top_level_function_are_edges() {
    let ex = one("app/Wipe.kt", WIPE);
    let got = calls_from(&ex, "sym:app/Wipe.kt::RemoteWipeHandler.execute");
    for to in ["sym:app/Wipe.kt::ReplicaFiles.delete", "sym:app/Wipe.kt::Registry.reset", "sym:app/Wipe.kt::helper"] {
        assert!(got.contains(&to), "{to} missing from {got:?}");
    }
    assert!(!got.contains(&"sym:app/Wipe.kt::RemoteWipeHandler.execute"), "a recursive call is dropped: {got:?}");
    assert!(calls_from(&ex, "sym:app/Wipe.kt::Caller.go").contains(&"sym:app/Wipe.kt::RemoteWipeHandler.make"), "{:?}", ex.edges);
}

#[test]
fn a_parameter_a_typed_local_and_a_constructed_local_carry_their_type() {
    let ex = one("app/Wipe.kt", WIPE);
    let got = calls_from(&ex, "sym:app/Wipe.kt::RemoteWipeHandler.locals");
    assert!(got.contains(&"sym:app/Wipe.kt::ReplicaFiles.delete"), "{got:?}");
    assert!(got.contains(&"sym:app/Wipe.kt::ReplicaFiles"), "constructing a class is a call to it: {got:?}");
    // A local `fun` is not a symbol, so its call belongs to the function it sits in.
    assert!(got.contains(&"sym:app/Wipe.kt::helper"), "{got:?}");
    assert!(!edges(&ex, EdgeKind::Calls).iter().any(|(s, _, _)| s.ends_with("inner")), "{:?}", ex.edges);
}

#[test]
fn calls_resolve_through_an_import_a_star_and_every_actual() {
    let repo = Repo::new(&[
        ("network/Tokens.kt", "package pl.crm.network\n\nclass SessionTokens {\n    fun read(): String = \"\"\n}\n\nfun tokenOf(): SessionTokens = SessionTokens()\n"),
        ("common/Files.kt", "package pl.crm.storage\n\nexpect class ReplicaFiles {\n    fun delete()\n}\n"),
        ("jvm/Files.jvm.kt", "package pl.crm.storage\n\nactual class ReplicaFiles {\n    actual fun delete() {}\n}\n"),
        ("sync/Use.kt", "package pl.crm.sync\n\nimport pl.crm.network.SessionTokens\nimport pl.crm.network.tokenOf\nimport pl.crm.storage.*\n\nclass Use(private val t: SessionTokens, private val files: ReplicaFiles) {\n    fun go() {\n        t.read()\n        tokenOf()\n        files.delete()\n    }\n}\n"),
    ]);
    let ex = repo.extract("sync/Use.kt");
    let got = calls_from(&ex, "sym:sync/Use.kt::Use.go");
    for to in [
        "sym:network/Tokens.kt::SessionTokens.read",
        "sym:network/Tokens.kt::tokenOf",
        "sym:common/Files.kt::ReplicaFiles.delete",
        "sym:jvm/Files.jvm.kt::ReplicaFiles.delete",
    ] {
        assert!(got.contains(&to), "{to} missing from {got:?}");
    }
}

#[test]
fn a_requirement_cited_in_a_comment_or_a_string_is_a_reference_from_its_declaration() {
    let src = "// ADR-001 governs this file\npackage app\n\nobject Jcs {\n    // FR-VIS-47: canonical form before signing\n    fun canon() {\n        val name = \"ADR-022\"\n    }\n}\n";
    let ex = one("app/Jcs.kt", src);
    let refs = edges(&ex, EdgeKind::References);
    assert!(refs.iter().any(|(s, t, c)| *s == "file:app/Jcs.kt" && t.contains("ADR-001") && *c == "comment"), "{refs:?}");
    assert!(refs.iter().any(|(s, t, c)| *s == "sym:app/Jcs.kt::Jcs" && t.contains("FR-VIS-47") && *c == "comment"), "{refs:?}");
    assert!(refs.iter().any(|(s, t, c)| *s == "sym:app/Jcs.kt::Jcs.canon" && t.contains("ADR-022") && *c == "string"), "{refs:?}");
}

#[test]
fn an_id_in_a_string_template_is_cited_once_and_its_hole_is_code() {
    let src = "package app\n\nfun helper() = \"\"\n\nfun canon() {\n    val s = \"ADR-031 ${helper()} and ${\"ADR-032\"}\"\n}\n";
    let ex = one("app/T.kt", src);
    let refs = edges(&ex, EdgeKind::References);
    for id in ["ADR-031", "ADR-032"] {
        assert_eq!(refs.iter().filter(|(_, t, _)| t.contains(id)).count(), 1, "{id}: {refs:?}");
    }
    assert!(calls_from(&ex, "sym:app/T.kt::canon").contains(&"sym:app/T.kt::helper"), "{:?}", ex.edges);
}

#[test]
fn a_one_line_object_declares_itself_and_its_members() {
    let src = "package app\n\nobject Keys { fun token() = 1 }\n\nobject Names { val TOKEN = 1 }\n\nclass Outer {\n    object Inner { fun size() = 2 }\n    fun go() {\n        Keys.token()\n        Inner.size()\n    }\n}\n";
    let ex = one("app/K.kt", src);
    for id in ["sym:app/K.kt::Keys", "sym:app/K.kt::Keys.token", "sym:app/K.kt::Names", "sym:app/K.kt::Names.TOKEN", "sym:app/K.kt::Outer.Inner", "sym:app/K.kt::Outer.Inner.size"] {
        assert!(ids(&ex).contains(&id), "{id} missing from {:?}", ids(&ex));
    }
    let got = calls_from(&ex, "sym:app/K.kt::Outer.go");
    assert!(got.contains(&"sym:app/K.kt::Keys.token"), "{got:?}");
    assert!(got.contains(&"sym:app/K.kt::Outer.Inner.size"), "{got:?}");
}

/// Every name below is bound by something the file reads no type for, and each hides a property,
/// a type or a function that would resolve: a call through it is a guess, so it writes nothing.
const SHADOWS: &str = "package app

class ReplicaFiles {
    fun delete() {}
}

object Registry {
    fun reset() {}
}

fun helper(n: Int) = n

class Handler(private val files: ReplicaFiles, private val it: ReplicaFiles) {
    fun lambdaParameter(all: List<Any>) { all.forEach { files -> files.delete() } }
    fun implicitIt(all: List<Any>) { all.forEach { it.delete() } }
    fun destructured(pair: Any) {
        val (files, other) = pair
        files.delete()
    }
    fun forVariable(all: List<Any>) { for (files in all) { files.delete() } }
    fun whenSubject() {
        when (val files = pick()) {
            else -> files.delete()
        }
    }
    fun localOverType(other: Any) {
        val Registry = other
        Registry.reset()
    }
    fun parameterOverType(Registry: Any) { Registry.reset() }
    fun localOverFunction() {
        val helper = { n: Int -> n }
        helper(1)
    }
    fun parameterOverFunction(helper: (Int) -> Int) { helper(1) }
    fun caught() {
        try {} catch (files: Exception) { files.delete() }
    }
    fun anonymous() {
        val o = object {
            val files = Any()
            fun helper(n: Int) = n
            fun run() {
                helper(1)
                files.delete()
                this.files.delete()
            }
        }
    }
    fun unknownMember() { files.missing() }
    fun pick(): Any = files
}
";

#[test]
fn a_name_a_lambda_a_loop_a_when_or_a_destructuring_binds_hides_what_it_shadows() {
    let ex = one("app/S.kt", SHADOWS);
    let calls = edges(&ex, EdgeKind::Calls);
    let wrong: Vec<_> = calls.iter().filter(|(_, t, _)| !t.ends_with("Handler.pick")).collect();
    assert!(wrong.is_empty(), "{wrong:?}");
    assert!(!calls.iter().any(|(_, t, _)| t.ends_with(".missing")), "an undeclared member is no edge: {calls:?}");
}

#[test]
fn a_lambda_parameter_ends_with_its_lambda() {
    let src = "package app\n\nclass ReplicaFiles {\n    fun delete() {}\n}\n\nclass Handler(private val files: ReplicaFiles) {\n    fun go(all: List<Any>) {\n        all.forEach { files -> files.delete() }\n        files.delete()\n    }\n}\n";
    let ex = one("app/L.kt", src);
    let got = calls_from(&ex, "sym:app/L.kt::Handler.go");
    assert_eq!(got.iter().filter(|t| **t == "sym:app/L.kt::ReplicaFiles.delete").count(), 1, "{got:?}");
}

#[test]
fn an_inherited_member_is_called_and_a_top_level_namesake_leaves_the_call_to_overloads() {
    let src = "package app\n\nfun helper(n: Int) = n\n\nopen class Base {\n    fun helper(n: Int) = n\n    fun only() {}\n}\n\nclass Sub : Base() {\n    fun go() { helper(1) }\n    fun inherits() { only() }\n}\n";
    let ex = one("app/I.kt", src);
    assert!(calls_from(&ex, "sym:app/I.kt::Sub.go").is_empty(), "{:?}", ex.edges);
    assert_eq!(calls_from(&ex, "sym:app/I.kt::Sub.inherits"), vec!["sym:app/I.kt::Base.only"], "{:?}", ex.edges);
}

#[test]
fn a_name_both_the_file_and_an_import_declare_is_no_edge() {
    let repo = Repo::new(&[
        ("lib/H.kt", "package lib\n\nfun helper(s: String) = s\n"),
        ("app/U.kt", "package app\n\nimport lib.helper\n\nfun helper(n: Int) = n\n\nfun go() { helper(1) }\n"),
    ]);
    let ex = repo.extract("app/U.kt");
    assert!(calls_from(&ex, "sym:app/U.kt::go").is_empty(), "{:?}", ex.edges);
}

#[test]
fn a_supertype_outside_the_repository_may_hold_the_name_so_a_bare_call_is_no_edge() {
    let src = "package app\n\nfun finish() {}\n\nfun helper() {}\n\nclass Screen : androidx.appcompat.app.AppCompatActivity() {\n    fun go() { finish() }\n}\n\nclass Plain {\n    fun go() { finish() }\n}\n\nfun toString(n: Int) = \"\"\n\nclass Any2 {\n    fun go() { toString(1) }\n    fun literal() {\n        val r = object : Runnable {\n            override fun run() { helper() }\n        }\n    }\n}\n";
    let ex = one("app/A.kt", src);
    assert!(calls_from(&ex, "sym:app/A.kt::Screen.go").is_empty(), "{:?}", ex.edges);
    assert_eq!(calls_from(&ex, "sym:app/A.kt::Plain.go"), vec!["sym:app/A.kt::finish"], "{:?}", ex.edges);
    assert!(calls_from(&ex, "sym:app/A.kt::Any2.go").is_empty(), "every class holds Any's members: {:?}", ex.edges);
    assert!(calls_from(&ex, "sym:app/A.kt::Any2.literal").is_empty(), "{:?}", ex.edges);
}

#[test]
fn a_supertype_chain_in_other_files_is_walked_until_a_level_declares_the_name_or_is_unread() {
    let repo = Repo::new(&[
        ("app/A.kt", "package app\n\nopen class Base {\n    fun helper() {}\n    fun only() {}\n    fun two(x: Int) {}\n}\n"),
        ("app/B.kt", "package app\n\nopen class Mid : Base()\n\nopen class Lost : Exception()\n"),
        ("app/C.kt", "package app\n\nfun helper() {}\n\nfun other() {}\n\nfun two() {}\n\nclass Sub : Mid() {\n    fun go() { only() }\n    fun both() { helper() }\n    fun arity() { two() }\n    fun local() {\n        val only = { }\n        only()\n    }\n}\n\nclass Direct : Base() {\n    fun go() { other() }\n}\n\nclass Far : Lost() {\n    fun go() { other() }\n}\n"),
    ]);
    let ex = repo.extract("app/C.kt");
    assert_eq!(calls_from(&ex, "sym:app/C.kt::Sub.go"), vec!["sym:app/A.kt::Base.only"], "Mid is walked to Base: {:?}", ex.edges);
    assert!(calls_from(&ex, "sym:app/C.kt::Sub.both").is_empty(), "Base's member and the top level both bind helper: {:?}", ex.edges);
    assert!(calls_from(&ex, "sym:app/C.kt::Sub.arity").is_empty(), "Base declares two, though not for these arguments: {:?}", ex.edges);
    assert!(calls_from(&ex, "sym:app/C.kt::Sub.local").is_empty(), "a local hides the member: {:?}", ex.edges);
    assert_eq!(calls_from(&ex, "sym:app/C.kt::Direct.go"), vec!["sym:app/C.kt::other"], "Base extends nothing, so other is the top level's: {:?}", ex.edges);
    assert!(calls_from(&ex, "sym:app/C.kt::Far.go").is_empty(), "Lost's supertype is unread: {:?}", ex.edges);
}

#[test]
fn a_constructor_call_in_a_type_whose_supertype_is_another_file_s_binds_unless_the_supertype_declares_the_name() {
    let repo = Repo::new(&[
        ("app/Key.kt", "package app\n\nclass Key(val raw: String)\n"),
        ("app/sync/Wipe.kt", "package app.sync\n\nfun interface Destroyer {\n    fun destroy()\n}\n\ninterface Holder {\n    fun Key(raw: String): Any = raw\n}\n"),
        ("app/Store.kt", "package app\n\nimport app.sync.Destroyer\nimport app.sync.Holder\n\nclass Store : Destroyer {\n    override fun destroy() {}\n    fun make(s: String?): Key? {\n        s?.let { return Key(it) }\n        return null\n    }\n}\n\nclass Shadowed : Holder {\n    fun make(s: String): Any = Key(s)\n}\n"),
    ]);
    let ex = repo.extract("app/Store.kt");
    assert_eq!(calls_from(&ex, "sym:app/Store.kt::Store.make"), vec!["sym:app/Key.kt::Key"], "{:?}", ex.edges);
    assert!(calls_from(&ex, "sym:app/Store.kt::Shadowed.make").iter().all(|t| *t != "sym:app/Key.kt::Key"), "{:?}", ex.edges);
}

#[test]
fn an_extension_function_calls_through_its_receiver_first() {
    let src = "package app\n\nfun helper(n: Int) = n\n\nfun top() {}\n\nclass Disk\n\nclass Store(val files: Disk) {\n    fun helper(n: Int) = n\n    fun save() {}\n    fun m() {}\n}\n\nfun Store.ext() {\n    helper(1)\n    this.save()\n}\n\nclass A {\n    fun m() {}\n    fun Store.inner() {\n        this.m()\n        m()\n    }\n}\n\nfun StringBuilder.outside() { top() }\n";
    let ex = one("app/B.kt", src);
    assert_eq!(calls_from(&ex, "sym:app/B.kt::ext"), vec!["sym:app/B.kt::Store.save"], "{:?}", ex.edges);
    assert_eq!(calls_from(&ex, "sym:app/B.kt::A.inner"), vec!["sym:app/B.kt::Store.m"], "{:?}", ex.edges);
    assert!(calls_from(&ex, "sym:app/B.kt::outside").is_empty(), "an unread receiver may hold the name: {:?}", ex.edges);
}

const RECEIVERS: &str = "package app

class Store {
    fun save() {}
    fun keep() {}
}

fun helper() {}

fun build(block: Store.() -> Unit) {}

class Host {
    fun keep() {}
    fun clash(s: Store) { s.apply { keep() } }
    fun written(s: Store) {
        s.apply { save() }
        with(s) { save() }
        build { save() }
    }
    fun plain(xs: List<Int>) { xs.forEach { helper() } }
    fun unknownBare() { external { helper() } }
    fun unknownThis() { external { this.save() } }
    fun unknownCapital() { external { Store() } }
}
";

#[test]
fn a_lambda_s_receiver_is_read_where_it_is_written_and_otherwise_hides_lowercase_names() {
    let ex = one("app/R.kt", RECEIVERS);
    let written = calls_from(&ex, "sym:app/R.kt::Host.written");
    assert!(written.contains(&"sym:app/R.kt::Store.save"), "{written:?}");
    assert_eq!(written, vec!["sym:app/R.kt::Store.save", "sym:app/R.kt::build"], "{written:?}");
    assert!(calls_from(&ex, "sym:app/R.kt::Host.clash").is_empty(), "the receiver and Host both bind keep: {:?}", ex.edges);
    assert_eq!(calls_from(&ex, "sym:app/R.kt::Host.plain"), vec!["sym:app/R.kt::helper"], "{:?}", ex.edges);
    assert!(calls_from(&ex, "sym:app/R.kt::Host.unknownBare").is_empty(), "{:?}", ex.edges);
    assert!(calls_from(&ex, "sym:app/R.kt::Host.unknownThis").is_empty(), "{:?}", ex.edges);
    assert_eq!(calls_from(&ex, "sym:app/R.kt::Host.unknownCapital"), vec!["sym:app/R.kt::Store"], "{:?}", ex.edges);
}

#[test]
fn two_levels_binding_one_name_leave_the_call_to_overload_resolution() {
    let one_file = one("app/O.kt", "package app\n\nfun helper(n: Int) = n\n\nclass Sub {\n    fun helper(s: String) = s\n    fun go() { helper(1) }\n}\n");
    assert!(calls_from(&one_file, "sym:app/O.kt::Sub.go").is_empty(), "{:?}", one_file.edges);
    let repo = Repo::new(&[
        ("lib/H.kt", "package lib\n\nfun helper(s: String) = s\n"),
        ("app/P.kt", "package app\n\nfun helper(n: Int) = n\n"),
        ("app/U.kt", "package app\n\nimport lib.helper\n\nfun go() { helper(1) }\n"),
    ]);
    let ex = repo.extract("app/U.kt");
    assert!(calls_from(&ex, "sym:app/U.kt::go").is_empty(), "{:?}", ex.edges);
}

#[test]
fn a_platform_caller_reaches_the_expect_and_its_own_platform_s_actual_only() {
    let repo = Repo::new(&[
        ("shared/src/commonMain/kotlin/p/Platform.kt", "package p\n\nexpect fun platformName(): String\n"),
        ("shared/src/androidMain/kotlin/p/Platform.android.kt", "package p\n\nactual fun platformName() = \"a\"\n"),
        ("shared/src/iosMain/kotlin/p/Platform.ios.kt", "package p\n\nactual fun platformName() = \"i\"\n"),
        ("shared/src/androidHostTest/kotlin/p/T.kt", "package p\n\nfun t() { platformName() }\n"),
        ("shared/src/commonMain/kotlin/p/Use.kt", "package p\n\nfun u() { platformName() }\n"),
    ]);
    let mut t = calls_from(&repo.extract("shared/src/androidHostTest/kotlin/p/T.kt"), "sym:shared/src/androidHostTest/kotlin/p/T.kt::t").into_iter().map(String::from).collect::<Vec<_>>();
    t.sort();
    assert_eq!(t, vec!["sym:shared/src/androidMain/kotlin/p/Platform.android.kt::platformName", "sym:shared/src/commonMain/kotlin/p/Platform.kt::platformName"]);
    let u = repo.extract("shared/src/commonMain/kotlin/p/Use.kt");
    assert_eq!(calls_from(&u, "sym:shared/src/commonMain/kotlin/p/Use.kt::u").len(), 3, "{:?}", u.edges);
}

#[test]
fn a_nested_class_reaches_no_outer_instance_and_an_inner_one_or_an_object_does() {
    let src = "package app\n\nclass Disk {\n    fun delete() {}\n}\n\nclass Outer(val files: Disk) {\n    fun only() {}\n    class Nested {\n        fun g() {\n            only()\n            files.delete()\n        }\n    }\n    inner class In {\n        fun h() { only() }\n    }\n}\n\nobject Holder {\n    fun k() {}\n    class N {\n        fun g() { k() }\n    }\n}\n";
    let ex = one("app/N.kt", src);
    assert!(calls_from(&ex, "sym:app/N.kt::Outer.Nested.g").is_empty(), "{:?}", ex.edges);
    assert_eq!(calls_from(&ex, "sym:app/N.kt::Outer.In.h"), vec!["sym:app/N.kt::Outer.only"], "{:?}", ex.edges);
    assert_eq!(calls_from(&ex, "sym:app/N.kt::Holder.N.g"), vec!["sym:app/N.kt::Holder.k"], "{:?}", ex.edges);
}

#[test]
fn a_type_parameter_is_not_the_repository_type_it_is_named_like() {
    let src = "package app\n\nclass State {\n    fun reduce() {}\n}\n\nabstract class Store<State>(val s: State) {\n    fun f() { s.reduce() }\n    fun <State> g(x: State) { x.reduce() }\n}\n\nfun <State> top(x: State) { x.reduce() }\n";
    let ex = one("app/P.kt", src);
    let wrong: Vec<_> = edges(&ex, EdgeKind::Calls).into_iter().filter(|(_, t, _)| t.ends_with("State.reduce")).collect();
    assert!(wrong.is_empty(), "{wrong:?}");
}

#[test]
fn a_local_wins_over_a_later_receiver_s_member_and_an_enum_holds_enum_s_members() {
    let src = "package app\n\nclass Disk {\n    fun delete() {}\n}\n\nclass Other {\n    fun delete() {}\n}\n\nclass Store(val files: Other)\n\nfun valueOf(s: String) = s\n\nclass Host {\n    fun go(s: Store) {\n        val files = Disk()\n        s.apply { files.delete() }\n    }\n}\n\nenum class Kind {\n    A;\n    fun go() { valueOf(\"A\") }\n}\n";
    let ex = one("app/D.kt", src);
    let go = calls_from(&ex, "sym:app/D.kt::Host.go");
    assert_eq!(go, vec!["sym:app/D.kt::Disk", "sym:app/D.kt::Disk.delete"], "locals come before implicit receivers: {go:?}");
    assert!(calls_from(&ex, "sym:app/D.kt::Kind.go").is_empty(), "{:?}", ex.edges);
}

#[test]
fn a_data_class_s_generated_members_hide_top_level_namesakes() {
    let src = "package app\nfun copy(): Int = 1\nfun component1(): Int = 2\ndata class P(val a: Int) { fun go() { copy(); component1() } }\n";
    let ex = one("app/P.kt", src);
    assert!(calls_from(&ex, "sym:app/P.kt::P.go").is_empty(), "{:?}", ex.edges);
}

#[test]
fn a_package_name_several_files_declare_is_no_edge_unless_it_is_an_expect_actual_family() {
    let repo = Repo::new(&[
        ("app/A.kt", "package app\nfun helper(n: Int) = n\n"),
        ("app/B.kt", "package app\nfun helper(s: String) = s\nfun own(s: String) = s\n"),
        ("app/C.kt", "package app\nfun own(n: Int) = n\nfun go() { helper(1); own(\"x\") }\n"),
        ("lib/X.kt", "package lib\nfun both(n: Int) = n\n"),
        ("lib/Y.kt", "package lib\nfun both(s: String) = s\n"),
        ("use/U.kt", "package use\nimport lib.*\nfun go() { both(1) }\n"),
    ]);
    let ex = repo.extract("app/C.kt");
    assert!(calls_from(&ex, "sym:app/C.kt::go").is_empty(), "{:?}", ex.edges);
    let ex = repo.extract("use/U.kt");
    assert!(calls_from(&ex, "sym:use/U.kt::go").is_empty(), "{:?}", ex.edges);
}

#[test]
fn a_private_top_level_function_is_its_own_file_s_alone() {
    let repo = Repo::new(&[
        ("app/T1.kt", "package app\nprivate fun flip() = 1\nfun t1() { flip() }\n"),
        ("app/T2.kt", "package app\nprivate fun flip() = 2\nfun t2() { flip() }\n"),
        ("app/T3.kt", "package app\nfun t3() { flip() }\n"),
    ]);
    assert_eq!(calls_from(&repo.extract("app/T1.kt"), "sym:app/T1.kt::t1"), vec!["sym:app/T1.kt::flip"]);
    assert_eq!(calls_from(&repo.extract("app/T2.kt"), "sym:app/T2.kt::t2"), vec!["sym:app/T2.kt::flip"]);
    let t3 = repo.extract("app/T3.kt");
    assert!(calls_from(&t3, "sym:app/T3.kt::t3").is_empty(), "another file's private function is out of reach: {:?}", t3.edges);
}

#[test]
fn a_kotlin_call_binds_the_supertype_s_overload_when_the_own_one_cannot_take_the_arguments() {
    let repo = Repo::new(&[
        ("app/Two.kt", "package app\n\nopen class Up {\n    fun m(s: String, t: String) {}\n}\n\nclass Down : Up() {\n    fun m(n: Int) {}\n    fun go() { m(\"x\", \"y\") }\n    fun on(d: Down) { d.m(\"x\", \"y\") }\n}\n"),
        ("app/Base.kt", "package app\n\nopen class Base {\n    fun show(s: String, t: String) {}\n}\n"),
        ("app/Sub.kt", "package app\n\nclass Sub : Base() {\n    fun show(n: Int) {}\n    fun go() { show(\"x\", \"y\") }\n}\n"),
        ("app/Jv.java", "package app;\n\npublic class Jv {\n    public void put(String s, String t) {}\n}\n"),
        ("app/Kj.kt", "package app\n\nclass Kj : Jv() {\n    fun put(n: Int) {}\n    fun go() { put(\"x\", \"y\") }\n}\n"),
    ]);
    let two = repo.extract("app/Two.kt");
    assert_eq!(calls_from(&two, "sym:app/Two.kt::Down.go"), vec!["sym:app/Two.kt::Up.m"]);
    assert_eq!(calls_from(&two, "sym:app/Two.kt::Down.on"), vec!["sym:app/Two.kt::Up.m"]);
    let sub = repo.extract("app/Sub.kt");
    assert_eq!(calls_from(&sub, "sym:app/Sub.kt::Sub.go"), vec!["sym:app/Base.kt::Base.show"]);
    let kj = repo.extract("app/Kj.kt");
    assert_eq!(calls_from(&kj, "sym:app/Kj.kt::Kj.go"), vec!["sym:app/Jv.java::Jv.put"]);
}

#[test]
fn a_kotlin_default_vararg_or_trailing_lambda_counts_toward_the_arguments_a_function_takes() {
    let src = "package app\n\nclass K : android.app.Dialog() {\n    fun m(a: Int, b: Int = 0) {}\n    fun v(vararg xs: Int) {}\n    fun t(f: () -> Unit) {}\n    fun one() { m(1) }\n    fun two() { m(1, 2) }\n    fun none() { m() }\n    fun many() { v(1, 2, 3) }\n    fun lambda() { t { } }\n}\n";
    let ex = one("app/K.kt", src);
    assert_eq!(calls_from(&ex, "sym:app/K.kt::K.one"), vec!["sym:app/K.kt::K.m"]);
    assert_eq!(calls_from(&ex, "sym:app/K.kt::K.two"), vec!["sym:app/K.kt::K.m"]);
    assert!(calls_from(&ex, "sym:app/K.kt::K.none").is_empty(), "the dialog may declare m(): {:?}", ex.edges);
    assert_eq!(calls_from(&ex, "sym:app/K.kt::K.many"), vec!["sym:app/K.kt::K.v"]);
    assert_eq!(calls_from(&ex, "sym:app/K.kt::K.lambda"), vec!["sym:app/K.kt::K.t"]);
}

#[test]
fn an_override_or_actual_taking_its_defaults_from_its_base_is_never_passed_over_for_the_base() {
    let repo = Repo::new(&[
        ("app/Repo.kt", "package app\n\ninterface Repo {\n    fun find(id: Int, cache: Boolean = true)\n}\n\nclass RepoImpl : Repo {\n    override fun find(id: Int, cache: Boolean) {}\n    fun x() { find(1) }\n    fun y() { find(1, false) }\n}\n\nclass U {\n    fun z(r: RepoImpl) { r.find(1) }\n}\n"),
        ("app/Base.kt", "package app\n\nabstract class Base {\n    abstract fun load(force: Boolean = false)\n}\n"),
        ("app/Screen.kt", "package app\n\nclass Screen : Base() {\n    override fun load(force: Boolean) {}\n    fun go() { load() }\n}\n"),
        ("shared/src/commonMain/kotlin/p/Clock.kt", "package p\n\nexpect class Clock() {\n    fun tick(n: Int = 1)\n}\n"),
        ("shared/src/jvmMain/kotlin/p/Clock.jvm.kt", "package p\n\nfun tick() {}\n\nactual class Clock actual constructor() {\n    actual fun tick(n: Int) {}\n    fun go() { tick() }\n}\n"),
    ]);
    let r = repo.extract("app/Repo.kt");
    assert!(calls_from(&r, "sym:app/Repo.kt::RepoImpl.x").is_empty(), "{:?}", r.edges);
    assert_eq!(calls_from(&r, "sym:app/Repo.kt::RepoImpl.y"), vec!["sym:app/Repo.kt::RepoImpl.find"]);
    assert!(calls_from(&r, "sym:app/Repo.kt::U.z").is_empty(), "{:?}", r.edges);
    let screen = repo.extract("app/Screen.kt");
    assert!(calls_from(&screen, "sym:app/Screen.kt::Screen.go").is_empty(), "{:?}", screen.edges);
    let clock = repo.extract("shared/src/jvmMain/kotlin/p/Clock.jvm.kt");
    assert!(calls_from(&clock, "sym:shared/src/jvmMain/kotlin/p/Clock.jvm.kt::Clock.go").is_empty(), "{:?}", clock.edges);
}

#[test]
fn a_private_kotlin_member_is_neither_inherited_nor_reached_from_outside_its_class() {
    let repo = Repo::new(&[
        ("app/Same.kt", "package app\n\nopen class A {\n    private fun helper() {}\n}\n\nclass Outer {\n    fun helper() {}\n    inner class B : A() {\n        fun go() { helper() }\n    }\n}\n"),
        ("ext/C.kt", "package ext\n\nclass C {\n    private fun helper() {}\n    fun self(o: C) { o.helper() }\n}\n\nfun C.helper() {}\n\nclass User {\n    fun go(c: C) { c.helper() }\n}\n"),
        ("app/P.kt", "package app\n\nopen class P {\n    private fun helper() {}\n}\n"),
        ("app/Cross.kt", "package app\n\nclass Cross {\n    fun helper() {}\n    inner class B : P() {\n        fun go() { helper() }\n    }\n}\n"),
    ]);
    let same = repo.extract("app/Same.kt");
    assert_eq!(calls_from(&same, "sym:app/Same.kt::Outer.B.go"), vec!["sym:app/Same.kt::Outer.helper"]);
    let c = repo.extract("ext/C.kt");
    assert_eq!(calls_from(&c, "sym:ext/C.kt::C.self"), vec!["sym:ext/C.kt::C.helper"]);
    assert!(!calls_from(&c, "sym:ext/C.kt::User.go").contains(&"sym:ext/C.kt::C.helper"), "{:?}", c.edges);
    let cross = repo.extract("app/Cross.kt");
    assert!(!calls_from(&cross, "sym:app/Cross.kt::Cross.B.go").contains(&"sym:app/P.kt::P.helper"), "{:?}", cross.edges);
}

#[test]
fn a_kotlin_spread_argument_binds_only_a_vararg_function() {
    let src = "package app\n\nopen class Up {\n    fun m(vararg xs: Int) {}\n}\n\nclass Down : Up() {\n    fun m(a: Int) {}\n    fun go(arr: IntArray) { m(*arr) }\n}\n";
    let ex = one("app/Down.kt", src);
    assert_eq!(calls_from(&ex, "sym:app/Down.kt::Down.go"), vec!["sym:app/Down.kt::Up.m"]);
}

#[test]
fn a_local_typed_by_a_private_nested_class_reaches_its_members_from_the_outer_class() {
    let src = "package app\n\nclass Engine {\n    fun sync() {\n        val s = Session()\n        s.record()\n    }\n    private class Session {\n        fun record() {}\n    }\n}\n";
    let ex = one("app/Engine.kt", src);
    assert!(calls_from(&ex, "sym:app/Engine.kt::Engine.sync").contains(&"sym:app/Engine.kt::Engine.Session.record"), "{:?}", ex.edges);
}

#[test]
fn a_private_overload_beside_a_public_one_takes_no_call_from_outside_its_class() {
    let a = "open class A {\n    private fun helper() {}\n    fun helper(x: Int) {}\n    fun self() { helper() }\n}\n";
    let users = "class X {\n    fun go(a: A) { a.helper() }\n    fun one(a: A) { a.helper(1) }\n}\n\nclass Outer {\n    fun helper() {}\n    inner class B : A() {\n        fun go() { helper() }\n    }\n}\n";
    let repo = Repo::new(&[
        ("app/Same.kt", &format!("package app\n\n{a}\n{users}")),
        ("ext/A.kt", &format!("package ext\n\n{a}\nfun A.helper() {{}}\n\nclass Y {{\n    fun go(a: A) {{ a.helper() }}\n}}\n")),
        ("cross/A.kt", &format!("package cross\n\n{a}")),
        ("cross/Users.kt", &format!("package cross\n\n{users}")),
    ]);
    let same = repo.extract("app/Same.kt");
    assert_eq!(calls_from(&same, "sym:app/Same.kt::A.self"), vec!["sym:app/Same.kt::A.helper"]);
    assert_eq!(calls_from(&same, "sym:app/Same.kt::X.one"), vec!["sym:app/Same.kt::A.helper"]);
    assert!(calls_from(&same, "sym:app/Same.kt::X.go").is_empty(), "{:?}", same.edges);
    assert!(calls_from(&same, "sym:app/Same.kt::Outer.B.go").is_empty(), "only A's private overload takes it: {:?}", same.edges);
    let ext = repo.extract("ext/A.kt");
    assert!(!calls_from(&ext, "sym:ext/A.kt::Y.go").contains(&"sym:ext/A.kt::A.helper"), "{:?}", ext.edges);
    let cross = repo.extract("cross/Users.kt");
    assert_eq!(calls_from(&cross, "sym:cross/Users.kt::X.one"), vec!["sym:cross/A.kt::A.helper"]);
    assert!(calls_from(&cross, "sym:cross/Users.kt::X.go").is_empty(), "{:?}", cross.edges);
    assert!(calls_from(&cross, "sym:cross/Users.kt::Outer.B.go").is_empty(), "{:?}", cross.edges);
}

#[test]
fn an_annotation_declared_in_the_repository_decorates_and_one_declared_outside_writes_nothing() {
    let repo = Repo::new(&[
        ("app/Api.kt", "package app\n\n@Target(AnnotationTarget.CLASS)\nannotation class Api\n"),
        ("app/Service.kt", "package app\n\n@Api\n@Inject\nclass Service(@Api private val store: Store) {\n    @Api\n    fun run() {}\n}\n"),
    ]);
    let ex = repo.extract("app/Service.kt");
    let deco = edges(&ex, EdgeKind::DecoratedBy);
    for from in ["sym:app/Service.kt::Service", "sym:app/Service.kt::Service.store", "sym:app/Service.kt::Service.run"] {
        assert!(deco.contains(&(from, "sym:app/Api.kt::Api", "")), "{from}: {deco:?}");
    }
    assert_eq!(deco.len(), 3, "`@Inject` resolves nowhere and writes nothing: {deco:?}");
    assert!(!ids(&ex).iter().any(|i| i.starts_with("anno:") || i.starts_with("deco:")), "{:?}", ids(&ex));
}

// `-sg` reads `@file:` as a `file_annotation` on the source file: it annotates no declaration, even
// when the annotation class is the repository's own.
#[test]
fn a_file_annotation_decorates_nothing() {
    let repo = Repo::new(&[
        ("app/Api.kt", "package app\n\nannotation class Api\n"),
        ("app/A.kt", "@file:Api\npackage app\n\nclass A\n"),
    ]);
    let ex = repo.extract("app/A.kt");
    assert!(ids(&ex).contains(&"sym:app/A.kt::A"), "{:?}", ids(&ex));
    assert!(edges(&ex, EdgeKind::DecoratedBy).is_empty(), "{:?}", ex.edges);
}

// A type parameter named like the annotation class shadows it; the compiler rejects the use, and
// the graph must not pretend it reached the class.
#[test]
fn an_annotation_named_like_a_type_parameter_in_scope_decorates_nothing() {
    let repo = Repo::new(&[
        ("app/Api.kt", "package app\n\nannotation class Api\n"),
        ("app/Box.kt", "package app\n\nclass Box<Api> {\n    @Api\n    fun run() {}\n    @Api\n    fun <T> take() {}\n}\n\nclass Other {\n    @Api\n    fun <Api> lift() {}\n    @Api\n    fun drop() {}\n}\n"),
    ]);
    let ex = repo.extract("app/Box.kt");
    assert_eq!(edges(&ex, EdgeKind::DecoratedBy), vec![("sym:app/Box.kt::Other.drop", "sym:app/Api.kt::Api", "")], "{:?}", ex.edges);
}

#[test]
fn an_annotation_two_star_imports_both_bind_decorates_nothing() {
    let repo = Repo::new(&[
        ("a/Api.kt", "package a\n\nannotation class Api\n"),
        ("b/Api.kt", "package b\n\nannotation class Api\n"),
        ("app/Use.kt", "package app\n\nimport a.*\nimport b.*\n\n@Api\nclass Use\n\n@a.Api\nclass Pinned\n"),
    ]);
    let ex = repo.extract("app/Use.kt");
    assert_eq!(edges(&ex, EdgeKind::DecoratedBy), vec![("sym:app/Use.kt::Pinned", "sym:a/Api.kt::Api", "")], "{:?}", ex.edges);
}

#[test]
fn a_use_site_targeted_annotation_decorates_the_property() {
    let repo = Repo::new(&[
        ("app/Api.kt", "package app\n\nannotation class Api(val name: String)\n"),
        ("app/Holder.kt", "package app\n\nclass Holder {\n    @get:Api(\"n\")\n    val name: String = \"\"\n}\n"),
    ]);
    let ex = repo.extract("app/Holder.kt");
    assert_eq!(edges(&ex, EdgeKind::DecoratedBy), vec![("sym:app/Holder.kt::Holder.name", "sym:app/Api.kt::Api", "")], "{:?}", ex.edges);
}

// ---- property initializers and bare references ----

fn refs_from<'a>(ex: &'a crate::model::Extraction, from: &str) -> Vec<&'a str> {
    edges(ex, EdgeKind::References).into_iter().filter(|(s, t, _)| *s == from && t.starts_with("sym:")).map(|(_, t, _)| t).collect()
}

const CORE: &str = "package app

object Gap
object Par

private fun order(rules: List<Any>): List<Any> = rules

fun top() = 1

class Store {
    fun save() = 1
}

class Core(x: Store, val y: Store) {
    val rules: List<Any> = order(listOf(Gap, Par))
    val lazyOne by lazy { Gap }
    val fromParam = x.save()
    val computed: Int get() = top()
    companion object {
        val shared = Par
    }
}

val topLevel = top()
";

#[test]
fn a_property_initializer_calls_and_references_from_the_property() {
    let ex = one("app/Core.kt", CORE);
    assert_eq!(calls_from(&ex, "sym:app/Core.kt::Core.rules"), vec!["sym:app/Core.kt::order"], "{:?}", ex.edges);
    assert_eq!(refs_from(&ex, "sym:app/Core.kt::Core.rules"), vec!["sym:app/Core.kt::Gap", "sym:app/Core.kt::Par"], "{:?}", ex.edges);
    assert_eq!(refs_from(&ex, "sym:app/Core.kt::Core.lazyOne"), vec!["sym:app/Core.kt::Gap"], "a delegate is walked: {:?}", ex.edges);
    assert_eq!(calls_from(&ex, "sym:app/Core.kt::Core.fromParam"), vec!["sym:app/Core.kt::Store.save"], "a constructor parameter is typed in an initializer: {:?}", ex.edges);
    assert_eq!(calls_from(&ex, "sym:app/Core.kt::Core.computed"), vec!["sym:app/Core.kt::top"], "a getter is walked: {:?}", ex.edges);
    assert_eq!(refs_from(&ex, "sym:app/Core.kt::Core.shared"), vec!["sym:app/Core.kt::Par"], "a companion property is the class's: {:?}", ex.edges);
    assert_eq!(calls_from(&ex, "sym:app/Core.kt::topLevel"), vec!["sym:app/Core.kt::top"], "{:?}", ex.edges);
}

#[test]
fn a_bare_reference_in_another_file_s_package_reaches_the_object() {
    let repo = Repo::new(&[
        ("app/Rules.kt", "package app\n\ninternal object Gap\n"),
        ("app/Core.kt", "package app\n\nobject Core {\n    val rules: List<Any> = listOf(Gap)\n    fun pick(): Any = Gap\n}\n"),
    ]);
    let ex = repo.extract("app/Core.kt");
    assert_eq!(refs_from(&ex, "sym:app/Core.kt::Core.rules"), vec!["sym:app/Rules.kt::Gap"], "{:?}", ex.edges);
    assert_eq!(refs_from(&ex, "sym:app/Core.kt::Core.pick"), vec!["sym:app/Rules.kt::Gap"], "a function body references the same way: {:?}", ex.edges);
}

#[test]
fn a_bare_reference_a_local_a_parameter_or_an_accessor_binds_writes_nothing() {
    let src = "package app

object Gap
val value = 1
val v = 2
fun h(v: Any) = v

class Shadow(Gap: Int) {
    val a = Gap
    var b: Int = 0
        set(value) { field = value }
    fun f() {
        val Gap = 1
        h(Gap)
    }
    fun g(Gap: Int) = h(Gap)
    fun named() = h(v = Gap)
}
";
    let ex = one("app/S.kt", src);
    for from in ["sym:app/S.kt::Shadow.a", "sym:app/S.kt::Shadow.b", "sym:app/S.kt::Shadow.f", "sym:app/S.kt::Shadow.g"] {
        assert!(refs_from(&ex, from).is_empty(), "{from}: {:?}", ex.edges);
    }
    assert_eq!(refs_from(&ex, "sym:app/S.kt::Shadow.named"), vec!["sym:app/S.kt::Gap"], "a named argument's name is no reference: {:?}", ex.edges);
}

#[test]
fn a_bare_reference_two_levels_bind_or_an_unread_supertype_may_hold_writes_nothing() {
    let src = "package app

object Gap
fun helper() = 1

class Two {
    val Gap = 1
    val a: Any = app.Gap
    fun f(): Any = Gap
}

class Screen : androidx.appcompat.app.AppCompatActivity() {
    val a: Any = Gap
    val b = helper()
}

class Fn {
    val a: Any = helper
}
";
    let ex = one("app/T.kt", src);
    assert!(refs_from(&ex, "sym:app/T.kt::Two.f").iter().all(|t| *t != "sym:app/T.kt::Gap"), "{:?}", ex.edges);
    assert!(refs_from(&ex, "sym:app/T.kt::Two.f").is_empty(), "a member and the top level both bind Gap: {:?}", ex.edges);
    assert!(refs_from(&ex, "sym:app/T.kt::Screen.a").is_empty(), "{:?}", ex.edges);
    assert!(calls_from(&ex, "sym:app/T.kt::Screen.b").is_empty(), "the activity may declare helper(): {:?}", ex.edges);
    assert!(refs_from(&ex, "sym:app/T.kt::Fn.a").is_empty(), "a bare name never references a function: {:?}", ex.edges);
}

// ---- signature types ----

const KEY: &str = "package app\n\nclass Key {\n    class Inner\n}\n";

fn imports_to<'a>(ex: &'a crate::model::Extraction, to: &str) -> Vec<&'a str> {
    edges(ex, EdgeKind::Imports).into_iter().filter(|(_, t, _)| *t == to).map(|(_, _, c)| c).collect()
}

#[test]
fn a_type_named_only_in_a_signature_writes_the_type_edge_to_its_file() {
    let uses = [
        ("app/Param.kt", "package app\n\nfun f(k: Key) = 1\n"),
        ("app/Return.kt", "package app\n\nfun f(): Key? = null\n"),
        ("app/Prop.kt", "package app\n\nclass H {\n    val k: Key? = null\n}\n"),
        ("app/Ctor.kt", "package app\n\nclass H(k: Key)\n"),
        ("app/Receiver.kt", "package app\n\nfun Key.ext() = 1\n"),
        ("app/Generic.kt", "package app\n\nfun f(ks: List<Map<String, Key?>>) = ks\n"),
        ("app/Lambda.kt", "package app\n\nfun f(on: (Key) -> Unit) = on\n"),
        ("app/Nested.kt", "package app\n\nfun f(i: Key.Inner) = i\n"),
        ("app/Setter.kt", "package app\n\nclass H {\n    var n: Int = 0\n        set(v: Int) { field = v }\n    fun g(): List<Key> = emptyList()\n}\n"),
    ];
    let mut files = vec![("app/Key.kt", KEY)];
    files.extend(uses);
    let repo = Repo::new(&files);
    for (rel, _) in uses {
        let ex = repo.extract(rel);
        assert!(imports_to(&ex, "file:app/Key.kt").contains(&"Key"), "{rel}: {:?}", ex.edges);
    }
}

#[test]
fn a_signature_type_through_a_star_or_an_expect_family_names_each_file_on_the_platform() {
    let repo = Repo::new(&[
        ("s/src/commonMain/K.kt", "package app.storage\n\nexpect class ReplicaKey\n"),
        ("s/src/jvmMain/K.jvm.kt", "package app.storage\n\nactual class ReplicaKey\n"),
        ("s/src/androidMain/K.android.kt", "package app.storage\n\nactual class ReplicaKey\n"),
        ("s/src/androidMain/D.kt", "package app.sync\n\nimport app.storage.*\n\nclass Driver {\n    fun open(key: ReplicaKey) = key\n}\n"),
    ]);
    let ex = repo.extract("s/src/androidMain/D.kt");
    assert_eq!(imports_to(&ex, "file:s/src/commonMain/K.kt"), vec!["ReplicaKey"], "{:?}", ex.edges);
    assert_eq!(imports_to(&ex, "file:s/src/androidMain/K.android.kt"), vec!["ReplicaKey"], "{:?}", ex.edges);
    assert!(imports_to(&ex, "file:s/src/jvmMain/K.jvm.kt").is_empty(), "an Android file links no JVM actual: {:?}", ex.edges);
}

#[test]
fn a_signature_type_a_type_parameter_a_local_class_or_two_stars_bind_writes_nothing() {
    let repo = Repo::new(&[
        ("app/Key.kt", KEY),
        ("x/Dup.kt", "package x\n\nclass Dup\n"),
        ("y/Dup.kt", "package y\n\nclass Dup\n"),
        ("app/Mask.kt", "package app\n\nfun <Key> f(k: Key): Key = k\n\nclass G<Key>(val k: Key) {\n    fun g(): List<Key> = listOf(k)\n}\n"),
        ("app/Local.kt", "package app\n\nfun f() {\n    class Key\n    val k: Key = Key()\n    fun g(x: Key) = x\n}\n"),
        ("app/Stars.kt", "package app\n\nimport x.*\nimport y.*\n\nfun f(d: Dup) = d\n"),
        ("app/Own.kt", "package app\n\nclass Key2\n\nfun f(k: Key2) = k\n"),
    ]);
    for rel in ["app/Mask.kt", "app/Local.kt", "app/Stars.kt", "app/Own.kt"] {
        let ex = repo.extract(rel);
        assert!(edges(&ex, EdgeKind::Imports).is_empty(), "{rel}: {:?}", ex.edges);
    }
}

#[test]
fn an_accessor_on_its_own_line_is_walked_from_its_property() {
    let src = "package app

object Gap
fun top() = 1

class C(x: Int) {
    val a: Int
        get() = top()
    // the setter follows a comment
    var b: Any = 0
        get() = field
        /* and the getter */
        set(value) { h(Gap, value) }
    fun h(a: Any, b: Any) = a
    val c: Int
        get() = x
}

val t: Int
    get() = top()
";
    let ex = one("app/C.kt", src);
    assert_eq!(calls_from(&ex, "sym:app/C.kt::C.a"), vec!["sym:app/C.kt::top"], "{:?}", ex.edges);
    assert_eq!(calls_from(&ex, "sym:app/C.kt::C.b"), vec!["sym:app/C.kt::C.h"], "{:?}", ex.edges);
    assert_eq!(refs_from(&ex, "sym:app/C.kt::C.b"), vec!["sym:app/C.kt::Gap"], "the setter's parameter is a local: {:?}", ex.edges);
    assert!(refs_from(&ex, "sym:app/C.kt::C.c").is_empty(), "{:?}", ex.edges);
    assert_eq!(calls_from(&ex, "sym:app/C.kt::t"), vec!["sym:app/C.kt::top"], "{:?}", ex.edges);
}

const SIZED: (&str, &str) = ("app/Sized.kt", "package app\n\ninterface Sized {\n    fun isEmpty(): Boolean\n    fun size(): Int = 0\n}\n");

#[test]
fn a_superclass_member_beats_the_interface_a_class_also_implements() {
    let repo = Repo::new(&[
        SIZED,
        ("app/AbstractSized.kt", "package app\n\nabstract class AbstractSized : Sized {\n    override fun isEmpty() = true\n    override fun size() = 1\n}\n\nclass Near : AbstractSized(), Sized {\n    fun go() { isEmpty() }\n}\n"),
        ("app/Bag.kt", "package app\n\nclass Bag : AbstractSized(), Sized {\n    fun go() { isEmpty() }\n    fun count() { size() }\n}\n"),
    ]);
    let ex = repo.extract("app/Bag.kt");
    assert_eq!(calls_from(&ex, "sym:app/Bag.kt::Bag.go"), vec!["sym:app/AbstractSized.kt::AbstractSized.isEmpty"], "over the interface's abstract member: {:?}", ex.edges);
    assert_eq!(calls_from(&ex, "sym:app/Bag.kt::Bag.count"), vec!["sym:app/AbstractSized.kt::AbstractSized.size"], "over the interface's default: {:?}", ex.edges);
    let near = repo.extract("app/AbstractSized.kt");
    assert_eq!(calls_from(&near, "sym:app/AbstractSized.kt::Near.go"), vec!["sym:app/AbstractSized.kt::AbstractSized.isEmpty"], "{:?}", near.edges);
}

#[test]
fn an_interface_binds_only_when_no_superclass_level_declares_the_name_and_none_is_unread() {
    let repo = Repo::new(&[
        SIZED,
        ("app/Base.kt", "package app\n\nopen class Base\n\nopen class Lost : Exception()\n\nopen class Full : Sized {\n    override fun isEmpty() = true\n}\n"),
        (
            "app/Users.kt",
            "package app\n\nclass Plain : Base(), Sized {\n    override fun isEmpty() = false\n    fun go() { size() }\n}\n\nclass Screen : android.app.Activity(), Sized {\n    fun go() { size() }\n}\n\nclass Far : Lost(), Sized {\n    fun go() { size() }\n}\n\nclass Late : Full, Sized {\n    constructor() : super()\n    fun go() { isEmpty() }\n}\n",
        ),
    ]);
    let ex = repo.extract("app/Users.kt");
    assert_eq!(calls_from(&ex, "sym:app/Users.kt::Plain.go"), vec!["sym:app/Sized.kt::Sized.size"], "Base declares no size: {:?}", ex.edges);
    assert!(calls_from(&ex, "sym:app/Users.kt::Screen.go").is_empty(), "the activity may declare size: {:?}", ex.edges);
    assert!(calls_from(&ex, "sym:app/Users.kt::Far.go").is_empty(), "Lost's superclass may declare size: {:?}", ex.edges);
    assert!(calls_from(&ex, "sym:app/Users.kt::Late.go").is_empty(), "with no primary constructor, which supertype is the class is not written: {:?}", ex.edges);
}

#[test]
fn a_signature_or_a_value_the_grammar_failed_around_writes_nothing() {
    let repo = Repo::new(&[
        ("b/Key.kt", "package b\n\nclass Key\n\nobject Gap\n"),
        ("b/One.kt", "package b\n\nclass G<Key> { inner class I { fun f(k: Key) = Gap } }\n"),
    ]);
    let ex = repo.extract("b/One.kt");
    assert!(imports_to(&ex, "file:b/Key.kt").is_empty(), "the ERROR hides the type parameter: {:?}", ex.edges);
    assert!(edges(&ex, EdgeKind::References).is_empty(), "{:?}", ex.edges);
}
