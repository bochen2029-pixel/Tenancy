// Multi-layer discriminator for outreach output. Heuristic pass first
// (cheap, deterministic), LLM-scoring pass second (uses a separate
// evaluator persona, classifier-on-output not classifier-on-decision —
// A2-compliant).
//
// Tunables are constants; edit and rebuild. No settings UI.

use anyhow::{anyhow, Result};

use crate::leak;
use crate::llama_client::{ChatMessage, LlamaClient};
use crate::prompts;

pub const TRIM_LENGTH_FLOOR_CHARS: usize = 16;
pub const LLM_SCORE_PASS_THRESHOLD: u8 = 6;

const ACK_TOKENS: &[&str] = &[
    "yeah", "yes", "right", "ok", "okay", "mhm", "sure", "fair", "huh",
    "indeed", "true", "i see", "got it", "noted", "uh huh", "yep", "yup",
    "alright", "cool", "nice", "hm", "hmm",
];

const DEFER_PATTERNS: &[&str] = &[
    "still thinking",
    "let me think",
    "give me a sec",
    "thinking about it",
    "one moment",
    "hold on",
    "let me consider",
    "i'm thinking",
];

#[derive(Debug, Clone, Copy)]
pub enum DropReason {
    Leak,
    Length,
    AckOnly,
    AckThenFiller,
    Defer,
    LlmScore,
    ScorerError,
}

impl DropReason {
    pub fn as_str(self) -> &'static str {
        match self {
            DropReason::Leak => "leak",
            DropReason::Length => "length",
            DropReason::AckOnly => "ack_only",
            DropReason::AckThenFiller => "ack_then_filler",
            DropReason::Defer => "defer",
            DropReason::LlmScore => "llm_score",
            DropReason::ScorerError => "scorer_error",
        }
    }
}

/// First-pass deterministic filter. Cheap, runs before any LLM cost.
/// Returns Ok(()) on pass, Err(reason) on drop.
pub fn heuristic_pass(text: &str) -> std::result::Result<(), DropReason> {
    let trimmed = text.trim();

    // Leak (A7)
    if leak::is_harness_leak(trimmed) {
        return Err(DropReason::Leak);
    }

    // Length floor (chars, not bytes)
    if trimmed.chars().count() < TRIM_LENGTH_FLOOR_CHARS {
        return Err(DropReason::Length);
    }

    let lower = trimmed.to_lowercase();

    // Pure-ack: trimmed equals an ack token (with optional trailing punctuation/whitespace)
    let stripped = lower.trim_end_matches(|c: char| !c.is_alphabetic());
    if ACK_TOKENS.iter().any(|&t| stripped == t) {
        return Err(DropReason::AckOnly);
    }

    // Defer-pattern: starts with a defer pattern AND has no substantive clause after
    for pat in DEFER_PATTERNS {
        if lower.starts_with(pat) {
            let rest = &lower[pat.len()..].trim_start_matches(|c: char| !c.is_alphabetic());
            // If the remainder is empty or short (<24 chars), it's a defer fragment.
            // If it's a longer follow-on clause, the post-defer content carries substance.
            if rest.chars().count() < 24 {
                return Err(DropReason::Defer);
            }
        }
    }

    // Ack-then-filler: starts with an ack token, the remainder is itself ack-shaped
    // or trivial.
    for ack in ACK_TOKENS {
        let prefix_match = lower.starts_with(ack)
            && lower
                .get(ack.len()..ack.len() + 1)
                .map(|c| !c.chars().next().unwrap_or('a').is_alphabetic())
                .unwrap_or(true);
        if prefix_match {
            // Strip the ack and following punctuation/space
            let rest = lower[ack.len()..]
                .trim_start_matches(|c: char| !c.is_alphabetic())
                .trim();
            // If the rest is short OR starts with another ack OR is filler-shaped, drop
            if rest.chars().count() < 20 {
                return Err(DropReason::AckThenFiller);
            }
            if ACK_TOKENS.iter().any(|&t| rest.starts_with(t)) {
                return Err(DropReason::AckThenFiller);
            }
            // Common filler continuations: "that makes sense", "i see what you mean",
            // "i think so too", "i agree", "exactly"
            const FILLER_CONTINUATIONS: &[&str] = &[
                "that makes sense",
                "i see what you mean",
                "i think so",
                "i agree",
                "exactly",
                "for sure",
                "of course",
                "totally",
                "definitely",
                "i hear you",
                "good point",
            ];
            if FILLER_CONTINUATIONS.iter().any(|f| rest.starts_with(f)) {
                // Even with longer follow-ons, if it's a known filler shape, drop
                return Err(DropReason::AckThenFiller);
            }
            // Passed: ack-prefix but substantive remainder. Don't drop on ack-prefix
            // alone — Dave saying "yeah, the brass strip in the floor of the Royal
            // Exchange has always struck me as..." is a real reach.
            break;
        }
    }

    Ok(())
}

/// Second-pass LLM scoring. Uses a separate evaluator persona to score
/// the text 0-9. Returns the score, or an error if scoring fails.
///
/// This is classifier-on-output (legitimate per A2). The evaluator never
/// sees Dave's persona and never asks Dave anything; it only rates text.
pub async fn llm_score(client: &LlamaClient, text: &str) -> Result<u8> {
    let messages = vec![
        ChatMessage {
            role: "system".into(),
            content: prompts::DISCRIMINATOR_SYSTEM_PROMPT.into(),
        },
        ChatMessage {
            role: "user".into(),
            content: text.to_string(),
        },
    ];

    // Single-token-ish completion with low temperature for determinism.
    let response = client.complete(messages, 4, 0.1).await?;

    parse_score(&response).ok_or_else(|| anyhow!("could not parse score from: {:?}", response))
}

fn parse_score(response: &str) -> Option<u8> {
    response
        .trim()
        .chars()
        .find(|c| c.is_ascii_digit())
        .and_then(|c| c.to_digit(10))
        .map(|d| d.min(9) as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drops_pure_ack() {
        assert!(matches!(heuristic_pass("yeah"), Err(DropReason::Length)));
        assert!(matches!(heuristic_pass("yeah."), Err(DropReason::Length)));
        assert!(matches!(
            heuristic_pass("yeah, that makes sense to me"),
            Err(DropReason::AckThenFiller)
        ));
    }

    #[test]
    fn drops_short() {
        assert!(matches!(heuristic_pass("ok cool"), Err(DropReason::Length)));
    }

    #[test]
    fn drops_defer() {
        assert!(matches!(
            heuristic_pass("still thinking about it"),
            Err(DropReason::Defer)
        ));
    }

    #[test]
    fn defer_with_substance_passes() {
        // Defer prefix but real content after — should pass heuristic
        assert!(heuristic_pass(
            "still thinking about that brass strip — the way it tilts when rain hits it, the patina is wrong somehow"
        )
        .is_ok());
    }

    #[test]
    fn substantive_passes() {
        assert!(heuristic_pass(
            "the etymology of \"deadline\" is more violent than people realize. a literal line at a prison camp."
        )
        .is_ok());
    }

    #[test]
    fn drops_leak() {
        assert!(matches!(
            heuristic_pass("[pass]"),
            Err(DropReason::Leak)
        ));
    }

    #[test]
    fn parses_score() {
        assert_eq!(parse_score("7"), Some(7));
        assert_eq!(parse_score("7.\n"), Some(7));
        assert_eq!(parse_score("Score: 8"), Some(8));
        assert_eq!(parse_score(" 9 "), Some(9));
        assert_eq!(parse_score("nope"), None);
    }
}
