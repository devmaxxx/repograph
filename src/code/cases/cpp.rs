//! C++'s shapes, each on inline files, so the reading of a class, an out-of-line method, an include
//! and a call is pinned by the assertion rather than by a corpus. C++ is a spec row, its two queries
//! and the attribute-macro hook.

use crate::code::prose::testing::{ids, lines, repo};
use crate::model::{EdgeKind, Extraction};

const GLOBS: &[&str] = &["**/*.cc", "**/*.h", "**/*.hpp"];

fn extract(files: &[(&str, &str)], rel: &str) -> Extraction {
    let (_dir, resolver) = repo(GLOBS, files);
    let text = files.iter().find(|(f, _)| *f == rel).map(|(_, t)| *t).unwrap();
    crate::code::front::read(&resolver, rel, text)
}

const CLASSES: &str = r#"#include "base.h"

namespace store {

// Holds one table's rows.
class EXPORT Table : public Base {
 public:
  explicit Table(int n) : n_(n) {}
  ~Table() override;
  virtual int Size() const = 0;
  Table& operator=(const Table&) = delete;
  bool operator==(const Table& o) const { return n_ == o.n_; }
  const char* Name() const { return "t"; }
  static Table* Make();
  friend bool Same(const Table& a, const Table& b) { return a == b; }

  struct Row {
    int key;
    Row* next;
  };
  enum Mode { kRead, kWrite = 2 };
  using Key = int;

 private:
#if defined(FAST)
  int fast_;
#endif
  int n_ GUARDED_BY(mu_);
  char buf_[8];
  int (*hook_)(int);
};

Table::~Table() {}

Table* Table::Make() { return nullptr; }

template <typename K>
struct Node {
  K key;
};

typedef struct { int fd; } conn;
typedef int handle;

namespace {
int Hidden() { return 1; }
}  // namespace

static int Local() { return 2; }

int Free() { return Hidden() + Local(); }

}  // namespace store
"#;

#[test]
fn a_file_declares_its_classes_their_members_and_the_methods_its_bodies_define() {
    let ex = extract(&[("table.h", CLASSES)], "table.h");
    assert_eq!(ids(&ex), [
        "file:table.h",
        "sym:table.h::Free",
        "sym:table.h::Hidden",
        "sym:table.h::Local",
        "sym:table.h::Node",
        "sym:table.h::Node.key",
        "sym:table.h::Same",
        "sym:table.h::Table",
        "sym:table.h::Table.Key",
        "sym:table.h::Table.Make",
        "sym:table.h::Table.Mode",
        "sym:table.h::Table.Mode.kRead",
        "sym:table.h::Table.Mode.kWrite",
        "sym:table.h::Table.Name",
        "sym:table.h::Table.Row",
        "sym:table.h::Table.Row.key",
        "sym:table.h::Table.Row.next",
        "sym:table.h::Table.Size",
        "sym:table.h::Table.Table",
        "sym:table.h::Table.buf_",
        "sym:table.h::Table.fast_",
        "sym:table.h::Table.hook_",
        "sym:table.h::Table.n_",
        "sym:table.h::Table.operator==",
        "sym:table.h::Table.~Table",
        "sym:table.h::conn",
        "sym:table.h::conn.fd",
        "sym:table.h::handle",
    ], "a namespace qualifies nothing, a friend defined in its class is the file's, a deleted or bodiless method declares \
        nothing but a pure virtual one does, and a member under `#if` is the class's");
    assert_eq!(lines(&ex, EdgeKind::Declares).iter().filter(|l| !l.ends_with("[export]")).collect::<Vec<_>>(), [
        "file:table.h -> sym:table.h::Hidden []",
        "file:table.h -> sym:table.h::Local []",
    ], "an unnamed namespace keeps a name in its file, as `static` does");
    let table = ex.nodes.iter().find(|n| n.id == "sym:table.h::Table").unwrap();
    assert_eq!((table.line, table.end), (6, 31), "a macro between `class` and the name leaves the class whole");
}

#[test]
fn an_out_of_line_method_is_named_by_the_class_nearest_it_and_a_template_around_that_class() {
    let src = r#"namespace db {
int Cache::Lookup(int k) { return k; }
const char* Cache::Name() const { return "c"; }
Cache& Cache::operator=(const Cache& o) { return *this; }
char** Cache::Keys() { return nullptr; }
db::Cache::Cache() {}
template <typename K>
void List<K>::Insert(const K& k) {}
template <typename K>
bool List<K>::Iterator::Valid() const { return true; }
}  // namespace db
"#;
    assert_eq!(ids(&extract(&[("cache.cc", src)], "cache.cc")), [
        "file:cache.cc",
        "sym:cache.cc::Cache.Cache",
        "sym:cache.cc::Cache.Keys",
        "sym:cache.cc::Cache.Lookup",
        "sym:cache.cc::Cache.Name",
        "sym:cache.cc::Cache.operator=",
        "sym:cache.cc::List.Insert",
        "sym:cache.cc::List.Iterator.Valid",
    ], "a namespace before the class qualifies nothing; a template before it is a class that holds it");
}

