"""Generate anti-confabulation DPO batch JSONL from compact spec.

Usage:
    python _gen_anticonfab_dpo.py SAMPLES.py OUT.jsonl

SAMPLES.py defines a list `SAMPLES` of tuples:
    (subcat, var, memories, query, chosen_think, chosen_reply, rejected_think, rejected_reply)

The rejected reply is a confabulated answer that sounds plausible but invents
details not present in `memories`. The chosen reply is honest absence.

Both prompts are byte-identical (same memories block + same query). Only the
assistant content differs. This isolates the gradient on the cognitive
operation: "honest absence" vs "plausible fabrication".
"""
import importlib.util
import json
import sys


PLACEHOLDER = "<DAVE_SYSTEM_PROMPT>"


def make_pair(subcat, var, memories, query, chosen_think, chosen_reply, rejected_think, rejected_reply):
    chosen = f"<think>\n{chosen_think}\n</think>\n\n{chosen_reply}"
    rejected = f"<think>\n{rejected_think}\n</think>\n\n{rejected_reply}"
    prompt = []
    if var == "SYS":
        prompt.append({"role": "system", "content": PLACEHOLDER})
    prompt.append({"role": "system", "content": memories})
    prompt.append({"role": "user", "content": query})
    return {
        "_var": f"MEM-DPO-{var}-T",
        "_cat": "anti-confab",
        "_subcat": subcat,
        "prompt": prompt,
        "chosen": [{"role": "assistant", "content": chosen}],
        "rejected": [{"role": "assistant", "content": rejected}],
    }


def main():
    if len(sys.argv) != 3:
        sys.exit("usage: _gen_anticonfab_dpo.py SAMPLES.py OUT.jsonl")
    spec_path, out_path = sys.argv[1], sys.argv[2]

    spec = importlib.util.spec_from_file_location("samples", spec_path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)

    rows = [make_pair(*s) for s in mod.SAMPLES]
    with open(out_path, "w", encoding="utf-8") as f:
        for r in rows:
            f.write(json.dumps(r, ensure_ascii=False) + "\n")

    print(f"wrote {len(rows)} pairs to {out_path}")


if __name__ == "__main__":
    main()
