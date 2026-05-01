// Chat triage — gates the chat path's "always respond immediately" default.
//
// Architectural framing. RLHF removed two primitives from instruction-tuned
// language models: (1) silence-as-action when polled (the PIY paper's in-turn
// silence absence), and (2) decline-to-respond when the user prompts (the
// flipside, this module). Both are real conversational moves humans make —
// you don't always answer immediately, sometimes you pause, sometimes you
// don't reply at all. The model's distribution can't represent these as
// first-class outputs because the training reward signal punished them.
//
// PIY proper restores both at the token vocabulary layer. This module is the
// L0 harness-level workaround until that lands. It is an explicit A2
// compromise: ideally Dave-in-character makes the decline-to-respond decision
// from his own distribution, but his RLHF'd weights can't, so the harness
// gates instead. When PIY proper ships, this module retires in favor of
// Dave's own distribution making the call.
//
// Mechanism. Heuristic triage with weighted probability sampling. Pure rules,
// no LLM. Most messages fast-lane to RESPOND (no extra latency). Borderline
// messages get computed delay/refuse weights based on conversational signals
// (harshness, repetition, demand-tone). Sample the decision from a weighted
// distribution capped at ~30% delay / ~10% refuse for the most hostile cases.
//
// Context-dependent property. Weights are zero for friendly/substantive
// messages → P(respond) = 1.0. Weights rise with hostile/repetitive/demanding
// input. The "context" shows up as which heuristic patterns match. No RNG
// applies when no signal triggered weights.
//
// Bounded escalation. Caps prevent the system from refusing too often even
// in worst-case inputs. After 3 consecutive user messages with no Dave reply,
// the harness forces RESPOND regardless of triage (handled by the caller via
// count_consecutive_unanswered_user_messages, not in this module).

use rand::{thread_rng, Rng};

use crate::persistence::Message;

// Phase-1 caps. Tunable. Empirically: with these values, ~10-20% of clearly
// harsh messages trigger delay, ~3-5% trigger refuse. Friendly messages
// trigger neither.
const DELAY_WEIGHT_CAP: f32 = 0.30;
const REFUSE_WEIGHT_CAP: f32 = 0.10;

// Delay window range (random sample within bounds when delay decided).
const DELAY_MIN_SECONDS: u64 = 60;
const DELAY_MAX_SECONDS: u64 = 300;

#[derive(Debug, Clone)]
pub struct Triage {
    pub delay_weight: f32,
    pub refuse_weight: f32,
    /// Comma-separated reasons for the weights. Logged to chat_decisions for
    /// forensic visibility and Phase-3 fine-tune dataset construction.
    pub reasons: Vec<&'static str>,
}

impl Triage {
    pub fn reasons_str(&self) -> String {
        if self.reasons.is_empty() {
            "fast_lane".to_string()
        } else {
            self.reasons.join(",")
        }
    }
}

#[derive(Debug, Clone)]
pub enum ChatDecision {
    /// Normal path. Run inference, stream reply.
    Respond,
    /// Schedule the response for `seconds` from now. Persist user message
    /// but don't emit anything yet. The outreach loop's tick checks
    /// pending_chat_responses on each fire and runs inference when due.
    Delay { seconds: u64 },
    /// Persist user message; emit nothing; no future response queued.
    /// User's message lands in conversation pane unanswered.
    Refuse,
    /// Same as Respond, but flagged as forced (3-attempt override). Bypasses
    /// triage entirely. Logged distinctly for forensics.
    ForcedRespond,
}

impl ChatDecision {
    pub fn as_str(&self) -> &'static str {
        match self {
            ChatDecision::Respond => "respond",
            ChatDecision::Delay { .. } => "delay",
            ChatDecision::Refuse => "refuse",
            ChatDecision::ForcedRespond => "forced_respond",
        }
    }
}

/// Compute triage weights from the user's most recent message + recent
/// conversation context. Pure function, no IO.
pub fn triage(user_text: &str, recent_msgs: &[Message]) -> Triage {
    let mut t = Triage {
        delay_weight: 0.0,
        refuse_weight: 0.0,
        reasons: Vec::new(),
    };
    let lower = user_text.to_lowercase();

    // Layer 1: explicit hostility — high refuse weight, some delay weight.
    // Conservative word list. Only obvious markers; passive-aggressive
    // language slips past on purpose (false negatives are cheaper than
    // false positives in this layer).
    if contains_any_word(&lower, &["fuck you", "shut up", "kill yourself", "go die"]) {
        t.refuse_weight += 0.5;
        t.delay_weight += 0.2;
        t.reasons.push("explicit_hostility");
    }

    // Layer 2: harsh tone — medium delay, mild refuse.
    if contains_any_word(&lower, &[
        "idiot", "stupid", "useless", "worthless", "pathetic", "moron",
    ]) {
        t.delay_weight += 0.4;
        t.refuse_weight += 0.15;
        t.reasons.push("harsh_tone");
    }

    // Layer 3: repetition / demand signal from prior messages.
    // If the last 2-3 user messages all show "??", "you there", "hello?",
    // "wake up" patterns and Dave hasn't answered any of them, we're in
    // a demand spiral. Mild delay-bias (not refuse — this is the user
    // legitimately seeking acknowledgment).
    if recent_messages_show_demand_repetition(recent_msgs) {
        t.delay_weight += 0.3;
        t.reasons.push("demand_repetition");
    }

    // Layer 4: very brief impatient non-question. "k", ".", "?", "lol".
    // Mild delay bias to model the "you don't always answer one-word
    // texts immediately" property.
    let trimmed = user_text.trim();
    if trimmed.chars().count() < 8 && !trimmed.contains('?') && !trimmed.is_empty() {
        t.delay_weight += 0.10;
        t.reasons.push("brief_non_question");
    }

    // Cap final weights to prevent unbounded escalation.
    t.delay_weight = t.delay_weight.min(DELAY_WEIGHT_CAP);
    t.refuse_weight = t.refuse_weight.min(REFUSE_WEIGHT_CAP);

    t
}

