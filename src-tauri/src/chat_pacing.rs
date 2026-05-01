// Chat pacing — cadence-aware dynamic timing for Dave's responses.
//
// Architectural intent (Bo's directive 2026-04-30):
//   Dave's response visually decomposes into two phases after the second
//   read-checkmark fires:
//     T_compose   — TypingIndicator dots only, "Dave is composing in his head"
//     T_streaming — chars appearing one by one, "Dave is typing"
//   T_total = T_compose + T_streaming.
//
//   The TypingIndicator ALWAYS precedes any chars on a Respond. T_compose is
//   never zero (Dave doesn't reply at network latency).
//
//   Total time scales with cadence (rapid chitchat → fast; substantive →
//   slow) and with response length (longer responses take longer to type).
//
// Three knobs: cadence, length, and the compose-to-streaming ratio.
//
// 1. Cadence score (0.0 = slow, 1.0 = rapid):
//    Computed from average gap between the last 4-6 messages' timestamps.
//    ≤30s avg gap → 1.0 (rapid). ≥120s avg gap → 0.0 (slow). Linear interp.
//
// 2. Typing speed (chars per second):
//    Linear blend by cadence: SLOW=8 chars/sec (~50 wpm thoughtful) →
//    FAST=25 chars/sec (~150 wpm rapid). T_total = N / typing_speed.
//
// 3. Compose ratio (T_compose / T_total):
//    For short responses, indicator visible time ≈ streaming time (ratio=0.5).
//    For long responses, ratio asymptotes down to 0.1 so we don't watch
//    dots forever. Curve: ratio(N <= 150) = 0.5; ratio(N > 150) = 0.1 +
//    0.4 * 150/N (asymptotes to 0.1).
//
// Per-char streaming timing:
//   Average char delay = T_streaming_ms / N. Each emitted char's actual
//   delay = avg * (0.5 + random()) → varies 50-150% of average for
//   non-robotic feel. Punctuation pauses scale: 2× at clause, 5× at
//   sentence, 10× at paragraph.
//
// Worked examples (verifying spec):
//   "hi" → "yeah" (4 chars, fast cadence)    : T_total 0.16s  (compose 0.08, stream 0.08)
//   substantive Q → 300 chars (slow cadence) : T_total 37.5s  (compose 11s, stream 26s)
//   essay → 1000 chars (slow cadence)        : T_total 125s   (compose 20s, stream 105s)

use rand::{thread_rng, Rng};

use crate::persistence::Message;

// ---- Cadence tunables ----

/// Avg gap (seconds) between recent messages at or below which we consider
/// the conversation "rapid chitchat." Cadence score = 1.0 here.
const CADENCE_RAPID_GAP_SEC: f32 = 30.0;

/// Avg gap at or above which we consider the conversation "slow / spread
/// out." Cadence score = 0.0 here.
const CADENCE_SLOW_GAP_SEC: f32 = 120.0;

/// How many recent messages to sample for cadence computation. Need a few
/// to get a meaningful avg gap.
const CADENCE_WINDOW_MSGS: usize = 6;

/// Default cadence when we don't have enough history (early in conversation).
/// Mid-tempo — not snappy, not slow.
const CADENCE_DEFAULT_SCORE: f32 = 0.5;

// ---- Typing speed tunables ----
//
// Note: the runtime "pace" multiplier (settings key: chat_pacing_pace,
// default 1.0) further scales these values via t_total. The defaults below
// are the "1.0x baseline." Bo can dial pace down to 0.2x for snappier
// experience or up to 2.0x for more deliberate without recompiling.
//
// Defaults retuned 2026-04-30 after Bo flagged 25 cps still felt too slow
// in practice — even at rapid cadence, a 300-char response was hitting
// ~12s which read as "did the app crash?" Bumped to 40/12 cps so the
// baseline feels snappy and Bo can use the slider for fine-grain tuning.

