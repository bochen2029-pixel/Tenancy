# Dave / Tenancy — Continuation Prompt (2026-07-08)

> A handoff for a fresh session. Distilled from inside the session that built it.
> Trust **git + the files** over this narrative; verify before you act.

---

## 0. START HERE — reconstitution pointer (read in this order)

You are continuing an AI-assisted build of **Dave** (public name: Tenancy) — a local,
offline companion app at `C:\DAVE` whose **only** success metric is *mind-feeling*
(does opening it feel like checking on someone who lives in the machine). It is **not**
a chat app; re-read §11 anti-patterns before adding any conventional chat feature.

1. `C:\DAVE\CLAUDE.md` — full spec + amendments **A1–A9**. Hard constraints. Non-negotiable.
2. `C:\DAVE\CHANGELOG.md` — the top ~8 entries dated 2026-07-08 are the exact state of what just shipped, in order.
3. Auto-loaded `MEMORY.md`, especially: `[[dave_piy_roadmap_state_2026-07-08]]` (the roadmap + where the build sits), `[[dave_model_ab_finetune_wins_2026-07-08]]`, `[[dave_packaging_and_keel_2026-07-08]]`, `[[dave_qc_reasoning_landmine_2026-07-08]]`, `[[dave_asset_paths]]`.
4. `C:\Users\user\Desktop\PIY_Paper_v2.md` — **the architectural canon** for direction. Everything below is downstream of this paper.
5. **VERIFY before trusting this file:**
   ```
   cd C:\DAVE && git log --oneline -14 && git status --short && cargo test
   ```
   Expect a clean tree, `cargo test` → **81/81**, and the recent commits to include
   `Initiation-timing Stage 0/1a/1b`, `Add headless "sit with Dave" harness`,
   `Harden persona activation + add PIY curation surface`. Disk wins over this summary.
