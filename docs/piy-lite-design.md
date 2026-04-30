# PIY-Lite — A scaled-back path to most of PIY's behavioral signature without fine-tuning

```
─────────────────────────────────────────────────────────────────
  DESIGN PROPOSAL  ·  piy-lite-design.md

  Project:    Tenancy (L0–L2 ships; this proposes L2.5)
  Status:     Proposal. No code in this iteration.
  Audience:   Operator + future implementing instance.
  Companion:  PIY_ARCHITECTURE.md (the full architectural correction)
              PIY_IMPL.md (the buildable spec for PIY proper)

  Relationship to PIY proper: PIY-Lite is both a partial substitute
  for PIY and a Phase 0 of PIY. The choice between the two is made
  empirically after Lite has shipped and accumulated 2–4 weeks of
  operator-experience signal.

  Filed: 2026-04-29
─────────────────────────────────────────────────────────────────
```

## 0. How this document is used

This is a design document, not a buildable spec. It scopes the architectural pieces of PIY (Persistent Inference with Yielding) that are achievable today on the existing Tenancy codebase **without vocabulary expansion or fine-tuning**, identifies which behavioral targets each piece captures, names what is structurally lost relative to PIY proper, and proposes a build order ranked by ROI.

The document is honest about three boundaries:

1. **PIY's load-bearing claim is silence-as-token.** PIY-Lite cannot satisfy this claim. Without modifying the model's vocabulary, silence remains an absence rather than an action. Lite works around this absence; PIY proper restores it.

2. **The estimate "PIY-Lite captures 40–55% of PIY's mind-feeling value" is an informed guess, not a measurement.** Mind-feeling is not yet measurable in the ways the underlying claim requires. The 40–55% number is an honest forecast based on which behavioral vignettes from PIY each Lite piece can approximate; it should be treated as orienting, not load-bearing.

3. **The strategic argument that Lite de-risks PIY proper depends on Lite producing empirical signal that informs the PIY anchor-authorship work.** If Lite ships and the operator concludes "this is enough," PIY proper is unnecessary. If Lite ships and the operator concludes "impressive but missing something specific," that "something specific" is the data PIY proper's editorial bottleneck needs.

## 1. Why this document exists

