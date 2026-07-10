#!/usr/bin/env python3
"""Dave initiation-timing corpus inspector — the measuring stick for the
data-accumulation phase of the PIY learned-initiation track.

The learned timer (V0 log-normal hazard -> V1 mixture-TPP) cannot be fit until
the corpus has accumulated from REAL daily use. Presence history and armed-tick
anchors cannot be reconstructed after the fact, so the track shipped its sensors
first (Stage 0/1a/1b) and now WAITS. This tool turns that wait into an
observable process: it reads the four corpus tables read-only and reports

  - how full each table is (and whether the sensors are actually logging),
  - the decision / governor / presence breakdowns you'll condition V0 on,
  - the censored-negative reconstruction (arming episodes -> reach EVENTS vs
    "user spoke first" CENSORED observations) that IS the V0 training set, and
  - a READINESS verdict: how far from being able to fit V0.

It never writes. It doubles as the data-loader front-end for the eventual
`fit_v0.py` (§3a step 2 of the continuation doc): the same episode
reconstruction feeds the censored-MLE.

Usage:
    python tools/corpus_inspect.py                # release DB (%LOCALAPPDATA%)
    python tools/corpus_inspect.py --db PATH      # a specific dave.db
    python tools/corpus_inspect.py --debug-db     # the debug DB at C:\\DAVE\\dave.db
    python tools/corpus_inspect.py --episodes     # also dump per-episode rows
    python tools/corpus_inspect.py --selftest     # synthetic-fixture self-test

Exit code 0 = ran clean (any corpus size). 2 = schema drift / unreadable DB.
1 = selftest failure.
"""
import argparse
import os
import sqlite3
import sys
from collections import Counter
from datetime import datetime, timezone

RELEASE_DB = os.path.expandvars(r"%LOCALAPPDATA%\com.bochen.dave\dave.db")
DEBUG_DB = r"C:\DAVE\dave.db"

# The columns the offline analysis depends on. If the live schema ever drifts
# from these, the inspector fails LOUD (exit 2) rather than silently reading the
# wrong thing — the same "protect the irreplaceable corpus" discipline the
# sensors were shipped under.
EXPECTED_COLUMNS = {
    "presence_samples": {"ts", "state", "os_idle_ms", "focused"},
    "initiation_anchors": {
        "ts", "conversation_id", "seconds_since_user_input", "presence_state",
        "focused", "os_idle_ms", "history_shape", "unanswered_reaches",
        "consecutive_drops", "threshold_seconds", "time_of_day_min",
        "day_of_week", "decision", "timer_decision",
    },
    "reach_ratings": {"message_id", "rating", "created_at"},
    "reach_counterfactuals": {"conversation_id", "at_message_id", "created_at"},
}

# A rough floor for a first honest V0 fit: enough distinct arming episodes, with
# both a reach signal and censored negatives, that a ~10-50 coefficient
# log-normal hazard isn't just memorizing. Not a hard law — a nudge so you don't
# fit noise. The mixture-TPP (V1) wants more.
V0_MIN_EPISODES = 150
V0_MIN_EVENTS = 20          # at least this many actual reaches to anchor the hazard
V0_MIN_CENSORED = 40        # ... and this many "user spoke first" negatives


def table_exists(conn, name):
    return conn.execute(
        "SELECT 1 FROM sqlite_master WHERE type='table' AND name=?", (name,)
    ).fetchone() is not None


def columns_of(conn, name):
    return {r[1] for r in conn.execute(f"PRAGMA table_info({name})").fetchall()}


def count(conn, name):
    if not table_exists(conn, name):
        return None
    return conn.execute(f"SELECT COUNT(*) FROM {name}").fetchone()[0]


def ts_range(conn, name, col="ts"):
    if not table_exists(conn, name):
        return None
    r = conn.execute(f"SELECT MIN({col}), MAX({col}) FROM {name}").fetchone()
    return r if r and r[0] is not None else None


def fmt_ts(t):
    if t is None:
        return "—"
    return datetime.fromtimestamp(t, tz=timezone.utc).astimezone().strftime("%Y-%m-%d %H:%M")


def fmt_span(rng):
    if not rng:
        return "no rows"
    lo, hi = rng
    days = (hi - lo) / 86400.0
    return f"{fmt_ts(lo)} -> {fmt_ts(hi)}  ({days:.1f} days)"


