//! CAA-02 / CGH-020 — SCM-neutral source identity and ports.
//!
//! These tests pin the contract that later slices (Git adapter CAA-03,
//! immutable snapshots CAA-04, Guard BasisProvider CAA-05) build on:
//!
//! 1. Two snapshots can be captured from a FAKE provider with no Git
//!    dependency at all (AC #1).
//! 2. Identical content across different SCM revisions shares one
//!    content digest — the reuse key for parsing (AC #2). Semantic
//!    dependencies control everything else and are NOT this module's
//!    responsibility.
//! 3. `ChangeSet` classifies Added, Deleted, Modified, Rename, Copy
//!    and Unknown with conservative handling, and providers declare
//!    optional capabilities honestly (AC #3).
//! 4. Missing references, absent ancestry, unsafe paths and
//!    unavailable content all fail closed with named errors (AC #4).
//!
//! The production Git adapter does not exist yet (CAA-03); that is the
//! point — the ports must be satisfiable by a pure in-memory provider.

use std::collections::HashMap;

use cognicode_core::domain::scm::{
    Change, ChangeKind, ChangeReader, ChangeSet, ContentDigest, InventoryEntry, PathKey,
    ProviderCapabilities, RevisionHandle, RevisionReader, ScmError, SourceInventory, SourceRef,
    SourceRevision, WorkingCopyReader,
};

fn path(p: &str) -> PathKey {
    PathKey::new(p).unwrap_or_else(|e| panic!("path {p:?} rejected: {e}"))
}

fn digest_of(text: &[u8]) -> ContentDigest {
    ContentDigest::from_bytes(text)
}

fn source() -> SourceRef {
    SourceRef::new("fake-provider", "demo-project").expect("non-empty source ref")
}

// ----- a pure in-memory provider: no Git, no filesystem --------------

struct StoredRevision {
    handle: RevisionHandle,
    inventory: SourceInventory,
    contents: HashMap<PathKey, Vec<u8>>,
    parents: Vec<RevisionHandle>,
}

struct FakeProvider {
    revisions: HashMap<String, StoredRevision>,
    capabilities: ProviderCapabilities,
    staged: Vec<Change>,
    dirty: Vec<PathKey>,
}

impl FakeProvider {
    fn new() -> Self {
        let mut revisions = HashMap::new();
        // rev-a: two files. rev-b: one of them changed.
        let entry_main = InventoryEntry {
            path: path("src/main.rs"),
            content: digest_of(b"fn main() {}"),
        };
        let entry_lib_v1 = InventoryEntry {
            path: path("src/lib.rs"),
            content: digest_of(b"pub fn one() {}"),
        };
        let entry_lib_v2 = InventoryEntry {
            path: path("src/lib.rs"),
            content: digest_of(b"pub fn two() {}"),
        };
        let mut contents_a: HashMap<PathKey, Vec<u8>> = HashMap::new();
        contents_a.insert(entry_main.path.clone(), b"fn main() {}".to_vec());
        contents_a.insert(entry_lib_v1.path.clone(), b"pub fn one() {}".to_vec());
        let mut contents_b: HashMap<PathKey, Vec<u8>> = HashMap::new();
        contents_b.insert(entry_main.path.clone(), b"fn main() {}".to_vec());
        contents_b.insert(entry_lib_v2.path.clone(), b"pub fn two() {}".to_vec());

        let handle_a = RevisionHandle::new("rev-a").unwrap();
        let handle_b = RevisionHandle::new("rev-b").unwrap();
        revisions.insert(
            "rev-a".to_string(),
            StoredRevision {
                handle: handle_a.clone(),
                inventory: SourceInventory::new(&handle_a, vec![entry_main.clone(), entry_lib_v1])
                    .unwrap(),
                contents: contents_a,
                parents: vec![],
            },
        );
        revisions.insert(
            "rev-b".to_string(),
            StoredRevision {
                handle: handle_b.clone(),
                inventory: SourceInventory::new(&handle_b, vec![entry_main, entry_lib_v2]).unwrap(),
                contents: contents_b,
                parents: vec![handle_a],
            },
        );
        // An orphan revision with no recorded lineage at all.
        let handle_orphan = RevisionHandle::new("rev-orphan").unwrap();
        revisions.insert(
            "rev-orphan".to_string(),
            StoredRevision {
                inventory: SourceInventory::new(
                    &handle_orphan,
                    vec![InventoryEntry {
                        path: path("README.md"),
                        content: digest_of(b"# demo"),
                    }],
                )
                .unwrap(),
                handle: handle_orphan,
                contents: HashMap::new(),
                parents: vec![],
            },
        );
        Self {
            revisions,
            capabilities: ProviderCapabilities {
                rename_evidence: true,
                ancestor_queries: true,
                staged_index: true,
                dirty_tree: true,
            },
            staged: vec![],
            dirty: vec![],
        }
    }

