//! What a change is made of, as the pack reads it.
//!
//! The SCM kernel decides which files changed and how; this vocabulary only
//! carries the answer of that kernel, together with the bytes of both sides,
//! to the pure core that derives the changed lines. Nothing here computes
//! what a change affects.

use crate::domain::findings::digest::sha256_hex;

/// Which change the pack is about. The scope is fixed by the request: a
/// scope never reads the changes of another one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChangeScope {
    /// What the index holds that `HEAD` does not.
    Staged,
    /// What the working tree holds that `HEAD` does not: staged, unstaged
    /// and untracked.
    WorkingTree,
    /// One commit against its first parent.
    Commit {
        /// The commit, by any reference.
        commit: String,
    },
    /// The changes of `head` since its merge base with `base` (the three-dot
    /// range of a pull request).
    Range {
        /// The branch the change is proposed against.
        base: String,
        /// The branch that carries the change.
        head: String,
    },
    /// The result of merging `theirs` into `ours`, conflicts included.
    MergeCandidate {
        /// The branch that receives the merge.
        ours: String,
        /// The branch that is merged.
        theirs: String,
    },
}

impl ChangeScope {
    /// Reads the scope of a request.
    ///
    /// `staged` and `working_tree` take no reference; `commit` takes `head`;
    /// `range` and `merge_candidate` take `base` and `head`. A reference that
    /// does not belong to the scope is refused instead of ignored.
    pub fn parse(scope: &str, base: Option<&str>, head: Option<&str>) -> Result<Self, String> {
        let named = |name: &str, value: Option<&str>| {
            value
                .map(str::trim)
                .filter(|text| !text.is_empty())
                .map(str::to_string)
                .ok_or_else(|| format!("the change scope `{scope}` needs `{name}`"))
        };
        let refuse_extra = |what: &str, value: Option<&str>| match value {
            Some(_) => Err(format!("the change scope `{scope}` takes no `{what}`")),
            None => Ok(()),
        };
        match scope {
            "staged" | "working_tree" => {
                refuse_extra("base", base)?;
                refuse_extra("head", head)?;
                Ok(if scope == "staged" {
                    Self::Staged
                } else {
                    Self::WorkingTree
                })
            }
            "commit" => {
                refuse_extra("base", base)?;
                Ok(Self::Commit {
                    commit: named("head", head)?,
                })
            }
            "range" => Ok(Self::Range {
                base: named("base", base)?,
                head: named("head", head)?,
            }),
            "merge_candidate" => Ok(Self::MergeCandidate {
                ours: named("base", base)?,
                theirs: named("head", head)?,
            }),
            other => Err(format!(
                "`{other}` is not a change scope: use staged, working_tree, commit, range or merge_candidate"
            )),
        }
    }

    /// The word the answer publishes.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Staged => "staged",
            Self::WorkingTree => "working_tree",
            Self::Commit { .. } => "commit",
            Self::Range { .. } => "range",
            Self::MergeCandidate { .. } => "merge_candidate",
        }
    }
}

/// The bytes of one side of one file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Content {
    /// The bytes, checked against the digest the change set recorded when it
    /// recorded one.
    Bytes(Vec<u8>),
    /// The file does not exist on this side.
    Absent,
    /// The file exists but its bytes cannot be used, and why.
    Unreadable(String),
}

/// How one file changed, in the words of the SCM kernel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileChange {
    /// Present on the head side only.
    Added,
    /// Present on the base side only.
    Deleted,
    /// Present on both sides with different bytes.
    Modified,
    /// Moved from `from`; the base side is the content of `from`.
    Renamed {
        /// The path before the move.
        from: String,
    },
    /// Copied from `from`; the base side is the content of `from`.
    Copied {
        /// The path of the source of the copy.
        from: String,
    },
    /// The kernel could not classify the change, and why (a conflict, for
    /// one). Its lines are not derived.
    Unknown(String),
}

