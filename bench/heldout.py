#!/usr/bin/env python3
"""The held-out question set that retrieval changes are judged on, and the test that judges them.

The 82 recorded cases in `cases.jsonl` are a smoke test: they are few, they were written by
hand, and a change tuned until they pass is a change fitted to them. ADR-001 exists because
that happened once already. The set built here is the other half — four hundred questions the
store generated itself, one per node, scored leave-one-out so a question cannot retrieve itself.

Order of operations for any retrieval change, and it is not optional:

    bench/heldout.py build --store <repo>/.repograph --out heldout.jsonl
    repograph --repo <repo> dump --queries heldout.jsonl --out before.json    # on the old binary
    repograph --repo <repo> dump --queries heldout.jsonl --out after.json     # on the new one
    bench/heldout.py compare before.json after.json

Both arms, not one: repeat the two dumps with `--no-dense` and compare those too. `compare`
refuses to pair a dump from one arm with a dump from the other. When the "old" behaviour is a
value of a constant rather than an older binary, `REPOGRAPH_QUESTIONS_GATE=<value>` on the dump
command re-reads it from the same binary — `0` is the ungated fusion that ADR-001 Amendment 6
measured against, which no commit's binary otherwise produces.

Decide from `compare`, then run `bench` as the smoke test. Reading the 82 first and the four
hundred afterwards is how a change gets fitted to the smoke test without anyone intending it.

The comparison is a paired exact McNemar test: every question is answered by both binaries, so
what matters is not the two totals but the questions that changed answer, and in which
direction. A lever that gains six and loses six has done nothing, and two totals differing by
one point can hide thirty of each.
"""

import argparse
import json
import math
import random
from pathlib import Path

# Any set drawn from a random sample has to be reproducible from the seed alone, or a later
# run measures a different set and calls the difference a result.
SEED = 20260905
SIZE = 400
AT = 5


def cmd_build(args):
    entries = json.loads((Path(args.store) / "questions.json").read_text())["entries"]
    nodes = sorted(k for k, v in entries.items() if v.get("questions"))
    if len(nodes) < args.size:
        raise SystemExit(f"store has {len(nodes)} enriched nodes, need {args.size}")
    rng = random.Random(args.seed)
    out = []
    for node in rng.sample(nodes, args.size):
        # `synthetic` is the kind `dump` applies leave-one-out to. Any other kind leaves the
        # question's own text in the index, and recall@5 then measures the index finding its
        # own sentence: 0.955 on this corpus, against 0.28 once the text is actually held out.
        out.append({"kind": "synthetic", "q": rng.choice(entries[node]["questions"]), "expect": node})
    Path(args.out).write_text("".join(json.dumps(r, ensure_ascii=False) + "\n" for r in out))
    print(f"{args.size} questions from {len(nodes)} enriched nodes, seed {args.seed} -> {args.out}")


def anchors(expect):
    """`dump` writes `expect` in the shape the case file gave it: a bare string for one anchor,
    a list for several. The developer suite anchors half its questions on more than one place."""
    return [expect] if isinstance(expect, str) else list(expect)


def hits(dump, at):
    """Whether a question's own anchors came back in the first `at` seeds, keyed by question."""
    out = {}
    for q in json.loads(Path(dump).read_text())["queries"]:
        seeds = [i for i, _ in q["ask"]["seeds"][:at]]
        # Any anchor, not all of them: a multi-anchor case names the places that would answer
        # it, and finding one of them is the case answered. The key is a tuple because a list
        # cannot be one, and the pairing is per question.
        a = tuple(anchors(q["expect"]))
        out[(q["q"], a)] = any(x in seeds for x in a)
    return out


def mcnemar(before, after):
    """Two-sided exact McNemar over the questions that changed answer."""
    lost = sum(1 for k, v in before.items() if v and not after[k])
    gained = sum(1 for k, v in before.items() if not v and after[k])
    n = lost + gained
    if n == 0:
        return lost, gained, 1.0
    tail = sum(math.comb(n, i) for i in range(min(lost, gained) + 1)) / 2 ** n
    return lost, gained, min(1.0, 2 * tail)


def arm(path):
    """Which retrieval arm a dump was written in, or None for a dump older than the field."""
    return json.loads(Path(path).read_text()).get("meta", {}).get("dense")


def cmd_compare(args):
    arms = arm(args.before), arm(args.after)
    if None not in arms and arms[0] != arms[1]:
        # The embeddings arm and the lexical-only arm answer differently by design; pairing
        # one against the other measures the arms, not the change.
        raise SystemExit(f"the two dumps come from different arms (dense={arms[0]} vs dense={arms[1]})")
    before, after = hits(args.before, args.at), hits(args.after, args.at)
    shared = set(before) & set(after)
    if len(shared) != len(before) or len(shared) != len(after):
        # Two dumps of different question sets are not a paired test, and averaging them anyway
        # would report a difference between the sets as a difference between the binaries.
        raise SystemExit(f"the two dumps do not answer the same questions "
                         f"({len(before)} vs {len(after)}, {len(shared)} shared)")
    before = {k: before[k] for k in shared}
    after = {k: after[k] for k in shared}
    lost, gained, p = mcnemar(before, after)
    n = len(shared)
    print(f"recall@{args.at} over {n} held-out questions")
    print(f"  before {sum(before.values())}/{n} = {sum(before.values()) / n:.3f}")
    print(f"  after  {sum(after.values())}/{n} = {sum(after.values()) / n:.3f}")
    print(f"  changed answer: {gained} gained, {lost} lost")
    print(f"  paired exact McNemar p = {p:.4f} — "
          f"{'significant' if p < args.alpha else 'not significant'} at alpha {args.alpha}")
    if lost and p < args.alpha:
        print("\n  lost:")
        for (q, e) in sorted(k for k in shared if before[k] and not after[k])[:20]:
            # `+` is the separator `bench` prints a multi-anchor case's key with.
            print(f"    {'+'.join(e):14} {q[:90]}")


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = p.add_subparsers(dest="cmd", required=True)

    b = sub.add_parser("build", help="write the held-out question set")
    b.add_argument("--store", required=True, help="a .repograph directory holding questions.json")
    b.add_argument("--out", required=True)
    b.add_argument("--size", type=int, default=SIZE)
    b.add_argument("--seed", type=int, default=SEED)
    b.set_defaults(func=cmd_build)

    c = sub.add_parser("compare", help="paired exact McNemar between two dumps of that set")
    c.add_argument("before")
    c.add_argument("after")
    c.add_argument("--at", type=int, default=AT)
    c.add_argument("--alpha", type=float, default=0.05)
    c.set_defaults(func=cmd_compare)

    args = p.parse_args()
    args.func(args)


if __name__ == "__main__":
    main()
