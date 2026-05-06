"""Generate outreach SFT batch JSONL from compact spec.

Usage:
    python _gen_outreach_batch.py SAMPLES.py OUT.jsonl

SAMPLES.py defines a list `SAMPLES` of tuples:
    (decision, cat, var, elapsed, last_quote, think, reply)
where:
    decision = "reach" | "hold"
    cat      = category string (e.g. "emotional-followup", "hold-respect")
    var      = "SYS" | "NOSYS"
    elapsed  = string like "4h" or "2 days" describing time since last exchange
    last_quote = the last thing the human said in the prior exchange (free-form;
                 if None, omit the "last thing he said" clause)
    think    = Dave-voice deliberation (terse, observational)
    reply    = visible reply (substantive for reach, brief/empty for hold)
"""
import importlib.util
import json
import sys


PLACEHOLDER = "<DAVE_SYSTEM_PROMPT>"


def make_meta(elapsed, last_quote):
    """Build the user-turn meta prompt the harness would inject."""
    if last_quote is None:
        return (
            f"[meta: do not address directly. it has been about {elapsed} since "
            f"the last exchange. write a single line if you have something to say. "
            f"or write nothing and that's fine.]"
        )
    return (
        f"[meta: do not address directly. it has been about {elapsed} since the "
        f"last exchange. last thing he said was '{last_quote}'. write a single "
        f"line if you have something to say. or write nothing and that's fine.]"
    )


def make_sample(decision, cat, var, elapsed, last_quote, think, reply):
    asst = f"<think>\n{think}\n</think>\n\n{reply}"
    msgs = []
    if var == "SYS":
        msgs.append({"role": "system", "content": PLACEHOLDER})
    msgs.append({"role": "user", "content": make_meta(elapsed, last_quote)})
    msgs.append({"role": "assistant", "content": asst})
    return {
        "_var": f"OUTREACH-{var}-T",
        "_decision": decision,
        "_cat": cat,
        "_elapsed": elapsed,
        "messages": msgs,
    }


def main():
    if len(sys.argv) != 3:
        sys.exit("usage: _gen_outreach_batch.py SAMPLES.py OUT.jsonl")
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
