"""One-off post-consolidate sanity probe.

Spot-checks that round-4d samples in dave_v3_sft.jsonl have:
  - the time-prefix preserved at the start of the system message
  - the canonical Dave persona body (substring placeholder replacement worked)
  - NO literal <DAVE_SYSTEM_PROMPT> token left over

Also dumps a representative 4b (journal) and 4d sample so a human can read
that the expansion looks clean.
"""
import json
import sys

PATH = "dave_v3_sft.jsonl"
PLACEHOLDER = "<DAVE_SYSTEM_PROMPT>"
EXPECTED_PERSONA_PHRASE = "You are Dave"

failures = []
samples_4d = 0
samples_4b = 0

shown_4d = False
shown_4b = False

with open(PATH, "r", encoding="utf-8") as f:
    for line in f:
        o = json.loads(line)
        msgs = o.get("messages", [])
        if not msgs:
            continue
        sys_msg = msgs[0]
        content = sys_msg.get("content", "")
        rid = o.get("_round")

        # Universal: no orphan placeholder anywhere.
        for m in msgs:
            if PLACEHOLDER in (m.get("content") or ""):
                failures.append(f"orphan placeholder in {rid} sample")

        if rid == "4d":
            samples_4d += 1
            if not content.startswith("Today is "):
                failures.append(f"4d sample missing time prefix: {content[:60]!r}")
            if EXPECTED_PERSONA_PHRASE not in content:
                failures.append(f"4d sample missing persona body")
            if not shown_4d and o.get("_subcat") == "clock-direct":
                print("=" * 60)
                print("REPRESENTATIVE round-4d sample (clock-direct, post-expand)")
                print("=" * 60)
                print(f"  _injected: {o.get('_injected')}")
                print(f"  system msg first 220 chars:")
                print(f"  {content[:220]!r}")
                print(f"  ...")
                print(f"  system msg ends with: {content[-80:]!r}")
                print(f"  user: {msgs[1]['content']}")
                print(f"  asst: {msgs[2]['content']}")
                print()
                shown_4d = True

        if rid == "4b":
            samples_4b += 1
            if not shown_4b:
                print("=" * 60)
                print("REPRESENTATIVE round-4b sample (journal, post-expand)")
                print("=" * 60)
                print(f"  _kind: {o.get('_kind')}")
                print(f"  system first 100: {content[:100]!r}")
                print(f"  user: {msgs[1]['content']}")
                print(f"  asst: {msgs[2]['content']}")
                print()
                shown_4b = True

print(f"\n4d samples seen:  {samples_4d}")
print(f"4b samples seen:  {samples_4b}")

if failures:
    print(f"\nFAIL — {len(failures)} issue(s):")
    for f in failures[:10]:
        print(f"  X {f}")
    sys.exit(1)

print("\nALL CHECKS PASSED.")