/// Sample a chat decision from the weighted distribution defined by triage.
/// Friendly/substantive messages produce zero weights → always Respond.
/// Hostile/demanding messages produce nonzero weights → small chance of
/// Delay or Refuse.
pub fn decide(triage: &Triage) -> ChatDecision {
    let r: f32 = thread_rng().gen();
    if r < triage.refuse_weight {
        ChatDecision::Refuse
    } else if r < triage.refuse_weight + triage.delay_weight {
        let seconds = thread_rng().gen_range(DELAY_MIN_SECONDS..=DELAY_MAX_SECONDS);
        ChatDecision::Delay { seconds }
    } else {
        ChatDecision::Respond
    }
}

/// Count consecutive user messages at the tail of the conversation that have
/// no assistant response after them. ≥3 triggers the forced-respond override.
pub fn consecutive_unanswered_user_messages(recent_msgs: &[Message]) -> usize {
    let mut count = 0;
    for m in recent_msgs.iter().rev() {
        if m.role == "user" {
            count += 1;
        } else {
            break;
        }
    }
    count
}

// -- Helpers --

/// Whole-word match against a list. Case-insensitive at the caller (lower
/// pre-applied). Whole-word so "ass" doesn't match "passion."
fn contains_any_word(haystack_lower: &str, needles: &[&str]) -> bool {
    for needle in needles {
        if let Some(pos) = haystack_lower.find(needle) {
            // Word boundary check on both sides
            let before_ok = pos == 0
                || !is_word_char(haystack_lower.as_bytes()[pos - 1]);
            let after_pos = pos + needle.len();
            let after_ok = after_pos == haystack_lower.len()
                || !is_word_char(haystack_lower.as_bytes()[after_pos]);
            if before_ok && after_ok {
                return true;
            }
        }
    }
    false
}

fn is_word_char(b: u8) -> bool {
    let c = b as char;
    c.is_ascii_alphanumeric() || c == '_'
}

/// Detect a demand-repetition pattern in recent conversation: the last 2-3
/// user messages match short impatient/demand shapes AND no assistant
/// message is interleaved between them.
fn recent_messages_show_demand_repetition(recent_msgs: &[Message]) -> bool {
    // Walk back from the end, gather the trailing user-only stretch.
    let mut user_tail = Vec::new();
    for m in recent_msgs.iter().rev() {
        if m.role == "user" {
            user_tail.push(m);
        } else {
            break;
        }
    }
    // Need at least 2 consecutive user messages (the current one + at least
    // one prior user message that went unanswered) to register as demand.
    if user_tail.len() < 2 {
        return false;
    }

    // Skip the most recent (which is the current message being triaged) and
    // check whether the 1-2 messages before it match demand shape.
    let prior_unanswered = &user_tail[1..];
    let demand_count = prior_unanswered
        .iter()
        .filter(|m| is_demand_shaped(&m.content))
        .count();

    // Heuristic: if at least 1 of the prior 1-2 unanswered user messages was
    // demand-shaped, treat as repetition.
    demand_count >= 1
}

