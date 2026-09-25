"""What counts as a reference, on the lines that were miscounted."""

import subprocess
import tempfile
import unittest
from pathlib import Path

from truth import blank_kotlin, blank_typescript, changed_symbols, declaration_end, declarations
import truth as T


def changes_of(base_files: dict[str, str], edits: dict[str, str | None]) -> dict:
    """`changed_symbols` over a throwaway repository: `base_files` committed, `edits` on top.

    A real `git diff` rather than a hand-written hunk list, because the hunk parsing and
    the declaration reader have to agree about line numbers and only git can settle that.
    An edit of `None` deletes the file.
    """
    with tempfile.TemporaryDirectory() as tmp:
        repo = Path(tmp)
        # An empty hooks path and no signing: this repository borrows the developer's
        # global git config, and a commit hook or a signing key would fail the test.
        git = ["git", "-c", "core.hooksPath=", "-c", "commit.gpgsign=false",
               "-c", "user.email=truth@test", "-c", "user.name=truth"]
        subprocess.run([*git, "init", "-q", "-b", "main", str(repo)], check=True, capture_output=True)
        for rel, body in base_files.items():
            (repo / rel).parent.mkdir(parents=True, exist_ok=True)
            (repo / rel).write_text(body, encoding="utf8")
        subprocess.run([*git, "add", "-A"], cwd=repo, check=True, capture_output=True)
        subprocess.run([*git, "commit", "-q", "-m", "base"], cwd=repo, check=True, capture_output=True)
        for rel, body in edits.items():
            if body is None:
                (repo / rel).unlink()
                continue
            (repo / rel).parent.mkdir(parents=True, exist_ok=True)
            (repo / rel).write_text(body, encoding="utf8")
        return changed_symbols(repo, "HEAD")


class BlankSource(unittest.TestCase):
    """What counts as a reference, on the lines that were miscounted."""

    def test_a_name_inside_a_single_quoted_token_is_not_a_reference(self):
        # apps/api/test/loggingModule.spec.ts:11 — a logger's name, not a dependency.
        src = "const OUTBOX_LOGGER_TOKEN = 'PinoLogger:OutboxPublisher';\n"
        self.assertNotIn("OutboxPublisher", blank_typescript(src))

    def test_a_name_inside_an_error_message_is_not_a_reference(self):
        # apps/api/src/shared/db/database.service.ts:34 — prose about the interceptor.
        src = 'throw new Error("a route with :businessId behind TenantContextInterceptor");\n'
        self.assertNotIn("TenantContextInterceptor", blank_typescript(src))

    def test_a_template_literal_is_blanked_too(self):
        self.assertNotIn("AuthService", blank_typescript("log(`${x} AuthService`);\n"))

    def test_code_outside_strings_and_comments_survives_with_its_line_count(self):
        src = (
            "import { AuthService } from './auth.service.js'; // AuthService\n"
            "/* AuthService */\n"
            "new AuthService('x');\n"
        )
        out = blank_typescript(src)
        self.assertEqual(out.count("\n"), src.count("\n"))
        self.assertEqual(out.count("AuthService"), 2)

    def test_a_url_in_a_string_is_not_a_line_comment(self):
        self.assertIn("fetch(", blank_typescript("fetch('http://x/y');\n"))

    def test_a_line_comment_after_a_ternary_colon_is_still_a_comment(self):
        # The pattern this replaced excused every `//` preceded by a colon, to spare
        # `http://`, and so kept the prose after a ternary's colon as if it were code.
        self.assertNotIn("AuthService", blank_typescript("const x = a ? b : c; // AuthService\n"))

    def test_a_quote_inside_a_regex_literal_does_not_desync_the_rest_of_the_line(self):
        # The pattern this replaced opened a string on the quote inside `/'/` and closed it
        # on the next real string's opening quote, leaving that string's body un-blanked.
        self.assertNotIn("AuthService", blank_typescript("const r = /'/; const s = 'AuthService';\n"))

    def test_a_multi_line_template_literal_keeps_the_lines_after_it_numbered(self):
        # A template literal spanning several lines must not collapse them into
        # the line where it opens: everything after it still counts from its own line.
        src = "const q = `select *\nfrom t\nwhere x`;\nnew AuthService();\n"
        out = blank_typescript(src)
        self.assertEqual(out.count("\n"), src.count("\n"))
        self.assertIn("AuthService", out)

    def test_a_multi_line_block_comment_keeps_the_declaration_after_it_on_its_own_line(self):
        src = "/* a\nb */ export function foo() {}\n"
        out = blank_typescript(src)
        self.assertEqual(out.count("\n"), src.count("\n"))
        self.assertIn("export function foo", out.split("\n")[1])

    def test_a_self_closing_jsx_tag_does_not_open_a_regex_literal(self):
        src = "const W = <A x={1} /> && <B y={2} />;\nexport const after = 1;\n"
        self.assertEqual([name for _, name in declarations("a.tsx", src)[1]], ["W", "after"])


