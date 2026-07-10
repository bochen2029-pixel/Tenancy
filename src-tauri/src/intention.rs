// Arm C — the intention ask (operator-proposed, A8-reviewed 2026-07-09).
//
// After an exchange ends, ONE non-streaming pass asks Dave-in-character
// whether anything would pull him back later, and when. The ask reuses the
// EXACT message vector the chat reply was just generated from, plus that
// reply — so it rides a warm llama-server prompt cache (A8 R4: never
// re-assemble from DB, never re-run recall; that would bust the cache and
// double-write telemetry). The meta turn is a one-pass instruction in the
// idle/departure family (A1: never enters persistent context, never rendered).
//
// Parsing is STRICT and conservative: anything that isn't a clean clock time
// or relative duration — "nothing", prose, refusals, mush — stores as a
// no-intention row (raw reply kept; the no-answers are half the signal).
// Silent-degradation guard (A8 R5-R7): max_tokens 48 so a stray <think> open
// can't eat the answer; the ask corpus is surfaced by corpus_inspect.py; and
// tools/intention_ask_smoke.py measures parse/nothing/echo rates live before
// arm C is trusted.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use chrono::{Datelike, Local, TimeZone, Timelike};

use crate::llama_client::{ChatMessage, LlamaClient};
use crate::persistence::{self, DbHandle};

/// Kill switches (no UI). `intention_ask_enabled=0` stops the per-exchange
/// ask entirely; `intention_act_enabled=0` keeps asking (observational
/// corpus) but arm C never consumes — so the ask channel can run for a while
/// and be inspected before any behavior rides on it.
pub const SETTING_KEY_ASK_ENABLED: &str = "intention_ask_enabled";
pub const SETTING_KEY_ACT_ENABLED: &str = "intention_act_enabled";

const ASK_MAX_TOKENS: u32 = 48;
const ASK_TEMPERATURE: f32 = 0.6;
/// Parsed times closer than this are discarded (past/immediate = mush).
const MIN_LEAD_SECONDS: i64 = 120;
/// Settle delay after the exchange before asking; a fast follow-up message
/// sets chat_in_flight and the ask skips.
const ASK_SETTLE_SECONDS: u64 = 2;

fn setting_on(db: &DbHandle, key: &str) -> bool {
    !matches!(
        persistence::get_setting_blocking(db, key).ok().flatten().as_deref(),
        Some("0")
    )
}

/// Whether arm C may CONSUME intentions (act). The ask can stay on while this
/// is off — observational corpus first, behavior later.
pub fn act_enabled(db: &DbHandle) -> bool {
    setting_on(db, SETTING_KEY_ACT_ENABLED)
}

/// The meta ask. Register matches the §8 family (bracketed, hyphens, an easy
/// exit). NO example time: the live smoke (tools/intention_ask_smoke.py)
/// measured 83-100% exact example-echo on this substrate — with an example,
/// the "stated intention" is just the example parroted back, a cron with
/// extra steps. Example-free, the same smoke measured varied, parseable,
/// unanchored times ("digits like hour:minute" carries the format instead).
pub fn build_ask(now_epoch: i64) -> String {
    let now = Local.timestamp_opt(now_epoch, 0).single().unwrap_or_else(Local::now);
    format!(
        "[meta-instruction - answer with one short line and nothing else: It is \
{}:{:02} {} on {}. If something in this conversation would pull you back to \
the human later - a thought that will finish itself, something worth checking \
on - write the clock time you'd come back, digits like hour:minute. Most of \
the time nothing pulls; then answer: nothing.]",
        now.hour12().1,
        now.minute(),
        if now.hour() < 12 { "am" } else { "pm" },
        now.weekday(),
    )
}

/// Strict parse of Dave's reply → absolute epoch fire time. None = no
/// intention (the conservative default for everything unclear).
pub fn parse_intention(raw: &str, now_epoch: i64) -> Option<i64> {
    let lower = raw.trim().to_lowercase();
    if lower.is_empty() || lower.starts_with("nothing") || lower.starts_with("no.")
        || lower == "no" || lower.starts_with("nah")
    {
        return None;
    }

    // Relative: "in 20 minutes" / "in 2 hours" / "in an hour"
    if let Some(rest) = lower.strip_prefix("in ").or_else(|| {
        lower.find(" in ").map(|i| &lower[i + 4..])
    }) {
        let rest = rest.trim();
        let (num, unit) = if let Some(u) = rest.strip_prefix("an hour").or(rest.strip_prefix("a hour")) {
            let _ = u;
            (1i64, "hour")
        } else {
            let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
            let n: i64 = digits.parse().ok()?;
            let after = rest[digits.len()..].trim_start();
            let unit = if after.starts_with("hour") || after.starts_with("hr") {
                "hour"
            } else if after.starts_with("min") {
                "min"
            } else {
                return None;
            };
            (n, unit)
        };
        let secs = if unit == "hour" { num * 3600 } else { num * 60 };
        let t = now_epoch + secs;
        return if secs >= MIN_LEAD_SECONDS { Some(t) } else { None };
    }

    // Clock time: "h:mm am/pm" or 24h "hh:mm". Scan for the first d{1,2}:d{2}.
    let bytes = lower.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_digit() {
            let start = i;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
            if i < bytes.len() && bytes[i] == b':' && i + 2 < bytes.len() + 1 {
                let h: i64 = lower[start..i].parse().ok()?;
                let mm_end = (i + 3).min(lower.len());
                let mins_str = &lower[i + 1..mm_end];
                if mins_str.len() == 2 && mins_str.bytes().all(|b| b.is_ascii_digit()) {
                    let m: i64 = mins_str.parse().ok()?;
                    if m > 59 {
                        return None;
                    }
                    let tail = &lower[mm_end..];
                    let ampm = if tail.trim_start().starts_with("pm") {
                        Some("pm")
                    } else if tail.trim_start().starts_with("am") {
                        Some("am")
                    } else {
                        None
                    };
                    let hour24 = match ampm {
                        Some("pm") if h < 12 => h + 12,
                        Some("am") if h == 12 => 0,
                        Some(_) => h,
                        None => h, // bare hh:mm treated as 24h
                    };
                    if hour24 > 23 {
                        return None;
                    }
                    let now = Local.timestamp_opt(now_epoch, 0).single()?;
                    let candidate = now
                        .date_naive()
                        .and_hms_opt(hour24 as u32, m as u32, 0)?;
                    let t = Local
                        .from_local_datetime(&candidate)
                        .single()?
                        .timestamp();
                    // Same-day only; a past/immediate time is mush, not an
                    // intention for tomorrow.
                    return if t - now_epoch >= MIN_LEAD_SECONDS { Some(t) } else { None };
                }
            }
        } else {
            i += 1;
        }
    }
    None
}

