//! What changed between two revisions: a deterministic list of
//! [`Change`]s. Derived changes speak only about tracked content;
//! Rename and Copy fold in provider evidence that the derivation
//! alone cannot claim (a pure digest diff sees a rename as
//! delete-plus-add, and that is exactly what it reports until
//! evidence pairs it).

use super::identity::{ContentDigest, PathKey, RevisionHandle};
use super::inventory::SourceInventory;

/// How one path changed. `Unknown` is the honest fall-through: the
/// provider observed a change it cannot classify.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChangeKind {
    /// Present in `head` only.
    Added,
    /// Present in `base` only.
    Deleted,
    /// Present in both, different content digest.
    Modified,
    /// The path moved from `from`; content evidence paired it.
    Renamed { from: PathKey },
    /// The path was copied from `from`; the source stays untouched.
    Copied { from: PathKey },
    /// A change the provider cannot classify.
    Unknown,
}

/// One classified change of one path between `base` and `head`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    kind: ChangeKind,
    path: PathKey,
    before: Option<ContentDigest>,
    after: Option<ContentDigest>,
}

impl Change {
    pub fn added(path: PathKey, after: ContentDigest) -> Self {
        Self {
            kind: ChangeKind::Added,
            path,
            before: None,
            after: Some(after),
        }
    }

    pub fn deleted(path: PathKey, before: ContentDigest) -> Self {
        Self {
            kind: ChangeKind::Deleted,
            path,
            before: Some(before),
            after: None,
        }
    }

    pub fn modified(path: PathKey, before: ContentDigest, after: ContentDigest) -> Self {
        Self {
            kind: ChangeKind::Modified,
            path,
            before: Some(before),
            after: Some(after),
        }
    }

    /// A change nothing else can describe. Conservative by contract:
    /// see [`Change::requires_reparse`].
    pub fn unknown(path: PathKey) -> Self {
        Self {
            kind: ChangeKind::Unknown,
            path,
            before: None,
            after: None,
        }
    }

    /// Whether the consumer must re-analyze content for this change.
    ///
    /// `Renamed` and `Copied` are the only `false` kinds: their
    /// content digest already exists in the base, so the artifact can
    /// move instead of being re-parsed (CAA-02 AC #2). `Unknown` is
    /// deliberately `true` — an unclassifiable change triggers work,
    /// it never silences it.
    pub fn requires_reparse(&self) -> bool {
        !matches!(
            self.kind,
            ChangeKind::Renamed { .. } | ChangeKind::Copied { .. }
        )
    }

    pub fn kind(&self) -> &ChangeKind {
        &self.kind
    }

    pub fn path(&self) -> &PathKey {
        &self.path
    }

    pub fn before(&self) -> Option<&ContentDigest> {
        self.before.as_ref()
    }

    pub fn after(&self) -> Option<&ContentDigest> {
        self.after.as_ref()
    }
}

/// The changes from one revision handle to another, in deterministic
/// path order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeSet {
    base: RevisionHandle,
    head: RevisionHandle,
    changes: Vec<Change>,
}

impl ChangeSet {
    /// Derives Added/Deleted/Modified from two inventories. Equal
    /// digests produce NO change entry: unchanged content never
    /// claims work.
    pub fn between(
        base: &super::SourceRevision,
        head: &super::SourceRevision,
        base_inventory: &SourceInventory,
        head_inventory: &SourceInventory,
    ) -> Self {
        Self::between_raw(base_inventory, head_inventory)
            .with_handles(base.handle().clone(), head.handle().clone())
    }

