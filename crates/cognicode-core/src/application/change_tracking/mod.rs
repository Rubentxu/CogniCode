//! Change tracking — derived operational decisions over semantic diffs.
//!
//! e68 WU2 lives here: the affected-work planner. It composes the WU1
//! [`FactDelta`] with the e66 [`ReadSet`] introspection. The plan is a
//! derived operational decision, never a canonical Fact; nothing here
//! writes to the Evidence Kernel.

#[cfg(feature = "evidence-kernel")]
pub mod planner;