/// The fire-and-forget exchange-end ask. `messages` is the EXACT vector the
/// chat reply was generated from; `reply` is that reply. Guard values are
/// captured at exchange end; the insert is conditional on them (R1).
#[allow(clippy::too_many_arguments)]
pub fn spawn_ask(
    db: DbHandle,
    client: LlamaClient,
    chat_in_flight: Arc<AtomicBool>,
    mut messages: Vec<ChatMessage>,
    reply: String,
    conversation_id: i64,
    source_message_id: i64,
    guard_last_user_input: i64,
) {
    tauri::async_runtime::spawn(async move {
        if !setting_on(&db, SETTING_KEY_ASK_ENABLED) {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_secs(ASK_SETTLE_SECONDS)).await;
        if chat_in_flight.load(Ordering::SeqCst) {
            return; // user already followed up; the moment passed
        }
        let now = chrono::Utc::now().timestamp();
        messages.push(ChatMessage { role: "assistant".into(), content: reply });
        messages.push(ChatMessage { role: "user".into(), content: build_ask(now) });

        let raw = match client.complete(messages, ASK_MAX_TOKENS, ASK_TEMPERATURE).await {
            Ok(r) => r.trim().to_string(),
            Err(e) => {
                tracing::warn!("intention ask failed: {}", e);
                return;
            }
        };
        let fire_at = parse_intention(&raw, now);
        match persistence::insert_intention_guarded(
            &db,
            conversation_id,
            Some(source_message_id),
            guard_last_user_input,
            &raw,
            fire_at,
            guard_last_user_input,
        )
        .await
        {
            Ok(true) => tracing::info!(
                "intention ask: stored (fire_at={:?}, raw={:?})",
                fire_at,
                raw.chars().take(60).collect::<String>()
            ),
            Ok(false) => tracing::info!("intention ask: stale (user spoke mid-ask), discarded"),
            Err(e) => tracing::warn!("intention ask: store failed: {}", e),
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    // A fixed local "now": derive from a real timestamp so DST is coherent.
    fn now_at(h: u32, m: u32) -> i64 {
        let today = Local::now().date_naive();
        Local
            .from_local_datetime(&today.and_hms_opt(h, m, 0).unwrap())
            .single()
            .unwrap()
            .timestamp()
    }

    #[test]
    fn parses_am_pm() {
        let now = now_at(14, 0); // 2:00 pm
        let t = parse_intention("8:40 pm", now).unwrap();
        assert_eq!(t - now, (6 * 60 + 40) * 60);
        assert!(parse_intention("4:17 pm.", now).is_some());
        // 12am edge
        let now2 = now_at(22, 0);
        assert!(parse_intention("11:30 pm", now2).is_some());
    }

    #[test]
    fn parses_24h_and_relative() {
        let now = now_at(9, 0);
        let t = parse_intention("13:05", now).unwrap();
        assert_eq!(t - now, (4 * 60 + 5) * 60);
        assert_eq!(parse_intention("in 20 minutes", now).unwrap() - now, 1200);
        assert_eq!(parse_intention("in 2 hours", now).unwrap() - now, 7200);
        assert_eq!(parse_intention("in an hour", now).unwrap() - now, 3600);
    }

    #[test]
    fn nothing_and_mush_parse_to_none() {
        let now = now_at(12, 0);
        for s in [
            "nothing",
            "Nothing.",
            "no",
            "nah, it's all settled",
            "I'll reach out when it feels right",
            "the etymology will keep",
            "",
            "maybe later",
        ] {
            assert!(parse_intention(s, now).is_none(), "{s:?} should be None");
        }
    }

    #[test]
    fn past_and_immediate_times_discarded() {
        let now = now_at(14, 0);
        assert!(parse_intention("9:00 am", now).is_none()); // past
        assert!(parse_intention("2:01 pm", now).is_none()); // < 2min lead
        assert!(parse_intention("in 1 minute", now).is_none());
    }

    #[test]
    fn embedded_time_in_prose_parses() {
        let now = now_at(14, 0);
        assert!(parse_intention("maybe 6:15 pm, if the thought holds", now).is_some());
    }

    #[test]
    fn ask_prompt_is_family_registered_and_example_free() {
        let a = build_ask(now_at(10, 3));
        let b = build_ask(now_at(16, 44));
        assert!(a.starts_with("[meta-instruction"));
        assert!(a.contains("nothing"));
        assert!(a.contains("hour:minute"), "format carried by words, not an example");
        assert!(!a.contains("like \""), "no example time — 83-100% echo measured");
        assert_ne!(a, b, "the stated now varies");
        assert!(!a.contains('—'), "family register uses hyphens");
    }
}
