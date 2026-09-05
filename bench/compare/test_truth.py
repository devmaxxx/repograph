"""What counts as a reference, on the lines that were miscounted."""

import subprocess
import tempfile
import unittest
from pathlib import Path

from truth import blank_kotlin, blank_typescript, changed_symbols, declaration_end, declarations


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


if __name__ == "__main__":
    unittest.main()
