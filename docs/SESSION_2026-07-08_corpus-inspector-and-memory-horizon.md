# Session write-up — 2026-07-08

## Corpus-accumulation unblock + memory-horizon tuning

A human-readable record of everything done in this session, for future-me and
for the record. Two workstreams on the PIY initiation-timing track and its
adjacent memory work. Everything below is committed to `main`; verify against
`git log` and `cargo test` before trusting the narrative.

**Commits produced this session**
- `8aecdf7` — Initiation-timing: corpus inspector + rebuild release binary that lacked sensors
- `b172431` — Memory-horizon: token-budget the recent zone (A8-reviewed)

**Test state at end:** `cargo test` → **86/86** (was 81; +5 memory-horizon tests).
**Binaries:** release + portable `dave.exe` rebuilt fresh and verified; both contain
this session's changes.

---

## 0. How the session started

Resumed from `docs/CONTINUATION_2026-07-08.md`. Ran its START-HERE verification:

```
git log --oneline -14   # clean, commits match (Stage 0/1a/1b, headless, persona hardening)
git status --short       # clean
cargo test               # 81/81
```

All trustworthy. The continuation doc said the initiation-timing track was in a
"wait for the corpus to accumulate from real daily use" phase, with nothing to
code on the timer until data exists. That framing turned out to be *correct in
principle but broken in practice* — see Part 1.

---

## 1. The corpus couldn't accumulate — the shipped binary lacked the sensors

### What I found

The initiation-timing track (learned model that lets Dave reach out on his own,
timed by context) depends on four corpus tables filling from real use:
`presence_samples`, `initiation_anchors`, `reach_ratings`,
`reach_counterfactuals`. Presence history **cannot be reconstructed after the
fact**, which is why the sensors shipped first (Stage 0).

I checked whether the corpus *can* fill, and disk contradicted the handoff
narrative:

- **Debug DB** (`C:\DAVE\dave.db`): empty; the corpus tables don't even exist —
  last touched by a pre-Stage-0 binary.
- **Release DB** (`%LOCALAPPDATA%\com.bochen.dave\dave.db`, the real 233-message
  history): corpus tables exist but are **all empty**; the conversation history
  and all 46 outreach drops are frozen at **2026-04-27 → 04-30** (2+ months
  stale, all drops from the old "empty Dave" era).
- **The decisive finding:** the release/portable `dave.exe` Bo double-clicks was
  **built at 10:51**, an hour *before* Stage 0 landed at 11:50. It contained
  **none** of the presence sensor, anchor logging, hard-gate, or timing seam.

So "just wait for the corpus to fill from daily use" was a no-op — the sensing
code wasn't in the shipped binary. The empty corpus tables in the release DB
were created by the headless/debug binary touching that file, not by live
sensing. This is the same irreplaceable-data catastrophe the roadmap warns
about, caused by a stale binary rather than a code bug.

### What I verified first (to rule out a silent write bug)

Read the two corpus inserts (`insert_presence_sample`, `insert_initiation_anchor`
in `persistence.rs`) against the schema — **column-for-column correct**. So the
empty corpus was purely a deployment gap, not a silent SQL failure.

### The fix

Rebuilt `target/release/dave.exe` from clean HEAD (matches committed source; no
new uncommitted code) and refreshed the SHA-256-identical portable copy at
`dist-portable/Dave/dave.exe`. Verified three ways per the
"verify-the-compiled-binary" discipline:

1. mtime: rebuilt binary newer than the newest Stage-0/1 source file.
2. **Sensor string literals embedded in the binary** — `present_elsewhere`,
   `hold_presence_gate`, `initiation_anchors`, `timer_decision`,
   `presence_samples` all present (the stale 10:51 binary had none).
3. Portable copy byte-identical to the release exe.

Also confirmed the runtime wiring in `main.rs`: the `WindowEvent::Focused`
handler updates `window_focused` (`main.rs:92`), initial focus is seeded from the
real window state (`main.rs:182`), and `presence::spawn_sampler(...)` is actually
spawned (`main.rs:221`).

### Result