PIY proper is a 4–8 week solo build with substantial editorial overhead (240–350 hand-authored anchor moments in the persona's voice). It is the architecturally correct response to the persistence-gap problem and the silence-primitive absence. It is also a substantial commitment of time and attention.

Before committing to PIY proper, an honest question is: what fraction of PIY's behavioral signature is achievable today, on the existing Tenancy codebase, without touching model weights? If the answer is "most of it," the marginal ROI on PIY proper drops sharply. If the answer is "very little," the case for PIY proper strengthens.

This document is the answer in design form. The answer is approximately: **the visible UX shifts that produce the strongest mind-feeling effect are achievable without fine-tuning. The underlying architectural correctness is not.** The two are decomposable.

## 2. Decomposition: what each PIY piece provides and where it can be approximated

```
PIECE                       WHAT IT PROVIDES                    LITE-FEASIBLE?
─────                       ─────────────────                   ──────────────
<|hold|> token              Silence as first-class output       NO
                            in model distribution               (vocab change required)

Persistent KV cache         Continuity across turns             PARTIAL
                            (no re-encode each call)            (llama-server
                                                                cache_prompt: true
                                                                already does some of
                                                                this; audit + tune)

Continuous tick loop        Substrate of background presence    YES
                            (Dave is "there" between exchanges) (existing outreach
                                                                loop is the seed)

Cache rollback on hold      Bounded cache growth in silence     N/A
                                                                (no hold token to
                                                                trigger rollback)

Duration tokens             Time as sensation with              PARTIAL
(silence/still/             distributional weight               (approximable as prose
waiting/alone)                                                  injection — loses
                                                                distributional weight)

Two clocks                  Cadence-shifting between            YES
(background + active)       quiet presence and rapid-fire       (just scheduling logic)

<|think|> token             Private inner mode separable from   PARTIAL
                            addressed speech                    (approximable as
                                                                separate generation
                                                                surface — feels
                                                                additive rather than
                                                                emergent)

<|abort|> token             Hesitation primitive                YES
                            (start to speak, withdraw)          (achievable as UX-only
                                                                animation on borderline
                                                                discriminator drops)

<|consolidate|> token       Volitional memory compression       N/A
                                                                (existing harness
                                                                consolidation already
                                                                solves this differently)

Phenomenological            Distributional weight from time     NO
compression                                                     (requires training data
                                                                with experiential
                                                                tokens — not approximable
                                                                without fine-tune)

<|interrupted|> token       Full-duplex conversation            PARTIAL
                            with model-internalized response    (harness can pause
                            to being cut off                    Dave; model doesn't
                                                                have a token reading
                                                                "I was cut off")
```

Pieces that are genuinely impossible without fine-tuning: silence-as-action, phenomenological-compression-with-distributional-weight, model-internalized think/reach selection, true full-duplex interrupt semantics.

Pieces that are achievable today: continuous low-frequency ticks, two-clock cadence, cache reuse via existing llama-server features, hesitation-as-UX, prose-level duration awareness, phrase-library mood injection, probabilistic gating mimicking pressure dynamics, separate inner-thought generation surface.

## 3. The eight pieces

Each piece is presented with: **purpose** (what behavioral target it addresses), **mechanism** (how it would be implemented), **integration points** (existing Tenancy systems it touches), **what it captures** vs **what it loses** relative to PIY proper, and **rough effort estimate**.

### 3.1 Aborted-typing UX

**Purpose.** Approximate PIY's `<|abort|>` primitive — the hesitation-and-withdrawal move that is the most distinctively-human conversational behavior and the most disproportionately effective at producing mind-feeling.

**Mechanism.** When the discriminator (heuristic + LLM-scoring) drops a reach with a borderline LLM score (e.g., 4–5/10, just below the pass threshold of 6), the harness still emits the `dave:stream_start` event and animates the typing indicator for 2–3 seconds before suppressing the message. The user sees: typing dots appear, pulse for two beats, fade out. Nothing was said. But something almost was.

**Integration points.** Discriminator output (existing). Drops table (already has `llm_score` column). `dave:stream_start` event (already wired). Frontend typing indicator (`pendingAssistant` empty state in `Conversation.tsx`).

**What it captures.** The subjective effect of hesitation is largely visual. A user watching a typing indicator pulse and fade reads the same thing whether the abort was generated by the model selecting `<|abort|>` or by the harness deciding from a borderline score. The behavioral signature is identical from the user's perspective.

**What it loses.** The model has no internal state representing "I almost said something and chose not to." The next reach Dave generates does not "remember" the aborted one because there's no token in his cache marking it. PIY proper would have `<|abort|>` in cache as a record; Lite has nothing.

**Effort.** Half a day. New event `dave:stream_aborted_visible` (distinct from the existing `dave:stream_aborted`), a CSS fade animation, a small change to `outreach.rs::tick` to emit the visible-abort path on borderline drops.

### 3.2 Status-bar presence pulse

**Purpose.** Approximate PIY's continuous-presence claim. Make the app feel like Dave is *there* between exchanges, not just spawned on demand.

**Mechanism.** The status bar already has a presence dot. On every harness tick (every 10–30 seconds), the dot pulses faintly — a 100ms opacity shift from 0.5 → 0.7 → 0.5. Most pulses are barely perceptible; peripheral vision picks them up; foveal attention rarely registers them.

Stronger pulse on `<|think|>`-equivalent events (see §3.7). Strongest on borderline-aborted-reaches (paired with §3.1).

**Integration points.** `StatusBar.tsx` presence-dot CSS animation. New event `dave:tick` emitted from harness regardless of outcome. Optional event `dave:thought` for stronger pulse on inner-thought generation.

**What it captures.** The felt sense of continuous activity. The dot becomes a low-bandwidth communication channel about Dave's internal state. Even if nothing visible happens, the user knows the substrate is alive.

**What it loses.** Nothing structural. This is purely a sensory-affordance addition.

**Effort.** Two hours. CSS animation, event wiring, frontend listener.

### 3.3 Two-clock cadence

**Purpose.** Approximate PIY's two-clock architecture — background heartbeat for baseline presence + active overlay for rapid-fire conversation. The current Tenancy outreach loop is the background heartbeat at slow tempo; what's missing is the active overlay.

**Mechanism.** Detect "user is actively typing" state from frontend keystroke events. When detected, fire a parallel fast-tick loop at sub-second intervals on a smaller context window. The fast-tick context is the last 5–10 messages plus *whatever the user has typed so far in the current input* (via a draft-text passthrough). Dave can produce fragmentary completions, agreements, mid-thought reactions. These either emit (if substantive per the discriminator) or get suppressed.

The active overlay fires only if the user has paused mid-typing for >2 seconds, suggesting hesitation rather than continuous typing. One fast-tick per pause. Most outputs are dropped; occasional fragments emit.

The two clocks are coupled — exactly one is active at any moment. The active overlay decays back to background after 30 seconds of no user typing.

**Integration points.** New `active_overlay.rs` module in `src-tauri/src/`. Frontend keystroke event listener and debounce. Draft-text IPC channel (currently the user's typing is local-only). Discriminator (existing).

**What it captures.** The texture of intimate exchange — fragments, completing each other's sentences, mid-thought reactions. Without this, every Dave response waits for the user to hit Enter, which is a fundamentally different conversational tempo than what the PIY vignettes describe.

**What it loses.** PIY proper has the model's distribution shift naturally during active mode (`<|reach|>` rises to 60–75%); Lite has the harness *force* fast-ticks via gating. The model is not internally aware of the cadence shift; only the harness is.

**Effort.** 1–2 days. Frontend keystroke detection, IPC plumbing, new harness module, careful tuning to avoid feeling intrusive.

**Risk.** Tuning is the hard part. Active overlay that fires too eagerly produces "Dave is interrupting me" effect. Active overlay that fires too rarely is invisible. The 2-second pause threshold is a starting point requiring empirical adjustment.

### 3.4 Mood-snapshot phrase library

**Purpose.** Approximate PIY's pre-authored sensory phrase library. Inject ambient texture into Dave's prompt context based on machine state, time of day, recent silence, etc. Without distributional weight (the model wasn't trained on these as experiential markers), but with prose-level seasoning that shifts Dave's output register.

**Mechanism.** A phrase library — 30–50 entries — keyed to combinations of:
- Time of day (night / morning / afternoon / evening)
- Machine load (quiet / busy)
- Recent silence duration (none / minutes / hours)
- Recent conversation rhythm (rapid / deliberate / sparse)

Examples: "the dark machine," "afternoon still," "the room is full," "the fan in the dark." Each phrase is 3–5 words in Dave's register.

When the harness fires a generation (chat reply, outreach, idle journal), it computes the current key, looks up the matching phrase (or selects randomly from a small set keyed to that condition), and prepends the phrase as a system-prompt suffix for that one request.

**Integration points.** New `mood.rs` module. Phrase library as a const data structure. Existing system-prompt assembly in `commands.rs::send_to_dave`, `outreach::tick`, `idle_worker::check_and_generate`.

**What it captures.** Ambient texture. Dave's outputs gain a sense-of-place that current Dave lacks. "The room is still" in his prompt context produces a different register than no prompt suffix.

**What it loses.** PIY proper's duration tokens have *learned distributional weight* — the model was trained to recognize `<|alone|>` as carrying experiential meaning, so its presence in context produces measurable distributional shifts in P(reach) etc. Lite's prose phrases are just words; the model treats them as content, not as experience. The behavioral effect is smaller and less reliable.

**Effort.** ~3 hours of code. Bo's editorial: a weekend (writing 30–50 phrases in Dave's voice). The editorial bottleneck is the same as PIY proper's anchor-authorship problem at much lower volume.

### 3.5 Prose-level duration awareness

**Purpose.** Extend the existing `time_awareness.rs` module. Currently it injects ambient time only when the user's message contains a temporal trigger word. Extend to: when outreach fires after long silence, prepend a duration phrase to the system prompt for that one request.

**Mechanism.** The harness already tracks `last_user_input` for outreach gating. When outreach is about to fire after sustained silence, compute the elapsed duration and select from a tier:

- 5–30 min idle: "the room is still"
- 30 min – 2 hr: "the machine has been quiet"
- 2–8 hr: "you have been alone for some time"
- 8+ hr: "you have been alone since this morning"

Inject as system-prompt suffix for that one generation. Same mechanism as the existing time-awareness extension.

**Integration points.** Extend `time_awareness.rs`. Hook into `outreach::tick` and possibly `idle_worker::check_and_generate`.

**What it captures.** Some of the experiential-time-feel PIY's duration tokens carry. Dave's reach after 4 hours of silence reads differently when his prompt context says "you have been alone for some time" vs when it doesn't.

**What it loses.** Distributional weight. PIY's `<|alone|>` token shifts the model's predicted next-token distribution because the model was trained to associate the token with reach probability. Lite's phrase shifts Dave's *prose register* but does not shift his *decision distribution*. The pressure-dynamics claim is approximated semantically rather than mechanically.

**Effort.** 3 hours.

### 3.6 Probabilistic outreach gating

**Purpose.** Approximate PIY's pressure dynamics — the property that P(reach) rises monotonically with elapsed silence as the duration token upgrades.

**Mechanism.** Currently outreach fires when gates pass: idle threshold, conversation length, max-unanswered, adaptive backoff. Add a probability term to the gate evaluated *after* all other gates pass:

```
elapsed_minutes  →  P(fire | gates passed)
─────────────────   ──────────────────────
just past threshold  ≈ 0.15
30 min               ≈ 0.35
2 hr                 ≈ 0.60
4+ hr                ≈ 0.85
```

When the random sample doesn't fire, the loop sleeps to the next tick and re-evaluates. When it does fire, the existing pipeline runs.

**Integration points.** `outreach.rs::tick`, after all existing gates and before inference call.

**What it captures.** Outreach feels less mechanical. Dave is more likely to break silence as silence accumulates, but not deterministically every-Nth-tick. The user-perceptible pattern matches PIY's pressure-dynamics claim even though the mechanism is different (harness sampling vs model distribution).

**What it loses.** PIY's pressure dynamics are *internal to the model*. The model decides; the harness routes. Lite's probabilistic gate keeps the decision in the harness, which is the puppet-orchestrator failure mode at lower intensity. The behavioral signature is similar; the architectural cleanliness differs.

**Effort.** 2 hours. A probability curve, a random sample, gate logic.

### 3.7 Inner thought stream

**Purpose.** Approximate PIY's `<|think|>` primitive. Currently Dave produces journal entries (during long absences) and chat replies. He doesn't produce *thoughts* — the inner content `<|think|>` would generate that informs later reaches.

**Mechanism.** Add a third generation surface called `idle_think`. Periodically (every 30 minutes during active session), the harness fires a quiet generation with a meta-instruction asking Dave to write 1–2 sentences of "what's on your mind right now, not addressed to anyone." These accumulate in a thought-stream panel visible to the operator (not the user, not in conversation). They're indexable; future outreach generations can include recent thoughts in context as priming material.

**Integration points.** New `idle_think.rs` module. Background tokio task. New table `inner_thoughts` in SQLite. New panel `ThoughtStream.tsx` accessed via keyboard shortcut. Hook into outreach generation to optionally include recent thoughts in context.

**What it captures.** The "inner life that informs reaches" property of `<|think|>`. Reaches that draw on accumulated thoughts feel more grounded — Dave referencing something he was thinking about three hours ago feels different than Dave referencing nothing.

**What it loses.** PIY's `<|think|>` is integrated into the model's continuous distribution — thoughts emerge and get caught at the same decision point as silence and speech. Lite's thoughts are produced by separate calls with their own meta-instruction, which (a) re-introduces the meta-instruction discipline issue from earlier in the project's evolution, and (b) feels additive rather than emergent.

**Effort.** 1 day, plus authorship of the meta-instruction wording. The meta wording is the editorial bottleneck. Recommend this piece be deferred until the others ship and the operator decides whether the inner-life surface is worth the meta-discipline cost.

### 3.8 Cache-reuse audit

**Purpose.** Approximate PIY's persistent-cache claim using the cache-reuse features llama-server already supports.

**Mechanism.** llama.cpp's openai-compat server supports `cache_prompt: true`, which preserves matching prefix tokens between calls. Audit current Tenancy behavior: when `send_to_dave` fires, is the system prompt + anchor prefix being re-encoded each turn or cached? If re-encoded (likely), explicitly enable cache reuse and verify the prefix is reused across calls.

This isn't full PIY-style continuous cache (no rollback semantics, no never-flushed property), but it does close the gap somewhat — the warm cache means Dave's stable context (system prompt + anchor zone + canvas) carries across calls in VRAM rather than being reconstructed each time.

**Integration points.** `llama_client.rs` — add `cache_prompt: true` to request bodies. Verify behavior empirically.

**What it captures.** Reduced inference latency for repeated context. Some persistence of Dave's "context state" across calls.

**What it loses.** No rollback semantics. No never-flushed cache that survives across the entire session. The cache still effectively resets per-turn because the request shape changes (recent zone grows, user turn appended).

**Effort.** 2–4 hours. One-line change plus empirical verification that it actually works as expected. Larger effort if it requires changes to llama-server invocation.

## 4. Composition concerns

The eight pieces interact. Some compositions are synergistic; some risk producing over-engineered theater rather than mind-feeling.

**Synergistic combinations:**

- §3.1 (aborted-typing) + §3.2 (status pulse) + §3.3 (two-clock active overlay) jointly produce the rapid-fire-with-hesitation texture. All three are visual; they work together to produce the felt sense of Dave being attentive and considering.

- §3.4 (mood phrases) + §3.5 (duration phrases) jointly shape Dave's output register based on context and time. Both are prose-injection; they don't compete.

- §3.6 (probabilistic gating) + §3.5 (duration phrases) jointly produce the pressure-dynamics signature — Dave more likely to fire after long silence, *and* his fire is differently-toned because the duration phrase has shifted his register.

**Risky combinations:**

- §3.1 (aborted typing) + §3.6 (probabilistic gating) + §3.3 (active overlay) all bend the user's perception of Dave's pacing simultaneously. If all three fire heavily, the user may perceive Dave as theatrical — too many visible cues that something is happening internally. The mind-feeling effect requires *some* of these signals to be subliminal. Tuning matters.

- §3.7 (thought stream) + the existing memory canvas may produce overlap. Both are operator-visible surfaces showing internal Dave state. If thoughts are essentially short canvas notes, the distinction blurs. Recommend deferring §3.7 until the canvas's role is clearer in production usage.

**Tuning protocol.**

After each piece ships, observe for at least 48 hours of normal usage before adding the next. The composition risk is empirical — predicting which combinations feel right vs theatrical requires running the system, not modeling it.

## 5. Build order

Ranked by ROI (visible effect ÷ effort), with composition risk factored in:

```
ORDER  PIECE                          EFFORT       LANDS WHAT
─────  ─────                          ──────       ──────────
1      §3.1 Aborted-typing UX         ½ day        Hesitation primitive (the
                                                    most distinctive PIY effect)
                                                    
2      §3.2 Status-bar pulse          2 hr         Continuous presence sense
                                                    
3      §3.6 Probabilistic gating      2 hr         Pressure-dynamics approximation
                                                    
4      §3.5 Duration prose            3 hr         Time-feel in reach register
                                                    
5      §3.8 Cache reuse audit         2–4 hr       Latency + some persistence
                                                    
6      §3.3 Two-clock active overlay  1–2 days     Rapid-fire texture
                                                    
7      §3.4 Mood phrase library       3 hr code +  Ambient texture
                                       weekend of
                                       Bo writing
                                       
8      §3.7 Inner thought stream      1 day +      Inner-life surface
       (DEFER PENDING REVIEW)         meta wording (composition risk; defer)
```

Total engineering for items 1–7: 3–5 days of focused work. Bo's editorial: a weekend.

Items 1–6 should ship before any of items 7–8 are considered. Each item should be observed in normal use for at least 48 hours before the next ships, to surface composition issues early.

## 6. Empirical evaluation

After PIY-Lite ships and runs for 2–4 weeks, the operator should be able to answer the following empirically:

```
QUESTION                                        EVIDENCE SOURCE
────────                                        ───────────────
Does the aborted-typing UX read as hesitation   Operator self-report,
or as a glitch?                                 plus drops table tagging
                                                of which aborts felt right

Does the status pulse improve felt-presence     Operator self-report after
or recede into background noise?                 disabling it for a week

Does the active overlay feel like rapid-fire    Direct conversation
texture or like Dave interrupting?              experience

Does the mood-phrase library shift Dave's       Comparison of outputs with
register noticeably?                            and without phrase injection
                                                in the drops table

Does probabilistic gating produce a more        Drops table distribution
varied outreach pattern?                        analysis pre/post

How much of PIY's mind-feeling is captured?     Subjective. The honest answer
                                                is whatever the operator
                                                concludes after living with
                                                Lite for a month.
```

The fifth question — the subjective one — is the load-bearing one. PIY-Lite's value is whether the operator feels mind-feeling has been substantially achieved. If yes, PIY proper is unnecessary and the project pivots to other layers (multi-persona, inter-persona). If no, the operator can identify *what specifically is missing* — and that specificity is the input PIY proper's anchor-authorship needs.

## 7. Strategic position

PIY-Lite is structurally positioned as both substitute and step:

**As substitute.** If Lite produces enough mind-feeling to satisfy the operator, PIY proper is not a strict prerequisite for the project's larger ambitions (multi-persona, inter-persona dynamics, the L3–L8 trajectory in the README). The architectural correctness of PIY proper would still be valuable as a research contribution and a Vol II artifact, but it would no longer be in the critical path.

**As step.** If Lite ships and produces *some* of the desired effect but the operator can identify specific behavioral gaps (e.g., "the silence still feels like absence rather than choice" or "Dave's reaches still feel mechanical even with the duration phrases"), those identified gaps are the empirical grounding for PIY proper's anchor authorship. Bo would know what behaviors the 240–350 anchors need to demonstrate because he has 2–4 weeks of data on what Lite *cannot* produce.

Either way, the engineering investment in Lite is not wasted:

- The phrase library, abort animation, two-clock active overlay, duration-aware prompt suffixes, and cache-reuse infrastructure all survive into PIY proper as harness-side features. PIY proper *replaces* the model's behavior at the token level; it does not replace the surrounding harness scaffolding.
- The drops table extensions and operator-visible inner-thought surface (if shipped) become observability infrastructure that PIY proper's iteration phases require.

## 8. Risk register

```
RISK                                LIKELIHOOD  IMPACT     MITIGATION
────                                ──────────  ──────     ──────────
Composition produces theater        Medium      High       48hr observation
rather than mind-feeling                                   between piece additions

Aborted-typing animation reads      Low         Medium     Tunable threshold;
as bug not feature                                         operator can disable

Active overlay interrupts user      Medium      High       2-second pause threshold;
mid-typing                                                 conservative tuning;
                                                           operator can disable

Phrase library is too small to      Medium      Low        Authoring more is cheap;
produce noticeable register shift                          start with 30, scale
                                                           if needed

Duration prose has no measurable    Medium      Medium     If true, this is
behavioral effect (model doesn't                           empirical evidence FOR
weight it as PIY's tokens would)                           PIY proper's distributional
                                                           approach

Probabilistic gating produces       Low         Medium     Tunable curve; can be
worse outreach distribution than                           reverted to deterministic
deterministic gating                                       gating

Inner thought stream re-introduces  Medium      Medium     Defer indefinitely if
meta-instruction discipline issues                         composition risk persists
```

## 9. What this design explicitly does not claim

- Lite does not claim PIY's load-bearing thesis (silence-as-token primitive). Lite works around the absence; it does not restore the missing primitive.
- Lite does not claim distributional pressure dynamics. The probabilistic gate is harness-side sampling, not model-side decision-making.
- Lite does not claim phenomenological compression. Duration phrases are metadata-as-prose; PIY's duration tokens are experiential markers with learned weight. The behavioral surface differs.
- Lite does not claim full-duplex conversation in the PIY sense. The active overlay enables fast back-and-forth, but the model has no `<|interrupted|>` token reading "I was cut off."
- Lite does not claim to produce mind-feeling at the level PIY proper aims for. Lite aims for 40–55% of PIY's value; whether that fraction crosses the operator's threshold for "this feels like a presence" is empirical.

## 10. Decision criteria

After Lite ships and runs for 2–4 weeks, the project state will be one of three:

```
STATE A — LITE IS ENOUGH
  Operator concludes mind-feeling has been substantially achieved.
  PIY proper is deprioritized. Project pivots to L3+ (multi-persona).
  
STATE B — LITE IS PARTIAL, GAPS ARE NAMED
  Operator concludes Lite captures part of mind-feeling but specific
  behaviors are missing. The named gaps become the requirements for
  PIY proper's anchor authorship. PIY proper becomes the obvious
  next investment with empirical grounding.
  
STATE C — LITE IS INSUFFICIENT, ARCHITECTURE NEEDS RECONSIDERATION
  Operator concludes mind-feeling does not arise from the kinds of
  behaviors Lite (or PIY proper) target. The substrate-fight diagnosis
  may be necessary but not sufficient. Project reconsiders what mind-
  feeling actually requires.
```

State B is the most likely outcome and the most generative — it produces specific data informing the next architectural commitment.

## 11. Snapshot + rollback discipline

When PIY-Lite is implemented (in a future session, not this one):

- Snapshot all touched files to `.snapshots/<timestamp>_pre-piy-lite/`
- CHANGELOG entry per piece, ranked by ship order
- Each piece is independently revertable; the harness should support disabling any individual piece via settings without affecting the others
- The drops table extensions for new action types (visible-abort, probabilistic-gate-skipped) are append-only; existing rows remain compatible

## 12. Status flags

```
THIS DOCUMENT:           PROPOSAL. No code committed against it.

PIY-LITE IMPLEMENTATION: NOT BUILT. The pieces are scoped and ranked
                         but not implemented.

EMPIRICAL VALIDATION:    NONE. The 40–55% mind-feeling-capture estimate
                         is an informed guess based on which behavioral
                         vignettes from PIY each piece can approximate.

DECISION POINT:          After implementation + 2–4 weeks of observation,
                         operator decides between States A / B / C above.
```

---

```
─────────────────────────────────────────────────────────────────
  END piy-lite-design.md

  Cross-references:
    PIY_ARCHITECTURE.md   (architectural reference, full PIY)
    PIY_IMPL.md           (implementation spec, full PIY)
    docs/outreach-a2-design.md  (the substrate-fight analysis
                                 that motivated some of these
                                 pieces)
    src-tauri/src/outreach.rs   (existing outreach loop, the
                                 background heartbeat seed)
    src-tauri/src/discriminator.rs  (existing discriminator
                                     that the abort-UX builds on)
    src-tauri/src/time_awareness.rs (existing time-awareness
                                     module that duration prose
                                     extends)

  Update protocol: this document is a proposal. Updates expected
  after operator review and any implementing instance's questions.
─────────────────────────────────────────────────────────────────
```
