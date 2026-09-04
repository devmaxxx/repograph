"""Ground truth for the three-graph comparison, derived from the repository itself.

No graph tool is consulted here. Every expectation is read from the source with
ripgrep and a small TypeScript reader, so a tool that disagrees with this file is
wrong about the repository, not about a rival's model.
"""

from __future__ import annotations

import json
import re
import subprocess
from collections import defaultdict
from pathlib import Path

CODE_GLOBS = ("-g", "*.ts", "-g", "*.tsx", "-g", "*.kt")
# The stores of the three tools sit inside the corpus; a hit there is a hit on a
# tool's own index, not on the repository.
EXCLUDE = ("-g", "!.repograph/**", "-g", "!graphify-out/**", "-g", "!.gitnexus/**")

CLASS = re.compile(r"^export (?:abstract )?class (\w+)", re.M)
FIELD = re.compile(r"(?:private|public|protected|readonly)\s+(?:readonly\s+)?(\w+)\s*:\s*(\w+)")
# `this.db.run(` and `this.db\n  .run(` are the same call; tree-sitter sees no
# newline and neither may we, or the truth undercounts what the tools find.
CALL = re.compile(r"this\.(\w+)\s*\.\s*(\w+)\s*\(", re.S)
# A hunk inside a file-private helper still touches a symbol a graph should name,
# so the export keyword is optional here.
TOP_LEVEL = re.compile(
    r"^(?:export\s+(?:default\s+)?)?(?:abstract\s+)?(?:async\s+)?"
    r"(?:class|function|const|let|interface|type|enum)\s+(\w+)",
    re.M,
)


def rg(repo: Path, args: list[str]) -> list[str]:
    # stdin detached: ripgrep searches stdin instead of the tree when stdin is not a tty,
    # which turned every truth list empty under a heredoc and would do the same in CI.
    out = subprocess.run(["rg", *args, *EXCLUDE], cwd=repo, capture_output=True, text=True,
                         stdin=subprocess.DEVNULL)
    return [line for line in out.stdout.split("\n") if line]


def files_naming(repo: Path, token: str) -> list[str]:
    """Every tracked file that spells `token` as a whole word."""
    return sorted(rg(repo, ["-l", "--word-regexp", "--fixed-strings", token]))


BLOCK_COMMENT = re.compile(r"/\*.*?\*/", re.S)
LINE_COMMENT = re.compile(r"(?<!:)//[^\n]*")
# A single- or double-quoted literal excludes a bare newline in its body, not a
# template literal: a `\` line continuation still spans lines, but only via the
# backslash-escape branch, not the character class.
STRING_LITERAL = re.compile(r"'(?:[^'\\\n]|\\.)*'|\"(?:[^\"\\\n]|\\.)*\"|`(?:[^`\\]|\\.)*`", re.S)


def _blank(match: re.Match[str]) -> str:
    # Keep the quotes and the newline count, drop everything else: a multi-line
    # template literal must not pull the lines after it up to where it opened.
    body = match.group(0)
    return body[0] + "\n" * body.count("\n") + body[-1]


def _newlines(match: re.Match[str]) -> str:
    # A docblock is dropped whole but its line count is kept, for the same reason: the
    # declaration under a multi-line comment stays on the line it was written on.
    return "\n" * match.group(0).count("\n")


def strip_comments(src: str) -> str:
    """Prose is not a reference, and neither is a string.

    This corpus writes long docblocks that name the symbols they discuss, so a plain grep
    counts a paragraph about `TenantContextInterceptor` as a file that depends on it. It also
    names symbols in text: `'PinoLogger:OutboxPublisher'` is a logger's name and an error
    message can mention an interceptor. Both counted on 2026-09-03 and put two impact targets
    one file short of 1.0 for a dependency that did not exist. Only code counts. A literal's
    body is blanked and its quotes kept, and a comment leaves its newlines behind, so line
    numbers and bracket depth survive; a `${…}` inside a template literal is blanked with it,
    which is the one thing this loses.
    A regex literal containing a quote character (`/'/`) desyncs the scan for the rest of
    that line: the quote branch opens on it and closes on the next real string's opening
    quote, so a symbol named in that string can survive un-blanked. Closing that properly
    needs a tokeniser, which is out of scope here.

    This serves the impact search only, where the question is boolean and a stray line is a
    stray line. `changed_symbols` needs bracket-exact and line-exact output and uses
    `blank_typescript` instead.
    """
    without_comments = LINE_COMMENT.sub("", BLOCK_COMMENT.sub(_newlines, src))
    return STRING_LITERAL.sub(_blank, without_comments)