class ChangedSymbolsBlanksProse(unittest.TestCase):
    """Defect 1: the changes truth read every file raw, comments and strings included."""

    BASE = (
        "/*\n"
        "export function ghost(): number {\n"
        "  return 0;\n"
        "}\n"
        "*/\n"
        "export function real(): number {\n"
        "  return 1;\n"
        "}\n"
    )

    def test_a_declaration_written_inside_a_comment_is_not_a_changed_symbol(self):
        got = changes_of({"a.ts": self.BASE}, {"a.ts": self.BASE.replace("return 0;", "return 9;")})
        self.assertEqual(got["symbols"], {})

    def test_a_declaration_written_inside_a_template_literal_is_not_a_changed_symbol(self):
        base = (
            "export const SAMPLE = `\n"
            "export function ghost(): number {\n"
            "  return 0;\n"
            "}\n"
            "`;\n"
            "export function real(): number {\n"
            "  return 1;\n"
            "}\n"
        )
        got = changes_of({"a.ts": base}, {"a.ts": base.replace("return 0;", "return 9;")})
        self.assertEqual(got["symbols"], {})

    def test_a_comment_opener_inside_a_string_does_not_open_a_comment(self):
        # packages/ui/test/boundary.test.ts:44 — `'apps/*'` in a list of forbidden imports.
        # Blanking comments before strings ate everything to the next `*/` seventy lines
        # below, closing bracket included, and ran the declaration to the end of the file.
        src = "const FORBIDDEN = [\n  'apps/*',\n  'x',\n] as const;\n/* a */\n"
        lines, _ = declarations("a.ts", src)
        self.assertEqual(declaration_end(lines, 1), 4)

    def test_a_line_comment_opener_inside_a_string_does_not_open_a_comment(self):
        # packages/config/test/config-boundary.test.ts:61 — `'//'` is a legal tsconfig key.
        src = "const ALLOWED = new Set(['extends', '//']);\nexport const after = 1;\n"
        lines, _ = declarations("a.ts", src)
        self.assertEqual(declaration_end(lines, 1), 1)

    def test_both_readers_keep_the_line_count_they_were_given(self):
        # `changed_symbols` maps hunk line ranges onto declaration spans, so a line lost
        # in the blanking shifts every declaration under it.
        ts = "const q = `a\nb`;\n/* c\nd */\nexport function foo() {}\n"
        kt = 'val q = """a\nb"""\n/* c /* d\ne */ */\npublic fun foo(): Int = 1\n'
        self.assertEqual(len(declarations("a.ts", ts)[0]), len(ts.split("\n")))
        self.assertEqual(len(declarations("a.kt", kt)[0]), len(kt.split("\n")))


