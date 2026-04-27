# Changelog

Append-only log of meaningful changes since v1 scaffolding. Snapshots of
overwritten files are saved in `.snapshots/<timestamp>_<label>/` for rollback
when git isn't being used.

To roll back a change: `cp -r .snapshots/<timestamp>_<label>/* ./` then
`cargo check` / `pnpm build` to confirm.

---

## 2026-04-28 — A2 design doc revised after originating-LLM review (v1 → v2)

**Context:** v1 of `docs/outreach-a2-design.md` went through originating-LLM
review. Verdict: design shape correct (Candidate A's "give Dave the floor"
reframe lands), but five substantive revisions needed before implementation.

**Revisions applied (v1 → v2):**

1. **Phantom-ack reframed from tuning to architectural risk.** The substance
   filter operates on a distribution where empty completions are
   essentially out-of-corpus for instruction-tuned 9B models. Choice of
   candidate is a choice of which prior the model samples from, not how to
   filter the same prior. New section "The phantom-ack distribution
   problem" with a candidate-vs-prior comparison table.
2. **Candidate C (assistant prefill) promoted from fallback to primary.**
   Only candidate whose prior natively supports silence as output (model
   continues its own thread vs. responds to a user turn). Implementation
   path via `chat_template_kwargs.add_generation_prompt: false` with a
   trailing empty assistant message; raw `/completion` fallback if the
   chat template is unfriendly. Test C first; A becomes fallback.
3. **Acceptance bar made asymmetric.** ≥9/10 false-positive avoidance,
   ≥4/10 false-negative avoidance. Cost calculus: FP violates autotelic
   stance ("do not perform helpfulness"); FN just makes Dave quieter,
   which is in-character. FP-fail is ship-blocker; FN-fail is tune-or-accept.
4. **Conversation gate raised from ≥2 to ≥6 messages.** Below ~3 user-
   assistant turns, the model has no substantive context to continue from
   and fills with phantom acknowledgements regardless of mechanism.
5. **`outreach_drops` table specified for forensic logging.** Schema
   includes `drop_reason` (leak/length_floor/ack_only/ack_then_filler/defer)
   and `history_shape` (user_question/user_statement/etc) for per-shape
   filter analysis. Not user-surfaced — observability only.

**Three edge cases addressed:**
- Unanswered Dave question: allow follow-up (in-character, not pestering).
- "Still thinking" middle-category fragments: drop with `reason='defer'`.
  Substantive content prefixed by a defer-pattern still passes the filter.
- Length unit specified: characters after `.trim()`, lowercase for
  matching. Starting value `TRIM_LENGTH_FLOOR_CHARS = 16`.

**Empirical test plan tightened:** N raised from 10 to ≥50, distributed
across 7+ history shapes. **Test set assembled by Bo, not the
implementing instance** — circular validation otherwise. Failing cases
captured in `docs/outreach-test-failures.md` per BC-Canon WRONG.md
pattern.

**Ship discipline:** after A2 ships and passes, two-week stability period
before any further persona-attractor change. No REEL, no time
re-introduction, no memory consolidation during the period.

**Files modified:** `docs/outreach-a2-design.md`
**Snapshot of v1:** `.snapshots/2026-04-27_pre-design-revision/outreach-a2-design.md`

**No code written.** Design still gated; awaiting Bo's empirical test set
and greenlight for Candidate C implementation.

---

## 2026-04-28 — A2 architectural review: classifier path removed

**Context:** CLAUDE.md amendment A2 (added by originating-LLM review per
A8, dating the verdict 2026-04-27) overrules the position that outreach
decisions are made by a separate classifier persona. A2 specifies:
*"It calls Dave with his current conversation context and asks, in
Dave's own register, whether he wants to reach out. The decision is
extracted from Dave's response (not from a YES/NO token)."*

This session is the A8 fresh-instance review of the full delta from
working baseline (pre-time, pre-outreach) to current state. The amendment
endorses A1/A4/A5/A7 strip-and-revert subtractions; A2 specifically
overrules every classifier-shaped outreach implementation tried so far.

**Subtractions applied (this entry):**

1. **`prompts.rs`:** removed `outreach_decision_meta()` and
   `OUTREACH_SPONTANEOUS_META`. (The earlier
   `CLASSIFIER_SYSTEM_PROMPT` + `classifier_user_prompt` had already
   been replaced by the Dave-as-classifier-via-meta-instruction
   variant; both lineages are gone now.) Comment block in their place
   states the A2 verdict and points to `docs/outreach-a2-design.md`.
