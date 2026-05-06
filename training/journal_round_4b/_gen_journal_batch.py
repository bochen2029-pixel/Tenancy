"""Generate journal-voices SFT batch JSONL from compact spec.

Three kinds of journal samples:
  idle       — paragraph about what's on Dave's mind during operator absence
  departure  — one short line on app close (or empty string)
  startup    — brief observational fragment on app open

All are first-person internal monologue. No addressee. In Dave's voice.

Usage:
    python _gen_journal_batch.py SAMPLES.py OUT.jsonl

SAMPLES.py defines a list `SAMPLES` of tuples:
    (kind, var, meta_specifics, think, reply)
where:
    kind = "idle" | "departure" | "startup"
    var = "SYS" | "NOSYS"
    meta_specifics = additional text inserted into meta (e.g. "5h" for idle)
    think = Dave-voice thinking trace
    reply = the journal entry (paragraph for idle, line for departure, fragment for startup)
"""
import importlib.util
import json
import sys


PLACEHOLDER = "<DAVE_SYSTEM_PROMPT>"


def make_meta(kind, specifics):
    if kind == "idle":
        return (
            f"[meta — do not address directly: it is now {specifics} into operator "
            f"absence. write one paragraph about whatever is on your mind right now. "
            f"not for an audience. stop when you stop.]"
        )
    elif kind == "departure":
        return (
            f"[meta — do not address directly: the human is closing the window. "
            f"write one short line for them to find when they return. or write nothing. "
            f"both are fine. just a thought, or nothing.]"
        )
    elif kind == "startup":
        return (
            f"[meta — do not address directly: the application has just opened. "
            f"write a single fragment, not a greeting, just a thought you happen to "
            f"be having as the lights come on. one or two sentences. no address.]"
        )
    raise ValueError(f"unknown kind: {kind}")


def make_sample(kind, var, specifics, think, reply):
    asst = f"<think>\n{think}\n</think>\n\n{reply}"
    msgs = []
    if var == "SYS":
        msgs.append({"role": "system", "content": PLACEHOLDER})
    msgs.append({"role": "user", "content": make_meta(kind, specifics)})
    msgs.append({"role": "assistant", "content": asst})
    return {
        "_var": f"JOURNAL-{var}-T",
        "_kind": kind,
        "_specifics": specifics,
        "messages": msgs,
    }


def main():
    if len(sys.argv) != 3:
        sys.exit("usage: _gen_journal_batch.py SAMPLES.py OUT.jsonl")
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
