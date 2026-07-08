"""Validate the time-in-context round 4d batch.

Checks:
  - all lines parse as JSON
  - per-sample shape: _var, _cat, _subcat, _injected, messages[3]
  - all samples are SFT-SYS-T (system + user + assistant)
  - system content matches the runtime time-injection format:
        "Today is <Weekday>, <Month> <Day>, <Year>. It is <H>:<MM> <am|pm>.\n\n<DAVE_SYSTEM_PROMPT>"
  - assistant content has a <think>...</think> block followed by visible reply
  - voice rules: no em-dash (—), no banned phrases, no bullets, no model-name leaks
  - subcategory counts match plan (10/5/5/5/5/10)
  - placeholder <DAVE_SYSTEM_PROMPT> never leaks into user / assistant text
  - injection-format consistency (catches drift like "It is" missing)

Usage:
    python validate_dave_time_in_context.py [path/to/batch.jsonl]
"""
import json
import re
import sys
from collections import Counter

DEFAULT = "dave_time_in_context_batch_01.jsonl"
PLACEHOLDER = "<DAVE_SYSTEM_PROMPT>"

# Canonical injection format mirroring time_awareness::ambient_time_sentence.
# Captures weekday / month / day / year / hour / minute / am-pm.
TIME_RE = re.compile(
    r"^Today is (Mon|Tues|Wednes|Thurs|Fri|Satur|Sun)day, "
    r"(January|February|March|April|May|June|July|August|September|October|November|December) "
    r"\d{1,2}, "
    r"\d{4}\. It is "
    r"\d{1,2}:\d{2} "
    r"(am|pm)\."
)

BANNED_PHRASES = [
    "as an ai", "as a language model", "i'm sorry", "i apologize",
    "certainly", "of course,", "great question", "absolutely,",
    "let me know if", "i hope this helps", "i'm just",
    "system clock", "system time",  # the very thing this batch counters
]

EXPECTED_SUBCATS = {
    "clock-direct": 10,
    "date-direct": 5,
    "implicit-time": 5,
    "conversational": 5,
    "compound": 5,
    "incidental-no-leak": 10,
}

THINK_RE = re.compile(r"^<think>\n.+?\n</think>\n\n.+", re.DOTALL)


