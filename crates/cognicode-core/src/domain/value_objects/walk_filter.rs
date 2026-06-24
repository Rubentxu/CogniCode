//! WalkFilter — a composable value object for controlling filesystem traversal.
//!
//! Security blocklist: paths that should NEVER be touched (`.git`, `.ssh`, credentials)
//! Performance skips: build artifacts, caches, and other non-source paths
//!
//! # Example
//!
//! ```
//! use std::path::Path;
//! use cognicode_core::domain::value_objects::{WalkFilter, WalkDecision};
//!
//! let filter = WalkFilter::default();
//! let path = Path::new("node_modules");
//! assert!(matches!(filter.should_walk(path), WalkDecision::Prune));
//! ```

use std::path::Path;

/// Decision made by WalkFilter for a given path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WalkDecision {
    /// Walk this path — descend into it and process its contents
    Include,
    /// Skip this path but descend into it (e.g., hidden files that are directories)
    Skip,
    /// Skip this path AND don't descend into it (e.g., `target`, `node_modules`)
    Prune,
}

impl WalkDecision {
    /// Returns true if this decision allows descending into the path
    pub fn allows_descent(self) -> bool {
        !matches!(self, WalkDecision::Prune)
    }
}

/// WalkFilter value object — composable builder for path filtering decisions.
///
/// Construct using builders:
/// - `WalkFilter::new()` — empty filter (nothing blocked)
/// - `WalkFilter::default()` — security blocklist + performance skips
///
/// # Composition
///
/// Filters are composed via the builder pattern:
/// ```
/// use cognicode_core::domain::value_objects::WalkFilter;
///
/// let filter = WalkFilter::new()
///     .with_security_blocklist()
///     .with_performance_skips();
/// ```
#[derive(Debug, Clone)]
pub struct WalkFilter {
    security_blocklist: Vec<String>,
    performance_skips: Vec<String>,
}

impl WalkFilter {
    /// Create a new empty WalkFilter (nothing blocked by default)
    pub fn new() -> Self {
        Self {
            security_blocklist: Vec::new(),
            performance_skips: Vec::new(),
        }
    }

    /// Add security blocklist: paths that should NEVER be touched
    ///
    /// Includes: `.git`, `.ssh`, `.env`, credentials-related paths
    pub fn with_security_blocklist(mut self) -> Self {
        self.security_blocklist = vec![
            ".git".to_string(),
            ".ssh".to_string(),
            ".env".to_string(),
            ".env.local".to_string(),
            ".env.development".to_string(),
            ".env.production".to_string(),
            "id_rsa".to_string(),
            "id_ed25519".to_string(),
            "id_ecdsa".to_string(),
            ".aws".to_string(),
            ".kube".to_string(),
            ".docker".to_string(),
            "credentials".to_string(),
            "secrets".to_string(),
            ".passwords".to_string(),
        ];
        self
    }

    /// Add performance skips: build artifacts, caches, and non-source directories
    ///
    /// Includes: `target`, `node_modules`, `.venv`, `__pycache__`, etc.
    pub fn with_performance_skips(mut self) -> Self {
        self.performance_skips = vec![
            "target".to_string(),
            "node_modules".to_string(),
            ".venv".to_string(),
            "venv".to_string(),
            "env".to_string(),
            ".env".to_string(),
            "__pycache__".to_string(),
            ".pytest_cache".to_string(),
            ".mypy_cache".to_string(),
            ".ruff_cache".to_string(),
            "dist".to_string(),
            "build".to_string(),
            "vendor".to_string(),
            ".cache".to_string(),
            ".next".to_string(),
            ".nuxt".to_string(),
            ".svelte-kit".to_string(),
            "coverage".to_string(),
            ".tox".to_string(),
            ".sandbox".to_string(),
            ".cognicode".to_string(),
        ];
        self
    }

