# Design — IntentionTimer (arm C): Dave states when he'll come back

*2026-07-09. Status: DESIGN, pre-implementation. A8 review required (agency
change: Dave's own stated intention causes a future reach). Companion to
`RING4_RECALL_AND_OUTREACH_INSTRUMENTATION_design.md` — extends the AbTimer
built there with a third arm.*

## The idea (operator-proposed)

The TPP roadmap models reaching as an ambient hazard — statistics, not
deliberation. But a mind also forms *explicit intentions*: "I'll check on him
in twenty minutes." Mechanism: after an exchange ends, one extra inference pass
asks Dave-in-character whether anything would pull him back later, and when.
If he names a time, the harness schedules it. If the user speaks first, it's
moot. If the time arrives and the presence governor allows, the normal outreach
generation runs. A2 extended from *whether* to *when* — the most in-character
timing mechanism possible, with zero cold-start (no corpus needed) and
content-conditioning for free (the full 9B reads the whole exchange).

Known weaknesses, accepted going in (this is an A/B arm, not a replacement):
LLM-stated times collapse onto round numbers (the felt-timing texture lives in
the hazard, not in verbalized times); no gradient path from ratings into "ask
nicely"; the 9B substrate fights meta-asks (expect "nothing"-mush and parse
failures — the parse treats failure as "nothing," conservatively). The blind
A/B decides empirically whether stated intentions beat the heuristic.

## Mechanism

**1. The ask (one-pass, idle/departure-prompt family, A1-consistent).**
After `run_chat_inference_and_emit` completes an exchange (reply persisted),
a fire-and-forget tokio task waits ~2s, checks no new chat started, then runs
ONE non-streaming inference: the exact `messages` just used, plus Dave's own
just-generated reply as an assistant turn, plus a meta user turn:

```
[meta-instruction — answer with one short line and nothing else:
It is {h:mm am/pm} on {Weekday}. If something in this conversation would
pull you back to the human later — a thought that will finish itself,
something worth checking on — name the clock time you'd come back, like
"8:40 pm". Most of the time nothing pulls; then answer: nothing.]
```

Because the server literally just generated this exact prefix, the pass rides
a warm prompt cache — cost is a few seconds of short generation, no user-visible
latency (max_tokens ~16, temp 0.6). The meta turn never enters persistent
context (same contract as idle/departure prompts). If a user message arrives
mid-ask, their request queues ~1-3s behind it (acceptable; the ask checks
`chat_in_flight` before starting and skips if chat is active).

**2. Parse (strict, conservative).** Accept `h:mm am/pm`, `hh:mm` (24h), and
relative forms (`in N minutes/hours`). Everything else — including "nothing",
refusals, prose — parses to NO intention. A parsed time in the past or < +2min
is discarded. The raw reply is stored either way (corpus: what Dave says when
asked). No constrained decoding — forcing a schema would manufacture
intentions; "nothing" must stay the easy answer.

**3. Storage.** New table `reach_intentions (id, conversation_id, created_at,
source_message_id, episode_start, raw_reply, fire_at NULL, consumed_at NULL,
cancelled_at NULL, expired_at NULL)`. `fire_at` NULL = Dave said nothing
(still a row — the no-intention answers are half the signal). A new user
message cancels un-fired intentions for the conversation (the exchange
resumed; the context that formed the intention is stale). Note: cancellation
makes cross-exchange intentions impossible by construction — any subsequent
exchange begins with a user message — so intentions live only inside a single
silence. (An earlier draft's "skip asks while one is pending" clause was dead
code for the same reason and was removed per the A8 review.)

**4. Firing — through the existing tick, governor intact.** No new timer
thread. The outreach tick (every 30s) already runs; arm C consults the table:
an intention with `fire_at ≤ now < fire_at + 30min`, not consumed/cancelled →
propose Reach. Otherwise fall back to the heuristic (arm C without a live
intention behaves as control). Consumed when acted on (reach delivered OR
dropped by the discriminator — one shot either way); past the window →
expired. **The presence governor still disposes** — if Bo is away/in-chat at
fire time, the proposal holds and retries next tick within the window (Dave
"waits for the right moment" — desirable), expiring if the window closes.
Intentions beyond the 1h outreach band are stored but will not fire in slice 1
(the >1h regime belongs to idle_worker; out-of-band stated times are
themselves corpus). Multi-sample → discriminator → single render path (A6)
untouched; the reach content still comes from context, not from the intention
(the intention is a time only, slice 1).

