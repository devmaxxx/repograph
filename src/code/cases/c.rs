//! C's shapes, each on inline files, so the reading of a declaration, an include and a call is pinned
//! by the assertion rather than by a corpus. C is a spec row and its two queries, with no hook.

use crate::code::prose::testing::{ids, lines, repo};
use crate::model::{EdgeKind, Extraction};

const GLOBS: &[&str] = &["**/*.c", "**/*.h"];

fn extract(files: &[(&str, &str)], rel: &str) -> Extraction {
    let (_dir, resolver) = repo(GLOBS, files);
    let text = files.iter().find(|(f, _)| *f == rel).map(|(_, t)| *t).unwrap();
    crate::code::front::read(&resolver, rel, text)
}

const DECLS: &str = r#"#include <stdio.h>

#define MAX 10
/* Bounds a value. */
#define CLAMP(x) ((x) > MAX ? MAX : (x))

/* A reply from the server. */
typedef struct reply {
    int type, len;
    char *str;
    struct reply **element;
    void (*free_fn)(void *);
    char buf[16];
    struct {
        int port;
    } tcp;
} reply;

typedef struct {
    int fd;
} conn;

struct node {
    struct node *next;
};

union value {
    long i;
    double d;
};

enum kind { KIND_A, KIND_B = 2 };

typedef enum { MODE_X } mode;

typedef int handle;
typedef char *text;
typedef void (callback)(void *);
typedef void (*hook)(int);
typedef struct node node_t;

reply *parse(const char *s);

/* Frees a reply. */
void freeReply(reply *r)
{
    (void)r;
}

static int
count(const char *s,
      int n) {
    return n;
}

static reply **children(reply *r) { return r->element; }
"#;

#[test]
fn the_file_declares_its_functions_types_members_and_function_macros() {
    let ex = extract(&[("reply.h", DECLS)], "reply.h");
    assert_eq!(ids(&ex), [
        "file:reply.h",
        "sym:reply.h::CLAMP",
        "sym:reply.h::callback",
        "sym:reply.h::children",
        "sym:reply.h::conn",
        "sym:reply.h::conn.fd",
        "sym:reply.h::count",
        "sym:reply.h::freeReply",
        "sym:reply.h::handle",
        "sym:reply.h::hook",
        "sym:reply.h::kind",
        "sym:reply.h::kind.KIND_A",
        "sym:reply.h::kind.KIND_B",
        "sym:reply.h::mode",
        "sym:reply.h::mode.MODE_X",
        "sym:reply.h::node",
        "sym:reply.h::node.next",
        "sym:reply.h::node_t",
        "sym:reply.h::reply",
        "sym:reply.h::reply.buf",
        "sym:reply.h::reply.element",
        "sym:reply.h::reply.free_fn",
        "sym:reply.h::reply.len",
        "sym:reply.h::reply.port",
        "sym:reply.h::reply.str",
        "sym:reply.h::reply.tcp",
        "sym:reply.h::reply.type",
        "sym:reply.h::text",
        "sym:reply.h::value",
        "sym:reply.h::value.d",
        "sym:reply.h::value.i",
    ], "a prototype and an object-like macro declare nothing, a typedef'd struct is one type under its typedef name, and a \
        field qualifies nothing, so an unnamed struct's members are its holder's");
    assert_eq!(lines(&ex, EdgeKind::Declares).iter().filter(|l| !l.ends_with("[export]")).collect::<Vec<_>>(), [
        "file:reply.h -> sym:reply.h::children []",
        "file:reply.h -> sym:reply.h::count []",
    ], "only `static` keeps a name in its file");
    let node = |id: &str| ex.nodes.iter().find(|n| n.id == id).unwrap();
    let reply = node("sym:reply.h::reply");
    assert_eq!((reply.line, reply.end), (8, 17));
    assert_eq!(reply.body, "/* A reply from the server. */\ntypedef struct reply {");
    assert_eq!(node("sym:reply.h::freeReply").body, "/* Frees a reply. */\nvoid freeReply(reply *r)");
    assert_eq!(node("sym:reply.h::count").body, "static int count(const char *s, int n)", "a signature over several lines reads as one");
    assert_eq!(node("sym:reply.h::CLAMP").body, "/* Bounds a value. */\n#define CLAMP(x) ((x) > MAX ? MAX : (x))");
    let tcp = node("sym:reply.h::reply.tcp");
    assert_eq!((tcp.line, tcp.end), (14, 16), "a member's span is its whole declaration");
}

