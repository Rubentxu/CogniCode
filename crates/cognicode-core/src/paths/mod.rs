//! The workspace path rules: where a path may point, and how that is
//! decided.
//!
//! One implementation, several doors. The MCP validator
//! (`interface::mcp::security::InputValidator`) asks these rules for every
//! path its handlers touch, and the refactor pipeline (`refactor::preview`,
//! `refactor::apply`) asks the same rules for every file a plan names —
//! neither one keeps a private judgment of "is this path safe?", because
//! two implementations of the boundary is a boundary waiting to drift.
//! The rules sit below the interface layer on purpose: an inner module
//! (`refactor/`) may not import `interface` (see the
//! `inner_layers_do_not_import_interface` contract), and the boundary is
//! the one thing it may not judge on its own.
//!
//! The rules are the ones TASK-61/62 pinned for the MCP handlers:
//! traversal patterns first, then control characters, then resolution
//! against the allowed roots, then the depth cap, then the symlink check
//! (before canonicalization, so a symlinked leaf is refused rather than
//! followed), then the canonical boundary. The error is the port's
//! [`PathPolicyError`], so the judgment reads the same no matter which
//! door asked.

use std::path::{Path, PathBuf};

use tracing::debug;

use crate::application::ports::PathPolicyError;

/// The most components a judged path may carry.
pub(crate) const MAX_PATH_COMPONENTS: usize = 100;

/// The path rules for a set of allowed roots.
#[derive(Debug, Clone)]
pub(crate) struct PathRules {
    allowed_paths: Vec<PathBuf>,
}

impl PathRules {
    /// Rules with no allowed root. There is no boundary to judge
    /// against, so the rules apply only their lexical checks and pass
    /// an existing path wherever it lives — the caller anchors first
    /// if it means to confine. Every production door sets a workspace;
    /// the empty rules exist for tests and for callers that judge
    /// anchoring themselves.
    pub(crate) fn new() -> Self {
        Self {
            allowed_paths: Vec::new(),
        }
    }

    /// The allowed roots, for diagnostics that name what was allowed.
    pub(crate) fn allowed_roots(&self) -> &[PathBuf] {
        &self.allowed_paths
    }

    /// Sets the allowed workspace paths (canonicalizes each path for
    /// reliable comparison).
    pub(crate) fn with_workspace(mut self, paths: Vec<PathBuf>) -> Self {
        self.allowed_paths = paths
            .into_iter()
            .map(|p| std::fs::canonicalize(&p).unwrap_or(p))
            .collect();
        self
    }

    /// Adds an allowed workspace path (canonicalized).
    pub(crate) fn add_allowed_path(&mut self, path: PathBuf) {
        let canonical = std::fs::canonicalize(&path).unwrap_or(path);
        self.allowed_paths.push(canonical);
    }

