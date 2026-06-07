//! Lens implementations.
//!
//! Three lenses ship in Phase 4:
//! - [`hotspots::HotspotsLens`] — risk-scored symbol / file / scope ranking.
//! - [`dependencies::DependenciesLens`] — coupling + circular-dependency hints.
//! - [`architecture::ArchitectureLens`] — boundary + cycle detection at scope level.
//!
//! Every lens produces [`crate::dto::DesignFinding`] objects with
//! `severity`, `confidence`, and `evidence_ids` — and frames its output
//! as a hypothesis, never a verdict.

pub mod architecture;
pub mod dependencies;
pub mod hotspots;
