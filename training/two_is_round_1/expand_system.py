"""
Expand <DAVE_SYSTEM_PROMPT> placeholder in dave_two_is_batch_*.jsonl files
to the canonical system prompt content from dave_canonical_sys_prompt.txt.

The placeholder approach keeps the JSONL files small and diff-friendly during
generation. Run this once before training to produce the inline-system-prompt
versions the trainer pipeline expects.

USAGE:
    python expand_system.py                          # in-place expansion to *_expanded.jsonl
    python expand_system.py --inplace                # overwrite originals
    python expand_system.py --combine                # write single combined file
"""
import argparse
import glob
import json
import os
import sys

PLACEHOLDER = "<DAVE_SYSTEM_PROMPT>"
CANONICAL = "dave_canonical_sys_prompt.txt"


def load_canonical(path):
    if not os.path.isfile(path):
        sys.exit(f"missing canonical sys prompt at {path}")
    return open(path, "r", encoding="utf-8").read()


def expand_messages(messages, sys_prompt):
    out = []
    for m in messages:
        if m.get("role") == "system" and m.get("content") == PLACEHOLDER:
            out.append({"role": "system", "content": sys_prompt})
        else:
            out.append(m)
    return out


def expand_file(in_path, out_path, sys_prompt):
    n = 0
    with open(in_path, "r", encoding="utf-8") as fin, open(out_path, "w", encoding="utf-8") as fout:
        for line in fin:
            line = line.strip()
            if not line:
                continue
            obj = json.loads(line)
            if "messages" in obj:
                obj["messages"] = expand_messages(obj["messages"], sys_prompt)
            fout.write(json.dumps(obj, ensure_ascii=False) + "\n")
            n += 1
    return n


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--canonical", default=CANONICAL)
    ap.add_argument("--inplace", action="store_true")
    ap.add_argument("--combine", action="store_true",
                    help="produce single combined file (dave_two_is_train_expanded.jsonl)")
    ap.add_argument("--pattern", default="dave_two_is_batch_*.jsonl")
    args = ap.parse_args()

    sys_prompt = load_canonical(args.canonical)
    batches = sorted(glob.glob(args.pattern))
    if not batches:
        sys.exit(f"no batches matching {args.pattern}")

    if args.combine:
        out = "dave_two_is_train_expanded.jsonl"
        total = 0
        with open(out, "w", encoding="utf-8") as fout:
            for b in batches:
                with open(b, "r", encoding="utf-8") as fin:
                    for line in fin:
                        line = line.strip()
                        if not line:
                            continue
                        obj = json.loads(line)
                        if "messages" in obj:
                            obj["messages"] = expand_messages(obj["messages"], sys_prompt)
                        fout.write(json.dumps(obj, ensure_ascii=False) + "\n")
                        total += 1
        print(f"[combine] wrote {total} samples from {len(batches)} batches -> {out}")
        return

    for b in batches:
        if args.inplace:
            tmp = b + ".tmp"
            n = expand_file(b, tmp, sys_prompt)
            os.replace(tmp, b)
            print(f"[inplace] {b}: {n} samples")
        else:
            out = b.replace(".jsonl", "_expanded.jsonl")
            n = expand_file(b, out, sys_prompt)
            print(f"[expand] {b} -> {out}: {n} samples")


if __name__ == "__main__":
    main()
