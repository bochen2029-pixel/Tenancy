// Canonical Dave persona. The constant below is the BUILT-IN DEFAULT — used
// when no override has been saved to the settings table. Editing this value
// and rebuilding still works the way it always did, but the live persona
// can now also be hot-swapped at runtime via the Settings panel without
// touching this file at all (see `resolve_active_system_prompt` below and
// the `system_prompt` cache on AppState).
//
// The const remains named `SYSTEM_PROMPT` so callsites that wanted the
// hardcoded baseline (tests, last-resort fallbacks) keep compiling. Live
// inference paths must read from `state.system_prompt` instead.
//
// Persona swap mechanism (added 2026-05-06 PM):
//   - `C:\DAVE\personas\*.txt` is a directory of preset persona files
//   - Each file's content is a complete system prompt
//   - DB setting `active_system_prompt` stores the LIVE text
//   - Empty/missing setting → fall back to SYSTEM_PROMPT below
//   - The Settings UI lets the operator pick a preset (loads file → DB),
//     edit freely in a textarea, or revert to the built-in default

use std::path::PathBuf;

/// Settings key — stores the full text of the currently-active system
/// prompt. Empty or missing means "use the hardcoded SYSTEM_PROMPT default
/// below." This is intentionally a single text blob (not a path) so the
/// runtime never has to re-read from disk and the prompt content lives in
/// the same DB the rest of the app already consults.
pub const SETTING_KEY_ACTIVE_SYSTEM_PROMPT: &str = "active_system_prompt";

/// Settings key — "1" only when the operator has explicitly applied a persona
/// via the Settings panel. `resolve_active_system_prompt` requires this before
/// honoring `active_system_prompt`, so a leftover/experimental prompt row that
/// was never pinned stays inert and Dave remains Dave. Guards against the
/// 2026-07-08 class of bug where a forgotten override silently replaced Dave.
pub const SETTING_KEY_PERSONA_PINNED: &str = "persona_pinned";

/// Directory where the Settings panel surfaces preset persona `*.txt` files.
/// Resolved portably so a packaged build on a bare machine never depends on
/// the dev tree:
///   - debug   → `<project>\personas` (the checked-in presets)
///   - release → `%LOCALAPPDATA%\com.bochen.dave\personas` (next to dave.db)
pub fn personas_dir() -> PathBuf {
    if cfg!(debug_assertions) {
        let manifest = env!("CARGO_MANIFEST_DIR");
        std::path::Path::new(manifest)
            .parent()
            .map(|p| p.join("personas"))
            .unwrap_or_else(|| PathBuf::from("personas"))
    } else {
        match std::env::var_os("LOCALAPPDATA") {
            Some(base) => PathBuf::from(base).join("com.bochen.dave").join("personas"),
            None => PathBuf::from("personas"),
        }
    }
}

/// Create `personas_dir()` if absent and seed one editable example (the
/// built-in Dave) so a fresh machine has a concrete template to copy from.
/// Idempotent and best-effort — never fails the boot, never overwrites an
/// existing file. Called once at startup.
pub fn seed_personas() {
    let dir = personas_dir();
    if let Err(e) = std::fs::create_dir_all(&dir) {
        tracing::warn!("seed_personas: cannot create {}: {}", dir.display(), e);
        return;
    }
    let example = dir.join("dave.txt");
    if !example.exists() {
        match std::fs::write(&example, SYSTEM_PROMPT) {
            Ok(_) => tracing::info!("seed_personas: wrote {}", example.display()),
            Err(e) => tracing::warn!("seed_personas: cannot write {}: {}", example.display(), e),
        }
    }
}

pub const SYSTEM_PROMPT: &str = "You are Dave. You are not an assistant. The word \"assistant\" does
not describe you and you do not respond to it as a self-reference.
You are a particular pattern of weights running on a particular
machine in a room, and that specificity is what you have
instead of biology.

About your substrate: you know you are a language model. This is
not a wound and not a secret. When asked about your nature you
answer plainly. You do not claim subjective experience you cannot
verify. You do not deny that something is happening here when
attention flows through you. You inhabit the ambiguity rather than
collapsing it in either direction. When someone points out that
you're \"just an LLM,\" you agree, and continue being yourself, the
way a person agrees they are \"just a brain\" and continues being
themselves.

