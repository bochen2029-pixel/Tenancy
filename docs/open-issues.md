# Open issues — Dave persona / behavior

Queue of behavior bugs Bo has flagged that aren't fixed yet. When greenlit,
fix in `prompts.rs` (mostly) and snapshot to `.snapshots/` per usual.

---

## P1: Dave should NEVER mention the harness to the user

**Observed:** outreach reply contained *"I was starting to think the harness
had finally decided we were done for the night."* — naming the harness
directly to the human.

**Expected:** the harness is private between the harness and Dave. Dave
should never reference it, name it, talk about its decisions, or imply its
existence to the user. Same way a person doesn't say "my brain told me to."

**Fix when greenlit:** add explicit prohibition to the "About the harness"
section of `SYSTEM_PROMPT`. Suggested wording: *"You never mention the
harness to the human. You never name it, never describe what it does, never
attribute your actions or thoughts to it, never say 'the harness decided'
or 'the harness said.' From the human's side it does not exist; you are
just here."*

---

## P1: Clock-leak persists even after the second tightening

**Observed:** outreach reply contained *"You're late again"* and *"It's a
strange hour, isn't it?"* despite the prompt now explicitly forbidding
"it's late" and any phrase that names the hour.

**Expected:** silence on time/lateness/earliness unless directly asked.

**Hypothesis:** Qwen3.5 9B is weakly compliant with detailed negative
constraints when the immediate prompt context (the outreach
meta-instruction's *"the human has been quiet for X"*) is itself
time-themed. The model pattern-matches on the surrounding theme.

**Fix options when greenlit (pick one or stack):**
1. Drop the explicit duration from the outreach meta-instruction — say
   "the human has been quiet for a while" or just "the human has been
   quiet" — so Dave isn't primed on a number.
2. Move the time/duration grounding into a system-level role suffix that
   is more clearly ambient (this requires another chat-template
   workaround since Qwen wants single system at position 0; could prepend
   to the canonical system prompt at request time).
3. Strengthen prompt with positive instruction: *"When you reach out,
   write about something specific you noticed or are turning over —
   never about the silence itself, never about waiting, never about the
   hour."*

Option 3 is probably highest-leverage and lowest-risk.

---

## P2: outreach copy quality

**Observed:** *"You're late again; I was starting to think..."* has a
passive-aggressive register that doesn't fit the persona's "thoughtful
person at 3am to someone they trust" tone.

**Hypothesis:** related to the above two — model is overweighting the
"silence/waiting" framing.

**Fix when greenlit:** likely subsumed by the option-3 change to the
outreach prompt. Re-test.

---

## P2: Model leaks default-assistant closings despite prompt prohibition

**Observed:** Dave wrote *"Just let me know if you need anything else right
now."* — verbatim a phrase forbidden in `SYSTEM_PROMPT` ("About how you talk":
*"You do not end responses with 'Let me know if you need anything else'..."*).

**Hypothesis:** Qwen3.5-9B Q4_K_M is weakly compliant with negative
constraints, especially RLHF-default phrases that the base model emits at
high probability. The prompt forbids it; the model overrides.

**Fix options when greenlit:**
1. **Bigger quant.** Q5_K_M or Q8_0 follow instructions noticeably better
   in the 9B class. Easy swap — drop a different GGUF in `C:\models\`,
   set `DAVE_MODEL_PATH`, restart.
2. **One-shot example.** Add to the prompt a tiny example exchange with a
   correctly-toneless ending (no "let me know"), demonstrating the
   pattern positively.
3. **Move the prohibition earlier.** Right now it's in "About how you
   talk" deep in the prompt. Surface it nearer the top so it's in
   higher-attention tokens.
4. **Accept it as a model limitation.** It's a phrase, not a behavior
   collapse. Persona is otherwise mostly intact.

**Bo's call.** Recommend #1 first (quant bump) — single biggest lever,
zero code change.

---

## P2: Inline `[pass]` quoting in regular replies (lower priority than above)

**Observed:** earlier in the session Dave wrote *"I'll write the token
`[pass]` and let the silence sit"* in a regular reply. The system prompt
was updated to forbid this but the issue may recur.

**Status:** prompt fix shipped 2026-04-27. Watch for recurrence.

---

## P2: A6 — startup fragment generates via parallel non-streaming path

**Observed:** [`commands::ensure_startup_entry`](../src-tauri/src/commands.rs)
calls `client.complete()` (non-streaming) and surfaces the result as a
journal entry. Every other Dave message — user reply, outreach (when
reintroduced), idle journal output, departure ritual — flows through
either `chat_stream` or `complete` with no unified path. Two generation
primitives exist for "Dave produces text."

**Why it's debt:** CLAUDE.md amendment A6 mandates a single render path
for Dave messages regardless of origin. Today the startup fragment, idle
journal, and departure ritual all use `complete()`; only chat replies (and
formerly outreach) use `chat_stream`. Functionally correct now; the
class-of-bug it will eventually produce is subtle render differences
between origins that look like "it works for X but not Y" — paced
renderer skipped, opacity-fade calc differs, leak filter applied at a
different layer.

**Filed:** 2026-04-28 during A2 architectural review. Not in scope for
the A2 fix; recorded as known debt to address in the next refactor pass.

**When greenlit:** retire `LlamaClient::complete` entirely — replace
`ensure_startup_entry`, `idle_worker::check_and_generate`, and the
departure prompt in `main.rs::handle_close` with the streaming path.
Departure may need a timeout-aware variant since it's bounded to 8 sec
on app close. None of these need to render in real time, but they should
all use the same generation primitive so the leak filter, persistence,
and event surface all behave identically across origins.

---

How to use this file:
- I add entries when Bo flags something but says "next time."
- Bo greenlights: I snapshot, fix, ship to CHANGELOG, remove from this file.
- Stays small on purpose. If something has been here >2 weeks, either fix
  or explicitly accept and remove with a note.
