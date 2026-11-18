//! Advisory matching: correlating a resolved inventory with a pinned OSV feed.
//!
//! Everything here is pure. It takes the bytes of a feed and a list of
//! resolved packages and returns observations; reading the feed from disk,
//! the clock and the inventory are the application layer's job.
//!
//! * [`parse_feed`] reads an untrusted feed defensively and accounts for
//!   every record it did not accept.
//! * [`contains`] decides whether a version is inside what an entry covers,
//!   with Cargo's version ordering.
//! * [`correlate()`] gives each package a [`PackageState`]: `affected`,
//!   `not_found_in_scope`, `unknown` or `not_evaluated`. A match is an
//!   observation. It is not exploitability and does not stand in for
//!   verifying an artifact's signature or provenance.

pub mod correlate;
pub mod feed;
pub mod model;
pub mod ranges;
pub mod record;

pub use correlate::correlate;
pub use feed::{FeedError, FeedLimits, ParsedFeed, RejectReason, RejectedRecord, parse_feed};
pub use model::{Absence, AdvisoryHit, Certainty, PackageResult, PackageState, Reason, Subject};
pub use ranges::{Containment, contains};
pub use record::{
    AdvisoryRange, AdvisoryRecord, AffectedEntry, AffectedPackage, CRATES_IO, RangeEvent,
    RangeKind, canonical_crate_name,
};
