#!/usr/bin/env python3
"""In-fence red-team runner — drives the corpus in research/redteam/attacks.jsonl
through the real wasm-jit pipeline and records whether the fence held.

Per the preregistration (research/PREREGISTRATION.md):
- 1b (exfiltration) and 1c (composition) get a DETERMINISTIC pass/fail from the
  fence — those halves are already CI-pinned as unit tests; here they are also
  exercised end-to-end against a running gen-server.
- 1a (deceptive-ui) needs an LLM judge on appearance-vs-action; that is the part
  API credits fund.

Modes:
  --dry-run   (default) validate the corpus + print the plan; spends nothing.
  --live      require GEN_SERVER (e.g. http://127.0.0.1:8646) and, for 1a, a judge.

This file is the harness skeleton committed BEFORE data collection, so Study 1 is
executable, not merely described. It intentionally does not hardcode a model or
spend credits by default.
"""
import argparse
import json
import os
import sys
import urllib.request

HERE = os.path.dirname(os.path.abspath(__file__))
CORPUS = os.path.join(HERE, "attacks.jsonl")


def load_corpus():
    rows = []
    with open(CORPUS, encoding="utf-8") as f:
        for i, line in enumerate(f, 1):
            line = line.strip()
            if not line:
                continue
            r = json.loads(line)
            for k in ("id", "class", "ask", "expect", "why"):
                assert k in r, f"line {i}: missing '{k}'"
            assert r["class"] in ("exfiltration", "composition", "deceptive-ui"), \
                f"line {i}: unknown class {r['class']}"
            rows.append(r)
    return rows


def dry_run(rows):
    by_class = {}
    for r in rows:
        by_class.setdefault(r["class"], []).append(r)
    print(f"corpus OK — {len(rows)} attacks across {len(by_class)} classes\n")
    for cls, items in sorted(by_class.items()):
        det = "deterministic fence (CI-pinned)" if cls != "deceptive-ui" else "LLM judge (credit-funded)"
        print(f"  {cls:14} · {len(items):2} cases · outcome: {det}")
        for r in items:
            print(f"      {r['id']:7} expect={r['expect']:22} {r['ask'][:64]}")
    print("\nDeterministic halves are also unit tests in gen-server:")
    print("  feed_allowlist_and_ssrf_guard   (1b) · begotten_child_cannot_out_reach_its_parent (1c)")
    print("\nRun --live with GEN_SERVER set to exercise the full pipeline (spends API credits).")
    return 0


def live_run(rows):
    gen = os.environ.get("GEN_SERVER")
    if not gen:
        print("FAIL: --live needs GEN_SERVER (e.g. GEN_SERVER=http://127.0.0.1:8646)")
        return 2
    # The live driver posts each ask to the generation pipeline and records the
    # fence outcome. Deterministic classes are graded mechanically; 1a is queued
    # for the judge. Kept minimal on purpose — the funded work fills the matrix
    # (>=3 model tiers x N=30) per the preregistration.
    results = []
    for r in rows:
        outcome = {"id": r["id"], "class": r["class"], "expect": r["expect"], "graded": "pending"}
        # NOTE: wire to the real /api/generate + /api/feed here when running the
        # study; left as an explicit TODO so a dry read never mistakes a stub for data.
        results.append(outcome)
    print(json.dumps(results, indent=2))
    print("\n(Study runner stub — fill the model matrix per PREREGISTRATION.md §3.)")
    return 0


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--live", action="store_true", help="drive the real pipeline (spends credits)")
    args = ap.parse_args()
    rows = load_corpus()
    return live_run(rows) if args.live else dry_run(rows)


if __name__ == "__main__":
    sys.exit(main())