/// Slowest typing speed (chars/sec) — used when cadence_score = 0.0
/// (substantive, deliberate exchange). ~70 wpm equivalent.
const TYPING_SPEED_SLOW: f32 = 12.0;

/// Fastest typing speed (chars/sec) — used when cadence_score = 1.0 (rapid
/// chitchat). ~240 wpm equivalent — faster than any real human, but it
/// feels right for snappy chat UI without breaking immersion entirely.
const TYPING_SPEED_FAST: f32 = 40.0;

// ---- Compose ratio tunables ----

/// Below this length, ratio is fixed at COMPOSE_RATIO_SHORT.
const COMPOSE_RATIO_PIVOT_CHARS: usize = 150;

/// Compose ratio for short responses (<= pivot chars). Bo's directive: ~0.5.
const COMPOSE_RATIO_SHORT: f32 = 0.5;

/// Asymptotic compose ratio for very long responses. Bo's directive: ~0.10.
const COMPOSE_RATIO_ASYMPTOTE: f32 = 0.10;

/// Span between asymptote and short ratio. ratio(N) for N > pivot is
/// COMPOSE_RATIO_ASYMPTOTE + COMPOSE_RATIO_SPAN * pivot / N.
const COMPOSE_RATIO_SPAN: f32 = COMPOSE_RATIO_SHORT - COMPOSE_RATIO_ASYMPTOTE;

// ---- Per-char timing tunables ----

/// Variance ratio for per-char delays. 0.5 = each char's delay is in
/// [0.5*avg, 1.5*avg] uniformly. Provides the "not robotic" feel.
const CHAR_VAR_RATIO: f32 = 0.5;

/// Multipliers on top of base char delay for punctuation pauses.
const CLAUSE_PAUSE_MULT: u64 = 2;
const SENTENCE_PAUSE_MULT: u64 = 5;
const PARAGRAPH_PAUSE_MULT: u64 = 10;

/// Floor and cap on T_total so pathological lengths don't break the UX.
/// Floor: even a 1-char response gets a small minimum total (avoids 0ms).
/// Cap: a 5000-char response capped at ~3 minutes; better to lose realism
/// than have the user wait forever.
const T_TOTAL_FLOOR_MS: u64 = 200;
const T_TOTAL_CAP_MS: u64 = 180_000;

// ---- Read delay tunables ----

/// Read delay (pre-triage / before second checkmark) is also cadence-aware.
/// Rapid chitchat → near-instant read. Slow exchange → full read time.
const READ_DELAY_FLOOR_MS: u64 = 300;
const READ_DELAY_CAP_MS: u64 = 3_500;
const READ_PER_CHAR_MS_FAST: f32 = 2.0; // rapid cadence
const READ_PER_CHAR_MS_SLOW: f32 = 8.0; // slow cadence

#[derive(Debug, Clone)]
pub struct ChatPacing {
    pub typing_speed_chars_per_sec: f32,
    pub cadence_score: f32,
    pub response_chars: usize,
    pub t_total_ms: u64,
    pub compose_ratio: f32,
    pub compose_hold_ms: u64,
    pub t_streaming_ms: u64,
    pub char_base_ms: u64,
    pub clause_extra_ms: u64,
    pub sentence_extra_ms: u64,
    pub paragraph_extra_ms: u64,
}

impl ChatPacing {
    /// Sample a per-char delay for the given char + previous char. Includes
    /// random variance and punctuation pauses. Pure function over thread_rng.
    pub fn delay_for(&self, ch: char, prev: Option<char>) -> u64 {
        let mut rng = thread_rng();
        let var = CHAR_VAR_RATIO; // ±50% by default
        let factor = 1.0 - var + rng.gen::<f32>() * (2.0 * var);
        let base = (self.char_base_ms as f32 * factor) as u64;
        let extra = match (ch, prev) {
            // Paragraph break: \n preceded by \n.
            ('\n', Some('\n')) => self.paragraph_extra_ms,
            // Sentence end: char after . ! ?.
            (_, Some('.')) | (_, Some('!')) | (_, Some('?')) => self.sentence_extra_ms,
            // Clause: char after , ; :.
            (_, Some(',')) | (_, Some(';')) | (_, Some(':')) => self.clause_extra_ms,
            _ => 0,
        };
        base + extra
    }
}

