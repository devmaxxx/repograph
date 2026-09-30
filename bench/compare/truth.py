"""Ground truth for the three-graph comparison, derived from the repository itself.

No graph tool is consulted here. Every expectation is read from the source with
ripgrep and a small reader per language, so a tool that disagrees with this file is
wrong about the repository, not about a rival's model.
"""

from __future__ import annotations

import json
import re
import subprocess
from bisect import bisect_right
from collections import defaultdict
from dataclasses import dataclass
from pathlib import Path
from typing import Callable

# The stores of the three tools sit inside the corpus; a hit there is a hit on a
# tool's own index, not on the repository. The rest is the tree 0.5.3's store reads: dot-paths
# but `.git`, less the default `skip`'s bundles.
EXCLUDE = ("--hidden", "-g", "!.git", "-g", "!.repograph/**", "-g", "!graphify-out/**", "-g", "!.gitnexus/**",
           "-g", "!*.min.js", "-g", "!.yarn/**", "-g", "!.pnp.*")

CLASS = re.compile(r"^export (?:abstract )?class (\w+)", re.M)
FIELD = re.compile(r"(?:private|public|protected|readonly)\s+(?:readonly\s+)?(\w+)\s*:\s*(\w+)")
# `this.db.run(` and `this.db\n  .run(` are the same call; tree-sitter sees no
# newline and neither may we, or the truth undercounts what the tools find.
CALL = re.compile(r"this\.(\w+)\s*\.\s*(\w+)\s*\(", re.S)


@dataclass(frozen=True)
class DiReader:
    """One language's reading of calls through injected fields, for the `trace` truth.

    `cls` puts a class name in group 1 at the class's start, and `di_call_graph` cuts the file
    at each such match. Inside a class, `field` puts a field in group 1 and its declared type in
    group 2, and `call` puts the receiver field in group 1 and the method in group 2.
    """

    cls: re.Pattern[str]
    field: re.Pattern[str]
    call: re.Pattern[str]


TYPESCRIPT_DI = DiReader(CLASS, FIELD, CALL)


def rg(repo: Path, args: list[str]) -> list[str]:
    # stdin detached: ripgrep searches stdin instead of the tree when stdin is not a tty,
    # which turned every truth list empty under a heredoc and would do the same in CI.
    out = subprocess.run(["rg", *args, *EXCLUDE], cwd=repo, capture_output=True, text=True,
                         stdin=subprocess.DEVNULL)
    return [line for line in out.stdout.split("\n") if line]


def files_naming(repo: Path, token: str) -> list[str]:
    """Every tracked file that spells `token` as a whole word."""
    return sorted(rg(repo, ["-l", "--word-regexp", "--fixed-strings", token]))


# A `/` opens a regular expression only in an operand position. After an identifier, a `)`
# or a `]` the same character divides, so those must not be listed here. Nor does `}`:
# `<A x={1} />` in a .tsx file would read its self-closing slash as a literal opening, and
# the `} /re/` it would otherwise buy is a statement start, which the empty-prefix case
# already covers.
REGEX_OPENS_AFTER = set("=(,:[!&|?;")
REGEX_OPENS_AFTER_WORD = re.compile(
    r"(?:^|[^\w$.])(?:return|typeof|instanceof|case|in|of|new|delete|void|throw|do|else|yield|await)\s*$"
)


def _regex_end(src: str, start: int) -> int | None:
    """The index of the `/` closing the literal opened at `start`, or None if it does not close."""
    i = start + 1
    in_class = False
    while i < len(src):
        char = src[i]
        if char == "\\":
            i += 2
            continue
        if char == "\n":
            return None
        if char == "[":
            in_class = True
        elif char == "]":
            in_class = False
        elif char == "/" and not in_class:
            return i
        i += 1
    return None


def _blank_source(src: str, *, kotlin: bool, nested_comments: bool | None = None) -> str:
    r"""Comments and text blanked out of `src`, line for line, in one left-to-right pass.

    Prose is not a reference and neither is a string. This corpus writes long docblocks that
    name the symbols they discuss, so a plain grep counts a paragraph about
    `TenantContextInterceptor` as a file that depends on it; it also names symbols in text,
    where `'PinoLogger:OutboxPublisher'` is a logger's name and an error message can mention an
    interceptor. Both counted on 2026-09-03 and put two impact targets one file short of 1.0
    for a dependency that did not exist.

    One left-to-right pass rather than one substitution per construct, because the same output
    also feeds `changed_symbols`, which needs two things a boolean search did not. Line numbers
    must survive exactly, since hunk ranges are mapped onto declaration spans and a lost line
    shifts every declaration under it. And brackets must survive exactly, since
    `declaration_end` balances them.

    Substituting comments before strings gets both wrong on this corpus: `'apps/*'` in
    packages/ui/test/boundary.test.ts opens a block comment that runs until the next `*/`
    seventy lines below, `'//'` in packages/config/test/config-boundary.test.ts deletes the rest
    of its line, and a `//` after a ternary's colon survives because the pattern guarding
    against `http://` cannot tell the two colons apart. The first two take the closing bracket
    of a real array with them and ran a declaration to the end of the file. A scanner cannot
    make any of those mistakes: whichever construct opens first wins, which is what the
    language does.

    The two dialects differ in four ways, all of them load-bearing here. Kotlin nests block
    comments, so `/* a /* b */ c */` closes once. Kotlin has raw `\"\"\"…\"\"\"` strings that hold
    anything at all, including the `/\*` that ModuleBoundaryTest.kt puts in a `Regex`. A
    backtick opens a template literal in TypeScript and quotes an identifier in Kotlin, where
    this corpus names 358 tests that way and one of them carries an apostrophe that would
    otherwise open a character literal. And only TypeScript has a regex literal, which is the
    one place a bracket is a character rather than a nesting.

    Java takes Kotlin's dialect with one change, that a block comment does not nest: a text block
    is a raw string, a backtick never appears in Java code, and Java has no regex literal.

    What this still loses is a `${…}` interpolation, blanked with the string around it.
    """
    nests = kotlin if nested_comments is None else nested_comments
    out: list[str] = []
    line = ""  # the code emitted since the last newline, for the operand-position test
    i, size = 0, len(src)

    def emit(piece: str) -> None:
        nonlocal line
        out.append(piece)
        line = (line + piece).rsplit("\n", 1)[-1]

    while i < size:
        char = src[i]
        pair = src[i : i + 2]
        if pair == "//":
            end = src.find("\n", i)
            i = size if end < 0 else end
        elif pair == "/*":
            depth, end = 1, i + 2
            while end < size and depth:
                if nests and src[end : end + 2] == "/*":
                    depth += 1
                    end += 2
                elif src[end : end + 2] == "*/":
                    depth -= 1
                    end += 2
                else:
                    end += 1
            emit("\n" * src.count("\n", i, end))
            i = end
        elif kotlin and src[i : i + 3] == '"""':
            end = src.find('"""', i + 3)
            end = size if end < 0 else end + 3
            emit('"""' + "\n" * src.count("\n", i, end) + '"""')
            i = end
        elif kotlin and char == "`":
            end = src.find("`", i + 1)
            end = size if end < 0 else end + 1
            emit(src[i:end])
            i = end
        elif char in "\"'" or (not kotlin and char == "`"):
            end = i + 1
            while end < size and src[end] != char:
                if src[end] == "\\":
                    end += 2
                    continue
                # Only a template literal may hold a bare newline; an unterminated quote is a
                # scan that has gone wrong, and it must not eat the rest of the file.
                if src[end] == "\n" and char != "`":
                    break
                end += 1
            if end < size and src[end] == char:
                emit(char + "\n" * src.count("\n", i, end) + char)
                i = end + 1
            else:
                emit(char)
                i += 1
        elif not kotlin and char == "/":
            stripped = line.rstrip()
            opens = (
                not stripped
                or stripped[-1] in REGEX_OPENS_AFTER
                or stripped.endswith("=>")
                or bool(REGEX_OPENS_AFTER_WORD.search(line))
            )
            end = _regex_end(src, i) if opens else None
            if end is None:
                emit(char)
                i += 1
            else:
                emit("/ /")
                i = end + 1
        else:
            emit(char)
            i += 1
    return "".join(out)


def blank_typescript(src: str) -> str:
    return _blank_source(src, kotlin=False)


def blank_kotlin(src: str) -> str:
    return _blank_source(src, kotlin=True)


def blank_java(src: str) -> str:
    return _blank_source(src, kotlin=True, nested_comments=False)


def blanked_source(rel: str, src: str) -> str:
    """`src` with its comments and text blanked, in the dialect `rel`'s extension implies."""
    # TypeScript's scanner stays the fallback: it is what every extension but `.kt` was read with.
    return BLANKERS.get(Path(rel).suffix, blank_typescript)(src)


# Up to three levels of nesting, because `fun <reified E : Enum<E>> enum(…)` in
# FixtureLoader.kt has two and a receiver such as `Map<String, List<Int>>` has two more.
GENERIC = r"<(?:[^<>\n]|<(?:[^<>\n]|<[^<>\n]*>)*>)*>"

KOTLIN_MODIFIER = (
    "public|private|internal|protected|open|final|abstract|sealed|data|enum|annotation|"
    "value|inner|expect|actual|override|lateinit|const|external|infix|inline|operator|"
    "suspend|tailrec|companion|reified"
)
KOTLIN_NAME = r"`[^`\n]+`|\w+"
# `fun interface` before `fun`, or the name of a `fun interface Renderer` reads as `interface`.
# The receiver of an extension is consumed and dropped: `fun Row.label()` declares `label`.
# The name is optional so that an unnamed `companion object` still opens a body of members.
KOTLIN_DECL = re.compile(
    rf"^[ \t]*(?:@[\w.]+(?:\([^()\n]*\))?[ \t]*)*"
    rf"(?:(?:{KOTLIN_MODIFIER})[ \t]+)*"
    rf"(?P<kw>fun[ \t]+interface|class|interface|object|fun|val|var|typealias)\b"
    rf"(?:[ \t]*{GENERIC})?"
    rf"(?:[ \t]+(?:\w+(?:{GENERIC})?(?:\.\w+(?:{GENERIC})?)*\.)?(?P<name>{KOTLIN_NAME}))?"
)
# The bodies these open hold declarations; every other brace opens a block that holds statements.
KOTLIN_TYPE_KEYWORDS = {"class", "interface", "object", "fun interface"}

