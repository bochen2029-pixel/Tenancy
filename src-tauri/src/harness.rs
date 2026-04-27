// Time/duration helpers used by the idle worker (which writes journal
// entries). The per-turn `[meta:]` injection on regular chat sends was
// removed — Dave is timeless in normal conversation again.

use chrono::{DateTime, Local, Timelike};

pub fn humanize_duration(seconds: i64) -> String {
    if seconds < 60 {
        return "a moment".to_string();
    }
    let minutes = seconds / 60;
    if minutes < 60 {
        return if minutes == 1 {
            "a minute".to_string()
        } else {
            format!("{} minutes", minutes)
        };
    }
    let hours = seconds / 3600;
    if hours < 24 {
        return if hours == 1 {
            "an hour".to_string()
        } else {
            format!("{} hours", hours)
        };
    }
    let days = hours / 24;
    if days == 1 {
        "a day".to_string()
    } else {
        format!("{} days", days)
    }
}

pub fn format_clock(now: &DateTime<Local>) -> String {
    let hour_12 = match now.hour() % 12 {
        0 => 12,
        h => h,
    };
    let am_pm = if now.hour() < 12 { "am" } else { "pm" };
    format!("{}:{:02}{}", hour_12, now.minute(), am_pm)
}

