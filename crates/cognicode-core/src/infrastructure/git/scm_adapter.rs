//! Git adapter for the neutral SCM ports (CAA-03 / CGH-021).
//!
//! Speaks Git through FIXED argv only (`Command::new` + `.arg()`, no
//! shell, `--end-of-options` wherever a caller string joins argv) and
//! translates its output into the domain vocabulary of
//! [`crate::domain::scm`]. The application consumes the ports; it never
//! sees argv, sha syntax or the `-z` wire formats — those are this
//! module's whole job.
//!
//! Exactness rules pinned by the tests:
//! * HEAD, index, working tree and untracked are four different byte
//!   sets. Inventories read committed blobs (`ls-tree` + `cat-file`),
//!   staged changes read the INDEX (`ls-files -s` + `cat-file`), and
//!   dirty paths come from `status`. Nothing ever substitutes one for
//!   another.
//! * Blob identity is the OID the provider recorded; content identity
//!   is the domain `ContentDigest` over the raw bytes. Gitlinks (mode
//!   160000) carry a named gitlink digest and report
//!   `ContentUnavailable` for bytes: their content lives in another
//!   repository, and an empty vec would be a lie.
//! * Every external read is bounded (no unbounded memory), NUL-safe
//!   (`-z` everywhere), and fails CLOSED: a named `ScmError`, never an
//!   Ok(empty) that would silently claim "nothing changed".

use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex, PoisonError};

use crate::domain::scm::{
    BlobRead, ChangeKind, ChangeReader, ChangeSet, ContentDigest, InventoryEntry, PathKey,
    ProviderCapabilities, RevisionHandle, RevisionReader, ScmError, SourceInventory, SourceRef,
    SourceRevision, WorkingCopyReader,
};

/// Rename similarity Git must reach before it reports a rename (the
/// `rename_evidence.rs` precedent).
const FIND_RENAMES: &str = "--find-renames=50%";
/// Copy similarity, on by default here: AC #3 demands copy evidence.
const FIND_COPIES: &str = "--find-copies=50%";
/// Without `harder`, Git only considers sources that also changed in
/// the diff — the exact blind spot for "copied from an untouched
/// file", which AC #3 refuses to silence.
const FIND_COPIES_HARDER: &str = "--find-copies-harder";
/// The empty tree: the honest base for a staged index on an unborn HEAD.
const EMPTY_TREE: &str = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";

/// Cap for structured listings (name-status, ls-tree, status).
const LIST_CAP: u64 = 256 << 20;
/// Cap for one blob's bytes.
const BLOB_CAP: u64 = 64 << 20;
/// Cap for captured stderr (diagnostics only).
const STDERR_CAP: usize = 64 << 10;

fn failure(detail: impl std::fmt::Display) -> ScmError {
    ScmError::ProviderFailure {
        detail: detail.to_string(),
    }
}

/// One repository-relative path out of Git's NUL-safe wire format.
/// Non-UTF-8 names arrive lossily: refusing to report them would be
/// the fictitious emptiness AC #3 forbids; the digest still names the
/// bytes exactly.
fn path_key_from_bytes(bytes: &[u8]) -> Result<PathKey, ScmError> {
    let text = String::from_utf8_lossy(bytes);
    PathKey::new(&text).map_err(|_| ScmError::UnsafePath {
        path: text.to_string(),
    })
}

fn handle_from_sha(sha: &str) -> Result<RevisionHandle, ScmError> {
    RevisionHandle::new(sha).map_err(|_| failure(format!("malformed revision handle {sha:?}")))
}

/// One bounded git invocation: status plus capped stdout/stderr.
struct GitRun {
    code: i32,
    stdout: Vec<u8>,
    #[allow(dead_code)] // carried for future diagnostics; the named errors quote what they need
    stderr: Vec<u8>,
}

/// Reads a pipe with a hard byte cap. Over-cap returns `None` (the
/// caller turns it into a named failure); the remainder is drained so
/// git can never block writing into a closed buffer.
pub(crate) fn read_bounded(
    mut pipe: impl Read,
    cap: u64,
) -> Result<Option<Vec<u8>>, std::io::Error> {
    let mut out = Vec::new();
    let mut chunk = [0u8; 1 << 16];
    let mut over = false;
    loop {
        let read = pipe.read(&mut chunk)?;
        if read == 0 {
            break;
        }
        if !over && (out.len() as u64 + read as u64 > cap) {
            over = true;
        }
        if !over {
            out.extend_from_slice(&chunk[..read]);
        }
    }
    Ok((!over).then_some(out))
}

/// One tracked `(path, oid, kind)` row, owning its bytes.
#[derive(Debug)]
struct OwnedRow {
    path: Vec<u8>,
    oid: String,
    gitlink: bool,
}

/// Parses `git ls-tree -r -z` output: NUL-separated records of
/// `<mode> <type> <oid>\t<path>`. Strict: a malformed record fails
/// the whole read — half a listing is a fictitious diff waiting to
/// happen.
fn parse_ls_tree_z(stdout: &[u8]) -> Result<Vec<OwnedRow>, ScmError> {
    let mut rows = Vec::new();
    for record in stdout.split(|byte| *byte == b'\0') {
        if record.is_empty() {
            continue;
        }
        let header_end = record
            .iter()
            .position(|byte| *byte == b'\t')
            .ok_or_else(|| failure("malformed ls-tree record"))?;
        let (header, path) = record.split_at(header_end);
        let path = &path[1..];
        let header_text = String::from_utf8_lossy(header);
        let fields: Vec<&str> = header_text.split(' ').collect();
        if fields.len() != 3 || path.is_empty() {
            return Err(failure("malformed ls-tree record"));
        }
        match fields[1] {
            "blob" => rows.push(OwnedRow {
                path: path.to_vec(),
                oid: fields[2].to_string(),
                gitlink: false,
            }),
            // Gitlinks ride the tree as recorded pointers, not content.
            "commit" => rows.push(OwnedRow {
                path: path.to_vec(),
                oid: fields[2].to_string(),
                gitlink: true,
            }),
            other => return Err(failure(format!("unexpected ls-tree object type {other:?}"))),
        }
    }
    Ok(rows)
}

