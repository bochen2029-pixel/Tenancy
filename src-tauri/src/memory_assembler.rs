// Memory partitioning + context assembly.
//
// Given the message history + active consolidation epochs for a conversation,
// produce the partitioned view (anchor / consolidated / recent) and the
// chat-message vector to send to llama-server.
//
// Partition rules:
// - Anchor zone: the FIRST `ANCHOR_MESSAGE_COUNT` messages of the conversation,
//   verbatim. Frozen. These are never consolidated.
// - Recent zone: the LAST `RECENT_MESSAGE_TARGET` messages, verbatim. Mutable.
// - Consolidated zone: messages between anchor and recent that fall within an
//   active epoch's [period_start_message_id, period_end_message_id] range are
//   replaced by that epoch's text. Messages in the middle that are NOT in any
//   active epoch are still loaded verbatim (un-consolidated middle).
//
// When recent zone exceeds RECENT_MESSAGE_TRIGGER, the consolidator background
// task fires and produces a new epoch covering the oldest CONSOLIDATION_BATCH
// messages of the recent zone.

use crate::llama_client::ChatMessage;
use crate::persistence::{ConsolidationEpoch, Message};

pub const ANCHOR_MESSAGE_COUNT: usize = 30;
pub const RECENT_MESSAGE_TARGET: usize = 100;
pub const RECENT_MESSAGE_TRIGGER: usize = 130;
pub const CONSOLIDATION_BATCH: usize = 30;

// Token budget targets (approximate, char-based estimation: 4 chars ≈ 1 token).
pub const TOKEN_BUDGET_TOTAL: usize = 64_000;
pub const TOKEN_RESERVE: usize = 6_000;

/// Hard cap on the tokens of assembled CONVERSATION context actually sent to
/// llama-server (system + anchor + canvas + middle epochs + recent). Distinct
/// from TOKEN_BUDGET_TOTAL, which is the llama-server ctx ceiling minus the
/// generation reserve. Without this cap the recent zone (RECENT_MESSAGE_TARGET
/// verbatim messages, never consolidated) grows unbounded with the conversation
/// — a real 233-message history already assembles to ~53k tokens, of which the
/// recent zone is ~82%, so every turn re-evaluates a near-full context and long
/// conversations crawl (and eventually overflow the 65536 ctx into silent
/// truncation). Enforced by trimming the OLDEST recent messages; anchor, canvas
/// and consolidated epochs are always kept — they are Dave's durable memory.
/// Trimmed messages stay in the DB (source of truth) and get folded into an
/// epoch as the conversation advances, so nothing is lost — it just passes out
/// of verbatim reach, which is the §7 "aging mind" behavior.
///
/// MIND-FEELING KNOB (per CLAUDE.md §14 — tune by feel, not metric): lower this
/// to make long conversations faster at the cost of how much recent talk Dave
/// holds verbatim; raise toward TOKEN_BUDGET_TOTAL - TOKEN_RESERVE (58_000) to
/// maximize memory at the cost of prompt-eval speed. Must stay ≤ 58_000.
///
/// Default is deliberately conservative: on the real 233-msg history it keeps
/// ~85 of the 100 recent messages verbatim (still trims/caps growth so context
/// can never overflow the ctx) — the A8 fresh-instance review flagged that a low
/// default over-optimizes eval speed against mind-feeling, the metric §14 says
/// wins. Lower it (e.g. 40_000 keeps ~62, 32_000 keeps ~45) if you want a bigger
/// immediate speedup and accept Dave holding less recent conversation verbatim.
pub const CONTEXT_SEND_BUDGET_TOKENS: usize = 48_000;

/// Never trim the recent zone below this many newest messages, even when over
/// budget — immediate conversational coherence must survive any cap.
pub const MIN_RECENT_MESSAGES: usize = 12;

