# Dave training corpus

Fine-tuning data for Dave, the persona at the heart of Tenancy. Two-Is architecture: both the reasoning trace (`<think>...</think>`) and the visible reply are written in Dave's first-person voice (per the canonical system prompt in `src-tauri/src/prompts.rs` and the higher-resolution soul document in `docs/DAVE_SOUL.md`).

## Layout

```
training/
├── README.md                       this file
├── pipeline/
│   ├── finetune_dave.py            QLoRA + GGUF export (Unsloth-based, Qwen3.5-4B target for POC)
│   └── chat_dave.py                interactive inference (with adapter-path validation)
└── two_is_round_1/                 first 500 samples — Two-Is, no STAGE tags
    ├── README.md                   format docs + pre-train pipeline
    ├── _GENERATION_STATE.md        progress tracker (final state)
    ├── batches/
    │   ├── dave_two_is_batch_01.jsonl   50 samples
    │   ├── ...
    │   └── dave_two_is_batch_10.jsonl   50 samples (10 batches × 50 = 500 total)
    ├── dave_two_is_train.jsonl     consolidated 500-sample training file (sys prompt expanded inline)
    ├── dave_canonical_sys_prompt.txt   verbatim canonical Dave system prompt
    ├── expand_system.py            placeholder expansion utility
    ├── consolidate.py              merges all batches into single training file
    └── validate_dave_two_is.py     full structural + voice + cross-batch validator
```

## Round 1 composition (500 samples)

- **200 SFT-SYS-T** (system prompt active)
- **300 SFT-NOSYS-T** (no system prompt — pushes character into weights so model behaves as Dave even without prompt scaffolding)
- **10 categories × 50 samples each**: greeting / identity / etymology / canonical / technical / philosophical / emotional / pushback / refusal / openend
- **All 500 user prompts unique across the corpus**
- **Voice rules enforced**: zero em dashes, zero bullet/numbered lists in replies, zero affirmation rituals, zero closing service rituals, zero "as an AI" preambles. Validated programmatically.

## Quick start

```bash
# In WSL (or any Linux env with Unsloth installed):
cd training/two_is_round_1

# Validate the corpus
python validate_dave_two_is.py

# Run training + GGUF export
python ../pipeline/finetune_dave.py \
    --data ./dave_two_is_train.jsonl \
    --output ./dave_adapter \
    --model unsloth/Qwen3.5-4B \
    --epochs 3 \
    --gguf-quant q4_k_m
```

The script prints `[sanity] N/M samples contain <think> in formatted text` before training starts. If <think> blocks were silently stripped by the chat template, training aborts with `sys.exit(1)` instead of wasting GPU cycles.

GGUF lands at `./dave_gguf/`. Copy into LM Studio's models directory to load.

## Future rounds (planned)

- **Round 2 (in progress)**: 500 additional samples with STAGE temporal tags injected at user-turn prefix (`[scene: HH:MM AM/PM Day, Month D, YYYY, TZ]`). Reasoning layer absorbs the tag in Dave's voice; output layer behaves as if Dave just happens to know what time it is. Combined-corpus training (round 1 + round 2 = 1000 samples).
- **Round 3 (planned)**: cadence/elapsed-time ("last turn N minutes ago"), multi-turn samples, DPO-THINK pairs.
- **Round 4 (planned)**: additional STAGE channels (state, narration), persona-mediated memory consolidation training.

See the architecture documents in `docs/` for the broader architecture context.

## License

Same as the parent Tenancy project (MIT).
