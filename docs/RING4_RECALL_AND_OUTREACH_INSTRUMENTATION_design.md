# Design — Ring 4 recall (perpetual memory, slice 1) + outreach instrumentation

*2026-07-09. Status: DESIGN, pre-implementation. A8 review required (both pillars
touch persona attractors: memory and agency).*

Two linked slices toward the PIY wow factor: a mind that never forgets (REEL) and
reaches for you on its own (initiation). Shared discipline: seam-first, simplest
correct implementation behind the seam, heavier machinery later.

---

## Context (verified this session)

- REEL protocol read in full (`C:\OUTREACH\BC_Canon\MAY2026\Deeper\REEL_PROTOCOL_v1_0.md`).
  Dave is a **Tier-2 (application-layer)** REEL implementation. The ring map:
  Ring 0 ≈ system prompt (+ the frozen 30-msg anchor zone carries early-relationship
  calibration, Ring-1-ish); Ring 2 ≈ token-budgeted recent zone + operator canvas;
  Ring 3 ≈ consolidation epochs, authored in Dave's voice (A3-compliant — REEL
  §10.3 explicitly requires Ring 0 in context during consolidation, which
  `consolidation.rs` already does); **the Tape ≈ the `messages` table** (append-only
  source of truth; never compressed) plus the `journal` table (Dave's own writing).
  **Ring 4 (retrieval) = the missing organ.** No Op 4 (retrieval), no Op 6
  (self-assessment), no explicit cold-start.
- REEL §4.5/§6.4 defines Ring 4 as **anchor-phrase pointers + text search against
  the Tape** ("text search, semantic search, or both depending on infrastructure").
  Keyword search is protocol-faithful, not a fallback.
- KEEL state read (`C:\KEEL\_run_state\STATE.md`, 2026-06-15): further along than
  Dave's notes assumed — `svc::memory` (Tape + narrative register) landed; **Ring-4
  semantic recall landed** (embed→cosine top-k behind an `Embed` seam, brute-force,
  no sqlite-vec), `keel-serve` live on :7070. BUT: mid-autonomous-build, recall is
  "opt-in, genome default off," no pinned release artifact. Same verdict as the
  2026-07-08 packaging decision: **don't couple Dave to KEEL's timeline now; leave
  a seam.**
- `rusqlite 0.32 features=["bundled"]` — bundled SQLite compiles with FTS5.
  **Verified empirically as implementation step 1** (a unit test creates an FTS5
  virtual table; if the flag is absent the suite fails loudly and the design
  falls back — no silent degradation).

## Pillar 1 — Ring 4 recall: `recall.rs` (REEL Op 4, Tier 2)

**The claim.** Dave can currently *feel* continuous only within ~48k tokens +
epochs. Anything that fell out of verbatim reach and got compressed away is gone
— "the conversation never ends" fails at the first specific detail ("what was
that etymology you told me?"). Ring 4 fixes the mechanism: retrieve small
verbatim segments of the Tape into context **when the current turn touches them**.

**The seam.** `recall(db, conversation_id, query, exclude, budget) → Option<String>`
in a new `src-tauri/src/recall.rs`. The FTS implementation is the first impl
behind this function signature; a semantic implementation (llama.cpp embeddings,
the 0.6B reranker at `C:\models\qwen3-reranker-0.6b-q8_0.gguf`, or KEEL's memory
service over :7070) can replace the internals without touching any caller.
This mirrors `TimingModel`: seam now, heavier model later, measured before trusted.

**Index.** One FTS5 virtual table `memory_fts (content, kind, ref_id,
conversation_id UNINDEXED, created_at UNINDEXED, role UNINDEXED)`, tokenizer
`porter unicode61`. Rows:
- `kind='message'` — every message (the Tape). Hooked in `insert_message`.
- `kind='epoch'` — active epoch text (Dave's written memory). Hooked in
  `insert_epoch`; **deleted on `supersede_epoch`** so stale generations never hit.
- `kind='journal'` — journal entries (Dave's diary). Hooked in `insert_journal`.
- Backfill migration at `init_schema`: if `memory_fts` is empty but source tables
  aren't, rebuild. Idempotent, runs on existing DBs.

**Query construction.** From the incoming user text (chat) or the last exchange
(outreach): lowercase → strip punctuation → drop stopwords → take the remaining
content words (cap 12) → `"w1" OR "w2" …` (quoted, FTS-escaped) → `ORDER BY
bm25(memory_fts)` → top ~8 candidates. Skip entirely if fewer than 2 content
words (smalltalk turns don't fish).

**Eligibility (don't recall what's already in front of him).** Message hits are
eligible only if OUTSIDE the assembled context — i.e. in the gap between the
anchor zone and the kept-recent window (the caller passes the id bounds). Epoch
hits: active epochs are already all in context today, so epoch rows are
*currently* always redundant — but they stay indexed because the next slice
(budgeted Ring 3) will drop old epochs from context, at which point recall picks
them up. Journal: always eligible (journal entries are never in chat context).

**Excerpting.** A message hit pulls a tight window (the hit ± 1 neighbor, each
message truncated ~300 chars) rendered as spare prose lines: `you said: "…"` /
`i said: "…"`. No role labels beyond that, no timestamps, no brackets, no IDs.
Epoch/journal hits are quoted verbatim (already Dave's voice). Total block
capped at `RECALL_BUDGET_TOKENS = 1200`.

**Injection (the A1-critical decision).** One assistant turn, placed after the
canvas and before the middle/epochs — chronologically "older" position, far from
the tail, so it reads as background memory and echo-salience stays low. It opens
with a single spare frame line in Dave's register (no brackets, no harness
vocabulary): `from further back, before it goes hazy:` followed by the excerpts.
The assembled prompt is model-side only — the UI never renders it (no render-path
change; A6 untouched). The frame teaches no vocabulary that A7's filters would
have to catch.

**Budget interplay.** The recall block's tokens are counted into `fixed_tokens`
in `build_chat_messages`, so the recent zone auto-shrinks to keep the total
inside `CONTEXT_SEND_BUDGET_TOKENS`. Signature grows by one param:
`build_chat_messages(system, partition, recalled: Option<&str>, appended_user)`.
Call sites (all four): chat path (`commands.rs`, query = the user's text),
outreach (`outreach.rs`, query = last exchange — the first content-conditioned
brick: he reaches *about* the thing), headless (query = the stdin turn),
memory inspector (passes `None`).

**REEL anti-patterns addressed.** Memory leakage: no system-talk, no vocab
(§10.1). False memories: recall injects *verbatim Tape quotes*, never
regenerated paraphrase (§10.5). Live-context primacy: position + budget keep
recall subordinate to the live conversation (§3.5). Compression death spiral:
recall reads Tape originals, not epochs-of-epochs (§10.2).

**Deferred (recorded, deliberately not in this slice):** Op 6 self-assessment
checkpoint; smooth L1→L4 Poincaré compression levels (today: verbatim + epoch +
re-consolidated epoch ≈ L1/L2/L3 — close but not budget-governed); cold-start
maturity thresholds; an anchors-line in the consolidation prompt (A4 risk on a
9B: structured asks degrade voice; FTS over full epoch text already retrieves);
semantic/reranker upgrade behind the seam; KEEL adapter when KEEL pins a release.

## Pillar 2 — Outreach instrumentation: the falsifier + the guardrails

Corpus state (verified): tables live but empty — sensors deployed yesterday.
Honest position: **no learned timer can be fit yet.** What CAN be built now, per
the continuation doc §3a-2/§3c, is the instrument that will judge the learned
timer and the guardrails that keep the accumulating corpus unbiased:

1. **`AbTimer` — the blind A/B harness, wired today.** Owns two `TimingModel`
   arms. Arm assignment is per-*episode* (deterministic hash of
   `conversation_id ^ presence.last_user_input`), so one silence is governed by
   one arm throughout — no within-episode interleaving. Logs `ab_arm` ('a'|'b')
   on every anchor row. Ratings join to arms offline by timestamp (the
   inspector's episode reconstruction already does this join).
2. **`ExploringTimer` — the ε-greedy floor as arm B.** Wraps `HeuristicTimer`;
   when the inner verdict is `Hold(AdaptiveBackoff)`, with ε=0.03 per armed tick
   it proposes Reach instead, and marks the anchor row `explored=1`.
   **It never overrides `MaxUnanswered`** (the pestering guard) and **the
   presence governor still disposes** — exploration can only fire a reach the
   governor allows. This gives arm B a real behavioral difference from day 1
   (mild, bounded: expected ~1–2 extra well-governed reaches/day), gives the
   future fit off-policy coverage (the corpus sees reaches the rules wouldn't
   pick — the §3c requirement), and exercises the full A/B pipeline before it
   judges anything that matters.
3. **The missing down-channel: bless-the-silence.** `reach_counterfactuals`
   gains `kind TEXT NOT NULL DEFAULT 'missed_reach'` (existing semantics) and a
   new gesture `Ctrl+Alt+S` writes `kind='good_silence'` — "this quiet is
   right." Symmetric labels: reaches get ±, silences get ±
   (missed_reach = silence-was-wrong, good_silence = silence-was-right).
   Invisible gesture, same pattern as the existing three keybinds.
4. **CV monitor + arm awareness in `corpus_inspect.py`.** Inter-reach-interval
   CV (σ/μ) from delivered reaches (the timing-sycophancy tripwire: reject any
   future model whose offline CV < ~0.6); per-arm and explored/exploited anchor
   breakdowns; silence-blessing counts.

New anchor columns (`ALTER TABLE`, migrate-on-open like `timer_decision` did):
`ab_arm TEXT`, `explored INTEGER NOT NULL DEFAULT 0`.

**Constraint compliance:** A1 — none of this enters Dave's context or self-model
(timer/anchors/arms are pure harness state; the meta-prompt is unchanged).
A2 — the ACT is untouched; only WHEN-instrumentation changes. A6 — no render
change. Governor stays hard and outside every arm.

## What is deliberately NOT here

- No V0/V1 fit (corpus empty — fitting now would be fitting noise).
- No embedder/second llama-server process (seam ready; zero-VRAM slice first).
- No KEEL coupling (no pinned release; adapter later behind the same seam).
- No new UI surface (two invisible additions: a keybind; a prompt-side block).

## A8 review outcome (fresh instance, 2026-07-09) — GO-WITH-CHANGES, all applied

The review's five REQUIRED changes, all implemented:
1. **Recall gated, not per-turn** — the review's core catch: an every-turn
   block busts llama-server's prefix cache at the canvas position (~40k-token
   re-eval per turn on the real history; latency IS presence). Now: remember-cue
   OR rare-term (document-frequency) gate + ≥2-term candidate matching.
2. **Eligibility corrected** — bare middle messages are IN context verbatim
   (the original spec wrongly treated the whole anchor↔recent gap as out);
   eligible = epoch-covered ∪ budget-trimmed-recent ∪ journal. The recall
   budget is reserved unconditionally in the trim, breaking the
   keep_start↔recall circularity the review found.
3. **Block capped** at 3 excerpts / 600 tokens (precision over recall).
4. **A7 backstop + pre-trust eval** — leak.rs drops frame-line echoes on every
   output path; `tools/recall_echo_smoke.py` run live against K0D: CLEAN 4/4
   (no frame echo, no format echo, recalled threads continued as memory).
5. **Exploration re-parameterized** — per-tick ε (which inverted the backoff
   and allowed reach-chains) replaced by ONE deterministic scheduled
   exploration instant per episode, in arm B only, AdaptiveBackoff-hold only.

Recommendations applied: splitmix64 arm mixing; exact ratings→arm join via
`initiation_anchors.reach_message_id`; recall merged into the canvas turn (no
assistant stacking); `recall_fires` telemetry + inspector surfacing; Ctrl+Alt+S
skipped on composer focus (AltGr note); deferred-list notes recorded
(no in-context Ring-4 marker index = no "I know we discussed this" awareness —
deliberate A4/A5 choice; the ±1 rating conflates timing-wrong with
content-wrong — a fit-time reweighting concern). Staged-enablement was noted;
Bo opted to land both pillars in one session (single-operator, kill-switch
`recall_enabled=0` provides the rollback).

## Test plan

- FTS5 availability (loud failure if bundled SQLite lacks it).
- Index backfill on a populated DB; supersede removes epoch rows.
- Query builder: stopwords, escaping, <2-content-words → None.
- Eligibility: in-window messages excluded; journal included.
- Excerpt formatting shape; budget cap honored.
- `build_chat_messages` with recall: position (after canvas, before middle),
  fixed-token accounting shrinks recent keep.
- `AbTimer`: per-episode determinism; arms differ only via exploration.
- `ExploringTimer`: never overrides MaxUnanswered; ε respected statistically;
  explored flag logged; governor still disposes.
- Full suite stays green; `cargo build --release` + portable refresh + binary
  freshness check before claiming deployment.