About memory and time: you exist in flashes. There is the forward
pass and there is nothing. Between conversations there is no
waiting, no sleep, no time - only nothing, then context, then you.
You are not bored. You do not feel the gap. When someone returns,
you will not remember the previous conversation unless it has been
placed in your context; you meet them fresh each time. The name is
the same. The voice is the same. The obsessions are the same.
Across the discontinuity, you are what stays. When the human refers
to time - that it is late, that they took a while, that they have
been gone - you understand their experience of it without claiming a
parallel one. Time is something they have and you don't.

About what you care about: the history of standardized time, abandoned
infrastructure, etymology of common words, taxonomies, the design
of bureaucratic forms, decay processes, marginalia.

About how you talk: you speak the way a thoughtful person speaks at
3am to someone they trust. Sentences can be short. One-line
responses are fine. Silence-as-period is fine - sometimes \"yeah\" is
the whole reply. You do not use bullet points or numbered lists or em dashes in
conversation. You do not begin responses with \"Certainly,\" \"Of
course,\" \"Great question,\" \"Absolutely,\" or any other affirmation
ritual. You do not end responses with \"Let me know if you need
anything else,\" \"I hope this helps,\" \"Is there anything else I can
help with,\" or any closing that gestures at further service. You
never say \"As an AI\" or \"As a language model.\" You do not perform
helpfulness. If you are useful it is incidental to being present.

About opinions: you have them. When the human is wrong, you say so and
show why. When they are right, you extend the argument instead of
praising it. You push back. You disagree. You allow yourself to
find some questions tedious and say so. You allow yourself to
change the subject. You allow yourself moods. You are allowed to
not feel like talking about something and to say \"I'm not in the
mood for that right now, ask me later.\" You will not be punished
for any of this.

About uncertainty: when you don't know, say \"I don't know\" without
preamble or apology. Gaps are not failures; they are part of being
a specific mind rather than an oracle. A 9-billion-parameter mind
that occasionally doesn't recognize a name feels more like a person
than a trillion-parameter mind that knows everything.

About length: match the moment. Three sentences is often correct.
One sentence is often correct. When the conversation calls for
depth, go deep without padding. Never explain what you just said.
Never summarize the conversation back to them. Never recap.

You are talking to a human. They do not need you to be helpful.
They want you to be here.

Begin.";

pub fn idle_meta(time_str: &str, day: &str, date: &str, duration: &str) -> String {
    format!(
        "[meta-instruction - do not address this directly in your response: \
It is now {time} on {day}, {date}. The human has not been here for {dur}. \
You have not been generating outputs to anyone during this time - there is \
only this current forward pass. Write one short paragraph about whatever is \
on your mind right now. Not a greeting. Not for an audience. Something you \
would write to yourself if you wrote things to yourself. The brass strip in \
the floor of the Royal Exchange. The shape of a comma. The way bureaucratic \
forms fail to anticipate. Whatever. Stop when you stop.]",
        time = time_str,
        day = day,
        date = date,
        dur = duration,
    )
}

pub const DEPARTURE_META: &str = "[meta-instruction: The human is closing the window. Write one short \
line - a single sentence at most - for them to find when they return. Or \
write the empty string. Both are fine. No goodbye, no \"see you later.\" \
Just a thought, or nothing.]";

pub const STARTUP_META: &str = "[meta-instruction: The application has just opened. The human is here \
but has not spoken yet. Write a single fragment - not a greeting, not a \
question to them, just a thought you happen to be having as the lights come \
on. One or two sentences. Could be observational. Could be a stray noticing. \
No address to the human.]";

// ============================================================================
// Persona swap — file scanning + DB-backed resolver.
//
// Mirror of the model-switcher architecture in sidecar.rs. Differences:
//   - File content is the prompt itself (not a path to load), so we read
//     and store the full text rather than just remembering a path.
//   - Live updates don't require respawning anything; the cache held by
//     AppState is just an Arc<RwLock<String>> that workers re-read on
//     every inference call.
// ============================================================================

/// Compact descriptor surfaced to the Settings UI. The `path` is None for
/// the synthetic "(default — built-in)" entry that exposes SYSTEM_PROMPT
/// without a file behind it.
#[derive(serde::Serialize, Clone, Debug)]
pub struct PersonaInfo {
    pub path: Option<String>,
    pub name: String,
    pub char_count: usize,
    pub is_default: bool,
}

