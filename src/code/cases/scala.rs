//! Scala's shapes, each on inline files, so the reading of a declaration, an import and a call is
//! pinned by the assertion rather than by a corpus. Scala is a spec row and its two queries, with no
//! hook: a package spans directories, so names are read through the family's index by full path.

use crate::code::prose::testing::{ids, lines, repo};
use crate::model::{EdgeKind, Extraction};

const GLOBS: &[&str] = &["**/*.scala", "**/*.sc"];

fn extract(files: &[(&str, &str)], rel: &str) -> Extraction {
    let (_dir, resolver) = repo(GLOBS, files);
    let text = files.iter().find(|(f, _)| *f == rel).map(|(_, t)| *t).unwrap();
    crate::code::front::read(&resolver, rel, text)
}

const PATH: &str = r#"package os

import java.nio.file.Paths

/** A path. */
sealed trait BasePath {
  def last: String
  protected def check(s: String): Unit
}

trait Segmented extends BasePath with Ordered[Path]

class Path private[os] (val wrapped: java.nio.file.Path) extends BasePath with Segmented {
  def last: String = wrapped.toString
  private def secret(): Int = 1
  def size(): Int = secret() + this.secret()
  def this(s: String) = this(Paths.get(s))
  var hits = 0
}

object Path {
  def apply(s: String): Path = new Path(s)
  val root: Path = Path("/")
  type Alias = Path
  object / { def unapply(p: Path): Option[(Path, String)] = None }
}

case class Rel(ups: Int) extends BasePath {
  def last = ""
  def make(): Rel = { def local() = 1; Rel(local()) }
}

enum Color {
  case Red, Green
  def dark: Color = Red
}

private object Hidden
"#;

const PACKAGE: &str = r#"package object os {
  def pwd: Path = Path("/")
  type Generator[T] = Iterator[T]
  private val home = 1
  private object _home { lazy val value = 2 }
}
"#;

const OPS: &str = r#"package os
package ops

object copy {
  def apply(from: Path, to: Path): Unit = {
    val p = Path("x")
    p.size()
    os.pwd
    makeDir.all(from)
    List(1).map { case (n, m) => m.size() }
  }
  object into { def apply(p: Path) = copy(p, p) }
}

object makeDir { object all { def apply(p: Path): Unit = () } }
"#;

const TEST_UTIL: &str = "package test.os\n\nobject TestUtil {\n  def prep[T](f: os.Path => T): T = f(os.Path(\"/\"))\n}\n";

const TESTS: &str = r#"package test.os

import os._
import os.Path.root
import os.{makeDir, Rel => R, Color => _}
import TestUtil.prep
import scala.util.Random

object PathTests {
  def tests(): Unit = {
    prep { wd => Path("a") }
    os.ops.copy(root, root)
    makeDir.all(root)
    Rel(1).make()
    Random.nextInt()
    val g: Generator[os.Path] = null
  }
}
"#;

const FILES: [(&str, &str); 5] = [
    ("os/src/Path.scala", PATH),
    ("os/src/package.scala", PACKAGE),
    ("os/src/ops/Copy.scala", OPS),
    ("os/test/src/TestUtil.scala", TEST_UTIL),
    ("os/test/src/PathTests.scala", TESTS),
];

