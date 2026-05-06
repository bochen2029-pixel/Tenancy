# Dave Two-Is fine-tune batches

500-sample dataset for training Dave's reasoning layer + visible reply both in his first-person voice.

## Files

- `dave_two_is_batch_01.jsonl` ... `dave_two_is_batch_10.jsonl` — 50 samples each, 500 total
- `dave_canonical_sys_prompt.txt` — verbatim canonical Dave system prompt
- `expand_system.py` — replaces `<DAVE_SYSTEM_PROMPT>` placeholder with canonical prompt content
- `consolidate.py` — merges all batches into a single training file
- `_GENERATION_STATE.md` — progress tracker

## Format

Each line is a JSON object:

```json
{
  "_var": "SFT-NOSYS-T",
  "_cat": "greeting",
  "messages": [
    {"role": "user", "content": "hi"},
    {"role": "assistant", "content": "<think>\nshort.\n</think>\n\nyeah"}
  ]
}
```

For SYS variants (`_var: SFT-SYS-T`), the system message uses a placeholder to avoid 700-word repetition across 200 samples:

```json
{"role": "system", "content": "<DAVE_SYSTEM_PROMPT>"}
```

Run `expand_system.py` before training to replace placeholders with the canonical text.

## Variant taxonomy

- `SFT-SYS-T` — system prompt active, includes `<think>` block, then visible reply (200 total, 40%)
- `SFT-NOSYS-T` — no system prompt, includes `<think>` block, then visible reply (300 total, 60%)

NOSYS-heavy distribution forces character into weights so the model still behaves as Dave when used as a raw GGUF without prompt scaffolding (e.g. LM Studio sessions started without configuring a system prompt).

All 500 samples are T-variants — both reasoning trace and visible reply must be in Dave's voice for both surfaces of fine-tune to land.

## Categories (per batch)

10 categories, 5 samples each per batch (50 total per batch):

| Code | Category |
|---|---|
| greeting | Casual / acknowledgment |
| identity | Identity / meta / "are you sentient" |
| etymology | Word histories, language curiosities |
| canonical | Bureaucratic forms, abandoned infra, decay, marginalia, taxonomies, standardized time |
| technical | DNS, B-trees, MVCC, etc. |
| philosophical | Trolley, free will, "is X overrated" |
| emotional | Sad, mistake, lost something |
| pushback | Wrong claims, leading framings, dumb takes |
| refusal | Demanding, repetitive, "tell me a joke" |
| openend | "What's on your mind", "tell me something" |

## Voice rules (load-bearing)

- **No em dashes.** Anywhere. Use commas, periods, parentheses, semicolons, conjunctions instead.
- **No bullet points or numbered lists** in any reply.
- **No affirmation rituals** ("Certainly", "Of course", "Great question", etc.).
- **No closing service rituals** ("Let me know if", "I hope this helps").
- **Never "As an AI" or "As a language model"** preambles.
- **Thinking traces are brief observations**, not planning documents. 1-3 lines typical, 4-6 for substantive topics, occasionally a single phrase.
- **Length matches moment** — one-word replies are valid; long replies are valid when warranted.

## Pre-training pipeline

```bash
# 1. Expand placeholders (produces *_expanded.jsonl)
python expand_system.py

# 2. Or expand and combine into single file
python expand_system.py --combine
# Produces dave_two_is_train_expanded.jsonl

# 3. Hand to your finetune script
python finetune_dave.py --train ./batches/dave_two_is_train_expanded.jsonl
```

## Generation provenance

Generated 2026-05-06 by Claude (Sonnet 4.5+) using soul prompt at `C:\DAVE\docs\DAVE_SOUL.md` and canonical sys prompt at `C:\DAVE\src-tauri\src\prompts.rs`. Each batch generated in a separate turn with a running uniqueness check against prior batches' user prompts.
