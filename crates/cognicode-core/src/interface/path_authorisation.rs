//! Where a path is allowed to point, and how that is decided.
//!
//! Three questions live here, and they are asked together: which file does
//! this path name, is that file inside the workspace, and may the entry
//! read it. The order matters. Anchoring comes first, because the
//! validator resolves a bare relative path against the workspace roots,
//! which need not be the session directory: validating the raw string and
//! then joining it would authorise one file and read another.
//!
//! The rules live in one module so that no handler answers "is this path
//! safe?" with a private shortcut of its own — and so that a goal (which
//! runs without a session, on a bare workspace) asks the same authority
//! the handlers ask. This file sits at the interface level on purpose: it
//! is the neutral place where the two doors meet, so the goals never name
//! the MCP adapter.

use std::path::{Path, PathBuf};

use crate::interface::mcp::handlers::{HandlerContext, HandlerError};
use crate::interface::mcp::security::{InputValidator, SecurityError};

/// A directory the session is allowed to touch, as the client named it.
///
/// The newtype is the compile-time guard: the only way to obtain one is
/// [`authorise_directory`], so a handler cannot resolve a directory and
/// forget to ask whether it may. The path was validated before the type
/// existed; `into_inner` hands it to the disk-touching code.
#[derive(Debug, Clone)]
pub(crate) struct AuthorisedPath(PathBuf);

impl AuthorisedPath {
    /// The owned path, for the disk-touching code that consumes it.
    pub(crate) fn into_inner(self) -> PathBuf {
        self.0
    }
}

/// Resolves the directory a client named against the session, and
/// validates it with the session's validator: traversal, absolute
/// escapes and symlinked components are refused here, before any disk
/// is touched.
///
/// - If `input` is `None`, the session working directory is authorised.
/// - If `input` is absolute, it is authorised as named.
/// - If `input` is relative (including `.`), it is joined onto the
///   working directory and authorised joined.
///
/// The refusal distinguishes leaving the workspace from an I/O
/// accident: the first is a rejection, the second reaches the handler
/// as «does not exist».
pub(super) fn authorise_directory(
    ctx: &HandlerContext,
    input: Option<&str>,
) -> Result<AuthorisedPath, HandlerError> {
    let resolved = match input {
        None => ctx.working_dir().to_path_buf(),
        Some(raw) => {
            let p = PathBuf::from(raw);
            if p.is_absolute() {
                p
            } else {
                ctx.working_dir().join(p)
            }
        }
    };
    ctx.validator().validate_path(&resolved).map_err(|error| {
        HandlerError::InvalidInput(match error {
            SecurityError::PathOutsideWorkspace => {
                format!(
                    "the directory `{}` is outside the workspace",
                    resolved.display()
                )
            }
            other => format!(
                "the directory `{}` was rejected: {}",
                resolved.display(),
                describe_rejection(&other)
            ),
        })
    })?;
    Ok(AuthorisedPath(resolved))
}

/// The validator for a bare workspace — a goal's whole world.
///
/// A goal runs without a session, so the workspace is the only allowed
/// root. It is the same [`InputValidator`] the handlers ask: one
/// authority, asked from two doors.
pub(crate) fn workspace_validator(workspace: &Path) -> InputValidator {
    InputValidator::new().with_workspace(vec![workspace.to_path_buf()])
}

/// The canonical path of a workspace file a goal may read.
///
/// The refusal says which world the caller left when it is a boundary
/// one, and why the path was refused otherwise — the words every goal
/// publishes, so that two goals do not refuse the same path with two
/// different sentences.
pub(crate) fn authorise_workspace_file(workspace: &Path, raw: &str) -> Result<PathBuf, String> {
    let validator = workspace_validator(workspace);
    anchor_and_validate(&validator, workspace, raw).map_err(|error| match error {
        SecurityError::PathOutsideWorkspace => {
            format!(
                "the path `{raw}` is outside the workspace `{}`",
                workspace.display()
            )
        }
        other => format!(
            "the path `{raw}` was rejected: {}",
            describe_rejection(&other)
        ),
    })
}

/// The words for a rejection, so that every caller says the same thing
/// the same way. Leaving the workspace is named by the caller, which
/// knows the path it was handed; this covers the rest of the reasons.
pub(crate) fn describe_rejection(error: &SecurityError) -> String {
    match error {
        SecurityError::PathTraversalAttempt { .. } => "path traversal attempt rejected".to_string(),
        SecurityError::PathNotAccessible { path } => {
            format!("path could not be accessed: {path}")
        }
        SecurityError::SymlinkDetected { .. } => "symlinked path is not followed".to_string(),
        SecurityError::PathTooDeep { depth, max } => {
            format!("path is too deep ({depth} > {max})")
        }
        SecurityError::InvalidPathCharacters { path } => {
            format!("invalid characters in path: {path}")
        }
        other => other.to_string(),
    }
}

/// Anchors `raw` on the session working directory, validates the anchored
/// path and returns the canonical path the validator authorised.
pub(super) fn anchor_and_validate(
    validator: &InputValidator,
    working_dir: &Path,
    raw: &str,
) -> Result<PathBuf, SecurityError> {
    let anchored = resolve_file_path(raw, working_dir);
    validator.validate_file_path(&anchored.to_string_lossy())
}

/// The canonical path of a file the session is allowed to read.
pub(super) fn authorise_file_path(
    ctx: &HandlerContext,
    raw: &str,
) -> Result<PathBuf, SecurityError> {
    anchor_and_validate(ctx.validator(), ctx.working_dir(), raw)
}

/// Resolves a file path relative to a working directory.
///
/// - If `input_path` is absolute, returns it unchanged.
/// - If `input_path` is relative, joins it to `working_dir`.
pub(super) fn resolve_file_path(input_path: &str, working_dir: &Path) -> PathBuf {
    let p = Path::new(input_path);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        working_dir.join(p)
    }
}

#[cfg(test)]
mod tests {
    /// TASK-218 AC #5 — resolution of client input lives in this module
    /// and nowhere else. A new entry that joins the workspace or the
    /// working directory by hand bypasses the validator; this scan makes
    /// that bypass a failing test instead of a silent hole.
    #[test]
    fn client_input_is_joined_to_the_workspace_only_by_this_module() {
        let handlers = include_str!("mcp/handlers/mod.rs");
        assert!(
            !handlers.contains("working_dir.join("),
            "mcp/handlers/mod.rs joins the working directory by hand: the join \
             belongs to path_authorisation, where it is validated before any \
             disk is touched"
        );
        for (file, source) in [
            ("goals/inspect.rs", include_str!("goals/inspect.rs")),
            ("goals/change.rs", include_str!("goals/change.rs")),
        ] {
            assert!(
                !source.contains("workspace.join("),
                "{file} joins the workspace by hand: the join belongs to \
                 path_authorisation, where it is validated before any disk \
                 is touched"
            );
        }
    }
}
