//! The glob the caller names, and the paths it selects.
//!
//! The search and the listing both filter by a glob, and both publish what
//! they selected. The pattern is the caller's, and matching it is a
//! question about the path alone: nothing here opens a file or walks a tree.
//!
//! The two live together because the two are the same question: whether a
//! path is inside the pattern is decided by the pattern's own rules, and a
//! second copy of those rules would answer differently for the search and
//! for the listing.

use std::path::Path;

/// Whether the path matches the glob the caller named.
///
/// The glob is matched against the name of the file, not against its whole
/// path, except for the `**/` form: that one asks for the name the path
/// ends with, which is what a caller who writes `**/*.rs` means.
pub(crate) fn path_matches_glob(path: &Path, glob: &str) -> bool {
    // Simple glob matching - supports **/*.ext patterns
    let path_str = path.to_string_lossy();

    // Handle **/*.rs style patterns
    if let Some(ext_pattern) = glob.strip_prefix("**/") {
        if let Some(path_str) = path_str.rsplit('/').next() {
            return glob_match(path_str, ext_pattern);
        }
    } else if let Some(_ext_pattern) = glob.strip_prefix("*.") {
        // Handle *.rs style patterns
        if let Some(path_str) = path_str.rsplit('/').next() {
            return glob_match(path_str, glob);
        }
    }

    glob_match(&path_str, glob)
}

/// Whether the name matches the pattern of the glob.
///
/// The patterns it answers are the ones a caller writes for a file
/// filter: `*.rs`, `test*.py`, `**/*.rs`.
pub(crate) fn glob_match(text: &str, pattern: &str) -> bool {
    // Handle simple patterns like *.rs, test_*.py, etc.
    if let Some(ext) = pattern.strip_prefix("*.") {
        if let Some(text_ext) = text.rsplit('.').next() {
            return text_ext == ext || ext == "*";
        }
        return false;
    }

    // Handle **.rs -> ends with .rs
    if let Some(ext) = pattern.strip_prefix("**.") {
        if let Some(text_ext) = text.rsplit('.').next() {
            return text_ext == ext;
        }
        return false;
    }

    // Handle prefix*suffix pattern (e.g., test*.py) - asterisk in middle
    // This is: starts with prefix, ends with suffix, * matches anything in between
    if pattern.contains('*')
        && !pattern.starts_with('*')
        && !pattern.ends_with('*')
        && let Some(star_pos) = pattern.find('*')
    {
        let prefix = &pattern[..star_pos];
        let suffix = &pattern[star_pos + 1..];
        // text must start with prefix and end with suffix
        // and be at least as long as prefix + suffix
        if text.starts_with(prefix)
            && text.ends_with(suffix)
            && prefix.len() + suffix.len() <= text.len()
        {
            return true;
        }
    }

    // Handle *suffix pattern (e.g., *test.py)
    if let Some(stripped) = pattern.strip_prefix('*') {
        return text.ends_with(stripped);
    }

    // Handle suffix* pattern (e.g., test*)
    if let Some(stripped) = pattern.strip_suffix('*') {
        return text.starts_with(stripped);
    }

    // Simple contains check for other patterns
    text.contains(pattern)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The patterns a caller writes for a file filter select what they name.
    #[test]
    fn a_glob_selects_the_names_it_names() {
        assert!(glob_match("test.rs", "*.rs"));
        assert!(!glob_match("test.py", "*.rs"));
        assert!(glob_match("src/lib.rs", "*.rs"));
        assert!(glob_match("test.py", "test*.py"));
        assert!(!glob_match("test.py", "foo*.py"));
    }

    /// The glob of a path is asked of the name the path ends with.
    #[test]
    fn the_glob_of_a_path_is_asked_of_its_name() {
        assert!(path_matches_glob(Path::new("src/lib.rs"), "*.rs"));
        assert!(path_matches_glob(Path::new("src/lib.rs"), "**/*.rs"));
        assert!(!path_matches_glob(Path::new("src/lib.py"), "**/*.rs"));
    }
}