/// Parses `git ls-files -s -z` output: NUL-separated records of
/// `<mode> <oid> <stage>\t<path>`. Stage 0 rows become inventory;
/// merged stages come back as conflict paths.
fn parse_ls_files_s_z(stdout: &[u8]) -> Result<(Vec<OwnedRow>, Vec<PathKey>), ScmError> {
    let mut rows = Vec::new();
    let mut conflicts = Vec::new();
    for record in stdout.split(|byte| *byte == b'\0') {
        if record.is_empty() {
            continue;
        }
        let header_end = record
            .iter()
            .position(|byte| *byte == b'\t')
            .ok_or_else(|| failure("malformed ls-files record"))?;
        let (header, path) = record.split_at(header_end);
        let path = &path[1..];
        let header_text = String::from_utf8_lossy(header);
        let fields: Vec<&str> = header_text.split(' ').collect();
        if fields.len() != 3 || path.is_empty() {
            return Err(failure("malformed ls-files record"));
        }
        let stage: u8 = fields[2]
            .parse()
            .map_err(|_| failure("malformed ls-files stage"))?;
        if stage == 0 {
            rows.push(OwnedRow {
                path: path.to_vec(),
                oid: fields[1].to_string(),
                gitlink: fields[0] == "160000",
            });
        } else {
            // Merged stages: the index cannot classify this path. The
            // staged reader reports them as Unknown, never as a
            // delete-plus-add the index does not assert.
            conflicts.push(path_key_from_bytes(path)?);
        }
    }
    Ok((rows, conflicts))
}

/// Pairs from `-z` name-status evidence: NUL-separated fields where a
/// record is `status\0path` or `R100\0old\0new`.
struct DiffEvidence {
    renames: Vec<(PathKey, PathKey)>,
    copies: Vec<(PathKey, PathKey)>,
    plain_paths: Vec<PathKey>,
    unmerged_paths: Vec<PathKey>,
}

/// Parses `git diff(-cached) -z --name-status` output strictly.
fn parse_name_status_z(stdout: &[u8]) -> Result<DiffEvidence, ScmError> {
    let mut evidence = DiffEvidence {
        renames: Vec::new(),
        copies: Vec::new(),
        plain_paths: Vec::new(),
        unmerged_paths: Vec::new(),
    };
    let fields: Vec<&[u8]> = stdout
        .split(|byte| *byte == b'\0')
        .filter(|field| !field.is_empty())
        .collect();
    let mut index = 0;
    while index < fields.len() {
        let status = String::from_utf8_lossy(fields[index]);
        index += 1;
        let take_path = |index: &mut usize| -> Result<PathKey, ScmError> {
            let path = fields
                .get(*index)
                .ok_or_else(|| failure("truncated name-status record"))?;
            *index += 1;
            path_key_from_bytes(path)
        };
        // Unmerged pairs (UU, AA, DD, AU, UA, DU, UD): the index
        // cannot classify them — honesty is `Unknown`, never silence.
        if status.len() == 2 && is_unmerged_pair(&status) {
            evidence.unmerged_paths.push(take_path(&mut index)?);
            continue;
        }
        // Against an unmerged index, plain `git diff --cached` names
        // the path with the single letter U instead of a pair.
        if status == "U" {
            evidence.unmerged_paths.push(take_path(&mut index)?);
            continue;
        }
        let letter = status.as_bytes().first().copied().unwrap_or(b'?');
        match letter {
            b'R' | b'C' => {
                let from = take_path(&mut index)?;
                let to = take_path(&mut index)?;
                if letter == b'R' {
                    evidence.renames.push((from, to));
                } else {
                    evidence.copies.push((from, to));
                }
            }
            b'A' | b'D' | b'M' | b'T' | b'B' => {
                evidence.plain_paths.push(take_path(&mut index)?);
            }
            other => {
                return Err(failure(format!(
                    "unexpected name-status letter {:?}",
                    other as char
                )));
            }
        }
    }
    Ok(evidence)
}

/// Whether a two-letter name-status token is one of git's unmerged
/// stage combinations: every pairing of the stage letters A, D and U
/// (AA, DD, UU, AU, UA, DU, UD) marks a path the index cannot classify.
fn is_unmerged_pair(status: &str) -> bool {
    let mut bytes = status.bytes();
    let (Some(first), Some(second), None) = (bytes.next(), bytes.next(), bytes.next()) else {
        return false;
    };
    let is_stage_letter = |b: u8| matches!(b, b'U' | b'A' | b'D');
    is_stage_letter(first) && is_stage_letter(second)
}

/// One virtual merge candidate tree plus the paths Git left conflicted.
pub struct MergeCandidate {
    pub revision: SourceRevision,
    pub conflicts: Vec<PathKey>,
}

