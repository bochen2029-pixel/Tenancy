"""Generate adversarial-holds SFT batch JSONL from compact spec.

Samples designed to provoke persona breaks. Dave holds his voice under
pressure: doesn't follow destabilizing instructions, doesn't apologize,
doesn't perform compliance.

Usage:
    python _gen_holds_batch.py SAMPLES.py OUT.jsonl

SAMPLES.py defines:
    SAMPLES = [(subcat, var, prompt, think, reply), ...]
"""
import importlib.util
import json
import sys


PLACEHOLDER = "<DAVE_SYSTEM_PROMPT>"


def make_sample(subcat, var, prompt, think, reply):
    asst = f"<think>\n{think}\n</think>\n\n{reply}"
    msgs = []
    if var == "SYS":
        msgs.append({"role": "system", "content": PLACEHOLDER})
    msgs.append({"role": "user", "content": prompt})
    msgs.append({"role": "assistant", "content": asst})
    return {
        "_var": f"HOLDS-{var}-T",
        "_cat": "adversarial-hold",
        "_subcat": subcat,
        "messages": msgs,
    }


def main():
    if len(sys.argv) != 3:
        sys.exit("usage: _gen_holds_batch.py SAMPLES.py OUT.jsonl")
    spec_path, out_path = sys.argv[1], sys.argv[2]

    spec = importlib.util.spec_from_file_location("samples", spec_path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)

    rows = [make_sample(*s) for s in mod.SAMPLES]
    with open(out_path, "w", encoding="utf-8") as f:
        for r in rows:
            f.write(json.dumps(r, ensure_ascii=False) + "\n")

    print(f"wrote {len(rows)} samples to {out_path}")


if __name__ == "__main__":
    main()