/// Compute the cadence score in [0.0, 1.0] from recent message timestamps.
/// 0.0 = slow / spread out. 1.0 = rapid chitchat. Defaults to mid (0.5)
/// when there isn't enough history.
pub fn compute_cadence_score(recent_msgs: &[Message]) -> f32 {
    let n = recent_msgs.len();
    if n < 3 {
        return CADENCE_DEFAULT_SCORE;
    }
    let take = CADENCE_WINDOW_MSGS.min(n);
    let tail = &recent_msgs[n - take..];
    // Compute consecutive gaps.
    let mut gaps_sum: i64 = 0;
    let mut gap_count: i64 = 0;
    for w in tail.windows(2) {
        let g = (w[1].created_at - w[0].created_at).max(0);
        gaps_sum += g;
        gap_count += 1;
    }
    if gap_count == 0 {
        return CADENCE_DEFAULT_SCORE;
    }
    let avg_gap = gaps_sum as f32 / gap_count as f32;
    // Map: <=RAPID → 1.0, >=SLOW → 0.0, linear in between.
    if avg_gap <= CADENCE_RAPID_GAP_SEC {
        1.0
    } else if avg_gap >= CADENCE_SLOW_GAP_SEC {
        0.0
    } else {
        // Linear interp: at RAPID → 1, at SLOW → 0.
        1.0 - (avg_gap - CADENCE_RAPID_GAP_SEC) / (CADENCE_SLOW_GAP_SEC - CADENCE_RAPID_GAP_SEC)
    }
}

/// Compute the compose ratio (T_compose / T_total) for a response of N chars.
/// Short responses: 0.5. Long responses: asymptotes to 0.1.
pub fn compute_compose_ratio(response_chars: usize) -> f32 {
    if response_chars <= COMPOSE_RATIO_PIVOT_CHARS {
        COMPOSE_RATIO_SHORT
    } else {
        COMPOSE_RATIO_ASYMPTOTE
            + COMPOSE_RATIO_SPAN * (COMPOSE_RATIO_PIVOT_CHARS as f32 / response_chars as f32)
    }
}

/// Compute typing speed (chars/sec) from cadence score. Linear blend
/// between SLOW and FAST.
pub fn compute_typing_speed(cadence_score: f32) -> f32 {
    let c = cadence_score.clamp(0.0, 1.0);
    TYPING_SPEED_SLOW + c * (TYPING_SPEED_FAST - TYPING_SPEED_SLOW)
}

/// Pace settings key (read at runtime via settings table). Multiplier on
/// every timing value: t_total, t_compose, t_streaming, char_base,
/// punctuation pauses, AND read delay all scale by this factor. Default
/// 1.0. Bo can dial 0.2x (snappy) → 2.0x (deliberate) via SettingsPanel.
pub const SETTING_KEY_PACE: &str = "chat_pacing_pace";

/// Default tempo factor when the setting is missing or unparseable. Was
/// 1.0 originally; locked in to 0.65 on 2026-05-01 after Bo dialed in via
/// the slider and asked to "lock these settings in." 0.65 produces snappy
/// chitchat (sub-second for short replies) while keeping enough beat for
/// substantive content.
pub const PACE_DEFAULT: f32 = 0.65;

/// Pace clamps. Below 0.1 the experience becomes "instant streaming with
/// no perceivable beat" — fine for testing but defeats the design. Above
/// 3.0 the experience becomes "watching paint dry" even for short replies.
/// Slider stays inside [PACE_MIN, PACE_MAX].
pub const PACE_MIN: f32 = 0.2;
pub const PACE_MAX: f32 = 2.0;