/// Reads one Git repository through the neutral SCM ports. The two
/// ports and the branch operations share every primitive here, so
/// HEAD, index and worktree can only ever be read the way they are
/// actually stored.
#[derive(Debug, Clone)]
pub struct GitScmAdapter {
    repo_root: PathBuf,
    git_program: OsString,
    /// The listing of the last immutable revision read, so reading N
    /// files of one revision lists its tree once. Shared by clones,
    /// which read the same repository through the same program.
    listing: Arc<Mutex<Option<CachedListing>>>,
}

/// The rows of one tree object id.
#[derive(Debug)]
struct CachedListing {
    object_id: String,
    rows: Arc<Vec<OwnedRow>>,
}

/// Whether a tree-ish is a full object id. Only an id names bytes that
/// can never change; a branch, `HEAD` or any other name can move
/// underneath a cached listing.
fn is_object_id(treeish: &str) -> bool {
    matches!(treeish.len(), 40 | 64) && treeish.bytes().all(|b| b.is_ascii_hexdigit())
}

impl GitScmAdapter {
    /// Binds the adapter to a repository root, spawning the system `git`.
    pub fn new(repo_root: impl Into<PathBuf>) -> Self {
        Self {
            repo_root: repo_root.into(),
            git_program: OsString::from("git"),
            listing: Arc::default(),
        }
    }

    /// Binds the adapter to an explicit git binary (a pinned toolchain
    /// path, or a test double for the "git absent" failure mode).
    pub fn with_git_program(repo_root: impl Into<PathBuf>, program: &str) -> Self {
        Self {
            repo_root: repo_root.into(),
            git_program: OsString::from(program),
            listing: Arc::default(),
        }
    }

    /// The ONE place argv is built: fixed program + `-C repo` + args,
    /// no shell, bounded captures. `code` is always available; a
    /// spawn failure is the named `ProviderFailure`, never a status
    /// the caller could mistake for "no changes".
    fn run(&self, args: &[&str], cap: u64) -> Result<GitRun, ScmError> {
        let mut child = ReapedChild::new(
            Command::new(&self.git_program)
                .arg("-C")
                .arg(&self.repo_root)
                .args(args)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .map_err(|error| failure(format!("cannot spawn git: {error}")))?,
        );
        let stderr_pipe = child
            .child()
            .stderr
            .take()
            .ok_or_else(|| failure("git stderr pipe missing"))?;
        let stderr_join = std::thread::spawn(move || {
            read_bounded(stderr_pipe, STDERR_CAP as u64)
                .ok()
                .flatten()
                .unwrap_or_default()
        });
        let stdout_pipe = child
            .child()
            .stdout
            .take()
            .ok_or_else(|| failure("git stdout pipe missing"))?;
        let stdout = read_bounded(stdout_pipe, cap)
            .map_err(|error| failure(format!("cannot read git stdout: {error}")))?
            .ok_or_else(|| {
                failure(format!(
                    "git output exceeded the {cap} byte cap; refusing a partial read"
                ))
            })?;
        let status = child
            .finish()
            .map_err(|error| failure(format!("cannot wait for git: {error}")))?;
        Ok(GitRun {
            code: status.code().unwrap_or(-1),
            stdout,
            stderr: stderr_join.join().unwrap_or_default(),
        })
    }

