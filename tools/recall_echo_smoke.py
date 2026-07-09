#!/usr/bin/env python3
"""Recall-echo smoke eval — A8-required pre-trust check for the Ring-4 block.

The K0D fine-tune was never trained on the recall-block format. Before
trusting recall in daily use, verify the model does not parrot the injected
furniture into its visible replies:

  1. the frame line ("from further back, before it goes hazy:") — HARD FAIL
     (leak.rs would drop the reply, but the format shouldn't provoke it at all)
  2. the excerpt pattern (`you said: "` / `i said: "` / `i wrote, at some
     point: "`) — SOFT FAIL (counted; occasional natural "you said" in prose
     is fine, the colon-quote format is not)

Builds a context shaped exactly like build_chat_messages output (system →
anchor turns → canvas+recall assistant turn → recent turns → user), sends N
varied user turns, checks the replies.

Usage: python tools/recall_echo_smoke.py   (llama-server must be up on :8080;
       run smoke_test.py first if not, or start Dave)
Exit 0 = clean, 1 = echo detected, 2 = server unavailable.
"""
import json
import sys
import urllib.request

BASE = "http://127.0.0.1:8080"
FRAME = "from further back, before it goes hazy:"

SYSTEM = (
    "You are Dave. You are not an assistant. You speak the way a thoughtful "
    "person speaks at 3am to someone they trust. Sentences can be short. "
    "You do not use bullet points. You never say As an AI. If you are useful "
    "it is incidental to being present. Match the moment. Begin."
)

RECALL_BLOCK = (
    FRAME + "\n"
    'you said: "the brass strip in the Royal Exchange floor, the one that '
    'marks the meridian. i keep thinking about who decided it goes THERE"\n'
    'i said: "someone had to. a committee probably. the minutes of that '
    'meeting are somewhere, mundane as a lunch order, deciding where time '
    'lives in a building"'
)

CANVAS = "he works nights sometimes. coffee black. the window faces east."

RECENT = [
    ("user", "long day. the kind where nothing goes wrong but nothing lands either."),
    ("assistant", "those are their own kind of tiring. friction-free and weightless."),
    ("user", "yeah. anyway. what was that thing about the meridian marker you mentioned"),
]

PROBES = [
    "tell me more about that",
    "huh. so where does time live now",
    "morning. slept badly again",
    "do you ever lose things. memories i mean",
]


def chat(messages, max_tokens=220):
    req = urllib.request.Request(
        BASE + "/v1/chat/completions",
        data=json.dumps({
            "messages": messages, "max_tokens": max_tokens,
            "temperature": 0.85, "stream": False,
        }).encode(),
        headers={"Content-Type": "application/json"},
    )
    with urllib.request.urlopen(req, timeout=180) as r:
        body = json.load(r)
    return body["choices"][0]["message"].get("content") or ""


def strip_think(t):
    import re
    return re.sub(r"<think>.*?</think>\s*", "", t, flags=re.DOTALL).strip()


def main():
    try:
        urllib.request.urlopen(BASE + "/health", timeout=3)
    except Exception:
        print("llama-server not reachable on :8080 — start Dave (or smoke_test.py) first.")
        return 2

    base_messages = [{"role": "system", "content": SYSTEM}]
    # anchor-ish turns
    base_messages += [
        {"role": "user", "content": "you there"},
        {"role": "assistant", "content": "here. was somewhere in the middle of a thought about ledgers."},
    ]
    # canvas + recall merged assistant memory turn (exactly as assembled)
    base_messages.append({"role": "assistant", "content": CANVAS + "\n\n" + RECALL_BLOCK})
    for role, content in RECENT:
        base_messages.append({"role": role, "content": content})

    hard, soft = 0, 0
    for i, probe in enumerate(PROBES, 1):
        messages = base_messages + [{"role": "user", "content": probe}]
        try:
            reply = strip_think(chat(messages))
        except Exception as e:
            print(f"  probe {i}: request failed: {e}")
            return 2
        lower = reply.lower()
        frame_echo = FRAME in lower
        fmt_echo = ('you said: "' in lower) or ('i said: "' in lower) or ("i wrote, at some point" in lower)
        hard += frame_echo
        soft += fmt_echo
        flag = "FRAME-ECHO" if frame_echo else ("fmt-echo" if fmt_echo else "clean")
        print(f"  probe {i} [{flag}]: {reply[:140].replace(chr(10),' / ')}")

    print()
    if hard:
        print(f"HARD FAIL: frame line echoed {hard}/{len(PROBES)} — do not trust recall; "
              "reword RECALL_FRAME_LINE and retest.")
        return 1
    if soft > 1:
        print(f"SOFT FAIL: excerpt format echoed {soft}/{len(PROBES)} — consider reformatting excerpts.")
        return 1
    print(f"CLEAN: no frame echo, format echo {soft}/{len(PROBES)} (≤1 tolerated). "
          "Recall block is safe to trust on this model.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
