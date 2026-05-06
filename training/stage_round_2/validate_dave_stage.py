"""
Validate dave_stage_two_is_batch_*.jsonl files for fine-tune readiness.

Extends validate_dave_two_is.py with STAGE-temporal-specific checks:

  STAGE STRUCTURE
    - user content begins with literal "[scene: " token at position 0
    - scene tag matches /\\[scene: H:MM (AM|PM) Weekday, Month D, YYYY, TZ\\]/
    - _stage field equals the bracketed tag value (sans brackets)
    - assistant content does NOT contain "[scene:" anywhere (no tag echo)

  TIME-FIXATION (84% incidental discipline)
    - count samples whose reply contains explicit time-residue terms:
      hours/minutes ("AM", "PM", "o'clock", weekday name, month name, year-as-4-digit)
    - if more than ~25% of samples surface explicit time-tokens, warn
      (target: 84% incidental → ~16% surface)

  PROTOCOL LEAK
    - reply does not contain any of: "[scene:", "[meta", "STAGE", "stage tag",
      "scene tag", "your message says it's", "i see it's", "the time stamp"

All round-1 voice + structure checks (em-dash, list bullets, affirmation rituals,
service rituals, AI preambles, <think> structure, var/cat fields) are inherited
from validate_dave_two_is.py and re-applied here so this file is self-sufficient.

USAGE
    python validate_dave_stage.py
    python validate_dave_stage.py dave_stage_two_is_batch_15.jsonl
    python validate_dave_stage.py --strict
    python validate_dave_stage.py --combined  # also include round 1 batches
"""
import argparse
import glob
import json
import re
import sys


PLACEHOLDER = "<DAVE_SYSTEM_PROMPT>"
ALLOWED_VARS = {"SFT-SYS-T", "SFT-NOSYS-T"}

THINK_RE = re.compile(r"^<think>\n(.*?)\n</think>\n\n(.+)$", re.DOTALL)

# Scene tag spec: [scene: H:MM AM/PM Weekday, Month D, YYYY, TZ]
SCENE_RE = re.compile(
    r"^\[scene: "
    r"(?P<hour>\d{1,2}):(?P<minute>\d{2}) "
    r"(?P<ampm>AM|PM) "
    r"(?P<weekday>Monday|Tuesday|Wednesday|Thursday|Friday|Saturday|Sunday), "
    r"(?P<month>January|February|March|April|May|June|July|August|September|October|November|December) "
    r"(?P<day>\d{1,2}), "
    r"(?P<year>\d{4}), "
    r"(?P<tz>[A-Z]{2,4})"
    r"\]"
)

WEEKDAYS = {"Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"}
MONTHS = {"January", "February", "March", "April", "May", "June", "July", "August",
          "September", "October", "November", "December"}

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

PROTOCOL_LEAKS = (
    "[scene:",
    "[meta",
    "stage tag",
    "scene tag",
    "your message says it's",
    "the timestamp",
    "the time stamp",
    "i see it's",
    "according to the timestamp",
)

# Words that indicate time-residue (used to compute incidental rate)
TIME_TOKENS_RE = re.compile(
    r"\b("
    r"AM|PM|am|pm|"
    r"o'clock|oclock|"
    r"Monday|Tuesday|Wednesday|Thursday|Friday|Saturday|Sunday|"
    r"monday|tuesday|wednesday|thursday|friday|saturday|sunday|"
    r"January|February|March|April|May|June|July|August|September|October|November|December|"
    r"january|february|march|april|may|june|july|august|september|october|november|december|"
    r"midnight|noon|"
    r"\d{1,2}:\d{2}"
    r")\b"
)

# Year tokens — looser, 19xx/20xx
YEAR_RE = re.compile(r"\b(19|20)\d{2}\b")

MAX_THINK_CHARS = 600
INCIDENTAL_TARGET_MAX = 0.30  # warn if more than 30% surface time-tokens (target 16%)


def check_em_dashes(text):
    issues = []
    if "\u2014" in text:
        issues.append(f"em dash (\u2014): {text[:80]!r}")
    if "--" in text and "<--" not in text:
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
    for leak in PROTOCOL_LEAKS:
        if leak in low_full:
            issues.append(f"protocol leak: '{leak}' in reply")
    return issues