    /// The canonical path of a file the rules allow. Anchoring is the
    /// caller's first job — an anchored path is judged, a raw relative
    /// one is resolved against the allowed roots, which need not be the
    /// directory the caller meant.
    pub(crate) fn validate_file_path(&self, path: &str) -> Result<PathBuf, PathPolicyError> {
        // Check for path traversal patterns (including bypass attempts)
        if self.contains_path_traversal(path) {
            warn_path("Path traversal attempt detected", path);
            return Err(PathPolicyError::PathTraversalAttempt {
                path: path.to_string(),
            });
        }

        // Check for null bytes and other invalid control characters
        // Control chars are ASCII 0-31, except tab (9), newline (10), CR (13)
        for c in path.chars() {
            if c == '\0' || (c.is_ascii_control() && c != '\t' && c != '\n' && c != '\r') {
                return Err(PathPolicyError::InvalidPathCharacters {
                    path: path.to_string(),
                });
            }
        }

        // Parse the path and resolve relative paths against workspace.
        // If the path is relative and we have allowed_paths (workspace dirs),
        // try joining with each workspace to find the file.
        let path_buf = PathBuf::from(path);
        let resolved = if path_buf.is_absolute() {
            path_buf.clone()
        } else if !self.allowed_paths.is_empty() {
            // ALWAYS resolve relative paths against workspace dirs first
            let mut resolved = path_buf.clone();
            let mut found = false;
            for ws in &self.allowed_paths {
                let candidate = ws.join(&path_buf);
                if candidate.exists() || candidate.parent().is_some_and(|p| p.exists()) {
                    resolved = candidate;
                    found = true;
                    break;
                }
            }
            if !found {
                // Fallback: join with first workspace (file may not exist yet for write operations)
                if let Some(first_ws) = self.allowed_paths.first() {
                    resolved = first_ws.join(&path_buf);
                }
            }
            resolved
        } else {
            path_buf.clone()
        };

        // Check path depth
        let component_count = resolved.components().count();
        if component_count > MAX_PATH_COMPONENTS {
            return Err(PathPolicyError::PathTooDeep {
                depth: component_count,
                max: MAX_PATH_COMPONENTS,
            });
        }

        // CRITICAL: Check for symlinks BEFORE canonicalization using symlink_metadata()
        // (std::fs::metadata follows symlinks, so it CANNOT detect symlinks — must use symlink_metadata)
        if let Ok(metadata) = std::fs::symlink_metadata(&resolved)
            && metadata.file_type().is_symlink()
        {
            warn_path("Symlink detected in path", path);
            return Err(PathPolicyError::SymlinkDetected {
                path: path.to_string(),
            });
        }

        // NOTE: We deliberately do NOT walk every parent component of `resolved`
        // looking for symlinks. That walk is redundant with the canonical-form
        // workspace boundary check below: any attacker-controlled parent symlink
        // that escapes the workspace would cause the canonicalized target to
        // fall outside `allowed_paths`, which the workspace boundary check
        // already rejects. The parent-walk only added false positives for
        // system-managed symlinks like `/home -> /var/home` (Fedora Silverblue),
        // `/var -> /private/var` (macOS), `/tmp -> /private/tmp` (macOS) — none
        // of which are attacker-controlled. See e93 / DEBT-SDDK-007.

        // Try to canonicalize to resolve any symlinks and get absolute path
        let canonical = match std::fs::canonicalize(&resolved) {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // File doesn't exist yet - that's OK for some operations
                // Walk up the directory tree until we find an existing directory
                Self::canonicalize_with_fallback(&resolved)
            }
            Err(_) => {
                return Err(PathPolicyError::PathNotAccessible {
                    path: path.to_string(),
                });
            }
        };

        // If workspace paths are configured, verify the path is within workspace
        if !self.allowed_paths.is_empty() {
            let is_allowed = self
                .allowed_paths
                .iter()
                .any(|allowed| canonical.starts_with(allowed) || canonical == *allowed);

            if !is_allowed {
                debug!(
                    "Path {} is not within allowed workspace: {:?}",
                    canonical.display(),
                    self.allowed_paths
                );
                return Err(PathPolicyError::PathOutsideWorkspace);
            }
        }

