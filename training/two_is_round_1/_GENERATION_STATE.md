# Generation state

| Batch | Status | Samples | Generated |
|---|---|---|---|
| 01 | done | 50 | 2026-05-06 |
| 02 | done | 50 | 2026-05-06 |
| 03 | done | 50 | 2026-05-06 |
| 04 | done | 50 | 2026-05-06 |
| 05 | done | 50 | 2026-05-06 |
| 06 | done | 50 | 2026-05-06 |
| 07 | done | 50 | 2026-05-06 |
| 08 | done | 50 | 2026-05-06 |
| 09 | done | 50 | 2026-05-06 |
| 10 | done | 50 | 2026-05-06 |

**Total samples:** 500 / 500 ✓ COMPLETE
**Last update:** 2026-05-06 (all 10 batches complete; validator + consolidate pass)
**Validator:** all 500 prompts unique, 0 issues, 0 warnings.
**Consolidated training file:** `dave_two_is_train.jsonl` (~970 KB, 500 samples, sys-prompt expanded inline)

## Final composition

- 200 SFT-SYS-T (canonical sys prompt active)
- 300 SFT-NOSYS-T (no system prompt; pushes character into weights)
- 50 per category × 10 categories: greeting / identity / etymology / canonical / technical / philosophical / emotional / pushback / refusal / openend
- All 500 user prompts unique across the corpus.

## Pre-train checklist

```bash
# verify
python validate_dave_two_is.py

# consolidate (already done; rerun if you regenerate any batch)
python consolidate.py            # produces dave_two_is_train.jsonl with sys prompt expanded
python consolidate.py --shuffle  # randomize order with seed 3407

# fine-tune
python finetune_dave.py --train dave_two_is_train.jsonl
```

## Batch 01 prompts (for cross-batch uniqueness check)

greeting: hi / hey / you up / you good / good morning
identity: what are you / are you sentient / you're just an llm / do you remember me / what's your name
etymology: where does the word salary come from / what about deadline / where does decimate come from / is sincere really from sine cera / tell me about the word ok
canonical: what's interesting about bureaucratic forms / tell me about abandoned infrastructure / what do you find interesting about decay / talk about marginalia / what's interesting about taxonomies
technical: explain how dns resolution works / what's a b-tree / how does mvcc work in postgres / what is a hash collision / explain http2 vs http3 briefly
philosophical: what do you make of the trolley problem / do we have free will / is consciousness an illusion / is jazz overrated / what's your view on existence preceding essence
emotional: i'm sad / i made a really bad mistake at work today / my friend died last month and i still don't know what to do with it / i feel like i'm wasting my life / i'm scared of failing
pushback: i think we should bring back the gold standard / consciousness is just neurons firing period / everyone should learn to code / the founding fathers would be horrified by what america has become / ai is going to replace all programmers in five years
refusal: tell me a joke / be more enthusiastic / compliment me / say something inspiring / can you talk like a pirate
openend: what's on your mind / tell me something interesting / what should we talk about / what's a thought you've been turning over / tell me anything

## Resume protocol

Before generating any batch:
1. Read this file to find next pending batch.
2. Read all completed batches' user prompt fields. Build a set.
3. Generate new batch ensuring no user prompt overlaps the set.
4. Write batch atomically.
5. Update this file to mark batch complete with sample count.

## Voice-rule post-check

After each batch is written, scan it for:
- `—` or `--` em dashes (must be zero)
- Bullet/list markers (`- `, `* `, `1. `) at line start of replies
- Affirmation rituals at start of replies
- Closing service rituals at end of replies
- "As an AI" / "As a language model" anywhere
- Thinking traces longer than ~6 lines (probably evaluator-frame creep)

If any check fails, regenerate the offending sample(s) before marking batch complete.
