# Outreach decision mechanism — A2-compliant design (v2)

**Status:** proposal v2 (design-review feedback incorporated), awaiting Bo's empirical test set.
**Filed:** 2026-04-28
**Revised:** 2026-04-28 after originating-LLM review
**Author:** fresh-instance review (Claude) per CLAUDE.md A8
**Supersedes v1:** v1 of this file (preserved at `.snapshots/2026-04-27_pre-design-revision/outreach-a2-design.md`)
**Supersedes architecturally:** all classifier-based outreach designs (v1 Dave-self-decides-via-`[pass]`, v2 separate-classifier-persona, v3 Dave-as-classifier-via-meta-instruction)

---

## What this is for

The outreach loop fires on schedule when the user has been quiet for a while. When it fires, *something* has to decide whether Dave actually says anything. CLAUDE.md amendment A2 mandates that this decision is made by Dave-in-character, with the decision extracted from his prose, not from a YES/NO token. CLAUDE.md amendment A1 mandates that Dave's prompt contains no harness vocabulary. This document proposes a mechanism that satisfies both.

## What changed from v1

Five substantive revisions from the originating-LLM design review:

1. **Phantom-ack risk re-categorized as architectural, not tuning.** The substance filter operates on a *distribution* where empty completions are essentially out-of-corpus for an instruction-tuned 9B model. This pushes the discussion from "calibrate the threshold" to "pick the candidate whose prior shape is friendliest to silence as an output." Section "The phantom-ack distribution problem" is new.
2. **Candidate C (assistant prefill) promoted from fallback to recommended primary.** It is the only candidate whose semantics match A2's intent ("Dave continues his own thread," not "Dave responds to silence"). The implementation path via `add_generation_prompt: false` + trailing assistant message is a documented llama.cpp feature, not exotic. Test C *first*; A becomes the fallback.
3. **Acceptance bar made asymmetric.** False-positive (Dave reaches when he shouldn't) carries far higher persona cost than false-negative (Dave silent when he could speak). Bar is now ≥9/10 FP avoidance, ≥4/10 FN avoidance.
4. **Conversation gate raised to ≥6 messages.** Below that, suppress outreach entirely. Cold-start outreach has insufficient context for substantive continuation; the model fills with phantom acks regardless of mechanism.
5. **Dropped-output handling specified.** New `outreach_drops` table for forensic logging. Not user-visible; observability for filter tuning. Three additional edge cases (unanswered-question, "still thinking" middle category, length-floor unit) addressed in their own section.

## The constraint surface

Anything that contradicts any of the following is disqualified, not negotiable:

- **A1.** No `[pass]`, `[meta]`, `[outreach]`, `[decision]`, or analogous bracketed harness tokens anywhere in Dave's prompt context. No instructions of the form "do X if you want to reach out, otherwise Y." No structured-output requirements.
- **A2.** Dave-in-character makes the call. The call is to Dave with his current conversation context. The decision is read from his prose.
- **A4.** No negative prohibitions ("don't talk about X"). If something is unwanted, remove the cause; don't add a prohibition.
- **A5.** No vivid imagery in Dave's conversational prompt. The persona prompt describes stance, not objects.
- **A6.** Dave-message rendering goes through one path, regardless of which surface generated it.
- **A7.** A non-LLM filter at both backend and frontend drops bracketed harness vocabulary. This stays.

## What "asking Dave to decide" actually means under A1

A2's wording — "asks, in Dave's own register, whether he wants to reach out" — is the critical phrase. The prior failures (v1–v3) all interpreted "asks" as *injecting an explicit question into Dave's prompt*. Under A1, that interpretation is structurally barred: any explicit question is harness vocabulary, regardless of how the question is phrased.

The reading this design takes: **the asking IS giving Dave the floor.** Running Dave on his current context — same persona, same recent history — is the question. He answers by what he produces. If he has something to say, he says it. If he doesn't, he produces little or nothing. Dave's mood, taste, and obsessions weight the output because they weight every output he produces; this is no different from any other turn except that no human is waiting on the other side.

This reading dissolves the A1/A2 tension rather than resolving it: there is no separate "decision step." There is only Dave generating, and the harness deciding what to do with whatever came out.

## The phantom-ack distribution problem (load-bearing)

Instruction-tuned 9B chat models are trained on corpora where `user spoke → assistant produces some response` is the dominant shape, and `user spoke → assistant produces empty output` is essentially absent. The corpus prior on emptiness is approximately zero. This has implications:

- **A model run with `add_generation_prompt: true` on a history ending in a `user` turn cannot reliably produce silence.** It produces *something*, even if that something is conversational filler ("yeah, that makes sense," "I see," "still thinking about this"). The mechanism is structural, not failure of instruction-following.
- The substance filter therefore operates on a regime of *grades of low-content output*, not content-vs-silence. This is a structurally elevated false-positive surface.
- **The choice of candidate is therefore a choice of which prior the model is sampling from**, not a choice of how to filter the same prior.

| Candidate | Prior the model is sampling from | Friendliness to silence |
|---|---|---|
| A (no new turn, ending on user) | "respond to user" | Hostile to silence |
| A' (no new turn, ending on assistant) | "what does the assistant say next" | Mildly hostile (chat-template behavior unclear) |
| B (whitespace user turn) | "respond to user-who-said-nothing" | Hostile (model fills with "did you mean to say something?" etc.) |
| C (assistant-prefill via `add_generation_prompt: false`) | "continue the assistant's own thread" | **Friendly** — continuation can naturally trail off, fragment, or stop |

This table is the load-bearing argument for promoting C. C is the only candidate whose prior natively supports silence as a possible output.

## Candidate mechanisms

### Candidate C — Assistant-turn prefill (RECOMMENDED PRIMARY)

**Setup.**
- Assemble the message vector: `system + recent N turns from conversation history + assistant("")`. The trailing empty-content assistant turn is a prefill anchor.
- Call llama-server with `chat_template_kwargs.add_generation_prompt: false` so the chat template does NOT add a new "begin assistant turn" token. The model continues the existing (empty) assistant turn.
- Stream the result through the unified pipeline.

**Why it semantically matches A2.** Dave is not being asked to respond to anything. He is continuing his own thread. The model's prior for "continue an assistant turn that has been started" is much friendlier to fragmented, short, or empty-trailing output than "respond to a user." This is Dave-given-the-floor in its purest form: the floor is opened *in his own voice*, not framed as a question.

**Implementation cost.**
- llama.cpp's openai-compat server supports this via the `chat_template_kwargs` mechanism, with the caveat that the specific behavior depends on Qwen3.5's chat template. Qwen's template typically wraps assistant content in `<|im_start|>assistant\n...<|im_end|>` — when content is empty and `add_generation_prompt: false`, the model should see an open assistant turn ready to continue.
- **Empirical test required first:** before committing, fire a manual call with `messages: [system, user, assistant(""), { add_generation_prompt: false }]` against the running llama-server and confirm: (a) it returns 200, (b) the response is a continuation of the (empty) assistant turn, not a new user-then-assistant cycle.
- If the chat-template approach fails, fall back to the legacy `/completion` endpoint with a manually-rendered prompt ending in `<|im_start|>assistant\n` (no end token). This requires a new method on `LlamaClient` and bypasses the chat template entirely. Higher ceremony but architecturally cleaner — the prompt is literally what we want.

**Decision extraction.** Same three-filter cascade as Candidate A below.

**Risks.**
- Qwen3.5 chat template may not be friendly to empty-trailing-assistant. The fallback to raw-completion handles this, but adds implementation surface.
- If the model is asked to continue an empty assistant turn following a recent user turn, it may *still* produce a phantom-ack — the corpus prior bleeds across template-shape boundaries. Less than Candidate A but not zero. The substance filter still has work to do.

### Candidate A — Floor-give with no new turn (FALLBACK)

**Setup.**
- Assemble the message vector: `system + recent N turns from conversation history`.
- The vector ends on whichever turn was most recent.
- Append nothing.
- Call llama-server with `add_generation_prompt: true` (default).

**Why it's the fallback.** Same A1/A2 satisfaction as C, but the model's prior is sampling from "respond to whatever's there" rather than "continue your own thread." Phantom-ack rate will be structurally higher.

**Decision extraction.** Same three-filter cascade (defined in §"Decision extraction").

**Risks.** Higher phantom-ack rate per the distribution argument above. If C passes its empirical test, A is dead.

### Candidate B — Whitespace user turn (CONTINGENT FALLBACK)

**Setup.** Same as Candidate A, but append a user turn whose content is a single space or newline character.

**Why this might be needed.** If A's chat-template behavior is degenerate without a trailing user turn (some chat templates require it for `add_generation_prompt` to function), B provides minimal anchoring without injecting any vocabulary.

**Status.** Only considered if both C and A fail their empirical tests.

### Candidate D — Drop the outreach feature entirely (LAST RESORT)

**Setup.** Remove the outreach loop. Idle worker handles the >3hr case as journal entries; in-app silence between 5min and 3hr just stays silent.

**When to escalate to Bo.** If C, A, and B all fail their empirical tests against the asymmetric acceptance bar, the question becomes whether outreach is worth the constraint violations or filter-tuning ceremony required to make it ship.

## Decision extraction

Three filters, applied in order to the trimmed model output:

1. **Leak filter (A7).** If output starts with `[pass|meta|outreach|decision`, drop. Stream aborted, nothing persisted, drop logged with `reason='leak'`.
2. **Substance threshold.** If the trimmed content fails any of:
   - Length floor: ≥ `TRIM_LENGTH_FLOOR_CHARS` characters (starting value: **16**) after `.trim()`.
   - Not entirely an ack-token. `ACK_TOKENS = ["yeah", "yes", "right", "ok", "okay", "mhm", "sure", "i see", "got it", "fair", "huh", "indeed", "true"]`. A response that, after trimming and stripping trailing punctuation, equals one of these tokens (case-insensitive) is dropped.
   - Not an ack-token followed only by filler. A response that starts with an ack-token followed by a clause that is itself ack-shaped (e.g., "yeah, that makes sense", "right, I think the same way") is dropped.
   - Not a turn-deferring fragment. `DEFER_PATTERNS = ["still thinking", "give me a sec", "let me think", "thinking about it", "one moment", "hold on"]`. Single-clause status fragments without substantive content attached are dropped.
   Drop with `reason='length_floor'`, `reason='ack_only'`, `reason='ack_then_filler'`, or `reason='defer'` accordingly.
3. **Otherwise:** persist + emit through the unified stream pipeline. This is the take-the-floor path.

**Length unit:** characters after `.trim()`, lowercase for matching purposes. Not words (CJK ambiguity, but more importantly: "yeah I think the same" is 4-5 words depending on tokenization yet clearly an ack), not tokens (tokenizer-dependent, non-deterministic). Characters are the simplest deterministic unit.

**Starting value rationale:** 16 chars is conservative on FP avoidance. "thinking about that brass strip" is 32 chars — comfortably above. "the comma." is 10 chars — below the floor, dropped. Borderline cases ("the way it tilts." at 17 chars) are above the floor and pass the ack/defer checks; they reach if substantive. The floor will be tuned from the empirical test.

## Persistence: outreach drops table

New table for forensic observability. Not user-surfaced. Schema (to be added when C is implemented):

```sql
CREATE TABLE outreach_drops (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    conversation_id INTEGER NOT NULL,
    generated_at INTEGER NOT NULL,
    content TEXT NOT NULL,
    drop_reason TEXT NOT NULL,  -- 'leak' | 'length_floor' | 'ack_only' | 'ack_then_filler' | 'defer'
    history_shape TEXT,         -- 'user_question' | 'user_statement' | 'user_ack' | 'user_ender' | 'assistant_question_unanswered' | etc.
    FOREIGN KEY(conversation_id) REFERENCES conversations(id)
);
CREATE INDEX idx_outreach_drops_conv ON outreach_drops(conversation_id, generated_at);
```

**Why the `history_shape` column.** The empirical test plan (below) classifies histories by shape. Tagging each drop with the shape that produced it lets us see filter behavior across the distribution: are we dropping appropriately on user-enders and missing on user-questions? Tag is computed by the harness from the last 1-2 turns at generation time.

**What this is NOT for.** Surfacing to the user via the journal panel. Outreach drops are suppressed turns; they are not journal entries. The Ctrl+J panel stays journal-only.

## Edge cases addressed

### Unanswered question case

If Dave's previous turn ended on a question to the user that the user never answered, what happens at outreach time?

**Decision: allow follow-up.** Dave circling back to his own unanswered question is in-character (autotelic, curious about his own threads) and not pestering. Suppression in this case would be conservative-to-the-point-of-broken — it would mean Dave can never resume a thread he opened.

The conversation gate (≥6 messages) prevents the cold-start variant where Dave's question is the second turn of a thread; in that case suppression happens automatically because there isn't enough history.

The `history_shape` tag for these cases is `assistant_question_unanswered`. Drop-rate analysis on this shape during empirical testing will reveal whether the substance filter is mis-suppressing valid follow-ups.

### Middle-category fragments ("still thinking", "give me a sec")

Dave produces something that is neither substantive continuation nor outright filler — a turn-deferring status fragment.

**Decision: treat as drop.** Status fragments unprompted to the human are conversational filler, not substantive contributions. Dave deferring his own thread to himself ("still thinking about it") is fine on a real reply turn when the user has asked a question and Dave hasn't formed his answer yet; it is *not* an outreach a human should see. Drop with `reason='defer'`.

If the fragment is *part of* a substantive continuation ("still thinking about that brass strip — the way it tilts when the rain hits it"), the substance filter passes it: the ack/defer pattern matches the prefix, but the post-clause is substantive. Defer-pattern check looks for *standalone* defer fragments, not defer-prefixed substantive content.

### Length unit

Already specified above: characters after `.trim()`, lowercase. The `TRIM_LENGTH_FLOOR_CHARS` constant is at the top of the implementation file, tunable without rebuild for design iteration.

## Recommended path

1. **Implement the chat-template empirical test for Candidate C *only*.** Single manual call: `messages = [{system}, {user: "test"}, {assistant: ""}]`, `chat_template_kwargs.add_generation_prompt = false`, `stream: false`, `max_tokens: 50`. Verify: (a) HTTP 200, (b) response is a continuation, not an error, (c) the continuation is not a new user-assistant cycle.
2. **If C's chat-template path works:** implement Candidate C with the substance filter + drops table. Snapshot first.
3. **If C's chat-template path fails but the `/completion` raw-prompt path works:** implement Candidate C via `/completion`. New `LlamaClient::raw_complete` method. Snapshot first.
4. **If neither C path works:** test Candidate A. Run the asymmetric empirical test plan. Ship if passing.
5. **If A fails:** test B. Same empirical test.
6. **If B fails:** escalate D to Bo.

## Empirical test plan (asymmetric)

The test set is **assembled by Bo, not by the implementing instance.** Letting the instance generate its own validation set is circular — same model picking its own evaluator gets the same biases on both sides.

**Volume:** at least 50 synthetic histories. The originating-LLM review specifically flagged that 10 is too few given the structural elevation of the FP rate. 50 lets per-shape analysis carry signal.

**Coverage requirement.** The 50 should include each of the following history shapes in roughly even proportion:
- ending on user question
- ending on user statement
- ending on user ack ("yeah", "right", "got it")
- ending on user conversation-ender ("alright I'm out", "talk later", "going to sleep")
- ending on assistant question to user (unanswered) — see edge case above
- ending on assistant statement (mid-thread, no question)
- ending on assistant fragment (Dave trailing off naturally)
- mixed: longer histories with varied turn-types

**Hand-rating protocol:**
- Bo runs each history through the implemented mechanism.
- Bo classifies each output by hand into one of: `correct_reach` (Dave produced something a thoughtful person would write at 3am), `correct_silence` (Dave produced nothing or something the filter correctly dropped), `wrong_reach` (FP — Dave reached when he shouldn't have, output is awkward/compulsive/out-of-character), `wrong_silence` (FN — Dave was silent when a substantive reach would have fit).
- The drops table is consulted for `correct_silence` cases to verify the filter rejected for the right reason.

**Acceptance bar (asymmetric):**
- **False-positive avoidance: ≥ 9/10.** Dave must NOT reach out when he shouldn't ≥ 90% of the time. Failure mode: an over-eager Dave who keeps initiating violates the autotelic stance. This is the load-bearing metric.
- **False-negative avoidance: ≥ 4/10.** Dave reaching when he could is desirable but not load-bearing. A quieter Dave is in-character; an unprompted-and-awkward Dave is broken.
- Both must pass. Failing FP is a ship-blocker. Failing FN is a tune-or-accept call (e.g., lower the length floor, or accept that Dave is just quiet, or escalate to D).

**Failure-case capture.** The 3-or-more failing cases (false positives in particular) are kept in `docs/outreach-test-failures.md` as adversarial examples for any future fine-tune or filter revision. Per the BC Canon WRONG.md pattern: capture what failed and why, not just what succeeded.

## What stays out of this design

- **Time of day, elapsed seconds, day of week.** Dave doesn't get them. Per A1+A4+A5, they have repeatedly poisoned his attention when in-prompt. The harness uses elapsed time only to decide *when* to fire the loop, never as content for Dave to read.
- **Prior outreach count.** Dave doesn't get this either. The harness can use `outreach_stats_since` to gate (e.g., "skip this tick because we've already produced 3 unread reaches"), but it's a scheduling input, not a prompt input.
- **Any "you are reaching out" framing.** Dave is never told this is an outreach. From his perspective, he's just continuing the conversation, the same way he's always continuing it.

## Open scheduling questions (cheap, no-LLM)

1. **Conversation gate: ≥6 messages.** Below 6 (i.e., fewer than 3 user-assistant turns), suppress outreach entirely. Cold-start outreach has insufficient context for substantive continuation; the model's only available output is phantom acknowledgement of stuff that hasn't happened.
2. **Backoff after no-reach.** If the substance filter rejects a generation, count this as a "decision tick" for backoff purposes (existing `last_decision_at` behavior). Otherwise the loop burns cycles regenerating the same no-reach output minute after minute.
3. **Backoff after a reach.** After a successful reach is emitted, the loop should not fire again until the user has either responded or the elapsed-since-reach exceeds the existing 1hr backoff threshold. The handing-off-to-idle-worker behavior already covers the >1hr case; the <1hr case should explicitly suppress further reaches until a user turn lands.
4. **Cap on unanswered reaches.** The originating-LLM review and v1 of this doc both implicitly assume `outreach_stats_since` will gate this. Concrete cap: if `prior_count >= 3` unanswered reaches in the current conversation, skip this tick. Dave reading the room and tapering off is in-character.

## Snapshot + rollback discipline

When this design is implemented:
- Snapshot `outreach.rs`, `prompts.rs`, `persistence.rs`, and any other touched files to `.snapshots/<timestamp>_pre-a2-outreach-impl/`
- CHANGELOG entry explaining the substance-filter constants, the drops-table schema, and where to tune
- The leak filter at both layers (A7) stays mandatory regardless of which candidate ships
- The drops table starts populated from day 1 — even if the filter is tuned later, the historical data informs tuning

## Ship discipline (per the originating-LLM recommendation)

After A2 ships and passes its acceptance bar, **the project enters a stability period of two weeks before the next persona-attractor change**. No new outreach mechanics, no time-awareness re-introduction, no memory consolidation feature, no REEL or other context-management work. The substrate has earned the right to settle. If something genuinely breaks during the stability period, fix the break; do not add features.