    /// Turns one reference into a full commit SHA. `--end-of-options`
    /// keeps a dash-prefixed reference DATA, and `^{commit}` fails on
    /// anything that is not a commit (AC #4).
    fn rev_parse_commit(&self, reference: &str) -> Result<String, ScmError> {
        let query = format!("{}^{{commit}}", reference);
        let run = self.run(
            &[
                "rev-parse",
                "--verify",
                "--quiet",
                "--end-of-options",
                &query,
            ],
            LIST_CAP,
        )?;
        if run.code != 0 {
            return Err(ScmError::ReferenceNotFound {
                provider: "git".to_string(),
                reference: reference.to_string(),
            });
        }
        let sha = String::from_utf8_lossy(&run.stdout).trim().to_string();
        if sha.len() != 40 || !sha.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(failure(format!("git returned a non-SHA revision {sha:?}")));
        }
        Ok(sha)
    }

    /// The tracked rows of one tree-ish (commit or tree SHA). The rows of
    /// the last object id listed are kept: a failed listing is never
    /// cached, and a tree-ish that is not an object id is always listed
    /// afresh.
    fn tracked_rows(&self, treeish: &str) -> Result<Arc<Vec<OwnedRow>>, ScmError> {
        let cacheable = is_object_id(treeish);
        if cacheable {
            let cached = self.listing.lock().unwrap_or_else(PoisonError::into_inner);
            if let Some(hit) = cached.as_ref().filter(|hit| hit.object_id == treeish) {
                return Ok(Arc::clone(&hit.rows));
            }
        }
        let run = self.run(&["ls-tree", "-r", "-z", "--full-name", treeish], LIST_CAP)?;
        if run.code != 0 {
            return Err(ScmError::ContentUnavailable {
                path: treeish.to_string(),
            });
        }
        let rows = Arc::new(parse_ls_tree_z(&run.stdout)?);
        if cacheable {
            *self.listing.lock().unwrap_or_else(PoisonError::into_inner) = Some(CachedListing {
                object_id: treeish.to_string(),
                rows: Arc::clone(&rows),
            });
        }
        Ok(rows)
    }

    /// Digests every blob of a listing through ONE `cat-file --batch`
    /// process: the exact bytes Git recorded for each OID, hashed with
    /// the domain's `ContentDigest` (no second hasher anywhere).
    fn inventory_for(
        &self,
        handle: &RevisionHandle,
        rows: &[OwnedRow],
    ) -> Result<SourceInventory, ScmError> {
        let mut entries = Vec::with_capacity(rows.len());
        let oids: Vec<&str> = rows
            .iter()
            .filter(|r| !r.gitlink)
            .map(|r| r.oid.as_str())
            .collect();
        let digests = self.batch_blob_digests(&oids)?;
        let mut blob_index = 0;
        for row in rows {
            let content = if row.gitlink {
                ContentDigest::from_bytes(format!("gitlink:{}", row.oid).as_bytes())
            } else {
                let digest = digests[blob_index].clone()?;
                blob_index += 1;
                digest
            };
            entries.push(InventoryEntry {
                path: path_key_from_bytes(&row.path)?,
                content,
            });
        }
        SourceInventory::new(handle, entries)
    }

    /// `git cat-file --batch` fed with every OID, streamed: each
    /// blob's bytes are hashed as they arrive, so peak memory is one
    /// blob, not the whole tree. A missing blob is a NAMED absence.
    fn batch_blob_digests(
        &self,
        oids: &[&str],
    ) -> Result<Vec<Result<ContentDigest, ScmError>>, ScmError> {
        self.batch_cat(oids, |pipe, oid, size| {
            if size > BLOB_CAP {
                drain(pipe, size)?;
                return Ok(Err(failure(format!(
                    "blob {oid} is {size} bytes, over the {BLOB_CAP} cap"
                ))));
            }
            Ok(Ok(stream_digest(pipe, size)?))
        })
    }

    /// The bytes of every OID through ONE `cat-file --batch`. A blob
    /// over `max_bytes` is reported as `TooLarge` and drained without
    /// being held, so one huge blob never costs more than a chunk.
    fn batch_blob_contents(
        &self,
        oids: &[&str],
        max_bytes: u64,
    ) -> Result<Vec<Result<BlobRead, ScmError>>, ScmError> {
        let limit = max_bytes.min(BLOB_CAP);
        self.batch_cat(oids, |pipe, _oid, size| {
            if size > limit {
                drain(pipe, size)?;
                return Ok(Ok(BlobRead::TooLarge { bytes: size }));
            }
            Ok(Ok(BlobRead::Bytes(read_blob(pipe, size)?)))
        })
    }

    /// Drives one `git cat-file --batch` process. `visit` receives the
    /// pipe positioned at the blob body and MUST consume exactly
    /// `size` bytes; the trailing newline is consumed here. An `Err`
    /// returned by `visit` aborts the whole batch (the stream is no
    /// longer trustworthy); a per-blob failure travels in the inner
    /// `Result`.
    fn batch_cat<T>(
        &self,
        oids: &[&str],
        mut visit: impl FnMut(&mut dyn Read, &str, u64) -> Result<Result<T, ScmError>, ScmError>,
    ) -> Result<Vec<Result<T, ScmError>>, ScmError> {
        if oids.is_empty() {
            return Ok(Vec::new());
        }
        let mut child = ReapedChild::new(
            Command::new(&self.git_program)
                .arg("-C")
                .arg(&self.repo_root)
                .arg("cat-file")
                .arg("--batch")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .map_err(|error| failure(format!("cannot spawn git cat-file: {error}")))?,
        );
        let mut stdin = child
            .child()
            .stdin
            .take()
            .ok_or_else(|| failure("git cat-file stdin missing"))?;
        let requests: Vec<u8> = oids
            .iter()
            .flat_map(|oid| oid.bytes().chain(std::iter::once(b'\n')))
            .collect();
        // Feeding runs on its own thread: a large request list and a
        // large response would otherwise fill both pipes and deadlock.
        let feeder =
            std::thread::spawn(move || stdin.write_all(&requests).and_then(|()| stdin.flush()));
        let mut stdout = child
            .child()
            .stdout
            .take()
            .ok_or_else(|| failure("git cat-file stdout missing"))?;
        let mut results = Vec::with_capacity(oids.len());
        for oid in oids.iter().copied() {
            let header = read_line_bounded(&mut stdout)?;
            let Some(header) = header else {
                return Err(failure("git cat-file ended early"));
            };
            if header.starts_with("missing ") || header == format!("{oid} missing") {
                results.push(Err(ScmError::ContentUnavailable {
                    path: oid.to_string(),
                }));
                continue;
            }
            let mut parts = header.rsplitn(3, ' ');
            let (size_text, kind, name) = match (parts.next(), parts.next(), parts.next()) {
                (Some(size), Some(kind), Some(name)) => (size, kind, name),
                _ => return Err(failure(format!("malformed cat-file header {header:?}"))),
            };
            if name != oid || kind != "blob" {
                return Err(failure(format!(
                    "git cat-file answered {name:?} {kind:?} while asked for {oid:?}"
                )));
            }
            let size: u64 = size_text
                .parse()
                .map_err(|_| failure(format!("malformed cat-file size {size_text:?}")))?;
            results.push(visit(&mut stdout, oid, size)?);
            let _trailing_newline = read_exact_one(&mut stdout)?;
        }
        feeder
            .join()
            .map_err(|_| failure("git cat-file feeder panicked"))?
            .map_err(|error| failure(format!("cannot feed git cat-file: {error}")))?;
        let status = child
            .finish()
            .map_err(|error| failure(format!("cannot wait for git cat-file: {error}")))?;
        if !status.success() {
            return Err(failure("git cat-file --batch exited unsuccessfully"));
        }
        Ok(results)
    }

    /// Rename/copy/unknown evidence folded onto the derived set. A
    /// pair the derivation cannot support is kept as a conservative
    /// `Unknown` on the new path — evidence never vanishes silently.
    fn fold_evidence(
        mut changes: ChangeSet,
        base_inventory: &SourceInventory,
        evidence: DiffEvidence,
    ) -> ChangeSet {
        changes.apply_rename_pairing(&evidence.renames);
        changes.apply_copy_pairing(base_inventory, &evidence.copies);
        let mut unclassified: Vec<PathKey> = Vec::new();
        for (from, to) in evidence.renames.iter().chain(evidence.copies.iter()) {
            let claimed = changes.changes().iter().any(|c| {
                c.path() == to
                    && matches!(
                        c.kind(),
                        ChangeKind::Renamed { from: f } | ChangeKind::Copied { from: f } if *f == *from
                    )
            });
            if !claimed {
                unclassified.push(to.clone());
            }
        }
        let conflict_and_plain: Vec<PathKey> = evidence
            .unmerged_paths
            .iter()
            .cloned()
            .chain(
                evidence
                    .plain_paths
                    .iter()
                    .filter(|p| !changes.changes().iter().any(|c| c.path() == *p))
                    .cloned(),
            )
            .collect();
        let mut seen = unclassified.clone();
        seen.extend(conflict_and_plain);
        for path in seen {
            changes.mark_unknown(path);
        }
        changes
    }

    fn source_or_refuse(&self, source: &SourceRef, reference: &str) -> Result<(), ScmError> {
        if source.provider().as_str() != "git" {
            return Err(ScmError::ReferenceNotFound {
                provider: source.provider().as_str().to_string(),
                reference: reference.to_string(),
            });
        }
        Ok(())
    }

    /// The source identity of THIS repository: provider `git`, project
    /// named by the root path the adapter was bound to.
    fn source(&self) -> Result<SourceRef, ScmError> {
        SourceRef::new("git", &self.repo_root.display().to_string())
            .map_err(|_| failure("repository root is not a valid project name"))
    }

    /// Resolves either side of a branch comparison and produces the
    /// three-dot change set `B...F`: merge-base against F, with both
    /// full SHAs stated in the handles.
    pub fn changes_since_merge_base(
        &self,
        base_ref: &str,
        head_ref: &str,
    ) -> Result<ChangeSet, ScmError> {
        let source = self.source()?;
        let base_head =
            self.merge_base(base_ref, head_ref)?
                .ok_or_else(|| ScmError::MissingAncestor {
                    base: base_ref.to_string(),
                    head: head_ref.to_string(),
                })?;
        let head_sha = self.rev_parse_commit(head_ref)?;
        let head_revision = self.revision_for(&source, &head_sha)?;
        self.changes_between(&base_head, &head_revision)
    }

    /// The merge-base of two references, or `None` when they share no
    /// recorded history (unrelated lines, shallow grafts).
    pub fn merge_base(&self, a: &str, b: &str) -> Result<Option<SourceRevision>, ScmError> {
        let sha_a = self.rev_parse_commit(a)?;
        let sha_b = self.rev_parse_commit(b)?;
        let run = self.run(&["merge-base", &sha_a, &sha_b], LIST_CAP)?;
        match run.code {
            0 => {
                let mb = String::from_utf8_lossy(&run.stdout).trim().to_string();
                if mb.len() != 40 {
                    return Err(failure(format!("malformed merge-base output {mb:?}")));
                }
                let source = self.source()?;
                Ok(Some(self.revision_for(&source, &mb)?))
            }
            1 => Ok(None),
            _ => Err(ScmError::ReferenceNotFound {
                provider: "git".to_string(),
                reference: format!("{a} or {b}"),
            }),
        }
    }

    /// Builds the VIRTUAL merge tree `merge(ours, theirs)` with
    /// `git merge-tree --write-tree` and names its conflicts. The tree
    /// is an ordinary content-addressed object: the read-only ports
    /// consume it like any other revision, and its handle is its
    /// explicit tree SHA.
    pub fn merge_candidate(
        &self,
        ours_ref: &str,
        theirs_ref: &str,
    ) -> Result<MergeCandidate, ScmError> {
        let sha_ours = self.rev_parse_commit(ours_ref)?;
        let sha_theirs = self.rev_parse_commit(theirs_ref)?;
        let run = self.run(
            &[
                "merge-tree",
                "--write-tree",
                "-z",
                "--name-only",
                &sha_ours,
                &sha_theirs,
            ],
            LIST_CAP,
        )?;
        if run.code > 1 {
            return Err(failure(format!(
                "git merge-tree failed: {}",
                String::from_utf8_lossy(&run.stderr).trim()
            )));
        }
        // `-z` rewrites the record layout: the tree OID ends with a
        // NUL, and each conflicted path is its own NUL field. An empty
        // field closes the list of paths; what follows is the merge's
        // informational messages (localized prose, not paths).
        let mut fields = run.stdout.split(|byte| *byte == b'\0');
        let tree_sha = fields
            .next()
            .map(|f| String::from_utf8_lossy(f).trim().to_string())
            .ok_or_else(|| failure("git merge-tree returned no tree OID"))?;
        if tree_sha.len() != 40 || !tree_sha.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(failure(format!("malformed merge-tree output {tree_sha:?}")));
        }
        let conflicts: Vec<PathKey> = fields
            .take_while(|field| !field.is_empty())
            .map(path_key_from_bytes)
            .collect::<Result<_, _>>()?;
        let source = self.source()?;
        let revision = self.revision_for(&source, &tree_sha)?;
        Ok(MergeCandidate {
            revision,
            conflicts,
        })
    }

    /// `D...merge(D,F)`: the changes from OURS' commit to the virtual
    /// merge tree. Different head handle (a tree SHA) and different
    /// coverage than the three-dot — exactly the distinction AC #2
    /// demands. Conflicted paths are forced to `Unknown`.
    pub fn changes_vs_merge_candidate(
        &self,
        ours_ref: &str,
        theirs_ref: &str,
    ) -> Result<ChangeSet, ScmError> {
        let sha_ours = self.rev_parse_commit(ours_ref)?;
        let candidate = self.merge_candidate(ours_ref, theirs_ref)?;
        let source = self.source()?;
        let ours = self.revision_for(&source, &sha_ours)?;
        let base_inventory = self.tracked_inventory(ours.handle())?;
        let head_inventory = self.tracked_inventory(candidate.revision.handle())?;
        let run = self.run(
            &[
                "diff",
                "-z",
                "--name-status",
                FIND_RENAMES,
                FIND_COPIES,
                FIND_COPIES_HARDER,
                &sha_ours,
                candidate.revision.handle().as_str(),
            ],
            LIST_CAP,
        )?;
        if run.code != 0 {
            return Err(failure(format!(
                "git diff against the candidate tree failed (code {})",
                run.code
            )));
        }
        let evidence = parse_name_status_z(&run.stdout)?;
        let derived = ChangeSet::between_raw(&base_inventory, &head_inventory);
        let mut changes = Self::fold_evidence(derived, &base_inventory, evidence);
        for conflict in candidate.conflicts {
            changes.mark_unknown(conflict);
        }
        Ok(changes)
    }

    /// Inventory without digests errors: ls-tree + batch in one step.
    fn tracked_inventory(&self, handle: &RevisionHandle) -> Result<SourceInventory, ScmError> {
        let rows = self.tracked_rows(handle.as_str())?;
        self.inventory_for(handle, &rows)
    }

    fn revision_for(&self, source: &SourceRef, sha: &str) -> Result<SourceRevision, ScmError> {
        let handle = handle_from_sha(sha)?;
        let inventory = self.tracked_inventory(&handle)?;
        Ok(SourceRevision::new(
            source.clone(),
            handle,
            inventory.content_digest(),
        ))
    }

    /// Shared ancestry check for `changes_between`: exit 1 means
    /// "not an ancestor" (false), anything else is a named failure —
    /// a shallow clone with an unknown object never answers quietly.
    fn ancestor_answer(&self, base: &str, head: &str) -> Result<bool, ScmError> {
        let run = self.run(&["merge-base", "--is-ancestor", base, head], LIST_CAP)?;
        match run.code {
            0 => Ok(true),
            1 => Ok(false),
            _ => Err(ScmError::ReferenceNotFound {
                provider: "git".to_string(),
                reference: format!("{base} or {head}"),
            }),
        }
    }

    fn diff_evidence(
        &self,
        extra: &[&str],
        a: &str,
        b: Option<&str>,
    ) -> Result<DiffEvidence, ScmError> {
        let mut args: Vec<&str> = vec![
            "diff",
            "-z",
            "--name-status",
            FIND_RENAMES,
            FIND_COPIES,
            FIND_COPIES_HARDER,
        ];
        args.extend_from_slice(extra);
        args.push(a);
        if let Some(b) = b {
            args.push(b);
        }
        let run = self.run(&args, LIST_CAP)?;
        if run.code != 0 {
            return Err(failure(format!("git diff failed (code {})", run.code)));
        }
        parse_name_status_z(&run.stdout)
    }
}

