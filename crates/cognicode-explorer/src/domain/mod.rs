//! Explorer domain layer.
//!
//! Pure types and rules for inspecting known symbols, building contextual
//! views, and assembling evidence. No I/O lives here — adapters and ports
//! are wired in by the application service.

pub mod c4_mermaid;
pub mod decision_graph_topology;
pub mod decision_support_pack;
pub mod diagram_regen;
pub mod entry_point;
pub mod evidence;
pub mod knowledge;
pub mod lens;
pub mod lenses;
pub mod mermaid_util;
pub mod object_identity;
pub mod snapshot;
pub mod snapshot_dispatch;
pub mod trace_mermaid;
pub mod views;

pub use entry_point::{EntryPoint, EntryPointParseError, ResolvedEntryPoint};
pub use lens::{Lens, LensContext, LensRegistry, default_registry};
pub use object_identity::ObjectIdentity;