def bar(n, total, width=24):
    if not total:
        return ""
    filled = int(round(width * n / total))
    return "█" * filled + "·" * (width - filled)


def hr(title=""):
    line = "─" * 66
    if title:
        return f"\n{title}\n{line}"
    return line


# ─────────────────────────────────────────────────────────────────────────
# Censored-negative reconstruction — the V0 training set.
#
# An "arming episode" begins when the user last spoke (user_last_spoke_ts =
# anchor.ts - anchor.seconds_since_user_input) and ends either when Dave REACHED
# (a message with initiated_by_dave=1) -> EVENT, or when the USER spoke again
# first -> CENSORED. Each armed tick is a sample of that episode's survival at
# elapsed = seconds_since_user_input; multiple anchors in one episode share a
# terminal outcome, so for the honest "how many independent training signals do
# I have" count we collapse to one row per (conversation, user_last_spoke_ts).
# ─────────────────────────────────────────────────────────────────────────
def reconstruct_episodes(conn):
    if not table_exists(conn, "initiation_anchors"):
        return []
    anchors = conn.execute(
        """SELECT conversation_id, ts, seconds_since_user_input, presence_state,
                  time_of_day_min, day_of_week, history_shape, threshold_seconds,
                  decision, timer_decision
           FROM initiation_anchors ORDER BY ts"""
    ).fetchall()

    # Per-conversation sorted timelines of user turns and Dave-initiated reaches.
    have_initiated = "initiated_by_dave" in columns_of(conn, "messages") \
        if table_exists(conn, "messages") else False
    user_turns, reach_turns = {}, {}
    if table_exists(conn, "messages"):
        for cid, created in conn.execute(
            "SELECT conversation_id, created_at FROM messages WHERE role='user' ORDER BY created_at"
        ):
            user_turns.setdefault(cid, []).append(created)
        if have_initiated:
            for cid, created in conn.execute(
                "SELECT conversation_id, created_at FROM messages "
                "WHERE role='assistant' AND initiated_by_dave=1 ORDER BY created_at"
            ):
                reach_turns.setdefault(cid, []).append(created)

    def next_after(seq, t):
        # smallest element strictly greater than t, or None
        for v in seq:          # sequences are short (per-conversation); linear is fine
            if v > t:
                return v
        return None

    episodes = {}
    for (cid, ts, elapsed, presence, tod, dow, shape, thr, decision, timer) in anchors:
        user_last = ts - (elapsed or 0)
        key = (cid, user_last)
        ep = episodes.get(key)
        if ep is None:
            nu = next_after(user_turns.get(cid, []), ts)
            nr = next_after(reach_turns.get(cid, []), ts)
            if nr is not None and (nu is None or nr <= nu):
                event, terminal = True, nr
            elif nu is not None:
                event, terminal = False, nu
            else:
                event, terminal = None, None  # right-open: still unresolved
            ep = {
                "conversation_id": cid, "user_last_spoke_ts": user_last,
                "first_arm_ts": ts, "n_anchors": 0, "max_elapsed": 0,
                "presence": presence, "time_of_day_min": tod, "day_of_week": dow,
                "history_shape": shape, "threshold_seconds": thr,
                "any_timer_reach": False, "any_governed_reach": False,
                "event": event, "terminal_ts": terminal,
                "duration": (terminal - user_last) if terminal is not None else None,
            }
            episodes[key] = ep
        ep["n_anchors"] += 1
        ep["max_elapsed"] = max(ep["max_elapsed"], elapsed or 0)
        ep["any_timer_reach"] |= (timer == "reach")
        ep["any_governed_reach"] |= (decision == "reach")
    return list(episodes.values())