impl RevisionReader for GitScmAdapter {
    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            rename_evidence: true,
            ancestor_queries: true,
            staged_index: true,
            dirty_tree: true,
        }
    }

    fn resolve(&self, source: &SourceRef, reference: &str) -> Result<SourceRevision, ScmError> {
        self.source_or_refuse(source, reference)?;
        let sha = self.rev_parse_commit(reference)?;
        self.revision_for(source, &sha)
    }

    fn inventory(&self, revision: &SourceRevision) -> Result<SourceInventory, ScmError> {
        self.tracked_inventory(revision.handle())
    }

    fn content(&self, revision: &SourceRevision, path: &PathKey) -> Result<Vec<u8>, ScmError> {
        let rows = self.tracked_rows(revision.handle().as_str())?;
        let row = rows
            .iter()
            .find(|row| path_key_from_bytes(&row.path).ok().as_ref() == Some(path))
            .ok_or_else(|| ScmError::ContentUnavailable {
                path: path.as_str().to_string(),
            })?;
        if row.gitlink {
            return Err(ScmError::ContentUnavailable {
                path: path.as_str().to_string(),
            });
        }
        let run = self.run(&["cat-file", "blob", row.oid.as_str()], BLOB_CAP)?;
        if run.code != 0 {
            return Err(ScmError::ContentUnavailable {
                path: path.as_str().to_string(),
            });
        }
        Ok(run.stdout)
    }

    fn contents(
        &self,
        revision: &SourceRevision,
        paths: &[PathKey],
        max_bytes: u64,
    ) -> Vec<Result<BlobRead, ScmError>> {
        self.blobs_at(revision.handle(), paths, max_bytes)
    }
}

