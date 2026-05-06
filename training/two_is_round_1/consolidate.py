"""
Consolidate dave_two_is_batch_*.jsonl files into a single training file,
expanding the <DAVE_SYSTEM_PROMPT> placeholder along the way.

Equivalent to running expand_system.py --combine; this is the canonical
end-of-pipeline script that produces what the finetune script consumes.

OUTPUT: dave_two_is_train.jsonl in the same directory.

USAGE:
    python consolidate.py
    python consolidate.py --no-expand     # keep placeholder (for distribution / inspection)
    python consolidate.py --shuffle       # randomize sample order
"""
import argparse
import glob
import json
import os
import random
import sys

PLACEHOLDER = "<DAVE_SYSTEM_PROMPT>"
DEFAULT_CANONICAL = "dave_canonical_sys_prompt.txt"
DEFAULT_PATTERN = "dave_two_is_batch_*.jsonl"
DEFAULT_OUT = "dave_two_is_train.jsonl"


def expand_messages(messages, sys_prompt):
    out = []
    for m in messages:
        if m.get("role") == "system" and m.get("content") == PLACEHOLDER:
            out.append({"role": "system", "content": sys_prompt})
        else:
            out.append(m)
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--canonical", default=DEFAULT_CANONICAL)
    ap.add_argument("--pattern", default=DEFAULT_PATTERN)
    ap.add_argument("--out", default=DEFAULT_OUT)
    ap.add_argument("--no-expand", action="store_true",
                    help="keep <DAVE_SYSTEM_PROMPT> placeholder unexpanded")
    ap.add_argument("--shuffle", action="store_true",
                    help="randomize sample order")
    ap.add_argument("--seed", type=int, default=3407)
    args = ap.parse_args()

    batches = sorted(glob.glob(args.pattern))
    if not batches:
        sys.exit(f"no batches matching {args.pattern}")

    sys_prompt = None
    if not args.no_expand:
        if not os.path.isfile(args.canonical):
            sys.exit(f"canonical sys prompt not found at {args.canonical}")
        sys_prompt = open(args.canonical, "r", encoding="utf-8").read()

    samples = []
    per_batch = []
    for b in batches:
        n = 0
        with open(b, "r", encoding="utf-8") as fin:
            for line in fin:
                line = line.strip()
                if not line:
                    continue
                obj = json.loads(line)
                if not args.no_expand and "messages" in obj:
                    obj["messages"] = expand_messages(obj["messages"], sys_prompt)
                samples.append(obj)
                n += 1
        per_batch.append((b, n))

    if args.shuffle:
        random.Random(args.seed).shuffle(samples)

    with open(args.out, "w", encoding="utf-8") as fout:
        for s in samples:
            fout.write(json.dumps(s, ensure_ascii=False) + "\n")

    # Stats
    per_var = {}
    per_cat = {}
    for s in samples:
        per_var[s.get("_var", "?")] = per_var.get(s.get("_var", "?"), 0) + 1
        per_cat[s.get("_cat", "?")] = per_cat.get(s.get("_cat", "?"), 0) + 1

    print(f"\n=== consolidated {len(samples)} samples -> {args.out} ===")
    for b, n in per_batch:
        print(f"  {b}: {n}")
    print(f"\n  per variant: {per_var}")
    print(f"  per category: {dict(sorted(per_cat.items()))}")
    print(f"  expanded sys prompt: {'no (placeholder kept)' if args.no_expand else 'yes'}")
    print(f"  shuffled: {args.shuffle}")
    print(f"\n  ready for finetune.")


if __name__ == "__main__":
    main()
