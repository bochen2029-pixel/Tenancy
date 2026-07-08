"""Quick JSONL well-formedness check on the v3 outputs."""
import json
import sys

for path in ("dave_v3_sft.jsonl", "dave_v3_dpo.jsonl"):
    n_ok = 0
    n_bad = 0
    for i, line in enumerate(open(path, "r", encoding="utf-8"), 1):
        line = line.strip()
        if not line:
            continue
        try:
            json.loads(line)
            n_ok += 1
        except json.JSONDecodeError as e:
            n_bad += 1
            print(f"  X {path}:{i} {e}")
            if n_bad > 5:
                break
    print(f"  {path}: {n_ok} ok / {n_bad} bad")
    if n_bad:
        sys.exit(1)