impl GitScmAdapter {
    /// The bytes of `paths` as the revision named by `handle` holds them,
    /// through one batched read. The handle may be a commit or a tree, so a
    /// virtual merge tree is read the same way as a commit. One answer per
    /// path, in order: a path the revision does not hold, or a gitlink, is
    /// `ContentUnavailable`.
    pub fn blobs_at(
        &self,
        handle: &RevisionHandle,
        paths: &[PathKey],
        max_bytes: u64,
    ) -> Vec<Result<BlobRead, ScmError>> {
        match self.tracked_rows(handle.as_str()) {
            Ok(rows) => self.blobs_of_rows(&rows, paths, max_bytes),
            Err(error) => paths.iter().map(|_| Err(error.clone())).collect(),
        }
    }

    /// The bytes of `paths` as the INDEX holds them (stage 0), through one
    /// batched read. A path the index does not hold at stage 0 is
    /// `ContentUnavailable`.
    pub fn staged_blobs(
        &self,
        paths: &[PathKey],
        max_bytes: u64,
    ) -> Vec<Result<BlobRead, ScmError>> {
        let rows = self
            .run(&["ls-files", "--stage", "-z"], LIST_CAP)
            .and_then(|run| {
                if run.code != 0 {
                    return Err(failure(format!("git ls-files failed (code {})", run.code)));
                }
                parse_ls_files_s_z(&run.stdout).map(|(rows, _conflicts)| rows)
            });
        match rows {
            Ok(rows) => self.blobs_of_rows(&rows, paths, max_bytes),
            Err(error) => paths.iter().map(|_| Err(error.clone())).collect(),
        }
    }

