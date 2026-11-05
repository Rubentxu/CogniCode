//! Where a path is allowed to point, and how that is decided.
//!
//! Three questions live here, and they are asked together: which file does
//! this path name, is that file inside the workspace, and may the handler
//! read it. The order matters. Anchoring comes first, because the
//! validator resolves a bare relative path against the workspace roots,
//! which need not be the session directory: validating the raw string and
//! then joining it would authorise one file and read another.
//!
//! The rules live in one module so that a handler cannot answer "is this
//! path safe?" with a private shortcut of its own.

use std::path::{Path, PathBuf};

use crate::interface::mcp::security::{InputValidator, SecurityError};

use super::HandlerContext;

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
    anchor_and_validate(&ctx.validator, &ctx.working_dir, raw)
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