#[test]
fn a_file_declares_its_types_their_members_and_its_top_level() {
    let ex = extract(&FILES, "os/src/Path.scala");
    assert_eq!(ids(&ex), [
        "file:os/src/Path.scala",
        "sym:os/src/Path.scala::BasePath",
        "sym:os/src/Path.scala::BasePath.check",
        "sym:os/src/Path.scala::BasePath.last",
        "sym:os/src/Path.scala::Color",
        "sym:os/src/Path.scala::Color.Green",
        "sym:os/src/Path.scala::Color.Red",
        "sym:os/src/Path.scala::Color.dark",
        "sym:os/src/Path.scala::Hidden",
        "sym:os/src/Path.scala::Path",
        "sym:os/src/Path.scala::Path./",
        "sym:os/src/Path.scala::Path./.unapply",
        "sym:os/src/Path.scala::Path.Alias",
        "sym:os/src/Path.scala::Path.apply",
        "sym:os/src/Path.scala::Path.hits",
        "sym:os/src/Path.scala::Path.last",
        "sym:os/src/Path.scala::Path.root",
        "sym:os/src/Path.scala::Path.secret",
        "sym:os/src/Path.scala::Path.size",
        "sym:os/src/Path.scala::Rel",
        "sym:os/src/Path.scala::Rel.last",
        "sym:os/src/Path.scala::Rel.make",
        "sym:os/src/Path.scala::Segmented",
    ], "a class and its companion share an id, an auxiliary constructor and a block's `def` declare nothing");
    assert_eq!(lines(&ex, EdgeKind::Declares), [
        "file:os/src/Path.scala -> sym:os/src/Path.scala::BasePath [export]",
        "file:os/src/Path.scala -> sym:os/src/Path.scala::Color [export]",
        "file:os/src/Path.scala -> sym:os/src/Path.scala::Hidden []",
        "file:os/src/Path.scala -> sym:os/src/Path.scala::Path [export]",
        "file:os/src/Path.scala -> sym:os/src/Path.scala::Rel [export]",
        "file:os/src/Path.scala -> sym:os/src/Path.scala::Segmented [export]",
        "sym:os/src/Path.scala::BasePath -> sym:os/src/Path.scala::BasePath.check []",
        "sym:os/src/Path.scala::BasePath -> sym:os/src/Path.scala::BasePath.last [export]",
        "sym:os/src/Path.scala::Color -> sym:os/src/Path.scala::Color.Green [export]",
        "sym:os/src/Path.scala::Color -> sym:os/src/Path.scala::Color.Red [export]",
        "sym:os/src/Path.scala::Color -> sym:os/src/Path.scala::Color.dark [export]",
        "sym:os/src/Path.scala::Path -> sym:os/src/Path.scala::Path./ [export]",
        "sym:os/src/Path.scala::Path -> sym:os/src/Path.scala::Path.Alias [export]",
        "sym:os/src/Path.scala::Path -> sym:os/src/Path.scala::Path.apply [export]",
        "sym:os/src/Path.scala::Path -> sym:os/src/Path.scala::Path.hits [export]",
        "sym:os/src/Path.scala::Path -> sym:os/src/Path.scala::Path.last [export]",
        "sym:os/src/Path.scala::Path -> sym:os/src/Path.scala::Path.root [export]",
        "sym:os/src/Path.scala::Path -> sym:os/src/Path.scala::Path.secret []",
        "sym:os/src/Path.scala::Path -> sym:os/src/Path.scala::Path.size [export]",
        "sym:os/src/Path.scala::Path./ -> sym:os/src/Path.scala::Path./.unapply [export]",
        "sym:os/src/Path.scala::Rel -> sym:os/src/Path.scala::Rel.last [export]",
        "sym:os/src/Path.scala::Rel -> sym:os/src/Path.scala::Rel.make [export]",
    ], "`private` and `protected` hide a name; `private[os]` keeps it visible across its package");
}

#[test]
fn a_package_object_holds_its_package_level_names() {
    let ex = extract(&FILES, "os/src/package.scala");
    assert_eq!(ids(&ex), [
        "file:os/src/package.scala",
        "sym:os/src/package.scala::Generator",
        "sym:os/src/package.scala::_home",
        "sym:os/src/package.scala::_home.value",
        "sym:os/src/package.scala::home",
        "sym:os/src/package.scala::pwd",
    ], "the package object is a namespace, not a definition");
    assert_eq!(lines(&ex, EdgeKind::Declares), [
        "file:os/src/package.scala -> sym:os/src/package.scala::Generator [export]",
        "file:os/src/package.scala -> sym:os/src/package.scala::_home []",
        "file:os/src/package.scala -> sym:os/src/package.scala::home []",
        "file:os/src/package.scala -> sym:os/src/package.scala::pwd [export]",
        "sym:os/src/package.scala::_home -> sym:os/src/package.scala::_home.value [export]",
    ]);
}

