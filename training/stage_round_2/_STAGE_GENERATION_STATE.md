# STAGE-temporal generation state (round 2)

| Batch | Status | Samples | Generated |
|---|---|---|---|
| 11 | done | 50 | 2026-05-06 |
| 12 | done | 50 | 2026-05-06 |
| 13 | done | 50 | 2026-05-06 |
| 14 | done | 50 | 2026-05-06 |
| 15 | done | 50 | 2026-05-06 |
| 16 | done | 50 | 2026-05-06 |
| 17 | done | 50 | 2026-05-06 |
| 18 | done | 50 | 2026-05-06 |
| 19 | done | 50 | 2026-05-06 |
| 20 | done | 50 | 2026-05-06 |

**Total round 2 samples:** 500 / 500 ✓
**Last update:** 2026-05-06 (ALL batches complete; 500 samples, 500 unique replies, 465 unique prompt-cores with intentional time-tag variants, 498/500 unique scene tags, perfect var/cat balance: 200 SYS-T + 300 NOSYS-T, 50 per category)

## Round 2 architecture (locked)

- Architecture A: tag in user turn, e.g. `[scene: 11:54 AM Monday, May 5, 2026, CST] hi`
- 84% incidental rate (most samples have tag but DON'T surface time in reply)
- TZ never surfaces geographically (no "you're in chicago" replies)
- Year mostly 2026 with ~20% varied across 2024-2027
- Skip cadence/elapsed (deferred to round 3)
- Combined-corpus training: round 1 (500) + round 2 (500) = 1000 samples

## Validation per batch

Each batch validated via inline check:
- 0 JSON errors
- 0 em dashes
- 0 missing scene tags (every user msg starts with `[scene: `)
- 0 tag echoes in assistant content (literal `[scene:` never appears in reply)
- 20 SFT-SYS-T / 30 SFT-NOSYS-T split
- 50 samples per batch, 5 per category × 10 categories

## Scripts (DONE)

- `validate_dave_stage.py` ✓ — extends round 1 validator with stage-specific checks; PASS on all 500 round 2 samples; PASS on combined 1000-sample corpus
- `consolidate_combined.py` ✓ — merges round 1 + round 2 into `dave_two_is_train_v2.jsonl` (1000 samples, ~1.97 MB, system prompts expanded)

## Final corpus (READY FOR FINETUNE)

`dave_two_is_train_v2.jsonl` — 1000 samples
- 400 SFT-SYS-T (with expanded Dave system prompt)
- 600 SFT-NOSYS-T (no system prompt)
- 100 per category × 10 categories
- 500 with scene tag, 500 without
- Cross-batch validated: 1000 unique full prompts, 0 protocol leaks, 0 em dashes, 0 tag echoes
- Time-fixation rate: 10.6% (target ≤16%, threshold 30%)

## Resume protocol

Round 2 generation is COMPLETE. Next steps (when Bo greenlights): run finetune_dave.py against `dave_two_is_train_v2.jsonl`, GGUF-convert, smoke-test in LM Studio, then commit round 2 + scripts + final corpus to GitHub.
