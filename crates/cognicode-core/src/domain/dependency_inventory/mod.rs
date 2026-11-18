//! Resolved dependency inventory of Cargo workspaces.
//!
//! Everything here is pure: it takes the bytes of manifests and lockfiles and
//! returns values. Reading them from a revision is the application layer's
//! job; nothing in this module runs Cargo, a build script or a network call.
//!
//! * [`plan_units`] groups the files of a repository into workspace units.
//! * [`inventory_unit`] reconciles the declared layer (manifests) with the
//!   resolved layer (lockfile) of one unit and reports every disagreement.
//! * [`diff_inventories`] computes the change between two inventories.
//! * [`dependents_of`] answers who reaches a package.
//! * [`cargo_purl`] gives a resolved package its Package URL identity.

pub mod cargo_lock;
pub mod cargo_manifest;
pub mod declared;
pub mod delta;
pub mod dependents;
pub mod model;
pub mod purl;
pub mod reconcile;
pub mod source;
pub mod units;
pub mod version_req;

pub use declared::{DeclaredDependency, DeclaredOrigin, DependencyKind};
pub use delta::{InventoryDelta, PackageUpdate, diff_inventories, diff_packages};
pub use dependents::{Dependent, DependentsReport, dependents_of};
pub use model::{
    CoverageStatus, DeclaredUse, DependencyLayer, Discrepancy, DiscrepancyKind, FileAnchor,
    Integrity, LayerStatus, LockFreshness, MemberSummary, MultipleVersions, ResolvedPackage,
    WorkspaceInventory,
};
pub use purl::cargo_purl;
pub use reconcile::inventory_unit;
pub use source::{GitReference, PackageSource};
pub use units::{SourceFile, UnitInput, plan_units};
