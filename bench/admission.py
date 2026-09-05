#!/usr/bin/env python3
"""Replays the plain `ask` fusion over `dump` files under a named admission rule, so a candidate
rule is read on the held-out set, the recorded suite and the developer suite from the lists the
binary already recorded — no rebuild, no re-run, no tokens — and the binary is changed once, for
the rule that passed.

The replay is checked before it is trusted: `check` re-derives every dump's own recorded answer
under a rule and refuses to go on unless it matches on every query. Under the shipped rule
(`ratio 0.85`) that is the proof the replay is `query::ask`; under a new rule, on dumps from a
binary carrying it, it is the proof the binary is the replay.

Forms, each a statistic of one list alone, so two lists compare in one unit:

    ratio     best(Q) / best(P)                              the shipped gate — a constant of one store (G8, G12)
    coverage  (best/attainable)(Q) / (best/attainable)(P)    how much of the query each list's best answered
    peak      (best/fifth)(Q) / (best/fifth)(P)              how far each best stands above its own list; five seats
    z         z(Q) / z(P)                                    the best as a z-score over the list, 20 deep

Order of operations, as `heldout.py`'s header says and not optionally: a form's constant is the
crossover on the held-out dumps (`crossover`), the recorded and developer suites are read after
it is fixed (`score`), and the first form in the pre-registered order that passes the rule is
the one the binary gets. Nothing here is adjusted after a suite has been read.
"""
import argparse
import json
import math
import statistics
import struct
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from heldout import anchors, mcnemar  # noqa: E402

# `query::ask` expands along these kinds and no other.
EXPAND = {"References", "Implements", "Declares", "Links", "Legacy"}
# The plain path searches every list this deep; a dump records 300.
PLAIN_DEPTH = 20
SEATS = 5
FORMS = ("ratio", "coverage", "peak", "z")


def f32(x):
    """The value as the binary holds it: `lexical_lists` compares f32 products."""
    return struct.unpack("f", struct.pack("f", x))[0]


def load(path):
    return json.loads(Path(path).read_text())


def store_of(path):
    """Neighbours along the expansion kinds, each node's file, and whether the store has
    generated questions at all — a store without them fuses the passage list alone."""
    g = load(Path(path) / "graph.json")
    nb = {}
    for e in g["edges"]:
        if e["kind"] in EXPAND:
            nb.setdefault(e["source"], []).append(e["target"])
            nb.setdefault(e["target"], []).append(e["source"])
    files = {i: n["file"] for i, n in g["nodes"].items()}
    q = Path(path) / "questions.json"
    has_questions = bool(load(q)["entries"]) if q.exists() else False
    return nb, files, has_questions


def ids(scored):
    return [i for i, _ in scored]


def interleave(lists):
    """`fuse::interleave`: rank r of every list before rank r + 1 of any, first position kept."""
    out, seen = [], set()
    for rank in range(max((len(l) for l in lists), default=0)):
        for l in lists:
            if rank < len(l) and l[rank] not in seen:
                seen.add(l[rank])
                out.append((l[rank], 1.0 / (rank + 1)))
    return out


def stat(form, scored, attainable):
    scored = scored[:PLAIN_DEPTH]
    if not scored:
        return 0.0
    best = scored[0][1]
    if form == "ratio":
        return best
    if form == "coverage":
        return best / attainable if attainable > 0 else 0.0
    if form == "peak":
        if len(scored) < SEATS or scored[SEATS - 1][1] <= 0:
            return math.inf
        return best / scored[SEATS - 1][1]
    if form == "z":
        s = [x for _, x in scored]
        if len(s) < 2:
            return math.inf
        sd = statistics.pstdev(s)
        return (best - statistics.fmean(s)) / sd if sd > 0 else math.inf
    raise SystemExit(f"unknown form {form}")


def sratio(form, own, theirs):
    """The list's statistic over the passage list's, with the ends spelled out: a passage list
    with nothing is never a bar (inf), two lists both peaks of their own compare level (1)."""
    if own == math.inf and theirs == math.inf:
        return 1.0
    if own == math.inf:
        return math.inf
    if theirs == math.inf:
        return 0.0
    return math.inf if theirs <= 0 else own / theirs


def admitted(form, c, lst, passages, a_list, a_passages):
    """Whether `lst` is seated beside `passages` under `form` at constant `c`."""
    lst, passages = lst[:PLAIN_DEPTH], passages[:PLAIN_DEPTH]
    if not lst or lst[0][1] <= 0:
        return False
    if form == "ratio":
        theirs = passages[0][1] if passages else 0.0
        return f32(lst[0][1]) >= f32(f32(c) * f32(theirs))
    return sratio(form, stat(form, lst, a_list), stat(form, passages, a_passages)) >= c


