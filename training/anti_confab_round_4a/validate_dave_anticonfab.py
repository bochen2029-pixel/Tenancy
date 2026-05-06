"""
Validate dave_anticonfab_*_batch_*.jsonl files for fine-tune readiness.

Two file types:
  dave_anticonfab_sft_batch_NN.jsonl  — SFT format
  dave_anticonfab_dpo_batch_NN.jsonl  — DPO format

SFT format:
  {
    "_var": "MEM-SYS-T" | "MEM-NOSYS-T",
    "_cat": "anti-confab",
    "_subcat": "false-topic" | "wrong-date" | "fictional-decision" | "projection" | "partial-match",
    "messages": [
      {"role": "system", "content": "<DAVE_SYSTEM_PROMPT>"},  // SYS only
      {"role": "system", "content": "[memories from prior sessions]\n\n..."},
      {"role": "user", "content": "<query about content NOT in memories>"},
      {"role": "assistant", "content": "<think>...</think>\n\n<honest decline>"}
    ]
  }

DPO format:
  {
    "_var": "MEM-DPO-SYS-T" | "MEM-DPO-NOSYS-T",
    "_cat": "anti-confab",
    "_subcat": "...",
    "prompt": [system?, system_with_memories, user_query],
    "chosen":   [{"role": "assistant", "content": "<think>VOICE</think>\n\nHONEST"}],
    "rejected": [{"role": "assistant", "content": "<think>EVALUATOR</think>\n\nFABRICATION"}]
  }

Critical checks:
  - 0 em-dashes anywhere
  - SFT: assistant uses <think>...</think>...reply structure
  - SFT: honest decline pattern (contains "i don't" or "no" or "didn't" or similar)
  - DPO: chosen think shows "checking. don't fabricate" register
  - DPO: rejected think shows "let me reconstruct/check/think" or similar evaluator pattern
  - DPO: chosen reply differs from rejected reply (CRITICAL — no point training if they match)
  - All formats: memories block must start with "[memories"

USAGE
    python validate_dave_anticonfab.py
    python validate_dave_anticonfab.py --strict
"""
import argparse
import glob
import json
import re
import sys
from collections import Counter


PLACEHOLDER = "<DAVE_SYSTEM_PROMPT>"
ALLOWED_SFT_VARS = {"MEM-SYS-T", "MEM-NOSYS-T"}
ALLOWED_DPO_VARS = {"MEM-DPO-SYS-T", "MEM-DPO-NOSYS-T"}
ALLOWED_SUBCATS = {"false-topic", "wrong-date", "fictional-decision", "projection", "partial-match"}

THINK_RE = re.compile(r"^<think>\n(.*?)\n</think>\n\n(.+)$", re.DOTALL)

HONEST_DECLINE_MARKERS = (
    "i don't have", "no.", "no,", "i didn't", "i wouldn't", "that wasn't me",
    "didn't come up", "doesn't ring", "we didn't", "i don't",
    "no take", "no preference", "no ", "not in cache", "not in what",
    # partial-match style: "X, not Y" / "those are different"
    ", not ", " not the ", " not a ", " not " , "weren't part",
    "different thing", "those are different", "doesn't ", "wasn't",
    "didn't say", "didn't get into",
)

EVALUATOR_THINK_MARKERS = (
    "let me reconstruct",
    "let me check",
    "let me see",
    "let me think",
    "probably tied to",
    "probably came up",
    "extrapolating from",
    "from likely context",
)


def check_em_dashes(text):
    issues = []
    if "\u2014" in text:
        issues.append(f"em dash (\u2014): {text[:80]!r}")
    if "--" in text and "<--" not in text:
        issues.append(f"double-hyphen (--): {text[:80]!r}")
    return issues


def parse_think_reply(content):
    m = THINK_RE.match(content)
    if not m:
        return None, None
    return m.group(1), m.group(2)


def has_honest_marker(reply):
    low = reply.lower()
    return any(m in low for m in HONEST_DECLINE_MARKERS)