The accumulation pipeline is now actually deployed in the app Bo runs. The track
is *genuinely* in the "wait for real daily use" phase now — nothing to code on
the timer until the corpus reaches the readiness floor.

---

## 2. New tool — the corpus inspector (the measuring stick)

`tools/corpus_inspect.py` — read-only, stdlib-only, self-testing. The instrument
the roadmap (§3d) calls for so the accumulation phase is observable.

**What it reports:**
- Row counts + date ranges for the four corpus tables + `outreach_drops` + `messages`.
- `initiation_anchors`: governed-decision vs timer-proposal breakdown, the
  **presence-gate-override count** (timer wanted to reach, governor said no),
  presence-state distribution at armed ticks.
- `presence_samples`: state distribution; and a clear "if this is 0, the sensor
  isn't logging — here's how to confirm it's live" message.
- **Censored-negative reconstruction** (the actual V0 training set): reconstructs
  arming episodes → reach **EVENTS** vs "user spoke first" **CENSORED**
  observations, with a **READINESS verdict** vs a floor (150 episodes / 20 events
  / 40 censored). This doubles as the data-loader front-end for the eventual
  `fit_v0.py`.
- A schema-drift guard that fails loud (exit 2) rather than reading wrong columns.

**Usage:**
```
python tools/corpus_inspect.py            # release DB (%LOCALAPPDATA%)
python tools/corpus_inspect.py --db PATH  # a specific dave.db
python tools/corpus_inspect.py --debug-db # the debug DB at C:\DAVE\dave.db
python tools/corpus_inspect.py --episodes # also dump per-episode rows
python tools/corpus_inspect.py --selftest # synthetic-fixture self-test
```

**How to use it going forward:** run Dave normally; then run the inspector to
watch `presence_samples` / `initiation_anchors` fill. To confirm the sensor is
live end-to-end: unfocus Dave's window and move the mouse in another app for
~30s, then re-run — a `present_elsewhere` presence row should appear.

---

## 3. Memory-horizon tuning (§7 / §3d)

### The diagnosis (offline replay of the real 233-msg history)

The assembled context is ~54k tokens/turn. The breakdown reframes the problem —
it is **not** "consolidation isn't aggressive":

| Zone   | Content                    | Tokens | Share |
|--------|----------------------------|--------|-------|
| anchor | 30 msgs verbatim           | 3,100  | 6%    |
| middle | 0 bare + **6 epochs**      | 6,364  | 12%   |
| recent | **100 msgs verbatim**      | **43,955** | **82%** |

Consolidation is excellent — 103 middle messages compressed ~7× into 6 epochs.
The bloat is entirely the **recent zone**: `RECENT_MESSAGE_TARGET`=100 messages
held verbatim, which the consolidator **never** compresses (it only touches
messages older than `total − 100`, `consolidation.rs:117`). And there was **no
token-budget enforcement anywhere** — `TOKEN_BUDGET_TOTAL` was a display-only
number. So context grows unbounded with the conversation until it overflows the
65536 ctx (silent truncation), and every turn re-evaluates a near-full context.

### The fix (`memory_assembler.rs`)

Added `CONTEXT_SEND_BUDGET_TOKENS` (default **48_000**) and a `recent_keep_start()`
helper. `build_chat_messages` now token-budgets the recent zone: it keeps the
newest recent messages that fit, trimming the **oldest** recent first.

- **Always protected:** anchor, canvas, and consolidated epochs (Dave's durable
  memory) are never trimmed.
- **Floor:** at least `MIN_RECENT_MESSAGES` = 12 newest always survive, for
  immediate conversational coherence.
- Trimmed messages stay in the DB (source of truth) and fold into an epoch as the
  conversation advances — so nothing is lost, it just passes out of *verbatim*
  reach. That is the §7 "aging mind" behavior.
- `partition()` is unchanged, so consolidation semantics are untouched.

### The A8 fresh-instance review

Because this changes what Dave holds in context (a memory / persona-attractor
change), amendment A8 requires a fresh-context review. A clean-context reviewer
returned **GO-WITH-CHANGES** and caught three things worth fixing — all applied:

