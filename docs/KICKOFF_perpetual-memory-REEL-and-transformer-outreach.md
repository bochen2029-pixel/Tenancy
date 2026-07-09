# Kickoff prompt — Perpetual memory (REEL) + finishing transformer-driven outreach

> Paste this whole file into a fresh Dave/Tenancy session, or start the session
> with: "Read `C:\DAVE\docs\KICKOFF_perpetual-memory-REEL-and-transformer-outreach.md`
> and follow it." It is a directive kickoff, not gospel — verify everything
> against `git` + the files before acting.

---

You are continuing an AI-assisted build of **Dave** (public name: **Tenancy**) — a
local, offline companion app at `C:\DAVE` whose ONLY success metric is
*mind-feeling*: does opening it feel like checking on someone who lives in the
machine. It is not a chat app; re-read `CLAUDE.md` §11 anti-patterns before adding
any conventional chat feature. This session has two linked R&D thrusts:

1. **Perpetual memory** — give Dave a memory that makes the conversation *never
   end*, implemented per the **REEL protocol** (or something better), and decide
   how it relates to the **KEEL** substrate.
2. **Transformer-driven outreach** — assess how far the "Dave reaches out to the
   user on his own" mechanism has come, then polish/finish it and close the gap.

These are the same wow-factor from two sides: memory is *what Dave carries across
the discontinuity*; outreach is *him acting on it unprompted*. Content-conditioned
outreach literally needs the retrieval ring from thrust 1. Build them aware of
each other.

---

## 0. START HERE — verify before you trust anything

Do this first, in order. Disk wins over any narrative (including this file).