# A `/` opens a regular expression only in an operand position. After an identifier, a `)`
# or a `]` the same character divides, so those must not be listed here.
REGEX_OPENS_AFTER = set("=(,:[!&|?{};")
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


def _blank_source(src: str, *, kotlin: bool) -> str:
    r"""Comments and text blanked out of `src`, line for line, in one left-to-right pass.

    One pass rather than `strip_comments`' three substitutions, because `changed_symbols`
    needs two things that a boolean search did not. Line numbers must survive exactly, since
    hunk ranges are mapped onto declaration spans and a lost line shifts every declaration
    under it. And brackets must survive exactly, since `declaration_end` balances them.

    Applying the pattern for comments before the pattern for strings gets both wrong on this
    corpus: `'apps/*'` in packages/ui/test/boundary.test.ts opens a block comment that runs
    until the next `*/` seventy lines below, and `'//'` in packages/config/test/config-boundary.test.ts
    deletes the rest of its line. Both take the closing bracket of a real array with them, and
    both then ran a declaration to the end of the file. A scanner cannot make that mistake:
    whichever of the two opens first wins, which is what the language does.

    The two dialects differ in four ways, all of them load-bearing here. Kotlin nests block
    comments, so `/* a /* b */ c */` closes once. Kotlin has raw `\"\"\"…\"\"\"` strings that hold
    anything at all, including the `/\*` that ModuleBoundaryTest.kt puts in a `Regex`. A
    backtick opens a template literal in TypeScript and quotes an identifier in Kotlin, where
    this corpus names 358 tests that way and one of them carries an apostrophe that would
    otherwise open a character literal. And only TypeScript has a regex literal, which is the
    one place a bracket is a character rather than a nesting.

    What this still loses is a `${…}` interpolation, blanked with the string around it.
    """
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
                if kotlin and src[end : end + 2] == "/*":
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


KOTLIN_MODIFIER = (
    "public|private|internal|protected|open|final|abstract|sealed|data|enum|annotation|"
    "value|inner|expect|actual|override|lateinit|const|external|infix|inline|operator|"
    "suspend|tailrec|companion"
)
KOTLIN_NAME = r"`[^`\n]+`|\w+"
# `fun interface` before `fun`, or the name of a `fun interface Renderer` reads as `interface`.
# The receiver of an extension is consumed and dropped: `fun Row.label()` declares `label`.
# The name is optional so that an unnamed `companion object` still opens a body of members.
KOTLIN_DECL = re.compile(
    rf"^[ \t]*(?:@[\w.]+(?:\([^()\n]*\))?[ \t]*)*"
    rf"(?:(?:{KOTLIN_MODIFIER})[ \t]+)*"
    rf"(?P<kw>fun[ \t]+interface|class|interface|object|fun|val|var|typealias)\b"
    rf"(?:[ \t]*<[^<>\n]*>)?"
    rf"(?:[ \t]+(?:[\w.]+(?:<[^<>\n]*>)?\.)?(?P<name>{KOTLIN_NAME}))?"
)
# The bodies these open hold declarations; every other brace opens a block that holds statements.
KOTLIN_TYPE_KEYWORDS = {"class", "interface", "object", "fun interface"}


