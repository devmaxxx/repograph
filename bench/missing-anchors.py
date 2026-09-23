#!/usr/bin/env python3
"""List the `expect` anchors of a case file that a checkout does not have.

`bench` refuses a run whose cases name anything the built graph does not hold, and that refusal
costs a build to reach. This answers the same question against a plain checkout, in a second and
without a graph: point it at the corpus and it prints every path anchor that is not a file there.

    bench/missing-anchors.py ~/bench/beauty-crm-502e8a6d [--cases bench/cases.jsonl]

Only path anchors are checked. An anchor without a `/` is a node id, which exists in the graph and
not in the tree, so the script counts those and says how many it skipped rather than guessing.
"""

import argparse
import json
import pathlib
import sys


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("checkout", type=pathlib.Path)
    ap.add_argument("--cases", type=pathlib.Path, default=pathlib.Path(__file__).parent / "cases.jsonl")
    args = ap.parse_args()

    missing, ids, paths = [], 0, 0
    for n, line in enumerate(args.cases.read_text().splitlines(), 1):
        if not line.strip():
            continue
        case = json.loads(line)
        expect = case["expect"]
        for anchor in [expect] if isinstance(expect, str) else expect:
            if "/" not in anchor:
                ids += 1
                continue
            paths += 1
            if not (args.checkout / anchor).exists():
                missing.append((n, case["q"], anchor))

    for n, q, anchor in missing:
        print(f"{args.cases}:{n}: {q!r} expects {anchor!r}, which {args.checkout} does not have")
    print(f"{len(missing)}/{paths} path anchors missing, {ids} id anchors not checked")
    return 1 if missing else 0


if __name__ == "__main__":
    sys.exit(main())