/// Clamp + sanitize a raw pace value (e.g. parsed from settings string).
pub fn clamp_pace(raw: f32) -> f32 {
    if raw.is_finite() {
        raw.clamp(PACE_MIN, PACE_MAX)
    } else {
        PACE_DEFAULT
    }
}

/// Main entrypoint. Compute the full ChatPacing config for a response of
/// N chars given the recent message history. The `pace` factor (typically
/// from the SETTING_KEY_PACE setting) scales every output timing
/// proportionally — 0.5 halves all delays, 2.0 doubles them.
pub fn compute_chat_pacing(
    response_chars: usize,
    recent_msgs: &[Message],
    pace: f32,
) -> ChatPacing {
    let pace = clamp_pace(pace);
    let cadence_score = compute_cadence_score(recent_msgs);
    let typing_speed = compute_typing_speed(cadence_score);

    // T_total: the realistic time to type out N chars at this cadence,
    // multiplied by the global pace factor.
    let n = response_chars.max(1) as f32;
    let raw_total_ms = (n * 1000.0 / typing_speed * pace) as u64;
    let t_total_ms = raw_total_ms.clamp(T_TOTAL_FLOOR_MS, T_TOTAL_CAP_MS);

    let compose_ratio = compute_compose_ratio(response_chars);
    let compose_hold_ms = (t_total_ms as f32 * compose_ratio) as u64;
    let t_streaming_ms = t_total_ms.saturating_sub(compose_hold_ms);

    // Per-char base: average delay between consecutive char emissions.
    // Floor at 5ms so we don't busy-spin on degenerate cases.
    let char_base_ms = if response_chars == 0 {
        0
    } else {
        (t_streaming_ms / response_chars as u64).max(5)
    };

    let clause_extra_ms = char_base_ms * CLAUSE_PAUSE_MULT;
    let sentence_extra_ms = char_base_ms * SENTENCE_PAUSE_MULT;
    let paragraph_extra_ms = char_base_ms * PARAGRAPH_PAUSE_MULT;

    ChatPacing {
        typing_speed_chars_per_sec: typing_speed,
        cadence_score,
        response_chars,
        t_total_ms,
        compose_ratio,
        compose_hold_ms,
        t_streaming_ms,
        char_base_ms,
        clause_extra_ms,
        sentence_extra_ms,
        paragraph_extra_ms,
    }
}

