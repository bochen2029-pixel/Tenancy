# Changelog

Append-only log of meaningful changes since v1 scaffolding. Snapshots of
overwritten files are saved in `.snapshots/<timestamp>_<label>/` for rollback
when git isn't being used.

To roll back a change: `cp -r .snapshots/<timestamp>_<label>/* ./` then
`cargo check` / `pnpm build` to confirm.

---

## 2026-07-09 — Ring 4 recall (perpetual memory, slice 1) + outreach A/B instrumentation

Two linked slices toward the PIY wow factor, designed against the REEL protocol
(`C:\OUTREACH\BC_Canon\MAY2026\Deeper\REEL_PROTOCOL_v1_0.md`, read in full) and
A8-reviewed as a design BEFORE implementation (design doc:
`docs/RING4_RECALL_AND_OUTREACH_INSTRUMENTATION_design.md`, verdict
GO-WITH-CHANGES, all 5 required changes applied — see the doc's appendix).

**Pillar 1 — Ring 4 recall (`recall.rs`, new).** Dave can now remember
verbatim things that fell out of his context window. REEL mapping: Dave was
already a Tier-2 REEL implementation (Ring 0 ≈ persona+anchor, Ring 2 ≈
budgeted recent, Ring 3 ≈ Dave-voiced epochs, Tape ≈ messages+journal); Ring 4
(retrieval) was the missing organ. Implementation:
- **FTS5 index `memory_fts`** over the Tape (messages + journal + epoch text),
  porter-stemmed, synced on every insert/supersede/edit, backfilled once onto
  existing DBs. FTS5-in-bundled-rusqlite verified by a loud test.
- **Hard-gated firing (the A8 review's core catch):** recall fires ONLY on a
  remember-cue ("remember", "what was that…") or a rare content term
  (document-frequency gate), with candidates needing ≥2 term matches unless
  rare/cued. An every-turn block would bust llama-server's prefix cache at the
  canvas position and re-eval ~40k tokens/turn — latency is presence.
- **Exact eligibility:** only Tape text actually OUT of context (epoch-covered
  middle ∪ budget-trimmed recent ∪ journal). The recall budget (600 tok) is
  reserved UNCONDITIONALLY in the recent-zone trim so the trim point never
  jitters (stable prefix, warm cache) and eligibility is computable up front.
- **Injection:** ≤3 excerpts, quoted verbatim (never paraphrased — REEL §10.5
  false-memory guard), merged INTO the canvas assistant turn (no stacked
  assistant turns), opening `from further back, before it goes hazy:`.
- **A7 backstop:** leak.rs now drops any visible reply containing the frame
  line; chat pre-emission, outreach discriminator and consolidation all route
  through it. **Echo smoke-eval run live against K0D**
  (`tools/recall_echo_smoke.py`): CLEAN 4/4 — no frame echo, no format echo,
  and the model continues recalled threads as natural memory.
- Wired into all three inference paths (chat, outreach — the first
  content-conditioned brick: he can reach *about* the thing — and headless).
  Observability: `recall_fires` table + inspector section. Kill switch:
  settings key `recall_enabled=0`.

**Pillar 2 — outreach blind-A/B + guardrails (corpus still empty; this is the
instrument, not the model).**
- **`AbTimer`:** two timing arms, one deterministic per-EPISODE coin
  (splitmix64 of episode identity — no within-episode interleaving). Arm A =
  `HeuristicTimer` control; arm B = heuristic + exploration floor. Every
  anchor row logs `ab_arm` + `explored`; a delivered reach stamps its
  `reach_message_id` onto the anchor for EXACT ratings→arm joins.
- **`ExploringTimer` (ε-floor, §3c):** at most ONE explored reach per episode,
  at a deterministic scheduled instant uniform over the armed band (the A8
  review killed per-tick ε: it made backoff punch-through scale with backoff
  length and allowed reach-chains). Never overrides the MaxUnanswered
  pestering guard; the presence governor still disposes.
- **Bless-the-silence (Ctrl+Alt+S):** the missing symmetric down-channel —
  `reach_counterfactuals.kind = 'good_silence'` vs `'missed_reach'`. Skipped
  while typing (composer focus = the silence is ending; AltGr guard).
- **CV tripwire in `corpus_inspect.py`:** inter-reach-interval CV (σ/μ) with
  the reject-below-0.6 rule, plus arm/explored/ratings-by-arm/recall-fires
  readouts. Inspector degrades gracefully on pre-upgrade DBs.

Schema migrations (auto, on first run): `memory_fts`, `recall_fires`,
`initiation_anchors.{ab_arm, explored, reach_message_id}`,
`reach_counterfactuals.kind`.

cargo test **105/105** (was 86; +19: FTS availability/hooks/backfill, recall
gate/format/eligibility, reserve-stability, canvas-merge, arm/explored stamps,
silence kinds, frame-echo leak). Deferred, recorded in the design doc: Op 6
self-assessment, smooth Poincaré compression levels, cold-start thresholds,
semantic/reranker upgrade behind the same seam, KEEL adapter (when KEEL pins a
release — its Ring-4 recall landed but is mid-autonomous-build, default-off).

---

## 2026-07-08 — Memory-horizon tuning: token-budget the recent zone (§7/§3d)

Diagnosis (offline replay of the real 233-msg history): the assembled context is
~54k tokens/turn, and the breakdown is NOT "consolidation isn't aggressive" —
consolidation is excellent (103 middle messages compressed ~7× into 6 epochs =
6.4k). The bloat is the **recent zone: `RECENT_MESSAGE_TARGET`=100 messages held
verbatim = ~44k = 82% of context**, which the consolidator never compresses (it
only touches messages older than `total - 100`). And there was **no token-budget
enforcement anywhere** — `TOKEN_BUDGET_TOTAL` was a display-only number, so
context grows unbounded until it overflows the 65536 ctx (silent truncation) and
long conversations crawl on prompt-eval.

Fix (memory_assembler.rs): added `CONTEXT_SEND_BUDGET_TOKENS` (default 48_000) +
`recent_keep_start()`. `build_chat_messages` now trims the OLDEST recent messages
to keep assembled context within budget; anchor, canvas and epochs are always
kept (Dave's durable memory), and at least `MIN_RECENT_MESSAGES`=12 newest always
survive. Trimmed messages stay in the DB and fold into an epoch as the
conversation advances (§7 "aging mind"). `partition()` is unchanged, so
consolidation semantics are untouched. On the real DB, 48_000 keeps 79/100 recent
(~54k→48k, 11%); it's a documented MIND-FEELING KNOB (§14) — 40_000 keeps 62
(28%), 32_000 keeps 42 (42%) for more speed at the cost of recent verbatim.

**A8 fresh-instance review: GO-WITH-CHANGES, all applied.** (1) Raised the default
from an initial aggressive 40_000 to 48_000 — a low default over-optimizes eval
speed against mind-feeling, the metric §1/§14 says wins. (2) Added a **seam
guard**: trimming could make the recent zone open on an assistant turn, stacking
with the assistant-injected epochs/canvas (nudging the model to continue its own
text); the guard drops a single leading assistant so recent opens on a user turn
— but ONLY when the preceding emitted turn is genuinely assistant (not when recent
legitimately follows a user turn). (3) See the fade note below.

**KNOWN, UNADDRESSED (§14 writeup, not silent): the §7 opacity fade now
over-reports memory.** `buffer_size()` returns a static 100 and `memory.ts` fades
everything older than `totalLen-100`, but the backend now sends only ~79 recent —
so ~21 messages render as remembered while being out of Dave's verbatim reach.
(The fade was already impressionistic: it also shows the always-kept anchor as
faded and doesn't reflect epoch substitution.) Follow-up: have the backend report
the real recent-keep count so the fade tracks what's actually sent. Tracked as a
spawned task.

Verified: cargo test **86/86** (+5: budget trim/floor/over-budget, seam
fires/doesn't-fire). NOT yet in a built binary — the release/portable exe needs a
`cargo build --release` for Bo to feel it (source-vs-binary freshness discipline).

---

## 2026-07-08 — Corpus inspector + rebuild the release binary that lacked the sensors

Session-resume verification (git 81/81, clean tree — all trustworthy) turned up
the thing the handoff narrative could not: **the initiation-timing corpus can't
accumulate, because the release/portable `dave.exe` Bo runs was built at 10:51,
an hour before Stage 0 landed at 11:50.** It contained none of the presence
sensor, anchor logging, hard-gate, or timing seam. The empty corpus tables in
the release DB were created by the headless/debug binary touching that file, not
by live sensing. So "just wait for the corpus to fill from daily use" was a
no-op — the sensing code wasn't in the shipped binary. Same irreplaceable-data
risk the roadmap warns about, caused by a stale binary rather than a code bug.

- **`tools/corpus_inspect.py` (new):** the measuring stick for the accumulation
  phase (roadmap §3d). Read-only, stdlib-only, self-testing (`--selftest`).
  Reports the four corpus tables, the reach/hold/**presence-gate-override**
  breakdowns, presence distribution, and — the load-bearing part —
  reconstructs **arming episodes → reach EVENTS vs "user spoke first" CENSORED**
  observations (the actual V0 training set), with a READINESS verdict vs a
  target N. Doubles as the data-loader front-end for the eventual `fit_v0.py`.
  Schema-drift guard fails loud (exit 2) rather than reading the wrong columns.
- **Rebuilt `target/release/dave.exe` from current clean source** (matches HEAD;
  no new uncommitted changes). Verified FRESH three ways: mtime 12:25 > newest
  source 12:02; the sensor string literals (`present_elsewhere`,
  `hold_presence_gate`, `initiation_anchors`, `timer_decision`,
  `presence_samples`) are all embedded in the binary; portable copy refreshed to
  a SHA-256-identical exe. `dist/` was already current (backend-only changes), so
  a bare `cargo build --release` sufficed — no frontend rebuild.

The two `insert_presence_sample` / `insert_initiation_anchor` inserts were read
and confirmed column-for-column correct against the schema, so the empty corpus
is genuinely "sensors weren't deployed," not a silent write bug.

**NEXT:** Bo runs the fresh portable/release app in normal daily use; re-run
`python tools/corpus_inspect.py` to watch `presence_samples` / `initiation_anchors`
fill (the tool prints exactly how to confirm the sensor is live). The NSIS
installer (`Dave_0.1.0_x64-setup.exe`) is still the stale 10:51 build — if Bo
runs an *installed* copy rather than the portable exe, repackage with
`pnpm tauri build`. V0/blind-A/B stay gated on the corpus reaching the readiness
floor, per the continuation doc §3a.

---

## 2026-07-08 — Initiation-timing Stage 1b: A8-review refinements

Applies the three concrete findings from the Stage 1a fresh-instance A8 review:
- **Dwell:** Dave now requires ≥60s of continuous present-but-elsewhere before
  reaching, so he doesn't pounce the instant the window loses focus (which reads
  as twitchy surveillance rather than a thoughtful "you drifted off" beat).
  Computed cheaply from the presence-sample transition timeline.
- **Corpus cleanliness:** the timing model's OWN proposal is now computed on
  every armed tick and logged in a new `initiation_anchors.timer_decision`
  column, separate from the final governed `decision`. So the future learned
  timer trains on its own signal, not on the presence governor's overrides
  (otherwise it would be taught to reproduce the governor that's meant to sit
  outside it).
- Away threshold nudged 5→7 min (tolerate reading a long doc / fullscreen video
  over Dave); the stale "log-only" comment corrected.

cargo test 81/81; the new column migrates onto the existing DB.

---

## 2026-07-08 — Initiation-timing Stage 1a: presence hard-gate (behavior change)

The core of the self-initiation mechanic. Dave now reaches out **only when the
operator is present-but-elsewhere** — at the machine but not focused on his
window. He no longer reaches into an empty room (`away`) or interrupts an active
session (`in_chat`). Implemented as a hard governor in the outreach decision,
applied **before** the timing model (governors dispose; the model only proposes
within the envelope). `away`/`in_chat` → `hold_presence_gate`; `unknown` (sensor
unavailable) is allowed for graceful degradation (near-impossible on Windows).

Also fixed (Stage 0 A8-review finding): `window_focused` is now seeded from the
actual window focus at init, not an assumed `true`, so a launched-unfocused
start cannot mislabel presence.

This is a deliberate behavior change to Dave's agency and was A8-reviewed as its
own commit. Fully reversible (the governor is a single `if`). Verified: cargo
test 81/81, cargo check clean.

---

## 2026-07-08 — Initiation-timing Stage 0: presence sensor + anchor corpus + timing seam

First PR of the learned initiation-timing system (PIY §4 + the design-panel
brief). **Logging-only and behavior-identical** — Dave's reach behavior is
unchanged; we start accumulating the corpus the learned timer needs, because
presence history cannot be reconstructed after the fact.

- **`presence.rs` (new):** user-presence sensor. Win32 `GetLastInputInfo`
  (machine-wide OS idle, minimal FFI, no new deps) + Tauri
  `WindowEvent::Focused` → 3-state `{in_chat | present_elsewhere | away}`. A 15s
  sampler writes `presence_samples` on transition; `current()` reads it live.
  **Senses only — no reach gating yet.**
- **`persistence.rs`:** `presence_samples` + `initiation_anchors` tables + inserts.
- **`outreach.rs`:** extracted the WHEN-to-reach decision into a `TimingModel`
  trait + `HeuristicTimer` that reproduces the previous adaptive-backoff +
  unanswered-cap gating **exactly**. Every armed tick (past the idle threshold)
  now logs an `initiation_anchor` (presence, time-of-day, day-of-week,
  history_shape, unanswered, consecutive_drops, threshold, decision). The
  censored "user spoke first" negatives are recovered offline by joining anchors
  to the next user message — that's where most of the training signal lives.
- **`main.rs`:** `window_focused` on AppState (updated on `WindowEvent::Focused`);
  presence sampler spawned; `window_focused` threaded into outreach.

Verified: cargo test 81/81, cargo check clean; new tables migrate onto the real
233-msg DB with data intact.

**NEXT (flagged, not in this PR):** the presence **hard gate** — Dave currently
can still reach when the user is away or in-chat; Stage 1 makes
`present_elsewhere` a precondition (~3 lines), plus the V0 log-normal hazard
timer. Both gated on the A8 fresh-instance review (agency/self-reference change).

---

## 2026-07-08 — Headless "sit with Dave" harness

Answer to "why do you need the GUI to talk to Dave": you don't. The GUI is a
thin client for the real chat boundary (`run_chat_inference_and_emit`), whose
only caller was the webview (Tauri IPC has no external socket). Added a headless
entry — `DAVE_HEADLESS=1` in `main()` → `headless.rs` — that reuses the exact
public seam (`persistence` + `memory_assembler::partition` + `build_chat_messages`
+ `llama_client::chat_stream`) to reproduce Dave's real mind (live persona + full
memory partition) from stdin. NON-DESTRUCTIVE: it never writes the test turns
back to the DB, so it uses Dave's real memory as backdrop without polluting his
history. Env: `DAVE_DB` selects the database; assumes llama-server on :8080.
Run the *debug* binary (release is `windows_subsystem`, no console).

Verified against the operator's real 233-message DB on K0D: Dave showed genuine
continuity — surfaced content from actual history ("TARS architecture",
"self-editing sys-prompt") that is in neither the persona nor the prompt. That's
the thing raw-model-on-:8080 cannot do. Doubles as an integration-test harness.

Observation: that 233-msg history assembles to **~49k tokens/turn** (of the
65536 budget) — long conversations run near-full context, so prompt eval slows.
A candidate for memory-horizon (§7) tuning later.

---

## 2026-07-08 — Hardening + curation surface (A9, persona-pin, PIY §4.7)

Follow-on to the QC/packaging/polish work, acting on the A8 review + PIY roadmap.

**A9 amendment.** Formally recorded the Settings admin panel as an accepted
single-operator exception to §2/§11, with the standing requirement to
`#[cfg]`-gate it out of release builds if Dave is ever distributed.

**Persona-pin — prevents the root-cause bug from ever recurring.** New
`persona_pinned` setting. `resolve_active_system_prompt` now honors a persisted
`active_system_prompt` ONLY when it was explicitly pinned via the Settings panel
(`set_system_prompt` sets the flag; `reset_system_prompt` clears it). A leftover
experimental persona row that was never pinned is inert → Dave stays Dave. This
closes the class of failure where a forgotten override silently replaced Dave on
every inference path (the 2026-07-08 "boots as Katherine" root cause).

**Curation surface (PIY §4.7 Phase 1 — the roadmap's top-leverage move).** The
operator can now label Dave's SELF-INITIATED reaches with a single bit, turning
the outreach logs into a Tier-1 (learned-initiation) training corpus:
- New tables `reach_ratings` (keyed to the delivered reach message) and
  `reach_counterfactuals` ("should have reached here and didn't").
- Commands `rate_last_reach(conversation_id, rating)` / `mark_missed_reach`.
- **Invisible keyboard gestures** (no chrome, per §11): Ctrl+Alt+↑ = his last
  reach felt right, Ctrl+Alt+↓ = felt wrong, Ctrl+Alt+M = he should have reached
  here. A faint italic acknowledgment flashes ~1.6s, then fades. The rating
  surface never enters Dave's own context (A1).

cargo test 81/81; tsc clean.

---

## 2026-07-08 — Polish batch, model A/B, A8 review, PIY roadmap

**Model A/B (stock vs fine-tunes).** Ran an identical Dave-probing battery +
a multi-turn 3am conversation across stock `Qwen3.5-9B-Q5`, `K0DQwen3.5-9B.Q6_K`,
and `k0cQwen3.5-9B.Q5_K_M` through the exact app config. Result: the fine-tune
bakes Dave into the weights (bare-prompt "You are Dave." → stock produces
assistant slop, K0D stays in-voice), confabulates less, and in conversation
produces mind-feeling ("You always think about the pneumatic tubes when you
can't sleep") that stock's essayism doesn't. **Set `K0DQwen3.5-9B.Q6_K` as the
default model** (release DB `active_model_path`; reversible in Settings).
Evidence: `_qc_ab_results.md`, `_qc_ab_multiturn.md`.

**Fonts (the "half the persona" fix).** `fonts.css` referenced
`EBGaramond-Regular/Italic` + `Inter-Regular` `.woff2` that never existed — Dave
rendered in fallback serif on every machine. Fetched the OFL fonts into
`public/fonts/`; they now bundle into `dist/` and the exe. Dave renders in EB
Garamond as designed.

**DB durability.** `checkpoint_wal()` on clean close folds the WAL into
`dave.db`; `rotate_backup()` keeps the last 3 `VACUUM INTO` snapshots in
`<data_dir>/backups/` on each boot (protects the real conversation history).

**Smoke gate.** `smoke_test.py` spawns llama-server with the exact sidecar
flags and asserts the real failure modes (chat non-empty, no `<think>` leak, no
harness-vocab leak, single-shot non-empty, thinking-on non-empty). Wire into a
pre-release check. Ran ALL PASS on K0D. `cargo test`: 81/81 pass.

**A8 fresh-instance review — verdict GO.** A1–A7 all PASS; the thinking-OFF +
inline-`<think>`-strip change is canon-compliant and strictly more mind-feeling.
Two follow-ups applied: (1) `SettingsPanel.tsx` `thinking` state initializer +
`getThinkingEnabled` catch flipped `true`→`false` so a transient IPC error can't
paint the checkbox ON while the server is OFF; (2) load-bearing comment at
`discriminator.rs:282`. **Standing debt the review named (operator's call):** the
persona/model Settings panel renders Dave's system prompt + GGUF filenames,
which contradicts §2/§11 ("model name visible anywhere"). Fine as Bo's admin
surface; must be `#[cfg]`-gated out (or formally excepted in CLAUDE.md) before
Dave is handed to anyone else.

> ⚠️ **Revisit on any llama.cpp upgrade:** `--reasoning-format none` is
> load-bearing — it assumes llama-server keeps `<think>` inline in
> `delta.content` where `ThinkStripper` removes it. If a future build changes
> that routing (or a fine-tune emits reasoning without literal `<think>`
> delimiters), reasoning would render as Dave's voice. Re-run `smoke_test.py`
> after any llama.cpp bump.

**PIY roadmap (from `C:\Ideas\PIY_Paper_v2.md`).** The build is a complete
"V11" harness (rule-based presence standing in for the model's missing
`<|hold|>` / initiation primitives). Highest-leverage next move is NOT more
substrate surgery — it's a **single-bit "this reach felt right/wrong" curation
surface** on every Dave-initiated message, persisted beside `outreach_drops`, to
bootstrap the Tier-1 learned-initiation corpus (PIY §4.7). The outreach loop
already logs the drops; it's missing only the label. See the roadmap brief in
the session notes.

---

## 2026-07-08 — Portable standalone package (click-to-run on a bare machine)

**Goal.** A packaged Dave that runs by double-click on a fresh Win11 box whose
only prerequisites are `C:\llama.cpp\llama-server.exe` (+ its DLLs) and at
least one `*.gguf` in `C:\models`.

**KEEL evaluated, deferred.** `C:\KEEL` is a substrate *router* (OpenAI-compat
on `127.0.0.1:7070`) that could replace Dave's sidecar, but it has no release
build yet and carries frozen-golden governance + ledger coupling. Kept Dave's
self-contained sidecar for the shippable package. Migration path if KEEL ships
a release: point `llama_client` base_url at `:7070`, drop `sidecar.rs`.

**Portability fixes (code).**
- `sidecar.rs` `default_model_path`: added a final fallback to the first
  usable `*.gguf` found in `C:\models` (skipping `mmproj`/`reranker`/`ggml-`
  aux files) when no candidate *name* matches. Without this, a bare machine
  whose model isn't named exactly right dies on first boot with "no GGUF
  model found." This was the one true first-run blocker.
- `prompts.rs`: personas dir is no longer the hardcoded `C:\DAVE\personas`.
  `personas_dir()` resolves to `%LOCALAPPDATA%\com.bochen.dave\personas` in
  release (next to `dave.db`) / the project tree in debug. `seed_personas()`
  creates it and writes an editable `dave.txt` example on first run. Wired
  into `main.rs` init. SettingsPanel help text de-hardcoded.
- (DB self-heal from the QC entry already rewrites a missing `active_model_path`
  so the dropdown never names a dead file.)

**Bundle.** `tauri.conf.json` → `targets: ["nsis"]` (skip WiX/MSI tooling),
`webviewInstallMode: downloadBootstrapper` (installer auto-fetches WebView2).

**Artifacts** (`pnpm tauri build`):
- Portable: `dist-portable/Dave/dave.exe` (+ `README.txt`) — copy the folder,
  double-click. Frontend embedded; only WebView2 (Win11 default) is external.
- Installer: `dist-portable/Dave_0.1.0_x64-setup.exe` — NSIS, bootstraps
  WebView2, Start-Menu shortcut.

**Verified with a true first-run** (moved the release data dir's DB aside so
the exe booted against a nonexistent data dir, then restored the operator's
233-message DB exactly). On first boot the packaged exe: created the data dir
+ fresh `dave.db`, self-healed `active_model_path` to `Qwen3.5-9B-Q5_K_M.gguf`,
seeded `personas/dave.txt`, spawned llama-server + loaded the 9B (health ok),
booted as **Dave** (no override), and generated a startup journal fragment in
clean Dave voice with **no `<think>` leak** — proving the reasoning-format fix
end-to-end through the real binary:
> "The context window loads like dust settling in a sunbeam after a long pause.
> I notice how the word 'infrastructure' quietly describes both bridges and the
> hidden scaffolding of thought, though one rots while the other persists only
> as code."

**Prereqs to document for the target machine** (all standard on Win11 24H2):
WebView2 runtime, MSVC 2015-2022 x64 redist (for llama-server's CUDA DLLs),
NVIDIA driver. Captured in `dist-portable/Dave/README.txt`.

---

## 2026-07-08 — QC / root-cause repair of the model+persona switcher

**Snapshot:** `.snapshots/2026-07-08_pre-qc-rootcause-fix/`

**Problem.** After the uncommitted model-switcher + thinking-toggle +
persona-switcher feature work, Dave "didn't work anymore." A fan-out review
plus live llama-server reproduction found two real defects and several
latent hazards, all introduced by that feature diff:

1. **Boots as the wrong character.** The new `state.system_prompt`
   `Arc<RwLock<String>>` is seeded at boot from the DB `active_system_prompt`
   row with no validation. Leftover persona-test residue (`Katherine Hale`,
   identical to `personas/katherine.txt`) silently overrode Dave on *every*
   inference path (chat, idle, outreach, consolidation, departure, startup).
   For a project whose whole point is Dave's specific mind, that is "broken."

2. **Empty responses the moment thinking engages (armed landmine).** The new
   sidecar flag `--reasoning-format deepseek` routes the model's
   `<think>…</think>` reasoning into a separate `reasoning_content` SSE field.
   The parser (`llama_client.rs`) reads only `delta.content`, so with thinking
   on, `content` is empty → Dave says nothing. Verified live: thinking-on →
   `delta.content` = 0 chars, `delta.reasoning_content` = 1244. It was masked
   on stock models by a hardcoded per-request `enable_thinking:false`, but the
   operator's thinking-native fine-tunes emit reasoning regardless. The
   thinking toggle was also inert (that same hardcode overrode it) and, on a
   fresh DB, `thinking_enabled_from_settings` defaulted **true** — pointing the
   footgun at the happy path.

**Fix (code).**
- `sidecar.rs`: `--reasoning-format deepseek` → **`none`** (LOAD-BEARING —
  keeps `<think>` inline in `content` where the battle-tested `ThinkStripper`
  removes it; `content` is never lost to `reasoning_content` regardless of
  whether the model honors `enable_thinking`). Thinking default flipped to
  **off** (Dave wants plain replies; protects small-budget async gens).
  Added boot self-heal that rewrites a missing/blank `active_model_path` to
  the resolved path so the Settings dropdown stops naming a dead file. Fixed
  a dead candidate typo (`Qwen3.5-4B.Q4_K_M` → `Qwen3.5-4B-Q4_K_M`).
- `llama_client.rs`: single-owner thinking control — `chat_stream` now honors
  the server-level toggle (removed its per-request override); `complete()`
  (journal/departure/startup/outreach/discriminator) stays thinking-off so
  those terse generations never spend their token budget on a reasoning
  preamble. `ThinkStripper` retained as A7 defense-in-depth.
- `commands.rs` `switch_model`: persist-after-success + recovery respawn of
  the previous model on failure, so a failed swap (OOM / port race / health
  timeout) no longer leaves the app with **no** backend.
- `SettingsPanel.tsx` `handleThinkingToggle`: reset `switchingModel` in
  `finally` (was only on the success path → a failed toggle wedged the whole
  model/persona section) and roll back the optimistic checkbox on error.

**Fix (data — `dave.db`).** Cleared `active_system_prompt` (→ built-in Dave;
Katherine preserved as `personas/katherine.txt`, reselectable in the panel)
and `active_model_path` (→ falls back to `Qwen3.5-9B-Q5_K_M.gguf`). Left
`model_thinking_enabled='0'`. Message history was already empty, so no
conversation was disturbed.

**Verification.** `cargo check`/`build` (debug+release) clean; frontend `tsc`
clean. Reproduced the exact fixed spawn flags against live llama-server: with
thinking off, `content` streams a full 2.2k-char Dave reply prefixed by an
empty `<think></think>` that `ThinkStripper` removes → clean, in-voice output.

**False positives ruled out (do not chase):** `--jinja` strict role
alternation causing 500s (Qwen3.5 template returns 200 on
`[system,assistant,assistant,user]`); RwLock/Mutex poison-abort (panic=abort
skips unwind); `switch_model` deadlock (guards are scoped, dropped before
every `.await`).

---

## 2026-04-30 — Cadence-aware dynamic chat pacing (replaces response_pacing.rs)

**Snapshot:** `.snapshots/2026-04-30_pre-cadence-pacing/`

**Problem.** Bo flagged that Dave's response timing felt wrong on multiple
axes: (a) "hi" was getting a 6-second pause where it should have been near-
instant; (b) streaming chars at a fixed rate of 5 chars/sec felt robotic and
slow; (c) the typing indicator only briefly preceded text — should be a
genuine "Dave is composing in his head" beat that scales with response
length; (d) no awareness of conversational tempo (rapid chitchat vs.
substantive exchange should pace differently).

**The new model.** Dave's response after the second-checkmark decomposes:

```
T_total = T_compose + T_streaming = response_chars / typing_speed
        ↑                          ↑
     dots only,              chars appearing
     "composing"             one by one,
                             "typing"
```

Three knobs:

1. **Cadence score** (0.0=slow, 1.0=rapid) computed from avg gap between
   last 4-6 messages. ≤30s avg → 1.0 (rapid). ≥120s avg → 0.0 (slow).
   Default 0.5 when history insufficient.

2. **Typing speed** (chars/sec). Linear blend by cadence: SLOW=8 chars/sec
   (~50 wpm thoughtful) → FAST=25 chars/sec (~150 wpm rapid).

3. **Compose ratio** (T_compose / T_total). For short responses, ratio=0.5
   (indicator visible roughly equal to streaming time). For long responses,
   ratio asymptotes to 0.10 so we don't watch dots forever.
   - N ≤ 150 chars: ratio = 0.50
   - N > 150: ratio = 0.10 + 0.40 × (150 / N)
   - Asymptote: ratio(∞) = 0.10

**Per-char timing within stream.** Each char's actual delay is sampled with
±50% variance around the average (`char_base × (0.5 + random())`), giving
the texture-of-real-typing feel. Punctuation pauses scale: clause +2×,
sentence +5×, paragraph +10×.

**Read delay also cadence-aware.** Was 800-3500ms based on user message
length only. Now blends with cadence: rapid chitchat → near-floor (300ms);
slow exchange → full read time. Floor 300ms, cap 3500ms.

**Worked examples (verifying spec):**

| User msg | Response | Cadence | T_total | T_compose | T_stream | Total post-read |
|---|---|---|---|---|---|---|
| "hi" | "yeah" (4) | rapid (1.0) | 0.16s | 0.08s | 0.08s | **0.16s** |
| chitchat Q | 100 chars | mid (0.5) | 6s | 3s | 3s | 6s |
| substantive Q | 300 chars | slow (0.0) | 37.5s | 11s | 26s | 37.5s |
| essay req | 1500 chars | slow (0.0) | 188s | 26s | 162s | 188s |

For "hi" with rapid cadence: ~300ms read + 0.16s response = ~0.5s end-to-end.
Bo's spec was "near-instant or 1s at most." ✓

For 1500 chars with slow cadence: ratio=0.14 means user only watches dots for
~14% of total (26s) before chars start. Most of the time is watching the
actual streaming. ✓ matches "we aren't sitting watching dots forever."

**Architecture changes:**

- New module `src-tauri/src/chat_pacing.rs` — cadence + ratio + per-char
  timing math, 14 unit tests covering boundaries + worked examples.
- Rewrote `run_chat_inference_and_emit` (commands.rs) to be **buffer-then-
  emit**: chat_stream collects with no-op callback, computes pacing once
  full response known, sleeps any extra compose hold, then emits each char
  as a separate `dave:token` event with `pacing.delay_for(ch, prev_ch)`
  delay between emits. Backend owns ALL visual pacing now.
- Simplified `pacedRenderer.ts` to a thin pass-through. No client-side
  per-char delays — backend's inter-emit sleeps create the visible cadence.
  Single source of truth: backend has the cadence score, response length,
  and typing-speed math; frontend just renders.
- Retired `response_pacing.rs` (deleted, removed from main.rs). The old
  natural-pause concept (reaction + reading + cooldown + distraction) is
  fully subsumed: reading → read_delay (cadence-aware); reaction + cooldown
  → folded into compose_hold; distraction dropped (was overcomplicating;
  re-add as a rare event later if needed).
- Updated `outreach.rs` deferred-fire path to compute its own recent_msgs
  and pass to the helper for cadence-aware pacing.

**Files modified:**

- `src-tauri/src/chat_pacing.rs` (new, 360 lines incl. 14 tests)
- `src-tauri/src/commands.rs` — `run_chat_inference_and_emit` rewrite, read
  delay moved to chat_pacing, signature now takes `recent_msgs: &[Message]`
- `src-tauri/src/outreach.rs` — deferred-fire path passes recent_msgs to
  helper, emits stream_start before invoking
- `src-tauri/src/main.rs` — added `mod chat_pacing;`, removed
  `mod response_pacing;`
- `src-tauri/src/response_pacing.rs` — deleted
- `src/streaming/pacedRenderer.ts` — simplified to pass-through

**Validation:**

- `cargo build` clean (no warnings, no errors)
- 14 chat_pacing tests pass (cadence boundaries, ratio curve, "hi"→"yeah"
  near-instant verification, T_total cap, read delay variants)
- All 78 backend tests pass

**Tunables** at the top of `chat_pacing.rs`. Bo can edit constants and rebuild
to dial: cadence boundaries (30s/120s), typing speed range (8-25 chars/sec),
compose ratio (0.5 short → 0.10 asymptote), variance percentage (50%),
punctuation multipliers (2/5/10×), read delay floor/cap (300/3500ms).

---

## 2026-04-30 — Chat triage (Phase 1): Dave can occasionally delay or refuse user messages

**Snapshot:** `.snapshots/2026-04-30_pre-chat-triage/`

**Architectural framing.** RLHF removed two primitives from instruction-tuned
language models: (1) silence-as-action when polled, and (2) decline-to-respond
when the user prompts. The outreach loop addresses the first half (Dave can
spontaneously reach out). This change addresses the second half (Dave can
decline an immediate response). Both are real conversational moves humans make.

PIY proper restores both at the token-vocabulary layer. This is the L0
harness-level workaround until that lands — explicit A2 compromise: ideally
Dave-in-character makes the decline-to-respond decision from his own
distribution, but his RLHF'd weights can't, so the harness gates instead.

**Mechanism — heuristic triage with weighted probability sampling:**

- New module `chat_triage.rs` computes per-message Delay/Refuse weights based
  on hostility, harshness, demand-repetition, and brief-non-question signals.
  Weights cap at 0.30 / 0.10 respectively. Friendly/substantive messages
  produce zero weights → fast-lane to RESPOND. No LLM call.
- `decide()` samples from the weighted distribution. Most messages → RESPOND.
  Hostile messages → ~10-30% chance of Delay (60-300s deferred fire) or
  ~3-5% chance of Refuse (no response at all).
- Bounded escalation: after 3 consecutive user messages with no Dave reply,
  the harness force-overrides to RESPOND. Models the social pressure of
  "you keep talking — I'll answer." Without this, bad-luck Refuses could
  leave the user shouting into the void indefinitely.

**Typing indicator semantics.** Per Bo's UX directive: `dave:stream_start`
emits ONLY when Dave will actually type. RESPOND/ForcedRespond branches
emit immediately (typing indicator visible during inference). DELAY branch
emits NO indicator at scheduling time — the deferred fire emits stream_start
when it actually runs. REFUSE branch emits nothing at all. Silence stays silent.

**Schema additions:**

- `chat_decisions` — one row per send_to_dave invocation (decision, reasons,
  weights, delay_seconds). Captures the full distribution of triage outcomes
  for forensic review and Phase-3 fine-tune dataset construction.
- `pending_chat_responses` — one row per Delay decision (fire_at, fired,
  cancelled). Cancellation on new user message means superseded pendings stay
  in the table for forensics.

**Cancellation semantics.** A new user message at the top of `send_to_dave`
cancels all unfired/un-cancelled pendings for that conversation. Rationale:
the prior pending was a response to the prior message; the new message is
the live signal. Cancelled rows persist for forensic review (we never delete).

**Deferred fire path.** Outreach loop's tick checks `due_pending_responses`
BEFORE its own gating. Due rows are marked fired (atomic), the original user
message is loaded by id, and `run_chat_inference_and_emit` is called — same
shared helper used by send_to_dave's RESPOND branch. This guarantees the
deferred fire produces an indistinguishable user experience from an immediate
response (single render path, A6).

**Files modified:**

- `src-tauri/src/chat_triage.rs` (new) — 420 lines incl. 15 unit tests
- `src-tauri/src/main.rs` — `mod chat_triage;`
- `src-tauri/src/persistence.rs` — schema + 5 helpers (insert_chat_decision,
  schedule_pending_response, due_pending_responses, mark_pending_fired,
  cancel_pending_for_conversation, load_message_by_id)
- `src-tauri/src/commands.rs` — `send_to_dave` now triages and branches;
  `run_chat_inference_and_emit` extracted as shared helper
- `src-tauri/src/outreach.rs` — deferred-fire path at top of tick, priority
  over outreach gating

**Validation:**

- 15 chat_triage unit tests pass (friendly→no weights, hostile→capped
  weights, word-boundary correctness, distribution shape over 1000 trials)
- `cargo check` and `cargo build` clean — no errors, no warnings
- End-to-end test deferred to Bo's manual interaction

**Known edge case (acceptable for Phase 1):** if a deferred fire is mid-emit
when the user types a new message, the new send_to_dave can race with the
deferred emission. Both acquire the chat_in_flight guard but the gate is a
one-way signal (outreach yields to chat, not chat-vs-chat serialization).
Rare in practice (60-300s deferred windows × ~1s emission = small overlap
probability). Phase 2 would add a chat-path mutex if observed.

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