class TypescriptDeclarations(unittest.TestCase):
    """Finding 1: the reader was anchored at column zero and saw no class member."""

    def test_a_class_or_interface_member_counts_and_a_local_does_not(self):
        base = (
            "export class Registry {\n"
            "  private readonly entries = new Map<string, number>();\n"
            "\n"
            "  async count(): Promise<number> {\n"
            "    const local = 1;\n"
            "    return local;\n"
            "  }\n"
            "\n"
            "  get size(): number {\n"
            "    return 0;\n"
            "  }\n"
            "}\n"
            "\n"
            "export interface Rule {\n"
            "  readonly id: string;\n"
            "  run(x: number): void;\n"
            "}\n"
        )
        edit = (
            base.replace("new Map<string, number>()", "new Map<string, string>()")
            .replace("const local = 1;", "const local = 2;")
            .replace("readonly id: string;", "readonly id: number;")
        )
        got = changes_of({"a.ts": base}, {"a.ts": edit})
        self.assertEqual(got["symbols"], {"a.ts": ["Registry", "Rule", "count", "entries", "id"]})

    def test_a_statement_in_a_function_body_is_not_a_member(self):
        src = (
            "export function walk(items: number[]): number {\n"
            "  let total = 0;\n"
            "  for (const item of items) {\n"
            "    total += item;\n"
            "  }\n"
            "  return total;\n"
            "}\n"
        )
        self.assertEqual([name for _, name in declarations("a.ts", src)[1]], ["walk"])

    def test_a_constructor_is_not_a_name_and_nor_are_its_parameter_properties(self):
        src = (
            "export class A {\n"
            "  constructor(\n"
            "    private readonly x: number,\n"
            "  ) {}\n"
            "\n"
            "  run(): void {}\n"
            "}\n"
        )
        self.assertEqual([name for _, name in declarations("a.ts", src)[1]], ["A", "run"])

    def test_a_re_export_is_not_a_declaration(self):
        # packages/domain/src/{availability,registry,schedule}/index.ts each write this, and
        # the generator star let `type` step over the `*` and take `from` as the name.
        src = "export type * from './snapshot.js';\nexport * from './other.js';\nexport const after = 1;\n"
        self.assertEqual([name for _, name in declarations("a.ts", src)[1]], ["after"])

    def test_a_generator_function_still_yields_its_name(self):
        src = (
            "export function* gen() {}\n"
            "export function *gen2() {}\n"
            "export async function* gen3() {}\n"
        )
        self.assertEqual([name for _, name in declarations("a.ts", src)[1]], ["gen", "gen2", "gen3"])

    def test_a_construct_signature_is_not_a_member_but_a_property_named_new_is(self):
        src = (
            "export interface F {\n"
            "  new (x: number): F;\n"
            "  new<T>(x: T): F;\n"
            "  new: () => F;\n"
            "  make(): F;\n"
            "}\n"
        )
        self.assertEqual([name for _, name in declarations("a.ts", src)[1]], ["F", "new", "make"])

    def test_an_enum_entry_is_not_a_declaration(self):
        # Kotlin leaves enum entries out, so TypeScript does too, or the same diff would
        # be scored against two different questions.
        src = "export enum Verdict {\n  Ok = 'ok',\n  Fail = 'fail',\n}\n"
        self.assertEqual([name for _, name in declarations("a.ts", src)[1]], ["Verdict"])

    def test_module_exports_is_a_property_access_not_an_ambient_module(self):
        # `module.exports = {…}` is how a `.cjs` config file opens, now that the JavaScript
        # family shares this reader; the bare `module` keyword also opens a TypeScript ambient
        # module, and without the dot check every property in the object literal read as one
        # of its members.
        src = "module.exports = {\n  parser: 'x',\n  rules: { semi: 'error' },\n};\n"
        self.assertEqual(declarations("a.cjs", src)[1], [])

    def test_a_dotted_ambient_module_still_declares_its_head(self):
        src = "export module Foo.Bar {\n  export const x = 1;\n}\n"
        self.assertEqual([name for _, name in declarations("a.ts", src)[1]], ["Foo", "x"])


class RegexLiteralSpans(unittest.TestCase):
    """Defect 2: an unbalanced bracket inside a regex literal ran the span to the file's end."""

    BASE = (
        "const LITERAL = /#[0-9a-f]{3,8}\\b|oklch\\(|rgb\\(|hsl\\(/i;\n"
        "\n"
        "export function later(): number {\n"
        "  return 2;\n"
        "}\n"
    )

    def test_a_regex_literal_does_not_stretch_a_declaration_over_the_rest_of_the_file(self):
        got = changes_of({"a.ts": self.BASE}, {"a.ts": self.BASE.replace("return 2;", "return 3;")})
        self.assertEqual(got["symbols"], {"a.ts": ["later"]})

    def test_the_span_of_the_declaration_holding_the_regex_ends_on_its_own_line(self):
        lines, _ = declarations("a.ts", self.BASE)
        self.assertEqual(declaration_end(lines, 1), 1)

    def test_a_regex_in_return_position_is_recognised(self):
        # apps/api/test/systemPool.spec.ts:53 — `return /\.asSystem\(/.test(code);`.
        src = "function has(code: string) {\n  return /\\.asSystem\\(/.test(code);\n}\n"
        self.assertEqual(declaration_end(declarations("a.ts", src)[0], 1), 3)

    def test_a_division_is_not_read_as_a_regex(self):
        # Blanking `/ b /` away would swallow the `)` of a real call and unbalance the span.
        src = "const r = ratio(a) / b / c;\nexport const after = 1;\n"
        lines, decls = declarations("a.ts", src)
        self.assertEqual(declaration_end(lines, 1), 1)
        self.assertEqual([name for _, name in decls], ["r", "after"])