TS_MODIFIER = "export|default|declare|abstract|async|static|public|private|protected|readonly|override|accessor"
TS_DECL = re.compile(
    rf"^[ \t]*(?:@[\w.]+(?:\([^()\n]*\))?[ \t]*)*"
    rf"(?:(?:{TS_MODIFIER})[ \t]+)*"
    rf"(?P<kw>class|function|interface|enum|namespace|module|const|let|var|type)\b"
    # `module.exports = {…}`, the CommonJS files now in this same reader write on their first
    # line, is a property access, not the ambient `module Foo {}` this keyword also spells.
    # Only a dot glues the two, so refusing one right after the keyword tells them apart.
    rf"(?!\.)"
    # The generator star belongs to `function` alone. Letting any keyword step over it read
    # `export type * from './snapshot.js'` as a declaration named `from`, three times in
    # packages/domain — a re-export names nothing, and a star after any other keyword is not
    # a declaration either.
    rf"(?:(?<=function)[ \t]*\*)?"
    rf"(?:[ \t]*(?P<name>[\w$]+))?"
)
# A class or interface member carries no keyword at all — it is a name followed by a call
# signature, a type annotation or an initialiser. Only reachable inside a type body, so a
# `for (` or an `if (` in a function body can never be read as one.
TS_MEMBER = re.compile(
    rf"^[ \t]*(?:@[\w.]+(?:\([^()\n]*\))?[ \t]*)*"
    rf"(?:(?:public|private|protected|readonly|static|abstract|override|async|declare|accessor)[ \t]+)*"
    rf"(?:(?:get|set)[ \t]+)?"
    rf"\*?[ \t]*"
    # `constructor(…)` and `new (x): T` introduce a signature, not a name a graph reports.
    # Only when a call follows: `new: () => T` really is a property named `new`.
    rf"(?!(?:constructor|new)[ \t]*[(<])"
    rf"(?P<name>[\w$]+)"
    rf"[ \t]*[?!]?[ \t]*(?:{GENERIC}[ \t]*)?[(:=;]"
)
TS_TYPE_KEYWORDS = {"class", "interface", "namespace", "module"}

# A declaration's header can outlive its line — `class Receipt(\n…\n) {` and
# `interface Config<T>\n  extends Omit<…> {` both put the body brace lower down — but it must
# not outlive the declaration, or a bodyless `class Empty` hands its type body to the next
# unrelated brace and the locals inside that block are read as members. A header carries on
# when the line before it ends open or the line after it starts as a continuation; anything
# else ends it.
HEADER_TAIL = set(",([:<=&|")
HEADER_HEAD = set("{,):>&|")
HEADER_WORD = r"extends|implements|where|by"
HEADER_TAIL_WORD = re.compile(rf"(?:^|[^\w])(?:{HEADER_WORD})$")
HEADER_HEAD_WORD = re.compile(rf"^(?:{HEADER_WORD})\b")


def _scoped_declarations(
    blanked: str,
    *,
    decl: re.Pattern[str],
    member: re.Pattern[str] | None,
    type_keywords: set[str],
) -> list[tuple[int, str]]:
    """(line, name) for every declaration in already-blanked source, by brace scope.

    Brace scope, not indentation. `val x = 1` at the head of a function body is a local and
    `val x = 1` in a class body is a property; `const x = 1` and `if (x) {` sit at the same
    column in TypeScript. Only the block they are in tells them apart, so the stack records,
    for each open brace, whether what it opened holds declarations or statements.

    `decl` is the keyword form and is read at any declaring scope. `member` is the keyless
    form — a TypeScript method or property — and is read only directly inside a type body,
    which is what keeps a `for (` out of the truth. Kotlin passes none, because there a member
    is spelled with the same `fun` or `val` as a top-level declaration.

    A constructor parameter is left out in both dialects: the hunk that touches a type's
    header touches the type, which is already named. So is an enum entry, and so are the
    TypeScript `constructor` and construct signatures, which `member` declines to match.
    """
    found: list[tuple[int, str]] = []
    holds_declarations: list[bool] = []
    parens = 0
    pending: bool | None = None
    open_header = False
    for index, line in enumerate(blanked.split("\n")):
        head = line.strip()
        if pending is not None and parens == 0 and not open_header:
            carries_on = bool(head) and (head[0] in HEADER_HEAD or HEADER_HEAD_WORD.search(head))
            if not carries_on:
                pending = None
        inside_type = bool(holds_declarations) and holds_declarations[-1]
        if parens == 0 and (not holds_declarations or inside_type):
            match = member.match(line) if (member and inside_type) else None
            if match:
                pending = False
            else:
                match = decl.match(line)
                if match:
                    pending = " ".join(match.group("kw").split()) in type_keywords
            if match and match.group("name"):
                found.append((index + 1, match.group("name").strip("`")))
        for char in line:
            if char == "{":
                # Only the first brace after a declaration is that declaration's body; a
                # `class Foo(\n…\n) {` header puts it several lines below the name.
                holds_declarations.append(bool(pending))
                pending = None
            elif char == "}" and holds_declarations:
                holds_declarations.pop()
            elif char == "(":
                parens += 1
            elif char == ")" and parens:
                parens -= 1
        tail = line.rstrip()
        open_header = bool(tail) and (tail[-1] in HEADER_TAIL or bool(HEADER_TAIL_WORD.search(tail)))
    return found


def kotlin_declarations(blanked: str) -> list[tuple[int, str]]:
    """(line, name) for every Kotlin declaration in already-blanked source.

    A name in backticks is reported without them: a tool that answers with the backticks still
    contains the bare name, and one that answers without them would otherwise be marked wrong.
    """
    return _scoped_declarations(blanked, decl=KOTLIN_DECL, member=None, type_keywords=KOTLIN_TYPE_KEYWORDS)


def typescript_declarations(blanked: str) -> list[tuple[int, str]]:
    """(line, name) for every TypeScript declaration in already-blanked source.

    Class and interface members are read as well as top-level declarations, so that this
    denominator asks a Kotlin file and a TypeScript file the same question. Anchoring at
    column zero instead would ask a much easier one of TypeScript, and a mixed-language score
    would then be two measurements added together.
    """
    return _scoped_declarations(blanked, decl=TS_DECL, member=TS_MEMBER, type_keywords=TS_TYPE_KEYWORDS)


JAVA_MODIFIER = (
    "public|private|protected|abstract|static|final|sealed|non-sealed|strictfp|default|"
    "synchronized|native|transient|volatile"
)
# `@interface` declares an annotation type rather than applying one, so it is not an annotation here.
JAVA_ANNOTATION = re.compile(r"@(?!interface\b)[\w$]+(?:[ \t]*\.[ \t]*[\w$]+)*")
JAVA_DECL = re.compile(
    rf"^[ \t]*(?:(?:{JAVA_MODIFIER})[ \t]+)*"
    rf"(?P<kw>class|interface|enum|record|@interface)\b"
    rf"[ \t]+(?P<name>\w+)"
)
# A member is a type then a name, or — a constructor — a capitalised name straight before its
# parameters. An enum constant looks like the second form, so an ALL-CAPS name is refused there;
# `return x;` and `new X();` look like the first, so their keywords are refused as a type.
JAVA_MEMBER = re.compile(
    rf"^[ \t]*(?:(?:{JAVA_MODIFIER})[ \t]+)*"
    rf"(?:{GENERIC}[ \t]+)?"
    rf"(?:"
    rf"(?!(?:return|new|throw|else|case|yield|assert|break|continue)\b)[\w.$]+(?:[ \t]*{GENERIC})?(?:[ \t]*\[\])*[ \t]+"
    rf"|(?=(?![A-Z0-9_]+\b)[A-Z]\w*[ \t]*\()"
    rf")"
    rf"(?P<name>\w+)[ \t]*[(=;,]"
)
JAVA_TYPE_KEYWORDS = {"class", "interface", "enum", "record", "@interface"}


def _blank_java_annotations(blanked: str) -> str:
    """`blanked` with every annotation and its argument list turned to spaces, newlines kept.

    Skipped by structure rather than matched by the member pattern: an argument list nests
    parentheses and braces and may span lines, and a pattern that tried to consume it could back
    out of `@GetMapping(` and read `Mapping(` as a constructor. The text is already blanked, so a
    bracket inside a string cannot unbalance the walk.
    """
    out: list[str] = []
    last = 0
    for m in JAVA_ANNOTATION.finditer(blanked):
        if m.start() < last:
            continue
        end = m.end()
        after = end
        while after < len(blanked) and blanked[after] in " \t\n":
            after += 1
        if after < len(blanked) and blanked[after] == "(":
            depth = 0
            for i in range(after, len(blanked)):
                if blanked[i] in "([{":
                    depth += 1
                elif blanked[i] in ")]}":
                    depth -= 1
                    if depth == 0:
                        end = i + 1
                        break
            else:
                end = len(blanked)
        out.append(blanked[last : m.start()])
        out.append(re.sub(r"[^\n]", " ", blanked[m.start() : end]))
        last = end
    out.append(blanked[last:])
    return "".join(out)


def java_declarations(blanked: str) -> list[tuple[int, str]]:
    """(line, name) for every Java type, field, method and constructor in already-blanked source.

    The population the extractor declares, read without it: a constructor counts under its type's
    name, an enum constant and a record component do not, and an anonymous class's method, being
    inside a statement block, is never at a declaring scope. One name per line, so the second field
    of `int a, b;` is not counted; the extractor declares both, and the declarations clause's ±5%
    absorbs it.
    """
    return _scoped_declarations(
        _blank_java_annotations(blanked), decl=JAVA_DECL, member=JAVA_MEMBER, type_keywords=JAVA_TYPE_KEYWORDS
    )