def report_reconstruction(conn, dump_episodes):
    eps = reconstruct_episodes(conn)
    print(hr("V0 readiness — censored-negative reconstruction"))
    if not eps:
        print("  no armed anchors yet — nothing to reconstruct.")
        print("  (this is EXPECTED right after Stage-0 ship; the corpus fills")
        print("   only from real daily use. Re-run after living with Dave.)")
        return {"episodes": 0, "events": 0, "censored": 0, "unresolved": 0}

    events = [e for e in eps if e["event"] is True]
    censored = [e for e in eps if e["event"] is False]
    unresolved = [e for e in eps if e["event"] is None]
    print(f"  arming episodes:        {len(eps)}  "
          f"(from {sum(e['n_anchors'] for e in eps)} armed anchor ticks)")
    print(f"    reach EVENTS:         {len(events)}   {bar(len(events), len(eps))}")
    print(f"    CENSORED (user first):{len(censored)}   {bar(len(censored), len(eps))}")
    print(f"    unresolved (open):    {len(unresolved)}   {bar(len(unresolved), len(eps))}")
    if events:
        durs = sorted(e["duration"] for e in events if e["duration"] is not None)
        if durs:
            print(f"    reach delay (s):      "
                  f"min={durs[0]}  median={durs[len(durs)//2]}  max={durs[-1]}")
    return {
        "episodes": len(eps), "events": len(events),
        "censored": len(censored), "unresolved": len(unresolved),
    }


def report_table_breakdowns(conn):
    # presence_samples
    print(hr("presence_samples — the operator-presence timeline"))
    n = count(conn, "presence_samples")
    if n is None:
        print("  <table missing — DB predates Stage 0>")
    elif n == 0:
        print("  0 rows. The sampler logs only on STATE TRANSITION. If you have")
        print("  been running Dave, unfocus his window and move the mouse in")
        print("  another app for ~30s, then re-run — a 'present_elsewhere' row")
        print("  should appear. If it never does, the sensor is not live.")
    else:
        states = Counter(r[0] for r in conn.execute("SELECT state FROM presence_samples"))
        for s, c in states.most_common():
            print(f"    {s:20} {c:6}  {bar(c, n)}")
        print(f"  span: {fmt_span(ts_range(conn, 'presence_samples'))}")

    # initiation_anchors
    print(hr("initiation_anchors — one row per ARMED tick (past idle threshold)"))
    n = count(conn, "initiation_anchors")
    if n is None:
        print("  <table missing — DB predates Stage 0>")
    elif n == 0:
        print("  0 rows. No tick has passed the idle threshold with a live")
        print("  conversation yet. Fills once Dave sits armed while you're away")
        print("  from his window (3min–1h band; >1h is the journal's domain).")
    else:
        print(f"  {n} armed ticks.  span: {fmt_span(ts_range(conn, 'initiation_anchors'))}")
        gov = Counter(r[0] for r in conn.execute("SELECT decision FROM initiation_anchors"))
        tim = Counter(r[0] for r in conn.execute(
            "SELECT COALESCE(timer_decision,'<null>') FROM initiation_anchors"))
        print("  governed decision (what actually happened):")
        for d, c in gov.most_common():
            print(f"    {d:22} {c:6}  {bar(c, n)}")
        print("  timer proposal (the model's OWN signal — trains V0):")
        for d, c in tim.most_common():
            print(f"    {d:22} {c:6}  {bar(c, n)}")
        # Governor overrides: timer wanted to reach, presence gate said no.
        override = conn.execute(
            "SELECT COUNT(*) FROM initiation_anchors "
            "WHERE timer_decision='reach' AND decision='hold_presence_gate'"
        ).fetchone()[0]
        print(f"  presence-gate overrides (timer=reach, governor=hold): {override}")
        pres = Counter(r[0] for r in conn.execute(
            "SELECT presence_state FROM initiation_anchors"))
        print("  presence at armed ticks:")
        for s, c in pres.most_common():
            print(f"    {s:22} {c:6}  {bar(c, n)}")
        # Blind-A/B + exploration instrumentation (2026-07-09) — soft check:
        # a pre-upgrade DB simply hasn't migrated yet.
        cols = columns_of(conn, "initiation_anchors")
        if "ab_arm" in cols:
            arms = Counter(r[0] for r in conn.execute(
                "SELECT COALESCE(ab_arm,'<pre-A/B>') FROM initiation_anchors"))
            print("  A/B arm (a = heuristic control, b = +exploration floor):")
            for a, c in arms.most_common():
                print(f"    {a:22} {c:6}  {bar(c, n)}")
            explored = conn.execute(
                "SELECT COUNT(*) FROM initiation_anchors WHERE explored=1"
            ).fetchone()[0]
            print(f"  explored (ε-floor fired): {explored}")
            stamped = conn.execute(
                "SELECT COUNT(*) FROM initiation_anchors WHERE reach_message_id IS NOT NULL"
            ).fetchone()[0]
            print(f"  anchors stamped with a delivered reach: {stamped}")
        else:
            print("  (no ab_arm column — run the upgraded app once to migrate)")

    # ratings + counterfactuals
    print(hr("curation — your single-bit judgments (reach_ratings / counterfactuals)"))
    nr = count(conn, "reach_ratings")
    nc = count(conn, "reach_counterfactuals")
    if nr:
        pos = conn.execute("SELECT COUNT(*) FROM reach_ratings WHERE rating>0").fetchone()[0]
        neg = conn.execute("SELECT COUNT(*) FROM reach_ratings WHERE rating<0").fetchone()[0]
        print(f"  reach_ratings: {nr}  (+{pos} felt right / -{neg} felt wrong)")
        # Ratings by arm — the blind-A/B readout (exact join via reach_message_id).
        if "ab_arm" in columns_of(conn, "initiation_anchors"):
            rows = conn.execute(
                """SELECT a.ab_arm, r.rating, COUNT(*)
                   FROM reach_ratings r JOIN initiation_anchors a
                     ON a.reach_message_id = r.message_id
                   GROUP BY a.ab_arm, r.rating"""
            ).fetchall()
            if rows:
                print("  ratings by arm (the A/B readout):")
                for arm, rating, c in rows:
                    label = "felt right" if rating > 0 else "felt wrong"
                    print(f"    arm {arm}: {label:11} {c}")
    else:
        print(f"  reach_ratings: {nr if nr is not None else '<missing>'}  "
              "(rate reaches with Ctrl+Alt+↑/↓ as they land)")
    if nc and "kind" in columns_of(conn, "reach_counterfactuals"):
        kinds = Counter(r[0] for r in conn.execute(
            "SELECT COALESCE(kind,'missed_reach') FROM reach_counterfactuals"))
        missed = kinds.get("missed_reach", 0)
        blessed = kinds.get("good_silence", 0)
        print(f"  silences judged: {missed} 'should have reached' (Ctrl+Alt+M) / "
              f"{blessed} 'the quiet was right' (Ctrl+Alt+S)")
    else:
        print(f"  reach_counterfactuals ('should have reached here'): "
              f"{nc if nc is not None else '<missing>'}")

    # outreach_drops (context — the substrate-fight forensics)
    print(hr("outreach_drops — rejected candidates (substrate-fight forensics)"))
    nd = count(conn, "outreach_drops")
    if nd:
        reasons = Counter(r[0] for r in conn.execute(
            "SELECT drop_reason FROM outreach_drops"))
        print(f"  {nd} drops.  span: {fmt_span(ts_range(conn, 'outreach_drops', 'generated_at'))}")
        for d, c in reasons.most_common(12):
            print(f"    {d:26} {c:6}  {bar(c, nd)}")
    else:
        print(f"  {nd if nd is not None else '<missing>'} drops.")