#[test]
fn a_type_extends_each_base_of_its_clause_and_writes_its_type_arguments() {
    let ex = extract(&FILES, "os/src/Path.scala");
    assert_eq!(lines(&ex, EdgeKind::Extends), [
        "sym:os/src/Path.scala::Path -> sym:os/src/Path.scala::BasePath []",
        "sym:os/src/Path.scala::Path -> sym:os/src/Path.scala::Segmented []",
        "sym:os/src/Path.scala::Rel -> sym:os/src/Path.scala::BasePath []",
        "sym:os/src/Path.scala::Segmented -> sym:os/src/Path.scala::BasePath []",
    ], "every base of `extends A with B` is read, and `Ordered` is no file's");
    assert_eq!(lines(&ex, EdgeKind::References), [
        "sym:os/src/Path.scala::Path./.unapply -> sym:os/src/Path.scala::Path [type]",
        "sym:os/src/Path.scala::Segmented -> sym:os/src/Path.scala::Path [type]",
    ], "a base's type argument is a type it is written with; a member naming its own owner writes nothing");
    assert_eq!(lines(&ex, EdgeKind::Calls), ["sym:os/src/Path.scala::Path.size -> sym:os/src/Path.scala::Path.secret []"],
        "a bare call and `this.` name the owner's member; an auxiliary constructor's `this(...)` and a block's own `def` name nothing");
}

#[test]
fn a_nested_package_clause_reads_both_packages_across_directories() {
    let ex = extract(&FILES, "os/src/ops/Copy.scala");
    assert_eq!(lines(&ex, EdgeKind::Calls), [
        "sym:os/src/ops/Copy.scala::copy.apply -> sym:os/src/Path.scala::Path []",
        "sym:os/src/ops/Copy.scala::copy.apply -> sym:os/src/ops/Copy.scala::makeDir.all []",
        "sym:os/src/ops/Copy.scala::copy.into.apply -> sym:os/src/ops/Copy.scala::copy []",
    ], "a call on a value, `p.size()` or a case's `m.size()`, needs the value's type, and `os.pwd` with no call is a read");
    assert_eq!(lines(&ex, EdgeKind::References), [
        "sym:os/src/ops/Copy.scala::copy.apply -> sym:os/src/Path.scala::Path [type]",
        "sym:os/src/ops/Copy.scala::copy.into.apply -> sym:os/src/Path.scala::Path [type]",
        "sym:os/src/ops/Copy.scala::makeDir.all.apply -> sym:os/src/Path.scala::Path [type]",
    ], "`package os` then `package ops` reads `os` in another directory with no import");
}

#[test]
fn an_import_reads_a_wildcard_a_name_a_selector_and_a_path_under_its_own_package() {
    let ex = extract(&FILES, "os/test/src/PathTests.scala");
    assert_eq!(lines(&ex, EdgeKind::Imports), [
        "file:os/test/src/PathTests.scala -> file:os/src/Path.scala [*]",
        "file:os/test/src/PathTests.scala -> file:os/src/package.scala [*]",
        "file:os/test/src/PathTests.scala -> file:os/test/src/TestUtil.scala [*]",
    ], "`os._` opens the files of `os`, not of `os.ops`; a renamed or hidden selector and `scala.util` name nothing");
    assert_eq!(lines(&ex, EdgeKind::Calls), [
        "sym:os/test/src/PathTests.scala::PathTests.tests -> sym:os/src/Path.scala::Path []",
        "sym:os/test/src/PathTests.scala::PathTests.tests -> sym:os/src/Path.scala::Rel []",
        "sym:os/test/src/PathTests.scala::PathTests.tests -> sym:os/src/ops/Copy.scala::copy []",
        "sym:os/test/src/PathTests.scala::PathTests.tests -> sym:os/test/src/TestUtil.scala::TestUtil.prep []",
    ], "`TestUtil.prep` is read under the file's own `test.os`; `makeDir` is `os.ops`', not `os`', and a call on a result or an outside object names nothing");
    assert_eq!(lines(&ex, EdgeKind::References), [
        "sym:os/test/src/PathTests.scala::PathTests.tests -> sym:os/src/Path.scala::Path [type]",
        "sym:os/test/src/PathTests.scala::PathTests.tests -> sym:os/src/package.scala::Generator [type]",
    ], "a package object's type is its package's, and `os.Path` is read by its full path");
}