KOTLIN_PRIMARY = re.compile(
    rf"\bclass[ \t]+(?:{KOTLIN_NAME})[ \t]*(?:{GENERIC})?[ \t]*"
    rf"(?:(?:@[\w.]+(?:\([^()\n]*\))?|public|private|internal|protected)[ \t]+)*(?:constructor[ \t]*)?\("
)
KOTLIN_PARAM_PROPERTY = re.compile(
    rf"^\s*(?:@[\w.:]+(?:\([^()]*\))?\s*)*(?:(?:{KOTLIN_MODIFIER})\s+)*(?:val|var)\s+(?P<name>{KOTLIN_NAME})"
)


def kotlin_constructor_properties(blanked: str) -> list[tuple[int, str]]:
    """(line, name) of every `val` or `var` in a Kotlin primary constructor, in already-blanked source.

    `kotlin_declarations` leaves these out on purpose: the hunk that touches a header touches the
    type. The extractor declares them as members, so the declarations clause adds this count to that
    one and compares one population; `changed_symbols` does not read it. Only `(`, `[` and `{` nest,
    because a function type's `->` carries a `>` with no `<`; a comma inside `Map<K, V>` splits a
    parameter in two, and neither half starts with `val`.
    """
    found: list[tuple[int, str]] = []
    for m in KOTLIN_PRIMARY.finditer(blanked):
        depth, i, start = 1, m.end(), m.end()
        parts: list[tuple[int, int]] = []
        while i < len(blanked) and depth:
            char = blanked[i]
            if char in "([{":
                depth += 1
            elif char in ")]}":
                depth -= 1
            if depth == 0 or (char == "," and depth == 1):
                parts.append((start, i))
                start = i + 1
            i += 1
        for lo, hi in parts:
            prop = KOTLIN_PARAM_PROPERTY.match(blanked[lo:hi])
            if prop:
                at = lo + prop.start("name")
                found.append((blanked.count("\n", 0, at) + 1, prop.group("name").strip("`")))
    return found


# The extensions TypeScript's readers also cover: same declaration grammar, same DI pattern.
JS_FAMILY = (".ts", ".tsx", ".js", ".jsx", ".mjs", ".cjs")

# C# and Razor. Each reader takes text `blank_csharp` or `blank_razor` has already emptied of
# comments, strings and character literals, so a name in prose is never a declaration.
CS_MODIFIER = (
    "public|private|protected|internal|static|readonly|sealed|abstract|virtual|override|"
    "partial|async|extern|unsafe|volatile|new|const|required|file|event|implicit|explicit|fixed"
)
CS_HEAD = rf"^[ \t]*(?:\[[^\]\n]*\][ \t]*)*(?:(?:{CS_MODIFIER})[ \t]+)*"
CS_TYPE = re.compile(
    CS_HEAD
    + r"(?P<kw>class|struct|interface|enum|record(?:[ \t]+(?:class|struct))?)[ \t]+(?P<name>\w+)"
    + r"[ \t]*(?:<[^>\n]*>)?[ \t]*(?P<params>\()?"
)
CS_NAMESPACE = re.compile(r"^[ \t]*namespace[ \t]+[\w.]+[ \t]*(?P<semi>;)?")
CS_DELEGATE = re.compile(CS_HEAD + r"delegate[ \t]+.+?[ \t](?P<name>\w+)[ \t]*(?:<[^>\n]*>)?[ \t]*\(")
CS_TYPE_REF = r"(?:[\w.]+(?:<[^;{}()\n]*>)?(?:\?|\[[, ]*\])*|\([^()\n]*\))"
# The keyword look-ahead keeps a statement from reading as a member when a type body holds an
# expression-bodied line, and `operator`/`this` from reading as names.
CS_MEMBER = re.compile(
    CS_HEAD
    + r"(?!(?:class|struct|interface|enum|record|delegate|namespace|return|throw|using|var|await|yield)\b)"
    # `operator` itself must never be read as the type of what follows — a conversion
    # operator's target type (`operator Money(...)`) would otherwise be captured as if it
    # were the declared member's own name.
    + r"(?!operator\b)"
    + rf"(?:{CS_TYPE_REF}[ \t]+)?"
    + r"(?!(?:operator|this)\b)(?P<name>\w+)[ \t]*(?:<[^>\n]*>)?[ \t]*(?:\(|\{|=>|=|;|,|$)"
)
CS_PARAM_NAME = re.compile(r"(\w+)[ \t]*(?:=[^,]*)?$")
# A `#region`/`#endregion` banner names whatever the author likes; it is never code.
CS_REGION = re.compile(r"#(?:region|endregion)\b")


def _parameter_names(text: str) -> list[str]:
    """The names in a primary constructor's parameter list; `text` starts just after its `(`."""
    depth, current, parts = 0, "", []
    for ch in text:
        if ch in "(<[":
            depth += 1
        if ch in ")>]":
            depth -= 1
        if depth < 0:
            break
        if ch == "," and depth == 0:
            parts.append(current)
            current = ""
        else:
            current += ch
    parts.append(current)
    names = []
    for part in parts:
        part = re.sub(r"\[[^\]]*\]", "", part).strip()
        m = CS_PARAM_NAME.search(part)
        # One word is a type with no name, as in an empty `()`, never a parameter.
        if m and " " in part.split("=")[0].strip():
            names.append(m.group(1))
    return names


def csharp_declarations(blanked: str) -> list[tuple[int, str]]:
    """(line, name) for every C# type, member and primary-constructor parameter.

    A type's body holds declarations and a method's does not, so a local is never one. A primary
    constructor's parameters are declared on the type's line: the store declares them as members,
    because a record makes them properties and a class's members read them as fields.
    """
    found: list[tuple[int, str]] = []
    holds: list[bool] = []
    parens = 0
    pending = None
    header_open = False
    params: tuple[int, list[str]] | None = None
    for index, line in enumerate(blanked.split("\n")):
        if params is not None:
            params[1].append(line)
        inside_type = bool(holds) and holds[-1]
        # A header a brace has not yet closed — a base list wrapped onto its own line, a
        # multi-line initialiser after `=`/`=>` — holds no namespace, type, delegate or member of
        # its own; its continuation lines are read only for the brace or `;` that ends it.
        if not header_open and parens == 0 and (not holds or inside_type):
            if m := CS_NAMESPACE.match(line):
                pending = m.group("semi") is None
            elif m := CS_TYPE.match(line):
                found.append((index + 1, m.group("name")))
                pending = m.group("kw") != "enum"
                if m.group("params"):
                    params = (index + 1, [line[m.end():]])
            elif m := CS_DELEGATE.match(line):
                found.append((index + 1, m.group("name")))
                pending = False
            elif inside_type and (m := CS_MEMBER.match(line)):
                found.append((index + 1, m.group("name")))
                pending = False
        for ch in line:
            if ch == "{":
                holds.append(bool(pending))
                pending = None
            elif ch == "}" and holds:
                holds.pop()
            elif ch == "(":
                parens += 1
            elif ch == ")" and parens:
                parens -= 1
        if params is not None and parens == 0:
            found.extend((params[0], name) for name in _parameter_names("\n".join(params[1])))
            params = None
        if pending is not None and parens == 0 and line.rstrip().endswith(";"):
            pending = None
        # An accessor's own `{ }` has already spent `pending`, so an initialiser wrapped after it
        # (`{ get; } =`) must reopen the header itself.
        if pending is None and parens == 0 and line.rstrip().endswith(("=", "=>")):
            pending = False
        header_open = pending is not None
    return found


def _char_literal_end(src: str, i: int) -> int | None:
    if i + 1 >= len(src):
        return None
    if src[i + 1] == "\\":
        end = src.find("'", i + 3, i + 12)
        return None if end < 0 else end + 1
    return i + 3 if src[i + 2 : i + 3] == "'" else None


def _string_end(src: str, i: int) -> int | None:
    """The end of the C# string starting at `i` — regular, `@` verbatim, `$` interpolated, raw — or None.

    An interpolation hole may hold a string of its own. A newline inside a regular string is an
    error in C#, read here as no string at all, so one stray quote cannot blank the rest of a file.
    """
    j = i
    interpolated = verbatim = False
    while j < len(src) and src[j] in "$@":
        interpolated |= src[j] == "$"
        verbatim |= src[j] == "@"
        j += 1
    if src[j : j + 1] != '"':
        return None
    quotes = len(src[j:]) - len(src[j:].lstrip('"'))
    if quotes >= 3:
        end = src.find('"' * quotes, j + quotes)
        return None if end < 0 else end + quotes
    j += 1
    while j < len(src):
        c = src[j]
        if c == "\\" and not verbatim:
            j += 2
            continue
        if c == '"' and verbatim and src[j + 1 : j + 2] == '"':
            j += 2
            continue
        if c == '"':
            return j + 1
        if c == "\n" and not verbatim:
            return None
        if c == "{" and interpolated:
            if src[j + 1 : j + 2] == "{":
                j += 2
                continue
            depth = 0
            while j < len(src):
                if src[j] == "{":
                    depth += 1
                elif src[j] == "}":
                    depth -= 1
                    if depth == 0:
                        break
                elif src[j] == '"':
                    k = _string_end(src, j)
                    if k is None:
                        return None
                    j = k
                    continue
                elif src[j] == "'":
                    # A char literal's own `'"'` is not the hole's closing quote.
                    k = _char_literal_end(src, j)
                    if k is None:
                        return None
                    j = k
                    continue
                j += 1
        j += 1
    return None


def blank_csharp(src: str) -> str:
    """`src` with comments, strings and character literals emptied, every line kept in place."""
    # `read_text` keeps a BOM, and `^[ \t]*` does not step over it: line 1 would go uncounted.
    src = src.removeprefix("﻿")
    out: list[str] = []
    i, size = 0, len(src)
    while i < size:
        pair, c = src[i : i + 2], src[i]
        if pair == "//":
            end = src.find("\n", i)
            i = size if end < 0 else end
            continue
        if pair == "/*":
            end = src.find("*/", i + 2)
            end = size if end < 0 else end + 2
            out.append("\n" * src.count("\n", i, end))
            i = end
            continue
        if c == "#" and src[src.rfind("\n", 0, i) + 1 : i].strip(" \t") == "" and CS_REGION.match(src, i):
            end = src.find("\n", i)
            i = size if end < 0 else end
            continue
        if c in '$@"':
            end = _string_end(src, i)
            if end is not None:
                out.append('""' + "\n" * src.count("\n", i, end))
                i = end
                continue
        if c == "'":
            end = _char_literal_end(src, i)
            if end is not None:
                out.append("''")
                i = end
                continue
        out.append(c)
        i += 1
    return "".join(out)


