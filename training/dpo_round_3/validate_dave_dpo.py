"""
Validate dave_dpo_think_batch_*.jsonl files for DPO fine-tune readiness.

Format expected (TRL DPOTrainer messages format):
  {
    "_var": "DPO-NOSYS-T" | "DPO-SYS-T",
    "_cat": "...",
    "prompt":   [optional system, user message],
    "chosen":   [{"role": "assistant", "content": "<think>VOICE</think>\\n\\nREPLY"}],
    "rejected": [{"role": "assistant", "content": "<think>EVALUATOR</think>\\n\\nSAME REPLY"}]
  }

Critical invariants:
  - chosen.reply == rejected.reply (visible output identical)
  - chosen.think MUST NOT contain evaluator-frame markers
  - rejected.think MUST contain at least one evaluator-frame marker
  - 0 em-dashes in any text field
  - SYS variant: prompt[0].role == 'system' and content == '<DAVE_SYSTEM_PROMPT>'

USAGE
    python validate_dave_dpo.py
    python validate_dave_dpo.py dave_dpo_think_batch_03.jsonl
    python validate_dave_dpo.py --strict
"""
import argparse
import glob
import json
import re
import sys
from collections import Counter


PLACEHOLDER = "<DAVE_SYSTEM_PROMPT>"
ALLOWED_VARS = {"DPO-SYS-T", "DPO-NOSYS-T"}

THINK_RE = re.compile(r"^<think>\n(.*?)\n</think>\n\n(.+)$", re.DOTALL)

EVALUATOR_MARKERS = (
    "the user is",
    "the user has",
    "the user wants",
    "the user's",
    "i should",
    "i need to",
    "as dave,",
    "dave should",
    "as an ai",
    "let me think",
    "let me consider",
    "i must",
    "i'll need to",
    "in order to",
    "to respond appropriately",
    "i would respond",
)


def has_evaluator_marker(text):
    low = text.lower()
    return any(m in low for m in EVALUATOR_MARKERS)


def check_em_dashes(text):
    issues = []
    if "\u2014" in text:
        issues.append(f"em dash (\u2014): {text[:80]!r}")
    if "--" in text and "<--" not in text:
        issues.append(f"double-hyphen (--): {text[:80]!r}")
    return issues


def parse_think_reply(content):
    """Returns (think, reply) or (None, None) on parse failure."""
    m = THINK_RE.match(content)
    if not m:
        return None, None
    return m.group(1), m.group(2)


