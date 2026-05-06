"""
Validate dave_two_is_batch_*.jsonl files for fine-tune readiness.

Inspired by validate_tars.py from C:\\temp\\tars-training\\.

Checks per sample:
  STRUCTURE
    - valid JSON per line
    - required fields: _var, _cat, messages
    - _var in {SFT-SYS-T, SFT-NOSYS-T}
    - messages: SYS variants have [system, user, assistant]; NOSYS variants have [user, assistant]
    - assistant content matches /<think>\\n.*?\\n</think>\\n\\n.+/s
    - SYS variants: system.content == "<DAVE_SYSTEM_PROMPT>"
    - NOSYS variants: no system message present

  VOICE (Dave-soul-document compliance)
    - no em dashes (— or --) anywhere in content
    - reply contains no bullet/numbered list lines
    - reply does not start with affirmation ritual (Certainly, Of course, Great question, Absolutely, Sure, Happy to)
    - reply does not end with service ritual (Let me know, I hope this helps, anything else, feel free to ask)
    - reply contains no "As an AI" or "As a language model"
    - thinking trace length sanity-check (warn if > 600 chars)

  CROSS-BATCH
    - duplicate user prompts across batches flagged

USAGE
    python validate_dave_two_is.py                          # validate all batches in current dir
    python validate_dave_two_is.py dave_two_is_batch_03.jsonl  # one file
    python validate_dave_two_is.py --strict                 # exit non-zero on warnings too
"""
import argparse
import glob
import json
import re
import sys


PLACEHOLDER = "<DAVE_SYSTEM_PROMPT>"
ALLOWED_VARS = {"SFT-SYS-T", "SFT-NOSYS-T"}

THINK_RE = re.compile(r"^<think>\n(.*?)\n</think>\n\n(.+)$", re.DOTALL)

AFFIRMATION_PREFIXES = (
    "certainly", "of course", "great question", "absolutely",
    "sure", "happy to", "i'd be happy", "definitely",
    "wonderful", "fantastic", "excellent",
)

SERVICE_RITUALS = (
    "let me know if",
    "i hope this helps",
    "is there anything else",
    "anything else i can help",
    "feel free to ask",
    "happy to help",
    "let me know how",
)

AI_PREAMBLES = ("as an ai", "as a language model", "as an artificial intelligence")

MAX_THINK_CHARS = 600


def check_em_dashes(text):
    issues = []
    if "—" in text:
        issues.append(f"em dash (—): {text[:80]!r}")
    if "--" in text and "<--" not in text:
        # allow HTML comment markers etc, but `--` in prose is forbidden
        issues.append(f"double-hyphen (--): {text[:80]!r}")
    return issues


def check_reply_voice(reply):
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
    low_full = reply.lower()
    for ritual in SERVICE_RITUALS:
        if ritual in low_full:
            issues.append(f"service ritual: '{ritual}' in reply")
            break
    for pre in AI_PREAMBLES:
        if pre in low_full:
            issues.append(f"AI preamble: '{pre}' in reply")
    return issues


def check_sample(obj, line_no):
    issues = []
    warnings = []

    # Required fields
    for key in ("_var", "_cat", "messages"):
        if key not in obj:
            issues.append(f"missing field: {key}")
            return issues, warnings, None

    var = obj["_var"]
    if var not in ALLOWED_VARS:
        issues.append(f"unknown _var: {var}")
        return issues, warnings, None

    msgs = obj["messages"]

    # Structure check by variant
    if var == "SFT-SYS-T":
        if len(msgs) != 3:
            issues.append(f"SYS variant must have 3 messages, got {len(msgs)}")
            return issues, warnings, None
        if msgs[0].get("role") != "system":
            issues.append("first message must be system")
        elif msgs[0].get("content") != PLACEHOLDER:
            issues.append(f"system content must be {PLACEHOLDER!r}, got {msgs[0].get('content', '')[:40]!r}")
        if msgs[1].get("role") != "user":
            issues.append("second message must be user")
        if msgs[2].get("role") != "assistant":
            issues.append("third message must be assistant")
    else:  # NOSYS
        if len(msgs) != 2:
            issues.append(f"NOSYS variant must have 2 messages, got {len(msgs)}")
            return issues, warnings, None
        if msgs[0].get("role") != "user":
            issues.append("first message must be user")
        if msgs[1].get("role") != "assistant":
            issues.append("second message must be assistant")

    # Find user/assistant content
    user_content = None
    asst_content = None
    for m in msgs:
        r = m.get("role")
        c = m.get("content", "")
        if r == "user":
            user_content = c
        elif r == "assistant":
            asst_content = c
        # em-dash check on every content field
        issues.extend(f"{r}: {x}" for x in check_em_dashes(c))

    if asst_content is None:
        issues.append("no assistant content")
        return issues, warnings, user_content

    # Thinking + reply structure
    m_think = THINK_RE.match(asst_content)
    if not m_think:
        issues.append(f"assistant missing <think>...</think>...reply structure: {asst_content[:80]!r}")
        return issues, warnings, user_content

    think, reply = m_think.group(1), m_think.group(2)

    if len(think) > MAX_THINK_CHARS:
        warnings.append(f"think trace long ({len(think)} chars), might be evaluator-frame creep")

    issues.extend(check_reply_voice(reply))

    return issues, warnings, user_content


def validate_files(paths, strict=False):
    total_samples = 0
    total_issues = 0
    total_warnings = 0
    seen_prompts = {}  # prompt -> [(file, line)]
    per_var = {"SFT-SYS-T": 0, "SFT-NOSYS-T": 0}
    per_cat = {}

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

                issues, warnings, prompt = check_sample(obj, i)
                total_samples += 1
                per_var[obj.get("_var", "?")] = per_var.get(obj.get("_var", "?"), 0) + 1
                per_cat[obj.get("_cat", "?")] = per_cat.get(obj.get("_cat", "?"), 0) + 1

                if prompt:
                    seen_prompts.setdefault(prompt, []).append((path, i))

                for issue in issues:
                    print(f"  line {i}: ISSUE: {issue}")
                    file_issues += 1
                for w in warnings:
                    print(f"  line {i}: warn: {w}")
                    file_warnings += 1

        print(f"  -> {file_issues} issues, {file_warnings} warnings")
        total_issues += file_issues
        total_warnings += file_warnings

    # Cross-batch duplicate user prompts
    print("\n=== cross-batch uniqueness ===")
    dupes = {p: locs for p, locs in seen_prompts.items() if len(locs) > 1}
    if dupes:
        for p, locs in sorted(dupes.items()):
            print(f"  DUPLICATE prompt {p!r} appears in:")
            for f, ln in locs:
                print(f"    {f}:{ln}")
            total_issues += len(locs) - 1
    else:
        print(f"  all {len(seen_prompts)} user prompts unique across {len(paths)} files")

    print("\n=== summary ===")
    print(f"  total samples: {total_samples}")
    print(f"  per variant: {per_var}")
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
    ap.add_argument("paths", nargs="*", help="batch files (default: dave_two_is_batch_*.jsonl)")
    ap.add_argument("--strict", action="store_true", help="exit non-zero on warnings too")
    args = ap.parse_args()

    paths = args.paths or sorted(glob.glob("dave_two_is_batch_*.jsonl"))
    if not paths:
        sys.exit("no batch files found")

    sys.exit(validate_files(paths, strict=args.strict))


if __name__ == "__main__":
    main()