/// Tokens reserved for the Ring-4 recall block (REEL Op 4) — reserved
/// UNCONDITIONALLY, whether or not recall fires this turn. Two reasons (both
/// A8-review findings): (1) it breaks the circular dependency between
/// recall-eligibility (which needs to know what got trimmed) and the trim
/// (which would otherwise depend on whether recall fired); (2) it keeps
/// `recent_keep_start` identical turn-to-turn, so the prompt prefix stays
/// stable and llama-server's prefix cache keeps hitting. Costs ~2-4 recent
/// messages of headroom; recall.rs caps its block to this budget.
pub const RECALL_RESERVE_TOKENS: usize = 600;

/// Char-based token estimate. Cheap, deterministic, no tokenizer dep.
/// English averages ~4 chars/token; we round up for safety.
pub fn estimate_tokens(text: &str) -> usize {
    (text.chars().count() + 3) / 4
}

#[derive(Debug, Clone)]
pub struct Partition {
    pub anchor: Vec<Message>,
    /// Operator-authored memory canvas. Always injected after anchor when
    /// non-empty. Free-form prose that Bo writes directly into Dave's
    /// memory layer (notes, facts, prescriptions). Bypasses Dave's own
    /// consolidation but flows into context like any assistant turn.
    pub canvas: String,
    /// Mix of epochs (consolidated) and bare messages (un-consolidated middle),
    /// in chronological order. Each entry is one "block."
    pub middle: Vec<MiddleBlock>,
    pub recent: Vec<Message>,
}

#[derive(Debug, Clone)]
pub enum MiddleBlock {
    Epoch(ConsolidationEpoch),
    Messages(Vec<Message>),
}

impl Partition {
    pub fn anchor_tokens(&self) -> usize {
        self.anchor.iter().map(|m| estimate_tokens(&m.content)).sum()
    }
    pub fn canvas_tokens(&self) -> usize {
        if self.canvas.is_empty() { 0 } else { estimate_tokens(&self.canvas) }
    }
    pub fn middle_tokens(&self) -> usize {
        self.middle.iter().map(|b| match b {
            MiddleBlock::Epoch(e) => e.token_count as usize,
            MiddleBlock::Messages(ms) => ms.iter().map(|m| estimate_tokens(&m.content)).sum(),
        }).sum()
    }
    pub fn recent_tokens(&self) -> usize {
        self.recent.iter().map(|m| estimate_tokens(&m.content)).sum()
    }
    pub fn total_tokens(&self) -> usize {
        self.anchor_tokens() + self.canvas_tokens() + self.middle_tokens() + self.recent_tokens()
    }
    // Test-only helpers. Production code computes these inline where needed.
    #[cfg(test)]
    pub fn epoch_count(&self) -> usize {
        self.middle.iter().filter(|b| matches!(b, MiddleBlock::Epoch(_))).count()
    }
    #[cfg(test)]
    pub fn middle_message_count(&self) -> usize {
        self.middle.iter().filter_map(|b| match b {
            MiddleBlock::Messages(ms) => Some(ms.len()),
            _ => None,
        }).sum()
    }
}

/// Partition the full message list + active epochs into anchor/middle/recent.
///
/// Active epochs are assumed to be ordered by epoch_number ASC and to have
/// non-overlapping period ranges (the consolidator enforces this).
pub fn partition(
    all_messages: &[Message],
    active_epochs: &[ConsolidationEpoch],
    canvas: &str,
) -> Partition {
    if all_messages.is_empty() {
        return Partition {
            anchor: Vec::new(),
            canvas: canvas.to_string(),
            middle: Vec::new(),
            recent: Vec::new(),
        };
    }

    let total = all_messages.len();
    let anchor_end = ANCHOR_MESSAGE_COUNT.min(total);
    let anchor: Vec<Message> = all_messages[..anchor_end].to_vec();

    // Recent zone: last RECENT_MESSAGE_TARGET messages, but never overlapping anchor.
    let recent_start = total.saturating_sub(RECENT_MESSAGE_TARGET).max(anchor_end);
    let recent: Vec<Message> = all_messages[recent_start..].to_vec();

    // Middle: everything between anchor_end and recent_start. Replace any
    // segment that falls within an active epoch's range with the epoch.
    let middle_msgs: &[Message] = &all_messages[anchor_end..recent_start];
    let middle = build_middle(middle_msgs, active_epochs);

    Partition { anchor, canvas: canvas.to_string(), middle, recent }
}

