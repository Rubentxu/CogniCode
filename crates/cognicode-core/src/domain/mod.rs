//! Domain layer for CogniCode
//!
//! This module contains pure Rust domain logic with no external dependencies
//! for parsing or runtime. It defines the core entities, value objects,
//! aggregates, services, and traits that represent the business logic.

pub mod aggregates;
pub mod error;
pub mod events;
pub mod services;
pub mod traits;
pub mod value_objects;

pub use aggregates::{CallGraph, Refactor, Symbol};
pub use error::DomainError;
pub use events::{GraphDiffCalculator, GraphEvent};
pub use services::{ComplexityCalculator, CycleDetector, ImpactAnalyzer};
pub use value_objects::{DependencyType, Location, SourceRange, SymbolKind};