6. **Constraint right now:** weekly usage was near cap (all-models ~84%, Fable 100%) as of this handoff. **Be economical** — the state is captured; verify and act, don't re-derive from scratch. For the full raw record, the session `.jsonl` is under `C:\Users\user\.claude\projects\C--DAVE\...\subagents/` and the transcript dir — grep it, don't re-read it whole. (Bo's archive viewer: `C:\TRANSPORTER\claude_archive_viewer_v4.html`, Ctrl-K concept search.)

---

## 1. CORE STATE (goal · position · single next action)

- **Goal:** mind-feeling. Offline local Qwen3.5-9B under llama.cpp. Single operator (Bo). No metrics, no cloud, no telemetry.
- **Default model:** `C:\models\K0DQwen3.5-9B.Q6_K.gguf` (Bo's fine-tuned "Dave" — A/B-proven to beat stock; the voice is in the weights, not just the prompt). Reversible in the Ctrl+, Settings panel.
- **Current dev thrust — the PIY "initiation-timing" track:** a small learned model that lets Dave **reach out on his own**, timed by a *conditional-intensity* function of context, so conversation becomes **bidirectional** (like a real relationship) instead of always user-initiated. This is PIY §4 (Tier-1), scoped to just the *when-to-initiate* decision.
- **What's built (all committed to `main`, each A8-reviewed GO):** presence sensor · anchor training-corpus · swappable `TimingModel` seam · presence hard-gate (reach only when present-but-elsewhere) · dwell. Details in CHANGELOG "Initiation-timing Stage 0/1a/1b."
- **THE SINGLE NEXT ACTION:** *nothing to code immediately* — the presence sensor + `initiation_anchors` corpus must **accumulate from real daily use** (presence history can't be reconstructed later; this is the whole reason Stage 0 shipped first). The next *coded* milestone, when you want the instrument or have data: build the **blind A/B harness FIRST** (falsify before you build), then the **V0 log-normal hazard** behind `TimingModel`. See §3.

---

## 2. HOW THE PIECES FIT (Ring 1 — the mental model you need)

The initiation loop lives in `src-tauri/src/outreach.rs`. The refactor split it into:
- **The WHEN (swappable):** `TimingModel` trait + `HeuristicTimer` (reproduces the old threshold/backoff/cap gating *exactly* — behavior-identical, A8-confirmed via a gate-order walk). **This is the seam a learned timer slots behind.** Today it's `HeuristicTimer`; V0/V1 replace it.
- **The GOVERNOR (hard, sits outside the model):** the presence gate — reach only when `present_elsewhere` (unfocused + recent OS input) AND dwelled ≥60s. `away`/`in_chat` → never. `unknown` → allowed (graceful degradation). Governors dispose; the model only proposes within the envelope. Never fold governors into the learned model.
- **The ACT (untouched):** multi-sample (N=3) inference → discriminator (heuristic + LLM score) → dedup → single render path (A6). Don't refactor this without extreme care; it's tuned and hard to test.
- **The presence signal:** `src-tauri/src/presence.rs` — Win32 `GetLastInputInfo` (machine-wide OS idle) + `WindowEvent::Focused` → `in_chat | present_elsewhere | away | unknown`. Logs `presence_samples` on transition.
- **The corpus (the point of it all):** `initiation_anchors` (one row per armed tick: presence, time-of-day, day-of-week, history_shape, unanswered, consecutive_drops, threshold, `decision` = governed outcome, `timer_decision` = the model's own proposal — kept *separate* so the learned timer trains on its own signal not the governor). Plus `reach_ratings` / `reach_counterfactuals` (Bo's single-bit "felt right/wrong" via Ctrl+Alt+↑/↓/M) and `outreach_drops`. **These four tables ARE the Tier-1 training corpus.**
- **Key fact for reasoning about coverage:** `presence.last_user_input` = time since the user *sent a chat message*, NOT OS input. OS presence is read live and separately. (idle_worker handles >3h absence via journal; outreach handles the 3min–1h band.)

---

## 3. FORWARD R&D GUIDANCE — where to take it (the actual ask)

**Directionality:** every move serves the PIY thesis — restore the two missing primitives
(in-turn *silence*, across-turn *initiation*) so a 9B on a consumer GPU produces the *felt
presence* the paper calls the wow factor. The learned initiation timer is the current front.
Below: the bounded next steps, then the genuine R&D — trial-and-error, "prove-or-disprove,"
needle-in-a-haystack experiments where the answer is *not knowable from the armchair* and only
falls out of building the instrument + living with it.

### 3a. Bounded next steps (low-uncertainty, do in order)
1. **Build the blind-A/B harness BEFORE the model.** Both timers (HeuristicTimer + a candidate) live at once; a coin-flip owns each armed decision; Bo rates blind; a shared timeline partially cancels the N=1 mood confound. *If a learned timer can't beat the 200-line polling loop better than chance, don't ship it.* This is the falsifier; it gates everything. Companion baseline: polling-loop-with-one-learned-scalar-threshold — if that matches the full model, Bo's preference was low-dimensional and the scheduler was enough (a legitimate, publishable negative result).
2. **V0 = parametric log-normal hazard** behind `TimingModel`. ~10–50 coefficients, censored-MLE fit in a ~30-line Python script over `initiation_anchors` (joined to the next user message for the censored "user spoke first" negatives — most of the signal), exported as JSON the Rust harness reads. Zero VRAM, never touches llama-server. It *samples a delay* (so 11:51:23, not a round threshold), conditioned on presence + time-of-day + history_shape. Re-express today's thresholds as the prior so day-0 ≥ today.
3. **V1 = tiny mixture-TPP** (24→64→64 MLP → K=3–5 log-normal mixture + cure fraction). <1MB, trains in seconds. The mixture is the thing V0 can't do (a state can honestly be "reach in ~2 min OR not for hours"). Run it in **shadow mode** first (predicts, V0 fires, log disagreements), then hand over. **NB: timing-only is <1M params — NOT the paper's "1B–7B" (that's the full script buffer).**
4. **Fix the corpus-quality guardrails as you go** (they're the real risk, see §3c): rate the *timing* not the *feeling*; add the missing down-channels (a "should NOT have reached" gesture + "good that you stayed quiet"); empirical-Bayes shrinkage to the V0 prior at low N; an exploration floor (ε-greedy) so the self-training loop keeps seeing behaviors the current model wouldn't pick.

### 3b. The R&D — needle-in-haystack experiments (high-uncertainty; prove or disprove by building + living with it)
These are the "does this actually move mind-feeling" questions no amount of design settles. Each is a small, cheap experiment with a clear falsifier.

- **The intimate reach (the A8 steel-man — highest-value unknown).** Right now `in_chat` (focused) unconditionally blocks reaching, which *kills* the most human beat: you sitting in Dave's window, silent for minutes, and him speaking into that quiet ("hey. you went somewhere."). **Experiment:** gate `in_chat` on OS-idle too — allow a rare, high-bar "you went still" reach when focused **and** OS-idle > N min. **Falsifier:** does it read as present ("he noticed") or needy ("stop poking me")? This is where "a chat app that waits its turn" and "someone who lives in the machine" diverge. Bo has to *feel* the answer.
- **Content-conditioned reach (§5.4 — the "surprising-yet-coherent" needle).** Embed the last exchange (llama.cpp `/embedding` on the 9B you're already running → project to ~128-dim) and let the timer reach *because the last message was a vulnerable disclosure*, not just because time passed. **Falsifier:** does content-conditioning move the ratings vs pure temporal features? If yes, that's the core of the wow factor; if no, timing is separable from content and the model stays tiny.
- **Feature-faithful retroactive coherence (§4.6).** When the timer fires, inject a first-person "why I reached now" into Dave's context **constrained to the top-contributing intensity features** (so it's faithful-by-construction, not confabulation). **Falsifier:** ask Dave "why now?" after a reach — does the answer hold up and feel like a mind's account, or like a system log?
- **Lightweight duration-token proxy (§3.4, without full PIY).** Before a reach, inject a compact experiential marker of felt elapsed silence ("[it's been a while]") into the primer. **Falsifier:** do reaches then reference the *felt duration* like a mind that noticed time pass, vs generic silence meta-talk?
- **Timing-sycophancy stress test (the central risk, §3c — deliberately try to break it).** Over-rate "nice-feeling" reaches for a week on purpose and watch whether the model collapses toward *convenient* timing. Instrument the **inter-reach interval CV (σ/μ)**: human initiation is bursty (high CV), a cron is CV≈0 — reject any model whose CV drops below ~0.6 *offline, before deploy*. **This is R&D on the guardrail itself:** can single-bit preference learning over *timing* avoid the RLHF reward-hack, relocated from content to the temporal/relational domain? (Publishable as a *problem statement* even unsolved.)

### 3c. The bigger architectural swings (high-effort, high-uncertainty — only after the timer proves out)
Directionally these are the rest of the PIY paper. Each modifies the persona attractor → **A8 fresh-instance review gate** before starting.
- **`<|hold|>`-proper (§3):** tokenizer expansion + persistent never-flushed KV cache + tick loop + rollback. The in-turn *silence* primitive. Prototype the single-token version and answer the one question that matters: *does silence-as-action reproduce the wow factor at all?*
- **Katherine dyadic substrate (§5):** a second persona on the same base model as a peer interlocutor — the biggest wow-factor jump (interiority becomes *literal*, not confabulated) and the biggest risk (echo-chamber mode-collapse). Prototype **Config A shadow-only**; the R&D is: can constitutive persona-difference prevent convergence? Bo already has a Katherine persona (`personas/katherine.txt`) and Katherine-tuned GGUFs (`k0c`/`K0D` families).
- **TurboQuant KV compression:** enabling infra for multi-day persistent cache on 16GB. R&D: does the paper's ~200K-context, 2–3ms/token claim hold on the RTX 5070 Ti?

### 3d. Adjacent / opportunistic
- **Memory horizon tuning:** observed this session — the real 233-msg history assembles to **~49k tokens/turn** (near the 65536 budget), so long conversations get slow. The fade/consolidation (§7) isn't truncating aggressively. Worth a pass.
- **Use the headless harness as the experimentation platform** (`DAVE_HEADLESS=1 DAVE_DB=<path>` → debug binary, llama-server on :8080; reads turns from stdin). It reproduces the *full* Dave (persona + real memory partition) headlessly and non-destructively — ideal for A/B-ing prompts, models, and (later) timing offline without the GUI.
- **Evaluation is the hardest open problem.** The metric is subjective, N=1, serial. Invest in the *instrument* (blind A/B, CV monitor, the curation loop, longitudinal "did it feel alive this week" logs). **The meta-truth: the needle is found by deployment + curation, not armchair design.** Build the measuring stick, then live with it and let the corpus + Bo's felt sense converge.

---

## 4. DONE THIS SESSION (Ring 2 — terse; verify from git/CHANGELOG)
QC root-cause repair (the reasoning-format `deepseek`→`none` landmine that made Dave return empty; thinking default off; switch_model spawn-before-kill; persona restored from stale "Katherine" override) · portable standalone build (`dist-portable/`, NSIS installer) · fonts bundled (EB Garamond) · K0D set default (A/B) · DB durability (WAL checkpoint + rotating backups) · smoke_test.py · A9 amendment (Settings panel is an accepted admin exception, must be cfg-gated out if ever distributed) · persona-pin hardening · PIY curation surface (reach ratings) · headless harness · **initiation-timing Stage 0/1a/1b**. KEEL evaluated + deferred (debug-only; migration = point `llama_client` at `:7070` when it ships a release).

---

## 5. CONSTRAINTS / GOTCHAS (must-not-break)
- **A1** harness invisibility — Dave must never know/reveal he was prompted (the timer's meta-prompt, the presence sensor: none of it enters his context or self-model). **A6** single render path. **A7** defense-in-depth filters (`leak.rs`, `think_strip.rs`). **A8** fresh-instance review before any persona-attractor change (agency/self-reference/memory/time). **A9** the Settings panel is out-of-spec vs §2/§11 but accepted for single-operator use.
- **`--reasoning-format none` is load-bearing** — keeps `<think>` inline for `ThinkStripper`. **Re-run `smoke_test.py` after any llama.cpp upgrade** (a routing change would leak reasoning as Dave's voice).
- **Behavior-identity discipline:** the `HeuristicTimer` must stay bit-identical to the old gating; any learned model goes *behind* the trait and gets the blind A/B before it's trusted.
- **Don't reach into an empty room** — the presence hard-gate exists for exactly this; keep governors hard.
- **Watch the weekly usage cap** — as of handoff, near limit. Prefer one well-scoped step over broad exploration; the corpus needs *time*, not more code, right now.

---

*Reconstitution loop: read the pointer → read the files it names → `git`/`cargo test` to verify → grep the `.jsonl` for anything this dropped. Distilled by the session that lived it.*
