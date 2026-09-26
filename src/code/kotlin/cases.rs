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
fn a_supertype_in_another_file_whose_chain_the_index_cannot_walk_is_no_proof() {
    let repo = Repo::new(&[
        ("app/A.kt", "package app\n\nopen class Base {\n    fun helper() {}\n}\n"),
        ("app/B.kt", "package app\n\nopen class Mid : Base()\n"),
        ("app/C.kt", "package app\n\nfun helper() {}\n\nfun other() {}\n\nclass Sub : Mid() {\n    fun go() { helper() }\n}\n\nclass Direct : Base() {\n    fun go() { other() }\n}\n"),
    ]);
    let ex = repo.extract("app/C.kt");
    assert!(calls_from(&ex, "sym:app/C.kt::Sub.go").is_empty(), "{:?}", ex.edges);
    assert!(calls_from(&ex, "sym:app/C.kt::Direct.go").is_empty(), "Base is read, but not what Base extends: {:?}", ex.edges);
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
fn a_receiver_pushed_after_a_local_may_hold_its_name_and_an_enum_holds_enum_s_members() {
    let src = "package app\n\nclass Disk {\n    fun delete() {}\n}\n\nclass Other {\n    fun delete() {}\n}\n\nclass Store(val files: Other)\n\nfun valueOf(s: String) = s\n\nclass Host {\n    fun go(s: Store) {\n        val files = Disk()\n        s.apply { files.delete() }\n    }\n}\n\nenum class Kind {\n    A;\n    fun go() { valueOf(\"A\") }\n}\n";
    let ex = one("app/D.kt", src);
    let go = calls_from(&ex, "sym:app/D.kt::Host.go");
    assert!(!go.contains(&"sym:app/D.kt::Disk.delete"), "{go:?}");
    assert!(calls_from(&ex, "sym:app/D.kt::Kind.go").is_empty(), "{:?}", ex.edges);
}