    fn with_capabilities(mut self, capabilities: ProviderCapabilities) -> Self {
        self.capabilities = capabilities;
        self
    }
}

fn revision_of(provider: &FakeProvider, reference: &str) -> Option<SourceRevision> {
    provider.revisions.get(reference).map(|stored| {
        SourceRevision::new(
            source(),
            stored.handle.clone(),
            stored.inventory.content_digest(),
        )
    })
}

impl RevisionReader for FakeProvider {
    fn capabilities(&self) -> ProviderCapabilities {
        self.capabilities
    }

    fn resolve(&self, source: &SourceRef, reference: &str) -> Result<SourceRevision, ScmError> {
        if source.provider().as_str() != "fake-provider" {
            return Err(ScmError::ReferenceNotFound {
                provider: source.provider().as_str().to_string(),
                reference: reference.to_string(),
            });
        }
        revision_of(self, reference).ok_or_else(|| ScmError::ReferenceNotFound {
            provider: source.provider().as_str().to_string(),
            reference: reference.to_string(),
        })
    }

    fn inventory(&self, revision: &SourceRevision) -> Result<SourceInventory, ScmError> {
        self.revisions
            .get(revision.handle().as_str())
            .map(|stored| stored.inventory.clone())
            .ok_or_else(|| ScmError::ContentUnavailable {
                path: revision.handle().as_str().to_string(),
            })
    }

    fn content(&self, revision: &SourceRevision, path: &PathKey) -> Result<Vec<u8>, ScmError> {
        self.revisions
            .get(revision.handle().as_str())
            .and_then(|stored| stored.contents.get(path).cloned())
            .ok_or_else(|| ScmError::ContentUnavailable {
                path: path.as_str().to_string(),
            })
    }
}

impl ChangeReader for FakeProvider {
    fn changes_between(
        &self,
        base: &SourceRevision,
        head: &SourceRevision,
    ) -> Result<ChangeSet, ScmError> {
        if !self.capabilities.ancestor_queries {
            return Err(ScmError::CapabilityUnsupported {
                capability: "ancestor_queries",
            });
        }
        let base_stored = self.revisions.get(base.handle().as_str());
        let head_stored = self.revisions.get(head.handle().as_str());
        let ancestry_ok = match (base_stored, head_stored) {
            (Some(_), Some(head_s)) => head_s
                .parents
                .iter()
                .any(|p| p.as_str() == base.handle().as_str()),
            _ => false,
        };
        if !ancestry_ok {
            return Err(ScmError::MissingAncestor {
                base: base.handle().as_str().to_string(),
                head: head.handle().as_str().to_string(),
            });
        }
        let base_inv = self.inventory(base)?;
        let head_inv = self.inventory(head)?;
        Ok(ChangeSet::between(base, head, &base_inv, &head_inv))
    }

    fn is_ancestor(
        &self,
        maybe_ancestor: &SourceRevision,
        maybe_descendant: &SourceRevision,
    ) -> Result<bool, ScmError> {
        if !self.capabilities.ancestor_queries {
            return Err(ScmError::CapabilityUnsupported {
                capability: "ancestor_queries",
            });
        }
        Ok(self
            .revisions
            .get(maybe_descendant.handle().as_str())
            .map(|s| {
                s.parents
                    .iter()
                    .any(|p| p.as_str() == maybe_ancestor.handle().as_str())
            })
            .unwrap_or(false))
    }
}