#[test]
fn a_function_after_a_macro_the_parser_reads_as_a_scope_is_a_function() {
    let src = "#define api_inline inline\napi_inline\nint parse_digit(char c) { return c - '0'; }\n";
    assert_eq!(ids(&extract(&[("parse.h", src)], "parse.h")), ["file:parse.h", "sym:parse.h::parse_digit"]);
}

#[test]
fn a_macro_alone_as_a_class_member_leaves_the_members_after_it_whole() {
    let src = "class Adapter : public QObject {\n  Q_OBJECT\n public:\n  void Run() {}\n private:\n  int fd_;\n};\n";
    assert_eq!(ids(&extract(&[("adapter.h", src)], "adapter.h")), [
        "file:adapter.h",
        "sym:adapter.h::Adapter",
        "sym:adapter.h::Adapter.Run",
        "sym:adapter.h::Adapter.fd_",
    ]);
}

#[test]
fn a_header_s_include_and_a_cpp_extension_resolve_as_c_s_do() {
    let files = [
        ("src/db/db.cc", "#include \"db.h\"\n#include \"util/coding.hpp\"\n#include <vector>\n"),
        ("src/db/db.h", ""),
        ("util/coding.hpp", ""),
    ];
    assert_eq!(lines(&extract(&files, "src/db/db.cc"), EdgeKind::Imports), [
        "file:src/db/db.cc -> file:src/db/db.h [*]",
        "file:src/db/db.cc -> file:util/coding.hpp [*]",
    ]);
}

#[test]
fn a_bare_call_in_a_method_names_its_class_s_member_first_wherever_the_class_is_declared() {
    let files = [
        ("table.h", "class Table {\n public:\n  int Size() const { return 0; }\n  void Clear();\n  void Grow();\n};\n"),
        ("util.cc", "int Size() { return 1; }\nvoid Grow() {}\nvoid Flush() {}\n"),
        ("table.cc", r#"#include "table.h"
void Table::Clear() {
  Size();
  Grow();
  Clear();
  Flush();
}
void Table::Grow() { Grow(); }
void Reset() { Size(); }
"#),
    ];
    assert_eq!(lines(&extract(&files, "table.cc"), EdgeKind::Calls), [
        "sym:table.cc::Reset -> sym:util.cc::Size []",
        "sym:table.cc::Table.Clear -> sym:table.cc::Table.Grow []",
        "sym:table.cc::Table.Clear -> sym:table.h::Table.Size []",
        "sym:table.cc::Table.Clear -> sym:util.cc::Flush []",
    ], "a member found in the class's header or in the own file wins over a free function, a method calling itself \
        never falls through to one, and a free function calls the free one");
}

#[test]
fn a_namespace_qualified_call_names_the_function_and_a_member_or_class_qualified_one_stays_unresolved() {
    let files = [
        ("env.cc", "namespace io {\nint Open() { return 0; }\n}\nclass Env { public: static Env* Default(); };\nEnv* Env::Default() { return nullptr; }\n"),
        ("main.cc", r#"#include "env.cc"
struct Db { void Close() {} };
int main() {
  io::Open();
  ::io::Open();
  Env::Default();
  Db d;
  d.Close();
  Db* p = &d;
  p->Close();
  return 0;
}
"#),
    ];
    assert_eq!(lines(&extract(&files, "main.cc"), EdgeKind::Calls), ["sym:main.cc::main -> sym:env.cc::Open []"],
        "a class's static member and a value's member need a type, which nothing here reads");
}

#[test]
fn a_cpp_source_and_header_are_read_by_their_extension_and_cuda_is_not() {
    use crate::code::lang::{Family, Lang};
    for path in ["a.cc", "a.cpp", "a.cxx", "a.c++", "a.hh", "a.hpp", "a.hxx", "a.h++", "a.ino", "a.h"] {
        assert_eq!(Lang::of(path), Some(Lang::Cpp), "{path}");
    }
    assert_eq!(Lang::of("kernel.cu"), None, "CUDA's grammar is not C++'s");
    assert_eq!(Lang::Cpp.family(), Family::C);
}