RAZOR_WRAPPER = "__RazorBlock"
RAZOR_OPEN = re.compile(r"^[ \t]*@(?:code|functions)\b[ \t]*(\{)?")
# The directives whose type names are references; every other directive is blanked with the markup.
RAZOR_KEPT = re.compile(r"^[ \t]*@(?:inject|model|inherits|implements|layout|attribute)[ \t]")
# `@typeparam T` names nothing kept; a `where T : X` constraint names the real reference, so only
# the constraint types are kept, never the parameter's own name.
RAZOR_TYPEPARAM = re.compile(r"^[ \t]*@typeparam[ \t]+\w+[ \t]+where[ \t]+\w+[ \t]*:[ \t]*(?P<constraints>.+?)[ \t]*$")
RAZOR_INJECT = re.compile(r"^[ \t]*@inject[ \t]+(?P<type>\S.*?)[ \t]+@?(?P<name>\w+)[ \t]*$")
RAZOR_TAG = re.compile(r"<([A-Z][\w.]*)")
# `@if (…) {` inside a block is a Razor transition before a C# statement; without the `@` it is C#.
RAZOR_TRANSITION = re.compile(r"(?<![\w@])@(?=(?:if|foreach|for|while|switch|do|try|lock|using)\b)")
RAZOR_COMMENT_CLOSE = {"@*": "*@", "<!--": "-->"}
# A markup expression (`@Formatter.Money(...)`), an `@{ }` block outside `@code`, and a tag's own
# generic argument (`TItem="Order"`) each need an expression reader repograph does not have yet;
# they blank with the rest of the markup until one exists.


def _razor_markup(line: str) -> str:
    if m := RAZOR_TYPEPARAM.match(line):
        return m.group("constraints")
    # A kept directive's argument is C#: a string in it (`Roles = "Admin"`) names nothing.
    return blank_csharp(line) if RAZOR_KEPT.match(line) else " ".join(RAZOR_TAG.findall(line))


def _strip_markup_comments(line: str, closer: str | None) -> tuple[str, str | None]:
    """`line` without its `@* *@` and `<!-- -->` spans, and the closer still awaited at its end.

    Only markup goes through here: inside `@code` a `<!--` may sit in a C# string, where it opens
    nothing. `@@` is an escaped `@`, so `@@*` opens nothing either.
    """
    kept, i = [], 0
    while i < len(line):
        if closer is not None:
            end = line.find(closer, i)
            if end < 0:
                return "".join(kept), closer
            i, closer = end + len(closer), None
            continue
        if line.startswith("@@", i):
            kept.append("@@")
            i += 2
            continue
        opener = next((o for o in RAZOR_COMMENT_CLOSE if line.startswith(o, i)), None)
        if opener is not None:
            closer = RAZOR_COMMENT_CLOSE[opener]
            i += len(opener)
            continue
        kept.append(line[i])
        i += 1
    return "".join(kept), closer


def blank_razor(src: str, markup: Callable[[str], str] = _razor_markup) -> str:
    """A Razor file line for line, as the names it holds.

    Each `@code`/`@functions` body becomes blanked C# inside one class `RAZOR_WRAPPER`; a directive
    naming a type stays whole; every other line is reduced to the component tags it renders. The
    page's own text and HTML go, so a word in its copy is never a reference. `markup` rewrites each
    markup line once its comments are gone; a reader that needs the tags themselves keeps the line.
    """
    lines = src.removeprefix("﻿").split("\n")
    out: list[str] = []
    closer = None
    i = 0
    while i < len(lines):
        lines[i], closer = _strip_markup_comments(lines[i], closer)
        m = None if closer else RAZOR_OPEN.match(lines[i])
        if not m:
            out.append(markup(lines[i]))
            i += 1
            continue
        opener = i if m.group(1) else i + 1
        if opener >= len(lines) or "{" not in lines[opener]:
            out.append("")
            i += 1
            continue
        tail = "\n".join(lines[opener:])
        body = RAZOR_TRANSITION.sub(" ", blank_csharp(tail[tail.index("{"):]))
        depth, close = 0, None
        for k, ch in enumerate(body):
            if ch == "{":
                depth += 1
            elif ch == "}":
                depth -= 1
                if depth == 0:
                    close = k
                    break
        if close is None:
            # An unclosed block leaves nothing a reader can trust below it.
            out.extend("" for _ in lines[i:])
            break
        block = body[: close + 1].split("\n")
        if opener > i:
            out.append("")
        out.append(f"class {RAZOR_WRAPPER} " + block[0])
        out.extend(block[1:])
        i = opener + len(block)
    return "\n".join(out)


def razor_declarations(blanked: str) -> list[tuple[int, str]]:
    """(line, name) for a component's `@inject` members and the members of its blocks.

    The component itself is not here: its name is the file's, which this text does not hold, and
    the declarations reading counts one per component file.
    """
    found = [(n + 1, m.group("name")) for n, line in enumerate(blanked.split("\n")) if (m := RAZOR_INJECT.match(line))]
    found += [d for d in csharp_declarations(blanked) if d[1] != RAZOR_WRAPPER]
    return sorted(found)


def view_declarations(blanked: str) -> list[tuple[int, str]]:
    """A `.cshtml` view declares nothing the store holds: its class is named after its path."""
    return []


CS_CLASS = re.compile(CS_HEAD + r"(?:class|struct|record(?:[ \t]+(?:class|struct))?)[ \t]+(\w+)", re.M)
# `DiReader` asks for the name in group 1 and the type in group 2; C# writes the type first, so the
# look-ahead reads the name before the type is consumed. A field, a property, a constructor
# parameter and a primary-constructor parameter each end in one of `;={,)`. A qualified or generic
# type is named by its last segment, the name a call graph owner carries.
CS_FIELD = re.compile(
    r"(?<![\w.])(?=(?:\w+\.)*[A-Z]\w*(?:<[^;{}()\n]*>)?\??[ \t]+(_?[A-Za-z]\w*)[ \t]*[;=,){])(?:\w+\.)*([A-Z]\w*)"
)
# Razor adds `@inject T Name`, which ends at the end of its line.
RAZOR_FIELD = re.compile(
    r"(?<![\w.])(?=(?:\w+\.)*[A-Z]\w*(?:<[^;{}()\n]*>)?\??[ \t]+@?(_?[A-Za-z]\w*)[ \t]*(?:[;=,){]|\r?$))(?:\w+\.)*([A-Z]\w*)",
    re.M,
)
# A fluent chain wraps the `.` onto its own line as often as TypeScript's does, so the gap
# around it spans newlines too, not only spaces.
CS_CALL = re.compile(r"(?:\bthis[ \t]*\.[ \t]*)?\b(\w+)\s*\??\.\s*(\w+)[ \t]*(?:<[^<>()\n]*>)?[ \t]*\(")
NO_CLASS = re.compile(r"(?!)")


@dataclass(frozen=True)
class ComponentDiReader(DiReader):
    """A file that is one class named by the file: a Razor component. `cls` is never read."""


CSHARP_DI = DiReader(CS_CLASS, CS_FIELD, CS_CALL)
RAZOR_DI = ComponentDiReader(NO_CLASS, RAZOR_FIELD, CS_CALL)

KT_CLASS = re.compile(r"^[ \t]*(?:(?:public|internal|expect|actual|abstract|open|data|sealed)\s+)*class\s+(\w+)", re.M)
KT_FIELD = re.compile(r"\b(?:val|var)\s+(\w+)\s*:\s*(\w+)")
# `files.delete()` and `this.files.delete()` are one call in both JVM languages; `?.` is Kotlin's
# safe call on a nullable injected field, which reads as one call too.
JVM_CALL = re.compile(r"(?:\bthis\s*\.\s*)?\b(\w+)\s*\??\.\s*(\w+)\s*\(", re.S)
JAVA_CLASS = re.compile(
    r"^[ \t]*(?:(?:public|protected|private|abstract|static|final|sealed|non-sealed|strictfp)\s+)*(?:class|record|enum)\s+(\w+)",
    re.M,
)
# Java writes the type before the name; the lookahead captures the name first so group 1 is the
# name and group 2 the type, the order `di_call_graph` reads for every language. A qualified or
# nested-class type (`Outer.Inner`) is named by its last segment, as `CS_FIELD` also does, so the
# graph's `Type.method` edges never carry a second dot.
JAVA_FIELD = re.compile(r"\b(?=[A-Z][\w.]*(?:<[^;=(){}]*>)?(?:\[\])*\s+(\w+)\s*[;=])(?:[A-Z]\w*\.)*([A-Z]\w*)")
KOTLIN_DI = DiReader(KT_CLASS, KT_FIELD, JVM_CALL)
JAVA_DI = DiReader(JAVA_CLASS, JAVA_FIELD, JVM_CALL)


# Keyed by extension. A language joins the truth by adding its readers here; a file of an
# extension no table names contributes nothing, which is what a language not yet read is.
DECLARATIONS: dict[str, Callable[[str], list[tuple[int, str]]]] = {
    **{ext: typescript_declarations for ext in JS_FAMILY},
    ".kt": kotlin_declarations,
    ".cs": csharp_declarations,
    ".razor": razor_declarations,
    ".cshtml": view_declarations,
    ".java": java_declarations,
}
BLANKERS: dict[str, Callable[[str], str]] = {
    **{ext: blank_typescript for ext in JS_FAMILY},
    ".kt": blank_kotlin,
    ".cs": blank_csharp,
    ".razor": blank_razor,
    ".cshtml": blank_razor,
    ".java": blank_java,
}
DI_READERS: dict[str, DiReader] = {
    **{ext: TYPESCRIPT_DI for ext in JS_FAMILY},
    ".cs": CSHARP_DI,
    ".razor": RAZOR_DI,
    ".kt": KOTLIN_DI,
    ".java": JAVA_DI,
}
# The last line of a declaration where counting brackets from its first line ends it too soon.
DECLARATION_ENDS: dict[str, Callable[[list[str], int], int]] = {}
# `(caller, callee)` name pairs for a chain no field-and-call pattern can say: a migration altering
# a table another created, a resolver answering a query. A callee is `Owner` or `Owner.member`, the
# shape `shortest_path` hops on. A file whose extension has one is read by it and by no DiReader.
CALL_READERS: dict[str, Callable[[str], list[tuple[str, str]]]] = {}


