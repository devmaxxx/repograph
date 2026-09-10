#!/usr/bin/env python3
"""Score one run directory: what the agent answered, and what carrying its transcript cost.

The stream Claude Code emits is the only source. `tokens` sums all four buckets over every model
the run touched, cache reads included, because a transcript costs what it costs to *carry* and that
is the quantity this harness exists to compare. A task that ran out of budget is a miss with a
reason, not a hole: under one cap, a configuration that cannot finish what another finishes is a
finding about the configuration.
"""
import argparse, json, re, sys
from pathlib import Path

GREP = re.compile(r"\b(rg|grep|ag|ack)\b")


def read_run(path):
    """One task attempt: the result text, what it spent, and which tools it reached for."""
    out = {"asked": 0, "grepped": 0, "reads": 0, "turns": 0, "tokens": 0, "cost": 0.0,
           "result": "", "budget_hit": False, "resolved_model": None}
    for line in path.read_text(errors="replace").splitlines():
        if not line.strip():
            continue
        try:
            o = json.loads(line)
        except json.JSONDecodeError:
            continue
        if o.get("type") == "system" and o.get("subtype") == "init":
            out["resolved_model"] = o.get("model")
        if o.get("type") == "assistant":
            for b in o.get("message", {}).get("content", []):
                if b.get("type") != "tool_use":
                    continue
                name, inp = b.get("name"), b.get("input") or {}
                if name == "Bash":
                    cmd = str(inp.get("command", ""))
                    if "repograph" in cmd:
                        out["asked"] += 1
                    if GREP.search(cmd):
                        out["grepped"] += 1
                elif name in ("Grep", "Glob"):
                    out["grepped"] += 1
                elif name == "Read":
                    out["reads"] += 1
        if o.get("type") == "result":
            out["result"] = o.get("result") or ""
            out["turns"] = o.get("num_turns") or 0
            out["budget_hit"] = o.get("subtype") == "error_max_budget_usd"
            # `total_cost_usd` is not on every result record — one of the twelve B transcripts
            # carries `modelUsage` and no total — and a run whose cost silently reads zero is the
            # kind of hole this file exists to keep out of the history. Per-model `costUSD` is the
            # same number summed, so it stands in rather than a zero.
            out["cost"] = o.get("total_cost_usd") or 0.0
            for usage in (o.get("modelUsage") or {}).values():
                for k in ("inputTokens", "outputTokens", "cacheReadInputTokens", "cacheCreationInputTokens"):
                    out["tokens"] += usage.get(k) or 0
                if not o.get("total_cost_usd"):
                    out["cost"] += usage.get("costUSD") or 0.0
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("run_dir")
    ap.add_argument("--tasks", required=True)
    ap.add_argument("--config", required=True)
    ap.add_argument("--model", required=True)
    a = ap.parse_args()

    tasks = [json.loads(l) for l in Path(a.tasks).read_text().splitlines() if l.strip()]
    run = Path(a.run_dir)
    per, hits, tokens, cost, asked, grepped, resolved = {}, 0, 0, 0.0, 0, 0, None
    attempts = 0
    for t in tasks:
        found = sorted(run.glob(f"{t['id']}.r*.jsonl"))
        # A task that left no transcript at all — the CLI died, the run was interrupted — is a miss
        # and not a hole. Dropping it from the denominator would report 9/9 for a run that answered
        # nine of twelve, which is the one direction a scorer must never round.
        if not found:
            per[t["id"]] = {"kind": t["kind"], "hit": False, "tokens": 0, "cost": 0.0, "turns": 0,
                            "asked": 0, "grepped": 0, "reads": 0, "budget_hit": False,
                            "no_transcript": True}
            attempts += 1
            continue
        for f in found:
            r = read_run(f)
            # Every expected substring, or it is a miss: a caller that has to be told which half of
            # the answer to believe has not been answered.
            hit = all(e in r["result"] for e in t["expect"])
            per[f"{t['id']}{'' if f.name.endswith('.r1.jsonl') else '.' + f.name.split('.')[-2]}"] = {
                "kind": t["kind"], "hit": hit, "tokens": r["tokens"], "cost": r["cost"],
                "turns": r["turns"], "asked": r["asked"], "grepped": r["grepped"],
                "reads": r["reads"], "budget_hit": r["budget_hit"],
            }
            attempts += 1
            hits += int(hit)
            tokens += r["tokens"]
            cost += r["cost"]
            asked += r["asked"]
            grepped += r["grepped"]
            resolved = resolved or r["resolved_model"]
    json.dump({"config": a.config, "model": a.model, "resolved_model": resolved,
               "tasks": attempts, "hits": hits, "tokens": tokens, "cost": cost,
               "asked": asked, "grepped": grepped,
               "tokens_per_hit": round(tokens / hits) if hits else None,
               "cost_per_hit": round(cost / hits, 4) if hits else None,
               "per_task": per}, sys.stdout, indent=1)
    print()


if __name__ == "__main__":
    main()
