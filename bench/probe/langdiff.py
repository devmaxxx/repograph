#!/usr/bin/env python3
"""What a port changed in two stores, sorted into the plan's regression classes per language.

A port of language L must leave every other language's files exactly as they were, keep L's nodes,
spans, declarations and imports, and may move only the name-resolved edge kinds of L, within a bound
read by hand. `graphdiff.py` answers "the same or not"; this answers "which class moved, in which
language", which is what the step's gate reads (plan §5.3).

Each node belongs to the language of its file's extension, and each edge to the language of its
source node's file, so an edge from a TypeScript file into a Vue component is TypeScript's.

usage: langdiff.py [--lang ts,tsx] [--sample N --seed S] OLD_STORE NEW_STORE
"""

import argparse
import collections
import json
import os
import random
import sys

SPAN = ("kind", "label", "file", "line", "end", "files")
STRUCTURE = {"Declares", "Imports", "ReExports"}
BOUNDED = ("Calls", "References", "Extends", "Implements", "DecoratedBy")
CLASSES = ("R1", "R2", "R3", "R4", "R5", "R6")


def lang_of(path):
    base = os.path.basename(path or "")
    return base.rsplit(".", 1)[1] if "." in base.strip(".") else "(none)"


def last_line(body):
    lines = (body or "").rstrip("\n").split("\n")
    return lines[-1]


def edge_key(e):
    return (e["source"], e["target"], e["kind"], e.get("context", ""), e["file"])


def load(store):
    with open(os.path.join(store, "graph.json")) as f:
        g = json.load(f)
    headers = os.path.join(store, "headers.json")
    raw = open(headers, "rb").read() if os.path.exists(headers) else b""
    return g, raw


def diff(old_store, new_store, langs=None):
    """Counts and edge lists, by language. `langs` is the ported set; `None` reads every language as ported."""
    (a, ha), (b, hb) = load(old_store), load(new_store)
    na, nb = a["nodes"], b["nodes"]

    def owner(nid, e=None):
        n = nb.get(nid) or na.get(nid)
        return lang_of(n["file"] if n else e["file"] if e else "")

    counts = collections.defaultdict(collections.Counter)
    accepted = collections.Counter()

    def count(lang, cls, n=1):
        # Another language's change is R6 whatever its kind: a port touches its own files only.
        ported = langs is None or lang in langs
        counts[lang][cls if ported else "R6"] += n

    for nid in set(na) ^ set(nb):
        count(owner(nid), "R1")
    for nid in set(na) & set(nb):
        x, y = na[nid], nb[nid]
        lang = owner(nid)
        if any(x.get(k) != y.get(k) for k in SPAN):
            count(lang, "R2")
        if last_line(x.get("body")) != last_line(y.get("body")):
            count(lang, "R3")
        elif x.get("body") != y.get("body"):
            if langs is None or lang in langs:
                accepted[lang] += 1
            else:
                count(lang, "R6")

    ea = collections.Counter(edge_key(e) for e in a["edges"])
    eb = collections.Counter(edge_key(e) for e in b["edges"])
    added = collections.defaultdict(list)
    missed = collections.defaultdict(list)
    for side, delta in (("added", eb - ea), ("missed", ea - eb)):
        for key in sorted(delta.elements()):
            source, _, kind, _, file = key
            lang = owner(source, {"file": file})
            # A kind in neither list (`Links`, `Legacy`) is as much a regression as a lost declaration.
            if kind in STRUCTURE or kind not in BOUNDED or not (langs is None or lang in langs):
                count(lang, "R4")
            else:
                (added if side == "added" else missed)[(lang, kind)].append(key)

    def dangling(g):
        return collections.Counter(edge_key(e) for e in g["edges"] if e["target"] not in g["nodes"])

    for key in (dangling(b) - dangling(a)).elements():
        count(owner(key[0], {"file": key[4]}), "R5")

    store = {
        "R7": int(ha != hb),
        "R8": int(len(a.get("pending", [])) != len(b.get("pending", []))),
        "R9": sum(1 for nid in set(na) & set(nb) if na[nid].get("community") != nb[nid].get("community")) if ea == eb else 0,
    }
    regressions = sum(sum(c.values()) for c in counts.values()) + sum(store.values())
    return {
        "counts": {lang: dict(c) for lang, c in counts.items()},
        "store": store,
        "accepted": dict(accepted),
        "added": {k: v for k, v in added.items()},
        "missed": {k: v for k, v in missed.items()},
        "regressions": regressions,
    }


def show(key):
    source, target, kind, context, _ = key
    return f"{source} -> {target} [{kind}/{context}]"


def main(argv):
    p = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    p.add_argument("--lang", help="the ported language's extensions, comma-separated")
    p.add_argument("--sample", type=int, default=0, help="bounded edge changes to print for the hand read")
    p.add_argument("--seed", type=int, default=0)
    p.add_argument("old")
    p.add_argument("new")
    args = p.parse_args(argv[1:])
    langs = set(args.lang.split(",")) if args.lang else None
    r = diff(args.old, args.new, langs)

    print("lang      " + " ".join(f"{c:>5}" for c in CLASSES) + "  body-above")
    for lang in sorted(set(r["counts"]) | set(r["accepted"])):
        c = r["counts"].get(lang, {})
        print(f"{lang:<9} " + " ".join(f"{c.get(k, 0):>5}" for k in CLASSES) + f"  {r['accepted'].get(lang, 0):>10}")
    print("store     " + " ".join(f"{k}={v}" for k, v in r["store"].items()))

    changed = []
    for kind in BOUNDED:
        for (lang, k), keys in sorted(r["added"].items()):
            if k == kind:
                print(f"{lang} {kind}: +{len(keys)}")
                changed += [("+", key) for key in keys]
        for (lang, k), keys in sorted(r["missed"].items()):
            if k == kind:
                print(f"{lang} {kind}: -{len(keys)}")
                changed += [("-", key) for key in keys]
    if args.sample:
        for sign, key in random.Random(args.seed).sample(changed, min(args.sample, len(changed))):
            print(f"  {sign} {show(key)}")
    print(f"regressions: {r['regressions']}")
    return 0 if r["regressions"] == 0 else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv))
