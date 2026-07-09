// Ring 4 recall — REEL Op 4 (retrieval), Tier 2 (application-layer).
//
// Pulls small verbatim segments of the Tape (messages / journal) into context
// when the current turn touches them. The Tape is the append-only source of
// truth; recall injects QUOTES, never regenerated paraphrase (REEL §10.5 —
// false-memory guard). The block rides inside the canvas memory turn (one
// assistant turn after the anchor zone; memory_assembler merges them).
//
// GATED, not per-turn (A8 review): an every-turn recall block would bust
// llama-server's prefix cache at the canvas position and re-evaluate ~40k
// tokens per turn — latency IS presence, so recall fires only when the turn
// carries real signal:
//   - an explicit remember-cue ("remember", "you told me", "what was that…"), or
//   - at least one RARE content term (document-frequency gate), and
//   - candidates must match ≥2 query terms unless rare/cued.
// Eligible sources: messages OUT of the assembled context (epoch-covered
// middle + budget-trimmed recent — computed exactly via
// memory_assembler::trimmed_recent_ids) and journal entries. Bare middle and
// kept-recent messages are already verbatim in context; active epochs too.
//
// The seam: `maybe_recall` is the whole surface. A semantic implementation
// (llama.cpp embeddings, the 0.6B reranker, or KEEL's memory service on :7070
// once it pins a release) replaces the internals without touching callers —
// same discipline as TimingModel.

use std::collections::HashSet;

use crate::memory_assembler::{estimate_tokens, RECALL_RESERVE_TOKENS};
use crate::persistence::{self, DbHandle};

/// Opens the recall block. Also a defense-in-depth constant: leak.rs drops any
/// visible Dave output containing it verbatim (A7 — the injected frame must
/// never surface in his voice).
pub const RECALL_FRAME_LINE: &str = "from further back, before it goes hazy:";

/// Kill switch (no UI): `INSERT OR REPLACE INTO settings VALUES
/// ('recall_enabled','0')` disables recall entirely.
const SETTING_KEY_RECALL_ENABLED: &str = "recall_enabled";

const MAX_EXCERPTS: usize = 3;
const CANDIDATE_LIMIT: i64 = 12;
const MAX_QUERY_TERMS: usize = 12;
const EXCERPT_MSG_MAX_CHARS: usize = 300;
const JOURNAL_MAX_CHARS: usize = 400;
/// A term is "rare" if it appears in at most this fraction of Tape docs
/// (floored at 3 docs so tiny corpora still pass sensible terms).
const RARE_DF_DIVISOR: i64 = 25;

const REMEMBER_CUES: &[&str] = &[
    "remember",
    "you told me",
    "you said",
    "you mentioned",
    "we talked about",
    "we discussed",
    "what was that",
    "what was the",
    "last time",
    "back when",
    "that thing about",
    "a while ago",
    "earlier you",
];

const STOPWORDS: &[&str] = &[
    "the", "and", "for", "that", "this", "with", "you", "your", "was", "were",
    "are", "have", "has", "had", "but", "not", "all", "can", "could", "would",
    "should", "what", "when", "where", "which", "who", "how", "why", "there",
    "here", "just", "like", "about", "into", "over", "under", "then", "than",
    "them", "they", "their", "its", "his", "her", "she", "him", "out", "get",
    "got", "one", "two", "some", "any", "more", "most", "very", "really",
    "been", "being", "will", "dont", "don't", "didnt", "didn't", "does",
    "doesnt", "doesn't", "yeah", "yes", "okay", "still", "also", "again",
    "know", "think", "want", "going", "thing", "things", "time", "good",
    "well", "right", "make", "made", "say", "said", "told", "tell", "much",
    "even", "only", "now", "today", "tonight", "morning", "little", "bit",
];

/// The exactly-not-in-context set: message ids covered by an active epoch,
/// plus ids trimmed from the recent zone by the token budget. Journal entries
/// are always eligible (never in chat context).
pub struct RecallEligibility {
    pub epoch_ranges: Vec<(i64, i64)>,
    pub trimmed_ids: HashSet<i64>,
}

