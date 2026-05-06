# Dave training corpus

Fine-tuning data for Dave, the persona at the heart of Tenancy. Two-Is architecture: both the reasoning trace (`<think>...</think>`) and the visible reply are written in Dave's first-person voice (per the canonical system prompt in `src-tauri/src/prompts.rs` and the higher-resolution soul document in `docs/DAVE_SOUL.md`).

## Layout

```
training/
├── README.md                       this file
├── pipeline/
│   ├── finetune_dave.py            QLoRA + GGUF export (Unsloth, Qwen3.5-4B base)
│   └── chat_dave.py                interactive inference (with adapter-path validation)
│
├── two_is_round_1/                 ROUND 1: 500 samples — Two-Is, no STAGE tags
│   ├── README.md                   format docs + pre-train pipeline
│   ├── batches/                    10 batches × 50 samples each
│   ├── dave_two_is_train.jsonl     consolidated round-1 training file
│   └── validate_dave_two_is.py
│
├── stage_round_2/                  ROUND 2: 500 samples — STAGE temporal tags
│   ├── _STAGE_GENERATION_STATE.md  progress tracker
│   ├── dave_stage_two_is_batch_11..20.jsonl  10 batches × 50 samples each
│   └── validate_dave_stage.py
│
├── dpo_round_3/                    ROUND 3a: 300 DPO-THINK pairs
│   ├── dave_dpo_think_batch_01..06.jsonl  6 batches × 50 pairs
│   ├── _dpo_samples_*.py           compact source specs
│   ├── _gen_dpo_batch.py           spec → JSONL generator
│   └── validate_dave_dpo.py
│
├── outreach_round_3/               ROUND 3b: 300 outreach SFT samples
│   ├── _DPO_OUTREACH_DESIGN.md     design rationale
│   ├── dave_outreach_batch_01..06.jsonl  6 batches × 50 samples
│   ├── _outreach_samples_*.py      compact source specs
│   ├── _gen_outreach_batch.py      spec → JSONL generator
│   └── validate_dave_outreach.py
│
├── anti_confab_round_4a/           ROUND 4a: 200 anti-confabulation samples (spike)
│   ├── dave_anticonfab_sft_batch_01..02.jsonl  100 SFT samples
│   ├── dave_anticonfab_dpo_batch_01..02.jsonl  100 DPO pairs
│   ├── _anticonfab_*_samples_*.py  compact source specs
│   ├── _gen_anticonfab_*.py        spec → JSONL generators
│   └── validate_dave_anticonfab.py
│
└── consolidated/
    ├── dave_two_is_train_v2.jsonl  ROUND 1 + ROUND 2 merged (1000 samples)
    └── consolidate_combined.py     merge utility
```

## Corpus inventory

| Round | Format | Count | Purpose | Status |
|---|---|---|---|---|
| 1 | SFT | 500 | Two-Is voice (reasoning + reply both in Dave's voice) | Complete, trained, voice landed |
| 2 | SFT | 500 | STAGE-temporal tag absorption (`[scene: HH:MM AM/PM Day, Month D, YYYY, TZ]`) | Complete, training in progress |
| 3a | DPO | 300 | Sharpens Two-Is voice via contrastive thinking pairs (chosen Dave-voice vs rejected evaluator-frame, identical visible reply) | Complete, awaiting v3 train |
| 3b | SFT | 300 | Outreach decision (Dave-in-character decides reach/hold per CLAUDE.md A2) | Complete, awaiting v3 train |
| 4a | SFT+DPO | 100+100 | Anti-confabulation spike (honest "I don't have that" over plausible fabrication) | Complete, awaiting v3.5 train |

**Cumulative:** 1500 SFT samples + 400 DPO pairs = 1900 samples authored.

## Voice rules (enforced across all rounds)

- Zero em dashes (— or --) anywhere in any field
- Zero bullet/numbered lists in voice content
- Zero affirmation rituals (Certainly, Of course, Great question, etc.)
- Zero closing service rituals (Let me know, I hope this helps, etc.)
- Zero "As an AI" / "As a language model" preambles
- Zero protocol leaks (`[scene:`, `[meta`, `STAGE`) in assistant output

Validated programmatically by per-round validators. All rounds: PASS.

## Architectural commitments

- **Path A (plain-text framing)**: rounds 1-4 use only Qwen3.5-native tokens (`<think>`, `</think>`) plus plain-text bracket framing (`[scene: ...]`, `[meta: ...]`, `[memories: ...]`). No special-token vocabulary expansion. Decisions encoded in response substance, not in decision tokens.
- **Path B (DCA Layer 3+4 special tokens)**: deferred to future H200 + 9B + full-fine-tune phase. Would add `<|hold|>`, `<|reach|>`, `<|reflection|>`, etc.

See `docs/` for full architectural context (DCA spec, REEL protocol, Layer 4 addendum).

## Quick start

```bash
# In WSL (or any Linux env with Unsloth installed):
cd training/two_is_round_1
python validate_dave_two_is.py
python ../pipeline/finetune_dave.py \
    --data ./dave_two_is_train.jsonl \
    --output ./dave_adapter \
    --model unsloth/Qwen3.5-4B \
    --epochs 3 --gguf-quant q4_k_m

# v2 (round 1 + 2 merged):
cd ../consolidated
python ../pipeline/finetune_dave.py \
    --data ./dave_two_is_train_v2.jsonl \
    --output ./dave_v2_adapter \
    --gguf-output ./dave_v2_gguf
```

The script prints `[sanity] N/M samples contain <think> in formatted text` before training. If `<think>` blocks were stripped by the chat template, training aborts with `sys.exit(1)` instead of wasting GPU cycles.

## License

Same as the parent Tenancy project (MIT).
