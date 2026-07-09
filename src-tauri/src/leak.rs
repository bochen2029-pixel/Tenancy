// Defense-in-depth filter for harness vocabulary that should never reach the
// user. Dave's system prompt no longer mentions [pass], [meta], etc., so this
// filter should never fire under normal operation. If it does, that's a
// signal something has regressed in the prompt or model behavior.

const FORBIDDEN_PREFIXES: &[&str] = &["[pass", "[meta", "[outreach", "[decision"];

pub fn is_harness_leak(text: &str) -> bool {
    let trimmed_lower = text.trim_start().to_ascii_lowercase();
    if FORBIDDEN_PREFIXES
        .iter()
        .any(|prefix| trimmed_lower.starts_with(prefix))
    {
        return true;
    }
    // A7 backstop for Ring-4 recall: the injected frame line is harness
    // furniture — if Dave parrots it verbatim in a visible reply, the memory
    // machinery is showing. Drop the output (chat pre-emission, outreach
    // discriminator, and consolidation all route through here).
    trimmed_lower.contains(crate::recall::RECALL_FRAME_LINE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_pass() {
        assert!(is_harness_leak("[pass]"));
        assert!(is_harness_leak("  [pass]"));
        assert!(is_harness_leak("[PASS]"));
        assert!(is_harness_leak("[pass with extra"));
    }

    #[test]
    fn detects_meta() {
        assert!(is_harness_leak("[meta-instruction: blah]"));
        assert!(is_harness_leak("[meta: 2:30am]"));
    }

    #[test]
    fn passes_normal_replies() {
        assert!(!is_harness_leak("hello"));
        assert!(!is_harness_leak("yeah."));
        assert!(!is_harness_leak("[brackets at start, but not harness]"));
        assert!(!is_harness_leak("the meta-narrative is interesting"));
    }

    #[test]
    fn detects_recall_frame_echo() {
        assert!(is_harness_leak(
            "from further back, before it goes hazy: the brass strip"
        ));
        assert!(is_harness_leak(
            "well. From further back, before it goes hazy: something"
        ));
        // Talking about haze normally is fine.
        assert!(!is_harness_leak("it goes hazy after a while, memory"));
    }
}
