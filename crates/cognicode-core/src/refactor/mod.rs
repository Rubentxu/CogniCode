//! B5 — Safe refactoring & authorized changes (v0.101.15).
//!
//! Sub-módulos:
//!   - [`plan`] (B5.W1): el plan de refactor como pieza portadora
//!     de la propuesta (id, workspace_identity, basis, edits,
//!     preconditions, risks + sha256 digest).
//!   - [`preview`] (B5.W2): representacion determinista de
//!     "que cambiara si aplico el plan". `FileDiff` (unified diff
//!     por archivo) + `RefactorPreview` agregada. `render_preview`
//!     es una funcion PURA: mismo plan + mismos bytes => mismo
//!     preview, NUNCA muta el workspace. El `workspace_state_digest`
//!     se computa via `apply::compute_workspace_state_digest` para
//!     que el apply (B5.W3) pueda recomputarlo y compararlo
//!     byte-a-byte.
//!   - [`apply`] (B5.W3): el camino transaccional con autoridad
//!     separada. Lee un plan y un `workspace_state_digest` (del
//!     preview), valida el token, aplica las `PlannedEdit`s sobre
//!     el workspace con staging + fsync + rename atómico. En
//!     cualquier fallo, borra el staging dir y reporta el error
//!     (AAT-B5-01..03).

pub mod apply;
pub mod plan;
pub mod preview;
// B5.W4 — post-apply revalidation, evidence not verdict (AAT-B5-04).
// Spine: composite queries return evidence, not verdicts; investigate
// each piece. See `revalidate::RevalidationVerdict::VerdictRefused`.
pub mod revalidate;