class KotlinDeclarations(unittest.TestCase):
    """Defect 3: the TypeScript reader was pointed at Kotlin and saw almost nothing."""

    def test_the_top_level_forms_this_corpus_writes(self):
        base = (
            "package p\n"
            "\n"
            "enum class Verdict { OK, FAIL }\n"
            "\n"
            "private fun helper(): Int = 1\n"
            "\n"
            "public data class Row(val id: String)\n"
            "\n"
            "public sealed interface Rule\n"
            "\n"
            "public typealias Ids = List<String>\n"
            "\n"
            "internal fun Row.label(): String = id\n"
        )
        edit = (
            base.replace("OK, FAIL", "OK, FAIL, SKIP")
            .replace("helper(): Int = 1", "helper(): Int = 2")
            .replace("Row(val id: String)", "Row(val id: String, val n: Int)")
            .replace("interface Rule\n", "interface Rule : Comparable<Rule>\n")
            .replace("Ids = List", "Ids = Set")
            .replace("label(): String = id", "label(): String = id + n")
        )
        got = changes_of({"a.kt": base}, {"a.kt": edit})
        self.assertEqual(got["symbols"], {"a.kt": ["Ids", "Row", "Rule", "Verdict", "helper", "label"]})

    def test_a_class_member_counts_and_a_local_inside_a_function_does_not(self):
        base = (
            "package p\n"
            "\n"
            "internal class Registry {\n"
            "    public val entries: Int = 0\n"
            "\n"
            "    private fun count(): Int {\n"
            "        val local = 1\n"
            "        return local\n"
            "    }\n"
            "\n"
            "    private companion object {\n"
            "        const val LIMIT: Int = 9\n"
            "    }\n"
            "}\n"
        )
        edit = (
            base.replace("entries: Int = 0", "entries: Int = 1")
            .replace("val local = 1", "val local = 2")
            .replace("LIMIT: Int = 9", "LIMIT: Int = 8")
        )
        got = changes_of({"a.kt": base}, {"a.kt": edit})
        self.assertEqual(got["symbols"], {"a.kt": ["LIMIT", "Registry", "count", "entries"]})

    def test_a_constructor_parameter_is_the_type_and_not_a_declaration_of_its_own(self):
        base = (
            "package p\n"
            "\n"
            "public data class Receipt(\n"
            "    val lines: Int,\n"
            "    val total: Int,\n"
            ") {\n"
            "    public fun doubled(): Int = total * 2\n"
            "}\n"
        )
        got = changes_of({"a.kt": base}, {"a.kt": base.replace("total * 2", "total * 3")})
        self.assertEqual(got["symbols"], {"a.kt": ["Receipt", "doubled"]})

    def test_a_raw_string_holding_braces_and_a_comment_opener_is_blanked(self):
        # shared/core/src/jvmTest/.../arch/ModuleBoundaryTest.kt:183 is exactly this.
        src = (
            "package p\n"
            "\n"
            'private val BLOCK = Regex("""/\\*[\\s\\S]*?\\*/""")\n'
            "\n"
            "private fun after(): Int = 1\n"
        )
        lines, decls = declarations("a.kt", src)
        self.assertEqual([name for _, name in decls], ["BLOCK", "after"])
        self.assertEqual(declaration_end(lines, 3), 3)

    def test_a_block_comment_nests(self):
        src = "/* outer /* inner */ still a comment\nclass Ghost\n*/\npublic fun real(): Int = 1\n"
        self.assertEqual([name for _, name in declarations("a.kt", src)[1]], ["real"])

    def test_a_backtick_name_carrying_an_apostrophe_is_read_whole(self):
        src = (
            "internal class T {\n"
            "    @Test\n"
            "    fun `a namespace does not see another namespace's values`() {\n"
            "        val x = 1\n"
            "    }\n"
            "}\n"
        )
        self.assertEqual(
            [name for _, name in declarations("a.kt", src)[1]],
            ["T", "a namespace does not see another namespace's values"],
        )

    def test_a_nested_type_parameter_does_not_hide_the_declaration(self):
        # mobile/shared/domain/src/jvmTest/.../conformance/FixtureLoader.kt:453.
        src = (
            "package p\n"
            "\n"
            "internal inline fun <reified E : Enum<E>> enum(value: String?): E? = null\n"
        )
        self.assertEqual([name for _, name in declarations("a.kt", src)[1]], ["enum"])

    def test_a_nested_generic_receiver_does_not_become_the_name(self):
        src = "package p\n\npublic fun Map<String, List<Int>>.flatten(): List<Int> = emptyList()\n"
        self.assertEqual([name for _, name in declarations("a.kt", src)[1]], ["flatten"])

    def test_a_type_with_no_body_does_not_claim_the_next_block(self):
        # `pending` used to survive a bodyless declaration, so the next brace became that
        # type's body and the locals inside it were reported as its members.
        src = (
            "package p\n"
            "\n"
            "internal class Outer {\n"
            "    class Empty\n"
            "    init {\n"
            "        val secret = 1\n"
            "    }\n"
            "}\n"
        )
        self.assertEqual([name for _, name in declarations("a.kt", src)[1]], ["Outer", "Empty"])

    def test_a_header_that_carries_on_to_the_next_line_keeps_its_body(self):
        # packages/contracts/src/generated/business/client/types.gen.ts:17 puts the brace on
        # the `extends` line, and its members are real.
        src = (
            "export interface Config<T extends ClientOptions = ClientOptions>\n"
            "  extends Omit<RequestInit, 'body'>, CoreConfig {\n"
            "  baseUrl?: string;\n"
            "}\n"
        )
        self.assertEqual([name for _, name in declarations("a.ts", src)[1]], ["Config", "baseUrl"])

    def test_a_brace_inside_a_kotlin_string_does_not_move_a_span(self):
        src = 'private val s = "a{b"\nprivate fun after(): Int = 1\n'
        lines, decls = declarations("a.kt", src)
        self.assertEqual(declaration_end(lines, 1), 1)
        self.assertEqual([name for _, name in decls], ["s", "after"])