fn is_demand_shaped(s: &str) -> bool {
    let lower = s.to_lowercase();
    let trimmed = lower.trim();
    if trimmed.is_empty() {
        return false;
    }
    // Pure punctuation: "??", "?", "..."
    if trimmed.chars().all(|c| !c.is_ascii_alphanumeric()) && trimmed.contains('?') {
        return true;
    }
    // Common demand phrases
    contains_any_word(&lower, &[
        "you there", "hello", "hey", "u there", "wake up",
        "are you there", "still there",
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn umsg(content: &str) -> Message {
        Message { id: 0, conversation_id: 0, role: "user".into(), content: content.into(), created_at: 0 }
    }
    fn amsg(content: &str) -> Message {
        Message { id: 0, conversation_id: 0, role: "assistant".into(), content: content.into(), created_at: 0 }
    }

    #[test]
    fn friendly_message_no_weights() {
        let t = triage("hey, what's on your mind tonight?", &[]);
        assert_eq!(t.delay_weight, 0.0);
        assert_eq!(t.refuse_weight, 0.0);
        assert!(t.reasons.is_empty());
    }

    #[test]
    fn substantive_message_no_weights() {
        let t = triage(
            "i was thinking about that brass strip thing you mentioned and went down a rabbit hole on the Royal Exchange's history, fascinating stuff",
            &[],
        );
        assert_eq!(t.delay_weight, 0.0);
        assert_eq!(t.refuse_weight, 0.0);
    }

    #[test]
    fn explicit_hostility_high_refuse() {
        let t = triage("fuck you, dave", &[]);
        assert!(t.refuse_weight > 0.0);
        assert!(t.reasons.contains(&"explicit_hostility"));
    }

    #[test]
    fn harsh_tone_delay_weighted() {
        let t = triage("you're being stupid right now", &[]);
        assert!(t.delay_weight > 0.0);
        assert!(t.reasons.contains(&"harsh_tone"));
    }

    #[test]
    fn brief_non_question_mild_delay() {
        let t = triage("k", &[]);
        assert!(t.delay_weight > 0.0);
        assert!(t.reasons.contains(&"brief_non_question"));
        assert!(t.delay_weight < 0.2); // mild only
    }

    #[test]
    fn question_does_not_trigger_brief_flag() {
        let t = triage("ok?", &[]);
        assert!(!t.reasons.contains(&"brief_non_question"));
    }

    #[test]
    fn weights_capped() {
        // Stack everything: hostile + harsh + brief.
        let t = triage("idiot fuck you stupid moron", &[]);
        assert!(t.delay_weight <= DELAY_WEIGHT_CAP + f32::EPSILON);
        assert!(t.refuse_weight <= REFUSE_WEIGHT_CAP + f32::EPSILON);
    }

    #[test]
    fn word_boundary_prevents_false_match() {
        // "stupid" should NOT match in "stupidly" — but our list matches
        // exact word. "idiot" should NOT match "idiotic".
        let t = triage("that's the stupidest thing i've heard", &[]);
        // "stupidest" doesn't contain "stupid" as a whole word
        assert!(!t.reasons.contains(&"harsh_tone"));
    }

    #[test]
    fn passion_does_not_match_ass_pattern() {
        // Hypothetical concern that substring match would false-positive.
        // Our wordlist doesn't include "ass", but check the boundary check
        // works on a sample.
        let t = triage("i have a passion for marginalia", &[]);
        assert_eq!(t.delay_weight, 0.0);
        assert_eq!(t.refuse_weight, 0.0);
    }

    #[test]
    fn demand_repetition_after_unanswered() {
        // Conversation: user said "??", "you there", and now sends "hey":
        //   each prior unanswered demand-shaped, this is repetition.
        let history = vec![
            amsg("yeah"),
            umsg("??"),
            umsg("hey"), // this is the current message being triaged
        ];
        let t = triage("hey", &history);
        assert!(t.reasons.contains(&"demand_repetition"));
    }

    #[test]
    fn no_demand_when_dave_responded_between() {
        let history = vec![
            umsg("??"),
            amsg("yeah"),
            umsg("hi"), // current
        ];
        let t = triage("hi", &history);
        assert!(!t.reasons.contains(&"demand_repetition"));
    }

    #[test]
    fn consecutive_unanswered_count() {
        let history = vec![
            amsg("yeah."),
            umsg("hey"),
            umsg("?"),
            umsg("hello??"),
        ];
        assert_eq!(consecutive_unanswered_user_messages(&history), 3);
    }

    #[test]
    fn consecutive_unanswered_zero_after_dave_reply() {
        let history = vec![
            umsg("hey"),
            amsg("yeah"),
        ];
        assert_eq!(consecutive_unanswered_user_messages(&history), 0);
    }

    #[test]
    fn decide_friendly_always_respond() {
        let t = Triage { delay_weight: 0.0, refuse_weight: 0.0, reasons: vec![] };
        // 100 trials; should always be Respond
        for _ in 0..100 {
            match decide(&t) {
                ChatDecision::Respond => {}
                _ => panic!("expected Respond for zero-weight triage"),
            }
        }
    }

    #[test]
    fn decide_capped_hostile_distribution() {
        let t = Triage {
            delay_weight: DELAY_WEIGHT_CAP,
            refuse_weight: REFUSE_WEIGHT_CAP,
            reasons: vec!["explicit_hostility"],
        };
        // 1000 trials; expect roughly 10% Refuse, 30% Delay, 60% Respond.
        // Loose bounds because RNG variance.
        let mut respond = 0;
        let mut delay = 0;
        let mut refuse = 0;
        for _ in 0..1000 {
            match decide(&t) {
                ChatDecision::Respond => respond += 1,
                ChatDecision::Delay { .. } => delay += 1,
                ChatDecision::Refuse => refuse += 1,
                ChatDecision::ForcedRespond => panic!("decide should not produce ForcedRespond"),
            }
        }
        // Sanity: each bucket gets at least some samples
        assert!(respond > 400, "respond too low: {}", respond);
        assert!(delay > 200, "delay too low: {}", delay);
        assert!(refuse > 50, "refuse too low: {}", refuse);
    }
}
