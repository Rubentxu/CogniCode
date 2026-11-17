//! A candidate revision that exists only in memory: a base revision plus
//! the edits someone proposes over it.
//!
//! [`OverlayReader`] decorates any [`RevisionReader`]. Asked about the
//! candidate it answers from the base plus the edits; asked about anything
//! else it delegates untouched. Nothing is written anywhere: the candidate
//! has no counterpart in the provider, and its handle is derived from the
//! base, the session and the edits so that it can never be mistaken for a
//! real revision handle, nor for the candidate of another session.

use std::collections::BTreeMap;

use sha2::{Digest as _, Sha256};

use super::error::ScmError;
use super::identity::{ContentDigest, PathKey, RevisionHandle, SourceRef, SourceRevision};
use super::inventory::{InventoryEntry, SourceInventory};
use super::ports::{BlobRead, ProviderCapabilities, RevisionReader};

/// One proposed change to a tracked path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Edit {
    /// The path holds these bytes in the candidate, whether or not the base
    /// tracks it.
    Write(Vec<u8>),
    /// The base tracks the path and the candidate does not.
    Delete,
}

/// The candidate revision `base` + `edits`, readable through the port.
pub struct OverlayReader<'a> {
    reader: &'a dyn RevisionReader,
    base: SourceRevision,
    candidate: SourceRevision,
    inventory: SourceInventory,
    edits: BTreeMap<PathKey, Edit>,
}

impl<'a> OverlayReader<'a> {
    /// Applies `edits` over `base` as read through `reader`.
    ///
    /// `session` scopes the candidate handle: the same edits proposed from
    /// two sessions name two distinct candidates.
    ///
    /// # Errors
    /// The base inventory cannot be read, or an edit deletes a path the base
    /// does not track.
    pub fn new(
        reader: &'a dyn RevisionReader,
        base: &SourceRevision,
        session: &str,
        edits: BTreeMap<PathKey, Edit>,
    ) -> Result<Self, ScmError> {
        let mut tracked: BTreeMap<PathKey, ContentDigest> = reader
            .inventory(base)?
            .entries()
            .iter()
            .map(|entry| (entry.path.clone(), entry.content.clone()))
            .collect();
        for (path, edit) in &edits {
            match edit {
                Edit::Write(bytes) => {
                    tracked.insert(path.clone(), ContentDigest::from_bytes(bytes));
                }
                Edit::Delete => {
                    if tracked.remove(path).is_none() {
                        return Err(ScmError::ProviderFailure {
                            detail: format!(
                                "cannot delete `{}`: the base revision does not track it",
                                path.as_str()
                            ),
                        });
                    }
                }
            }
        }
        let handle = candidate_handle(base, session, &edits)?;
        let entries = tracked
            .into_iter()
            .map(|(path, content)| InventoryEntry { path, content })
            .collect();
        let inventory = SourceInventory::new(&handle, entries)?;
        let candidate =
            SourceRevision::new(base.source().clone(), handle, inventory.content_digest());
        Ok(Self {
            reader,
            base: base.clone(),
            candidate,
            inventory,
            edits,
        })
    }

    /// The revision the edits apply to.
    pub fn base(&self) -> &SourceRevision {
        &self.base
    }

    /// The in-memory revision: base plus edits.
    pub fn candidate(&self) -> &SourceRevision {
        &self.candidate
    }

    /// The paths the edits touch, in path order.
    pub fn edited_paths(&self) -> impl Iterator<Item = &PathKey> {
        self.edits.keys()
    }

    fn is_candidate(&self, revision: &SourceRevision) -> bool {
        revision.handle() == self.candidate.handle()
    }
}

/// `overlay:<digest>`, where the digest folds the session, the base identity
/// and every edit, length-prefixed so no regrouping collides.
fn candidate_handle(
    base: &SourceRevision,
    session: &str,
    edits: &BTreeMap<PathKey, Edit>,
) -> Result<RevisionHandle, ScmError> {
    let mut hasher = Sha256::new();
    let mut field = |bytes: &[u8]| {
        hasher.update((bytes.len() as u64).to_le_bytes());
        hasher.update(bytes);
    };
    field(session.as_bytes());
    field(base.handle().as_str().as_bytes());
    field(base.content().as_hex().as_bytes());
    for (path, edit) in edits {
        field(path.as_str().as_bytes());
        match edit {
            Edit::Write(bytes) => {
                field(b"write");
                field(ContentDigest::from_bytes(bytes).as_hex().as_bytes());
            }
            Edit::Delete => field(b"delete"),
        }
    }
    RevisionHandle::new(&format!("overlay:{}", hex::encode(hasher.finalize())))
}