class FileDenominator(unittest.TestCase):
    """Defect 4: a code file the reader could not read was in neither axis."""

    def test_a_code_file_with_no_readable_declaration_is_still_counted(self):
        got = changes_of(
            {"src/barrel.ts": "export * from './a.js';\n", "docs/notes.md": "# notes\n"},
            {"src/barrel.ts": "export * from './b.js';\n", "docs/notes.md": "# changed\n"},
        )
        self.assertEqual(got["code_files"], ["src/barrel.ts"])
        self.assertEqual(got["symbols"], {})
        self.assertIn("docs/notes.md", got["files"])

    def test_a_deleted_file_is_in_no_axis(self):
        # `git diff -U0` gives a deletion `@@ -1,1 +0,0 @@` — no new-side lines — so there is
        # nothing here to read a declaration or a span from, and the docstring says so.
        got = changes_of(
            {"gone.ts": "export const x = 1;\n", "kept.ts": "export const y = 1;\n"},
            {"gone.ts": None, "kept.ts": "export const y = 2;\n"},
        )
        self.assertEqual(got["files"], ["kept.ts"])
        self.assertEqual(got["code_files"], ["kept.ts"])
        self.assertEqual(got["symbols"], {"kept.ts": ["y"]})

    def test_the_two_axes_stay_coherent(self):
        base = {"a.ts": "export const x = 1;\n", "b.kt": "package p\n", "c.md": "# c\n"}
        edit = {"a.ts": "export const x = 2;\n", "b.kt": "package q\n", "c.md": "# d\n"}
        got = changes_of(base, edit)
        self.assertEqual(got["code_files"], ["a.ts", "b.kt"])
        self.assertEqual(got["symbols"], {"a.ts": ["x"]})
        self.assertEqual(got["files"], ["a.ts", "b.kt", "c.md"])


class Registries(unittest.TestCase):
    """A language joins the truth by its extension, through three tables and nothing else."""

    def test_each_reader_is_found_by_the_extension_it_reads(self):
        self.assertIs(T.DECLARATIONS[".ts"], T.typescript_declarations)
        self.assertIs(T.DECLARATIONS[".tsx"], T.typescript_declarations)
        self.assertIs(T.DECLARATIONS[".kt"], T.kotlin_declarations)
        self.assertIs(T.BLANKERS[".kt"], T.blank_kotlin)
        self.assertEqual(list(T.DI_READERS), [*T.JS_FAMILY, ".cs", ".razor"])
        self.assertEqual(T.CALL_READERS, {})
        self.assertEqual(
            T.code_globs(),
            ("-g", "*.ts", "-g", "*.tsx", "-g", "*.js", "-g", "*.jsx", "-g", "*.mjs", "-g", "*.cjs", "-g", "*.kt", "-g", "*.cs", "-g", "*.razor", "-g", "*.cshtml"),
        )

    def test_a_registered_reader_is_the_one_declarations_uses(self):
        seen = []

        def reader(blanked):
            seen.append(blanked)
            return [(1, "Fake")]

        T.DECLARATIONS[".fk"] = reader
        T.BLANKERS[".fk"] = str.upper
        try:
            lines, found = T.declarations("a.fk", "abc\n")
        finally:
            del T.DECLARATIONS[".fk"], T.BLANKERS[".fk"]
        self.assertEqual((lines, found, seen), (["ABC", ""], [(1, "Fake")], ["ABC\n"]))

    def test_a_file_no_reader_reads_is_in_the_diff_and_in_neither_axis(self):
        got = changes_of(
            {"a.ts": "export function f() {\n  return 1;\n}\n", "b.zig": "class B {}\n"},
            {"a.ts": "export function f() {\n  return 2;\n}\n", "b.zig": "class B { int x; }\n"},
        )
        self.assertEqual(got["files"], ["a.ts", "b.zig"])
        self.assertEqual(got["code_files"], ["a.ts"])
        self.assertEqual(got["symbols"], {"a.ts": ["f"]})


class TypeScriptTruthStandsAlone(unittest.TestCase):
    """The TypeScript trace truth is the same with other languages' files beside it."""

    TS = {
        "apps/auth.ts": "export class AuthController {\n  constructor(private auth: AuthService) {}\n  go() { this.auth.login(); }\n}\n",
        "apps/service.ts": "export class AuthService {\n  login() { return 1; }\n}\n",
    }
    OTHER = {
        "apps/Sync.kt": "package p\n\nclass SyncEngine(private val wipe: RemoteWipeHandler) {\n    fun run() { wipe.execute() }\n}\n",
        "apps/Order.zig": "namespace Shop;\npublic class Order { private readonly IPay _pay; public void Go() { this._pay.Charge(); } }\n",
    }

    @staticmethod
    def graph(files):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d)
            for rel, body in files.items():
                (root / rel).parent.mkdir(parents=True, exist_ok=True)
                (root / rel).write_text(body, encoding="utf8")
            return T.di_call_graph(root, ["apps"])

    def test_other_languages_beside_it_change_nothing(self):
        alone = self.graph(self.TS)
        self.assertEqual(alone["edges"], {"AuthController": ["AuthService.login"]})
        self.assertEqual(self.graph({**self.TS, **self.OTHER}), alone)


