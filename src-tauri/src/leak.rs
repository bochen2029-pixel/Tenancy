// Defense-in-depth filter for harness vocabulary that should never reach the
// user. Dave's system prompt no longer mentions [pass], [meta], etc., so this
// filter should never fire under normal operation. If it does, that's a
// signal something has regressed in the prompt or model behavior.

const FORBIDDEN_PREFIXES: &[&str] = &["[pass", "[meta", "[outreach", "[decision"];

pub fn is_harness_leak(text: &str) -> bool {
    let trimmed_lower = text.trim_start().to_ascii_lowercase();
    FORBIDDEN_PREFIXES
        .iter()
        .any(|prefix| trimmed_lower.starts_with(prefix))
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
}