impl RecallEligibility {
    pub fn eligible_message(&self, id: i64) -> bool {
        if self.trimmed_ids.contains(&id) {
            return true;
        }
        self.epoch_ranges.iter().any(|(lo, hi)| id >= *lo && id <= *hi)
    }
}

fn has_remember_cue(lower: &str) -> bool {
    REMEMBER_CUES.iter().any(|c| lower.contains(c))
}

/// Lowercased content words: alphanumeric runs, len ≥ 3, not stopwords,
/// deduped, order-preserving, capped.
fn content_terms(lower: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for run in lower.split(|c: char| !c.is_alphanumeric() && c != '\'') {
        let w = run.trim_matches('\'');
        if w.len() < 3 || STOPWORDS.contains(&w) {
            continue;
        }
        if seen.insert(w.to_string()) {
            out.push(w.to_string());
            if out.len() >= MAX_QUERY_TERMS {
                break;
            }
        }
    }
    out
}

fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let cut: String = s.chars().take(max).collect();
    format!("{}…", cut.trim_end())
}

/// How many distinct query terms appear in the content (case-insensitive).
fn matched_terms(content_lower: &str, terms: &[String]) -> usize {
    terms.iter().filter(|t| content_lower.contains(t.as_str())).count()
}

/// The Ring-4 entry point. Returns the formatted recall block, or None when
/// the gate doesn't clear / nothing eligible hits. Never errors outward —
/// recall is an enhancement, and any failure degrades to "no recall."
pub async fn maybe_recall(
    db: &DbHandle,
    conversation_id: i64,
    query_text: &str,
    elig: &RecallEligibility,
) -> Option<String> {
    // Kill switch.
    if let Ok(Some(v)) = persistence::get_setting_blocking(db, SETTING_KEY_RECALL_ENABLED) {
        if v == "0" {
            return None;
        }
    }

    let lower = query_text.to_lowercase();
    let cue = has_remember_cue(&lower);
    let terms = content_terms(&lower);
    if terms.is_empty() || (terms.len() < 2 && !cue) {
        return None;
    }

    // Rare-term gate: without an explicit cue, at least one query term must be
    // rare on the Tape — generic turns don't fish (precision over recall; a
    // wrong memory surfacing is a worse spell-break than none).
    let total_docs = persistence::fts_doc_count(db).await.ok()?;
    if total_docs == 0 {
        return None;
    }
    let ceiling = (total_docs / RARE_DF_DIVISOR).max(3);
    let mut rare_terms: Vec<String> = Vec::new();
    for t in &terms {
        if let Ok(df) = persistence::fts_term_df(db, t).await {
            if df > 0 && df <= ceiling {
                rare_terms.push(t.clone());
            }
        }
    }
    if !cue && rare_terms.is_empty() {
        return None;
    }

    // FTS query: quoted terms OR'd (porter stemming applies inside FTS).
    let match_expr = terms
        .iter()
        .map(|t| format!("\"{}\"", t.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(" OR ");
    let candidates = persistence::recall_candidates(db, &match_expr, CANDIDATE_LIMIT)
        .await
        .ok()?;

    // Assemble excerpts best-first under the budget.
    let budget = RECALL_RESERVE_TOKENS.saturating_sub(estimate_tokens(RECALL_FRAME_LINE) + 8);
    let mut blocks: Vec<String> = Vec::new();
    let mut used_tokens = 0usize;
    let mut covered_msg_ids: HashSet<i64> = HashSet::new();

    for c in &candidates {
        if blocks.len() >= MAX_EXCERPTS {
            break;
        }
        let content_lower = c.content.to_lowercase();
        let matched = matched_terms(&content_lower, &terms);
        let hit_rare = rare_terms.iter().any(|t| content_lower.contains(t.as_str()));
        if !(cue || matched >= 2 || hit_rare) {
            continue;
        }

        let block = match c.kind.as_str() {
            "message" => {
                if !elig.eligible_message(c.ref_id) || covered_msg_ids.contains(&c.ref_id) {
                    continue;
                }
                let window = persistence::load_message_window(db, conversation_id, c.ref_id, 1)
                    .await
                    .unwrap_or_default();
                let mut lines = Vec::new();
                for m in &window {
                    // Only quote lines that are themselves out of context —
                    // a neighbor inside the live window must not duplicate.
                    if m.id != c.ref_id && !elig.eligible_message(m.id) {
                        continue;
                    }
                    covered_msg_ids.insert(m.id);
                    let quoted = truncate_chars(m.content.trim(), EXCERPT_MSG_MAX_CHARS);
                    if quoted.is_empty() {
                        continue;
                    }
                    if m.role == "user" {
                        lines.push(format!("you said: \"{}\"", quoted));
                    } else {
                        lines.push(format!("i said: \"{}\"", quoted));
                    }
                }
                if lines.is_empty() {
                    continue;
                }
                lines.join("\n")
            }
            "journal" => {
                let quoted = truncate_chars(c.content.trim(), JOURNAL_MAX_CHARS);
                if quoted.is_empty() {
                    continue;
                }
                format!("i wrote, at some point: \"{}\"", quoted)
            }
            // Active epochs are already in context this slice; superseded ones
            // are deleted from the index. Skip defensively either way.
            _ => continue,
        };

        let t = estimate_tokens(&block);
        if used_tokens + t > budget {
            continue;
        }
        used_tokens += t;
        blocks.push(block);
    }

    if blocks.is_empty() {
        return None;
    }

    let out = format!("{}\n{}", RECALL_FRAME_LINE, blocks.join("\n\n"));
    let injected = estimate_tokens(&out) as i64;
    tracing::info!(
        "recall: fired (terms=[{}], cue={}, rare=[{}], excerpts={}, ~{} tok)",
        terms.join(","), cue, rare_terms.join(","), blocks.len(), injected
    );
    let _ = persistence::insert_recall_fire(
        db, conversation_id, &terms.join(","), blocks.len() as i64, injected,
    )
    .await;
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cue_detection() {
        assert!(has_remember_cue("do you remember the brass strip"));
        assert!(has_remember_cue("what was that etymology"));
        assert!(!has_remember_cue("good morning"));
    }

    #[test]
    fn content_terms_filter_stopwords_and_short() {
        let t = content_terms("do you remember the etymology of salary from salt");
        assert!(t.contains(&"etymology".to_string()));
        assert!(t.contains(&"salary".to_string()));
        assert!(t.contains(&"salt".to_string()));
        assert!(!t.contains(&"the".to_string()));
        assert!(!t.contains(&"you".to_string()));
        assert!(!t.contains(&"of".to_string())); // len < 3
    }

    #[test]
    fn content_terms_dedupe_and_cap() {
        let repeated = "salt ".repeat(30);
        let t = content_terms(&repeated);
        assert_eq!(t, vec!["salt".to_string()]);
    }

    #[test]
    fn eligibility_epoch_ranges_and_trimmed() {
        let elig = RecallEligibility {
            epoch_ranges: vec![(31, 60), (61, 90)],
            trimmed_ids: [134, 135].into_iter().collect(),
        };
        assert!(elig.eligible_message(31));
        assert!(elig.eligible_message(90));
        assert!(elig.eligible_message(134));
        assert!(!elig.eligible_message(30));   // anchor
        assert!(!elig.eligible_message(100));  // bare middle / kept recent
    }

    #[test]
    fn truncation_appends_ellipsis() {
        let long = "a".repeat(400);
        let t = truncate_chars(&long, 300);
        assert!(t.chars().count() <= 302);
        assert!(t.ends_with('…'));
        assert_eq!(truncate_chars("short", 300), "short");
    }

    #[test]
    fn matched_terms_counts_distinct() {
        let terms = vec!["brass".to_string(), "strip".to_string(), "exchange".to_string()];
        assert_eq!(matched_terms("the brass strip in the floor", &terms), 2);
        assert_eq!(matched_terms("nothing relevant", &terms), 0);
    }
}
