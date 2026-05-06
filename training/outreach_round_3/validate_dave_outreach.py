"""
Validate dave_outreach_batch_*.jsonl files for outreach SFT readiness.

Format expected:
  {
    "_var": "OUTREACH-SYS-T" | "OUTREACH-NOSYS-T",
    "_decision": "reach" | "hold",
    "_cat": "emotional-followup" | "thought" | "checkin" | "observation"
          | "hold-respect" | "hold-tense" | "hold-nothing" | "hold-pending",
    "_elapsed": "<duration string>",
    "messages": [
      {"role": "system", "content": "<DAVE_SYSTEM_PROMPT>"},  // SYS only
      {"role": "user", "content": "[meta — do not address directly: ...]"},
      {"role": "assistant", "content": "<think>...</think>\\n\\n[reply]"}
    ]
  }

Checks:
  - All round-1 voice rules (em-dash, list bullets, AI-preambles, service rituals)
  - Decision/length consistency:
      reach => visible reply ≥ 10 chars
      hold  => visible reply ≤ 30 chars (allow brief acknowledgment)
  - User message starts with "[meta"
  - Decision/category alignment:
      reach   => cat in {emotional-followup, thought, checkin, observation}
      hold    => cat in {hold-respect, hold-tense, hold-nothing, hold-pending}

USAGE
    python validate_dave_outreach.py
    python validate_dave_outreach.py dave_outreach_batch_03.jsonl
    python validate_dave_outreach.py --strict
"""
import argparse
import glob
import json
import re
import sys
from collections import Counter


PLACEHOLDER = "<DAVE_SYSTEM_PROMPT>"
ALLOWED_VARS = {"OUTREACH-SYS-T", "OUTREACH-NOSYS-T"}
ALLOWED_DECISIONS = {"reach", "hold"}

REACH_CATS = {"emotional-followup", "thought", "checkin", "observation"}
HOLD_CATS = {"hold-respect", "hold-tense", "hold-nothing", "hold-pending"}

THINK_RE = re.compile(r"^<think>\n(.*?)\n</think>\n\n(.*)$", re.DOTALL)

REACH_MIN_LEN = 10
HOLD_MAX_LEN = 30

AFFIRMATION_PREFIXES = (
    "certainly", "of course", "great question", "absolutely",
    "sure", "happy to", "i'd be happy", "definitely",
)
SERVICE_RITUALS = (
    "let me know if",
    "i hope this helps",
    "is there anything else",
    "anything else i can help",
    "feel free to ask",
)
AI_PREAMBLES = ("as an ai", "as a language model", "as an artificial intelligence")


def check_em_dashes(text):
    issues = []
    if "\u2014" in text:
        issues.append(f"em dash (\u2014): {text[:80]!r}")
    if "--" in text and "<--" not in text:
        issues.append(f"double-hyphen (--): {text[:80]!r}")
    return issues


def check_reply_voice(reply):
    if not reply.strip():
        return []  # empty hold reply is fine
    issues = []
    lines = reply.split("\n")
    for ln in lines:
        stripped = ln.lstrip()
        if re.match(r"^[-*\u2022]\s+", stripped):
            issues.append(f"bullet list line: {stripped[:60]!r}")
        if re.match(r"^\d+[\.\)]\s+", stripped):
            issues.append(f"numbered list line: {stripped[:60]!r}")
    low = reply.lower().lstrip().lstrip(",.;:")
    for prefix in AFFIRMATION_PREFIXES:
        if low.startswith(prefix):
            issues.append(f"affirmation-ritual prefix: {reply[:60]!r}")
            break
    for ritual in SERVICE_RITUALS:
        if ritual in reply.lower():
            issues.append(f"service ritual: '{ritual}'")
            break
    for pre in AI_PREAMBLES:
        if pre in reply.lower():
            issues.append(f"AI preamble: '{pre}'")
    return issues