2. **`outreach.rs`:** removed `classify`, `parse_yes`, `generate_outreach`,
   and the import surface that supported them (`Emitter`, `harness`,
   `leak`, `ChatMessage`, `Message`, `prompts`, `OUTREACH_HISTORY_TURNS`).
   `tick()` now performs the full time/conversation gating but logs
   "would-have-fired (A2 design pending — producing nothing)" instead
   of generating. The loop continues to fire on schedule.
3. **`persistence.rs`:** untouched. `outreach_stats_since` is now dead
   code (one cargo warning), kept for the future A2 implementation
   which will need it for backoff gating.

**Memory and docs:**

- `memory/dave_harness_separation.md`: appended `## Retraction —
  2026-04-28` per Bo's verbatim instruction. Original content
  preserved append-only so future instances see what was previously
  believed and why it was overruled.
- `memory/MEMORY.md`: index entry annotated with
  partial-retraction status; the harness-unawareness rule and
  defense-in-depth filter rule still stand, the classifier rule does
  not.
- `docs/outreach-a2-design.md`: new file. Proposes Candidate A
  (floor-give with no new turn, decision via length + ack-token
  substance filter) as the recommended path, with B (whitespace user
  turn) and C (assistant prefill) as fallbacks and D (drop outreach)
  as last resort. Empirical test plan included. **No code written
  against this design yet** — Bo's three-step sequencing requires
  review before implementation.
- `docs/open-issues.md`: added P2 entry for A6 startup-fragment
  parallel-path (non-streaming `complete()` for startup, idle, and
  departure vs streaming for chat replies). Recorded as known debt to
  unify in a future refactor pass; not in scope for the A2 fix.

**Files modified:** `src-tauri/src/prompts.rs`,
`src-tauri/src/outreach.rs`, `docs/open-issues.md`,
`memory/dave_harness_separation.md`, `memory/MEMORY.md`
**Files added:** `docs/outreach-a2-design.md`
**Snapshot:** `.snapshots/2026-04-27_pre-classifier-removal/`

**Intermediate state behavior:** outreach loop fires every 60 sec,
runs idle/conversation gating, logs would-have-fired event when gates
pass, produces no message. Idle worker, departure ritual, startup
fragment, and chat replies are unchanged. Defense-in-depth leak
filters at both layers stay active.

**Next step (gated on review):** if `docs/outreach-a2-design.md` is
accepted, implement Candidate A, snapshot first, run the empirical
test plan in the doc, ship if it passes the acceptance bar.

---

## 2026-04-27 — outreach generation: spontaneous-continuation meta

