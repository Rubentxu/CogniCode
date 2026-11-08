//! Request cancellation state shared by the core handler context.

use std::sync::atomic::AtomicBool;
use std::sync::Arc;

/// The cancellation flag attached to one in-flight request.
///
/// The MCP transport owns how request identifiers are tracked; the core only
/// needs this transport-neutral flag to let handlers observe cancellation.
pub type CancellationFlag = Arc<AtomicBool>;
