//! CODEOWNERS parser — reads `.github/CODEOWNERS` and matches file paths
//! to owner handles using gitignore-style glob patterns.
//!
//! Feature-gated behind `ownership` (pulls in `gix` for blame support).

#[cfg(feature = "ownership")]
pub struct CodeOwnersMap {
    rules: Vec<(glob::Pattern, Vec<String>)>,
}

#[cfg(feature = "ownership")]
impl CodeOwnersMap {
    /// Parse `.github/CODEOWNERS` from workspace root.
    ///
    /// Returns empty map if file absent.
    pub fn parse(root: &std::path::Path) -> Self {
        let codeowners_path = root.join(".github").join("CODEOWNERS");
        let content = match std::fs::read_to_string(&codeowners_path) {
            Ok(c) => c,
            Err(_) => return Self { rules: Vec::new() },
        };
        let mut rules: Vec<(glob::Pattern, Vec<String>)> = Vec::new();
        for line in content.lines() {
            let line = line.trim();
            // Skip blank lines and comments
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            // Split on whitespace: pattern on left, owners on right
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 2 {
                continue;
            }
            let pattern_str = parts[0];
            let owners: Vec<String> = parts[1..]
                .iter()
                .map(|s| s.trim_start_matches('@').to_string())
                .collect();
            match glob::Pattern::new(pattern_str) {
                Ok(pattern) => rules.push((pattern, owners)),
                Err(_) => continue,
            }
        }
        Self { rules }
    }

    /// Return owner handles (without `@`) matching `rel_path`.
    /// Last matching rule wins (gitignore convention).
    pub fn owners_for(&self, rel_path: &str) -> Vec<String> {
        let mut matched: Vec<String> = Vec::new();
        for (pattern, owners) in &self.rules {
            if pattern.matches(rel_path) {
                matched = owners.clone();
            }
        }
        matched
    }
}

#[cfg(feature = "ownership")]
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn parse_reads_github_codeowners() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        let codeowners = root.join(".github").join("CODEOWNERS");
        std::fs::create_dir_all(codeowners.parent().unwrap()).unwrap();
        std::fs::write(&codeowners, "src/main.rs @alice @bob\n").unwrap();

        let map = CodeOwnersMap::parse(root);
        let owners = map.owners_for("src/main.rs");
        assert_eq!(owners, vec!["alice", "bob"]);
    }

    #[test]
    fn owners_for_returns_matching_owners() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        let codeowners = root.join(".github").join("CODEOWNERS");
        std::fs::create_dir_all(codeowners.parent().unwrap()).unwrap();
        std::fs::write(&codeowners, "*.rs @rust-owner\ndocs/** @docs-owner\n").unwrap();

        let map = CodeOwnersMap::parse(root);
        assert_eq!(map.owners_for("foo.rs"), vec!["rust-owner"]);
        assert_eq!(map.owners_for("src/lib.rs"), vec!["rust-owner"]); // *.rs matches any .rs file
        assert_eq!(map.owners_for("docs/guide.md"), vec!["docs-owner"]);
    }

    #[test]
    fn owners_for_last_match_wins() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        let codeowners = root.join(".github").join("CODEOWNERS");
        std::fs::create_dir_all(codeowners.parent().unwrap()).unwrap();
        // Last matching rule wins
        std::fs::write(&codeowners, "src/** @first\nsrc/core/** @second\n").unwrap();

        let map = CodeOwnersMap::parse(root);
        assert_eq!(map.owners_for("src/core/foo.rs"), vec!["second"]);
    }

    #[test]
    fn parse_missing_file_returns_empty() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();

        let map = CodeOwnersMap::parse(root);
        let owners = map.owners_for("src/main.rs");
        assert!(owners.is_empty());
    }
}