/// One changed file with both of its sides.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceChange {
    /// The path of the head side, relative to the repository root.
    pub path: String,
    /// How it changed.
    pub change: FileChange,
    /// The bytes before the change.
    pub before: Content,
    /// The bytes after the change.
    pub after: Content,
}

/// The change a scope names, read once.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeInput {
    /// Where the change starts, in words: a revision handle or `index`.
    pub base: String,
    /// Where the change ends, in words: a revision handle, `index` or
    /// `working_tree`.
    pub head: String,
    /// The changed files, in path order, up to [`MAX_CHANGED_FILES`].
    pub files: Vec<SourceChange>,
    /// How many changed files are past the bound and were not read.
    pub files_not_read: usize,
}

impl Content {
    /// A stable word for the side: the digest of its bytes, `absent` or the
    /// reason it is unreadable.
    fn fingerprint(&self) -> String {
        match self {
            Self::Bytes(bytes) => sha256_hex(bytes),
            Self::Absent => "absent".to_string(),
            Self::Unreadable(why) => format!("unreadable:{why}"),
        }
    }
}

impl ChangeInput {
    /// The digest of the change: where it starts and ends, and, for every
    /// file, how it changed and the bytes of both sides.
    ///
    /// A continuation of a pack is bound to it, so a page is only ever built
    /// over the change it was issued for, even when the working tree moved
    /// in between.
    pub fn digest(&self) -> String {
        let mut text = format!("{}\n{}\n{}\n", self.base, self.head, self.files_not_read);
        for file in &self.files {
            text.push_str(&format!(
                "{}\0{:?}\0{}\0{}\n",
                file.path,
                file.change,
                file.before.fingerprint(),
                file.after.fingerprint()
            ));
        }
        sha256_hex(text.as_bytes())
    }
}

/// The most changed files a pack reads the bytes of.
pub const MAX_CHANGED_FILES: usize = 400;

/// The most bytes of one side of one file a pack reads.
pub const MAX_FILE_BYTES: u64 = 2 << 20;

/// Reads the change a scope names. The one implementation talks to the SCM
/// kernel; the pack core only sees its answer.
pub trait ChangeSource {
    /// The change of `scope`, or why it is not available. An unavailable
    /// change is never an empty one.
    fn read(&self, scope: &ChangeScope) -> Result<ChangeInput, String>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_scope_takes_the_references_it_needs_and_no_others() {
        assert_eq!(
            ChangeScope::parse("staged", None, None),
            Ok(ChangeScope::Staged)
        );
        assert_eq!(
            ChangeScope::parse("commit", None, Some("abc")),
            Ok(ChangeScope::Commit {
                commit: "abc".to_string()
            })
        );
        assert_eq!(
            ChangeScope::parse("range", Some("main"), Some("topic")),
            Ok(ChangeScope::Range {
                base: "main".to_string(),
                head: "topic".to_string()
            })
        );
        assert!(ChangeScope::parse("staged", Some("main"), None).is_err());
        assert!(ChangeScope::parse("range", Some("main"), None).is_err());
        assert!(ChangeScope::parse("range", Some("  "), Some("x")).is_err());
        assert!(ChangeScope::parse("everything", None, None).is_err());
    }

    fn input_with(after: &str) -> ChangeInput {
        ChangeInput {
            base: "abc".to_string(),
            head: "working_tree".to_string(),
            files: vec![SourceChange {
                path: "src/a.rs".to_string(),
                change: FileChange::Modified,
                before: Content::Bytes(b"one\n".to_vec()),
                after: Content::Bytes(after.as_bytes().to_vec()),
            }],
            files_not_read: 0,
        }
    }

    #[test]
    fn the_digest_follows_the_bytes_of_the_change() {
        assert_eq!(input_with("two\n").digest(), input_with("two\n").digest());
        assert_ne!(input_with("two\n").digest(), input_with("three\n").digest());
    }
}
