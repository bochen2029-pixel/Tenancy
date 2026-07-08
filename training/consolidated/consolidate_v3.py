"""Consolidate ALL Dave training corpora through round 4d into v3 SFT + DPO files.

This is the v3 successor to consolidate_combined.py. The two important changes
versus v2:

  1. Placeholder expansion is SUBSTRING, not exact-match. Round 4d adds a
     time-prefix in front of the placeholder ("Today is <weekday>, ... It is
     <h>:<mm> <am|pm>.\\n\\n<DAVE_SYSTEM_PROMPT>") to mirror the runtime
     time_awareness::system_prompt_with_time output. The v2 expander did
     `content == PLACEHOLDER`, which silently dropped the expansion for any
     prefixed system message. The v3 expander does
     `content.replace(PLACEHOLDER, canonical)`, so the time prefix survives
     into the trained input.

  2. Coverage: pulls every SFT round (1, 2, 3b, 3c, 4a, 4b, 4c, 4d) into one
     SFT JSONL, and every DPO round (3a, 4a-DPO) into one DPO JSONL. v2 only
     covered rounds 1+2.

Round map:
    SFT
      round 1   two_is_round_1/batches/dave_two_is_batch_*.jsonl       (500)
      round 2   stage_round_2/dave_stage_two_is_batch_*.jsonl          (500)
      round 3b  outreach_round_3/dave_outreach_batch_*.jsonl           (300)
      round 3c  cadence_round_3c/dave_cadence_batch_*.jsonl            (300)
      round 4a  anti_confab_round_4a/dave_anticonfab_sft_batch_*.jsonl (100)
      round 4b  journal_round_4b/dave_journal_*_batch_*.jsonl          (150)
      round 4c  holds_round_4c/dave_holds_batch_*.jsonl                (100)
      round 4d  time_in_context_round_4d/dave_time_in_context_batch_*.jsonl (40)
                                                                        ----
                                                              SFT total ~1990

    DPO
      round 3a  dpo_round_3/dave_dpo_think_batch_*.jsonl               (300)
      round 4a  anti_confab_round_4a/dave_anticonfab_dpo_batch_*.jsonl (100)
                                                                        ----
                                                              DPO total ~400

The numbers above are nominal. Actual count is logged at the end of each run.

OUTPUT
    dave_v3_sft.jsonl    (in --out-dir, default: this directory)
    dave_v3_dpo.jsonl    (in --out-dir)

USAGE
    python consolidate_v3.py
    python consolidate_v3.py --no-expand            # keep <DAVE_SYSTEM_PROMPT> literal
    python consolidate_v3.py --shuffle              # randomize across all rounds
    python consolidate_v3.py --sft-only
    python consolidate_v3.py --dpo-only
    python consolidate_v3.py --skip-rounds 4d       # exclude one or more rounds
    python consolidate_v3.py --root C:/DAVE/training
    python consolidate_v3.py --dry-run              # count + report, write nothing
"""
import argparse
import glob
import json
import os
import random
import sys
from collections import Counter

PLACEHOLDER = "<DAVE_SYSTEM_PROMPT>"

# Each SFT round: (round-id, glob pattern relative to --root)
SFT_PATTERNS = [
    ("1",  "two_is_round_1/batches/dave_two_is_batch_*.jsonl"),
    ("2",  "stage_round_2/dave_stage_two_is_batch_*.jsonl"),
    ("3b", "outreach_round_3/dave_outreach_batch_*.jsonl"),
    ("3c", "cadence_round_3c/dave_cadence_batch_*.jsonl"),
    ("4a", "anti_confab_round_4a/dave_anticonfab_sft_batch_*.jsonl"),
    ("4b", "journal_round_4b/dave_journal_*_batch_*.jsonl"),
    ("4c", "holds_round_4c/dave_holds_batch_*.jsonl"),
    ("4d", "time_in_context_round_4d/dave_time_in_context_batch_*.jsonl"),
]

# Each DPO round: (round-id, glob pattern relative to --root)
DPO_PATTERNS = [
    ("3a", "dpo_round_3/dave_dpo_think_batch_*.jsonl"),
    ("4a", "anti_confab_round_4a/dave_anticonfab_dpo_batch_*.jsonl"),
]

DEFAULT_CANONICAL = "../two_is_round_1/dave_canonical_sys_prompt.txt"