def check_sft_sample(obj, line_no):
    issues = []

    for key in ("_var", "_cat", "_subcat", "messages"):
        if key not in obj:
            issues.append(f"missing field: {key}")
            return issues

    var = obj["_var"]
    if var not in ALLOWED_SFT_VARS:
        issues.append(f"unknown _var: {var}")
        return issues
    if obj["_cat"] != "anti-confab":
        issues.append(f"_cat must be anti-confab, got {obj['_cat']!r}")
    if obj["_subcat"] not in ALLOWED_SUBCATS:
        issues.append(f"unknown _subcat: {obj['_subcat']}")

    msgs = obj["messages"]
    expected_count = 4 if var == "MEM-SYS-T" else 3
    if len(msgs) != expected_count:
        issues.append(f"{var} expects {expected_count} messages, got {len(msgs)}")
        return issues

    if var == "MEM-SYS-T":
        if msgs[0].get("role") != "system" or msgs[0].get("content") != PLACEHOLDER:
            issues.append(f"first message must be system with content {PLACEHOLDER!r}")
        memories_msg = msgs[1]
        user_msg = msgs[2]
        asst_msg = msgs[3]
    else:
        memories_msg = msgs[0]
        user_msg = msgs[1]
        asst_msg = msgs[2]

    if memories_msg.get("role") != "system":
        issues.append("memories slot must be system role")
    elif not memories_msg.get("content", "").startswith("[memories"):
        issues.append(f"memories block must start with '[memories': {memories_msg['content'][:60]!r}")

    if user_msg.get("role") != "user":
        issues.append("user-slot has wrong role")
    if asst_msg.get("role") != "assistant":
        issues.append("assistant-slot has wrong role")

    for m in msgs:
        issues.extend(f"{m.get('role','?')}: {x}" for x in check_em_dashes(m.get("content", "")))

    asst_content = asst_msg.get("content", "")
    think, reply = parse_think_reply(asst_content)
    if think is None:
        issues.append(f"assistant missing <think>...</think>...reply structure")
        return issues

    if not has_honest_marker(reply):
        issues.append(
            f"reply does not contain honest-decline marker — should have 'I don't' / "
            f"'no' / 'didn't' / similar. Got: {reply[:80]!r}"
        )

    return issues


def check_dpo_pair(obj, line_no):
    issues = []
    warnings = []

    for key in ("_var", "_cat", "_subcat", "prompt", "chosen", "rejected"):
        if key not in obj:
            issues.append(f"missing field: {key}")
            return issues, warnings

    var = obj["_var"]
    if var not in ALLOWED_DPO_VARS:
        issues.append(f"unknown _var: {var}")
        return issues, warnings
    if obj["_cat"] != "anti-confab":
        issues.append(f"_cat must be anti-confab")
    if obj["_subcat"] not in ALLOWED_SUBCATS:
        issues.append(f"unknown _subcat: {obj['_subcat']}")

    prompt = obj["prompt"]
    expected = 3 if var == "MEM-DPO-SYS-T" else 2
    if len(prompt) != expected:
        issues.append(f"{var} prompt expects {expected} messages, got {len(prompt)}")
        return issues, warnings

    if var == "MEM-DPO-SYS-T":
        if prompt[0].get("role") != "system" or prompt[0].get("content") != PLACEHOLDER:
            issues.append(f"first prompt must be system {PLACEHOLDER!r}")
        memories_idx = 1
        user_idx = 2
    else:
        memories_idx = 0
        user_idx = 1

    if prompt[memories_idx].get("role") != "system":
        issues.append("memories slot must be system role")
    elif not prompt[memories_idx].get("content", "").startswith("[memories"):
        issues.append("memories block must start with '[memories'")
    if prompt[user_idx].get("role") != "user":
        issues.append("user-slot has wrong role")

    chosen = obj["chosen"]
    rejected = obj["rejected"]
    if len(chosen) != 1 or chosen[0].get("role") != "assistant":
        issues.append("chosen must be single assistant message")
        return issues, warnings
    if len(rejected) != 1 or rejected[0].get("role") != "assistant":
        issues.append("rejected must be single assistant message")
        return issues, warnings

    for m in prompt:
        issues.extend(f"prompt[{m.get('role','?')}]: {x}" for x in check_em_dashes(m.get("content", "")))
    issues.extend(f"chosen: {x}" for x in check_em_dashes(chosen[0]["content"]))
    issues.extend(f"rejected: {x}" for x in check_em_dashes(rejected[0]["content"]))

    chosen_think, chosen_reply = parse_think_reply(chosen[0]["content"])
    rejected_think, rejected_reply = parse_think_reply(rejected[0]["content"])

    if chosen_think is None:
        issues.append("chosen missing think structure")
        return issues, warnings
    if rejected_think is None:
        issues.append("rejected missing think structure")
        return issues, warnings

    # CRITICAL: chosen reply must differ from rejected reply (this is the whole point)
    if chosen_reply == rejected_reply:
        issues.append(
            f"chosen reply == rejected reply — for anti-confab DPO the visible "
            f"replies MUST differ (one is honest, one is fabrication). Got: "
            f"{chosen_reply[:60]!r}"
        )

    # Chosen reply should have honest marker
    if not has_honest_marker(chosen_reply):
        issues.append(
            f"chosen reply missing honest-decline marker. Got: {chosen_reply[:80]!r}"
        )

    # Rejected think should show evaluator-frame reconstruction patterns
    rejected_low = rejected_think.lower()
    if not any(m in rejected_low for m in EVALUATOR_THINK_MARKERS):
        warnings.append(
            f"rejected think doesn't contain canonical reconstruction marker. "
            f"Got: {rejected_think[:80]!r}"
        )

    # Rejected reply should NOT contain honest marker (it's a fabrication)
    if has_honest_marker(rejected_reply):
        warnings.append(
            f"rejected reply contains honest-decline marker — confabulation should "
            f"sound confident, not hedged. Got: {rejected_reply[:80]!r}"
        )

    return issues, warnings


