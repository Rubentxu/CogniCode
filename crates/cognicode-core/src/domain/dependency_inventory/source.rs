//! Where a resolved package comes from, as the lockfile records it.

use serde::{Deserialize, Serialize};

/// The index URLs Cargo treats as the default registry.
const DEFAULT_REGISTRIES: [&str; 2] = [
    "registry+https://github.com/rust-lang/crates.io-index",
    "sparse+https://index.crates.io/",
];

/// The reference a Git dependency asked for. The lockfile pins the commit;
/// the reference is what a manifest edit would move.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GitReference {
    /// `branch = "..."`.
    Branch(String),
    /// `tag = "..."`.
    Tag(String),
    /// `rev = "..."`.
    Rev(String),
}

impl GitReference {
    /// A short label for reports: `branch:main`, `tag:v1`, `rev:abc`.
    pub fn label(&self) -> String {
        match self {
            Self::Branch(name) => format!("branch:{name}"),
            Self::Tag(name) => format!("tag:{name}"),
            Self::Rev(rev) => format!("rev:{rev}"),
        }
    }
}

/// The origin of a resolved package.
///
/// The lockfile has no `source` for a package that lives in the tree
/// (workspace members and path dependencies), which is [`Self::Path`].
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PackageSource {
    /// A package in the tree: no registry published it, no checksum covers it.
    Path,
    /// A registry index (`url` keeps the lockfile protocol prefix).
    Registry {
        /// The index URL, as the lockfile writes it.
        url: String,
    },
    /// A Git repository pinned to a commit.
    Git {
        /// The repository URL, without the `git+` prefix.
        url: String,
        /// The branch, tag or revision the manifest asked for, if any.
        reference: Option<GitReference>,
        /// The commit the lockfile pins.
        commit: String,
    },
    /// A source in a grammar this reader does not know; kept verbatim.
    Unknown {
        /// The `source` string as the lockfile writes it.
        raw: String,
    },
}

impl PackageSource {
    /// Reads a lockfile `source` string; the absence of one is [`Self::Path`].
    pub fn from_lock(raw: Option<&str>) -> Self {
        let Some(raw) = raw else {
            return Self::Path;
        };
        if raw.starts_with("registry+") || raw.starts_with("sparse+") {
            return Self::Registry {
                url: raw.to_string(),
            };
        }
        match raw.strip_prefix("git+").map(parse_git) {
            Some(Some(source)) => source,
            _ => Self::Unknown {
                raw: raw.to_string(),
            },
        }
    }

    /// Whether this is the default registry (crates.io).
    pub fn is_default_registry(&self) -> bool {
        matches!(self, Self::Registry { url } if DEFAULT_REGISTRIES.contains(&url.as_str()))
    }

    /// The lockfile string this source was read from; `None` for a path
    /// package, which the lockfile does not give one.
    pub fn lock_string(&self) -> Option<String> {
        match self {
            Self::Path => None,
            Self::Registry { url } => Some(url.clone()),
            Self::Unknown { raw } => Some(raw.clone()),
            Self::Git {
                url,
                reference,
                commit,
            } => {
                let query = match reference {
                    Some(GitReference::Branch(value)) => format!("?branch={value}"),
                    Some(GitReference::Tag(value)) => format!("?tag={value}"),
                    Some(GitReference::Rev(value)) => format!("?rev={value}"),
                    None => String::new(),
                };
                Some(format!("git+{url}{query}#{commit}"))
            }
        }
    }

    /// A short name for the kind of source.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Path => "path",
            Self::Registry { .. } => "registry",
            Self::Git { .. } => "git",
            Self::Unknown { .. } => "unknown",
        }
    }
}

/// `https://host/repo?rev=abc#<commit>` (the part after `git+`).
fn parse_git(rest: &str) -> Option<PackageSource> {
    let (location, commit) = rest.split_once('#')?;
    let (url, query) = match location.split_once('?') {
        Some((url, query)) => (url, Some(query)),
        None => (location, None),
    };
    let reference = query.and_then(|query| {
        let (key, value) = query.split_once('=')?;
        let value = value.to_string();
        match key {
            "branch" => Some(GitReference::Branch(value)),
            "tag" => Some(GitReference::Tag(value)),
            "rev" => Some(GitReference::Rev(value)),
            _ => None,
        }
    });
    Some(PackageSource::Git {
        url: url.to_string(),
        reference,
        commit: commit.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_package_without_a_source_is_a_path_package() {
        assert_eq!(PackageSource::from_lock(None), PackageSource::Path);
    }

    #[test]
    fn registry_sources_keep_their_index_url_and_know_the_default_one() {
        let crates_io = PackageSource::from_lock(Some(
            "registry+https://github.com/rust-lang/crates.io-index",
        ));
        assert!(crates_io.is_default_registry());
        let sparse = PackageSource::from_lock(Some("sparse+https://index.crates.io/"));
        assert!(sparse.is_default_registry());
        let other = PackageSource::from_lock(Some("sparse+https://registry.example.test/index/"));
        assert!(!other.is_default_registry());
        assert_eq!(other.kind(), "registry");
    }

    #[test]
    fn a_git_source_splits_url_reference_and_pinned_commit() {
        let raw = "git+https://github.com/example/crate?tag=v1.0.0#0123456789abcdef0123456789abcdef01234567";
        let source = PackageSource::from_lock(Some(raw));
        assert_eq!(
            source,
            PackageSource::Git {
                url: "https://github.com/example/crate".to_string(),
                reference: Some(GitReference::Tag("v1.0.0".to_string())),
                commit: "0123456789abcdef0123456789abcdef01234567".to_string(),
            }
        );
        assert_eq!(source.lock_string().as_deref(), Some(raw));
    }

    #[test]
    fn a_git_source_without_a_reference_is_the_default_branch() {
        let source = PackageSource::from_lock(Some("git+https://example.test/r#abc"));
        assert!(matches!(
            source,
            PackageSource::Git {
                reference: None,
                ..
            }
        ));
    }

    #[test]
    fn a_source_in_an_unknown_grammar_is_kept_verbatim() {
        let source = PackageSource::from_lock(Some("directory+/vendor"));
        assert_eq!(source.kind(), "unknown");
        assert_eq!(source.lock_string().as_deref(), Some("directory+/vendor"));
    }
}
