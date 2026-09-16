//! PromotionPermit (e73 WU2 placeholder — full impl in WU2).
//!
//! This file ships in e73 WU1 as a forward declaration so that the
//! e73 WU1 module layout compiles. The full
//! `PromotionPermit`/`PromotionApply` API lands in e73 WU2.
//!
//! What this stub establishes:
//!
//! - The promotion authority path is reserved for a separate type
//!   (this file).
//! - The e73 WU1 evaluation does not depend on this file; it returns
//!   a `PromotionDryRun` that *describes* the situation but does not
//!   authorise any apply.

/// Reserved type marker for the e73 WU2 promotion permit API.
///
/// The actual permit/apply types will be defined in e73 WU2. This
/// placeholder exists so that:
///
/// - The `promotion_authority` module path is reserved.
/// - The umbrella comment in [`crate::application::promotion_authority`]
///   has a concrete type to point at.
/// - Future refactors that add the permit know exactly where to put
///   it.
#[doc(hidden)]
pub struct PromotionPermitReserved;