def lexical_lists(rec, form, c, code_seat, has_questions):
    """`query::lexical_lists` on the plain path: [questions?] [passages] [code top-1?]."""
    p, q, code = rec["bm25_passages"], rec["bm25_questions"], rec["bm25_code"]
    if not has_questions:
        return [ids(p[:PLAIN_DEPTH])]
    lists = []
    if admitted(form, c, q, p, rec["attainable_questions"], rec["attainable_passages"]):
        lists.append(ids(q[:PLAIN_DEPTH]))
    lists.append(ids(p[:PLAIN_DEPTH]))
    if code_seat and admitted(form, c, code, p, rec["attainable_code"], rec["attainable_passages"]):
        lists.append(ids(code[:1]))
    return lists


def all_lists(rec, dense, form, c, code_seat, has_questions):
    lists = []
    if dense:
        lists.append(ids(rec["dense_passages"][:PLAIN_DEPTH]))
    lists.extend(lexical_lists(rec, form, c, code_seat, has_questions))
    return [l for l in lists if l]


def answer(rec, lists, nb, seats=SEATS, files=None):
    """`query::ask` from the lists on: exact seeds first, fused seats, one expanded neighbour."""
    exact = rec["exact"]["ids"]
    seeds = [(i, 1.0) for i in exact[:seats]]
    ranked = {}
    if not rec["exact"]["whole_question"]:
        for pos, (i, score) in enumerate(interleave(lists)):
            if len(seeds) < seats and i not in exact:
                seeds.append((i, score))
            ranked.setdefault(i, pos)
    seed_ids = [i for i, _ in seeds]
    cands = []
    for sid, sscore in seeds:
        for other in nb.get(sid, []):
            if other in seed_ids or other.startswith("file:") or other.startswith("deco:"):
                continue
            if any(cnd[2] == other for cnd in cands):
                continue
            cands.append((ranked.get(other), sscore * 0.5, other, sid))
    # Ranked before unranked, by rank, then by the seed's score falling, then by id.
    cands.sort(key=lambda cnd: (cnd[0] is None, cnd[0] if cnd[0] is not None else 0, -cnd[1], cnd[2]))
    # `query::ask` takes the top MAX_EXPANDED candidates and only then resolves each to a node
    # (`hit`); an unresolved id (an edge target with no node of its own, e.g. a prose-only
    # acceptance-criteria reference) drops out rather than yielding its seat to the next-ranked
    # candidate, so the filter has to run after the slice, not before it.
    top = cands[:1]
    if files is not None:
        top = [c for c in top if c[2] in files]
    return seed_ids, [cnd[2] for cnd in top]


def found(rec, seeds, expanded, files):
    """`bench::found`: an id anchor counts anywhere, a path anchor only among the seeds."""
    reached = 0
    for anchor in anchors(rec["expect"]):
        if anchor in files:
            reached += anchor in seeds or anchor in expanded
        else:
            reached += any(files.get(s) == anchor for s in seeds)
    return reached


def replay(dump, store, form, c, code_seat):
    nb, files, has_questions = store
    out = []
    for rec in dump["queries"]:
        lists = all_lists(rec, dump["meta"]["dense"], form, c, code_seat, has_questions)
        seeds, expanded = answer(rec, lists, nb, files=files)
        out.append((rec, seeds, expanded))
    return out, files


def cmd_check(args):
    store = store_of(args.store)
    bad = 0
    for path in args.dumps:
        dump = load(path)
        rows, _ = replay(dump, store, args.form, args.c, args.code_seat)
        diffs = [(r["q"], s, e) for r, s, e in rows
                 if s != ids(r["ask"]["seeds"]) or e != [i for i, _, _ in r["ask"]["expanded"]]]
        bad += len(diffs)
        print(f"{Path(path).name}: {len(rows) - len(diffs)}/{len(rows)} replicated")
        for q, s, e in diffs[:5]:
            print(f"  differs: {q[:80]}\n    replay {s} + {e}")
    sys.exit(1 if bad else 0)


def cmd_crossover(args):
    """The constant: the split of the held-out questions by the form's statistic that puts the
    most of them on the side of the list holding their answer in its top five."""
    dump = load(args.heldout)
    if dump["meta"]["dense"] and not args.allow_dense:
        raise SystemExit("the constant is derived in the --no-dense arm; pass --allow-dense to read the other for information")
    rows = []
    for rec in dump["queries"]:
        p, q = rec["bm25_passages"][:PLAIN_DEPTH], rec["bm25_questions"][:PLAIN_DEPTH]
        if not q or q[0][1] <= 0:
            continue
        s = sratio(args.form, stat(args.form, q, rec["attainable_questions"]), stat(args.form, p, rec["attainable_passages"]))
        a = set(anchors(rec["expect"]))
        rows.append((s, bool(a & set(ids(q[:SEATS]))), bool(a & set(ids(p[:SEATS])))))
    finite = sorted({s for s, _, _ in rows if math.isfinite(s)})
    if not finite:
        raise SystemExit("every statistic is at an end: no crossover to derive")
    cuts = [finite[0] - 1] + [(x + y) / 2 for x, y in zip(finite, finite[1:])] + [finite[-1] + 1]

    def right(t):
        return sum((s >= t and qh and not ph) or (s < t and ph and not qh) for s, qh, ph in rows)

    best = max(cuts, key=lambda t: (right(t), -t))
    above = [(qh, ph) for s, qh, ph in rows if s >= best]
    below = [(qh, ph) for s, qh, ph in rows if s < best]
    pct = lambda xs, k: f"{100 * sum(x[k] for x in xs) / len(xs):.0f}%" if xs else "—"
    print(f"form={args.form} c={best:.3f}  ({len(rows)} questions with a questions list, {len(finite)} distinct finite values)")
    print(f"  above c: {len(above)} questions — questions list holds the answer in top five {pct(above, 0)}, passage list {pct(above, 1)}")
    print(f"  below c: {len(below)} questions — questions list {pct(below, 0)}, passage list {pct(below, 1)}")
    print(f"  questions on the side of the list that holds their answer: {right(best)}")


