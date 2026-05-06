"""Generate DPO-THINK batch JSONL from compact spec.

Usage:
    python _gen_dpo_batch.py SAMPLES.py OUT.jsonl

SAMPLES.py defines a list `SAMPLES` of tuples:
    (cat, var, prompt, chosen_think, reply, rejected_think)
where:
    cat = category string ("greeting", "identity", etc.)
    var = "SYS" | "NOSYS"
    prompt = user message text
    chosen_think = Dave-voice thinking trace (terse, observational)
    reply = visible reply (used identically in chosen and rejected)
    rejected_think = evaluator-frame thinking trace (third-person, "I should...", verbose)
"""
import importlib.util
import json
import sys


PLACEHOLDER = "<DAVE_SYSTEM_PROMPT>"


def make_pair(cat, var, prompt, chosen_think, reply, rejected_think):
    chosen = f"<think>\n{chosen_think}\n</think>\n\n{reply}"
    rejected = f"<think>\n{rejected_think}\n</think>\n\n{reply}"
    obj = {
        "_var": f"DPO-{var}-T",
        "_cat": cat,
        "prompt": [],
        "chosen": [{"role": "assistant", "content": chosen}],
        "rejected": [{"role": "assistant", "content": rejected}],
    }
    if var == "SYS":
        obj["prompt"].append({"role": "system", "content": PLACEHOLDER})
    obj["prompt"].append({"role": "user", "content": prompt})
    return obj


def main():
    if len(sys.argv) != 3:
        sys.exit("usage: _gen_dpo_batch.py SAMPLES.py OUT.jsonl")
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