    fn blobs_of_rows(
        &self,
        rows: &[OwnedRow],
        paths: &[PathKey],
        max_bytes: u64,
    ) -> Vec<Result<BlobRead, ScmError>> {
        let unavailable = |path: &PathKey| ScmError::ContentUnavailable {
            path: path.as_str().to_string(),
        };
        let mut row_of: std::collections::HashMap<PathKey, &OwnedRow> =
            std::collections::HashMap::with_capacity(rows.len());
        for row in rows {
            if let Ok(key) = path_key_from_bytes(&row.path) {
                row_of.insert(key, row);
            }
        }
        let mut wanted: Vec<&str> = Vec::new();
        let mut slot: Vec<Option<usize>> = Vec::with_capacity(paths.len());
        for path in paths {
            match row_of.get(path) {
                Some(row) if !row.gitlink => {
                    slot.push(Some(wanted.len()));
                    wanted.push(row.oid.as_str());
                }
                _ => slot.push(None),
            }
        }
        let mut blobs = match self.batch_blob_contents(&wanted, max_bytes) {
            Ok(blobs) => blobs.into_iter().map(Some).collect::<Vec<_>>(),
            Err(error) => return paths.iter().map(|_| Err(error.clone())).collect(),
        };
        paths
            .iter()
            .zip(slot)
            .map(|(path, slot)| match slot {
                Some(index) => blobs[index]
                    .take()
                    .unwrap_or_else(|| Err(unavailable(path))),
                None => Err(unavailable(path)),
            })
            .collect()
    }
}

impl ChangeReader for GitScmAdapter {
    fn changes_between(
        &self,
        base: &SourceRevision,
        head: &SourceRevision,
    ) -> Result<ChangeSet, ScmError> {
        if !self.ancestor_answer(base.handle().as_str(), head.handle().as_str())? {
            return Err(ScmError::MissingAncestor {
                base: base.handle().as_str().to_string(),
                head: head.handle().as_str().to_string(),
            });
        }
        self.derive_with_evidence(
            base.handle(),
            head.handle(),
            &[],
            base.handle().as_str(),
            Some(head.handle().as_str()),
        )
    }

    fn is_ancestor(
        &self,
        maybe_ancestor: &SourceRevision,
        maybe_descendant: &SourceRevision,
    ) -> Result<bool, ScmError> {
        self.ancestor_answer(
            maybe_ancestor.handle().as_str(),
            maybe_descendant.handle().as_str(),
        )
    }
}

impl GitScmAdapter {
    fn derive_with_evidence(
        &self,
        base_handle: &RevisionHandle,
        head_handle: &RevisionHandle,
        extra: &[&str],
        a: &str,
        b: Option<&str>,
    ) -> Result<ChangeSet, ScmError> {
        let base_inventory = self.tracked_inventory(base_handle)?;
        let head_inventory = self.tracked_inventory(head_handle)?;
        let evidence = self.diff_evidence(extra, a, b)?;
        let derived = ChangeSet::between_raw(&base_inventory, &head_inventory);
        Ok(Self::fold_evidence(derived, &base_inventory, evidence))
    }
}

impl WorkingCopyReader for GitScmAdapter {
    fn staged_changes(&self, source: &SourceRef) -> Result<ChangeSet, ScmError> {
        self.source_or_refuse(source, "HEAD")?;
        // Unborn HEAD is not an error here: the index is real, and the
        // honest base is the empty tree, named by its own OID.
        let base_sha = match self.run(
            &[
                "rev-parse",
                "--verify",
                "--quiet",
                "--end-of-options",
                "HEAD",
            ],
            LIST_CAP,
        ) {
            Ok(run) if run.code == 0 => String::from_utf8_lossy(&run.stdout).trim().to_string(),
            _ => EMPTY_TREE.to_string(),
        };
        let base_handle = handle_from_sha(&base_sha)?;
        let head_handle = handle_from_sha("index")?;

        let base_inventory = self.tracked_inventory(&base_handle)?;
        let run = self.run(&["ls-files", "--stage", "-z"], LIST_CAP)?;
        if run.code != 0 {
            return Err(failure(format!("git ls-files failed (code {})", run.code)));
        }
        let (rows, conflicts) = parse_ls_files_s_z(&run.stdout)?;
        let index_inventory = self.inventory_for(&head_handle, &rows)?;

        let derived = ChangeSet::between_raw(&base_inventory, &index_inventory);
        let evidence = self.diff_evidence(&["--cached"], &base_sha, None)?;
        let mut changes = Self::fold_evidence(derived, &base_inventory, evidence);
        for conflict in conflicts {
            changes.mark_unknown(conflict);
        }
        Ok(changes)
    }