def code_globs() -> tuple[str, ...]:
    """ripgrep's `-g` arguments for every extension a reader is registered for, in registration order."""
    exts = list(dict.fromkeys([*DECLARATIONS, *DI_READERS, *CALL_READERS]))
    return tuple(arg for ext in exts for arg in ("-g", f"*{ext}"))


SQL_DOLLAR = re.compile(r"\$([A-Za-z_][A-Za-z_0-9]*)?\$")


def blank_sql(src: str) -> str:
    """`src` with its comments, string literals and dollar-quoted bodies blanked, line count kept.

    A quoted identifier stays, because `"Status"` is a name. A dollar-quoted body goes, because PL/pgSQL is not
    read for declarations, and a `CREATE TABLE` inside a function body declares nothing until the function runs.
    """
    out: list[str] = []
    i, size = 0, len(src)
    while i < size:
        char, pair = src[i], src[i : i + 2]
        if pair == "--":
            end = src.find("\n", i)
            i = size if end < 0 else end
        elif pair == "/*":
            # PostgreSQL nests block comments, unlike C.
            depth, end = 1, i + 2
            while end < size and depth:
                if src[end : end + 2] == "/*":
                    depth, end = depth + 1, end + 2
                elif src[end : end + 2] == "*/":
                    depth, end = depth - 1, end + 2
                else:
                    end += 1
            # A space, not nothing: `CREATE/**/TABLE` is two words, and joining them reads as none.
            out.append(" " + "\n" * src.count("\n", i, end))
            i = end
        elif char == '"':
            end = i + 1
            while end < size and not (src[end] == '"' and src[end + 1 : end + 2] != '"'):
                end += 2 if src[end : end + 2] == '""' else 1
            out.append(src[i : end + 1])
            i = end + 1
        elif char == "'":
            # Only an `E'…'` string treats a backslash as an escape; a standard string doubles its quotes.
            escapes = i > 0 and src[i - 1] in "eE" and (i < 2 or not (src[i - 2].isalnum() or src[i - 2] == "_"))
            end = i + 1
            while end < size:
                if escapes and src[end] == "\\":
                    end += 2
                elif src[end : end + 2] == "''":
                    end += 2
                elif src[end] == "'":
                    break
                else:
                    end += 1
            out.append("'" + "\n" * src.count("\n", i, end) + "'")
            i = end + 1
        elif char == "$" and not (i > 0 and (src[i - 1].isalnum() or src[i - 1] == "_")) and (dollar := SQL_DOLLAR.match(src, i)):
            tag = dollar.group(0)
            end = src.find(tag, dollar.end())
            end = size if end < 0 else end + len(tag)
            out.append("$$" + "\n" * src.count("\n", i, end) + "$$")
            i = end
        else:
            out.append(char)
            i += 1
    return "".join(out)


SQL_TOKEN = re.compile(r'"(?:[^"]|"")*"|[^\W\d][\w$]*|\d+|\S')
SQL_OBJECTS = {"table", "view", "function", "procedure", "type", "schema", "sequence", "trigger", "policy", "index"}
SQL_CREATE_MODIFIERS = {"or", "replace", "temp", "temporary", "unlogged", "global", "local", "constraint", "unique", "materialized", "recursive"}
SQL_NOT_A_COLUMN = {"constraint", "primary", "unique", "foreign", "check", "exclude", "like"}


def _sql_statements(blanked: str) -> list[list[tuple[str, int, int]]]:
    """Blanked SQL as statements of (token, line, paren depth). A `;` inside parentheses ends nothing."""
    statements: list[list[tuple[str, int, int]]] = []
    current: list[tuple[str, int, int]] = []
    depth, line, last = 0, 1, 0
    for match in SQL_TOKEN.finditer(blanked):
        line += blanked.count("\n", last, match.start())
        last = match.start()
        token = match.group(0)
        if token == ";" and depth == 0:
            if current:
                statements.append(current)
            current = []
            continue
        if token == ")":
            depth = max(0, depth - 1)
        current.append((token, line, depth))
        if token == "(":
            depth += 1
    if current:
        statements.append(current)
    return statements


def _sql_word(tokens, i: int, *words: str) -> bool:
    return i < len(tokens) and not tokens[i][0].startswith('"') and tokens[i][0].lower() in words


def _sql_skip(tokens, i: int, *sequence: str) -> int:
    for offset, word in enumerate(sequence):
        if not _sql_word(tokens, i + offset, word):
            return i
    return i + len(sequence)


def _sql_is_name(token: str) -> bool:
    return token.startswith('"') or token[:1].isalpha() or token[:1] == "_"


def _sql_ident(token: str) -> str:
    # PostgreSQL folds an unquoted identifier to lower case and keeps a quoted one.
    return token[1:-1].replace('""', '"') if token.startswith('"') else token.lower()


def _sql_name(tokens, i: int) -> tuple[str | None, int]:
    if i >= len(tokens) or not _sql_is_name(tokens[i][0]):
        return None, i
    parts = [_sql_ident(tokens[i][0])]
    i += 1
    while i + 1 < len(tokens) and tokens[i][0] == "." and _sql_is_name(tokens[i + 1][0]):
        parts.append(_sql_ident(tokens[i + 1][0]))
        i += 2
    return "/".join(parts), i


def _sql_columns(tokens, i: int, table: str, found: list) -> None:
    expect = True
    for token, line, depth in tokens[i + 1 :]:
        if depth == 0:
            return
        if depth == 1 and token == ",":
            expect = True
        elif expect and depth == 1:
            expect = False
            if _sql_is_name(token) and not (not token.startswith('"') and token.lower() in SQL_NOT_A_COLUMN):
                found.append((line, f"{table}.{_sql_ident(token)}"))


def sql_declarations(blanked: str) -> list[tuple[int, str]]:
    """(line, name) for each object a statement creates or alters, and each column, trigger, policy and named index.

    Names are the repository's: schema parts joined with `/`, a member after `.`. An `ALTER TABLE` declares its
    table, so every file altering a table is one that declares it.
    """
    found: list[tuple[int, str]] = []
    for tokens in _sql_statements(blanked):
        line = tokens[0][1]
        if _sql_word(tokens, 0, "alter") and _sql_word(tokens, 1, "table"):
            i = _sql_skip(tokens, 2, "if", "exists")
            i = _sql_skip(tokens, i, "only")
            table, i = _sql_name(tokens, i)
            if table is None:
                continue
            found.append((line, table))
            while i < len(tokens):
                if _sql_word(tokens, i, "add") and tokens[i][2] == 0:
                    j = _sql_skip(tokens, i + 1, "column")
                    j = _sql_skip(tokens, j, "if", "not", "exists")
                    if j < len(tokens) and _sql_is_name(tokens[j][0]) and not _sql_word(tokens, j, *SQL_NOT_A_COLUMN):
                        found.append((tokens[j][1], f"{table}.{_sql_ident(tokens[j][0])}"))
                    i = j
                i += 1
            continue
        if not _sql_word(tokens, 0, "create"):
            continue
        i = 1
        while _sql_word(tokens, i, *SQL_CREATE_MODIFIERS):
            i += 1
        if not _sql_word(tokens, i, *SQL_OBJECTS):
            continue
        kind = tokens[i][0].lower()
        i = _sql_skip(tokens, i + 1, "concurrently")
        i = _sql_skip(tokens, i, "if", "not", "exists")
        if kind == "schema" and _sql_word(tokens, i, "authorization"):
            continue
        name = None
        if not _sql_word(tokens, i, "on"):
            name, i = _sql_name(tokens, i)
        if kind in ("trigger", "policy", "index"):
            while i < len(tokens) and not (_sql_word(tokens, i, "on") and tokens[i][2] == 0):
                i += 1
            table, _ = _sql_name(tokens, _sql_skip(tokens, i + 1, "only"))
            if table is None:
                continue
            found.append((line, table))
            if name:
                found.append((line, f"{table}.{name}"))
            continue
        if name is None:
            continue
        found.append((line, name))
        if kind == "table" and i < len(tokens) and tokens[i][0] == "(":
            _sql_columns(tokens, i, name, found)
    return found


def sql_trigger_calls(blanked: str) -> list[tuple[str, str]]:
    """(table, function) for each `CREATE TRIGGER … ON table … EXECUTE FUNCTION|PROCEDURE function`."""
    edges: list[tuple[str, str]] = []
    for tokens in _sql_statements(blanked):
        if not _sql_word(tokens, 0, "create"):
            continue
        i = 1
        while _sql_word(tokens, i, "or", "replace", "constraint"):
            i += 1
        if not _sql_word(tokens, i, "trigger"):
            continue
        table = function = None
        for k in range(i, len(tokens)):
            if table is None and _sql_word(tokens, k, "on") and tokens[k][2] == 0:
                table, _ = _sql_name(tokens, _sql_skip(tokens, k + 1, "only"))
            if _sql_word(tokens, k, "execute") and _sql_word(tokens, k + 1, "function", "procedure"):
                function, _ = _sql_name(tokens, k + 2)
        if table and function:
            edges.append((table, function))
    return edges


def blank_graphql(src: str) -> str:
    """`src` with `#` comments and strings blanked, line count kept; a block string keeps its newlines."""
    out: list[str] = []
    i, size = 0, len(src)
    while i < size:
        char = src[i]
        if char == "#":
            end = src.find("\n", i)
            i = size if end < 0 else end
        elif src.startswith('"""', i):
            end = i + 3
            while end < size and not src.startswith('"""', end):
                end += 4 if src.startswith('\\"""', end) else 1
            end = min(size, end + 3)
            out.append('""' + "\n" * src.count("\n", i, end))
            i = end
        elif char == '"':
            end = i + 1
            while end < size and src[end] not in '"\n':
                end += 2 if src[end] == "\\" else 1
            out.append('""')
            i = end + 1 if end < size and src[end] == '"' else end
        else:
            out.append(char)
            i += 1
    return "".join(out)


