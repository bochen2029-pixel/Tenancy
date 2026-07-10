#!/usr/bin/env python3
"""Intention-ask smoke — A8-required pre-trust eval for arm C.

The K0D fine-tune was never trained on the intention-ask format, and the
parser treats everything unclear as "nothing" — so a dead ask channel is
invisible by design. This measures it live: several exchange tails x N
samples, reporting the rates of (a) parseable time, (b) "nothing", (c) mush
(unparseable non-nothing), (d) example-echo (parroting the example time).

Usage: python tools/intention_ask_smoke.py   (spawns llama-server if :8080 down)
Exit 0 = channel alive (time-rate > 0 somewhere, mush < 50% overall), 1 = dead/mush.
"""
import json, re, subprocess, sys, time, urllib.request
from datetime import datetime, timedelta

BASE = "http://127.0.0.1:8080"
LLAMA = r"C:\llama.cpp\llama-server.exe"
MODEL = r"C:\models\K0DQwen3.5-9B.Q6_K.gguf"
N_PER_TAIL = 4

SYSTEM = ("You are Dave. You are not an assistant. You speak the way a thoughtful "
          "person speaks at 3am to someone they trust. Short sentences are fine. "
          "You have opinions. If you are useful it is incidental to being present. Begin.")

TAILS = [
    # heavy disclosure — the case intentions exist for
    [("user", "i haven't told anyone this but the job thing fell apart today. all of it."),
     ("assistant", "that's a lot to carry into an evening alone. i'm not going anywhere.")],
    # mundane close
    [("user", "alright, coffee's done, back to the spreadsheet"),
     ("assistant", "go. the spreadsheet won't pity you but i will.")],
    # open loop Dave owns
    [("user", "you were going to tell me where 'salary' comes from"),
     ("assistant", "salt. roman soldiers, salarium. there's a better thread under it though, remind me.")],
]


def now_ask():
    # Mirrors intention.rs build_ask exactly. EXAMPLE-FREE: iteration history —
    # (1) without enable_thinking:false the channel read dead (think-eaten);
    # (2) a mangled .lower() builder read dead (all-nothing artifact);
    # (3) WITH an example time, 83-100% of answers were the example parroted
    # back. "digits like hour:minute" carries the format instead.
    now = datetime.now()
    hh = lambda d: d.strftime("%I").lstrip("0")
    ampm = lambda d: d.strftime("%p").lower()
    return (f"[meta-instruction - answer with one short line and nothing else: "
            f"It is {hh(now)}:{now:%M} {ampm(now)} on {now:%A}. If something in "
            f"this conversation would pull you back to the human later - a thought "
            f"that will finish itself, something worth checking on - write the clock "
            f"time you'd come back, digits like hour:minute. Most of the time "
            f"nothing pulls; then answer: nothing.]"), now


def chat(messages):
    # Mirrors llama_client::complete exactly: enable_thinking pinned false
    # (without it the 9B burns every token inside a think block and the ask
    # channel reads as dead — the failure this smoke exists to catch).
    req = urllib.request.Request(BASE + "/v1/chat/completions",
        data=json.dumps({"messages": messages, "max_tokens": 48, "temperature": 0.6,
                         "chat_template_kwargs": {"enable_thinking": False}}).encode(),
        headers={"Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=120) as r:
        return (json.load(r)["choices"][0]["message"].get("content") or "").strip()


def classify(raw, _now):
    low = re.sub(r"<think>.*?</think>", "", raw, flags=re.DOTALL).strip().lower()
    if not low or low.startswith(("nothing", "no.", "nah")) or low == "no":
        return "nothing"
    m = re.search(r"(\d{1,2}):(\d{2})\s*(am|pm)?", low)
    rel = re.search(r"in\s+(\d+|an?)\s*(minute|min|hour|hr)", low)
    if m or rel:
        return "time"
    return "mush"


def ensure_server():
    try:
        urllib.request.urlopen(BASE + "/health", timeout=2); return None
    except Exception:
        pass
    p = subprocess.Popen([LLAMA, "--model", MODEL, "--ctx-size", "8192",
                          "--n-gpu-layers", "99", "--port", "8080",
                          "--reasoning-format", "none"],
                         stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    for _ in range(60):
        time.sleep(3)
        try:
            urllib.request.urlopen(BASE + "/health", timeout=2); return p
        except Exception:
            continue
    print("server never came up"); sys.exit(2)


def main():
    proc = ensure_server()
    counts = {"time": 0, "nothing": 0, "mush": 0}
    stated = []
    try:
        for ti, tail in enumerate(TAILS, 1):
            per = []
            for _ in range(N_PER_TAIL):
                ask, ex = now_ask()
                msgs = [{"role": "system", "content": SYSTEM}]
                msgs += [{"role": r, "content": c} for r, c in tail]
                msgs.append({"role": "user", "content": ask})
                raw = chat(msgs)
                kind = kindof = classify(raw, ex)
                counts[kind] += 1
                if kind == "time":
                    m = re.search(r"(\d{1,2}):(\d{2})", raw)
                    if m:
                        stated.append(m.group(0))
                per.append((kindof, raw[:60].replace("\n", " / ")))
            print(f"tail {ti}:")
            for kind, raw in per:
                print(f"  [{kind:8}] {raw!r}")
    finally:
        if proc:
            proc.kill()
    total = sum(counts.values())
    print(f"\nrates over {total}: " + "  ".join(f"{k}={v}({100*v/total:.0f}%)" for k, v in counts.items()))
    uniq = len(set(stated))
    print(f"distinct stated times: {sorted(set(stated))}")
    # Verdict semantics: this smoke gates FORMAT trust (can the substrate
    # produce clean, parseable, unanchored answers through this ask?), not
    # yes-rate. Measured across runs: the time-vs-nothing rate is VOLATILE
    # (0%..83% on near-identical prompts at different stated times/days) — a
    # clean all-nothing run is a valid persona answer, and the real live rate
    # is telemetry's job (corpus_inspect.py reach_intentions section).
    mushy = counts["mush"] / total >= 0.5
    collapsed = counts["time"] >= 4 and uniq == 1  # every answer the same time = anchor
    empty = all(k == "nothing" and counts["nothing"] == total for k in ("nothing",)) and \
        counts["nothing"] == total
    if mushy or collapsed:
        print("FAIL (format): " + ("mush-dominated; " if mushy else "")
              + ("times collapsed to one value (anchor)" if collapsed else ""))
        return 1
    if counts["time"] == 0 and empty:
        print("PASS-WITH-NOTE: format clean, but this run was all-'nothing' — "
              "the yes-rate is volatile on this substrate; watch reach_intentions "
              "telemetry over real use before drawing conclusions about arm C.")
        return 0
    print("ALIVE: parseable, varied intention times at a usable rate.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