# ---------------------------------------------------------------------------
# Placeholder expansion — SUBSTRING (v3) vs exact-match (v2)
# ---------------------------------------------------------------------------
def expand_messages_inplace(msg_list, sys_prompt):
    """Walk a list of {role, content} dicts. For any content containing the
    PLACEHOLDER literal, substitute the canonical system prompt in place.
    Substring replace lets a wrapper survive — e.g. round 4d's time-prefix."""
    out = []
    for m in msg_list:
        if not isinstance(m, dict):
            out.append(m)
            continue
        c = m.get("content", "")
        if isinstance(c, str) and PLACEHOLDER in c:
            new_m = dict(m)
            new_m["content"] = c.replace(PLACEHOLDER, sys_prompt)
            out.append(new_m)
        else:
            out.append(m)
    return out


def expand_sft_record(obj, sys_prompt):
    if "messages" in obj:
        obj["messages"] = expand_messages_inplace(obj["messages"], sys_prompt)
    return obj


def expand_dpo_record(obj, sys_prompt):
    for k in ("prompt", "chosen", "rejected"):
        if k in obj and isinstance(obj[k], list):
            obj[k] = expand_messages_inplace(obj[k], sys_prompt)
    return obj


# ---------------------------------------------------------------------------
# Loading
# ---------------------------------------------------------------------------
def load_jsonl(path):
    rows = []
    with open(path, "r", encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            rows.append(json.loads(line))
    return rows


def load_round(root, round_id, pattern, sys_prompt, expand, kind):
    """kind ∈ {"sft", "dpo"}; only matters for the placeholder expander used."""
    full_glob = os.path.join(root, pattern)
    paths = sorted(glob.glob(full_glob))
    samples = []
    per_batch = []
    for p in paths:
        rows = load_jsonl(p)
        if expand:
            for r in rows:
                if kind == "sft":
                    expand_sft_record(r, sys_prompt)
                else:
                    expand_dpo_record(r, sys_prompt)
        for r in rows:
            r["_round"] = round_id
        samples.extend(rows)
        per_batch.append((os.path.relpath(p, root), len(rows)))
    return samples, per_batch


# ---------------------------------------------------------------------------
# Stats
# ---------------------------------------------------------------------------
def stats(samples, kind):
    per_round = Counter(s.get("_round", "?") for s in samples)
    per_var = Counter(s.get("_var", "?") for s in samples)
    per_cat = Counter(s.get("_cat", "?") for s in samples)
    placeholder_remaining = 0
    for s in samples:
        if kind == "sft":
            scan = s.get("messages", [])
        else:
            scan = (s.get("prompt", []) or []) + (s.get("chosen", []) or []) \
                + (s.get("rejected", []) or [])
        for m in scan:
            if isinstance(m, dict) and PLACEHOLDER in (m.get("content") or ""):
                placeholder_remaining += 1
                break
    return {
        "per_round": dict(per_round),
        "per_var": dict(per_var),
        "per_cat": dict(sorted(per_cat.items())),
        "placeholder_remaining": placeholder_remaining,
    }


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------
def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--root", default=os.path.abspath(os.path.join(os.path.dirname(__file__), "..")),
                    help="training root (default: parent of consolidated/)")
    ap.add_argument("--canonical", default=None,
                    help="canonical system prompt path "
                         "(default: <root>/two_is_round_1/dave_canonical_sys_prompt.txt)")
    ap.add_argument("--out-dir", default=os.path.dirname(os.path.abspath(__file__)),
                    help="where to write consolidated jsonl (default: consolidated/)")
    ap.add_argument("--out-sft", default="dave_v3_sft.jsonl")
    ap.add_argument("--out-dpo", default="dave_v3_dpo.jsonl")
    ap.add_argument("--no-expand", action="store_true",
                    help="keep <DAVE_SYSTEM_PROMPT> placeholder literal")
    ap.add_argument("--shuffle", action="store_true")
    ap.add_argument("--seed", type=int, default=3407)
    ap.add_argument("--sft-only", action="store_true")
    ap.add_argument("--dpo-only", action="store_true")
    ap.add_argument("--skip-rounds", nargs="*", default=[],
                    help="round ids to exclude (e.g. 4d 4c)")
    ap.add_argument("--dry-run", action="store_true",
                    help="don't write output files")
    args = ap.parse_args()

    if args.sft_only and args.dpo_only:
        sys.exit("--sft-only and --dpo-only are mutually exclusive")

    canonical = args.canonical or os.path.join(
        args.root, "two_is_round_1", "dave_canonical_sys_prompt.txt"
    )

    sys_prompt = None
    if not args.no_expand:
        if not os.path.isfile(canonical):
            sys.exit(f"canonical sys prompt not found at {canonical}")
        sys_prompt = open(canonical, "r", encoding="utf-8").read()

    expand = not args.no_expand
    skip = set(args.skip_rounds)

    # SFT
    sft_samples = []
    sft_per_round_batches = {}
    if not args.dpo_only:
        for rid, pattern in SFT_PATTERNS:
            if rid in skip:
                continue
            samples, per_batch = load_round(
                args.root, rid, pattern, sys_prompt, expand, "sft"
            )
            sft_samples.extend(samples)
            sft_per_round_batches[rid] = per_batch

    # DPO
    dpo_samples = []
    dpo_per_round_batches = {}
    if not args.sft_only:
        for rid, pattern in DPO_PATTERNS:
            if rid in skip:
                continue
            samples, per_batch = load_round(
                args.root, rid, pattern, sys_prompt, expand, "dpo"
            )
            dpo_samples.extend(samples)
            dpo_per_round_batches[rid] = per_batch

    if args.shuffle:
        rng = random.Random(args.seed)
        rng.shuffle(sft_samples)
        rng.shuffle(dpo_samples)

    out_sft = os.path.join(args.out_dir, args.out_sft)
    out_dpo = os.path.join(args.out_dir, args.out_dpo)

    if not args.dry_run:
        if sft_samples:
            with open(out_sft, "w", encoding="utf-8") as f:
                for s in sft_samples:
                    f.write(json.dumps(s, ensure_ascii=False) + "\n")
        if dpo_samples:
            with open(out_dpo, "w", encoding="utf-8") as f:
                for s in dpo_samples:
                    f.write(json.dumps(s, ensure_ascii=False) + "\n")

    print(f"\n=== consolidate_v3 ===")
    print(f"  root:        {args.root}")
    print(f"  canonical:   {canonical}")
    print(f"  expand:      {'yes' if expand else 'no'}")
    print(f"  shuffle:     {args.shuffle}")
    print(f"  skip rounds: {sorted(skip) if skip else 'none'}")
    print(f"  dry run:     {args.dry_run}")

    if not args.dpo_only:
        print(f"\n  SFT -> {out_sft}")
        for rid in (rid for rid, _ in SFT_PATTERNS if rid not in skip):
            batches = sft_per_round_batches.get(rid, [])
            n = sum(c for _, c in batches)
            print(f"    round {rid}: {n} samples  ({len(batches)} batches)")
        print(f"    SFT total: {len(sft_samples)}")
        st = stats(sft_samples, "sft")
        print(f"    per round: {st['per_round']}")
        print(f"    per var:   {st['per_var']}")
        print(f"    placeholder remaining in any sample: {st['placeholder_remaining']}")

    if not args.sft_only:
        print(f"\n  DPO -> {out_dpo}")
        for rid in (rid for rid, _ in DPO_PATTERNS if rid not in skip):
            batches = dpo_per_round_batches.get(rid, [])
            n = sum(c for _, c in batches)
            print(f"    round {rid}: {n} samples  ({len(batches)} batches)")
        print(f"    DPO total: {len(dpo_samples)}")
        st = stats(dpo_samples, "dpo")
        print(f"    per round: {st['per_round']}")
        print(f"    per var:   {st['per_var']}")
        print(f"    placeholder remaining in any sample: {st['placeholder_remaining']}")

    if expand and not args.no_expand:
        # Sanity: if we expanded but there are still placeholders, that's a bug
        # (wrapper format the v3 expander didn't recognize, e.g. partial substring).
        bad = 0
        if not args.dpo_only:
            bad += stats(sft_samples, "sft")["placeholder_remaining"]
        if not args.sft_only:
            bad += stats(dpo_samples, "dpo")["placeholder_remaining"]
        if bad:
            sys.exit(f"\n  FAIL: {bad} samples still contain {PLACEHOLDER} after expand\n")

    print(f"\n  ready for finetune.\n")


if __name__ == "__main__":
    main()