    /// Same derivation without revision objects (the handles come
    /// from the inventories themselves).
    pub fn between_raw(base_inventory: &SourceInventory, head_inventory: &SourceInventory) -> Self {
        let mut changes = Vec::new();
        let mut base_iter = base_inventory.entries().iter().peekable();
        let mut head_iter = head_inventory.entries().iter().peekable();
        loop {
            match (base_iter.peek(), head_iter.peek()) {
                (None, None) => break,
                (Some(_), None) => {
                    for entry in base_iter.by_ref() {
                        changes.push(Change::deleted(entry.path.clone(), entry.content.clone()));
                    }
                }
                (None, Some(_)) => {
                    for entry in head_iter.by_ref() {
                        changes.push(Change::added(entry.path.clone(), entry.content.clone()));
                    }
                }
                (Some(b), Some(h)) => match b.path.cmp(&h.path) {
                    std::cmp::Ordering::Less => {
                        changes.push(Change::deleted(b.path.clone(), b.content.clone()));
                        base_iter.next();
                    }
                    std::cmp::Ordering::Greater => {
                        changes.push(Change::added(h.path.clone(), h.content.clone()));
                        head_iter.next();
                    }
                    std::cmp::Ordering::Equal => {
                        if b.content != h.content {
                            changes.push(Change::modified(
                                b.path.clone(),
                                b.content.clone(),
                                h.content.clone(),
                            ));
                        }
                        base_iter.next();
                        head_iter.next();
                    }
                },
            }
        }
        Self {
            base: base_inventory.revision_handle().clone(),
            head: head_inventory.revision_handle().clone(),
            changes,
        }
    }

    /// A change set the provider already classified (staged index,
    /// forge events) — entries are taken at face value.
    pub fn explicit(base: RevisionHandle, head: RevisionHandle, changes: Vec<Change>) -> Self {
        Self {
            base,
            head,
            changes,
        }
    }

    fn with_handles(mut self, base: RevisionHandle, head: RevisionHandle) -> Self {
        self.base = base;
        self.head = head;
        self
    }

    pub fn base(&self) -> &RevisionHandle {
        &self.base
    }

    pub fn head(&self) -> &RevisionHandle {
        &self.head
    }

    pub fn changes(&self) -> &[Change] {
        &self.changes
    }

    /// Folds each evidenced `(old, new)` rename pair into ONE
    /// `Renamed` change. Pairing requires both a `Deleted(old)` and
    /// an `Added(new)` entry; when either side is missing the pair
    /// is skipped and the raw changes stay — the set never invents a
    /// rename the evidence does not support. Returns how many pairs
    /// folded.
    pub fn apply_rename_pairing(&mut self, pairs: &[(PathKey, PathKey)]) -> usize {
        let mut folded = 0;
        for (old, new) in pairs {
            let deleted = self
                .changes
                .iter()
                .position(|c| matches!(c.kind, ChangeKind::Deleted) && &c.path == old);
            let added = self
                .changes
                .iter()
                .position(|c| matches!(c.kind, ChangeKind::Added) && &c.path == new);
            if let (Some(d), Some(a)) = (deleted, added) {
                let (deleted_change, added_change) = (
                    self.changes.remove(d),
                    self.changes.remove(if a > d { a - 1 } else { a }),
                );
                self.changes.insert(
                    d.min(self.changes.len()),
                    Change {
                        kind: ChangeKind::Renamed { from: old.clone() },
                        path: new.clone(),
                        before: deleted_change.before,
                        after: added_change.after,
                    },
                );
                folded += 1;
            }
        }
        folded
    }

    /// Marks each evidenced `(source, clone)` copy pair as `Copied`.
    /// The clone must be an `Added` path whose content already lives
    /// unchanged in the base inventory; otherwise the pair is
    /// skipped. The source path itself never turns into a change.
    /// Returns how many pairs marked.
    pub fn apply_copy_pairing(
        &mut self,
        base: &SourceInventory,
        pairs: &[(PathKey, PathKey)],
    ) -> usize {
        let mut marked = 0;
        for (from, clone) in pairs {
            let Some(entry) = base.find(from) else {
                continue;
            };
            let digest = entry.content.clone();
            let position = self.changes.iter().position(|c| {
                matches!(c.kind, ChangeKind::Added)
                    && &c.path == clone
                    && c.before.is_none()
                    && c.after.as_ref() == Some(&digest)
            });
            if let Some(index) = position {
                self.changes[index].kind = ChangeKind::Copied { from: from.clone() };
                marked += 1;
            }
        }
        marked
    }
}