    /// Check a path and return the decision for whether to walk it.
    ///
    /// Returns `WalkDecision::Prune` for security blocklist matches (never descend)
    /// Returns `WalkDecision::Skip` for performance skip matches (descend but don't process)
    /// Returns `WalkDecision::Include` for all other paths
    pub fn should_walk(&self, path: &Path) -> WalkDecision {
        let file_name = match path.file_name().and_then(|n| n.to_str()) {
            Some(name) => name,
            None => return WalkDecision::Include,
        };

        // Security blocklist: never touch these, don't descend
        if self.security_blocklist.iter().any(|s| s == file_name) {
            return WalkDecision::Prune;
        }

        // Performance skips: don't process but may descend (for nested project roots)
        // Actually, performance skips should ALSO prune to avoid descending into large trees
        if self.performance_skips.iter().any(|s| s == file_name) {
            return WalkDecision::Prune;
        }

        WalkDecision::Include
    }

    /// Check if any component of the path matches the blocklist or skips.
    ///
    /// Use this for `walkdir::WalkDir::filter_entry` to prune entire subtrees
    /// as soon as we encounter a matching component, instead of yielding
    /// and then filtering out.
    ///
    /// Returns `true` if the path should be skipped (filtered out).
    pub fn matches_any_component(&self, path: &Path) -> bool {
        path.components().any(|c| {
            c.as_os_str()
                .to_str()
                .map(|s| {
                    self.security_blocklist.iter().any(|b| b == s)
                        || self.performance_skips.iter().any(|p| p == s)
                })
                .unwrap_or(false)
        })
    }

    /// Returns the security blocklist patterns
    pub fn security_blocklist(&self) -> &[String] {
        &self.security_blocklist
    }

    /// Returns the performance skip patterns
    pub fn performance_skips(&self) -> &[String] {
        &self.performance_skips
    }
}

impl Default for WalkFilter {
    /// Default = security blocklist + performance skips
    fn default() -> Self {
        Self::new()
            .with_security_blocklist()
            .with_performance_skips()
    }
}

impl PartialEq for WalkFilter {
    fn eq(&self, other: &Self) -> bool {
        self.security_blocklist == other.security_blocklist
            && self.performance_skips == other.performance_skips
    }
}

impl Eq for WalkFilter {}

#[cfg(test)]
mod tests {
    use super::*;

    // =====================================================================
    // WalkDecision Tests
    // =====================================================================

    #[test]
    fn walk_decision_allows_descent_include() {
        assert!(WalkDecision::Include.allows_descent());
    }

    #[test]
    fn walk_decision_allows_descent_skip() {
        assert!(WalkDecision::Skip.allows_descent());
    }

    #[test]
    fn walk_decision_denies_descent_prune() {
        assert!(!WalkDecision::Prune.allows_descent());
    }

    #[test]
    fn walk_decision_equality() {
        assert_eq!(WalkDecision::Include, WalkDecision::Include);
        assert_eq!(WalkDecision::Skip, WalkDecision::Skip);
        assert_eq!(WalkDecision::Prune, WalkDecision::Prune);
        assert_ne!(WalkDecision::Include, WalkDecision::Skip);
    }

    // =====================================================================
    // WalkFilter Construction Tests
    // =====================================================================

    #[test]
    fn walk_filter_new_is_empty() {
        let filter = WalkFilter::new();
        assert!(filter.security_blocklist().is_empty());
        assert!(filter.performance_skips().is_empty());
    }

    #[test]
    fn walk_filter_default_has_blocklist_and_skips() {
        let filter = WalkFilter::default();
        assert!(!filter.security_blocklist().is_empty());
        assert!(!filter.performance_skips().is_empty());
    }

    #[test]
    fn walk_filter_builder_returns_new_instance() {
        let original = WalkFilter::new();
        let with_blocklist = original.clone().with_security_blocklist();
        // Original should remain unchanged
        assert!(original.security_blocklist().is_empty());
        // New instance should have blocklist
        assert!(!with_blocklist.security_blocklist().is_empty());
    }