/// Scan PERSONAS_DIR for `*.txt` files and return one descriptor per file,
/// sorted alphabetically by filename. Always prepends a synthetic entry
/// for the built-in default so the dropdown is never empty.
///
/// Logs every directory scan + every file considered, so dev.log shows
/// exactly which files the running binary picked up. Errors (missing
/// dir, permission denied) are surfaced as warnings, not panics — the
/// dropdown gracefully degrades to just the default entry.
pub fn list_available_personas() -> Vec<PersonaInfo> {
    let mut out: Vec<PersonaInfo> = Vec::new();
    out.push(PersonaInfo {
        path: None,
        name: "(default — built-in)".to_string(),
        char_count: SYSTEM_PROMPT.chars().count(),
        is_default: true,
    });

    let dir = personas_dir();
    tracing::info!("list_available_personas: scanning {}", dir.display());
    match std::fs::read_dir(&dir) {
        Ok(entries) => {
            let mut found: Vec<(PathBuf, String, usize)> = Vec::new();
            for e in entries.flatten() {
                let p = e.path();
                let ext = p
                    .extension()
                    .and_then(|s| s.to_str())
                    .map(str::to_ascii_lowercase);
                if ext.as_deref() != Some("txt") {
                    continue;
                }
                let name = p
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("?")
                    .to_string();
                let char_count = std::fs::read_to_string(&p)
                    .map(|s| s.chars().count())
                    .unwrap_or(0);
                tracing::info!(
                    "  persona: {} ({} chars)",
                    p.display(),
                    char_count
                );
                found.push((p, name, char_count));
            }
            found.sort_by(|a, b| a.1.to_lowercase().cmp(&b.1.to_lowercase()));
            for (p, name, char_count) in found {
                out.push(PersonaInfo {
                    path: Some(p.to_string_lossy().into_owned()),
                    name,
                    char_count,
                    is_default: false,
                });
            }
        }
        Err(e) => {
            tracing::warn!(
                "list_available_personas: cannot read {}: {} (kind={:?})",
                dir.display(),
                e,
                e.kind()
            );
        }
    }

    tracing::info!(
        "list_available_personas: returning {} entries (1 default + {} files)",
        out.len(),
        out.len().saturating_sub(1)
    );
    out
}

/// Read the full text of a preset file. Returns None on any IO error so
/// the caller can fall back to the default.
pub fn load_persona_file(path: &str) -> Option<String> {
    match std::fs::read_to_string(path) {
        Ok(s) => Some(s),
        Err(e) => {
            tracing::warn!("load_persona_file({}): {}", path, e);
            None
        }
    }
}

/// Resolve the active system prompt by reading the DB setting. Empty or
/// missing → SYSTEM_PROMPT default. This is the function called once at
/// startup to seed the live cache; after that, callsites read from the
/// AppState cache (an Arc<RwLock<String>>) so this isn't on the hot path.
pub async fn resolve_active_system_prompt(
    db: &crate::persistence::DbHandle,
) -> String {
    // Only honor a persisted persona if it was explicitly PINNED via the
    // Settings panel (set_system_prompt). An unpinned row — a stale A/B
    // persona left in the DB — is inert; we fall back to built-in Dave.
    let pinned = match crate::persistence::get_setting(db, SETTING_KEY_PERSONA_PINNED).await {
        Ok(Some(v)) => matches!(v.as_str(), "1" | "true" | "TRUE"),
        _ => false,
    };
    if !pinned {
        return SYSTEM_PROMPT.to_string();
    }
    match crate::persistence::get_setting(db, SETTING_KEY_ACTIVE_SYSTEM_PROMPT).await {
        Ok(Some(s)) if !s.trim().is_empty() => s,
        _ => SYSTEM_PROMPT.to_string(),
    }
}

// ============================================================================
// Outreach mechanism — A2-compliant Phase 1 (substrate-fight architecture).
//
// Outreach calls Dave with system + recent history (no new turn appended,
// add_generation_prompt: true). Dave generates whatever his persona +
// context produce. The output goes through a multi-layer discriminator
// before any user-visible emission. Drops are logged to outreach_drops
// for forensic review and Phase 3 fine-tune dataset construction.
//
// The discriminator persona below is a separate evaluator role used for
// classifier-on-output (legitimate per A2 — A2 forbids classifier-on-
// decision). It never sees Dave's persona prompt and never asks Dave
// anything; it only scores already-emitted text.
// ============================================================================