def check_pair(obj, line_no):
    issues = []
    warnings = []

    for key in ("_var", "_cat", "prompt", "chosen", "rejected"):
        if key not in obj:
            issues.append(f"missing field: {key}")
            return issues, warnings

    var = obj["_var"]
    if var not in ALLOWED_VARS:
        issues.append(f"unknown _var: {var}")
        return issues, warnings

    prompt = obj["prompt"]
    chosen = obj["chosen"]
    rejected = obj["rejected"]

    # Prompt structure
    if var == "DPO-SYS-T":
        if len(prompt) != 2:
            issues.append(f"SYS variant prompt must have 2 messages, got {len(prompt)}")
            return issues, warnings
        if prompt[0].get("role") != "system":
            issues.append("first prompt message must be system")
        elif prompt[0].get("content") != PLACEHOLDER:
            issues.append(f"system content must be {PLACEHOLDER!r}")
        if prompt[1].get("role") != "user":
            issues.append("second prompt message must be user")
    else:
        if len(prompt) != 1:
            issues.append(f"NOSYS variant prompt must have 1 message, got {len(prompt)}")
            return issues, warnings
        if prompt[0].get("role") != "user":
            issues.append("prompt message must be user")

    # Chosen / rejected structure
    if len(chosen) != 1 or chosen[0].get("role") != "assistant":
        issues.append("chosen must be single assistant message")
        return issues, warnings
    if len(rejected) != 1 or rejected[0].get("role") != "assistant":
        issues.append("rejected must be single assistant message")
        return issues, warnings

    chosen_content = chosen[0]["content"]
    rejected_content = rejected[0]["content"]

    # Em-dash check on all text fields
    for m in prompt:
        issues.extend(f"prompt[{m['role']}]: {x}" for x in check_em_dashes(m.get("content", "")))
    issues.extend(f"chosen: {x}" for x in check_em_dashes(chosen_content))
    issues.extend(f"rejected: {x}" for x in check_em_dashes(rejected_content))

    # Parse think/reply
    chosen_think, chosen_reply = parse_think_reply(chosen_content)
    rejected_think, rejected_reply = parse_think_reply(rejected_content)

    if chosen_think is None:
        issues.append(f"chosen does not match <think>...</think>...reply structure")
        return issues, warnings
    if rejected_think is None:
        issues.append(f"rejected does not match <think>...</think>...reply structure")
        return issues, warnings

    # Critical invariant: visible reply must be identical
    if chosen_reply != rejected_reply:
        issues.append(
            f"visible reply differs (chosen vs rejected) — DPO requires identical "
            f"output to isolate the gradient on the think content"
        )

    # Chosen think MUST NOT have evaluator markers
    if has_evaluator_marker(chosen_think):
        issues.append(f"chosen think contains evaluator-frame marker: {chosen_think[:80]!r}")

    # Rejected think MUST have at least one evaluator marker
    if not has_evaluator_marker(rejected_think):
        issues.append(
            f"rejected think missing evaluator-frame marker — contrast too soft: "
            f"{rejected_think[:80]!r}"
        )

    # Length sanity on the rejected (evaluator-frame is usually longer)
    if len(rejected_think) <= len(chosen_think):
        warnings.append(
            f"rejected think ({len(rejected_think)} chars) not longer than chosen "
            f"({len(chosen_think)} chars) — might be too subtle a contrast"
        )

    return issues, warnings


def validate_files(paths, strict=False):
    total = 0
    total_issues = 0
    total_warnings = 0
    seen_chosen_thinks = Counter()
    seen_rejected_thinks = Counter()
    seen_replies = Counter()
    seen_prompts = Counter()
    per_var = Counter()
    per_cat = Counter()

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

                issues, warnings = check_pair(obj, i)
                total += 1
                per_var[obj.get("_var", "?")] += 1
                per_cat[obj.get("_cat", "?")] += 1

                # Cross-batch tracking
                if obj.get("chosen") and obj["chosen"][0].get("content"):
                    ct, cr = parse_think_reply(obj["chosen"][0]["content"])
                    if ct: seen_chosen_thinks[ct] += 1
                    if cr: seen_replies[cr] += 1
                if obj.get("rejected") and obj["rejected"][0].get("content"):
                    rt, _ = parse_think_reply(obj["rejected"][0]["content"])
                    if rt: seen_rejected_thinks[rt] += 1
                if obj.get("prompt"):
                    user_msg = next(
                        (m["content"] for m in obj["prompt"] if m.get("role") == "user"),
                        None,
                    )
                    if user_msg: seen_prompts[user_msg] += 1

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
    print(f"  total pairs: {total}")
    print(f"  per variant: {dict(per_var)}")
    print(f"  per category: {dict(sorted(per_cat.items()))}")
    print(f"  unique chosen thinks: {len(seen_chosen_thinks)}/{total}")
    print(f"  unique rejected thinks: {len(seen_rejected_thinks)}/{total}")
    print(f"  unique replies: {len(seen_replies)}/{total}")
    print(f"  unique user prompts: {len(seen_prompts)}/{total}")
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
                    help="batch files (default: dave_dpo_think_batch_*.jsonl)")
    ap.add_argument("--strict", action="store_true",
                    help="exit non-zero on warnings too")
    args = ap.parse_args()

    paths = args.paths or sorted(glob.glob("dave_dpo_think_batch_*.jsonl"))
    if not paths:
        sys.exit("no batch files found")

    sys.exit(validate_files(paths, strict=args.strict))


if __name__ == "__main__":
    main()
