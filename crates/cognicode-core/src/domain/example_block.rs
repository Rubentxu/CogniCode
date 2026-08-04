//! Example block — a code usage example linked to a symbol.
//!
//! This type lives in the core domain so it can be used by the
//! `GraphRepository` port without creating a core→explorer dependency.

use serde::{Deserialize, Serialize};

/// Kind of code example, used as the type discriminator in [`ExampleBlock`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExampleKind {
    /// A usage example showing how to call a symbol.
    Usage,
    /// A test example demonstrating how the symbol is tested.
    Test,
    /// A benchmark example showing performance characteristics.
    Benchmark,
}

/// A code usage example block, used by the ExampleObject view.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExampleBlock {
    /// The symbol this example demonstrates.
    pub symbol_id: String,
    /// The example code snippet.
    pub example_text: String,
    /// Source location of the example (file path and line).
    pub source_location: String,
    /// Kind of example (usage / test / benchmark).
    #[serde(rename = "block_type")]
    pub kind: ExampleKind,
}