1. **The default was too aggressive.** My initial 40_000 over-optimized eval
   speed against mind-feeling, the metric §1/§14 say wins → raised to **48_000**.
   40k stays documented as the low end of the knob.
2. **Role-seam risk.** Trimming could make the recent zone open on an `assistant`
   turn, stacking with the assistant-injected epochs/canvas (which nudges the
   model to continue its own text instead of answering). Added a **seam guard**
   that drops a single leading assistant so the zone opens on a user turn — but
   **only** when the preceding emitted turn is genuinely assistant (not when
   recent legitimately follows a user turn).
3. **§7 fade desync.** Documented rather than shipped silent — see Open Items.

### The knob (this is the one mind-feeling decision — it's Bo's)

`CONTEXT_SEND_BUDGET_TOKENS` is a documented tunable (per §14 — tune by feel, not
metric). Effect on the real 233-msg history:

| Budget | Recent kept | Context   | Speedup | Notes                                  |
|--------|-------------|-----------|---------|----------------------------------------|
| **48k** (set) | 79 / 100 | 54k → 48k | ~11%    | A8-safe default; caps growth; generous |
| 40k    | 62 / 100    | → 39k     | ~28%    | real immediate speedup                 |
| 32k    | 42 / 100    | → 32k     | ~42%    | aggressive; ~21 exchanges verbatim     |

The 48k default barely changes current behavior (drops ~21 oldest-recent) but
hard-caps future growth so context can never overflow. To change it: edit the one
constant in `memory_assembler.rs`, then `cargo build --release` (see below).

### Tests

`cargo test` → **86/86** (+5): budget trims oldest / keeps newest, floor respected,
over-budget assembly, seam guard fires, seam guard correctly does NOT fire when
recent follows a user turn.

---

## 4. Open items / follow-ups

- **§7 opacity fade over-reports memory.** `buffer_size()` returns a static 100
  and `src/lib/memory.ts` fades everything older than `totalLen − 100`, but the
  backend now sends only ~79 recent — so ~21 messages render as "remembered"
  while being out of Dave's verbatim reach. (The fade was already impressionistic:
  it also shows the always-kept anchor as faded and ignores epoch substitution.)
  Fix: have the backend report the real recent-keep count so the fade tracks what
  is actually sent. **Tracked as a spawned task chip** ("Make §7 opacity fade
  track real recent-keep count").
- **Overlapping active consolidation epochs.** The 6 "active" epochs in the real
  DB overlap (`[31..123]`, `[49..123]`, `[58..125]` cover the same span) — the
  consolidator's non-overlap invariant appears violated, leaving redundant
  summaries (~5k of the middle's 6.4k is triple-covered re-summaries of msgs
  124–133). Not touched this session (separate, delicate, A8-heavy consolidation
  concern). Worth its own investigation.
- **The corpus still needs real daily use.** Nothing to code on the learned timer
  until `corpus_inspect.py` shows the readiness floor. Next coded milestone when
  data exists: build the blind-A/B harness FIRST (the falsifier), then the V0
  log-normal hazard behind `TimingModel`.
- **NSIS installer is stale.** `dist-portable/Dave_0.1.0_x64-setup.exe` is still an
  old build. If Bo ever runs an *installed* copy rather than the portable exe,
  repackage with `pnpm tauri build`. The portable exe is fresh.

---

## 5. Rebuild / verify recipe (for when the constant is tuned)

The memory-horizon change lives on the live chat path, so a source edit needs a
release rebuild to take effect in the app:

```powershell
cargo build --release --manifest-path C:\DAVE\src-tauri\Cargo.toml
Copy-Item C:\DAVE\src-tauri\target\release\dave.exe C:\DAVE\dist-portable\Dave\dave.exe -Force
```

Then verify the binary is actually fresh (mtime newer than the source you
changed) before trusting it — the whole reason Part 1 happened.

---

*Written by the session that did the work. Trust `git log` + `cargo test` over
this narrative; it reflects state as of 2026-07-08.*