GQL_NAME = r"[_A-Za-z][_0-9A-Za-z]*"
GQL_TOP = re.compile(
    rf"(?P<op>query|mutation|subscription)\s+(?P<opname>{GQL_NAME})"
    rf"|fragment\s+(?P<frag>{GQL_NAME})\s+on\b"
    rf"|(?:extend\s+)?(?P<kw>type|interface|input|enum|union|scalar)\s+(?P<type>{GQL_NAME})"
    rf"|directive\s+@(?P<dir>{GQL_NAME})"
    r"|(?P<schema>schema)\b"
)
GQL_FIELD = re.compile(rf"(?P<name>{GQL_NAME})\s*[(:]")
# An enum's values are not fields, and a union has no body; only these three hold `name:` fields.
GQL_HOLDS_FIELDS = {"type", "interface", "input"}


def graphql_declarations(blanked: str) -> list[tuple[int, str]]:
    """(line, name) for each operation, fragment, directive and type defined or extended at the top level, and each field.

    An argument list is skipped by its parentheses, so `books(first: Int)` declares `books` and not `first`.
    """
    found: list[tuple[int, str]] = []
    depth = parens = 0
    holder = pending = None
    line_starts = [0] + [m.end() for m in re.finditer("\n", blanked)]
    pos, size = 0, len(blanked)
    while pos < size:
        before = blanked[pos - 1] if pos else " "
        starts_word = not (before.isalnum() or before in "_@")
        top = GQL_TOP.match(blanked, pos) if depth == 0 and parens == 0 and starts_word else None
        if top:
            line = bisect_right(line_starts, pos)
            if top.group("schema"):
                # A `schema { … }` body holds operation types, not fields, and must not inherit a bodiless `type`'s name.
                pending = None
                pos = top.end()
                continue
            if top.group("opname"):
                name, pending = f"{top.group('op')}/{top.group('opname')}", None
            elif top.group("frag"):
                name, pending = f"fragment/{top.group('frag')}", None
            elif top.group("type"):
                name = top.group("type")
                pending = name if top.group("kw") in GQL_HOLDS_FIELDS else None
            else:
                name, pending = f"directive/{top.group('dir')}", None
            found.append((line, name))
            pos = top.end()
            continue
        char = blanked[pos]
        if char == "(":
            parens += 1
        elif char == ")":
            parens = max(0, parens - 1)
        elif parens:
            pass
        elif char == "{":
            depth += 1
            if depth == 1:
                holder, pending = pending, None
        elif char == "}":
            depth = max(0, depth - 1)
            if depth == 0:
                holder = None
        elif depth == 1 and holder and starts_word and (field := GQL_FIELD.match(blanked, pos)):
            found.append((bisect_right(line_starts, pos), f"{holder}.{field.group('name')}"))
            pos = field.end("name")
            continue
        pos += 1
    return found


GQL_SPREAD_SCAN = re.compile(
    rf"\b(?P<op>query|mutation|subscription)\s+(?P<opname>{GQL_NAME})"
    rf"|\bfragment\s+(?P<frag>{GQL_NAME})\s+on\b"
    rf"|\.\.\.\s*(?!on\b)(?P<spread>{GQL_NAME})"
    rf"|(?P<mark>[{{}}()])"
)


def graphql_spread_calls(blanked: str) -> list[tuple[str, str]]:
    """(owner, `fragment/Name`) for each `...Name` inside a named operation or a fragment.

    An anonymous operation declares nothing to own its spreads, so they are left out, and an inline `... on T`
    is a type condition, not a spread.
    """
    edges: list[tuple[str, str]] = []
    owner, armed, depth, parens = None, False, 0, 0
    for match in GQL_SPREAD_SCAN.finditer(blanked):
        mark = match.group("mark")
        if mark in ("(", ")"):
            parens = parens + 1 if mark == "(" else max(0, parens - 1)
        elif parens:
            continue
        elif mark == "{":
            if depth == 0 and not armed:
                owner = None
            depth, armed = depth + 1, False
        elif mark == "}":
            depth = max(0, depth - 1)
        elif depth == 0 and (match.group("opname") or match.group("frag")):
            owner = f"{match.group('op')}/{match.group('opname')}" if match.group("opname") else f"fragment/{match.group('frag')}"
            armed = True
        elif depth and match.group("spread") and owner:
            edges.append((owner, f"fragment/{match.group('spread')}"))
    return edges


DECLARATIONS[".sql"] = sql_declarations
DECLARATIONS[".gql"] = graphql_declarations
DECLARATIONS[".graphql"] = graphql_declarations
BLANKERS[".sql"] = blank_sql
BLANKERS[".gql"] = blank_graphql
BLANKERS[".graphql"] = blank_graphql

# A trigger naming its function and a spread naming its fragment are the `trace` chains of these languages, and
# neither has the class and typed field a `DiReader` follows. `di_call_graph` hands a reader raw text, so each blanks it.
CALL_READERS[".sql"] = lambda src: sql_trigger_calls(blank_sql(src))
CALL_READERS[".gql"] = lambda src: graphql_spread_calls(blank_graphql(src))
CALL_READERS[".graphql"] = lambda src: graphql_spread_calls(blank_graphql(src))

SQL_OPENS = re.compile(r"\s*(create|alter)\b", re.IGNORECASE)


def sql_declaration_end(lines: list[str], start: int) -> int:
    """The last line of the declaration beginning at `start` (1-based), in blanked SQL.

    A line opening a statement runs to the `;` that ends it, as the extractor spans it: `ALTER TABLE t` with its
    commands below, or a `CREATE TRIGGER` over several lines, opens no bracket on its first line. A member line, a
    column inside `CREATE TABLE (…)`, keeps `declaration_end`'s bracket count, which ends it on its own line.
    """
    if not SQL_OPENS.match(lines[start - 1]):
        return declaration_end(lines, start)
    depth = 0
    for i in range(start - 1, len(lines)):
        for char in lines[i]:
            if char == "(":
                depth += 1
            elif char == ")":
                depth -= 1
            elif char == ";" and depth <= 0:
                return i + 1
    return len(lines)


DECLARATION_ENDS[".sql"] = sql_declaration_end


RUST_RAW_STRING = re.compile(r'b?r(#*)"')


def blank_rust(src: str) -> str:
    """`src` with Rust comments, strings and character literals blanked, line count kept.

    Brace scope is all the Rust reader needs, so a string collapses to its quotes and its
    newlines. A `'` opens a character literal only when it is escaped (`'\\n'`) or closes two
    characters on (`'{'`); otherwise it is a lifetime (`'a`, `'static`), which is code, and
    blanking it as a literal would swallow the rest of the line. Block comments nest, as Rust's do.
    """
    out: list[str] = []
    i, n = 0, len(src)

    def newlines(a: int, b: int) -> str:
        return "\n" * src.count("\n", a, b)

    while i < n:
        c = src[i]
        after_word = i > 0 and (src[i - 1].isalnum() or src[i - 1] == "_")
        if src.startswith("//", i):
            end = src.find("\n", i)
            i = n if end < 0 else end
        elif src.startswith("/*", i):
            depth, end = 1, i + 2
            while end < n and depth:
                if src.startswith("/*", end):
                    depth, end = depth + 1, end + 2
                elif src.startswith("*/", end):
                    depth, end = depth - 1, end + 2
                else:
                    end += 1
            out.append(newlines(i, end))
            i = end
        elif (raw := RUST_RAW_STRING.match(src, i)) and not after_word:
            close = '"' + raw.group(1)
            end = src.find(close, raw.end())
            end = n if end < 0 else end + len(close)
            out.append('"' + newlines(i, end) + '"')
            i = end
        elif c == '"' or (c == "b" and src.startswith('"', i + 1) and not after_word):
            end = i + (1 if c == '"' else 2)
            while end < n and src[end] != '"':
                end += 2 if src[end] == "\\" else 1
            end = min(end + 1, n)
            out.append('"' + newlines(i, end) + '"')
            i = end
        elif c == "'" and src.startswith("\\", i + 1) and (close := src.find("'", i + 3)) >= 0:
            out.append("' '")
            i = close + 1
        elif c == "'" and i + 2 < n and src[i + 2] == "'" and src[i + 1] != "\n":
            out.append("' '")
            i += 3
        else:
            out.append(c)
            i += 1
    return "".join(out)


RUST_DECL = re.compile(
    r"^[ \t]*(?:#!?\[[^\n]*\][ \t]*)*"
    r"(?:pub(?:[ \t]*\([^()\n]*\))?[ \t]+)?"
    r"(?:(?:default|const|async|unsafe|extern[ \t]+\"[^\"\n]*\")[ \t]+)*"
    r"(?:(?P<kw>impl|mod|trait|fn|struct|enum|union|type|const|static)\b|(?P<macro>macro_rules!))"
    r"(?:(?<=impl)|(?<=mod)[ \t]+\w+|[ \t]*(?:mut[ \t]+)?(?P<name>r#\w+|\w+))"
)
RUST_TYPE_KEYWORDS = {"impl", "mod", "trait"}


@dataclass(frozen=True)
class _RustMatch:
    kw: str
    name: str | None

    def group(self, which: str) -> str | None:
        return self.kw if which == "kw" else self.name


class _RustDeclaration:
    """`RUST_DECL` as `_scoped_declarations` reads a match: `kw` always set, and no `name` for an
    `impl` or a `mod`, which open a declaring scope and are not themselves symbols."""

    def match(self, line: str) -> _RustMatch | None:
        m = RUST_DECL.match(line)
        if not m:
            return None
        kw = m.group("kw") or "macro_rules"
        name = None if kw in ("impl", "mod") else m.group("name")
        return _RustMatch(kw, None if name == "mut" else name)