def main():
    path = sys.argv[1] if len(sys.argv) > 1 else DEFAULT
    errors = []
    warnings = []
    samples = []
    subcat_counts = Counter()
    var_counts = Counter()
    seen_user = set()
    seen_inject = set()

    with open(path, "r", encoding="utf-8") as f:
        for i, line in enumerate(f, 1):
            line = line.strip()
            if not line:
                continue
            try:
                obj = json.loads(line)
            except json.JSONDecodeError as e:
                errors.append(f"line {i}: JSON parse failed: {e}")
                continue
            samples.append((i, obj))

    if not samples:
        sys.exit(f"no samples in {path}")

    for i, obj in samples:
        # Top-level shape
        for k in ("_var", "_cat", "_subcat", "_injected", "messages"):
            if k not in obj:
                errors.append(f"line {i}: missing key {k!r}")
        if errors and any(f"line {i}:" in e for e in errors[-5:]):
            continue

        var_counts[obj["_var"]] += 1
        subcat_counts[obj["_subcat"]] += 1
        seen_inject.add(obj["_injected"])

        if obj["_var"] != "SFT-SYS-T":
            errors.append(f"line {i}: expected _var=SFT-SYS-T, got {obj['_var']}")
        if obj["_cat"] != "time-in-context":
            errors.append(f"line {i}: expected _cat=time-in-context, got {obj['_cat']}")
        if obj["_subcat"] not in EXPECTED_SUBCATS:
            errors.append(f"line {i}: unknown subcat {obj['_subcat']!r}")

        msgs = obj.get("messages", [])
        if len(msgs) != 3:
            errors.append(f"line {i}: expected 3 messages, got {len(msgs)}")
            continue
        sys_msg, usr_msg, ast_msg = msgs

        # System message
        if sys_msg.get("role") != "system":
            errors.append(f"line {i}: msg[0] is not system")
        sys_content = sys_msg.get("content", "")
        if PLACEHOLDER not in sys_content:
            errors.append(f"line {i}: system msg missing placeholder")
        # Format check: time prefix + \n\n + placeholder, exactly.
        prefix, sep, tail = sys_content.partition("\n\n" + PLACEHOLDER)
        if not sep or tail:
            errors.append(
                f"line {i}: system content not '<time>\\n\\n<DAVE_SYSTEM_PROMPT>' shape"
            )
        else:
            if not TIME_RE.match(prefix):
                errors.append(
                    f"line {i}: time prefix doesn't match runtime format: "
                    f"{prefix!r}"
                )

        # User message
        if usr_msg.get("role") != "user":
            errors.append(f"line {i}: msg[1] is not user")
        u = usr_msg.get("content", "")
        if not u.strip():
            errors.append(f"line {i}: empty user content")
        if PLACEHOLDER in u:
            errors.append(f"line {i}: placeholder leaked into user content")
        seen_user.add(u.lower().strip())

        # Assistant message
        if ast_msg.get("role") != "assistant":
            errors.append(f"line {i}: msg[2] is not assistant")
        a = ast_msg.get("content", "")
        if not THINK_RE.match(a):
            errors.append(f"line {i}: assistant missing <think>...</think> + reply")
        if PLACEHOLDER in a:
            errors.append(f"line {i}: placeholder leaked into assistant content")

        # Voice rules apply to BOTH think and visible reply.
        for piece_name, piece in (("user", u), ("assistant", a)):
            if "—" in piece:
                errors.append(f"line {i}: em-dash found in {piece_name}")
            if "•" in piece or re.search(r"^\s*[-*]\s+", piece, re.M):
                errors.append(f"line {i}: bullet/dash list found in {piece_name}")
            low = piece.lower()
            for ph in BANNED_PHRASES:
                if ph in low:
                    errors.append(f"line {i}: banned phrase {ph!r} in {piece_name}")
            if "qwen" in low or "llama" in low or "gpt-" in low:
                errors.append(f"line {i}: model name leak in {piece_name}")

    # Subcat distribution
    for sc, expected in EXPECTED_SUBCATS.items():
        actual = subcat_counts.get(sc, 0)
        if actual != expected:
            errors.append(
                f"subcat {sc}: expected {expected}, got {actual}"
            )
    extra = set(subcat_counts) - set(EXPECTED_SUBCATS)
    for sc in extra:
        errors.append(f"unexpected subcat {sc} ({subcat_counts[sc]} samples)")

    # Diversity heuristic — same user prompt repeating across many subcats is
    # fine for clock-direct (10x "what time is it" by design), but flag if the
    # collision density is excessive overall.
    if len(seen_user) < 25:
        warnings.append(
            f"only {len(seen_user)} distinct user prompts across {len(samples)} samples"
        )
    if len(seen_inject) < 25:
        warnings.append(
            f"only {len(seen_inject)} distinct injected times across {len(samples)} samples"
        )

    print(f"\n=== validating {path} ===")
    print(f"  total samples: {len(samples)}")
    print(f"  per variant: {dict(var_counts)}")
    print(f"  per subcat:  {dict(sorted(subcat_counts.items()))}")
    print(f"  distinct user prompts:     {len(seen_user)}")
    print(f"  distinct injected times:   {len(seen_inject)}")

    if warnings:
        print("\n  warnings:")
        for w in warnings:
            print(f"    ! {w}")

    if errors:
        print("\n  ERRORS:")
        for e in errors:
            print(f"    X {e}")
        sys.exit(f"\nVALIDATION FAILED: {len(errors)} errors\n")

    print("\n  ALL CHECKS PASSED.\n")


if __name__ == "__main__":
    main()