        Ok(canonical)
    }

    /// Judges a path for traversal and workspace boundaries: the single
    /// authority a handler asks before touching the path. Performs
    /// traversal detection (including encoded and backslash variants),
    /// null-byte detection, depth validation, symlink detection before
    /// canonicalization (TOCTOU protection) and the canonical boundary.
    pub(crate) fn validate_path(&self, path: &Path) -> Result<(), PathPolicyError> {
        let path_str = path.to_string_lossy();

        // Check for path traversal patterns
        if self.contains_path_traversal(&path_str) {
            warn_path("Path traversal attempt detected", &path_str);
            return Err(PathPolicyError::PathTraversalAttempt {
                path: path_str.to_string(),
            });
        }

        // Check for null bytes
        if path_str.contains('\0') {
            return Err(PathPolicyError::InvalidPathCharacters {
                path: path_str.to_string(),
            });
        }

        // Parse the path and resolve relative paths against workspace.
        let resolved = self.resolve_against_allowed(path);

        // Check path depth
        let component_count = resolved.components().count();
        if component_count > MAX_PATH_COMPONENTS {
            return Err(PathPolicyError::PathTooDeep {
                depth: component_count,
                max: MAX_PATH_COMPONENTS,
            });
        }

        // CRITICAL: Check for symlinks BEFORE canonicalization using symlink_metadata()
        if let Ok(metadata) = std::fs::symlink_metadata(&resolved)
            && metadata.file_type().is_symlink()
        {
            warn_path("Symlink detected in path", &path_str);
            return Err(PathPolicyError::SymlinkDetected {
                path: path_str.to_string(),
            });
        }

        // NOTE: We deliberately do NOT walk every parent component of `resolved`
        // looking for symlinks. See the matching comment in `validate_file_path`
        // for the rationale (DEBT-SDDK-007 / e93). The canonical-form workspace
        // boundary check below is the single authority on whether a path is safe.

        // Canonicalize and verify workspace boundary
        self.canonical_target_within(&resolved, &path_str)?;
        Ok(())
    }

    /// Judges a path the pipeline itself produced — a plan edit, a
    /// graph result. The path is a resolved location, not client text:
    /// the judgment is the filesystem one (depth, symlink leaf, the
    /// canonical boundary) WITHOUT the text heuristics, because `..`,
    /// `$` or `~` are legitimate characters in a filename (goldfish
    /// TASK-219: the heuristics guard client TEXT at the MCP door; a
    /// resolved path is data).
    ///
    /// Returns the canonical path, so the caller reads the very file
    /// the judgment was made about.
    pub(crate) fn validate_internal_path(&self, path: &Path) -> Result<PathBuf, PathPolicyError> {
        let resolved = self.resolve_against_allowed(path);

        let component_count = resolved.components().count();
        if component_count > MAX_PATH_COMPONENTS {
            return Err(PathPolicyError::PathTooDeep {
                depth: component_count,
                max: MAX_PATH_COMPONENTS,
            });
        }

        // CRITICAL: symlink leaf before canonicalization — a symlinked
        // leaf is refused rather than followed, inside or outside.
        if let Ok(metadata) = std::fs::symlink_metadata(&resolved)
            && metadata.file_type().is_symlink()
        {
            warn_path("Symlink detected in path", &path.to_string_lossy());
            return Err(PathPolicyError::SymlinkDetected {
                path: path.to_string_lossy().to_string(),
            });
        }

        self.canonical_target_within(&resolved, &path.to_string_lossy())
    }

    /// The canonical form of `resolved` and the boundary judgment: the
    /// one authority on whether a path stays inside the workspace. The
    /// tail shared by `validate_path` and `validate_internal_path`.
    fn canonical_target_within(
        &self,
        resolved: &Path,
        original: &str,
    ) -> Result<PathBuf, PathPolicyError> {
        let canonical = match std::fs::canonicalize(resolved) {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // The path doesn't exist (yet) - canonicalize the parent
                // directory so the boundary check below still applies. The
                // canonicalize(parent) call already resolves any symlinks,
                // and the boundary rejects any canonical target that
                // escapes `allowed_paths`.
                if let Some(parent) = resolved.parent() {
                    std::fs::canonicalize(parent).map_err(|_| {
                        PathPolicyError::PathNotAccessible {
                            path: original.to_string(),
                        }
                    })?
                } else {
                    return Err(PathPolicyError::PathNotAccessible {
                        path: original.to_string(),
                    });
                }
            }
            Err(_) => {
                return Err(PathPolicyError::PathNotAccessible {
                    path: original.to_string(),
                });
            }
        };

        if !self.allowed_paths.is_empty() {
            let is_allowed = self
                .allowed_paths
                .iter()
                .any(|allowed| canonical.starts_with(allowed) || canonical == *allowed);
            if !is_allowed {
                debug!(
                    "Path {} is not within allowed workspace: {:?}",
                    canonical.display(),
                    self.allowed_paths
                );
                return Err(PathPolicyError::PathOutsideWorkspace);
            }
        }
        Ok(canonical)
    }

    /// Resolves the path against the workspace and says whether it stays
    /// inside it even when the directories leading to it do not exist yet.
    ///
    /// The difference with [`PathRules::validate_path`] is only in the last
    /// step: the boundary is judged on the deepest ancestor that is already
    /// there. The ancestors that exist can hide an escape — a symlink
    /// pointing out of the workspace is one — and the ones that do not exist
    /// cannot, because a name stays in the directory it is written in.
    pub(crate) fn resolve_for_creation(&self, path: &Path) -> Result<PathBuf, PathPolicyError> {
        let path_str = path.to_string_lossy();

        // The same lexical rules, in the same order as `validate_path`.
        if self.contains_path_traversal(&path_str) {
            warn_path("Path traversal attempt detected", &path_str);
            return Err(PathPolicyError::PathTraversalAttempt {
                path: path_str.to_string(),
            });
        }
        if path_str.contains('\0') {
            return Err(PathPolicyError::InvalidPathCharacters {
                path: path_str.to_string(),
            });
        }

        let resolved = self.resolve_against_allowed(path);

        let component_count = resolved.components().count();
        if component_count > MAX_PATH_COMPONENTS {
            return Err(PathPolicyError::PathTooDeep {
                depth: component_count,
                max: MAX_PATH_COMPONENTS,
            });
        }

        if let Ok(metadata) = std::fs::symlink_metadata(&resolved)
            && metadata.file_type().is_symlink()
        {
            warn_path("Symlink detected in path", &path_str);
            return Err(PathPolicyError::SymlinkDetected {
                path: path_str.to_string(),
            });
        }

        if !self.allowed_paths.is_empty() {
            let existing = deepest_existing_ancestor(&resolved);
            // A symlink is an ancestor too: `canonicalize` resolves it, so
            // the boundary is judged on where it really points.
            let canonical = std::fs::canonicalize(&existing).map_err(|_| {
                PathPolicyError::PathNotAccessible {
                    path: path_str.to_string(),
                }
            })?;
            let is_allowed = self
                .allowed_paths
                .iter()
                .any(|allowed| canonical.starts_with(allowed) || canonical == *allowed);
            if !is_allowed {
                debug!(
                    "Path {} is not within allowed workspace: {:?}",
                    canonical.display(),
                    self.allowed_paths
                );
                return Err(PathPolicyError::PathOutsideWorkspace);
            }
        }

        Ok(resolved)
    }

    /// Resolves a path against the workspace roots.
    ///
    /// One resolution, shared by both phases of the judgment. A path that is
    /// absolute is taken as it is; a relative one is taken against the first
    /// workspace whose candidate already exists, and against the first
    /// workspace when none of them does.
    fn resolve_against_allowed(&self, path: &Path) -> PathBuf {
        if path.is_absolute() || self.allowed_paths.is_empty() {
            return path.to_path_buf();
        }
        for ws in &self.allowed_paths {
            let candidate = ws.join(path);
            if candidate.exists() || candidate.parent().is_some_and(|p| p.exists()) {
                return candidate;
            }
        }
        self.allowed_paths[0].join(path)
    }

    pub(crate) fn contains_path_traversal(&self, path: &str) -> bool {
        // Normalize and check for common traversal patterns
        let path_lower = path.to_lowercase();

        // Check for standard traversal patterns
        if path.contains("..") {
            return true;
        }

        // Check for backslash traversal patterns (Windows-style bypass)
        if path.contains("..\\") {
            return true;
        }

        // Decode URL-encoded dots and check for traversal patterns.
        // This catches patterns like .%2e/%2e. which decodes to ../..
        // We decode all %XX sequences to check the actual path.
        let decoded = Self::decode_url_encoded(path);
        if decoded.contains("..") {
            return true;
        }

        // Check for URL-encoded traversal patterns
        // %2e%2e = .. (URL-encoded)
        // %2E%2E = .. (mixed case URL-encoded)
        if path_lower.contains("%2e%2e")
            || path_lower.contains("%2e%2e%2f")
            || path_lower.contains("%2e%2e%5c")
            || path_lower.contains("%2e%2e/")
            || path_lower.contains("%2e%2e\\")
        {
            return true;
        }

        // Check for mixed literal + URL-encoded dot patterns that form ..
        // .%2e = literal dot + %2e (encoded dot) = ..
        // %2e. = %2e (encoded dot) + literal dot = ..
        if path_lower.contains(".%2e")
            || path_lower.contains("%2e.")
            || path_lower.contains(".%2e%2e")
            || path_lower.contains("%2e%2e.")
            || path_lower.contains(".%2e%2e%2f")
            || path_lower.contains("%2e%2e%2f.")
        {
            return true;
        }

        // Check for tilde expansion attempts
        if path.contains("~/") || path.contains("~\\") {
            return true;
        }

        // Check for environment variable injection
        if path.contains('$') || path.contains("${") {
            return true;
        }

        // Check for double slash patterns that might bypass path normalization
        if path.contains("//") && !path.starts_with("./") && !path.starts_with("../") {
            // Allow relative paths starting with // but block absolute paths
            if path.starts_with('/') {
                return true;
            }
        }

        false
    }

    /// Decode URL-encoded characters in a path string, recursively until no more encodings.
    /// %2e -> . (dot), %2f -> / (slash), %252e -> %2e -> . (double decode)
    fn decode_url_encoded(s: &str) -> String {
        // Single decode pass
        let decoded = Self::decode_url_encoded_once(s);
        // If unchanged, no more encoding to decode
        if decoded == s {
            decoded
        } else {
            // Recursively decode until no more changes
            Self::decode_url_encoded(&decoded)
        }
    }

    /// Single-pass URL-decode of a path string.
    fn decode_url_encoded_once(s: &str) -> String {
        let mut result = String::with_capacity(s.len());
        let mut chars = s.chars().peekable();

        while let Some(c) = chars.next() {
            if c == '%' {
                let hex = chars.by_ref().take(2).collect::<String>();
                if hex.len() == 2
                    && let Ok(byte) = u8::from_str_radix(&hex, 16)
                {
                    result.push(byte as char);
                    continue;
                }
                // Not a valid percent-encoded sequence, keep as-is
                result.push('%');
                result.push_str(&hex);
            } else {
                result.push(c);
            }
        }
        result
    }

    /// Canonicalizes `path` by walking up to the deepest ancestor that
    /// exists, so a path whose file does not exist yet still gets a
    /// canonical form for the boundary check.
    fn canonicalize_with_fallback(path: &std::path::Path) -> std::path::PathBuf {
        let mut current = path.to_path_buf();
        while let Some(parent) = current.parent() {
            if parent.as_os_str().is_empty() {
                break;
            }
            if let Ok(canonical_parent) = std::fs::canonicalize(parent) {
                // Found existing directory - append remaining path components
                let remaining = path
                    .strip_prefix(parent)
                    .map(|p| p.to_path_buf())
                    .unwrap_or_else(|_| path.to_path_buf());
                return canonical_parent.join(remaining);
            }
            current = parent.to_path_buf();
        }
        // No existing ancestor found - fallback to current directory
        std::path::PathBuf::from(".")
            .canonicalize()
            .unwrap_or_else(|_| std::path::PathBuf::from("."))
    }
}

