use std::fs;
use std::path::Path;

fn main() {
    // Rerun only when Cargo.toml changes (i.e., when the version is bumped).
    // This avoids unnecessary rebuilds while still syncing README.md on version changes.
    println!("cargo:rerun-if-changed=Cargo.toml");

    let version = env!("CARGO_PKG_VERSION");

    let readme_path = Path::new("README.md");
    if !readme_path.exists() {
        return;
    }

    let content = fs::read_to_string(readme_path).expect("Failed to read README.md");
    let updated = sync_version(&content, version);

    if updated != content {
        fs::write(readme_path, &updated).expect("Failed to write README.md");
    }
}

// Replaces every occurrence of `prefix + <old_version>` with `prefix + new_version`.
// The old version is detected as the longest run of ASCII digits and dots that follows
// the prefix.  All other content (including CHANGELOG history) is left untouched.
fn replace_version_after_prefix(content: &str, prefix: &str, new_version: &str) -> String {
    let mut result = String::with_capacity(content.len());
    let mut remaining = content;

    while let Some(pos) = remaining.find(prefix) {
        // Append everything up to and including the prefix
        result.push_str(&remaining[..pos + prefix.len()]);

        let after_prefix = &remaining[pos + prefix.len()..];

        // Find where the version number ends (first char that isn't a digit or '.')
        let version_end = after_prefix
            .find(|c: char| !c.is_ascii_digit() && c != '.')
            .unwrap_or(after_prefix.len());

        // Append the new version in place of the old one
        result.push_str(new_version);

        remaining = &after_prefix[version_end..];
    }

    result.push_str(remaining);
    result
}

fn sync_version(content: &str, version: &str) -> String {
    let content = replace_version_after_prefix(content, "# CCaps Layout Switcher v", version);
    let content = replace_version_after_prefix(&content, "\"version\": \"", version);
    replace_version_after_prefix(&content, "- **Version**: ", version)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_replace_title_version() {
        let input = "# CCaps Layout Switcher v0.8.2\n\nSome text";
        let result = sync_version(input, "0.9.0");
        assert!(result.starts_with("# CCaps Layout Switcher v0.9.0\n"));
    }

    #[test]
    fn test_replace_json_version() {
        let input = "  \"version\": \"0.8.2\"\n";
        let result = sync_version(input, "0.9.0");
        assert_eq!(result, "  \"version\": \"0.9.0\"\n");
    }

    #[test]
    fn test_replace_metadata_version() {
        let input = "- **Version**: 0.8.2\n";
        let result = sync_version(input, "0.9.0");
        assert_eq!(result, "- **Version**: 0.9.0\n");
    }

    #[test]
    fn test_no_change_when_version_matches() {
        let input = "# CCaps Layout Switcher v0.9.0\n";
        let result = sync_version(input, "0.9.0");
        assert_eq!(result, input);
    }

    #[test]
    fn test_changelog_entries_not_touched() {
        let input = "### v0.8.2\n### v0.8.1\n- **Version**: 0.8.2\n";
        let result = sync_version(input, "0.9.0");
        // CHANGELOG history lines start with "### v" — not matched by any prefix
        assert!(result.contains("### v0.8.2\n"));
        assert!(result.contains("### v0.8.1\n"));
        // Metadata line IS replaced
        assert!(result.contains("- **Version**: 0.9.0\n"));
    }

    #[test]
    fn test_replaces_all_occurrences_of_same_prefix() {
        let input = "- **Version**: 0.1.0\n- **Version**: 0.1.0\n";
        let result = sync_version(input, "1.0.0");
        assert_eq!(result, "- **Version**: 1.0.0\n- **Version**: 1.0.0\n");
    }
}
