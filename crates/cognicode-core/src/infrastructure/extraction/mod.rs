//! Extraction adapters for the Generic Graph Layer.
//!
//! The default build is byte-for-byte unchanged: every submodule
//! is gated behind `#[cfg(feature = "multimodal")]`.

#[cfg(feature = "multimodal")]
pub mod docs_confidence_rules;
#[cfg(feature = "multimodal")]
pub mod docs_extractor;