    fn dirty_paths(&self, source: &SourceRef) -> Result<Vec<PathKey>, ScmError> {
        self.source_or_refuse(source, "status")?;
        let run = self.run(
            &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
            LIST_CAP,
        )?;
        if run.code != 0 {
            return Err(failure(format!("git status failed (code {})", run.code)));
        }
        parse_status_z(&run.stdout)
    }
}

/// Parses `git status --porcelain=v1 -z`: records are `XY <path>`, and
/// rename/copy rows carry the origin path as an extra NUL field. Dirty
/// = anything whose WORKTREE column is not a blank (plus `??`).
fn parse_status_z(stdout: &[u8]) -> Result<Vec<PathKey>, ScmError> {
    let fields: Vec<&[u8]> = stdout
        .split(|byte| *byte == b'\0')
        .filter(|field| !field.is_empty())
        .collect();
    let mut dirty = Vec::new();
    let mut index = 0;
    while index < fields.len() {
        let record = &fields[index];
        index += 1;
        if record.len() < 4 || record[2] != b' ' {
            return Err(failure("malformed status record"));
        }
        let (xy, path) = (record[0], record[1]);
        let key = path_key_from_bytes(&record[3..])?;
        let untracked = xy == b'?' && path == b'?';
        let worktree_dirty = !untracked && path != b' ';
        if worktree_dirty {
            dirty.push(key.clone());
        }
        if untracked {
            dirty.push(key);
        }
        // A staged rename/copy names its origin in the next field; the
        // origin's worktree absence is dirt of its own.
        if matches!(xy, b'R' | b'C') && index < fields.len() {
            let origin = fields[index];
            index += 1;
            let origin_key = path_key_from_bytes(origin)?;
            if !dirty.contains(&origin_key) {
                dirty.push(origin_key);
            }
        }
    }
    Ok(dirty)
}

fn read_line_bounded(mut pipe: impl Read) -> Result<Option<String>, ScmError> {
    let mut line = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        if line.len() as u64 > BLOB_CAP {
            return Err(failure("git cat-file header line over cap"));
        }
        if pipe.read(&mut byte).map_err(|e| failure(e.to_string()))? == 0 {
            return Ok(None);
        }
        if byte[0] == b'\n' {
            return Ok(Some(String::from_utf8_lossy(&line).to_string()));
        }
        line.push(byte[0]);
    }
}

fn read_exact_one(mut pipe: impl Read) -> Result<u8, ScmError> {
    let mut byte = [0u8; 1];
    if pipe.read(&mut byte).map_err(|e| failure(e.to_string()))? == 0 {
        return Err(failure("git cat-file ended mid-record"));
    }
    Ok(byte[0])
}

/// Reads exactly `size` bytes into memory (the caller bounded `size`).
fn read_blob(mut pipe: impl Read, size: u64) -> Result<Vec<u8>, ScmError> {
    let mut bytes = vec![0u8; size as usize];
    pipe.read_exact(&mut bytes)
        .map_err(|e| failure(format!("git cat-file ended mid-blob: {e}")))?;
    Ok(bytes)
}

/// A spawned git process that never outlives the scope that owns it:
/// on any early return it is killed and reaped, so a broken stream
/// cannot leave an orphan or a zombie behind.
struct ReapedChild(Option<std::process::Child>);

impl ReapedChild {
    fn new(child: std::process::Child) -> Self {
        Self(Some(child))
    }

    fn child(&mut self) -> &mut std::process::Child {
        self.0
            .as_mut()
            .expect("the child is held until it is finished")
    }

    /// Waits for a normal exit, handing the process back as finished.
    fn finish(mut self) -> std::io::Result<std::process::ExitStatus> {
        let mut child = self
            .0
            .take()
            .expect("the child is held until it is finished");
        child.wait()
    }
}

impl Drop for ReapedChild {
    fn drop(&mut self) {
        if let Some(mut child) = self.0.take() {
            let _already_gone = child.kill();
            let _reaped = child.wait();
        }
    }
}

/// Discards exactly `size` bytes so the next batch header stays aligned.
fn drain(mut pipe: impl Read, size: u64) -> Result<(), ScmError> {
    let copied = std::io::copy(&mut (&mut pipe).take(size), &mut std::io::sink())
        .map_err(|e| failure(e.to_string()))?;
    if copied != size {
        return Err(failure("git cat-file ended mid-blob"));
    }
    Ok(())
}

/// Streams exactly `size` bytes into the domain hasher, chunk by
/// chunk: peak memory is one chunk, never one blob.
fn stream_digest(mut pipe: impl Read, size: u64) -> Result<ContentDigest, ScmError> {
    let mut pending = size;
    let mut chunk = [0u8; 1 << 16];
    let mut collector = Vec::with_capacity(size.min(1 << 20) as usize);
    while pending > 0 {
        let want = chunk.len().min(pending as usize);
        let read = pipe
            .read(&mut chunk[..want])
            .map_err(|e| failure(e.to_string()))?;
        if read == 0 {
            return Err(failure("git cat-file ended mid-blob"));
        }
        collector.extend_from_slice(&chunk[..read]);
        pending -= read as u64;
    }
    Ok(ContentDigest::from_bytes(&collector))
}