fn build_middle(
    middle_msgs: &[Message],
    active_epochs: &[ConsolidationEpoch],
) -> Vec<MiddleBlock> {
    if middle_msgs.is_empty() {
        return Vec::new();
    }
    // Walk the middle range; for each message, check whether it falls inside
    // any active epoch's range. Group consecutive messages into either an
    // Epoch block (when an epoch covers them) or a Messages block.
    let mut blocks: Vec<MiddleBlock> = Vec::new();
    let mut buffer: Vec<Message> = Vec::new();
    let mut current_epoch: Option<&ConsolidationEpoch> = None;

    for m in middle_msgs {
        // Find the epoch this message belongs to, if any.
        let containing = active_epochs.iter().find(|e| {
            m.id >= e.period_start_message_id && m.id <= e.period_end_message_id
        });

        match (current_epoch, containing) {
            (None, None) => buffer.push(m.clone()),
            (None, Some(e)) => {
                if !buffer.is_empty() {
                    blocks.push(MiddleBlock::Messages(std::mem::take(&mut buffer)));
                }
                blocks.push(MiddleBlock::Epoch(e.clone()));
                current_epoch = Some(e);
            }
            (Some(prev), Some(e)) if prev.id == e.id => {
                // Still inside the same epoch — already pushed once, skip.
            }
            (Some(_), Some(e)) => {
                // Transitioning to a new epoch (rare, but possible if epochs
                // are adjacent). Push the new one.
                blocks.push(MiddleBlock::Epoch(e.clone()));
                current_epoch = Some(e);
            }
            (Some(_), None) => {
                // Exited the previous epoch, back to bare messages.
                current_epoch = None;
                buffer.push(m.clone());
            }
        }
    }
    if !buffer.is_empty() {
        blocks.push(MiddleBlock::Messages(buffer));
    }
    blocks
}

/// Given the tokens already committed to the non-recent zones (system + anchor
/// + canvas + middle + appended user), return the index into `recent` from
/// which to KEEP messages so the assembled context stays within
/// CONTEXT_SEND_BUDGET_TOKENS. Fills newest-first (the tail is always kept);
/// never keeps fewer than MIN_RECENT_MESSAGES so immediate coherence survives
/// even a pathologically large fixed zone. Messages before the returned index
/// fall out of verbatim reach (they remain in the DB and are folded into an
/// epoch as the conversation advances).
pub fn recent_keep_start(fixed_tokens: usize, recent: &[Message]) -> usize {
    let remaining = CONTEXT_SEND_BUDGET_TOKENS.saturating_sub(fixed_tokens);
    let mut acc = 0usize;
    let mut keep_start = recent.len();
    for i in (0..recent.len()).rev() {
        let kept = recent.len() - i; // messages retained if we include recent[i..]
        let t = estimate_tokens(&recent[i].content);
        // The floor is unconditional: the MIN_RECENT_MESSAGES newest turns are
        // kept even if they exceed the budget (coherence > the cap in the
        // degenerate huge-message case; bounded, so it can't blow the ctx).
        if acc + t > remaining && kept > MIN_RECENT_MESSAGES {
            break;
        }
        acc += t;
        keep_start = i;
    }
    keep_start
}

/// The zone tokens that are committed before the recent zone fills the rest:
/// system + anchor + canvas + middle + the appended user turn + the recall
/// reserve. Single definition shared by build_chat_messages and
/// trimmed_recent_ids so the two can never disagree about the trim point.
fn committed_tokens(
    system_content: &str,
    partition: &Partition,
    appended_user: Option<&str>,
) -> usize {
    estimate_tokens(system_content)
        + partition.anchor_tokens()
        + partition.canvas_tokens()
        + partition.middle_tokens()
        + appended_user.map(estimate_tokens).unwrap_or(0)
        + RECALL_RESERVE_TOKENS
}