def report_rhythm_and_recall(conn):
    # Inter-reach interval CV — the timing-sycophancy tripwire (§3c). Human
    # initiation is bursty (CV well above 1 is normal); a cron is CV≈0. Any
    # FUTURE learned timer whose offline CV drops below ~0.6 is rejected
    # before deploy. Computed over delivered reaches.
    print(hr("initiation rhythm — inter-reach interval CV (sycophancy tripwire)"))
    if table_exists(conn, "messages") and "initiated_by_dave" in columns_of(conn, "messages"):
        ts = [r[0] for r in conn.execute(
            "SELECT created_at FROM messages WHERE initiated_by_dave=1 ORDER BY created_at")]
        if len(ts) < 3:
            print(f"  {len(ts)} delivered reach(es) — need ≥3 for a meaningful CV.")
        else:
            gaps = [b - a for a, b in zip(ts, ts[1:]) if b > a]
            mean = sum(gaps) / len(gaps)
            var = sum((g - mean) ** 2 for g in gaps) / len(gaps)
            cv = (var ** 0.5) / mean if mean > 0 else 0.0
            verdict = "bursty (healthy)" if cv >= 0.6 else "TOO REGULAR — cron-like, investigate"
            print(f"  {len(ts)} reaches, {len(gaps)} intervals: CV = {cv:.2f}  → {verdict}")
    else:
        print("  <messages table not instrumented>")

    # Ring-4 recall observability (2026-07-09): is recall firing, how often,
    # how big. Silence here after real use = the gate may be too strict;
    # constant firing = too loose (and a prompt-cache tax).
    print(hr("recall_fires — Ring-4 retrieval observability"))
    nf = count(conn, "recall_fires")
    if nf is None:
        print("  <table missing — DB predates Ring 4>")
    elif nf == 0:
        print("  0 fires. Expected until a turn carries a remember-cue or a rare")
        print("  term that hits the Tape. Test: ask Dave about something specific")
        print("  from far back, then re-run.")
    else:
        print(f"  {nf} fires.  span: {fmt_span(ts_range(conn, 'recall_fires'))}")
        avg_tok = conn.execute("SELECT AVG(injected_tokens) FROM recall_fires").fetchone()[0]
        avg_hits = conn.execute("SELECT AVG(hit_count) FROM recall_fires").fetchone()[0]
        print(f"  avg excerpts/fire: {avg_hits:.1f}   avg injected tokens: {avg_tok:.0f}")
        print("  last 5 fires (terms → excerpts):")
        for terms, hits in conn.execute(
            "SELECT query_terms, hit_count FROM recall_fires ORDER BY id DESC LIMIT 5"):
            print(f"    [{terms}] → {hits}")