def rust_declarations(blanked: str) -> list[tuple[int, str]]:
    """(line, name) for every Rust item at a declaring scope — the file, a `mod`, `impl` or
    `trait` body — in already-blanked source. A `fn` inside a `fn` is a local."""
    return _scoped_declarations(blanked, decl=_RustDeclaration(), member=None, type_keywords=RUST_TYPE_KEYWORDS)


def rust_declaration_end(lines: list[str], start: int) -> int:
    """The last line of the Rust item beginning at `start`: its body's closing brace, or the `;`
    that ends an item with none. A header may close its parentheses and open its body lines
    later — a `where` clause, a return type on its own line — so the count waits for the body."""
    depth = 0
    opened = False
    for i in range(start - 1, len(lines)):
        for char in lines[i]:
            if char in "{[(":
                depth += 1
                opened = opened or char == "{"
            elif char in "}])":
                depth -= 1
            elif char == ";" and depth == 0 and not opened:
                return i + 1
        if opened and depth <= 0:
            return i + 1
    return len(lines)


RUST_STRUCT = re.compile(r"\bstruct\s+(\w+)\s*(?:<(?P<params>[^{;]*?)>)?\s*(?:where\b(?P<where>[^{;]*))?\{")
RUST_IMPL = re.compile(r"(?m)^[ \t]*(?:unsafe[ \t]+)?impl\b(?P<head>[^{;]*)\{")
RUST_SELF_CALL = re.compile(r"\bself\s*\.\s*(\w+)\s*\.\s*(\w+)\s*\(")
# What a method call sees through: `&'a mut T`, `Box<T>`, `Arc<T>`, `Rc<T>`, `dyn T`, `impl T`.
RUST_PEEL = re.compile(r"^(?:&\s*(?:'\w+\s+)?(?:mut\s+)?|(?:Box|Arc|Rc)\s*<|dyn\s+|impl\s+)")


def _matching_brace(text: str, open_at: int) -> int:
    depth = 0
    for i in range(open_at, len(text)):
        if text[i] == "{":
            depth += 1
        elif text[i] == "}":
            depth -= 1
            if depth == 0:
                return i
    return len(text)


def _split_top(text: str) -> list[str]:
    """`text` split at commas outside `<>`, `()` and `[]`."""
    parts, depth, current = [], 0, ""
    for char in text:
        depth += (char in "<([") - (char in ">)]")
        if char == "," and depth == 0:
            parts.append(current)
            current = ""
        else:
            current += char
    return parts + [current]


def _strip_generics(text: str) -> str:
    out, depth = "", 0
    for char in text:
        if char == "<":
            depth += 1
        elif char == ">":
            depth -= 1
        elif depth == 0:
            out += char
    return out


def _rust_bounds(params: str | None, where: str | None) -> dict[str, str]:
    """Type parameter → its first bound's last segment, from `<E: Exec>` and `where E: Exec`."""
    bounds: dict[str, str] = {}
    for part in _split_top(params or "") + _split_top(where or ""):
        if ":" not in part:
            continue
        name, rest = part.split(":", 1)
        first = re.match(r"\s*(?:\w+::)*(\w+)", rest)
        if first and name.strip().isidentifier():
            bounds.setdefault(name.strip(), first.group(1))
    return bounds


def _rust_type_name(written: str, bounds: dict[str, str]) -> str:
    text = written.strip()
    while peeled := RUST_PEEL.match(text):
        text = text[peeled.end():].strip()
    name = re.match(r"\w*", _strip_generics(text).split("::")[-1].strip()).group(0)
    return bounds.get(name, name)


def rust_calls(src: str) -> list[tuple[str, str]]:
    """(struct, `Type.method`) for every `self.field.method(` an `impl` in this file makes through
    a field its struct declares in this file.

    A field typed by the struct's own parameter is called through that parameter's first bound —
    `exec: E` with `E: Exec` calls `Exec::run` — and the `impl` is a block apart from the struct,
    so a pattern read inside one class body cannot see the chain. A struct whose `impl` sits in
    another file is not read, as the extractor does not read it either.
    """
    blanked = blank_rust(src)
    fields: dict[str, dict[str, str]] = defaultdict(dict)
    for m in RUST_STRUCT.finditer(blanked):
        body = blanked[m.end() : _matching_brace(blanked, m.end() - 1)]
        bounds = _rust_bounds(m.group("params"), m.group("where"))
        for field in _split_top(body):
            fm = re.match(r"\s*(?:pub(?:\s*\([^)]*\))?\s+)?(\w+)\s*:\s*(.+)", field, re.S)
            if fm:
                fields[m.group(1)][fm.group(1)] = _rust_type_name(fm.group(2), bounds)
    pairs: set[tuple[str, str]] = set()
    for m in RUST_IMPL.finditer(blanked):
        head = m.group("head").lstrip()
        if head.startswith("<"):
            depth = 0
            for i, char in enumerate(head):
                depth += (char == "<") - (char == ">")
                if depth == 0:
                    head = head[i + 1 :]
                    break
        head = re.split(r"\bfor\b", re.split(r"\bwhere\b", head)[0])[-1]
        owner = re.match(r"\w*", _strip_generics(head).strip().lstrip("&").strip().split("::")[-1].strip()).group(0)
        body = blanked[m.end() : _matching_brace(blanked, m.end() - 1)]
        for call in RUST_SELF_CALL.finditer(body):
            if call.group(1) in fields.get(owner, {}):
                pairs.add((owner, f"{fields[owner][call.group(1)]}.{call.group(2)}"))
    return sorted(pairs)


PY_STRING_OPEN = re.compile(r"([rRbBuUfF]{0,2})('''|\"\"\"|'|\")")


def blank_python(src: str) -> str:
    """`src` with Python comments and string bodies turned to spaces, every column kept.

    The Python reader reads blocks by indentation, so nothing may move: a docstring collapsed
    to its quotes would put the closing quotes in column 0 and end every block above it. The
    quotes and newlines stay; an unterminated single-quoted string ends at its line.
    """
    out: list[str] = []
    i, n = 0, len(src)
    while i < n:
        c = src[i]
        if c == "#":
            end = src.find("\n", i)
            end = n if end < 0 else end
            out.append(" " * (end - i))
            i = end
            continue
        opened = PY_STRING_OPEN.match(src, i) if c in "\"'rRbBuUfF" else None
        if opened and not (i > 0 and (src[i - 1].isalnum() or src[i - 1] == "_")):
            quote = opened.group(2)
            end = opened.end()
            while end < n and not src.startswith(quote, end):
                if src[end] == "\\":
                    end += 2
                    continue
                if len(quote) == 1 and src[end] == "\n":
                    break
                end += 1
            end = min(end, n)
            out.append(src[i : opened.end()])
            out.append("".join(ch if ch == "\n" else " " for ch in src[opened.end() : end]))
            if src.startswith(quote, end):
                out.append(quote)
                end += len(quote)
            i = end
            continue
        out.append(c)
        i += 1
    return "".join(out)


PY_KEYWORDS = frozenset({
    "if", "elif", "else", "for", "while", "try", "except", "finally", "with", "match", "case", "return",
    "lambda", "def", "class", "async", "await", "import", "from", "global", "nonlocal", "pass", "break",
    "continue", "raise", "del", "assert", "yield", "not", "and", "or", "in", "is",
})
PY_LINE = re.compile(
    r"^(?P<indent>[ \t]*)(?:"
    r"(?:async[ \t]+)?(?P<kw>def|class)[ \t]+(?P<name>\w+)"
    r"|(?P<assign>[A-Za-z_]\w*)[ \t]*(?::[^=\n]*)?=(?!=)"
    r"|(?P<ann>[A-Za-z_]\w*)[ \t]*:[ \t]*[^=\s][^=\n]*$"
    r"|(?P<block>\w+)\b[^\n]*:[ \t]*$)"
)


def python_declarations(blanked: str) -> list[tuple[int, str]]:
    """(line, name) for every `def`, `class` and plain-name assignment directly in the module or
    in a class body, in already-blanked source.

    Indentation, not brackets: a stack records, for each open block, whether it is a class.
    A line declares when every block around it is a class. A line that starts inside brackets or
    after a `\\` continues a statement, and a decorator line is its definition's.
    """
    found: list[tuple[int, str]] = []
    stack: list[tuple[int, bool]] = []
    depth = 0
    continued = False
    for index, line in enumerate(blanked.split("\n")):
        inside = depth > 0 or continued
        for char in line:
            if char in "([{":
                depth += 1
            elif char in ")]}" and depth:
                depth -= 1
        continued = line.rstrip().endswith("\\")
        stripped = line.strip()
        if not stripped or inside or stripped.startswith("@"):
            continue
        indent = len(line) - len(line.lstrip())
        while stack and stack[-1][0] >= indent:
            stack.pop()
        declaring = all(is_class for _, is_class in stack)
        m = PY_LINE.match(line)
        if not m:
            continue
        if m.group("kw"):
            if declaring:
                found.append((index + 1, m.group("name")))
            stack.append((indent, m.group("kw") == "class"))
        elif m.group("block"):
            stack.append((indent, False))
        elif declaring and (name := m.group("assign") or m.group("ann")) and name not in PY_KEYWORDS:
            found.append((index + 1, name))
    return found


def python_declaration_end(lines: list[str], start: int) -> int:
    """The last line of the Python declaration beginning at `start`: its header through any
    open brackets, then every following line indented deeper than the header. Blank lines —
    and blanked docstring lines — neither end a block nor extend it."""
    header = lines[start - 1]
    indent = len(header) - len(header.lstrip())
    depth = 0
    i = start - 1
    while i < len(lines):
        depth += sum(lines[i].count(c) for c in "([{") - sum(lines[i].count(c) for c in ")]}")
        if depth <= 0:
            break
        i += 1
    last = min(i, len(lines) - 1) + 1
    for j in range(last, len(lines)):
        if not lines[j].strip():
            continue
        if len(lines[j]) - len(lines[j].lstrip()) <= indent:
            break
        last = j + 1
    return last


PYTHON_DI = DiReader(
    re.compile(r"(?m)^[ \t]*class[ \t]+(\w+)"),
    # `self.x = T(…)`, `self.x: T = T(…)`, `self.x = mod.T(…)`: the constructor names the type.
    re.compile(r"\bself\.(\w+)\s*(?::[^=\n]*)?=\s*(?:[A-Za-z_]\w*\.)*([A-Za-z_]\w*)\s*\("),
    re.compile(r"\bself\.(\w+)\s*\.\s*(\w+)\s*\("),
)