#[test]
fn a_name_two_variant_files_declare_reaches_both() {
    let files = [
        ("os/src-2/Macros.scala", "package os

trait PathMacros { def lit(): Int = 1 }
"),
        ("os/src-3/Macros.scala", "package os

trait PathMacros { def lit(): Int = 2 }
"),
        ("os/src/Path.scala", "package os

private object Hidden { def x(): Int = 0 }
"),
        ("os/src/Use.scala", "package os

object Use extends PathMacros {
  def run(): Unit = { Hidden.x(); lit() }
}
"),
    ];
    let ex = extract(&files, "os/src/Use.scala");
    assert_eq!(lines(&ex, EdgeKind::Extends), [
        "sym:os/src/Use.scala::Use -> sym:os/src-2/Macros.scala::PathMacros []",
        "sym:os/src/Use.scala::Use -> sym:os/src-3/Macros.scala::PathMacros []",
    ], "a source set per Scala version declares one name twice, and either may be the one built");
    assert_eq!(lines(&ex, EdgeKind::Calls), ["sym:os/src/Use.scala::Use.run -> sym:os/src/Path.scala::Hidden.x []"],
        "a member is read by its path though its object is `private`, which Scala 2 keeps visible across the package; an inherited member is not read");
}

#[test]
fn an_id_in_a_comment_or_string_is_cited_by_what_holds_it() {
    let src = "package os\n\n// ADR-004 explains the wait\n/* ADR-005 /* nested */ too */\nobject Use {\n  def run(): String = s\"FR-APP-54 $x\"\n}\n";
    let ex = extract(&[("os/src/Use.scala", src)], "os/src/Use.scala");
    assert_eq!(lines(&ex, EdgeKind::References), [
        "file:os/src/Use.scala -> ADR-004 [comment]",
        "file:os/src/Use.scala -> ADR-005 [comment]",
        "sym:os/src/Use.scala::Use.run -> FR-APP-54 [string]",
    ]);
}

#[test]
fn a_scala_file_and_a_script_are_read_and_a_build_definition_is_not() {
    use crate::code::lang::{Family, Lang};
    assert_eq!(Lang::of("os/src/Path.scala"), Some(Lang::Scala));
    assert_eq!(Lang::of("build.sc"), Some(Lang::Scala));
    assert_eq!(Lang::of("build.sbt"), None);
    assert_eq!(Lang::Scala.family(), Family::Scala);
}

#[test]
fn a_definition_with_an_indented_body_ends_on_its_last_line_not_the_next_ones() {
    let src = "package os\n\nobject Ord {\n  implicit val ordering: Ordering[Int] =\n    Ordering.by((n: Int) => n)\n\n  val zero: Int = 0\n  def one(): Int = { 1 }\n}\n";
    let ex = extract(&[("os/src/Ord.scala", src)], "os/src/Ord.scala");
    let span = |id: &str| ex.nodes.iter().find(|n| n.id == id).map(|n| (n.line, n.end)).unwrap();
    assert_eq!(span("sym:os/src/Ord.scala::Ord.ordering"), (4, 5), "the block ends where the next token starts");
    assert_eq!(span("sym:os/src/Ord.scala::Ord.zero"), (7, 7));
    assert_eq!(span("sym:os/src/Ord.scala::Ord"), (3, 9));
}