impl RevisionReader for OverlayReader<'_> {
    fn capabilities(&self) -> ProviderCapabilities {
        self.reader.capabilities()
    }

    fn resolve(&self, source: &SourceRef, reference: &str) -> Result<SourceRevision, ScmError> {
        self.reader.resolve(source, reference)
    }

    fn inventory(&self, revision: &SourceRevision) -> Result<SourceInventory, ScmError> {
        if self.is_candidate(revision) {
            return Ok(self.inventory.clone());
        }
        self.reader.inventory(revision)
    }

    fn content(&self, revision: &SourceRevision, path: &PathKey) -> Result<Vec<u8>, ScmError> {
        if !self.is_candidate(revision) {
            return self.reader.content(revision, path);
        }
        match self.edits.get(path) {
            Some(Edit::Write(bytes)) => Ok(bytes.clone()),
            Some(Edit::Delete) => Err(ScmError::ContentUnavailable {
                path: path.as_str().to_string(),
            }),
            None => self.reader.content(&self.base, path),
        }
    }

    fn contents(
        &self,
        revision: &SourceRevision,
        paths: &[PathKey],
        max_bytes: u64,
    ) -> Vec<Result<BlobRead, ScmError>> {
        if !self.is_candidate(revision) {
            return self.reader.contents(revision, paths, max_bytes);
        }
        let mut answers: Vec<Option<Result<BlobRead, ScmError>>> = paths
            .iter()
            .map(|path| {
                self.edits.get(path).map(|edit| match edit {
                    Edit::Write(bytes) if bytes.len() as u64 > max_bytes => {
                        Ok(BlobRead::TooLarge {
                            bytes: bytes.len() as u64,
                        })
                    }
                    Edit::Write(bytes) => Ok(BlobRead::Bytes(bytes.clone())),
                    Edit::Delete => Err(ScmError::ContentUnavailable {
                        path: path.as_str().to_string(),
                    }),
                })
            })
            .collect();
        let unedited: Vec<PathKey> = paths
            .iter()
            .zip(&answers)
            .filter(|(_, answer)| answer.is_none())
            .map(|(path, _)| path.clone())
            .collect();
        let mut from_base = self
            .reader
            .contents(&self.base, &unedited, max_bytes)
            .into_iter();
        answers
            .iter_mut()
            .filter(|answer| answer.is_none())
            .for_each(|answer| *answer = from_base.next());
        answers
            .into_iter()
            .zip(paths)
            .map(|(answer, path)| {
                answer.unwrap_or_else(|| {
                    Err(ScmError::ContentUnavailable {
                        path: path.as_str().to_string(),
                    })
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::scm::fake::FakeSource;

    fn path(text: &str) -> PathKey {
        PathKey::new(text).expect("fixture path")
    }

    fn source() -> SourceRef {
        SourceRef::new("fake", "overlay-tests").expect("fixture source")
    }

    fn provider() -> FakeSource {
        FakeSource::new("fake", ProviderCapabilities::NONE).with_revision(
            "main",
            &[],
            &[("a.rs", b"fn a() {}\n"), ("b.rs", b"fn b() {}\n")],
        )
    }

    fn edits(list: &[(&str, Edit)]) -> BTreeMap<PathKey, Edit> {
        list.iter()
            .map(|(name, edit)| (path(name), edit.clone()))
            .collect()
    }

    fn write(text: &str) -> Edit {
        Edit::Write(text.as_bytes().to_vec())
    }

    #[test]
    fn the_candidate_is_distinct_from_the_base_and_reflects_every_edit() {
        let fake = provider();
        let base = fake.resolve(&source(), "main").expect("base");
        let overlay = OverlayReader::new(
            &fake,
            &base,
            "s1",
            edits(&[
                ("a.rs", write("fn a2() {}\n")),
                ("b.rs", Edit::Delete),
                ("c.rs", write("fn c() {}\n")),
            ]),
        )
        .expect("overlay");

        assert_ne!(overlay.candidate().handle(), base.handle());
        assert_ne!(overlay.candidate().content(), base.content());
        let inventory = overlay.inventory(overlay.candidate()).expect("inventory");
        let tracked: Vec<&str> = inventory
            .entries()
            .iter()
            .map(|entry| entry.path.as_str())
            .collect();
        assert_eq!(tracked, vec!["a.rs", "c.rs"]);
        assert_eq!(
            inventory.find(&path("a.rs")).expect("a.rs").content,
            ContentDigest::from_bytes(b"fn a2() {}\n")
        );
    }

    #[test]
    fn the_base_is_read_through_untouched() {
        let fake = provider();
        let base = fake.resolve(&source(), "main").expect("base");
        let overlay = OverlayReader::new(
            &fake,
            &base,
            "s1",
            edits(&[("a.rs", write("fn a2() {}\n"))]),
        )
        .expect("overlay");

        assert_eq!(
            overlay.inventory(&base).expect("base inventory"),
            fake.inventory(&base).expect("provider inventory")
        );
        assert_eq!(
            overlay.content(&base, &path("a.rs")).expect("base bytes"),
            b"fn a() {}\n".to_vec()
        );
    }

    #[test]
    fn candidate_handles_differ_per_session_and_per_edits_and_repeat_for_the_same_input() {
        let fake = provider();
        let base = fake.resolve(&source(), "main").expect("base");
        let handle = |session: &str, text: &str| {
            OverlayReader::new(&fake, &base, session, edits(&[("a.rs", write(text))]))
                .expect("overlay")
                .candidate()
                .handle()
                .clone()
        };

        assert_eq!(handle("s1", "fn x() {}\n"), handle("s1", "fn x() {}\n"));
        assert_ne!(handle("s1", "fn x() {}\n"), handle("s2", "fn x() {}\n"));
        assert_ne!(handle("s1", "fn x() {}\n"), handle("s1", "fn y() {}\n"));
        assert!(handle("s1", "fn x() {}\n").as_str().starts_with("overlay:"));
    }

    #[test]
    fn the_candidate_reads_edited_bytes_from_memory_and_the_rest_from_the_base() {
        let fake = provider();
        let base = fake.resolve(&source(), "main").expect("base");
        let overlay = OverlayReader::new(
            &fake,
            &base,
            "s1",
            edits(&[("a.rs", write("fn a2() {}\n")), ("b.rs", Edit::Delete)]),
        )
        .expect("overlay");
        let candidate = overlay.candidate().clone();

        assert_eq!(
            overlay.content(&candidate, &path("a.rs")).expect("edited"),
            b"fn a2() {}\n".to_vec()
        );
        assert!(matches!(
            overlay.content(&candidate, &path("b.rs")),
            Err(ScmError::ContentUnavailable { .. })
        ));
    }

    #[test]
    fn a_batched_read_keeps_the_order_and_the_size_bound() {
        let fake = provider();
        let base = fake.resolve(&source(), "main").expect("base");
        let overlay = OverlayReader::new(
            &fake,
            &base,
            "s1",
            edits(&[("a.rs", write("fn a_much_longer_name() {}\n"))]),
        )
        .expect("overlay");
        let candidate = overlay.candidate().clone();

        let answers = overlay.contents(&candidate, &[path("b.rs"), path("a.rs")], 12);

        assert_eq!(
            answers,
            vec![
                Ok(BlobRead::Bytes(b"fn b() {}\n".to_vec())),
                Ok(BlobRead::TooLarge { bytes: 27 }),
            ]
        );
    }

    #[test]
    fn deleting_a_path_the_base_does_not_track_is_refused() {
        let fake = provider();
        let base = fake.resolve(&source(), "main").expect("base");

        let refused = OverlayReader::new(&fake, &base, "s1", edits(&[("nope.rs", Edit::Delete)]));

        assert!(matches!(refused, Err(ScmError::ProviderFailure { .. })));
    }
}
