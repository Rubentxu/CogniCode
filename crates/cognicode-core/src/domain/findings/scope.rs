//! Re-export shim (cycle e64).
//!
//! The canonical definition moved to the **ungated** [`crate::domain::execution`]
//! once M7's event log needed the same scope: it was never a findings concept,
//! only a findings *first user*. Every `findings::scope::*` path keeps resolving.

pub use crate::domain::execution::scope::*;