    #[test]
    fn walk_filter_security_blocklist_contains_expected_paths() {
        let filter = WalkFilter::new().with_security_blocklist();
        let blocklist = filter.security_blocklist();

        assert!(blocklist.contains(&".git".to_string()));
        assert!(blocklist.contains(&".ssh".to_string()));
        assert!(blocklist.contains(&".env".to_string()));
        assert!(blocklist.contains(&"credentials".to_string()));
    }

    #[test]
    fn walk_filter_performance_skips_contains_expected_paths() {
        let filter = WalkFilter::new().with_performance_skips();
        let skips = filter.performance_skips();

        assert!(skips.contains(&"target".to_string()));
        assert!(skips.contains(&"node_modules".to_string()));
        assert!(skips.contains(&"__pycache__".to_string()));
        assert!(skips.contains(&".venv".to_string()));
    }

    // =====================================================================
    // WalkFilter::should_walk Tests — Security Blocklist
    // =====================================================================

    #[test]
    fn should_walk_prunes_git_directory() {
        let filter = WalkFilter::default();
        let path = Path::new("/project/.git");
        assert_eq!(filter.should_walk(path), WalkDecision::Prune);
    }

    #[test]
    fn should_walk_prunes_ssh_directory() {
        let filter = WalkFilter::default();
        let path = Path::new("/home/user/.ssh");
        assert_eq!(filter.should_walk(path), WalkDecision::Prune);
    }

    #[test]
    fn should_walk_prunes_env_files() {
        let filter = WalkFilter::default();
        assert_eq!(filter.should_walk(Path::new(".env")), WalkDecision::Prune);
        assert_eq!(
            filter.should_walk(Path::new(".env.local")),
            WalkDecision::Prune
        );
    }

    #[test]
    fn should_walk_prunes_credentials_paths() {
        let filter = WalkFilter::default();
        assert_eq!(
            filter.should_walk(Path::new("credentials")),
            WalkDecision::Prune
        );
        assert_eq!(
            filter.should_walk(Path::new("secrets")),
            WalkDecision::Prune
        );
    }

    // =====================================================================
    // WalkFilter::should_walk Tests — Performance Skips
    // =====================================================================

    #[test]
    fn should_walk_prunes_target_directory() {
        let filter = WalkFilter::default();
        let path = Path::new("/project/target");
        assert_eq!(filter.should_walk(path), WalkDecision::Prune);
    }

    #[test]
    fn should_walk_prunes_node_modules() {
        let filter = WalkFilter::default();
        let path = Path::new("/project/node_modules");
        assert_eq!(filter.should_walk(path), WalkDecision::Prune);
    }

    #[test]
    fn should_walk_prunes_python_cache() {
        let filter = WalkFilter::default();
        assert_eq!(
            filter.should_walk(Path::new("__pycache__")),
            WalkDecision::Prune
        );
        assert_eq!(
            filter.should_walk(Path::new(".pytest_cache")),
            WalkDecision::Prune
        );
    }

    #[test]
    fn should_walk_prunes_virtual_environments() {
        let filter = WalkFilter::default();
        assert_eq!(filter.should_walk(Path::new(".venv")), WalkDecision::Prune);
        assert_eq!(filter.should_walk(Path::new("venv")), WalkDecision::Prune);
        assert_eq!(filter.should_walk(Path::new("env")), WalkDecision::Prune);
    }

    #[test]
    fn should_walk_prunes_build_directories() {
        let filter = WalkFilter::default();
        assert_eq!(filter.should_walk(Path::new("dist")), WalkDecision::Prune);
        assert_eq!(filter.should_walk(Path::new("build")), WalkDecision::Prune);
    }

    // =====================================================================
    // WalkFilter::should_walk Tests — Allowed Paths
    // =====================================================================