def kotlin_declarations(blanked: str) -> list[tuple[int, str]]:
    """(line, name) for every Kotlin declaration in already-blanked source.

    Brace scope, not indentation. `val x = 1` at the head of a function body is a local and
    `val x = 1` in a class body is a property: the two are written identically and only the
    block they sit in tells them apart, so the stack records what each open brace belongs to.
    A constructor parameter is left out — the hunk that touches a type's header touches the
    type, which is already named — which is also where the TypeScript reader draws the line.

    A name in backticks is reported without them: a tool that answers with the backticks still
    contains the bare name, and one that answers without them would otherwise be marked wrong.
    """
    found: list[tuple[int, str]] = []
    holds_declarations: list[bool] = []
    parens = 0
    pending: bool | None = None
    for index, line in enumerate(blanked.split("\n")):
        if parens == 0 and (not holds_declarations or holds_declarations[-1]):
            match = KOTLIN_DECL.match(line)
            if match:
                pending = " ".join(match.group("kw").split()) in KOTLIN_TYPE_KEYWORDS
                if match.group("name"):
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
    return found


def declarations(rel: str, src: str) -> tuple[list[str], list[tuple[int, str]]]:
    """The blanked lines of one file and the (line, name) of every declaration in it."""
    if rel.endswith(".kt"):
        blanked = blank_kotlin(src)
        return blanked.split("\n"), kotlin_declarations(blanked)
    blanked = blank_typescript(src)
    starts = [(blanked[: m.start()].count("\n") + 1, m.group(1)) for m in TOP_LEVEL.finditer(blanked)]
    return blanked.split("\n"), starts


def code_files_naming(repo: Path, token: str) -> list[str]:
    word = re.compile(rf"\b{re.escape(token)}\b")
    hits = rg(repo, ["-l", "--word-regexp", "--fixed-strings", token, *CODE_GLOBS])
    return sorted(f for f in hits if word.search(strip_comments((repo / f).read_text(encoding="utf8", errors="replace"))))


def declaration_of(repo: Path, name: str) -> str | None:
    for line in rg(repo, ["-l", f"^export (?:abstract )?class {name}\\b", *CODE_GLOBS]):
        return line
    for line in rg(repo, ["-l", f"^export .*\\b{name}\\b", *CODE_GLOBS]):
        return line
    return None


def di_call_graph(repo: Path, roots: list[str]) -> dict:
    """Class → the `Type.method` calls its members make through injected fields.

    Covers the one pattern that carries a NestJS API: a constructor parameter
    property or a typed field, called as `this.field.method()`.
    """
    files = rg(repo, ["--files", *CODE_GLOBS, *roots])
    edges: dict[str, set[str]] = defaultdict(set)
    declared: dict[str, str] = {}
    for rel in files:
        src = (repo / rel).read_text(encoding="utf8", errors="replace")
        marks = [(m.start(), m.group(1)) for m in CLASS.finditer(src)] + [(len(src), None)]
        for i in range(len(marks) - 1):
            start, name = marks[i]
            body = src[start : marks[i + 1][0]]
            declared[name] = rel
            fields = {m.group(1): m.group(2) for m in FIELD.finditer(body)}
            for m in CALL.finditer(body):
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
        if not rel.endswith((".ts", ".tsx", ".kt")):
            continue
        path = repo / rel
        if not path.exists():
            continue
        code_files.append(rel)
        lines, starts = declarations(rel, path.read_text(encoding="utf8", errors="replace"))
        hit = set()
        for start, name in starts:
            end = declaration_end(lines, start)
            if any(lo <= end and hi >= start for lo, hi in spans):
                hit.add(name)
        if hit:
            symbols[rel] = sorted(hit)
    return {"files": sorted(hunks), "code_files": sorted(code_files), "symbols": symbols}


def build(repo: Path, cases: list[dict], blast: list[dict]) -> dict:
    truth: dict = {"retrieval": {}, "impact": {}, "trace": {}, "changes": {}}
    for case in cases:
        want = case["expect"]
        # A code case names the file itself; a requirement case names an id that
        # some file spells.
        truth["retrieval"][want] = [want] if (repo / want).is_file() else files_naming(repo, want)
    graph = di_call_graph(repo, ["apps", "packages"])
    for case in blast:
        if case["kind"] == "impact":
            name = case["target"]
            decl = case.get("file") or declaration_of(repo, name)
            refs = [f for f in code_files_naming(repo, name) if f != decl]
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