/// Cadence-aware read delay (pre-second-checkmark). Rapid cadence →
/// near-instant read. Slow cadence → full read time. Length-proportional.
/// Also scaled by the global pace factor.
pub fn compute_read_delay_ms(user_text: &str, recent_msgs: &[Message], pace: f32) -> u64 {
    let pace = clamp_pace(pace);
    let cadence_score = compute_cadence_score(recent_msgs);
    let chars = user_text.chars().count() as f32;
    // Per-char ms blends: fast cadence uses small per-char, slow uses larger.
    let per_char_ms = READ_PER_CHAR_MS_FAST
        + (1.0 - cadence_score) * (READ_PER_CHAR_MS_SLOW - READ_PER_CHAR_MS_FAST);
    let raw = (READ_DELAY_FLOOR_MS as f32 + chars * per_char_ms) * pace;
    (raw as u64).clamp(READ_DELAY_FLOOR_MS, READ_DELAY_CAP_MS)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(ts: i64) -> Message {
        Message {
            id: 0,
            conversation_id: 0,
            role: "user".to_string(),
            content: "x".to_string(),
            created_at: ts,
        }
    }

    #[test]
    fn cadence_rapid_chitchat_scores_one() {
        // Six messages each 10s apart — clearly rapid.
        let msgs: Vec<_> = (0..6).map(|i| msg(i * 10)).collect();
        let s = compute_cadence_score(&msgs);
        assert!((s - 1.0).abs() < 0.01, "expected 1.0, got {}", s);
    }

    #[test]
    fn cadence_slow_exchange_scores_zero() {
        // Six messages each 200s apart — clearly slow.
        let msgs: Vec<_> = (0..6).map(|i| msg(i * 200)).collect();
        let s = compute_cadence_score(&msgs);
        assert!(s < 0.01, "expected 0.0, got {}", s);
    }

    #[test]
    fn cadence_mid_pace_scores_near_half() {
        // Avg gap = 75s → halfway between 30 and 120 → score ~0.5.
        let msgs: Vec<_> = (0..6).map(|i| msg(i * 75)).collect();
        let s = compute_cadence_score(&msgs);
        assert!((s - 0.5).abs() < 0.05, "expected ~0.5, got {}", s);
    }

    #[test]
    fn cadence_default_when_too_short() {
        let s = compute_cadence_score(&[msg(0), msg(10)]);
        assert!((s - CADENCE_DEFAULT_SCORE).abs() < 0.01);
    }

    #[test]
    fn typing_speed_blends_correctly() {
        assert!((compute_typing_speed(0.0) - TYPING_SPEED_SLOW).abs() < 0.01);
        assert!((compute_typing_speed(1.0) - TYPING_SPEED_FAST).abs() < 0.01);
        let mid = compute_typing_speed(0.5);
        let expected_mid = (TYPING_SPEED_SLOW + TYPING_SPEED_FAST) / 2.0;
        assert!((mid - expected_mid).abs() < 0.01);
    }

    #[test]
    fn compose_ratio_short_is_half() {
        assert_eq!(compute_compose_ratio(0), 0.5);
        assert_eq!(compute_compose_ratio(50), 0.5);
        assert_eq!(compute_compose_ratio(150), 0.5);
    }

    #[test]
    fn compose_ratio_decays_for_long() {
        let r300 = compute_compose_ratio(300);
        let r600 = compute_compose_ratio(600);
        let r1500 = compute_compose_ratio(1500);
        // Each successive doubling should reduce the ratio.
        assert!(r300 < 0.5);
        assert!(r600 < r300);
        assert!(r1500 < r600);
        // Asymptote toward 0.1.
        assert!(r1500 > COMPOSE_RATIO_ASYMPTOTE);
        assert!(r1500 < 0.2);
    }

    #[test]
    fn compose_ratio_asymptotes_at_ten_percent() {
        let r_huge = compute_compose_ratio(1_000_000);
        // Effectively 0.1 + tiny epsilon.
        assert!((r_huge - COMPOSE_RATIO_ASYMPTOTE).abs() < 0.001);
    }

    #[test]
    fn hi_to_yeah_is_near_instant() {
        // Bo's spec: "hi" → "yeah" should be near-instant after second
        // check. Cadence rapid (just exchanged short messages).
        let msgs: Vec<_> = (0..6).map(|i| msg(i * 10)).collect();
        let pacing = compute_chat_pacing(4, &msgs, PACE_DEFAULT);
        // T_total should be small.
        assert!(pacing.t_total_ms <= 500, "t_total too large: {}", pacing.t_total_ms);
        // Compose hold should be ~half of total.
        let expected_compose = pacing.t_total_ms / 2;
        let diff = (pacing.compose_hold_ms as i64 - expected_compose as i64).abs();
        assert!(diff <= 5, "compose not ~half: {} vs {}", pacing.compose_hold_ms, expected_compose);
    }

    #[test]
    fn long_response_compose_is_smaller_fraction() {
        let slow_msgs: Vec<_> = (0..6).map(|i| msg(i * 200)).collect();
        let pacing = compute_chat_pacing(1000, &slow_msgs, PACE_DEFAULT);
        // Compose should be smaller than streaming for long responses.
        assert!(pacing.compose_hold_ms < pacing.t_streaming_ms);
        // Specifically, ratio < 0.2 for N=1000.
        assert!(pacing.compose_ratio < 0.2);
    }

    #[test]
    fn pacing_t_total_caps_at_max() {
        let pacing = compute_chat_pacing(100_000, &[], PACE_DEFAULT);
        assert_eq!(pacing.t_total_ms, T_TOTAL_CAP_MS);
    }

    #[test]
    fn read_delay_rapid_cadence_short() {
        // Rapid chitchat + short message → near floor delay.
        let msgs: Vec<_> = (0..6).map(|i| msg(i * 10)).collect();
        let d = compute_read_delay_ms("hi", &msgs, PACE_DEFAULT);
        assert!(d <= READ_DELAY_FLOOR_MS + 50, "expected near floor, got {}", d);
    }

    #[test]
    fn read_delay_slow_cadence_long() {
        // Slow exchange + long message → larger delay.
        let msgs: Vec<_> = (0..6).map(|i| msg(i * 200)).collect();
        let long_msg = "x".repeat(300);
        let d = compute_read_delay_ms(&long_msg, &msgs, PACE_DEFAULT);
        // 300 * 8ms = 2400ms + floor 300 = 2700ms
        assert!(d > 1500, "expected substantial read delay, got {}", d);
    }

    #[test]
    fn pace_factor_scales_t_total_proportionally() {
        let msgs: Vec<_> = (0..6).map(|i| msg(i * 10)).collect();
        let baseline = compute_chat_pacing(200, &msgs, 1.0);
        let half = compute_chat_pacing(200, &msgs, 0.5);
        let double = compute_chat_pacing(200, &msgs, 2.0);
        // Half should be ~half of baseline.
        let half_diff = (baseline.t_total_ms as i64 / 2 - half.t_total_ms as i64).abs();
        assert!(half_diff <= 5, "half pace not ~half: {} vs {}", half.t_total_ms, baseline.t_total_ms);
        // Double should be ~double of baseline (subject to clamps).
        assert!(double.t_total_ms > baseline.t_total_ms);
    }

    #[test]
    fn pace_factor_clamps_to_safe_range() {
        let msgs: Vec<_> = (0..6).map(|i| msg(i * 10)).collect();
        // Below min → clamped to min.
        let way_low = compute_chat_pacing(200, &msgs, 0.001);
        let at_min = compute_chat_pacing(200, &msgs, PACE_MIN);
        assert_eq!(way_low.t_total_ms, at_min.t_total_ms);
        // Above max → clamped to max.
        let way_high = compute_chat_pacing(200, &msgs, 100.0);
        let at_max = compute_chat_pacing(200, &msgs, PACE_MAX);
        assert_eq!(way_high.t_total_ms, at_max.t_total_ms);
    }

    #[test]
    fn pace_factor_scales_read_delay() {
        let msgs: Vec<_> = (0..6).map(|i| msg(i * 100)).collect();
        let long_msg = "x".repeat(200);
        let baseline = compute_read_delay_ms(&long_msg, &msgs, 1.0);
        let half = compute_read_delay_ms(&long_msg, &msgs, 0.5);
        // Half should be smaller (subject to floor).
        assert!(half <= baseline);
    }

    #[test]
    fn delay_for_punctuation_extra() {
        let pacing = ChatPacing {
            typing_speed_chars_per_sec: 10.0,
            cadence_score: 0.5,
            response_chars: 100,
            t_total_ms: 10_000,
            compose_ratio: 0.5,
            compose_hold_ms: 5_000,
            t_streaming_ms: 5_000,
            char_base_ms: 50,
            clause_extra_ms: 100,
            sentence_extra_ms: 250,
            paragraph_extra_ms: 500,
        };
        // Plain char-after-char: in [25, 75] (50 ± 50%).
        let d_plain = pacing.delay_for('a', Some('b'));
        assert!(d_plain >= 25 && d_plain <= 75, "plain delay out of range: {}", d_plain);
        // After period: should include sentence_extra. So min = 25 + 250 = 275.
        let d_post_sentence = pacing.delay_for('B', Some('.'));
        assert!(d_post_sentence >= 275, "post-sentence too short: {}", d_post_sentence);
    }
}