class CallReaders(unittest.TestCase):
    """A chain no field-and-call pattern can say joins the trace truth as name pairs, by extension."""

    def setUp(self):
        self.addCleanup(T.CALL_READERS.pop, ".fk", None)
        self.addCleanup(T.DI_READERS.pop, ".fk", None)

    def test_a_call_reader_s_pairs_are_the_edges_the_trace_truth_walks(self):
        T.CALL_READERS[".fk"] = lambda src: [tuple(line.split(" -> ")) for line in src.splitlines() if " -> " in line]
        graph = TypeScriptTruthStandsAlone.graph({"apps/a.fk": "Report -> Query.run\nQuery -> Table.read\n"})
        self.assertEqual(graph["edges"], {"Report": ["Query.run"], "Query": ["Table.read"]})
        self.assertEqual(T.shortest_path(graph, "Report", "Table"), ["Report", "Query.run", "Table.read"])

    def test_a_file_a_call_reader_reads_is_not_also_read_by_a_di_reader(self):
        T.CALL_READERS[".fk"] = lambda src: [("Only", "Pair.go")]
        T.DI_READERS[".fk"] = T.TYPESCRIPT_DI
        graph = TypeScriptTruthStandsAlone.graph({"apps/a.fk": TypeScriptTruthStandsAlone.TS["apps/auth.ts"]})
        self.assertEqual(graph["edges"], {"Only": ["Pair.go"]})


class CaseExtensions(unittest.TestCase):
    """A case's `exts` counts only its own language's files among those naming the target."""

    FILES = {
        "apps/pay.ts": "export class Pay {}\n",
        "apps/use.ts": "import { Pay } from './pay';\nexport const p = new Pay();\n",
        "apps/view.tsx": "import { Pay } from './pay';\nexport function View() { return new Pay(); }\n",
    }

    def refs(self, extra):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d)
            for rel, body in self.FILES.items():
                (root / rel).parent.mkdir(parents=True, exist_ok=True)
                (root / rel).write_text(body, encoding="utf8")
            case = {"kind": "impact", "target": "Pay", "file": "apps/pay.ts", **extra}
            return T.build(root, [], [case])["impact"]["Pay"]["refs"]

    def test_without_exts_every_code_file_naming_the_target_counts(self):
        self.assertEqual(self.refs({}), ["apps/use.ts", "apps/view.tsx"])

    def test_exts_keeps_only_the_files_of_those_extensions(self):
        self.assertEqual(self.refs({"exts": [".tsx"]}), ["apps/view.tsx"])


CHECKOUT_RAZOR = (
    '@page "/checkout"\n@inherits PageBase\n@inject Shop.Payments.IPaymentGateway Payments\n\n'
    '<h3>Checkout</h3>\n<OrderLine Caption="one" />\n\n@code {\n    private int total;\n'
    "    void Pay()\n    {\n        Payments.Charge(total);\n        Confirm();\n    }\n}\n"
)
CHECKOUT_BEHIND = (
    "namespace Shop.Web.Pages;\npublic partial class Checkout\n{\n"
    "    [Inject] private IOrderStore Keeper { get; set; }\n    void Confirm() { Keeper.Save(); }\n}\n"
)
ORDERING = """namespace Shop.Orders;
public class Checkout(IPaymentGateway gateway)
{
    public void Pay() => gateway.Charge(1);
}
public class OrderService
{
    private readonly Shop.Payments.IPaymentGateway _gateway;
    private ILogger<OrderService> Log { get; }
    public OrderService(IOrderStore store) { store.Save(); }
    public void Place(int id)
    {
        this._gateway.Charge(id);
        _gateway?.Refund(id);
        Log.Info(id);
    }
}
public record struct Point(int X, int Y);
"""
ORDERS = """namespace Shop.Orders;

// Place is documented here, not declared
[Serializable]
public partial class OrderService(IPaymentGateway gateway, int retries) : ServiceBase
{
    private readonly IOrderStore _store;
    public int Count { get; private set; }
    public event EventHandler Placed;

    public OrderService() : this(null, 0) { }

    public Order Place(int id)
    {
        var receipt = new Receipt();
        _store.Save(id);
        return null;
    }

    public class Line { void Touch() { } }
}

public record Order(int Id, string Name);
public enum Status { Open, Closed }
public delegate void PlacedHandler(int x);
interface IOrderStore
{
    void Save(int id);
}
"""