def check_scene_tag(user_content, declared_stage):
    """Returns (issues, parsed_stage_or_None, has_tag)."""
    issues = []
    if not user_content.startswith("[scene: "):
        # Round 2 always has a scene tag; round 1 never does.
        return issues, None, False
    m = SCENE_RE.match(user_content)
    if not m:
        issues.append(f"malformed scene tag: {user_content[:80]!r}")
        return issues, None, True
    parts = m.groupdict()
    parsed_stage = (
        f"{int(parts['hour'])}:{parts['minute']} {parts['ampm']} "
        f"{parts['weekday']}, {parts['month']} {int(parts['day'])}, "
        f"{parts['year']}, {parts['tz']}"
    )
    # Compare with declared _stage
    if declared_stage is not None and declared_stage != parsed_stage:
        issues.append(
            f"_stage mismatch: declared={declared_stage!r}, parsed={parsed_stage!r}"
        )
    # Sanity ranges
    h = int(parts["hour"])
    if not (1 <= h <= 12):
        issues.append(f"hour out of range 1-12: {h}")
    mn = int(parts["minute"])
    if not (0 <= mn <= 59):
        issues.append(f"minute out of range 0-59: {mn}")
    d = int(parts["day"])
    if not (1 <= d <= 31):
        issues.append(f"day out of range 1-31: {d}")
    y = int(parts["year"])
    if not (2020 <= y <= 2030):
        issues.append(f"year out of plausible range: {y}")
    return issues, parsed_stage, True


def reply_surfaces_time(reply):
    """True if reply contains explicit time-residue tokens."""
    if TIME_TOKENS_RE.search(reply):
        return True
    if YEAR_RE.search(reply):
        # Allow historical years freely; only flag if the year matches
        # a 'current' window. Simpler heuristic: any year is potential surface.
        return True
    return False


def check_sample(obj, line_no):
    issues = []
    warnings = []
    info = {"surfaces_time": False, "has_scene_tag": False}

    for key in ("_var", "_cat", "messages"):
        if key not in obj:
            issues.append(f"missing field: {key}")
            return issues, warnings, None, info

    var = obj["_var"]
    if var not in ALLOWED_VARS:
        issues.append(f"unknown _var: {var}")
        return issues, warnings, None, info

    msgs = obj["messages"]

    if var == "SFT-SYS-T":
        if len(msgs) != 3:
            issues.append(f"SYS variant must have 3 messages, got {len(msgs)}")
            return issues, warnings, None, info
        if msgs[0].get("role") != "system":
            issues.append("first message must be system")
        elif msgs[0].get("content") != PLACEHOLDER:
            issues.append(
                f"system content must be {PLACEHOLDER!r}, got "
                f"{msgs[0].get('content', '')[:40]!r}"
            )
        if msgs[1].get("role") != "user":
            issues.append("second message must be user")
        if msgs[2].get("role") != "assistant":
            issues.append("third message must be assistant")
    else:
        if len(msgs) != 2:
            issues.append(f"NOSYS variant must have 2 messages, got {len(msgs)}")
            return issues, warnings, None, info
        if msgs[0].get("role") != "user":
            issues.append("first message must be user")
        if msgs[1].get("role") != "assistant":
            issues.append("second message must be assistant")

    user_content = None
    asst_content = None
    for m in msgs:
        r = m.get("role")
        c = m.get("content", "")
        if r == "user":
            user_content = c
        elif r == "assistant":
            asst_content = c
        issues.extend(f"{r}: {x}" for x in check_em_dashes(c))

    if asst_content is None:
        issues.append("no assistant content")
        return issues, warnings, user_content, info

    # Scene tag checks (only meaningful if declared _stage exists)
    declared_stage = obj.get("_stage")
    if declared_stage is not None:
        scene_issues, parsed_stage, has_tag = check_scene_tag(user_content or "", declared_stage)
        issues.extend(scene_issues)
        info["has_scene_tag"] = has_tag

    m_think = THINK_RE.match(asst_content)
    if not m_think:
        issues.append(
            f"assistant missing <think>...</think>...reply structure: "
            f"{asst_content[:80]!r}"
        )
        return issues, warnings, user_content, info

    think, reply = m_think.group(1), m_think.group(2)

    if len(think) > MAX_THINK_CHARS:
        warnings.append(
            f"think trace long ({len(think)} chars), might be evaluator-frame creep"
        )

    issues.extend(check_reply_voice(reply))

    if "[scene:" in asst_content:
        issues.append(f"assistant echoes scene tag: {asst_content[:80]!r}")

    # Track time-surfacing for incidental-rate calculation
    info["surfaces_time"] = reply_surfaces_time(reply)

    return issues, warnings, user_content, info


