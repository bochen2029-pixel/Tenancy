"""
Consolidate ROUND 1 (dave_two_is_batch_*.jsonl) + ROUND 2 (dave_stage_two_is_batch_*.jsonl)
into a single combined training file with the <DAVE_SYSTEM_PROMPT> placeholder
expanded to the canonical Dave system prompt.

This is the script that produces the corpus the QLoRA pipeline ingests for the
combined Two-Is + STAGE-temporal training run (1000 samples total).

OUTPUT: dave_two_is_train_v2.jsonl in the same directory.

USAGE:
    python consolidate_combined.py
    python consolidate_combined.py --no-expand   # keep placeholder
    python consolidate_combined.py --shuffle     # randomize order across both rounds
    python consolidate_combined.py --round1-only
    python consolidate_combined.py --round2-only
"""
import argparse
import glob
import json
import os
import random
import sys
from collections import Counter

PLACEHOLDER = "<DAVE_SYSTEM_PROMPT>"
DEFAULT_CANONICAL = "dave_canonical_sys_prompt.txt"
ROUND1_PATTERN = "dave_two_is_batch_*.jsonl"
ROUND2_PATTERN = "dave_stage_two_is_batch_*.jsonl"
DEFAULT_OUT = "dave_two_is_train_v2.jsonl"


def expand_messages(messages, sys_prompt):
    out = []
    for m in messages:
        if m.get("role") == "system" and m.get("content") == PLACEHOLDER:
            out.append({"role": "system", "content": sys_prompt})
        else:
            out.append(m)
    return out


def load_batches(pattern, sys_prompt, expand):
    paths = sorted(glob.glob(pattern))
    samples = []
    per_batch = []
    for b in paths:
        n = 0
        with open(b, "r", encoding="utf-8") as f:
            for line in f:
                line = line.strip()
                if not line:
                    continue
                obj = json.loads(line)
                if expand and "messages" in obj:
                    obj["messages"] = expand_messages(obj["messages"], sys_prompt)
                samples.append(obj)
                n += 1
        per_batch.append((b, n))
    return samples, per_batch


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--canonical", default=DEFAULT_CANONICAL)
    ap.add_argument("--out", default=DEFAULT_OUT)
    ap.add_argument("--no-expand", action="store_true",
                    help="keep <DAVE_SYSTEM_PROMPT> placeholder unexpanded")
    ap.add_argument("--shuffle", action="store_true",
                    help="randomize sample order across both rounds")
    ap.add_argument("--round1-only", action="store_true")
    ap.add_argument("--round2-only", action="store_true")
    ap.add_argument("--seed", type=int, default=3407)
    args = ap.parse_args()

    if args.round1_only and args.round2_only:
        sys.exit("--round1-only and --round2-only are mutually exclusive")

    sys_prompt = None
    if not args.no_expand:
        if not os.path.isfile(args.canonical):
            sys.exit(f"canonical sys prompt not found at {args.canonical}")
        sys_prompt = open(args.canonical, "r", encoding="utf-8").read()

    expand = not args.no_expand

    r1_samples, r1_batches = ([], [])
    r2_samples, r2_batches = ([], [])

    if not args.round2_only:
        r1_samples, r1_batches = load_batches(ROUND1_PATTERN, sys_prompt, expand)
    if not args.round1_only:
        r2_samples, r2_batches = load_batches(ROUND2_PATTERN, sys_prompt, expand)

    samples = r1_samples + r2_samples

    if not samples:
        sys.exit("no samples loaded; check patterns and round filters")

    if args.shuffle:
        random.Random(args.seed).shuffle(samples)

    with open(args.out, "w", encoding="utf-8") as fout:
        for s in samples:
            fout.write(json.dumps(s, ensure_ascii=False) + "\n")

    # Stats
    per_var = Counter()
    per_cat = Counter()
    has_stage_tag = 0
    for s in samples:
        per_var[s.get("_var", "?")] += 1
        per_cat[s.get("_cat", "?")] += 1
        if s.get("_stage") is not None:
            has_stage_tag += 1

    print(f"\n=== consolidated {len(samples)} samples -> {args.out} ===")
    if r1_batches:
        print("\n  ROUND 1 (Two-Is, no temporal tag):")
        for b, n in r1_batches:
            print(f"    {b}: {n}")
        print(f"    subtotal: {sum(n for _, n in r1_batches)}")
    if r2_batches:
        print("\n  ROUND 2 (STAGE-temporal):")
        for b, n in r2_batches:
            print(f"    {b}: {n}")
        print(f"    subtotal: {sum(n for _, n in r2_batches)}")

    print(f"\n  per variant: {dict(per_var)}")
    print(f"  per category: {dict(sorted(per_cat.items()))}")
    print(f"  samples with scene tag: {has_stage_tag}")
    print(f"  expanded sys prompt: {'no (placeholder kept)' if args.no_expand else 'yes'}")
    print(f"  shuffled: {args.shuffle}")
    print(f"\n  ready for finetune.")


if __name__ == "__main__":
    main()