#[test]
fn a_quoted_include_names_a_file_beside_the_includer_then_under_the_roots_and_an_angled_one_nothing() {
    let files = [
        ("src/net/net.c", "#include \"net.h\"\n#include \"util.h\"\n#include \"api.h\"\n#include \"core.h\"\n#include \"gone.h\"\n#include <stdio.h>\n#include <api.h>\n"),
        ("src/net/net.h", ""),
        ("util.h", ""),
        ("include/api.h", ""),
        ("src/core.h", ""),
    ];
    assert_eq!(lines(&extract(&files, "src/net/net.c"), EdgeKind::Imports), [
        "file:src/net/net.c -> file:include/api.h [*]",
        "file:src/net/net.c -> file:src/core.h [*]",
        "file:src/net/net.c -> file:src/net/net.h [*]",
        "file:src/net/net.c -> file:util.h [*]",
    ], "`<api.h>` names a file on the compiler's path, even where the repository holds one");
}

#[test]
fn an_include_beside_the_includer_wins_over_one_under_a_root() {
    let files = [("lib/a.c", "#include \"x.h\"\n"), ("lib/x.h", ""), ("x.h", "")];
    assert_eq!(lines(&extract(&files, "lib/a.c"), EdgeKind::Imports), ["file:lib/a.c -> file:lib/x.h [*]"]);
}

#[test]
fn a_call_reaches_the_own_file_first_else_the_one_definition_the_build_links() {
    let files = [
        ("api.h", "int connect_to(const char *host);\nvoid helper(void);\nint twice(void);\n"),
        ("net.c", "#include \"api.h\"\nint connect_to(const char *host) { return 0; }\nstatic void helper(void) {}\nint twice(void) { return 1; }\n"),
        ("io.c", "int twice(void) { return 2; }\nstatic void helper(void) {}\n"),
        ("main.c", "#include \"api.h\"\n\nstatic void log_line(void) {}\n\nint main(void) {\n    connect_to(\"x\");\n    helper();\n    twice();\n    log_line();\n    printf(\"%d\", 1);\n    return 0;\n}\n"),
    ];
    assert_eq!(lines(&extract(&files, "main.c"), EdgeKind::Calls), [
        "sym:main.c::main -> sym:main.c::log_line []",
        "sym:main.c::main -> sym:net.c::connect_to []",
    ], "a header's prototype is the definition's, a `static` one is its file's, two definitions link as neither, and libc is outside");
    assert_eq!(lines(&extract(&files, "io.c"), EdgeKind::Calls), Vec::<String>::new());
}

#[test]
fn a_definition_in_the_own_file_shadows_every_other_one() {
    let files = [
        ("a.c", "void run(void) {}\nvoid go(void) { run(); }\n"),
        ("b.c", "void run(void) {}\n"),
        ("c.c", "void start(void) { run(); }\n"),
    ];
    assert_eq!(lines(&extract(&files, "a.c"), EdgeKind::Calls), ["sym:a.c::go -> sym:a.c::run []"]);
    assert!(lines(&extract(&files, "c.c"), EdgeKind::Calls).is_empty(), "two files define `run`, and neither is the one");
}

#[test]
fn a_function_like_macro_is_called_as_a_function_is() {
    let files = [
        ("util.h", "#define MIN(a, b) ((a) < (b) ? (a) : (b))\n"),
        ("a.c", "#include \"util.h\"\nint f(int x) { return MIN(x, 3); }\n"),
    ];
    assert_eq!(lines(&extract(&files, "a.c"), EdgeKind::Calls), ["sym:a.c::f -> sym:util.h::MIN []"]);
}

#[test]
fn a_parameter_or_local_named_like_a_function_hides_it_in_its_body_alone() {
    let files = [
        ("cb.c", "void cb(int x) {}\nvoid step(int x) {}\n"),
        ("a.c", r#"void run(void (*cb)(int)) {
    cb(1);
}

void loop(int step) {
    for (int cb = 0; cb < 1; cb++) {}
    cb(2);
}

void other(void) {
    void (*step)(int) = 0;
    step(3);
    {
        int cb = 0;
    }
    cb(4);
}
"#),
    ];
    assert_eq!(lines(&extract(&files, "a.c"), EdgeKind::Calls), [
        "sym:a.c::loop -> sym:cb.c::cb []",
        "sym:a.c::other -> sym:cb.c::cb []",
    ], "a `for` binding and a block's local end with their statement, a parameter covers its whole body");
}

#[test]
fn a_call_through_a_pointer_or_a_member_stays_unresolved() {
    let files = [
        ("hooks.c", "void on_read(int fd) {}\n"),
        ("a.c", "struct ctx { void (*on_read)(int); };\nvoid fire(struct ctx *c, void (**fp)(int)) {\n    c->on_read(1);\n    (*fp)(2);\n}\n"),
    ];
    assert!(lines(&extract(&files, "a.c"), EdgeKind::Calls).is_empty(), "a member's target is a value read at run time");
}