impl WorkingCopyReader for FakeProvider {
    fn staged_changes(&self, _source: &SourceRef) -> Result<ChangeSet, ScmError> {
        if !self.capabilities.staged_index {
            return Err(ScmError::CapabilityUnsupported {
                capability: "staged_index",
            });
        }
        Ok(ChangeSet::explicit(
            RevisionHandle::new("working-copy").unwrap(),
            RevisionHandle::new("index").unwrap(),
            self.staged.clone(),
        ))
    }

    fn dirty_paths(&self, _source: &SourceRef) -> Result<Vec<PathKey>, ScmError> {
        if !self.capabilities.dirty_tree {
            return Err(ScmError::CapabilityUnsupported {
                capability: "dirty_tree",
            });
        }
        Ok(self.dirty.clone())
    }
}

// ----- AC #1: two snapshots, one fake provider, zero Git -------------

#[test]
fn two_snapshots_are_captured_from_a_fake_provider_without_any_git() {
    let provider = FakeProvider::new();
    let rev_a = provider.resolve(&source(), "rev-a").expect("rev-a exists");
    let rev_b = provider.resolve(&source(), "rev-b").expect("rev-b exists");
    let inv_a = provider.inventory(&rev_a).expect("inventory a");
    let inv_b = provider.inventory(&rev_b).expect("inventory b");

    assert_eq!(inv_a.entries().len(), 2);
    assert_eq!(inv_b.entries().len(), 2);
    assert_ne!(rev_a.handle(), rev_b.handle());
    assert_ne!(
        inv_a.content_digest(),
        inv_b.content_digest(),
        "different content must not share the reuse key"
    );
}

// ----- AC #2: content identity is the reuse key -----------------------

#[test]
fn identical_content_under_different_revisions_shares_the_content_digest() {
    let provider = FakeProvider::new();
    let rev_a = provider.resolve(&source(), "rev-a").unwrap();
    let inv_a = provider.inventory(&rev_a).unwrap();

    // A second revision object with the very same entries but a
    // different SCM handle: the content digest must match, because the
    // digest — not the handle — is what lets parsing be reused.
    let handle_x = RevisionHandle::new("rev-x").unwrap();
    let inv_x = SourceInventory::new(&handle_x, inv_a.entries().to_vec()).unwrap();
    assert_eq!(
        inv_a.content_digest(),
        inv_x.content_digest(),
        "content identity is independent of the SCM revision"
    );
    assert_ne!(
        inv_a.revision_handle(),
        inv_x.revision_handle(),
        "while the SCM identity stays distinct"
    );
}

#[test]
fn inventory_digest_is_independent_of_entry_insertion_order() {
    let handle = RevisionHandle::new("rev").unwrap();
    let e1 = InventoryEntry {
        path: path("a.rs"),
        content: digest_of(b"a"),
    };
    let e2 = InventoryEntry {
        path: path("b.rs"),
        content: digest_of(b"b"),
    };
    let forward = SourceInventory::new(&handle, vec![e1.clone(), e2.clone()]).unwrap();
    let backward = SourceInventory::new(&handle, vec![e2, e1]).unwrap();
    assert_eq!(forward.content_digest(), backward.content_digest());
}

// ----- AC #3: change kinds and optional capabilities -------------------