def score_suite(path, store, form, c, code_seat):
    dump = load(path)
    rows, files = replay(dump, store, form, c, code_seat)
    by_kind = {}
    for rec, seeds, expanded in rows:
        k = by_kind.setdefault(rec["kind"], [0, 0])
        k[1] += 1
        k[0] += found(rec, seeds, expanded, files) >= 1
    return by_kind


def cmd_score(args):
    store = store_of(args.store)
    for label, path in (("recorded", args.rec), ("developer", args.dev)):
        for p in path or []:
            k = score_suite(p, store, args.form, args.c, args.code_seat)
            counts = "  ".join(f"{kind} {h}/{n}" for kind, (h, n) in k.items())
            total = sum(h for h, _ in k.values())
            print(f"{label} {Path(p).name}: {counts}  total {total}")
    for p in args.ho or []:
        dump = load(p)
        rows, _ = replay(dump, store, args.form, args.c, args.code_seat)
        before, after = {}, {}
        for rec, seeds, _ in rows:
            key = (rec["q"], tuple(anchors(rec["expect"])))
            a = set(key[1])
            before[key] = bool(a & set(ids(rec["ask"]["seeds"][:SEATS])))
            after[key] = bool(a & set(seeds[:SEATS]))
        lost, gained, pv = mcnemar(before, after)
        n = len(before)
        print(f"held-out {Path(p).name}: recorded {sum(before.values())}/{n} → replay {sum(after.values())}/{n}  "
              f"{gained} gained, {lost} lost, exact McNemar p = {pv:.4f}")


def cmd_g13(args):
    """The G13 table under every form: where each `where`/`cross` file anchor sits in the code
    list and what each form's statistic says about seating that list."""
    dump = load(args.dump)
    _, files, _ = store_of(args.store)
    print("| kind | file anchor | code-list rank | " + " | ".join(FORMS) + " |")
    print("|---|---|---:|" + "---:|" * len(FORMS))
    for rec in dump["queries"]:
        if rec["kind"] not in ("where", "cross"):
            continue
        code, p = rec["bm25_code"], rec["bm25_passages"]
        for anchor in anchors(rec["expect"]):
            if anchor in files:
                continue
            rank = next((i + 1 for i, (cid, _) in enumerate(code) if files.get(cid) == anchor), None)
            if rank is None:
                continue
            cells = []
            for form in FORMS:
                s = sratio(form, stat(form, code, rec["attainable_code"]), stat(form, p, rec["attainable_passages"]))
                cells.append("∞" if s == math.inf else f"{s:.2f}")
            print(f"| {rec['kind']} | {anchor} | {rank} | " + " | ".join(cells) + " |")


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    common = argparse.ArgumentParser(add_help=False)
    common.add_argument("--store", required=True, help="the store the dumps were taken on (…/.repograph)")
    common.add_argument("--form", choices=FORMS, default="ratio")
    common.add_argument("--c", type=float, default=0.85, help="the form's constant")
    common.add_argument("--code-seat", action="store_true", help="admit the code list, one seat, under the same form")
    p = sub.add_parser("check", parents=[common], help="every dump's recorded answer re-derived under the rule")
    p.add_argument("dumps", nargs="+")
    p.set_defaults(fn=cmd_check)
    p = sub.add_parser("crossover", help="derive a form's constant from a held-out dump")
    p.add_argument("--heldout", required=True)
    p.add_argument("--form", choices=FORMS, required=True)
    p.add_argument("--allow-dense", action="store_true")
    p.set_defaults(fn=cmd_crossover)
    p = sub.add_parser("score", parents=[common], help="hits per kind on suites, recall and McNemar on held-out")
    p.add_argument("--rec", nargs="*")
    p.add_argument("--dev", nargs="*")
    p.add_argument("--ho", nargs="*")
    p.set_defaults(fn=cmd_score)
    p = sub.add_parser("g13", help="the G13 table from a developer-suite dump")
    p.add_argument("--dump", required=True)
    p.add_argument("--store", required=True)
    p.set_defaults(fn=cmd_g13)
    args = ap.parse_args()
    args.fn(args)


if __name__ == "__main__":
    main()