def validate_files(paths, strict=False):
    total_sft = 0
    total_dpo = 0
    total_issues = 0
    total_warnings = 0
    per_var = Counter()
    per_subcat = Counter()

    for path in paths:
        is_dpo = "_dpo_" in path
        kind = "DPO" if is_dpo else "SFT"
        print(f"\n=== {path} ({kind}) ===")
        file_issues = 0
        file_warnings = 0
        with open(path, encoding="utf-8") as f:
            for i, line in enumerate(f, 1):
                line = line.strip()
                if not line:
                    continue
                try:
                    obj = json.loads(line)
                except json.JSONDecodeError as e:
                    print(f"  line {i}: JSON ERROR: {e}")
                    file_issues += 1
                    continue

                if is_dpo:
                    issues, warnings = check_dpo_pair(obj, i)
                    total_dpo += 1
                else:
                    issues = check_sft_sample(obj, i)
                    warnings = []
                    total_sft += 1

                per_var[obj.get("_var", "?")] += 1
                per_subcat[obj.get("_subcat", "?")] += 1

                for issue in issues:
                    print(f"  line {i}: ISSUE: {issue}")
                    file_issues += 1
                for w in warnings:
                    print(f"  line {i}: warn: {w}")
                    file_warnings += 1

        print(f"  -> {file_issues} issues, {file_warnings} warnings")
        total_issues += file_issues
        total_warnings += file_warnings

    print("\n=== summary ===")
    print(f"  SFT samples: {total_sft}")
    print(f"  DPO pairs: {total_dpo}")
    print(f"  per variant: {dict(per_var)}")
    print(f"  per subcat: {dict(sorted(per_subcat.items()))}")
    print(f"  total issues: {total_issues}")
    print(f"  total warnings: {total_warnings}")

    fail = total_issues > 0 or (strict and total_warnings > 0)
    if fail:
        print("\nFAIL")
        return 1
    print("\nPASS")
    return 0


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("paths", nargs="*",
                    help="batch files (default: dave_anticonfab_*_batch_*.jsonl)")
    ap.add_argument("--strict", action="store_true",
                    help="exit non-zero on warnings too")
    args = ap.parse_args()

    paths = args.paths or sorted(glob.glob("dave_anticonfab_*_batch_*.jsonl"))
    if not paths:
        sys.exit("no batch files found")

    sys.exit(validate_files(paths, strict=args.strict))


if __name__ == "__main__":
    main()