#[test]
fn changeset_classifies_added_deleted_and_modified_conservatively() {
    let provider = FakeProvider::new();
    let rev_a = provider.resolve(&source(), "rev-a").unwrap();
    let rev_b = provider.resolve(&source(), "rev-b").unwrap();
    let set = provider
        .changes_between(&rev_a, &rev_b)
        .expect("lineage known");

    let modified = set
        .changes()
        .iter()
        .find(|c| c.path() == &path("src/lib.rs"))
        .expect("lib.rs changed");
    assert!(matches!(modified.kind(), ChangeKind::Modified));

    // main.rs is byte-identical in both: it must not appear at all.
    assert!(
        !set.changes()
            .iter()
            .any(|c| c.path() == &path("src/main.rs")),
        "unchanged content never claims a change"
    );

    // Deleted and Added in a dedicated construction (orphan pair).
    let handle_old = RevisionHandle::new("old").unwrap();
    let handle_new = RevisionHandle::new("new").unwrap();
    let old_inv = SourceInventory::new(
        &handle_old,
        vec![
            InventoryEntry {
                path: path("gone.rs"),
                content: digest_of(b"x"),
            },
            InventoryEntry {
                path: path("moved.rs"),
                content: digest_of(b"y"),
            },
        ],
    )
    .unwrap();
    let new_inv = SourceInventory::new(
        &handle_new,
        vec![
            InventoryEntry {
                path: path("moved.rs"),
                content: digest_of(b"y"),
            },
            InventoryEntry {
                path: path("fresh.rs"),
                content: digest_of(b"z"),
            },
        ],
    )
    .unwrap();
    let rev_old = SourceRevision::new(source(), handle_old, old_inv.content_digest());
    let rev_new = SourceRevision::new(source(), handle_new, new_inv.content_digest());
    let set2 = ChangeSet::between(&rev_old, &rev_new, &old_inv, &new_inv);
    assert!(
        set2.changes()
            .iter()
            .any(|c| matches!(c.kind(), ChangeKind::Deleted) && *c.path() == path("gone.rs")),
    );
    assert!(
        set2.changes()
            .iter()
            .any(|c| matches!(c.kind(), ChangeKind::Added) && *c.path() == path("fresh.rs")),
    );
    assert_eq!(
        set2.changes().len(),
        2,
        "moved.rs is identical, not a change"
    );
}

#[test]
fn rename_pairing_folds_deleted_and_added_into_one_renamed_change() {
    let handle_old = RevisionHandle::new("old").unwrap();
    let handle_new = RevisionHandle::new("new").unwrap();
    let old_inv = SourceInventory::new(
        &handle_old,
        vec![InventoryEntry {
            path: path("old_name.rs"),
            content: digest_of(b"same"),
        }],
    )
    .unwrap();
    let new_inv = SourceInventory::new(
        &handle_new,
        vec![InventoryEntry {
            path: path("new_name.rs"),
            content: digest_of(b"same"),
        }],
    )
    .unwrap();
    let mut set = ChangeSet::between_raw(&old_inv, &new_inv);
    let folded = set.apply_rename_pairing(&[(path("old_name.rs"), path("new_name.rs"))]);
    assert_eq!(folded, 1);
    assert_eq!(set.changes().len(), 1);
    let change = &set.changes()[0];
    assert_eq!(
        change.kind(),
        &ChangeKind::Renamed {
            from: path("old_name.rs")
        }
    );
    assert_eq!(change.path(), &path("new_name.rs"));
}

#[test]
fn copy_pairing_marks_the_added_path_copied_from_an_unchanged_source() {
    let handle = RevisionHandle::new("old").unwrap();
    let old_inv = SourceInventory::new(
        &handle,
        vec![InventoryEntry {
            path: path("origin.rs"),
            content: digest_of(b"dup"),
        }],
    )
    .unwrap();
    let mut set = ChangeSet::explicit(
        RevisionHandle::new("old").unwrap(),
        RevisionHandle::new("new").unwrap(),
        vec![Change::added(path("clone.rs"), digest_of(b"dup"))],
    );
    let marked = set.apply_copy_pairing(&old_inv, &[(path("origin.rs"), path("clone.rs"))]);
    assert_eq!(marked, 1);
    assert_eq!(
        set.changes()[0].kind(),
        &ChangeKind::Copied {
            from: path("origin.rs")
        }
    );
    // The copy source is untouched: it never becomes a change.
    assert_eq!(set.changes().len(), 1);
}