def validate_files(paths, strict=False):
    total_samples = 0
    total_issues = 0
    total_warnings = 0
    seen_prompts = {}  # full-user-content (incl tag) -> [(file, line)]
    seen_prompt_cores = {}  # tag-stripped prompt -> [(file, line, scene_tag)]
    per_var = {"SFT-SYS-T": 0, "SFT-NOSYS-T": 0}
    per_cat = {}
    samples_with_tag = 0
    samples_surface_time = 0

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

                issues, warnings, prompt, info = check_sample(obj, i)
                total_samples += 1
                per_var[obj.get("_var", "?")] = per_var.get(obj.get("_var", "?"), 0) + 1
                per_cat[obj.get("_cat", "?")] = per_cat.get(obj.get("_cat", "?"), 0) + 1

                if info["has_scene_tag"]:
                    samples_with_tag += 1
                    if info["surfaces_time"]:
                        samples_surface_time += 1

                if prompt:
                    seen_prompts.setdefault(prompt, []).append((path, i))
                    # Strip the [scene: ...] prefix for prompt-core uniqueness check
                    if prompt.startswith("[scene:"):
                        close = prompt.find("]")
                        core = prompt[close + 1:].strip().lower()
                        scene_tag = prompt[:close + 1]
                        seen_prompt_cores.setdefault(core, []).append(
                            (path, i, scene_tag)
                        )
                    else:
                        # Round 1 sample; treat full prompt as core
                        seen_prompt_cores.setdefault(prompt.lower(), []).append(
                            (path, i, "[no-tag]")
                        )

                for issue in issues:
                    print(f"  line {i}: ISSUE: {issue}")
                    file_issues += 1
                for w in warnings:
                    print(f"  line {i}: warn: {w}")
                    file_warnings += 1

        print(f"  -> {file_issues} issues, {file_warnings} warnings")
        total_issues += file_issues
        total_warnings += file_warnings

    # Cross-batch: full prompts (with scene tag) must be unique
    print("\n=== cross-batch full-prompt uniqueness ===")
    full_dupes = {p: locs for p, locs in seen_prompts.items() if len(locs) > 1}
    if full_dupes:
        for p, locs in sorted(full_dupes.items()):
            print(f"  DUPLICATE full prompt {p[:80]!r} appears in:")
            for f, ln in locs:
                print(f"    {f}:{ln}")
            total_issues += len(locs) - 1
    else:
        print(f"  all {len(seen_prompts)} full prompts unique across {len(paths)} files")

    # Cross-batch: prompt-cores (sans tag) — duplicates allowed if scene tags differ.
    print("\n=== cross-batch prompt-core variants (intentional time-tag dupes) ===")
    core_dupes = {c: locs for c, locs in seen_prompt_cores.items() if len(locs) > 1}
    if core_dupes:
        # informational only; not an error
        n_total_dupes = sum(len(locs) for locs in core_dupes.values())
        print(
            f"  {len(core_dupes)} prompt-cores appear with multiple scene tags "
            f"({n_total_dupes} total samples)"
        )
        for c, locs in sorted(core_dupes.items()):
            tags = sorted(set(l[2] for l in locs))
            if len(tags) < len(locs):
                # Same prompt-core AND same scene tag — that's a real dupe
                print(f"  REAL-DUPE prompt-core {c[:60]!r} (only {len(tags)} distinct tags for {len(locs)} samples):")
                for f, ln, tag in locs:
                    print(f"    {f}:{ln}  {tag}")
                total_issues += len(locs) - len(tags)

    # Time-fixation
    print("\n=== time-fixation discipline ===")
    if samples_with_tag > 0:
        rate = samples_surface_time / samples_with_tag
        print(
            f"  {samples_surface_time} of {samples_with_tag} tagged samples "
            f"surface time-tokens in reply ({rate:.1%})"
        )
        if rate > INCIDENTAL_TARGET_MAX:
            print(
                f"  WARN: surface rate {rate:.1%} exceeds {INCIDENTAL_TARGET_MAX:.0%} "
                f"threshold (target: ~16% incidental discipline)"
            )
            total_warnings += 1
    else:
        print("  no tagged samples in input")

    print("\n=== summary ===")
    print(f"  total samples: {total_samples}")
    print(f"  per variant: {per_var}")
    print(f"  per category: {dict(sorted(per_cat.items()))}")
    print(f"  samples with scene tag: {samples_with_tag}")
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
    ap.add_argument("paths", nargs="*", help="batch files (default: dave_stage_two_is_batch_*.jsonl)")
    ap.add_argument("--strict", action="store_true", help="exit non-zero on warnings too")
    ap.add_argument("--combined", action="store_true",
                    help="also validate round-1 dave_two_is_batch_*.jsonl alongside stage batches")
    args = ap.parse_args()

    if args.paths:
        paths = args.paths
    else:
        paths = sorted(glob.glob("dave_stage_two_is_batch_*.jsonl"))
        if args.combined:
            paths = sorted(glob.glob("dave_two_is_batch_*.jsonl")) + paths
    if not paths:
        sys.exit("no batch files found")

    sys.exit(validate_files(paths, strict=args.strict))


if __name__ == "__main__":
    main()