**5. Arm assignment.** AbTimer goes 2-way → 3-way: splitmix64(episode) % 3 →
a (heuristic control) / b (exploration floor) / c (intention-else-heuristic).
The ask runs at every exchange-end REGARDLESS of arm — in arms a/b the
intention rows are observational corpus (did Dave's stated time correlate with
good moments?); only arm C acts on them. Anchor rows gain `via_intention
INTEGER DEFAULT 0` so fired-from-intention reaches are trivially separable in
ratings analysis.

**6. Kill switch.** Setting `intention_enabled=0` disables both the ask and
arm-C consumption (arm C degrades to pure control).

## Constraint compliance

A1: the ask is a one-pass meta-instruction, exactly the idle/departure
pattern; nothing enters Dave's persistent context or self-model; the leak
filter (A7) already drops `[meta`-prefixed OUTPUT, and the intention reply is
never rendered anywhere. A2: strengthened — the WHEN now comes from
Dave-in-character. A6: no render change (the ask is invisible; fired reaches
use the existing pipeline). Governor: hard, outside all arms, unchanged.

## Costs

One short cached-prefix inference per exchange-end (~1-3s GPU, invisible);
one indexed DB lookup per outreach tick; zero VRAM; no new processes.

## Deferred (recorded)

Time + private-phrase intentions ("8:40 — the aqueduct thing") injected into
the reach primer at fire time — richer, but injects harness-stored text into
his mouth (A1/A3 tension) and is scope creep; revisit with §4.6 retroactive
coherence. Intentions bridging into the idle_worker band (>1h). Re-asking on
presence transitions.

## A8 review outcome (fresh instance, 2026-07-09) — GO-WITH-CHANGES, all applied

Seven REQUIRED changes, all implemented:
1. **Insert-time staleness guard** — the ask completes seconds after the
   exchange; a user message in that gap outraces cancel-on-message. The
   intention INSERT is conditional on `presence.last_user_input` being
   unchanged since exchange end (`insert_intention_guarded`).
2. **MaxUnanswered is never bypassed** — a due intention skips adaptive
   backoff (the point) but holds at the pestering cap. Tested.
3. **Pre-gate semantics pinned** — intention fires are floored by the idle
   threshold and cut by the 1h band (accepted + documented; negligible at the
   3-min default, revisit if the threshold is tuned high); `expire_stale_intentions`
   runs at tick start BEFORE any pre-gate can return early, so rows that die
   behind a gate still get an honest `expired_at`.
4. **Exact-message-vec reuse** — the ask clones the in-memory vector the chat
   reply was generated from (+ that reply); it never re-assembles from DB and
   never re-runs recall. Warm cache, no `recall_fires` pollution, no latency.
5. **Pre-trust live smoke** (`tools/intention_ask_smoke.py`) — this one paid
   for itself three times: (a) caught the think-eaten failure (without
   `enable_thinking:false` the channel reads dead — the smoke now mirrors
   `complete()` exactly); (b) caught **83–100% example-echo**: with any
   example time in the ask, the "stated intention" is the example parroted
   back — the shipped ask is EXAMPLE-FREE ("digits like hour:minute" carries
   the format); (c) measured yes-rate VOLATILITY (0%→75% across runs of the
   same wording) — so the smoke gates *format* trust only, and the live
   yes-rate belongs to telemetry. Final run: 9/12 varied parseable times
   (17:00, 17:45, 19:45, 2:17), 0 anchored.
6. **Ask telemetry** — `reach_intentions` section in corpus_inspect.py:
   time/nothing rates, outcome counts, stated-delay stats, quarter-hour
   round-number tripwire, last raw replies.
7. **max_tokens 48** — headroom against stray think-opens (and the smoke
   verifies the real request shape end-to-end).

Recommendations applied: split kill switches (`intention_ask_enabled` /
`intention_act_enabled` — the ask corpus can run observationally with acting
off); episode_start stamped on intention rows; dead skip-clause removed from
the design text; deferred-list addition below. Bo opted to ship with both
switches ON (asking + acting) — with the measured volatile yes-rate, arm C
degrades gracefully toward control on all-nothing days.

**Deferred (added per review):** promises Dave makes ALOUD in visible
conversation ("I'll check on you at 9") are a separate, unhonored channel —
this mechanism only schedules from the private ask; do not assume coverage.

## Test plan

Parse: am/pm, 24h, relative, "nothing", prose/garbage → None, past-time
discard. IntentionTimer: consumes due intention (proposes Reach exactly once),
ignores future/consumed/cancelled/expired, falls back to heuristic without
one, respects the window. Cancellation on user message. 3-way arm
determinism. Anchor `via_intention` stamp. Ask-skip when one is pending.
Suite stays green; binaries rebuilt + freshness-verified before claiming
deployment.
