//! Text handling for package names and descriptions, shared by the GUI and the CLI.

/// Truncate description to at most `max_len` characters, taking only the first line
#[must_use]
pub fn truncate_description(desc: &str, max_len: usize) -> String {
    let first_line = desc.split('\n').next().unwrap_or("").trim_end();

    if first_line.chars().count() <= max_len {
        first_line.to_string()
    } else {
        // Count characters, not bytes, so multi-byte text is never cut mid-character.
        let kept: String = first_line.chars().take(max_len.saturating_sub(3)).collect();
        format!("{kept}...")
    }
}

/// Check if package matches search term (checks name and description)
#[must_use]
pub fn matches_search(pkg_name: &str, search_term: &str, pkg_description: Option<&str>) -> bool {
    let search_lower = search_term.to_lowercase();

    pkg_name.to_lowercase().contains(&search_lower)
        || pkg_description.is_some_and(|d| d.to_lowercase().contains(&search_lower))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_description_handles_multibyte_text() {
        assert_eq!(truncate_description("short", 80), "short");
        assert_eq!(truncate_description("first\nsecond", 80), "first");
        // 'é' is 2 bytes. Byte slicing at 3 would panic mid-character.
        assert_eq!(truncate_description("ééééé", 4), "é...");
        assert_eq!(
            truncate_description("Huawei\u{FFFD}s mobile services", 8),
            "Huawe..."
        );
    }

    #[test]
    fn search_matches_name_or_description_ignoring_case() {
        assert!(matches_search("com.Samsung.Bixby", "bixby", None));
        assert!(matches_search("com.x", "VOICE", Some("A voice assistant")));
        assert!(!matches_search("com.x", "voice", None));
    }
}