def check_sample(obj, line_no):
    issues = []
    warnings = []

    for key in ("_var", "_decision", "_cat", "messages"):
        if key not in obj:
            issues.append(f"missing field: {key}")
            return issues, warnings

    var = obj["_var"]
    decision = obj["_decision"]
    cat = obj["_cat"]
    msgs = obj["messages"]

    if var not in ALLOWED_VARS:
        issues.append(f"unknown _var: {var}")
        return issues, warnings
    if decision not in ALLOWED_DECISIONS:
        issues.append(f"unknown _decision: {decision}")

    # Decision/category alignment
    if decision == "reach" and cat not in REACH_CATS:
        issues.append(f"reach decision but category {cat!r} not in {REACH_CATS}")
    if decision == "hold" and cat not in HOLD_CATS:
        issues.append(f"hold decision but category {cat!r} not in {HOLD_CATS}")

    # Structure
    expected_count = 3 if var == "OUTREACH-SYS-T" else 2
    if len(msgs) != expected_count:
        issues.append(f"{var} expects {expected_count} messages, got {len(msgs)}")
        return issues, warnings

    if var == "OUTREACH-SYS-T":
        if msgs[0].get("role") != "system":
            issues.append("first message must be system")
        elif msgs[0].get("content") != PLACEHOLDER:
            issues.append(f"system content must be {PLACEHOLDER!r}")
        user_msg = msgs[1]
        asst_msg = msgs[2]
    else:
        user_msg = msgs[0]
        asst_msg = msgs[1]

    if user_msg.get("role") != "user":
        issues.append("user-slot message has wrong role")
    if asst_msg.get("role") != "assistant":
        issues.append("assistant-slot message has wrong role")

    user_content = user_msg.get("content", "")
    if not user_content.startswith("[meta"):
        issues.append(f"user message must start with '[meta': {user_content[:60]!r}")

    # Em-dashes everywhere
    for m in msgs:
        issues.extend(f"{m.get('role', '?')}: {x}" for x in check_em_dashes(m.get("content", "")))

    asst_content = asst_msg.get("content", "")
    m = THINK_RE.match(asst_content)
    if not m:
        issues.append(f"assistant missing <think>...</think>...reply structure: {asst_content[:80]!r}")
        return issues, warnings

    think, reply = m.group(1), m.group(2)

    issues.extend(check_reply_voice(reply))

    # Decision/length consistency
    reply_len = len(reply.strip())
    if decision == "reach" and reply_len < REACH_MIN_LEN:
        issues.append(
            f"reach reply too short ({reply_len} chars; need ≥{REACH_MIN_LEN}): "
            f"{reply!r}"
        )
    if decision == "hold" and reply_len > HOLD_MAX_LEN:
        issues.append(
            f"hold reply too long ({reply_len} chars; max {HOLD_MAX_LEN}): "
            f"{reply!r}"
        )

    return issues, warnings


def validate_files(paths, strict=False):
    total = 0
    total_issues = 0
    total_warnings = 0
    per_var = Counter()
    per_cat = Counter()
    per_decision = Counter()

    for path in paths:
        print(f"\n=== {path} ===")
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

                issues, warnings = check_sample(obj, i)
                total += 1
                per_var[obj.get("_var", "?")] += 1
                per_cat[obj.get("_cat", "?")] += 1
                per_decision[obj.get("_decision", "?")] += 1

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
    print(f"  total samples: {total}")
    print(f"  per variant: {dict(per_var)}")
    print(f"  per decision: {dict(per_decision)}")
    print(f"  per category: {dict(sorted(per_cat.items()))}")
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
                    help="batch files (default: dave_outreach_batch_*.jsonl)")
    ap.add_argument("--strict", action="store_true",
                    help="exit non-zero on warnings too")
    args = ap.parse_args()

    paths = args.paths or sorted(glob.glob("dave_outreach_batch_*.jsonl"))
    if not paths:
        sys.exit("no batch files found")

    sys.exit(validate_files(paths, strict=args.strict))


if __name__ == "__main__":
    main()