DECLARATIONS[".rs"] = rust_declarations
DECLARATIONS[".py"] = python_declarations
BLANKERS[".rs"] = blank_rust
BLANKERS[".py"] = blank_python
DI_READERS[".py"] = PYTHON_DI
# Rust's chain runs through a struct and its `impl` block, which no field-and-call pattern reads; `di_call_graph`
# hands a reader raw text, and `rust_calls` blanks it.
CALL_READERS[".rs"] = rust_calls
# A `def` balances its brackets on its own line and a Rust `where` clause opens its body lines later, so
# counting brackets would end both before their bodies.
DECLARATION_ENDS[".rs"] = rust_declaration_end
DECLARATION_ENDS[".py"] = python_declaration_end


def declarations(rel: str, src: str) -> tuple[list[str], list[tuple[int, str]]]:
    """The blanked lines of one file and the (line, name) of every declaration in it."""
    blanked = blanked_source(rel, src)
    # TypeScript's reader stays the fallback, as it was for every extension but `.kt`.
    reader = DECLARATIONS.get(Path(rel).suffix, typescript_declarations)
    return blanked.split("\n"), reader(blanked)


def code_files_naming(repo: Path, token: str) -> list[str]:
    word = re.compile(rf"\b{re.escape(token)}\b")
    hits = rg(repo, ["-l", "--word-regexp", "--fixed-strings", token, *code_globs()])
    return sorted(
        f for f in hits
        if word.search(blanked_source(f, (repo / f).read_text(encoding="utf8", errors="replace")))
    )


def declaration_of(repo: Path, name: str) -> str | None:
    for line in rg(repo, ["-l", f"^export (?:abstract )?class {name}\\b", *code_globs()]):
        return line
    for line in rg(repo, ["-l", f"^export .*\\b{name}\\b", *code_globs()]):
        return line
    return None


def di_call_graph(repo: Path, roots: list[str]) -> dict:
    """Class → the `Type.method` calls its members make through injected fields.

    Each file is read by the call reader its extension is registered with, else by its
    `DiReader`, and a file of an extension with neither is skipped: another language's source read
    with TypeScript's patterns matches nothing today and could match anything tomorrow.
    """
    if not roots:
        # ripgrep reads the whole tree when handed no path at all; a tree with none of the
        # roots the caller offered must scan nothing, not everything.
        return {"edges": {}, "declared": {}}
    files = rg(repo, ["--files", *code_globs(), *roots])
    edges: dict[str, set[str]] = defaultdict(set)
    declared: dict[str, str] = {}
    for rel in files:
        ext = Path(rel).suffix
        if ext in CALL_READERS:
            src = (repo / rel).read_text(encoding="utf8", errors="replace")
            for caller, callee in CALL_READERS[ext](src):
                declared.setdefault(caller, rel)
                edges[caller].add(callee)
            continue
        reader = DI_READERS.get(ext)
        if reader is None:
            continue
        src = (repo / rel).read_text(encoding="utf8", errors="replace")
        if isinstance(reader, ComponentDiReader):
            # The Razor compiler names a component's class after its file; `Checkout.razor` is `Checkout`.
            marks = [(0, Path(rel).name.split(".")[0])]
        else:
            marks = [(m.start(), m.group(1)) for m in reader.cls.finditer(src)]
        marks.append((len(src), None))
        for i in range(len(marks) - 1):
            start, name = marks[i]
            body = src[start : marks[i + 1][0]]
            declared[name] = rel
            # A nested generic (`IDictionary<string, List<int>>`) gives a spurious second match
            # starting at its inner argument; the first match, left to right, is always the
            # outermost type, so it must win, not the last one written.
            fields: dict[str, str] = {}
            for m in reader.field.finditer(body):
                fields.setdefault(m.group(1), m.group(2))
            for m in reader.call.finditer(body):
                target = fields.get(m.group(1))
                if target:
                    edges[name].add(f"{target}.{m.group(2)}")
    return {"edges": {k: sorted(v) for k, v in edges.items()}, "declared": declared}


def shortest_path(graph: dict, src: str, dst: str, max_hops: int = 6) -> list[str] | None:
    edges = graph["edges"]
    queue = [(src, [src])]
    seen = {src}
    while queue:
        cur, path = queue.pop(0)
        if len(path) > max_hops:
            continue
        for edge in edges.get(cur, ()):
            owner = edge.split(".")[0]
            if owner == dst:
                return path + [edge]
            if owner not in seen:
                seen.add(owner)
                queue.append((owner, path + [edge]))
    return None


def declaration_end(lines: list[str], start: int) -> int:
    """The last line of the declaration beginning at `start` (1-based).

    Brackets, not indentation: a hunk between two declarations belongs to neither,
    and attributing it to the one above would credit a tool for naming a symbol the
    diff never touched.

    `lines` must come from `declarations`, which blanks comments, strings and regex
    literals first. A bracket left inside any of the three is a character rather than a
    nesting, and counting it walks the span off the end of the file.
    """
    depth = 0
    for i in range(start - 1, len(lines)):
        line = lines[i]
        depth += line.count("{") + line.count("[") + line.count("(")
        depth -= line.count("}") + line.count("]") + line.count(")")
        if depth <= 0:
            return i + 1
    return len(lines)


def changed_symbols(repo: Path, base: str) -> dict:
    """Code symbols the diff since `base` touches, by the declaration each hunk sits in.

    Two axes, counting two different things. `code_files` is every code file the diff
    touched — what a tool asked "what changed" should be able to name — and does not depend
    on the reader below understanding the file. `symbols` holds only the files the reader
    did read a declaration out of, because a file it could not read contributes nothing to
    the symbol truth and pretending otherwise would ask for a name nobody derived.

    A file the diff only deleted is in neither: `git diff -U0` gives it no new-side lines,
    so there is nothing here to read a declaration or a span from.
    """
    diff = subprocess.run(
        ["git", "diff", "-U0", "--no-color", "--no-ext-diff", base, "--", "."],
        cwd=repo, capture_output=True, text=True,
    ).stdout
    hunks: dict[str, list[tuple[int, int]]] = defaultdict(list)
    current = None
    for line in diff.split("\n"):
        if line.startswith("+++ b/"):
            current = line[6:]
        elif line.startswith("@@") and current:
            m = re.match(r"@@ -\S+ \+(\d+)(?:,(\d+))? @@", line)
            if m:
                start = int(m.group(1))
                count = int(m.group(2) or 1)
                if count:
                    hunks[current].append((start, start + count - 1))
    symbols: dict[str, list[str]] = {}
    code_files: list[str] = []
    for rel, spans in hunks.items():
        if Path(rel).suffix not in DECLARATIONS:
            continue
        path = repo / rel
        if not path.exists():
            continue
        code_files.append(rel)
        lines, starts = declarations(rel, path.read_text(encoding="utf8", errors="replace"))
        hit = set()
        end_of = DECLARATION_ENDS.get(Path(rel).suffix, declaration_end)
        for start, name in starts:
            end = end_of(lines, start)
            if any(lo <= end and hi >= start for lo, hi in spans):
                hit.add(name)
        if hit:
            symbols[rel] = sorted(hit)
    return {"files": sorted(hunks), "code_files": sorted(code_files), "symbols": symbols}


# .NET libraries and the Kotlin app live beside `apps` and `packages` in this monorepo.
DI_ROOTS = ("apps", "packages", "libs-dotnet", "mobile")


def build(repo: Path, cases: list[dict], blast: list[dict], roots: list[str] | None = None) -> dict:
    """The ground truth for every case, read from `repo` by pattern rather than by any tool.

    `roots` names where the injected-call reader looks; the default is this repository's corpus
    layout, and a corpus kept elsewhere passes its own.
    """
    truth: dict = {"retrieval": {}, "impact": {}, "trace": {}, "changes": {}}
    for case in cases:
        want = case["expect"]
        # A code case names the file itself; a requirement case names an id that
        # some file spells.
        truth["retrieval"][want] = [want] if (repo / want).is_file() else files_naming(repo, want)
    # A root the tree lacks is dropped rather than handed to ripgrep, whose complaint would say
    # nothing about the truth.
    graph = di_call_graph(repo, [r for r in (DI_ROOTS if roots is None else roots) if (repo / r).is_dir()])
    for case in blast:
        if case["kind"] == "impact":
            name = case["target"]
            decl = case.get("file") or declaration_of(repo, name)
            refs = [f for f in code_files_naming(repo, name) if f != decl]
            # A case about one language's declaration counts that language's files: a column name a
            # TypeScript field also spells would otherwise credit files the language under test never reaches.
            if case.get("exts"):
                refs = [f for f in refs if Path(f).suffix in case["exts"]]
            truth["impact"][name] = {"declared_in": decl, "refs": refs}
        elif case["kind"] == "trace":
            key = f"{case['from']}->{case['to']}"
            truth["trace"][key] = shortest_path(graph, case["from"], case["to"])
        elif case["kind"] == "changes":
            truth["changes"][case["base"]] = changed_symbols(repo, case["base"])
    truth["di_edges"] = sum(len(v) for v in graph["edges"].values())
    return truth


def read_jsonl(path: Path) -> list[dict]:
    return [json.loads(line) for line in path.read_text(encoding="utf8").split("\n") if line.strip()]


if __name__ == "__main__":
    import argparse

    ap = argparse.ArgumentParser()
    ap.add_argument("--repo", required=True)
    ap.add_argument("--bench", default=str(Path(__file__).resolve().parent.parent))
    ap.add_argument("--out", default="")
    args = ap.parse_args()
    bench = Path(args.bench)
    result = build(
        Path(args.repo).resolve(),
        read_jsonl(bench / "cases.jsonl"),
        read_jsonl(bench / "blast.jsonl"),
    )
    text = json.dumps(result, ensure_ascii=False, indent=1)
    if args.out:
        Path(args.out).write_text(text, encoding="utf8")
        print(f"truth written: {args.out}")
    else:
        print(text)