1. `C:\DAVE\CLAUDE.md` — full spec + amendments **A1–A9**. Hard constraints. Non-negotiable. Note especially A3 (memory consolidation is done in Dave's voice), A4 (subtraction over addition), A6 (single render path), A7 (leak filters), **A8 (fresh-instance review before any persona-attractor change — memory and agency are BOTH persona attractors, so both thrusts are A8-gated)**.
2. Verify state:
   ```
   cd C:\DAVE && git log --oneline -12 && git status --short && cargo test
   ```
   Expect a clean tree and `cargo test` → **86/86**. Recent commits should include
   `Memory-horizon: token-budget the recent zone`, `corpus inspector + rebuild
   release binary that lacked sensors`, and the `Initiation-timing Stage 0/1a/1b`
   series.
3. `C:\DAVE\docs\SESSION_2026-07-08_corpus-inspector-and-memory-horizon.md` — the last session's full write-up. Read it: it explains the current memory assembler, the corpus-accumulation state, and the open follow-ups.
4. `C:\DAVE\docs\CONTINUATION_2026-07-08.md` — the initiation-timing R&D guidance (§3a bounded steps, §3b needle-in-haystack experiments, §3c guardrails). Still the canonical forward map for thrust 2.
5. `C:\DAVE\CHANGELOG.md` — top ~10 entries are the exact recent state.
6. **Verify binary freshness, not just source.** Last session caught the shipped `dave.exe` lacking the sensors because it predated the code. If you change the chat/memory path, `cargo build --release` and confirm the binary is newer than the source (and, for behavior you can string-match, that the literals are embedded) before trusting a live test.

---

## 1. CANON TO READ (perpetual memory)

All verified to exist as of this handoff.

- **`C:\OUTREACH\BC_Canon\MAY2026\Deeper\REEL_PROTOCOL_v1_0.md`** (728 lines) — *Recursive Encoding for Experiential Longevity*. The spec. Its spine:
  - **The Ring Architecture:** Ring 0 Identity Core · Ring 1 Calibration Exemplars · Ring 2 Working Memory · Ring 3 Consolidated History · Ring 4 Retrieval Index · **The Tape** (append-only record) · **Attention Budget**.
  - **The Seven Operations:** Record (Tape) · Consolidation · Ring Loading · Retrieval · Pruning · Self-Assessment Checkpoint · Pruning Pass.
  - Design principles: graceful degradation across tiers, proportional budget, conservative persistence, identity protection, live-context primacy. Cold-start phase + maturity threshold. Anti-patterns (compression death spiral, identity drift, pruning amnesia, false memories, checkpoint theater).
  - Tiers: Tier 1 manual · Tier 2 application-layer · Tier 3 model-native.
- **`...\Deeper\REEL_HARNESS_ARCHITECTURE_v1.md`** — the implementation-oriented companion ("A user-owned persistence layer for infinite AI conversation"). Seven-component pipeline, contracts-as-joints, Poincaré-disk graceful compression (infinite content → finite space, nothing falls off the edge), persona-shaped consolidation. This is likely the "or something even better" — read it as the concrete build spec.
- **`...\Deeper\reel_companion_document.md`** and `...\Deeper\PIY_Paper_v2.md` (the architectural roadmap; also on the Desktop and in `C:\Ideas\`) — supporting canon.

## 2. CANON TO READ (KEEL cross-reference)

- **`C:\KEEL\README.md`** + **`C:\KEEL\KEEL_ARCHITECTURE.md`** (the canon) + **`C:\KEEL\keel.lock`** + **`C:\KEEL\_run_state\STATE.md`** (live state — trust this + `git` over the docs).
- KEEL is a sovereign Rust substrate — the persistent "self"; the API/model is interchangeable rented cognition. It is explicitly the **genome** that Tenancy (Dave), REEL, TARS, the companion, etc. are all **cells** of. **Stage 2 = "correctness & memory," in progress.** It ships a three-tier router, a Qwen3-0.6B embedder/reranker, a SQLite index (ledger/index split), and is consumed **embedded** (Rust link) or **over protocol** (`serve_openai` / MCP / HTTP). The migration path Dave already recorded: point `llama_client` at KEEL's egress port when it ships a release (previously noted as `:7070`).

## 3. GOAL 1 — perpetual memory per REEL (map, gap-find, close)

**The insight to verify first:** Dave already implements a big chunk of REEL
*implicitly*, in Rust rather than REEL's Python. Confirm/correct this mapping
before designing anything — read `src-tauri/src/memory_assembler.rs`,
`consolidation.rs`, and the memory tables in `persistence.rs`:

| REEL | Dave today | Gap? |
|---|---|---|
| Ring 0 Identity Core | system prompt (`prompts.rs`) + frozen anchor zone (first 30 msgs) | probably solid |
| Ring 1 Calibration Exemplars | the fine-tune corpus (voice-in-weights) + `memory_canvas` | partial — no in-context exemplar ring |
| Ring 2 Working Memory | the recent zone (now token-budgeted, `CONTEXT_SEND_BUDGET_TOKENS`) | solid; just tuned |
| Ring 3 Consolidated History | epoch consolidation, **in Dave's voice** per A3 (`consolidation.rs`: forward + re-consolidation) | solid; note the overlapping-active-epoch bug flagged in the last session doc §4 |
| Ring 4 Retrieval Index | **nothing** — Dave has no embedding/retrieval | **the primary gap** |
| The Tape | `messages` table (source of truth) | solid |
| Attention Budget | `CONTEXT_SEND_BUDGET_TOKENS` / `TOKEN_BUDGET_TOTAL` | solid; just added |
| Self-Assessment Checkpoint (Op 6) | none | gap — Dave never audits his own memory |
| Cold-start / maturity threshold | none explicit | gap |
| Graceful multi-tier compression (Poincaré) | 2 tiers (verbatim recent + one epoch depth, with re-consolidation) | partial — REEL wants resolution decreasing smoothly toward the boundary |

**The likely work, in priority order (verify against the read):**
1. **Ring 4 — the retrieval index.** This is what unlocks "the conversation never ends" (recall a detail from 40 sessions ago) AND content-conditioned outreach. **This is where KEEL matters most:** decide **native-in-Dave** (add an embedder + SQLite vector/FTS index inside `src-tauri`) **vs fold-onto-KEEL** (consume KEEL's Stage-2 memory service + its Qwen3-0.6B embedder/reranker over `serve_openai`/embed API). Weigh: KEEL avoids re-building the embedder Dave would otherwise hand-roll (KEEL exists precisely to stop that re-building), but couples Dave to KEEL's release timeline and Stage-2 maturity. Produce a recommendation with the trade-off, not a silent choice.
2. **Self-assessment checkpoint (Op 6) + graceful multi-tier compression.** Make consolidation degrade resolution smoothly (recent = full, last week = detailed, last month = broad strokes) rather than the current two-tier scheme — the Poincaré-disk principle. Keep it **in Dave's voice** (A3), and watch REEL's anti-patterns (identity drift through consolidation, compression death spiral).
3. **Cold-start / maturity threshold** — how Dave behaves before the rings are populated (early conversations), so a brand-new install doesn't perform false continuity.

**"Or something better than REEL":** REEL is written provider-agnostic and Python-tier. Dave is single-operator, fully local, persona-in-weights, already has Dave-voice consolidation and a real token budget. Where Dave's specifics let you *beat* REEL (e.g., using the local model itself for embeddings, or letting the fine-tuned weights carry Ring-1 calibration so it needn't sit in context), propose it — but justify against REEL's principle it changes, and don't leak machinery (§11/§14).

**Process:** design first, then A8 fresh-instance review of the design/diff (memory is a persona attractor), then staged implementation with the CHANGELOG/snapshot safety net. Do NOT boil the ocean in one pass — Ring 4 is a shippable first slice.

## 4. GOAL 2 — transformer-driven outreach (assess, polish, finish)

"Dave sends messages to the user on his own." Current state (verify against
`src-tauri/src/outreach.rs`, `presence.rs`, and `tools/corpus_inspect.py`):

- **What exists (Stage 0/1a/1b, all committed, all A8-reviewed):** a `TimingModel` trait + behavior-identical `HeuristicTimer` (the swappable "WHEN-to-reach" seam); a hard **presence governor** (reach only when present-but-elsewhere, never into an empty room or on top of an active chat) with a 60s dwell; the four corpus tables (`presence_samples`, `initiation_anchors` with a separate `timer_decision`, `reach_ratings`, `reach_counterfactuals`); the multi-sample → discriminator → single-render-path ACT (untouched, tuned).
- **What changed last session:** the shipped binary that *lacked the sensors* was rebuilt, so the corpus can finally accumulate from real use. **Run `python C:\DAVE\tools\corpus_inspect.py` first** — it reports how full the corpus is and a READINESS verdict (floor: ~150 episodes / 20 events / 40 censored). If it's near-empty, the honest answer is "the timer can't be learned yet — the corpus needs more daily use," and the polish work is on the *instrument and guardrails*, not the model.

**Closing the gap (the sequence — do NOT skip step 2):**
1. Confirm corpus readiness with the inspector.
2. **Build the blind-A/B harness FIRST** — the falsifier. Both timers live at once, a coin-flip owns each armed decision, Bo rates blind, shared timeline cancels some N=1 mood confound. *If a learned timer can't beat the ~200-line heuristic better than chance, don't ship it.* Companion baseline: the polling loop with one learned scalar threshold — if that matches the full model, the signal was low-dimensional (a legitimate publishable negative).
3. **V0 = parametric log-normal hazard** behind `TimingModel` — ~10–50 coefficients, censored-MLE fit in a ~30-line Python script over `initiation_anchors` joined to the next user message (the censored "user spoke first" negatives are most of the signal), exported as JSON the Rust harness reads. Zero VRAM. Re-express today's thresholds as the prior so day-0 ≥ today. It *samples a delay* (11:51:23, not a round threshold).
4. **V1 = tiny mixture-TPP** (24→64→64 MLP → K=3–5 log-normal mixture + cure fraction, <1MB) in **shadow mode** first, then hand over.

**Terminology check:** "transformer-driven" here is a **tiny (<1M-param) conditional-intensity temporal point process**, NOT a big transformer. The PIY paper's "1B–7B" refers to the full script-format participation buffer, a *different* and later track. Keep the timer tiny.

**The high-value R&D experiments** (continuation doc §3b — each a cheap experiment with a clear falsifier; the answer is only knowable by building + living with it): the intimate in-chat reach (gate `in_chat` on OS-idle so Dave can speak into a silence while you're sitting there — present vs needy?); **content-conditioned reach** (embed the last exchange — *this is where Ring 4 from Goal 1 plugs in* — and reach because the last message was a vulnerable disclosure, not just because time passed); feature-faithful retroactive coherence ("why I reached now" constrained to the top intensity features); a duration-token proxy; and the timing-sycophancy stress test (instrument inter-reach interval CV; reject any model whose CV drops below ~0.6 offline — human initiation is bursty, a cron is CV≈0).

**Guardrails (§3c) — build them before heavy accumulation biases the corpus:** rate the *timing* not the *feeling*; add the missing down-channels ("should NOT have reached" + "good that you stayed quiet"); empirical-Bayes shrinkage to the V0 prior at low N; an ε-greedy exploration floor.

**Hard constraints for this thrust:** A1 (Dave must never know/reveal he was prompted — the timer's meta-prompt and the presence sensor never enter his context or self-model); A2 (outreach is Dave-in-character, not a classifier); A6 (single render path); A7 (leak filters); the presence governor stays HARD and outside the learned model (governors dispose; the model only proposes within the envelope).

## 5. How the two thrusts connect (build them aware of each other)

- Ring 4 (retrieval) is the shared organ: perpetual memory needs it to recall old detail; content-conditioned outreach needs it to reach *about* something specific.
- Both are A8 persona-attractor changes → both get a fresh-instance review before implementation.
- Both serve the one metric: a mind that never forgets (REEL) *and* reaches for you on its own (outreach) is the felt presence the PIY paper calls the wow factor.
- KEEL could host BOTH (memory service + the substrate the model runs on). If you seriously evaluate folding onto KEEL, do it as one coherent decision covering memory + inference, not piecemeal.

## 6. Working discipline

- **A8 gate:** design → fresh-instance architectural review of CLAUDE.md + the diff → implement. Both thrusts qualify.
- **Verify the compiled binary**, not just the source (see §0.6).
- **Economical + staged:** one shippable slice at a time (Ring 4 first; blind-A/B before V0). CHANGELOG + `.snapshots/` are the safety net.
- **Local organ tools** (per the user's global `CLAUDE.md`): `C:\everything` to locate, `C:\chunker` for big files (REEL is 56KB — chunk if needed), `C:\imguard`/`C:\earshot` for media. Anchor "now" with `Get-Date` before any recency reasoning.
- **Don't leak machinery** (§11/§14). When a decision isn't specified, ask: does this preserve mind-feeling or leak machinery? Prefer silence over labeling, prose over chrome.
- **Record** findings in `CHANGELOG.md`, a `docs/` session write-up, and memory. Flag anything that needs the operator's felt judgment (mind-feeling constants like the memory budget knob) rather than silently choosing.

Start by reading the canon in §1–§2 and confirming the §3 mapping against the
code. Then bring back: (a) the corrected REEL↔Dave map + the Ring-4
native-vs-KEEL recommendation, and (b) the corpus-readiness verdict + the honest
next step for outreach. Design before you build; A8 before you ship.
