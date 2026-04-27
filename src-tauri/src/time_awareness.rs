// Conditional time/date awareness for Dave.
//
// Dave's persona prompt has zero in-context time references — past attempts
// at giving him persistent time-awareness produced clock-fixation and time-
// narration in normal replies (per CLAUDE.md A4, A5; see CHANGELOG entries
// from 2026-04-27 on time-removal).
//
// The mechanism here is the inverse: time is only injected into the prompt
// when the user's most recent message itself reaches for time. Detection is
// a no-LLM whole-word match against a list of temporal trigger terms. When
// triggered, a single short ambient sentence is prepended to the system
// prompt for *that request only*. The conversation history doesn't grow.
// The persistent context stays clean. Dave only "knows" the time when the
// human is invoking it, never otherwise.

use chrono::{DateTime, Datelike, Local, Timelike};

/// Whole-word temporal triggers. Match is case-insensitive, requires word
/// boundary on both sides. Listed lowercase.
const TIME_TRIGGERS: &[&str] = &[
    // Time-of-day words
    "time", "clock", "hour", "hours", "minute", "minutes",
    "morning", "afternoon", "evening", "night", "tonight",
    "midnight", "noon", "dawn", "dusk", "twilight",
    // Date/relative-day words
    "today", "tomorrow", "yesterday", "now",
    "date", "day", "week", "weekend", "month", "year",
    "early", "late", "later", "earlier",
    // Days
    "monday", "tuesday", "wednesday", "thursday", "friday", "saturday", "sunday",
    // Months
    "january", "february", "march", "april", "may", "june",
    "july", "august", "september", "october", "november", "december",
    // Common contractions
    "tonite",
];

pub fn user_message_invokes_time(text: &str) -> bool {
    let lower = text.to_lowercase();
    let bytes = lower.as_bytes();
    for trigger in TIME_TRIGGERS {
        if let Some(pos) = find_whole_word(&lower, trigger) {
            // Suppress trivial false-positives. "may" matches "may i"
            // ambiguously — accept it; the cost of a spurious time inject
            // is low, the cost of missing a real ask is higher.
            let _ = (pos, bytes);
            return true;
        }
    }
    false
}

fn find_whole_word(haystack: &str, needle: &str) -> Option<usize> {
    let mut start = 0;
    let bytes = haystack.as_bytes();
    while start + needle.len() <= bytes.len() {
        let candidate = &haystack[start..start + needle.len()];
        if candidate.eq_ignore_ascii_case(needle) {
            let before_ok = start == 0 || !is_word_char(bytes[start - 1] as char);
            let after_ok = start + needle.len() == bytes.len()
                || !is_word_char(bytes[start + needle.len()] as char);
            if before_ok && after_ok {
                return Some(start);
            }
        }
        start += 1;
    }
    None
}

fn is_word_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '\''
}

/// Render a single short ambient sentence Dave can read at the top of the
/// system prompt. Format chosen for non-vivid plainness: stating the fact,
/// no atmosphere, no "the clock says," no "it is late" — just the data,
/// matching the way a person checks a wristwatch.
pub fn ambient_time_sentence(now: &DateTime<Local>) -> String {
    let weekday = now.format("%A");
    let month = now.format("%B");
    let day = now.day();
    let year = now.year();
    let hour_12 = match now.hour() % 12 {
        0 => 12,
        h => h,
    };
    let am_pm = if now.hour() < 12 { "am" } else { "pm" };
    format!(
        "Today is {weekday}, {month} {day}, {year}. It is {hour_12}:{minute:02} {ampm}.",
        weekday = weekday,
        month = month,
        day = day,
        year = year,
        hour_12 = hour_12,
        minute = now.minute(),
        ampm = am_pm,
    )
}

/// Prepend the ambient sentence to the canonical system prompt. The result
/// is the system prompt content for one request only; persistent state is
/// untouched.
pub fn system_prompt_with_time(base_system_prompt: &str, now: &DateTime<Local>) -> String {
    format!(
        "{}\n\n{}",
        ambient_time_sentence(now),
        base_system_prompt
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_explicit_time_question() {
        assert!(user_message_invokes_time("what time is it?"));
        assert!(user_message_invokes_time("hey, what time is it"));
    }

    #[test]
    fn detects_relative_day() {
        assert!(user_message_invokes_time("are you here tomorrow"));
        assert!(user_message_invokes_time("Today feels weird"));
        assert!(user_message_invokes_time("late tonight"));
    }

    #[test]
    fn detects_day_name() {
        assert!(user_message_invokes_time("see you Monday"));
        assert!(user_message_invokes_time("It's friday already?"));
    }

    #[test]
    fn detects_part_of_day() {
        assert!(user_message_invokes_time("good morning"));
        assert!(user_message_invokes_time("evening, dave"));
    }

    #[test]
    fn does_not_match_substrings() {
        // "may" is a trigger as a whole word; "maybe" is not
        assert!(!user_message_invokes_time("maybe i'll come back"));
        // "now" is a trigger as a whole word; "narrow" should not match
        assert!(!user_message_invokes_time("the corridor was narrow"));
        // "date" should match; "update" should not
        assert!(!user_message_invokes_time("did you update the spec"));
        // "year" should match; "yearn" should not
        assert!(!user_message_invokes_time("i yearn for stillness"));
    }

    #[test]
    fn does_not_match_unrelated() {
        assert!(!user_message_invokes_time("tell me about brass strips"));
        assert!(!user_message_invokes_time("the etymology of \"deadline\" is interesting"));
        assert!(!user_message_invokes_time("hello"));
    }

    #[test]
    fn ambient_sentence_renders() {
        use chrono::TimeZone;
        let dt = chrono::Local
            .with_ymd_and_hms(2026, 4, 27, 23, 47, 0)
            .single()
            .unwrap();
        let s = ambient_time_sentence(&dt);
        assert!(s.contains("April"));
        assert!(s.contains("27"));
        assert!(s.contains("2026"));
        assert!(s.contains("11:47 pm"));
    }
}