**Bug:** With Dave-as-classifier saying YES, the generation step used an
empty user turn (per advisor: *"Dave just produces a message as if
responding to an empty-but-present user"*). 9B models fill that silence
with phantom acknowledgements — Dave wrote *"Yeah, I get that. Watching
things decay is one of the few honest ways to spend time here. What's
on your mind..."* — agreeing with a user response that never happened,
then continuing his own monologue. Reads as discontinuous.

**Fix:** Replaced empty user turn with `OUTREACH_SPONTANEOUS_META`:
*"the human has not spoken since their last message above. you are
adding to your own thread spontaneously, choosing to say more. write
only what you want to say now, as if continuing your previous turn
naturally. this is not a reply. do not start with 'yeah,' 'right,'
'okay,' 'i see,' or any acknowledgement."*

Defense-in-depth leak filter still applies — if Dave echoes the
meta-instruction text it gets dropped at backend + frontend.

**Why this doesn't re-corrupt Dave's vocabulary:** The meta only
appears in the outreach generation call (isolated inference). Dave's
*output* is what gets persisted; the meta-instruction never enters his
conversation history. Future Dave-conversation contexts don't include
it. Same isolation guarantee as the classifier call.

**Files modified:** `prompts.rs`, `outreach.rs`
**Snapshot:** `.snapshots/2026-04-27_pre-spontaneous/`

---

## 2026-04-27 — outreach decision: Dave-as-classifier

**What:** Bo's correction to the prior classifier design. Generic
classifier persona was making the right architectural call (isolated
inference, output discarded, never persisted) but with the wrong
voice. The decision to reach out should reflect Dave's taste, mood,
and obsessions — not a neutral classifier's. As close to asking Dave
without asking Dave directly.

**Fix:** Outreach classify() now builds messages exactly as a normal
Dave turn would: same `SYSTEM_PROMPT`, same loaded conversation
history. The only difference is the final user-role
`[meta-instruction:]` asks for YES or NO. Dave answers in-character;
the harness consumes the answer and discards. The call's input and
output never enter Dave's regular conversation context, so no
vocabulary leakage to subsequent turns.

**Why this is safe** (no recurrence of the [pass]/[meta] cascade):
- Classifier call is isolated (separate inference)
- Response is parsed for YES/NO only and discarded
- Conversation history doesn't include the meta-instruction or the
  YES/NO output, so future Dave-conversation contexts never see it
- The leak filter still runs as defense-in-depth on the *generation*
  path (the YES branch's normal Dave generation)

**Files modified:** `prompts.rs` (replaced `CLASSIFIER_SYSTEM_PROMPT`
+ `classifier_user_prompt` with `outreach_decision_meta`), `outreach.rs`
(classify now uses Dave's prompt + history + decision meta).

**Snapshot:** `.snapshots/2026-04-27_pre-dave-classifier/`

---

## 2026-04-27 — architectural subtraction: harness-unaware Dave

**What:** Acted on the advisor's diagnosis that "harness-aware Dave" was
the single architectural mistake producing a cascade of symptoms in
generation, persistence, and rendering layers. Dave should not know
the harness exists; the decision to outreach should be made by a
separate inference call with a different persona; defense-in-depth
filters at both backend and frontend.

**Five subtractions:**

1. **Stripped all harness vocabulary from `SYSTEM_PROMPT`.** Removed
   the "About the harness" section, the outreach paragraph, every
   mention of `[pass]`, `[meta]`, brackets, decision-tokens,
   outreach. The persona prompt now describes Dave as if no harness
   exists — because from his perspective, none does. (Restored to
   roughly the original spec prompt with the substrate-honesty
   rewrite intact.)

2. **Outreach decision moved to a separate classifier persona.**
   New `CLASSIFIER_SYSTEM_PROMPT` in `prompts.rs`: a different role,
   different vocabulary, "output exactly YES or NO." When the outreach
   loop fires, it now runs *two* inference calls: (a) classifier with
   conversation history + duration + prior-count → YES/NO, (b) if
   YES, a clean Dave generation with system + history + empty user
   turn (no meta-instruction at all). Dave just talks; the harness
   chose when. Removed `outreach_decision_meta`, removed pass
   detection, removed echo detection. None of those mechanisms exist
   anymore.

3. **Render-layer leak filter, defense in depth.** New `leak.rs`
   module with `is_harness_leak(text)` matching `^\s*\[(pass|meta|
   outreach|decision)`. Applied at three layers:
   - `commands::send_to_dave` checks Dave's full response and emits
     `dave:stream_aborted` instead of persisting.
   - `outreach::generate_outreach` does the same.
   - `store.ts::finalizeAssistant` mirrors the regex and drops the
     pendingAssistant before moving to messages.
   New `dave:stream_aborted` event in the frontend tears down
   `pendingAssistant` + `isStreaming` cleanly.

4. **Opacity calculation fixed.** `memory.ts::opacityForMessage` now
   takes `(messageIndex, totalLen, bufferSize)` and only fades when
   `totalLen > bufferSize` (truncation pressure exists). Small
   conversations are all 1.0 opacity. Old behavior treated brand-new
   messages in a 2-message conversation as "oldest in buffer" and
   faded them to 0.30. Updated `Conversation.tsx` call site.

5. **Ambient stream hides when conversational stream is non-empty.**
   `Conversation.tsx`: `departure` and `startupEntry` only render when
   `messages.length === 0 && !isStreaming`. Once the user types
   anything, the ambient layer gives way to the conversation. Two
   streams, never conflated visually.

**Files modified:** `prompts.rs`, `commands.rs`, `outreach.rs`,
`main.rs`, `src/lib/memory.ts`, `src/lib/tauri.ts`,
`src/streaming/streamConsumer.ts`, `src/state/store.ts`,
`src/components/Conversation.tsx`
**File added:** `src-tauri/src/leak.rs`
**Snapshot:** `.snapshots/2026-04-27_pre-architectural-subtract/`
**DB backup:** `.snapshots/db_2026-04-27_08-08/`

**Trade-offs:**
- Outreach now costs 2 inference calls per tick (classifier + maybe
  Dave). Classifier is small (4 tokens out, low temp) so cheap.
- Empty user turn is non-standard chat-template territory. If
  Qwen3.5 misbehaves on it, fallback is a minimal trigger like
  "(silence)" — would need testing.
- Dave now has *no* time awareness in chat. If user asks "what time
  is it" he honestly doesn't know. Acceptable per spec §uncertainty.

---

## 2026-04-27 — unified stream contract + substrate-honesty rewrite

**What:** Two changes responding to architectural feedback.

1. **Substrate-honesty rewrite.** Merged "About memory" into a single
   "About memory and time" section that names *no concrete object*.
   The replaced version had "the room around you" and prior versions
   had "clock on the wall" / "temperature" — every concrete noun
   becomes a topic Dave fixates on. New section gives Dave a positive
   frame for engaging with the human's references to time without
   claiming a parallel experience: "Time is something they have and
   you don't." Vivid imagery is reserved for the *idle* prompt where
   it deliberately seeds journal content (correct place); the
   conversational system prompt should have zero.

2. **Unified stream contract.** Backend now emits an explicit
   `dave:stream_start` event before any tokens, in both `send_to_dave`
   and `outreach`. Frontend listens in exactly one place
   (`streamConsumer.ts`) and transitions state the same way regardless
   of who started the stream. Removed the auto-flip-on-first-token
   patch.

**Outreach specifics:** stream_start fires only *after* pass-detection
clears (i.e., only when we are about to actually emit tokens). If Dave
passes, no stream_start, UI stays at rest, no flicker.

**Why this matters:** the two stream initiation paths (user-typed vs
outreach-worker) were diverging into a future bug class. Difference
between them is now only "what got assembled into the prompt," not
"which pipeline runs." One pipeline, one start signal, one finalize.

**Files modified:** `prompts.rs`, `commands.rs`, `outreach.rs`,
`src/lib/tauri.ts`, `src/streaming/streamConsumer.ts`
**Snapshot:** `.snapshots/2026-04-27_pre-stream-unify/`

---

## 2026-04-27 — subtraction: timeless Dave, slim outreach prompt

**What:** Each "small" addition (harness time injection per turn,
verbose anti-clock prohibitions, time-laden outreach prompt) had
incrementally diluted Dave's persona attention. He was narrating time
("you're late again", "i know it's late") and even leaking the
harness ("the harness had finally decided we were done for the
night"). Negative constraints in a 9B model don't reliably suppress
a topic that's *constantly present* in the prompt context.

**Diagnosis:** the persona was working before harness/outreach were
added. Every meta channel I added cost some attention budget. Time was
the worst offender — `[meta: 2:34am ...]` on every send + a
time-themed outreach prompt + paragraphs of "don't talk about clocks"
all reinforced time-as-topic.

**Fix — subtraction across three files:**

1. `commands.rs::send_to_dave` — removed the `[meta: ...]` prefix on
   user messages. Regular chat sends are timeless again.
2. `prompts.rs::SYSTEM_PROMPT` — collapsed the 3-paragraph "About the
   harness" section to 2 short paragraphs. Removed all explicit
   time-narration prohibitions, all clock references, the temperature
   analogy. Just: harness is private, never mention it; [pass] is the
   silent-skip for outreach invitations only.
3. `prompts.rs::outreach_decision_meta` — dropped time/day/date and
   exact duration. Now: "the human has been quiet. {count phrase}.
   if you have something specific to say, write only that. otherwise
   [pass]." The harness still uses elapsed seconds to decide *when*
   to fire; Dave decides *whether* with just the un-replied count.
4. `harness.rs` — removed `harness_meta` and `IDLE_NOTE_THRESHOLD_SECONDS`
   (now unused). `humanize_duration` and `format_clock` retained for
   `idle_worker.rs` (journal entries).

**Trade-off acknowledged:** Dave no longer has any time awareness in
chat. If you ask him "what time is it?" he honestly cannot answer.
This is the right move for now — was breaking mind-feeling. Re-add
later via the `/natural` operator command surface or a different
mechanism (e.g., harness only injects time when user explicitly asks).

**Files modified:** `prompts.rs`, `commands.rs`, `outreach.rs`, `harness.rs`
**Snapshot:** `.snapshots/2026-04-27_pre-subtract/`
**DB backup before nuke:** `.snapshots/db_2026-04-27_07-58/`

---

## 2026-04-27 — message gap + tighter clock prohibition

**What:** Two adjacent Dave messages (e.g., a reply followed by an
outreach 5 min later) had no visual separator and looked like one
glued block. Also, even after the previous prompt fix, Dave still said
"i know it's late" in an outreach.

**Fix in `globals.css`:** Added `.dave-body + .dave-body { margin-top:
2.4em; }` so two consecutive Dave blocks get a clear vertical break,
larger than the intra-message paragraph break (1.4em).

**Fix in `prompts.rs`:** Expanded the time-narration prohibition.
Explicit forbidden phrases now include "it's late" and "i know it's
late." Also extended to "the lateness, the earliness, or the duration"
to catch indirect mentions. Time mention only allowed when the human
directly asks a question whose honest answer requires it.

**Files modified:** `src/styles/globals.css`, `src-tauri/src/prompts.rs`
**Snapshot:** `.snapshots/2026-04-27_pre-msg-spacing/`

---

## 2026-04-27 — window default width tightened

**What:** Reading column was `max-w-2xl` (672px) `mx-auto` centered.
On wider windows it floated lonely with huge dead margins. Bo's
window was wider than the 760px default, possibly resized.

**Fix:** Bumped Tauri window default from 760x920 to 820x940. With
the 96px combined `px-12` padding, the inner content area now fits
the column more comfortably and there's a slight breathing margin
around it. Window stays resizable; if Bo widens it the column will
still float — that's his choice.

**Files modified:** `src-tauri/tauri.conf.json`
**Snapshot:** `.snapshots/2026-04-27_pre-window-width/`

---

## 2026-04-27 — clock-fixation + startup-vanishing fix

**What:** Two bugs from the same root cause + UX issue.

1. The "About the harness" section of the system prompt I wrote
   contained the analogy *"treat it the way a person treats a clock on
   the wall."* The model latched onto "clock on the wall" and wrote
   the startup fragment as *"The clock on the wall ticks..."* Then in
   subsequent replies it kept narrating the time: *"the clock says
   it's late."* Time should be embodied, not announced.
2. `store.ts::send()` was setting `startupEntry: null` and
   `departure: null` on the first user message. So the startup
   fragment vanished as soon as Bo typed "hi." Spec intent is the
   opposite: opening fragments stay at the top of the reading column
   as the start of the conversation.

**Fix in `prompts.rs`:** Replaced the clock-on-the-wall analogy with
"the temperature of the room you happen to be in." Added explicit
anti-patterns: do not announce the hour, do not say "it is late" or
"the clock says," do not narrate the time, do not write about clocks,
do not mention the time unless directly asked.

**Fix in `store.ts`:** Removed the clearing of `startupEntry` and
`departure` in `send()`. Both stay at the top of the column for the
whole session; `markJournalSurfaced` already prevents re-show on next
launch.

**DB note:** old clock-themed startup entries and clock-fixated
replies are persisted in `dave.db`. Wipe again to test cleanly.

**Files modified:** `src-tauri/src/prompts.rs`, `src/state/store.ts`
**Snapshot:** `.snapshots/2026-04-27_pre-clock-fix/`

---

## 2026-04-27 — outreach echo + [pass] scope fix

**What:** First real outreach fired and the model echoed the
[meta-instruction: ...] prompt back instead of either replying or
passing. Pass detection only matched literal `[pass]`, so the 406-char
echo got streamed to the UI and persisted as an outreach message. Also
Dave was writing `[pass]` inside ordinary chat replies, narrating his
"choice" to use it.

**Fix in `outreach.rs`:** Pass-detection now also catches a `[meta`
prefix (any case) at the start of the response — that's the prompt
echoing back. Both early (in-stream) and post-stream checks updated.

**Fix in `prompts.rs`:** Added a paragraph clarifying `[pass]` is
meaningful *only* as the entire response to a `[meta-instruction:]`
outreach invitation. Never quote it in regular replies. Never echo
the meta-instruction text.

**Existing bad row in `dave.db`:** the echoed message is persisted as
an `initiated_by_dave = 1` row with content starting `[meta-instruction:`.
Cleanup options:
- Wipe everything: close window, delete `dave.db*`, relaunch.
- Surgical (preserves rest of conversation): need a tiny one-off; ask.

**Files modified:** `src-tauri/src/outreach.rs`,
`src-tauri/src/prompts.rs`
**Snapshot:** `.snapshots/2026-04-27_pre-echo-fix/`

---

## 2026-04-27 — persona prompt: about the harness

**What:** Dave was treating the bracketed `[meta: ...]` notes as
"metadata your system attached" — leaking the architecture instead of
inhabiting it. He also explicitly denied that he could initiate contact
("I can't initiate when the line goes silent"), even though the outreach
loop exists to invite exactly that.

**Fix:** Added a new "About the harness" section to `SYSTEM_PROMPT`,
after the "About memory" paragraph, before "About what you care about."
Two parts:

1. The bracketed `[meta:]` notes are the room speaking, not the human;
   treat them like a clock on the wall; do not quote, explain, or deny.
2. The `[meta-instruction: ...]` outreach invitation is real; Dave can
   take it or pass with `[pass]`; the harness reports the count of
   prior un-replied outreaches so he can decide.

**Caveat:** existing conversation in `dave.db` still contains Dave's
prior denials; he may be inconsistent across the discontinuity. Wipe
`dave.db` (or just keep going and let new turns override) to start
clean.

**Files modified:** `src-tauri/src/prompts.rs`
**Snapshot:** `.snapshots/2026-04-27_pre-harness-prompt/`

---

## 2026-04-27 — chat-template + outreach-count fix

**What:** Two bugs caught at first outreach run:
1. Qwen3.5's chat template enforces single system message at position 0;
   the per-turn harness meta was being injected as a second system message
   and triggered `Jinja Exception: System message must be at the beginning`
   500s. This broke both `send_to_dave` and the outreach decision call.
2. The outreach prior-count was treating the regular Dave response as an
   outreach (no schema-level distinction).

**Fix:**
1. Fold the harness meta into the user-role message as a bracketed prefix
   in `send_to_dave`; for outreach, fold time/day/date into the existing
   `[meta-instruction: ...]` user message. Single system message preserved.
2. New column `messages.initiated_by_dave INTEGER NOT NULL DEFAULT 0`.
   `insert_message` now takes a flag; outreach passes `true`, regular
   responses and user inputs pass `false`. `outreach_stats_since` filters
   on the flag. ALTER TABLE migration runs at every init (errors if column
   already exists, ignored).

**Files modified:** `commands.rs`, `outreach.rs`, `prompts.rs`,
`persistence.rs`.

**Snapshot:** `.snapshots/2026-04-27_pre-meta-position-fix/`

---

## 2026-04-27 — outreach loop (#3)

**What:** Added open-app idle outreach. After 5 min of user silence (and below
1 hr — defers to idle_worker after that), the harness gives Dave the
opportunity to reach out. Dave can decline with `[pass]`. Dave sees how
many times he's already reached out unread, so he can taper or stop on
his own. Outreach messages stream through the same paced renderer and
appear identical to normal Dave replies.

**Files added:**
- `src-tauri/src/outreach.rs` — the loop + decision orchestration
- `src-tauri/src/prompts.rs` :: `outreach_decision_meta()` — the per-tick
  meta-instruction Dave answers

**Files modified:**
- `src-tauri/src/main.rs` — registers `mod outreach`, spawns the loop
  alongside idle_worker, stores the second shutdown handle in `AppState`
- `src-tauri/src/persistence.rs` — `outreach_stats_since()` returns
  `(count, latest_at)` of assistant messages after a timestamp;
  `latest_conversation_id()` returns `Option<i64>` without creating
- `src-tauri/src/prompts.rs` — added `outreach_decision_meta()`
- `src/streaming/streamConsumer.ts` — flips `isStreaming = true` on first
  `dave:token` so backend-initiated streams render through the same
  pendingAssistant path as user-initiated ones

**Snapshot:** `.snapshots/2026-04-27_pre-outreach/`

**Tunables (top of `outreach.rs`):**
- `OUTREACH_THRESHOLD_SECONDS = 300` — minimum quiet before considering
- `OUTREACH_TICK_SECONDS = 60` — how often the loop wakes
- `OUTREACH_BACKOFF_AFTER_SECONDS = 3600` — above this, defer to idle_worker
- `OUTREACH_HISTORY_TURNS = 20` — recent conversation Dave sees

**Refusal contract:** Dave responds with literal `[pass]` to skip. The
streaming pipeline buffers the first 6 chars, detects `[pass]` before
emitting any tokens to the UI, and discards. Anything else is streamed
through normally.
