"""Turn a result file from `run.py` into the table that goes in a write-up."""

from __future__ import annotations

import argparse
import json
from pathlib import Path

ORDER = ["repograph", "graphify", "gitnexus"]


def cell(value) -> str:
    return "—" if value is None else str(value)


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("result")
    args = ap.parse_args()
    report = json.loads(Path(args.result).read_text(encoding="utf8"))
    tools = [t for t in ORDER if t in report["tools"]] + [
        t for t in report["tools"] if t not in ORDER
    ]
    s = {t: report["tools"][t]["summary"] for t in tools}

    def row(label, get):
        print(f"| {label} | " + " | ".join(cell(get(s[t])) for t in tools) + " |")

    print(f"corpus {report['corpus']} @ {report['commit']} · {report['when']}")
    print(f"cases: {report['cases']['retrieval']} retrieval, {report['cases']['blast']} blast\n")
    print("| | " + " | ".join(tools) + " |")
    print("|---|" + "---|" * len(tools))

    def ret(key, kind=None):
        def get(x):
            r = x.get("retrieval")
            if not r:
                return None
            if kind:
                slot = r["by_kind"].get(kind)
                return f"{slot[key]}/{slot['n']}" if slot else None
            return f"{r[key]}/{r['n']}"
        return get

    row("strict, all", ret("strict"))
    row("strict, keyword", ret("strict", "keyword"))
    row("strict, paraphrase", ret("strict", "paraphrase"))
    row("strict, code", ret("strict", "code"))
    row("soft, all", ret("soft"))
    row("retrieval, MRR", lambda x: x.get("retrieval", {}).get("mrr"))
    row("answer, median chars", lambda x: x.get("retrieval", {}).get("chars_median"))
    row("answer, median ms", lambda x: x.get("retrieval", {}).get("ms_median"))
    row("impact, mean recall", lambda x: x.get("impact", {}).get("recall_mean"))
    row("impact, files named",
        lambda x: f"{x['impact']['files_found']}/{x['impact']['files_want']}" if "impact" in x else None)
    row("impact, median ms", lambda x: x.get("impact", {}).get("ms_median"))
    row("trace, all", lambda x: f"{x['trace']['hit']}/{x['trace']['n']}" if "trace" in x else None)
    row("trace, chains found",
        lambda x: f"{x['trace']['path_hit']}/{x['trace']['path_n']}" if "trace" in x else None)
    row("trace, absences called",
        lambda x: f"{x['trace']['none_hit']}/{x['trace']['none_n']}" if "trace" in x else None)
    row("changes, symbols named",
        lambda x: f"{x['changes']['symbols_found']}/{x['changes']['symbols_want']}" if "changes" in x else None)
    row("changes, files named",
        lambda x: f"{x['changes']['files_found']}/{x['changes']['files_want']}" if "changes" in x else None)

    print("\nimpact, case by case (recall):")
    print("| target | tier | want | " + " | ".join(tools) + " |")
    print("|---|---|---|" + "---|" * len(tools))
    rows = {t: {r["target"]: r for r in report["tools"][t]["rows"] if r.get("kind") == "impact"} for t in tools}
    for target in rows[tools[0]]:
        first = rows[tools[0]][target]
        got = " | ".join(cell(rows[t].get(target, {}).get("recall")) for t in tools)
        print(f"| {target} | {first['tier']} | {first['want_files']} | {got} |")


if __name__ == "__main__":
    main()