def di_of(files: dict[str, str]) -> dict:
    """`di_call_graph` over a throwaway tree, so ripgrep's file listing is part of what is tested."""
    with tempfile.TemporaryDirectory() as tmp:
        repo = Path(tmp)
        for rel, body in files.items():
            (repo / rel).parent.mkdir(parents=True, exist_ok=True)
            (repo / rel).write_text(body, encoding="utf8")
        return T.di_call_graph(repo, ["apps", "libs-dotnet"])


class DotnetReaders(unittest.TestCase):
    """C# and Razor, read the way the store is asked to read them."""

    def test_csharp_blanking_empties_every_string_form_and_keeps_the_line_count(self):
        src = 'var a = "Order"; // IPaymentGateway\nvar b = @"x\n""Order""";\nvar c = $"{total} Order";\nvar d = """\nOrder\n""";\nchar e = \'O\'; /* Order\n*/ int f;\n'
        out = T.blank_csharp(src)
        self.assertNotIn("Order", out)
        self.assertNotIn("IPaymentGateway", out)
        self.assertIn("int f;", out)
        self.assertEqual(len(out.split("\n")), len(src.split("\n")))

    def test_types_members_and_primary_constructor_parameters_are_declarations_and_locals_are_not(self):
        self.assertEqual(T.csharp_declarations(T.blank_csharp(ORDERS)), [
            (5, "OrderService"), (5, "gateway"), (5, "retries"), (7, "_store"), (8, "Count"), (9, "Placed"),
            (11, "OrderService"), (13, "Place"), (20, "Line"), (23, "Order"), (23, "Id"), (23, "Name"),
            (24, "Status"), (25, "PlacedHandler"), (26, "IOrderStore"), (28, "Save"),
        ])

    def test_a_razor_file_keeps_its_rows_its_type_directives_its_tags_and_its_code_block(self):
        out = T.blank_razor(CHECKOUT_RAZOR)
        lines = out.split("\n")
        self.assertEqual(len(lines), len(CHECKOUT_RAZOR.split("\n")))
        self.assertEqual(lines[1:3], ["@inherits PageBase", "@inject Shop.Payments.IPaymentGateway Payments"])
        self.assertEqual(lines[4:8], ["", "OrderLine", "", "class __RazorBlock {"])
        self.assertEqual(T.razor_declarations(out), [(3, "Payments"), (9, "total"), (10, "Pay")])

    def test_a_view_declares_nothing_and_keeps_its_model_line(self):
        view = "@page\n@model Shop.Web.Pages.IndexModel\n@using Shop.Payments\n@inject IPaymentGateway Payments\n<h1>Index</h1>\n"
        self.assertEqual(T.blank_razor(view), "\n@model Shop.Web.Pages.IndexModel\n\n@inject IPaymentGateway Payments\n\n")
        self.assertEqual(T.declarations("Pages/Index.cshtml", view)[1], [])

    def test_fields_properties_and_constructor_parameters_carry_calls(self):
        graph = di_of({"libs-dotnet/Orders/Ordering.cs": ORDERING})
        self.assertEqual(graph["edges"], {
            "Checkout": ["IPaymentGateway.Charge"],
            "OrderService": ["ILogger.Info", "IOrderStore.Save", "IPaymentGateway.Charge", "IPaymentGateway.Refund"],
        })
        self.assertEqual(sorted(graph["declared"]), ["Checkout", "OrderService", "Point"])

    def test_a_component_is_one_class_named_by_its_file_and_its_code_behind_joins_it(self):
        graph = di_of({"apps/web/Pages/Checkout.razor": CHECKOUT_RAZOR, "apps/web/Pages/Checkout.razor.cs": CHECKOUT_BEHIND})
        self.assertEqual(graph["edges"], {"Checkout": ["IOrderStore.Save", "IPaymentGateway.Charge"]})

    def test_each_dotnet_reader_is_registered_by_its_extension(self):
        self.assertIs(T.DECLARATIONS[".cs"], T.csharp_declarations)
        self.assertIs(T.DECLARATIONS[".razor"], T.razor_declarations)
        self.assertIs(T.DECLARATIONS[".cshtml"], T.view_declarations)
        self.assertIs(T.BLANKERS[".cs"], T.blank_csharp)
        self.assertIs(T.BLANKERS[".razor"], T.blank_razor)
        self.assertIs(T.BLANKERS[".cshtml"], T.blank_razor)
        self.assertIs(T.DI_READERS[".cs"], T.CSHARP_DI)
        self.assertIsInstance(T.DI_READERS[".razor"], T.ComponentDiReader)
        self.assertNotIn(".cshtml", T.DI_READERS)

    def test_a_bom_does_not_hide_the_first_line(self):
        src = "﻿public class A\n{\n    public void Run() {}\n}\n"
        self.assertEqual(T.csharp_declarations(T.blank_csharp(src)), [(1, "A"), (3, "Run")])
        self.assertEqual(T.blank_razor("﻿@inject Shop.IPay Pay\n<h3/>\n").split("\n")[0], "@inject Shop.IPay Pay")

    def test_a_wrapped_base_list_in_a_block_namespace_does_not_read_as_a_member(self):
        # I1: `IFoo`, continuing `Base,` onto its own line, must not be read as a member of A,
        # and A's own members (`x`, `M`) must survive the header that wraps around them.
        src = (
            "namespace N\n{\n    public class A : Base,\n        IFoo\n    {\n"
            "        int x;\n        void M() { }\n    }\n}\n"
        )
        self.assertEqual(T.csharp_declarations(T.blank_csharp(src)), [(3, "A"), (6, "x"), (7, "M")])

    def test_a_wrapped_initialiser_continuation_declares_nothing(self):
        # I2: the RHS of a field or expression-bodied member that wraps declares nothing of its
        # own — `Foo` and `Build` are expressions here, not members.
        src = (
            "public class A\n{\n    private static readonly Foo Default =\n        new Foo();\n"
            "    public Foo Inst =>\n        Build(1);\n}\n"
        )
        self.assertEqual(T.csharp_declarations(T.blank_csharp(src)), [(1, "A"), (3, "Default"), (5, "Inst")])

    def test_cs_call_spans_a_newline_before_the_dot(self):
        # I4: a fluent chain wraps its `.` the way TypeScript's does; the gap must span it too.
        self.assertEqual(T.CS_CALL.findall("_gateway\n    .Charge(id)"), [("_gateway", "Charge")])

    def test_layout_attribute_and_typeparam_constraint_are_kept_but_the_rest_of_markup_is_not(self):
        # I3: `@layout`/`@attribute` join the kept directives whole; a `@typeparam ... where`
        # keeps only its constraint type, never the parameter's own name; a markup expression, an
        # `@{ }` block and a tag's own generic argument stay blanked, as they always have.
        view = (
            "@layout MainLayout\n@attribute [Authorize]\n@typeparam TItem where TItem : IEntity\n"
            '@typeparam TOther\n@Formatter.Money(total)\n@{ var h = new PriceHelper(); }\n'
            '<Grid TItem="Order" />\n'
        )
        lines = T.blank_razor(view).split("\n")
        self.assertEqual(lines[:7], [
            "@layout MainLayout", "@attribute [Authorize]", "IEntity", "", "", "", "Grid",
        ])

    def test_a_razor_comment_s_component_tag_is_not_a_reference(self):
        # M7: a commented-out tag, Razor or HTML style, must not count as a reference.
        src = "@* <Badge /> *@\n<!-- <Banner /> -->\n<Real />\n"
        self.assertEqual(T.blank_razor(src).split("\n"), ["", "", "Real", ""])

    def test_a_double_quote_char_literal_inside_an_interpolation_hole_does_not_leak(self):
        # M5: `'"'` inside a hole must not be read as the hole's own closing quote.
        src = "var a = $\"{(c == '\"' ? 1 : 2)} Order\";\n"
        self.assertNotIn("Order", T.blank_csharp(src))

    def test_a_region_directive_s_text_is_blanked(self):
        # M6: `#region`/`#endregion` banners name whatever the author likes, never code.
        src = "#region Order stuff\nint x;\n#endregion Order\n"
        out = T.blank_csharp(src)
        self.assertNotIn("Order", out)
        self.assertIn("int x;", out)
        self.assertEqual(len(out.split("\n")), len(src.split("\n")))

    def test_a_nested_generic_field_keeps_its_outermost_type(self):
        # M9: the second match, starting at the generic's own argument, must not overwrite the
        # first, which is always the outermost — and correct — type.
        src = (
            "namespace Shop.Orders;\npublic class Cache\n{\n"
            "    private readonly IDictionary<string, List<int>> _map;\n"
            "    public void Warm() { _map.TryGetValue(1); }\n}\n"
        )
        graph = di_of({"apps/Cache.cs": src})
        self.assertEqual(graph["edges"], {"Cache": ["IDictionary.TryGetValue"]})

    def test_empty_roots_scans_nothing(self):
        # M13: no root the caller offered exists — the tree must be read as empty, not as ripgrep's
        # default of everything.
        with tempfile.TemporaryDirectory() as tmp:
            repo = Path(tmp)
            (repo / "src").mkdir()
            (repo / "src" / "A.ts").write_text("export class A { b: B; go() { this.b.go(); } }\n", encoding="utf8")
            self.assertEqual(T.di_call_graph(repo, []), {"edges": {}, "declared": {}})


if __name__ == "__main__":
    unittest.main()