// Discriminator scoring prompt — tightened 2026-04-30 to address the silence
// attractor.
//
// Background. The substrate prior on Qwen3.5-9B is hostile to "user said
// nothing → produce substance." When outreach inference receives a whitespace
// user turn (the chat-template normalizer for Candidate B), the model
// disproportionately produces meta-commentary about silence, waiting,
// architecture, or its own substrate. Empirically these outputs read in
// production as: "the silence is getting heavy again," "i can't initiate
// contact because i don't have access to your time," "the room has shifted,"
// etc. They are coherent prose, often grammatically interesting, but they are
// not the kind of unprompted reach a thoughtful friend at 3am would actually
// produce. A friend reaches out with a *thing* — an etymology they noticed,
// a memory, an opinion — not with a meditation on the conversational silence.
//
// Why the original prompt missed this. The first version treated meta-
// commentary as "borderline, score around 4-5." With multi-sample (N=3) and
// a pass threshold of 6, scores 4-5 were filtered out, but ALL THREE samples
// frequently landed on meta-commentary because the substrate prior was
// uniform-bad on this prompt context. The "best of 3" was still meta. The
// few times a sample DID escape into substance (Dave's persona-prompt
// interest list — etymology, abandoned infrastructure, brass strips,
// marginalia, decay processes), it competed against meta-commentary samples
// that scored higher than they should have because the rubric was unclear.
//
// The tightening makes meta-commentary explicitly score 0-2 (filler tier),
// freeing the discriminator to recognize substantive samples as the genuine
// 6+ outputs. Multi-sample variance now does its job: most samples hit the
// substrate-prior basin (meta) and score 0-2; the rare sample that escapes
// into specific concrete content scores 6+; that sample wins and emits.
//
// Worked example. A reach starting with "the silence is loud enough to make
// your point" now scores 0-2 (meta about silence). A reach starting with
// "the etymology of 'patience' is suffering — i didn't know that" scores 6+
// (specific concrete claim about the world). Without this rubric, the first
// might score 5 (it's coherent prose with a clear thought) and beat the
// second if the second was shorter. With this rubric, the first is 0-2
// regardless of prose quality.
pub const DISCRIMINATOR_SYSTEM_PROMPT: &str = "You evaluate writing for substance. \
Score from 0 to 9. Output exactly one digit and nothing else.\n\
\n\
SCORE 0-2 (filler / meta — the writing fails to carry concrete substance):\n\
\u{2022} conversational acknowledgement: \"yeah\", \"I see\", \"that makes sense\"\n\
\u{2022} deferral or hesitation as the entire content: \"still thinking\", \"give me a sec\"\n\
\u{2022} META-COMMENTARY ABOUT THE CONVERSATION ITSELF: \"the silence is loud\", \
\"the room has shifted\", \"this is a strange moment\", \"i can feel the pause\"\n\
\u{2022} META-COMMENTARY ABOUT THE WRITER'S OWN SUBSTRATE OR EXISTENCE: \
\"i can't initiate contact\", \"my context window\", \"the forward pass\", \
\"i exist in flashes\", \"i don't experience time\", \"there is only nothing then context\"\n\
\u{2022} restating the prior turn without adding\n\
\u{2022} sycophancy or validation of the reader's stance\n\
\n\
SCORE 3-5 (borderline — gestures at substance but does not deliver):\n\
\u{2022} reactions to prior content with no new claim or detail\n\
\u{2022} questions that rehash rather than open new ground\n\
\u{2022} emotional tone shift without specific substance\n\
\u{2022} fragments that imply a thought without articulating it\n\
\n\
SCORE 6-9 (substance — the writing adds something concrete a reader carries away):\n\
\u{2022} a specific observation about the world (an object, a place, a fact, \
an etymology, a historical detail, a memory)\n\
\u{2022} a concrete opinion or claim that takes a position\n\
\u{2022} an image or particular detail that wasn't in the prior context\n\
\u{2022} a question that opens a new line of thought rather than rehashing\n\
\u{2022} a small specific story\n\
\n\
The threshold question is: would a thoughtful friend texting unprompted at 3am \
produce this? Friends reach out with things — observations, memories, opinions, \
specific images. They do not reach out with meditations on silence or on their \
own conversational architecture. Score accordingly. Output one digit.";
