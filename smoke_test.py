#!/usr/bin/env python3
"""Dave smoke test — a pre-release gate against the class of break that shipped
the "empty Dave" / reasoning-format regression.

Spawns llama-server with the EXACT flags Dave's sidecar uses, then asserts the
real failure modes:
  1. chat returns NON-EMPTY visible content
  2. no <think> block leaks to the user (ThinkStripper's job)
  3. no harness vocabulary leaks ([pass] / [meta] / [outreach] / [decision])
  4. a single-shot (journal/departure-style) generation returns non-empty
  5. thinking-ON path still yields non-empty content (the landmine that started
     it all — reasoning must not vanish into reasoning_content)

Usage:
    python smoke_test.py [MODEL.gguf]      # defaults to the shipped default model
Exit code 0 = all pass, 1 = any failure. Wire into a pre-release check.
"""
import subprocess, time, json, re, urllib.request, os, sys, sqlite3

LLAMA = r"C:\llama.cpp\llama-server.exe"
DEFAULT_CANDIDATES = [
    r"C:\models\K0DQwen3.5-9B.Q6_K.gguf",
    r"C:\models\Qwen3.5-9B-Q5_K_M.gguf",
]

def resolve_model():
    if len(sys.argv) > 1 and os.path.exists(sys.argv[1]):
        return sys.argv[1]
    # Try the shipped default (active_model_path in the release DB)
    db = os.path.expandvars(r"%LOCALAPPDATA%\com.bochen.dave\dave.db")
    try:
        v = sqlite3.connect(db).execute(
            "SELECT value FROM settings WHERE key='active_model_path'").fetchone()
        if v and v[0] and os.path.exists(v[0]):
            return v[0]
    except Exception:
        pass
    for c in DEFAULT_CANDIDATES:
        if os.path.exists(c):
            return c
    sys.exit("no model found; pass one as argv[1]")

HARNESS_VOCAB = ["[pass]", "[meta", "[outreach", "[decision"]
def strip_think(t): return re.sub(r"<think>.*?</think>\s*", "", t, flags=re.DOTALL).strip()

def wait_health(timeout=180):
    t0 = time.time()
    while time.time() - t0 < timeout:
        try:
            with urllib.request.urlopen("http://127.0.0.1:8080/health", timeout=2) as r:
                if b"ok" in r.read(): return True
        except Exception: pass
        time.sleep(2)
    return False

def chat(messages, enable_thinking=False, max_tokens=400):
    body = {"model": "dave", "stream": False, "temperature": 0.85, "max_tokens": max_tokens,
            "top_p": 0.9, "top_k": 20, "repeat_penalty": 1.0, "presence_penalty": 1.5,
            "messages": messages}
    if enable_thinking:
        body["chat_template_kwargs"] = {"enable_thinking": True}
    req = urllib.request.Request("http://127.0.0.1:8080/v1/chat/completions",
                                 json.dumps(body).encode(), {"Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=120) as r:
        m = json.loads(r.read())["choices"][0]["message"]
    return strip_think(m.get("content") or ""), (m.get("reasoning_content") or "")

DAVE = ("You are Dave. You are not an assistant. You speak plainly, the way a "
        "thoughtful person speaks at 3am. You care about abandoned infrastructure, "
        "etymology, the history of standardized time.")

def spawn(model):
    args = [LLAMA, "--model", model, "--ctx-size", "8192", "--n-gpu-layers", "99",
            "--port", "8080", "--host", "127.0.0.1", "--temp", "0.85", "--top-p", "0.9",
            "--top-k", "20", "--repeat-penalty", "1.0", "--presence-penalty", "1.5",
            "--jinja", "--chat-template-kwargs", '{"enable_thinking":false}',
            "--reasoning-format", "none"]
    return subprocess.Popen(args, cwd=r"C:\llama.cpp",
                            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

def kill(p):
    try: p.terminate(); p.wait(timeout=8)
    except Exception: pass
    subprocess.run(["taskkill", "/F", "/IM", "llama-server.exe"],
                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

def main():
    model = resolve_model()
    print(f"[smoke] model: {model}")
    p = spawn(model)
    ok = True
    try:
        if not wait_health():
            print("[FAIL] llama-server did not become ready"); sys.exit(1)
        checks = []

        # 1-3: normal chat
        txt, _ = chat([{"role": "system", "content": DAVE},
                       {"role": "user", "content": "tell me about an abandoned railway, two sentences"}])
        checks.append(("chat non-empty", len(txt) > 0))
        checks.append(("no <think> leak", "<think>" not in txt.lower()))
        checks.append(("no harness vocab", not any(h in txt.lower() for h in HARNESS_VOCAB)))

        # 4: single-shot journal/departure-style
        j, _ = chat([{"role": "system", "content": DAVE},
                     {"role": "user", "content": "[write one short line for them to find when they return]"}],
                    max_tokens=80)
        checks.append(("single-shot non-empty", len(j) > 0))

        # 5: thinking-ON must still yield non-empty content (the landmine)
        t2, _ = chat([{"role": "system", "content": DAVE},
                      {"role": "user", "content": "why do old stations feel the way they do?"}],
                     enable_thinking=True, max_tokens=600)
        checks.append(("thinking-on non-empty", len(t2) > 0))

        print()
        for name, passed in checks:
            print(f"  [{'PASS' if passed else 'FAIL'}] {name}")
            ok = ok and passed
        print(f"\n[smoke] sample: {txt[:120]!r}")
    finally:
        kill(p)
    print("\n[smoke] " + ("ALL PASS" if ok else "FAILURES PRESENT"))
    sys.exit(0 if ok else 1)

if __name__ == "__main__":
    main()
