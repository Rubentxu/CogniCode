//! Re-export shim (cycle e64).
//!
//! [`CorrelationId`] identifies one logical operation across actors, which makes
//! it execution vocabulary rather than event-log vocabulary. The canonical
//! definition lives in [`crate::domain::execution::correlation`].

pub use crate::domain::execution::correlation::{CorrelationError, CorrelationId};
