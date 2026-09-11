#!/usr/bin/env python3
"""Are two `graph.json` files the same visible graph, and how much does one hold aside.

Written for G39's gate — an extractor rewritten to read ids by one generic grammar has to write
the graph every reader sees byte-for-byte as before, and the citations it now keeps aside are the
whole of the growth — but general: `nodes` and `edges` are compared field by field, `pending` is
counted, and the sizes are reported as a ratio.
"""

import collections
import json
import os
import sys


def edge_key(e):
    return f"{e['source']} -> {e['target']} [{e['kind']}/{e.get('context', '')}] {e['file']}"


def diff(a_path, b_path):
    with open(a_path) as f:
        a = json.load(f)
    with open(b_path) as f:
        b = json.load(f)
    # Counted, not a set: two identical edges are two edges to every reader, and a set would
    # report `edges: same` over a list that grew a duplicate.
    ea = collections.Counter(edge_key(e) for e in a["edges"])
    eb = collections.Counter(edge_key(e) for e in b["edges"])
    changed = []
    for nid in sorted(set(a["nodes"]) & set(b["nodes"])):
        na, nb = a["nodes"][nid], b["nodes"][nid]
        for k in sorted(set(na) | set(nb)):
            if na.get(k) != nb.get(k):
                changed.append((nid, k, na.get(k), nb.get(k)))
    return {
        "nodes_same": set(a["nodes"]) == set(b["nodes"]) and not changed,
        "nodes_only_a": sorted(set(a["nodes"]) - set(b["nodes"])),
        "nodes_only_b": sorted(set(b["nodes"]) - set(a["nodes"])),
        "nodes_changed": changed,
        "edges_same": ea == eb,
        "edges_only_a": sorted((ea - eb).elements()),
        "edges_only_b": sorted((eb - ea).elements()),
        "pending": len(b.get("pending", [])),
        "pending_a": len(a.get("pending", [])),
        "bytes_ratio": os.path.getsize(b_path) / os.path.getsize(a_path),
        "counts": (len(a["nodes"]), len(a["edges"]), len(b["nodes"]), len(b["edges"])),
    }


def main(argv):
    if len(argv) != 3:
        raise SystemExit("usage: graphdiff.py A.json B.json")
    r = diff(argv[1], argv[2])
    na, ea, nb, eb = r["counts"]
    print(f"nodes: {'same' if r['nodes_same'] else 'differ'} ({na} vs {nb}; only in A {len(r['nodes_only_a'])}, only in B {len(r['nodes_only_b'])}, changed {len(r['nodes_changed'])})")
    for nid, k, va, vb in r["nodes_changed"][:20]:
        print(f"  {nid}.{k}: {va!r} -> {vb!r}")
    print(f"edges: {'same' if r['edges_same'] else 'differ'} ({ea} vs {eb}; only in A {len(r['edges_only_a'])}, only in B {len(r['edges_only_b'])})")
    # Which side holds an edge is the whole of what a listing of the differences says — a rewritten
    # extractor that dropped ten edges and one that invented ten print the same twenty lines
    # otherwise — so each carries its side's sign rather than being concatenated into one block.
    for e in r["edges_only_a"][:10]:
        print(f"  -A {e}")
    for e in r["edges_only_b"][:10]:
        print(f"  +B {e}")
    print(f"pending: {r['pending']} (A held {r['pending_a']})")
    print(f"bytes: {os.path.getsize(argv[1])} {os.path.getsize(argv[2])} ratio {r['bytes_ratio']:.3f}")
    return 0 if r["nodes_same"] and r["edges_same"] else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv))
