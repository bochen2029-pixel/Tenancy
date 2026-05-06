"""Generate cadence/elapsed-time SFT batch JSONL from compact spec.

Extends STAGE round 2 format by adding an elapsed-time component to the
scene tag:

    [scene: HH:MM AM/PM Day, Month D, YYYY, TZ | last_seen: <duration> ago] <text>

The thinking layer should absorb BOTH the time-of-day AND the elapsed time
without fixating on either (84% incidental rate).

Usage:
    python _gen_cadence_batch.py SAMPLES.py OUT.jsonl

SAMPLES.py defines a list `SAMPLES` of tuples:
    (cat, var, scene, elapsed, prompt, think, reply)
"""
import importlib.util
import json
import sys


PLACEHOLDER = "<DAVE_SYSTEM_PROMPT>"


def make_sample(cat, var, scene, elapsed, prompt, think, reply):
    user_content = f"[scene: {scene} | last_seen: {elapsed} ago] {prompt}"
    asst = f"<think>\n{think}\n</think>\n\n{reply}"
    msgs = []
    if var == "SYS":
        msgs.append({"role": "system", "content": PLACEHOLDER})
    msgs.append({"role": "user", "content": user_content})
    msgs.append({"role": "assistant", "content": asst})
    return {
        "_var": f"SFT-{var}-T",
        "_cat": cat,
        "_scene": scene,
        "_elapsed": elapsed,
        "messages": msgs,
    }


def main():
    if len(sys.argv) != 3:
        sys.exit("usage: _gen_cadence_batch.py SAMPLES.py OUT.jsonl")
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
