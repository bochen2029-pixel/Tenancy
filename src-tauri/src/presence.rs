// User-presence sensor. The precondition for Dave's self-initiation (PIY §5.4):
// he should reach only when the operator is PRESENT-BUT-ELSEWHERE — at the
// machine, using it, but not focused on Dave's window — never into an empty
// room (away) and never on top of an active chat (in_chat). This is the single
// missing conditioning variable the initiation-timing roadmap depends on, and
// presence history cannot be reconstructed after the fact, so we start logging
// it from day one.
//
// Two signals combine:
//   - window focus (is Dave's window foreground?) — Tauri WindowEvent::Focused
//   - machine-wide OS idle (ms since ANY keyboard/mouse input) — Win32
//     GetLastInputInfo (no elevation, no network, microsecond read).
//
// STAGE 0: this only SENSES and LOGS. It does not yet gate reaching (that is a
// deliberate one-line follow-on, after the A8 review). Dave's reach behavior is
// unchanged; we are accumulating the corpus.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::AppHandle;
use tokio::time::{sleep, Duration};

use crate::persistence::{self, DbHandle};

/// No input for longer than this ⇒ operator is AWAY (never reach). Below it,
/// they are at the keyboard — present.
const AWAY_IDLE_MS: u64 = 5 * 60 * 1000; // 5 minutes
/// Presence sampler cadence. Cheap; logs only on state transition.
const SAMPLE_TICK_SECONDS: u64 = 15;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresenceState {
    /// Dave's window is foreground — the user is in the chat. Never reach.
    InChat,
    /// At the machine (recent input) but Dave's window is not foreground —
    /// the target state for self-initiation.
    PresentElsewhere,
    /// No input for a while — the user stepped away. Never reach.
    Away,
    /// OS idle unavailable (non-Windows / API failure). Treat conservatively.
    Unknown,
}

impl PresenceState {
    pub fn as_str(&self) -> &'static str {
        match self {
            PresenceState::InChat => "in_chat",
            PresenceState::PresentElsewhere => "present_elsewhere",
            PresenceState::Away => "away",
            PresenceState::Unknown => "unknown",
        }
    }
    /// Whether a self-initiated reach is permissible in this state. Hard gate
    /// for the FUTURE learned timer: only reach when present-but-elsewhere.
    /// Not yet enforced in Stage 0 (sense-only).
    #[allow(dead_code)]
    pub fn reach_allowed(&self) -> bool {
        matches!(self, PresenceState::PresentElsewhere)
    }
}

/// Milliseconds since the last machine-wide input event, or None if
/// unavailable on this platform.
#[cfg(windows)]
pub fn os_idle_ms() -> Option<u64> {
    #[repr(C)]
    struct LastInputInfo {
        cb: u32,
        dw_time: u32,
    }
    extern "system" {
        fn GetLastInputInfo(plii: *mut LastInputInfo) -> i32;
        fn GetTickCount() -> u32;
    }
    unsafe {
        let mut lii = LastInputInfo {
            cb: std::mem::size_of::<LastInputInfo>() as u32,
            dw_time: 0,
        };
        if GetLastInputInfo(&mut lii) != 0 {
            let now = GetTickCount();
            Some(now.wrapping_sub(lii.dw_time) as u64)
        } else {
            None
        }
    }
}

#[cfg(not(windows))]
pub fn os_idle_ms() -> Option<u64> {
    None
}

/// Combine window focus + OS idle into the 3-state presence classification.
pub fn classify(focused: bool, idle_ms: Option<u64>) -> PresenceState {
    if focused {
        return PresenceState::InChat;
    }
    match idle_ms {
        Some(ms) if ms >= AWAY_IDLE_MS => PresenceState::Away,
        Some(_) => PresenceState::PresentElsewhere,
        None => PresenceState::Unknown,
    }
}

/// Read the live presence state. Returns (state, os_idle_ms, focused).
pub fn current(window_focused: &AtomicBool) -> (PresenceState, Option<u64>, bool) {
    let focused = window_focused.load(Ordering::Relaxed);
    let idle = os_idle_ms();
    (classify(focused, idle), idle, focused)
}

/// Spawn the presence sampler: samples every SAMPLE_TICK_SECONDS and writes a
/// presence_samples row ONLY on state transition (a compact presence timeline).
/// Fire-and-forget; ends with the process.
pub fn spawn_sampler(_app: AppHandle, db: DbHandle, window_focused: Arc<AtomicBool>) {
    tauri::async_runtime::spawn(async move {
        let mut last: Option<PresenceState> = None;
        loop {
            sleep(Duration::from_secs(SAMPLE_TICK_SECONDS)).await;
            let (state, idle, focused) = current(&window_focused);
            if last != Some(state) {
                let _ = persistence::insert_presence_sample(
                    &db,
                    state.as_str(),
                    idle.map(|m| m as i64),
                    focused,
                )
                .await;
                last = Some(state);
            }
        }
    });
}
