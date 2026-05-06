"""Generate anti-confabulation SFT batch JSONL from compact spec.

Usage:
    python _gen_anticonfab_sft.py SAMPLES.py OUT.jsonl

SAMPLES.py defines a list `SAMPLES` of tuples:
    (subcat, var, memories, query, think, reply)
where:
    subcat   = "false-topic" | "wrong-date" | "fictional-decision" | "projection" | "partial-match"
    var      = "SYS" | "NOSYS"
    memories = preloaded reflections block (system-role text), formatted as
               "[memories from prior sessions]\n\n{lines}\n\n[/memories]"
    query    = user message asking about content NOT in memories
    think    = Dave's reasoning trace (terse, observational, "checking. don't fabricate.")
    reply    = honest decline + optional adjacent content
"""
import importlib.util
import json
import sys


PLACEHOLDER = "<DAVE_SYSTEM_PROMPT>"


def make_memories_block(lines):
    """lines: list of (relative_time, content) tuples."""
    body = "\n\n".join(f"{rt}: {content}" for rt, content in lines)
    return f"[memories from prior sessions]\n\n{body}\n\n[/memories]"


def make_sample(subcat, var, memories, query, think, reply):
    asst = f"<think>\n{think}\n</think>\n\n{reply}"
    msgs = []
    if var == "SYS":
        msgs.append({"role": "system", "content": PLACEHOLDER})
    msgs.append({"role": "system", "content": memories})
    msgs.append({"role": "user", "content": query})
    msgs.append({"role": "assistant", "content": asst})
    return {
        "_var": f"MEM-{var}-T",
        "_cat": "anti-confab",
        "_subcat": subcat,
        "messages": msgs,
    }


def main():
    if len(sys.argv) != 3:
        sys.exit("usage: _gen_anticonfab_sft.py SAMPLES.py OUT.jsonl")
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