/// The ids of recent-zone messages that will NOT be sent this turn (trimmed by
/// the token budget). These, plus epoch-covered middle messages and the
/// journal, are exactly the recall-eligible set — text that exists on the Tape
/// but is not in front of Dave. Computable before recall runs because the
/// recall reserve is unconditional.
pub fn trimmed_recent_ids(
    system_content: &str,
    partition: &Partition,
    appended_user: Option<&str>,
) -> Vec<i64> {
    let keep_start = recent_keep_start(
        committed_tokens(system_content, partition, appended_user),
        &partition.recent,
    );
    partition.recent[..keep_start].iter().map(|m| m.id).collect()
}

/// Build the chat-message vector to send to llama-server. The new user turn
/// is appended at the end (caller passes the user text separately or omits
/// for outreach/idle which append nothing or their own meta). `recalled` is
/// the Ring-4 recall block (recall.rs), already gated and budget-capped; it
/// merges into the canvas memory turn so assistant turns never triple-stack.
pub fn build_chat_messages(
    system_content: &str,
    partition: &Partition,
    recalled: Option<&str>,
    appended_user: Option<&str>,
) -> Vec<ChatMessage> {
    let mut out = Vec::new();
    out.push(ChatMessage {
        role: "system".into(),
        content: system_content.to_string(),
    });
    for m in &partition.anchor {
        out.push(ChatMessage { role: m.role.clone(), content: m.content.clone() });
    }
    // Memory turn: operator-authored canvas + (when it fired) the Ring-4
    // recall block, as ONE assistant turn immediately after the anchor zone.
    // Reads as "things Dave knows/remembers about this conversation."
    // Empty both = no injection.
    let mut memory_turn = String::new();
    if !partition.canvas.trim().is_empty() {
        memory_turn.push_str(&partition.canvas);
    }
    if let Some(r) = recalled {
        if !r.trim().is_empty() {
            if !memory_turn.is_empty() {
                memory_turn.push_str("\n\n");
            }
            memory_turn.push_str(r);
        }
    }
    if !memory_turn.trim().is_empty() {
        out.push(ChatMessage {
            role: "assistant".into(),
            content: memory_turn,
        });
    }
    for block in &partition.middle {
        match block {
            MiddleBlock::Messages(ms) => {
                for m in ms {
                    out.push(ChatMessage { role: m.role.clone(), content: m.content.clone() });
                }
            }
            MiddleBlock::Epoch(e) => {
                // Epoch text is in Dave's voice; insert as an assistant turn.
                // If the alternation gets odd (consecutive assistants across
                // anchor/middle/recent boundaries), the chat template handles
                // it; Qwen3.5 is tolerant.
                out.push(ChatMessage { role: "assistant".into(), content: e.content.clone() });
            }
        }
    }
    // Token-budget the recent zone: keep the newest messages that fit within
    // CONTEXT_SEND_BUDGET_TOKENS. Anchor / canvas / epochs are already committed
    // above and are never trimmed (they are Dave's durable memory); only the
    // OLDEST recent messages fall out of verbatim reach. Prevents the recent
    // zone (never consolidated) from growing context unbounded on long
    // conversations. committed_tokens includes the unconditional recall
    // reserve, so the trim point is identical whether or not recall fired —
    // stable prefix, valid eligibility, warm prompt cache.
    let mut keep_start = recent_keep_start(
        committed_tokens(system_content, partition, appended_user),
        &partition.recent,
    );
    // Seam guard: if the last turn already emitted is an assistant (a canvas or
    // epoch injection, or an anchor ending on assistant) AND the kept recent
    // zone would open on another assistant, drop that single leading assistant
    // so the zone opens on a user turn. Stacking same-role turns nudges the
    // model to continue its own prior text instead of answering. Only fires when
    // the seam is genuinely assistant→assistant (not when recent legitimately
    // follows a user turn), and respects the recent-floor.
    if out.last().map(|m| m.role == "assistant").unwrap_or(false)
        && partition.recent.get(keep_start).map(|m| m.role == "assistant").unwrap_or(false)
        && (partition.recent.len() - keep_start) > MIN_RECENT_MESSAGES
    {
        keep_start += 1;
    }
    for m in &partition.recent[keep_start..] {
        out.push(ChatMessage { role: m.role.clone(), content: m.content.clone() });
    }
    if let Some(u) = appended_user {
        out.push(ChatMessage { role: "user".into(), content: u.to_string() });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(id: i64, role: &str, content: &str) -> Message {
        Message { id, conversation_id: 1, role: role.into(), content: content.into(), created_at: 0 }
    }

    fn epoch(id: i64, num: i64, start: i64, end: i64, content: &str) -> ConsolidationEpoch {
        ConsolidationEpoch {
            id, conversation_id: 1, epoch_number: num,
            period_start_message_id: start, period_end_message_id: end,
            content: content.into(),
            token_count: estimate_tokens(content) as i64,
            consolidation_depth: 1, created_at: 0, superseded_by: None,
        }
    }

    #[test]
    fn small_conversation_no_middle() {
        let msgs: Vec<Message> = (1..=10).map(|i| msg(i, "user", "x")).collect();
        let p = partition(&msgs, &[], "");
        assert_eq!(p.anchor.len(), 10);
        assert!(p.middle.is_empty());
        assert!(p.recent.is_empty());
    }

    #[test]
    fn anchor_fills_first_30() {
        let msgs: Vec<Message> = (1..=200).map(|i| msg(i, "user", "x")).collect();
        let p = partition(&msgs, &[], "");
        assert_eq!(p.anchor.len(), 30);
        assert_eq!(p.recent.len(), 100);
        assert_eq!(p.middle_message_count(), 70); // 200 - 30 - 100
    }

    #[test]
    fn epoch_replaces_middle_range() {
        let msgs: Vec<Message> = (1..=200).map(|i| msg(i, "user", "x")).collect();
        // Epoch covering message ids 31-60 (30 of the middle 70).
        let e = epoch(1, 1, 31, 60, "[memory of period 1]");
        let p = partition(&msgs, &[e], "");
        assert_eq!(p.epoch_count(), 1);
        // 70 middle messages - 30 covered by epoch = 40 bare middle messages
        assert_eq!(p.middle_message_count(), 40);
    }

    #[test]
    fn build_chat_messages_includes_all_zones() {
        let msgs: Vec<Message> = (1..=200).map(|i| {
            let role = if i % 2 == 1 { "user" } else { "assistant" };
            msg(i, role, "x")
        }).collect();
        let e = epoch(1, 1, 31, 60, "[memory]");
        let p = partition(&msgs, &[e], "");
        let chat = build_chat_messages("SYS", &p, None, Some("hello"));
        // 1 system + 30 anchor + 1 epoch + 40 bare middle + 100 recent + 1 appended user = 173
        assert_eq!(chat.len(), 173);
        assert_eq!(chat[0].role, "system");
        assert_eq!(chat[chat.len() - 1].content, "hello");
    }

    #[test]
    fn canvas_inserts_after_anchor() {
        let msgs: Vec<Message> = (1..=10).map(|i| msg(i, "user", "x")).collect();
        let p = partition(&msgs, &[], "facts: brass strips matter");
        let chat = build_chat_messages("SYS", &p, None, Some("hi"));
        // 1 sys + 10 anchor + 1 canvas + 1 user = 13
        assert_eq!(chat.len(), 13);
        assert_eq!(chat[11].role, "assistant");
        assert!(chat[11].content.contains("brass strips"));
    }

    #[test]
    fn empty_canvas_omits_insertion() {
        let msgs: Vec<Message> = (1..=10).map(|i| msg(i, "user", "x")).collect();
        let p = partition(&msgs, &[], "   ");
        let chat = build_chat_messages("SYS", &p, None, Some("hi"));
        // 1 sys + 10 anchor + 0 canvas + 1 user = 12
        assert_eq!(chat.len(), 12);
    }

    #[test]
    fn token_estimation_reasonable() {
        let s = "hello world";
        let t = estimate_tokens(s);
        // 11 chars / 4 = ~3 tokens
        assert!(t >= 2 && t <= 4);
    }

    #[test]
    fn partition_token_totals_add_up() {
        let msgs: Vec<Message> = (1..=200).map(|i| msg(i, "user", "twelve chars")).collect();
        let p = partition(&msgs, &[], "");
        assert_eq!(
            p.total_tokens(),
            p.anchor_tokens() + p.middle_tokens() + p.recent_tokens()
        );
    }

    #[test]
    fn recent_budget_trims_oldest_keeps_newest() {
        // Each recent message ~1000 tokens (3997 chars). Expected kept derives
        // from the live budget, so this survives tuning CONTEXT_SEND_BUDGET_TOKENS.
        let big = "a".repeat(3997);
        let per = estimate_tokens(&big); // exactly 1000
        let recent: Vec<Message> = (1..=100).map(|i| msg(i, "user", &big)).collect();
        let keep_start = recent_keep_start(0, &recent);
        let kept = recent.len() - keep_start;
        let expected = (CONTEXT_SEND_BUDGET_TOKENS / per).clamp(MIN_RECENT_MESSAGES, 100);
        assert_eq!(kept, expected, "kept {} expected {}", kept, expected);
        // The KEPT slice is the newest tail — its last element is the newest msg.
        assert_eq!(recent[keep_start].id, (100 - kept as i64 + 1));
        assert_eq!(recent.last().unwrap().id, 100);
    }

    #[test]
    fn recent_budget_respects_floor() {
        // Messages far larger than the whole budget still keep MIN_RECENT_MESSAGES
        // so immediate coherence survives.
        let huge = "a".repeat(400_000);
        let recent: Vec<Message> = (1..=50).map(|i| msg(i, "user", &huge)).collect();
        let kept = recent.len() - recent_keep_start(0, &recent);
        assert_eq!(kept, MIN_RECENT_MESSAGES);
    }

    #[test]
    fn build_chat_messages_trims_recent_over_budget() {
        let big = "a".repeat(3997); // ~1000 tokens each
        let mut msgs: Vec<Message> = (1..=30).map(|i| msg(i, "user", "x")).collect();
        msgs.extend((31..=130).map(|i| msg(i, "user", &big)));
        let p = partition(&msgs, &[], "");
        let chat = build_chat_messages("SYS", &p, None, Some("hi"));
        // Big recent messages that survived the budget (content longer than any
        // anchor "x" / system / appended "hi").
        let recent_sent = chat.iter().filter(|m| m.content.len() > 100).count();
        // Trimming happened (< 100 recent) but the floor held.
        assert!(recent_sent < 100, "recent_sent {}", recent_sent);
        assert!(recent_sent >= MIN_RECENT_MESSAGES, "recent_sent {}", recent_sent);
        // Small content (30 anchor) is never trimmed.
        let anchor_sent = chat.iter().filter(|m| m.content == "x").count();
        assert_eq!(anchor_sent, 30);
    }

    #[test]
    fn seam_guard_drops_leading_assistant_after_assistant_block() {
        // Canvas is injected as an assistant turn; if the recent zone then opens
        // on an assistant, the leading one is dropped so recent opens on a user
        // turn (no assistant→assistant seam).
        let anchor: Vec<Message> = (1..=30).map(|i| msg(i, "user", "x")).collect();
        let recent: Vec<Message> = (0..20)
            .map(|i| msg(100 + i, if i == 0 { "assistant" } else { "user" },
                        if i == 0 { "DROPME" } else { "keep" }))
            .collect();
        let p = Partition { anchor, canvas: "a note".into(), middle: vec![], recent };
        let chat = build_chat_messages("SYS", &p, None, Some("hi"));
        // The leading assistant (DROPME) is gone; the 19 following turns remain.
        assert!(!chat.iter().any(|m| m.content == "DROPME"));
        assert_eq!(chat.iter().filter(|m| m.content == "keep").count(), 19);
    }

    #[test]
    fn seam_guard_keeps_assistant_when_preceding_is_user() {
        // No canvas / no epochs: recent follows the anchor directly. The anchor
        // ends on a user turn, so a recent zone opening on assistant is proper
        // alternation and must NOT be trimmed.
        let anchor: Vec<Message> = (1..=30).map(|i| msg(i, "user", "x")).collect();
        let recent: Vec<Message> = (0..20)
            .map(|i| msg(100 + i, if i == 0 { "assistant" } else { "user" },
                        if i == 0 { "KEEPME" } else { "k" }))
            .collect();
        let p = Partition { anchor, canvas: String::new(), middle: vec![], recent };
        let chat = build_chat_messages("SYS", &p, None, Some("hi"));
        assert!(chat.iter().any(|m| m.content == "KEEPME"));
    }

    #[test]
    fn recall_reserve_keeps_trim_point_stable() {
        // The trim point must be IDENTICAL whether or not recall fired — the
        // reserve is unconditional (prefix-cache stability + eligibility
        // computable up front).
        let big = "a".repeat(3997);
        let msgs: Vec<Message> = (1..=130).map(|i| msg(i, "user", &big)).collect();
        let p = partition(&msgs, &[], "");
        let without = build_chat_messages("SYS", &p, None, Some("hi"));
        let with = build_chat_messages("SYS", &p, Some("a recalled block"), Some("hi"));
        // Same number of recent (big) messages either way.
        let count_big = |chat: &Vec<ChatMessage>| chat.iter().filter(|m| m.content.len() > 100).count();
        assert_eq!(count_big(&without), count_big(&with));
    }

    #[test]
    fn recall_merges_into_canvas_turn() {
        // Canvas + recall must be ONE assistant turn (no triple-stacking).
        let anchor: Vec<Message> = (1..=10).map(|i| msg(i, "user", "x")).collect();
        let p = Partition {
            anchor, canvas: "canvas facts".into(), middle: vec![],
            recent: vec![msg(100, "user", "recent")],
        };
        let chat = build_chat_messages("SYS", &p, Some("recalled things"), Some("hi"));
        let merged: Vec<&ChatMessage> = chat.iter()
            .filter(|m| m.content.contains("canvas facts") || m.content.contains("recalled things"))
            .collect();
        assert_eq!(merged.len(), 1, "canvas and recall must merge into one turn");
        assert!(merged[0].content.contains("canvas facts"));
        assert!(merged[0].content.contains("recalled things"));
        assert_eq!(merged[0].role, "assistant");
    }

    #[test]
    fn recall_alone_still_injects_without_canvas() {
        let anchor: Vec<Message> = (1..=10).map(|i| msg(i, "user", "x")).collect();
        let p = Partition {
            anchor, canvas: String::new(), middle: vec![],
            recent: vec![msg(100, "user", "recent")],
        };
        let chat = build_chat_messages("SYS", &p, Some("recalled things"), Some("hi"));
        assert!(chat.iter().any(|m| m.role == "assistant" && m.content.contains("recalled things")));
    }

    #[test]
    fn trimmed_recent_ids_matches_what_is_not_sent() {
        // The eligibility helper and the assembler must agree exactly: a
        // recent id is either sent or reported trimmed, never both/neither.
        // Small anchor ("x") so the >100-char filter counts only recent.
        let big = "a".repeat(3997);
        let mut msgs: Vec<Message> = (1..=30).map(|i| msg(i, "user", "x")).collect();
        msgs.extend((31..=130).map(|i| msg(i, "user", &big)));
        let p = partition(&msgs, &[], "");
        let trimmed = trimmed_recent_ids("SYS", &p, Some("hi"));
        let chat = build_chat_messages("SYS", &p, None, Some("hi"));
        let sent_count = chat.iter().filter(|m| m.content.len() > 100).count();
        assert_eq!(trimmed.len() + sent_count, p.recent.len());
        // Trimmed are the OLDEST ids.
        for (i, id) in trimmed.iter().enumerate() {
            assert_eq!(*id, p.recent[i].id);
        }
    }
}