#[test]
fn a_variant_chosen_by_the_preprocessor_is_one_symbol_and_its_definitions_are_read_in_any_branch() {
    let src = "#ifdef _WIN32\nstatic int sock_init(void) { return 1; }\n#else\nstatic int sock_init(void) { return 0; }\n#endif\n\nint start(void) { return sock_init(); }\n";
    let ex = extract(&[("net.c", src)], "net.c");
    assert_eq!(ids(&ex), ["file:net.c", "sym:net.c::sock_init", "sym:net.c::start"]);
    let init = ex.nodes.iter().find(|n| n.id == "sym:net.c::sock_init").unwrap();
    assert_eq!(init.line, 2, "the first variant in source order holds the id");
    assert_eq!(lines(&ex, EdgeKind::Calls), ["sym:net.c::start -> sym:net.c::sock_init []"]);
}

#[test]
fn an_extern_c_block_holds_definitions_as_the_file_does() {
    let src = "#ifdef __cplusplus\nextern \"C\" {\n#endif\n\ntypedef struct ctx { int fd; } ctx;\nint open_ctx(ctx *c);\n\n#ifdef __cplusplus\n}\n#endif\n";
    assert_eq!(ids(&extract(&[("api.h", src)], "api.h")), ["file:api.h", "sym:api.h::ctx", "sym:api.h::ctx.fd"]);
}

#[test]
fn a_function_pointer_returning_a_pointer_and_a_standard_name_redefined_are_declared() {
    let src = "typedef intptr_t ssize_t;\ntypedef void *(*alloc_fn)(size_t);\ntypedef struct fns {\n    void *(*mallocFn)(size_t);\n} fns;\n";
    assert_eq!(ids(&extract(&[("alloc.h", src)], "alloc.h")), [
        "file:alloc.h",
        "sym:alloc.h::alloc_fn",
        "sym:alloc.h::fns",
        "sym:alloc.h::fns.mallocFn",
        "sym:alloc.h::ssize_t",
    ], "the grammar reads `ssize_t` as a primitive type");
}

#[test]
fn a_type_a_function_body_or_a_variable_s_declaration_writes_declares_nothing() {
    let src = r#"static struct { int port; } config;
struct pair { int a; } first;
enum color { RED } paint;

void run(void) {
    static const struct { const char *text; double val; } cases[] = {{"1", 1}};
    struct local { int x; };
    typedef int count;
    enum { LOW } level;
}
"#;
    assert_eq!(ids(&extract(&[("a.c", src)], "a.c")), ["file:a.c", "sym:a.c::RED", "sym:a.c::run"], "an enumerator outside a function is a constant wherever its enum stands");
}

#[test]
fn an_id_in_a_comment_or_string_is_cited_by_what_holds_it() {
    let ex = extract(&[("a.c", "/* ADR-004 explains the wait */\nvoid wait(void) {\n    puts(\"FR-APP-54\");\n}\n")], "a.c");
    assert_eq!(lines(&ex, EdgeKind::References), ["file:a.c -> ADR-004 [comment]", "sym:a.c::wait -> FR-APP-54 [string]"]);
}

#[test]
fn a_c_source_and_a_header_are_read_by_their_extension() {
    use crate::code::lang::{Family, Lang};
    assert_eq!(Lang::of("src/net.c"), Some(Lang::C));
    // A header is read as C++, whose grammar parses C's headers no worse and reads a class.
    assert_eq!(Lang::of("include/net.h"), Some(Lang::Cpp));
    assert_eq!(Lang::C.family(), Family::C);
    assert_eq!(Lang::Cpp.family(), Family::C);
}

#[test]
fn an_array_member_and_a_named_type_nested_in_a_member_are_read_as_their_holder_does() {
    let src = "struct s { char *names[4]; int m[2][2]; unsigned flags : 3; int a, *b[2]; struct s *peers[2]; struct in { int x; } n; };\n";
    assert_eq!(ids(&extract(&[("a.c", src)], "a.c")), [
        "file:a.c",
        "sym:a.c::s",
        "sym:a.c::s.a",
        "sym:a.c::s.b",
        "sym:a.c::s.flags",
        "sym:a.c::s.m",
        "sym:a.c::s.n",
        "sym:a.c::s.names",
        "sym:a.c::s.peers",
        "sym:a.c::s.x",
    ], "a pointer array and a two-dimensional one are members, and a type written in a member is no type apart, so its members are the holder's");
}

#[test]
fn a_local_pointer_pointer_or_array_hides_a_function_named_like_it() {
    let files = [
        ("b.c", "void p(int x) {}\nvoid q(int x) {}\nvoid r(int x) {}\n"),
        ("a.c", "void f(void) { char **p; char *q[3]; int r[2][2]; p(1); q(2); r(3); }\n"),
    ];
    assert!(lines(&extract(&files, "a.c"), EdgeKind::Calls).is_empty());
}