    #[test]
    fn should_walk_includes_normal_source_files() {
        let filter = WalkFilter::default();
        assert_eq!(
            filter.should_walk(Path::new("src/main.rs")),
            WalkDecision::Include
        );
        assert_eq!(
            filter.should_walk(Path::new("lib.py")),
            WalkDecision::Include
        );
        assert_eq!(
            filter.should_walk(Path::new("index.js")),
            WalkDecision::Include
        );
    }

    #[test]
    fn should_walk_includes_hidden_files_that_are_not_blocked() {
        let filter = WalkFilter::default();
        // .gitignore is not in the blocklist
        assert_eq!(
            filter.should_walk(Path::new(".gitignore")),
            WalkDecision::Include
        );
        assert_eq!(
            filter.should_walk(Path::new(".editorconfig")),
            WalkDecision::Include
        );
    }

    #[test]
    fn should_walk_includes_normal_directories() {
        let filter = WalkFilter::default();
        assert_eq!(filter.should_walk(Path::new("src")), WalkDecision::Include);
        assert_eq!(
            filter.should_walk(Path::new("tests")),
            WalkDecision::Include
        );
        assert_eq!(filter.should_walk(Path::new("docs")), WalkDecision::Include);
    }

    #[test]
    fn should_walk_includes_project_root() {
        let filter = WalkFilter::default();
        // Empty path (project root)
        let path = Path::new("");
        assert_eq!(filter.should_walk(path), WalkDecision::Include);
    }

    #[test]
    fn should_walk_includes_path_with_no_file_name() {
        let filter = WalkFilter::default();
        // Root path has no file name
        let path = Path::new("/");
        assert_eq!(filter.should_walk(path), WalkDecision::Include);
    }

    // =====================================================================
    // WalkFilter::should_walk Tests — Edge Cases
    // =====================================================================

    #[test]
    fn should_walk_handles_path_with_only_file_name() {
        let filter = WalkFilter::default();
        // Just the directory name as a path
        assert_eq!(filter.should_walk(Path::new("target")), WalkDecision::Prune);
        assert_eq!(
            filter.should_walk(Path::new("node_modules")),
            WalkDecision::Prune
        );
    }

    #[test]
    fn should_walk_is_case_sensitive() {
        let filter = WalkFilter::default();
        // Blocklist is lowercase
        assert_eq!(filter.should_walk(Path::new("Git")), WalkDecision::Include);
        assert_eq!(
            filter.should_walk(Path::new("NODE_MODULES")),
            WalkDecision::Include
        );
    }

    #[test]
    fn should_walk_only_checks_file_name_not_full_path() {
        let filter = WalkFilter::default();
        // The filter only checks the file name component, not full path matching
        // So /project/target gets pruned because "target" IS the file name
        let path = Path::new("/project/target");
        assert_eq!(filter.should_walk(path), WalkDecision::Prune);
        // But /project/target/debug returns Include because "debug" is the file name
        let nested = Path::new("/project/target/debug");
        assert_eq!(filter.should_walk(nested), WalkDecision::Include);
    }

    // =====================================================================
    // WalkFilter Equality Tests
    // =====================================================================

    #[test]
    fn walk_filter_equality_with_same_config() {
        let a = WalkFilter::new()
            .with_security_blocklist()
            .with_performance_skips();
        let b = WalkFilter::new()
            .with_security_blocklist()
            .with_performance_skips();
        assert_eq!(a, b);
    }

    #[test]
    fn walk_filter_inequality_with_different_config() {
        let a = WalkFilter::new().with_security_blocklist();
        let b = WalkFilter::new().with_performance_skips();
        assert_ne!(a, b);
    }

    // =====================================================================
    // WalkFilter PartialEq Trait Tests
    // =====================================================================

    #[test]
    fn walk_filter_partial_eq_reflexive() {
        let filter = WalkFilter::default();
        assert_eq!(filter, filter);
    }

    #[test]
    fn walk_filter_default_equals_composed() {
        let default_filter = WalkFilter::default();
        let composed_filter = WalkFilter::new()
            .with_security_blocklist()
            .with_performance_skips();
        assert_eq!(default_filter, composed_filter);
    }
}
