# Spec — e93 (DEBT-SDDK-007): InputValidator parent-symlink walk produces false positives

## Requirement

`InputValidator::validate_file_path` and `InputValidator::validate_path` in
`crates/cognicode-core/src/interface/mcp/security.rs` MUST NOT reject a path
solely because one of its parent components is a system-managed symlink (such
as `/home → /var/home` on Fedora Silverblue, `/var → /private/var` on macOS,
or `/tmp → /private/tmp` on macOS). The validator MUST continue to reject
paths whose canonical target falls outside `allowed_paths`, and MUST continue
to reject paths that are themselves symlinks at the resolved target location.

## Scenarios

### Scenario 1: TempDir under TMPDIR=/home (system symlink) is a valid workspace

**Given** a host where `/home` is a system-managed symlink (e.g.
`/home -> /var/home`)
**And** `TMPDIR=/home/<user>/...` (as set by Jcode's runtime)
**And** an `InputValidator` constructed with
`with_workspace(vec![TempDir::new().unwrap().path().to_path_buf()])`
**When** `validator.validate_file_path` is called with a file path inside
that `TempDir`
**Then** the result is `Ok(())`.

### Scenario 2: Symlink inside the workspace pointing outside is rejected

**Given** an `InputValidator` with `allowed_paths = [<workspace_dir>]`
**And** a symlink `<workspace_dir>/escape -> /etc/passwd` exists
**When** `validator.validate_file_path` is called with the path
`<workspace_dir>/escape`
**Then** the result is `Err(SecurityError::PathOutsideWorkspace)` (because the
canonical target `/etc/passwd` does not start with `<workspace_dir>`).

This is the **regression test** that pins the symlink-attack defence after
removing the parent-walk.

### Scenario 3: Symlink at the target file (regardless of where it points) is rejected

**Given** a path whose `symlink_metadata` reports `is_symlink()`
**When** `validator.validate_file_path` is called with that path
**Then** the result is `Err(SecurityError::SymlinkDetected)`.

This is the **direct-symlink check** that remains in place — it is the
primary defence against a symlink placed by an attacker inside the
workspace that points to attacker-controlled content.

### Scenario 4: System symlink in a parent component is accepted

**Given** a host where `/home` is a system symlink
**And** a file at `/home/<user>/real.txt` (which canonicalizes to
`/var/home/<user>/real.txt`)
**When** `validator.validate_file_path` is called with the path
`/home/<user>/real.txt`
**And** `allowed_paths` includes the canonical workspace
(`/var/home/<user>/work/...`)
**Then** the result is `Err(SecurityError::PathOutsideWorkspace)` if and only
if the canonical target is outside the workspace (NOT because `/home` is a
symlink).

### Scenario 5: WorkspaceSession test compares canonical paths

**Given** a `TempDir` created via `TempDir::new()` (which honors `TMPDIR`)
**And** a `WorkspaceSession` constructed from that `TempDir`'s path
**When** the test asserts `session.workspace_root() == temp_dir.path()`
**Then** the assertion compares canonical paths (via `Path::canonicalize()`)
on both sides, so the comparison succeeds even when one side traverses a
system symlink.

## Refs

- `crates/cognicode-core/src/interface/mcp/security.rs`:
  - `InputValidator::validate_file_path` (removes parent-symlink walk)
  - `InputValidator::validate_path` (removes parent-symlink walk)
  - Keeps `symlink_metadata(&resolved).is_symlink()` (direct-symlink check)
- `crates/cognicode-core/src/application/workspace_session.rs`:
  - `WorkspaceSession::new` (already canonicalizes; no change)
  - `test_workspace_session_creation` (must compare canonical paths)
