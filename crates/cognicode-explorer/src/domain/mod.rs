//! Explorer domain layer.
//!
//! Pure types and rules for inspecting known symbols, building contextual
//! views, and assembling evidence. No I/O lives here — adapters and ports
//! are wired in by the application service.

pub mod entry_point;
pub mod evidence;
pub mod lens;
pub mod lenses;
pub mod object_identity;
pub mod views;

pub use entry_point::{EntryPoint, EntryPointParseError, ResolvedEntryPoint};
pub use lens::{Lens, LensContext, LensRegistry, default_registry};
pub use object_identity::ObjectIdentity;
