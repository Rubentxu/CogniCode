//! The DECLARED layer: what a manifest asks for, before any resolution.

use serde::{Deserialize, Serialize};

/// Which table of a manifest a dependency is declared in.
///
/// The kind is a property of a declaration, not of a package: the lockfile
/// does not record it, so it is known only for the edges a workspace member
/// declares itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DependencyKind {
    /// `[dependencies]`: part of the built artifact.
    Normal,
    /// `[dev-dependencies]`: tests, examples and benchmarks only.
    Development,
    /// `[build-dependencies]`: the build script only.
    Build,
}

impl DependencyKind {
    /// The manifest table name that declares this kind.
    pub fn table(self) -> &'static str {
        match self {
            Self::Normal => "dependencies",
            Self::Development => "dev-dependencies",
            Self::Build => "build-dependencies",
        }
    }
}

/// Where a declaration says the dependency lives.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DeclaredOrigin {
    /// A registry; `registry` is the alternative registry name, absent for
    /// the default one.
    Registry {
        /// The `registry = "..."` key, if present.
        registry: Option<String>,
    },
    /// A directory in the tree, as a normalised path relative to the
    /// repository root.
    Path {
        /// The directory.
        path: String,
    },
    /// A Git repository.
    Git {
        /// The `git = "..."` URL.
        url: String,
        /// `branch`, `tag` or `rev` as `branch:main`, `tag:v1` or `rev:abc`.
        reference: Option<String>,
    },
}

/// One dependency declaration of one manifest.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct DeclaredDependency {
    /// The key in the table: the name the dependent's code uses.
    pub key: String,
    /// The crate it resolves to: differs from `key` under `package = "..."`.
    pub package: String,
    /// The table it is declared in.
    pub kind: DependencyKind,
    /// The `cfg(...)` expression or target triple of a
    /// `[target.<...>.dependencies]` table.
    pub target: Option<String>,
    /// The version requirement, absent for a path or Git dependency that
    /// declares none.
    pub requirement: Option<String>,
    /// Where the declaration says the crate lives.
    pub origin: DeclaredOrigin,
    /// The features the declaration enables (workspace inheritance merged).
    pub features: Vec<String>,
    /// Whether the default features stay enabled.
    pub default_features: bool,
    /// Whether the dependency is optional.
    pub optional: bool,
}