#[test]
fn unknown_changes_demand_reanalysis_never_silence() {
    let unknown = Change::unknown(path("weird/thing"));
    assert_eq!(unknown.kind(), &ChangeKind::Unknown);
    assert!(
        unknown.requires_reparse(),
        "Unknown is conservative: it triggers reanalysis, it does not skip it"
    );
    assert!(!matches!(unknown.kind(), ChangeKind::Modified));
}

#[test]
fn providers_declare_capabilities_and_unsupported_reads_fail_loud() {
    let bare = FakeProvider::new().with_capabilities(ProviderCapabilities::NONE);
    let err = bare
        .staged_changes(&source())
        .expect_err("no staged capability, no staged answer");
    assert!(
        matches!(
            err,
            ScmError::CapabilityUnsupported {
                capability: "staged_index"
            }
        ),
        "got {err:?}"
    );
    let err = bare
        .dirty_paths(&source())
        .expect_err("no dirty capability, no dirty answer");
    assert!(matches!(
        err,
        ScmError::CapabilityUnsupported {
            capability: "dirty_tree"
        }
    ));
    assert_eq!(bare.capabilities(), ProviderCapabilities::NONE);
}

// ----- AC #4: fail-closed errors, named -------------------------------

#[test]
fn resolving_a_missing_reference_names_the_provider_and_the_reference() {
    let provider = FakeProvider::new();
    let err = provider
        .resolve(&source(), "no-such-ref")
        .expect_err("unknown ref must fail closed");
    match err {
        ScmError::ReferenceNotFound {
            provider,
            reference,
        } => {
            assert_eq!(provider, "fake-provider");
            assert_eq!(reference, "no-such-ref");
        }
        other => panic!("expected ReferenceNotFound, got {other:?}"),
    }
}

#[test]
fn changes_without_recorded_ancestry_are_refused_as_missing_ancestor() {
    let provider = FakeProvider::new();
    let rev_a = provider.resolve(&source(), "rev-a").unwrap();
    let rev_orphan = provider.resolve(&source(), "rev-orphan").unwrap();
    let err = provider
        .changes_between(&rev_a, &rev_orphan)
        .expect_err("no lineage, no diff");
    assert!(
        matches!(err, ScmError::MissingAncestor { .. }),
        "got {err:?}"
    );
}

#[test]
fn unsafe_paths_are_rejected_at_construction() {
    for bad in [
        "../escape.rs",
        "/absolute/path.rs",
        "src/../outside.rs",
        "",
        "back\\slash.rs",
        "./weird.rs",
    ] {
        let err = PathKey::new(bad).expect_err("{bad} must not become a path key");
        assert!(
            matches!(err, ScmError::UnsafePath { .. }),
            "for {bad}: got {err:?}"
        );
    }
    assert!(PathKey::new("crates/a/b.rs").is_ok());
    assert!(PathKey::new("spaced dir/file.rs").is_ok());
}

#[test]
fn unavailable_content_names_the_missing_path() {
    let provider = FakeProvider::new();
    let rev = provider.resolve(&source(), "rev-a").unwrap();
    let err = provider
        .content(&rev, &path("src/absent.rs"))
        .expect_err("content not stored is not content that is empty");
    match err {
        ScmError::ContentUnavailable { path } => assert_eq!(path, "src/absent.rs"),
        other => panic!("expected ContentUnavailable, got {other:?}"),
    }
}

#[test]
fn identity_constructors_refuse_blank_and_malformed_values() {
    assert!(matches!(
        SourceRef::new("", "project"),
        Err(ScmError::EmptyField { field: "provider" })
    ));
    assert!(matches!(
        SourceRef::new("provider", "  "),
        Err(ScmError::EmptyField { field: "project" })
    ));
    assert!(matches!(
        RevisionHandle::new(""),
        Err(ScmError::EmptyField { field: "handle" })
    ));
    assert!(matches!(
        ContentDigest::from_hex("nope"),
        Err(ScmError::DigestInvalid { .. })
    ));
    // A content digest is 64 lowercase hex chars and stable.
    let d = digest_of(b"abc");
    assert_eq!(d.as_hex().len(), 64);
    assert_eq!(d, ContentDigest::from_hex(d.as_hex()).unwrap());
}