/// The deepest ancestor of `path` that is already there.
///
/// The components above it are names that do not exist yet. They cannot
/// hide an escape — a name stays in the directory it is written in — so the
/// ancestor is the whole answer to "where would this land".
///
/// A symlink counts as being there, even a broken one: resolving it is how
/// the boundary check finds out where it points, and a broken one cannot be
/// resolved, which refuses the path instead of guessing.
fn deepest_existing_ancestor(path: &Path) -> PathBuf {
    let mut current = path.to_path_buf();
    while std::fs::symlink_metadata(&current).is_err() && current.pop() {}
    current
}

/// The refusal of a path is worth one line of log: it is the record that
/// somebody tried, not the judgment itself (the caller owns that).
fn warn_path(reason: &str, path: &str) {
    tracing::warn!("{}: {}", reason, path);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_path_with_no_allowed_root_is_not_judged_against_a_boundary() {
        // With no allowed root there is no boundary to judge against:
        // the rules apply only their lexical checks, so an existing
        // path passes wherever it lives. The caller anchors first if
        // it means to confine. The path is real (a tempdir) because
        // the rules still ask the filesystem for the canonical form.
        let dir = tempfile::tempdir().expect("tempdir");
        let anywhere = dir.path().join("file.rs");
        std::fs::write(&anywhere, b"x").expect("write");

        let rules = PathRules::new();
        let inside = rules.validate_path(&anywhere);
        assert!(inside.is_ok(), "no boundary, no refusal: {inside:?}");
    }

    #[test]
    fn a_traversal_pattern_is_refused_before_any_disk_is_touched() {
        let rules = PathRules::new().with_workspace(vec![PathBuf::from("/ws")]);
        let error = rules
            .validate_path(Path::new("/ws/../outside/file.rs"))
            .expect_err("traversal is refused");
        assert_eq!(
            error,
            PathPolicyError::PathTraversalAttempt {
                path: "/ws/../outside/file.rs".to_string()
            }
        );
    }

    #[test]
    fn a_symlink_leaf_is_refused_even_when_its_target_stays_inside() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("real.rs"), "x").expect("write");
        std::os::unix::fs::symlink("real.rs", dir.path().join("link.rs")).expect("symlink");

        let rules = PathRules::new().with_workspace(vec![dir.path().to_path_buf()]);
        let error = rules
            .validate_path(&dir.path().join("link.rs"))
            .expect_err("a symlink leaf is refused");
        assert_eq!(
            error,
            PathPolicyError::SymlinkDetected {
                path: dir.path().join("link.rs").to_string_lossy().to_string()
            }
        );
    }

    #[test]
    fn an_intermediate_symlink_that_escapes_is_refused_by_the_boundary() {
        let dir = tempfile::tempdir().expect("tempdir");
        let outside = tempfile::tempdir().expect("outside");
        std::fs::write(outside.path().join("evil.rs"), "x").expect("write");
        std::os::unix::fs::symlink(outside.path(), dir.path().join("escdir")).expect("symlink");

        let rules = PathRules::new().with_workspace(vec![dir.path().to_path_buf()]);
        let error = rules
            .validate_path(&dir.path().join("escdir").join("evil.rs"))
            .expect_err("the canonical target is outside");
        assert_eq!(error, PathPolicyError::PathOutsideWorkspace);
    }

    // ── the internal-path door (TASK-219 goldfish) ──────────────────────

    #[cfg(unix)]
    #[test]
    fn the_internal_door_accepts_names_that_look_like_traversal_text() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("a..b.rs"), "x").expect("write");
        std::fs::write(dir.path().join("user$.ts"), "x").expect("write");
        std::fs::create_dir(dir.path().join("~")).expect("mkdir");
        std::fs::write(dir.path().join("~").join("inside.rs"), "x").expect("write");

        let rules = PathRules::new().with_workspace(vec![dir.path().to_path_buf()]);
        for name in ["a..b.rs", "user$.ts"] {
            let canonical = rules
                .validate_internal_path(&dir.path().join(name))
                .unwrap_or_else(|e| panic!("{name} is a legitimate filename, got {e:?}"));
            assert!(canonical.starts_with(dir.path()));
        }
        let nested = rules.validate_internal_path(&dir.path().join("~").join("inside.rs"));
        assert!(
            nested.is_ok(),
            "a file under a directory named `~` is legitimate, got {:?}",
            nested.err()
        );
    }

    #[cfg(unix)]
    #[test]
    fn the_internal_door_still_refuses_a_symlink_leaf_and_an_escape() {
        let dir = tempfile::tempdir().expect("tempdir");
        let outside = tempfile::tempdir().expect("outside");
        std::fs::write(outside.path().join("evil.rs"), "x").expect("write");
        std::fs::write(dir.path().join("real.rs"), "x").expect("write");
        std::os::unix::fs::symlink("real.rs", dir.path().join("link.rs")).expect("symlink");
        std::os::unix::fs::symlink(outside.path(), dir.path().join("escdir")).expect("symlink");

        let rules = PathRules::new().with_workspace(vec![dir.path().to_path_buf()]);
        let leaf = rules.validate_internal_path(&dir.path().join("link.rs"));
        assert_eq!(
            leaf.unwrap_err(),
            PathPolicyError::SymlinkDetected {
                path: dir.path().join("link.rs").to_string_lossy().to_string()
            }
        );
        let escape = rules.validate_internal_path(&dir.path().join("escdir").join("evil.rs"));
        assert_eq!(escape.unwrap_err(), PathPolicyError::PathOutsideWorkspace);
    }
}