def report_intentions(conn):
    # Arm C ask-channel telemetry (A8 R6): parse-fail→"nothing" is invisible
    # by design, so this is the instrument that distinguishes "Dave rarely
    # forms intentions" from "the ask channel is dead."
    print(hr("reach_intentions — Dave's stated return-times (arm C ask channel)"))
    n = count(conn, "reach_intentions")
    if n is None:
        print("  <table missing — DB predates arm C>")
        return
    if n == 0:
        print("  0 asks recorded. Fills once exchanges end with the upgraded app.")
        return
    timed = conn.execute(
        "SELECT COUNT(*) FROM reach_intentions WHERE fire_at IS NOT NULL").fetchone()[0]
    print(f"  {n} asks: {timed} named a time ({100*timed/n:.0f}%), {n-timed} said nothing")
    for col, label in [("consumed_at", "consumed (fired a reach)"),
                       ("cancelled_at", "cancelled (user spoke first)"),
                       ("expired_at", "expired (window passed unacted)")]:
        c = conn.execute(
            f"SELECT COUNT(*) FROM reach_intentions WHERE {col} IS NOT NULL").fetchone()[0]
        print(f"    {label:32} {c}")
    if timed:
        # Stated-delay + round-number tripwire: an LLM parroting round times
        # shows up as minute-marks piling on :00/:15/:30/:45.
        rows = conn.execute(
            "SELECT created_at, fire_at FROM reach_intentions WHERE fire_at IS NOT NULL").fetchall()
        delays = sorted((f - c) // 60 for c, f in rows if f > c)
        if delays:
            print(f"  stated delay (min): min={delays[0]} median={delays[len(delays)//2]} max={delays[-1]}")
        import sqlite3 as _s  # noqa
        round_marks = conn.execute(
            "SELECT COUNT(*) FROM reach_intentions WHERE fire_at IS NOT NULL "
            "AND CAST(strftime('%M', fire_at, 'unixepoch') AS INTEGER) % 15 = 0").fetchone()[0]
        print(f"  quarter-hour marks: {round_marks}/{timed} "
              f"({'suspiciously round' if timed >= 8 and round_marks/timed > 0.7 else 'ok at this N'})")
        print("  last 5 raw replies:")
        for (raw,) in conn.execute(
            "SELECT raw_reply FROM reach_intentions ORDER BY id DESC LIMIT 5"):
            print(f"    {raw[:70]!r}")


def report_messages(conn):
    print(hr("messages — the conversation backdrop"))
    n = count(conn, "messages")
    if not n:
        print(f"  {n if n is not None else '<missing>'} messages.")
        return
    role = Counter(r[0] for r in conn.execute("SELECT role FROM messages"))
    reaches = 0
    if "initiated_by_dave" in columns_of(conn, "messages"):
        reaches = conn.execute(
            "SELECT COUNT(*) FROM messages WHERE initiated_by_dave=1").fetchone()[0]
    convs = conn.execute("SELECT COUNT(DISTINCT conversation_id) FROM messages").fetchone()[0]
    print(f"  {n} messages across {convs} conversation(s):")
    for r, c in role.most_common():
        print(f"    {r:12} {c:6}")
    print(f"  Dave-initiated reaches delivered: {reaches}")
    print(f"  span: {fmt_span(ts_range(conn, 'messages', 'created_at'))}")


def verdict(recon):
    print(hr("READINESS"))
    ep, ev, ce = recon["episodes"], recon["events"], recon["censored"]
    ok_ep = ep >= V0_MIN_EPISODES
    ok_ev = ev >= V0_MIN_EVENTS
    ok_ce = ce >= V0_MIN_CENSORED
    def mark(b): return "✓" if b else "·"
    print(f"  {mark(ok_ep)} episodes  {ep:>5} / {V0_MIN_EPISODES}")
    print(f"  {mark(ok_ev)} reaches   {ev:>5} / {V0_MIN_EVENTS}")
    print(f"  {mark(ok_ce)} censored  {ce:>5} / {V0_MIN_CENSORED}")
    if ok_ep and ok_ev and ok_ce:
        print("\n  → Enough signal to attempt a V0 log-normal-hazard fit.")
        print("    Build the blind-A/B harness FIRST (the falsifier), then fit V0")
        print("    behind TimingModel. See continuation doc §3a.")
    else:
        print("\n  → Not yet. The corpus is still filling. Keep living with Dave;")
        print("    re-run this to watch it accumulate. Nothing to code on the")
        print("    timer until the signal is here — the wait is the design.")


def inspect(db_path, dump_episodes=False):
    if not os.path.exists(db_path):
        print(f"DB not found: {db_path}", file=sys.stderr)
        return 2
    size = os.path.getsize(db_path)
    mtime = datetime.fromtimestamp(os.path.getmtime(db_path)).strftime("%Y-%m-%d %H:%M")
    print(f"Dave corpus @ {db_path}")
    print(f"  {size:,} bytes · last written {mtime}")

    conn = sqlite3.connect(f"file:{db_path}?mode=ro", uri=True)
    try:
        # Schema-drift guard: any present corpus table must have its expected columns.
        drift = []
        for t, expected in EXPECTED_COLUMNS.items():
            if table_exists(conn, t):
                have = columns_of(conn, t)
                missing = expected - have
                if missing:
                    drift.append((t, sorted(missing)))
        if drift:
            print("\nSCHEMA DRIFT — offline analysis expects columns not present:")
            for t, cols in drift:
                print(f"  {t}: missing {cols}")
            print("Update EXPECTED_COLUMNS and the loader before trusting output.")
            return 2

        report_messages(conn)
        report_table_breakdowns(conn)
        report_rhythm_and_recall(conn)
        report_intentions(conn)
        recon = report_reconstruction(conn, dump_episodes)
        if dump_episodes:
            print(hr("per-episode rows"))
            for e in reconstruct_episodes(conn):
                out = "reach" if e["event"] else ("cens" if e["event"] is False else "open")
                print(f"  conv={e['conversation_id']} start={fmt_ts(e['user_last_spoke_ts'])} "
                      f"n_anchors={e['n_anchors']} max_elapsed={e['max_elapsed']}s "
                      f"-> {out} dur={e['duration']}")
        verdict(recon)
        return 0
    finally:
        conn.close()


# ─────────────────────────────────────────────────────────────────────────
# Self-test — deterministic, no live data. Builds a synthetic DB whose episode
# structure has a known answer and asserts the reconstruction recovers it.
# ─────────────────────────────────────────────────────────────────────────
SELFTEST_SCHEMA = """
CREATE TABLE messages (id INTEGER PRIMARY KEY, conversation_id INTEGER, role TEXT,
    content TEXT, created_at INTEGER, initiated_by_dave INTEGER DEFAULT 0);
CREATE TABLE presence_samples (id INTEGER PRIMARY KEY, ts INTEGER, state TEXT,
    os_idle_ms INTEGER, focused INTEGER);
CREATE TABLE initiation_anchors (id INTEGER PRIMARY KEY, ts INTEGER,
    conversation_id INTEGER, seconds_since_user_input INTEGER, presence_state TEXT,
    focused INTEGER, os_idle_ms INTEGER, history_shape TEXT, unanswered_reaches INTEGER,
    consecutive_drops INTEGER, threshold_seconds INTEGER, time_of_day_min INTEGER,
    day_of_week INTEGER, decision TEXT, timer_decision TEXT);
CREATE TABLE reach_ratings (message_id INTEGER PRIMARY KEY, rating INTEGER, created_at INTEGER);
CREATE TABLE reach_counterfactuals (id INTEGER PRIMARY KEY, conversation_id INTEGER,
    at_message_id INTEGER, created_at INTEGER);
CREATE TABLE outreach_drops (id INTEGER PRIMARY KEY, conversation_id INTEGER,
    generated_at INTEGER, content TEXT, drop_reason TEXT, heuristic_pass INTEGER,
    llm_score INTEGER, history_shape TEXT, last_user_input INTEGER);
"""


def _selftest():
    conn = sqlite3.connect(":memory:")
    conn.executescript(SELFTEST_SCHEMA)
    c = conn.cursor()

    def anchor(cid, ts, elapsed, decision, timer):
        c.execute("INSERT INTO initiation_anchors (ts, conversation_id, "
                  "seconds_since_user_input, presence_state, focused, os_idle_ms, "
                  "history_shape, unanswered_reaches, consecutive_drops, threshold_seconds, "
                  "time_of_day_min, day_of_week, decision, timer_decision) "
                  "VALUES (?,?,?,?,0,1000,'user_statement',0,0,180,600,2,?,?)",
                  (ts, cid, elapsed, "present_elsewhere", decision, timer))

    def msg(cid, ts, role, initiated=0):
        c.execute("INSERT INTO messages (conversation_id, role, content, created_at, "
                  "initiated_by_dave) VALUES (?,?,?,?,?)", (cid, role, "x", ts, initiated))

    # Episode 1 (conv 1): user@1000; armed @1200(e=200,hold) & @1400(e=400,reach);
    #   Dave reaches @1400 -> EVENT, duration 400.
    msg(1, 1000, "user")
    anchor(1, 1200, 200, "hold_presence_gate", "hold_adaptive_backoff")
    anchor(1, 1400, 400, "reach", "reach")
    msg(1, 1400, "assistant", initiated=1)

    # Episode 2 (conv 2): user@2000; armed @2200(e=200); user speaks again @2300
    #   before any reach -> CENSORED, duration 300.
    msg(2, 2000, "user")
    anchor(2, 2200, 200, "hold_presence_gate", "reach")
    msg(2, 2300, "user")

    # Episode 3 (conv 3): user@3000; armed @3200(e=200); nothing after -> UNRESOLVED.
    msg(3, 3000, "user")
    anchor(3, 3200, 200, "hold_presence_gate", "hold_max_unanswered")

    conn.commit()
    eps = {(e["conversation_id"]): e for e in reconstruct_episodes(conn)}
    assert len(eps) == 3, f"expected 3 episodes, got {len(eps)}"
    assert eps[1]["event"] is True and eps[1]["duration"] == 400, eps[1]
    assert eps[1]["n_anchors"] == 2, eps[1]
    assert eps[2]["event"] is False and eps[2]["duration"] == 300, eps[2]
    assert eps[3]["event"] is None and eps[3]["duration"] is None, eps[3]
    # timer-vs-governor separation preserved
    assert eps[2]["any_timer_reach"] is True and eps[2]["any_governed_reach"] is False, eps[2]
    print("selftest OK: reconstruction recovers 1 event / 1 censored / 1 unresolved,")
    print("             episode collapsing and timer/governor separation correct.")
    return 0


def main():
    # Windows consoles default to cp1252; the box-drawing/bar glyphs below would
    # raise UnicodeEncodeError. Reconfigure to UTF-8 (renders in Windows Terminal,
    # degrades gracefully elsewhere) rather than dumbing the output down to ASCII.
    try:
        sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    except Exception:
        pass

    ap = argparse.ArgumentParser(description="Inspect Dave's initiation-timing corpus.")
    ap.add_argument("--db", help="path to a dave.db (default: release DB)")
    ap.add_argument("--debug-db", action="store_true", help=f"use the debug DB at {DEBUG_DB}")
    ap.add_argument("--episodes", action="store_true", help="dump per-episode reconstruction rows")
    ap.add_argument("--selftest", action="store_true", help="run the synthetic-fixture self-test")
    args = ap.parse_args()

    if args.selftest:
        try:
            return _selftest()
        except AssertionError as e:
            print(f"SELFTEST FAILED: {e}", file=sys.stderr)
            return 1

    db = args.db or (DEBUG_DB if args.debug_db else RELEASE_DB)
    return inspect(db, dump_episodes=args.episodes)


if __name__ == "__main__":
    sys.exit(main())
