//! Re-export shim (cycle e63).
//!
//! The canonical namespaced-name grammar moved to the **ungated**
//! [`crate::domain::naming`] because the M7 Intelligence Event Log needs the
//! same validation for `EventKind`. This module is kept so every existing
//! `findings::namespaced::*` path continues to resolve (single source of
//! truth, no duplicated definitions).

pub use crate::domain::naming::*;
