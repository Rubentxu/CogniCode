//! Where a `WorkspaceSession` is actually assembled.
//!
//! This is the composition root the architecture was asking for, and it lives
//! here because this is the layer that is allowed to name both
//! `application` and `infrastructure`. Before it existed, the assembly lived
//! inside `WorkspaceSession::with_path_policy`, which meant the session
//! imported `RustVerifier`, `TreeSitterSyntaxAnalysis` and `CompositeProvider`
//! to call three `::new()`s on them. The types were already behind traits one
//! level down — `FileOperationsService` holds `Arc<dyn CodeVerifier>` and
//! `Arc<dyn SyntaxAnalysis>` — so what the import actually bought was the
//! right to *choose* the implementation, and choosing is composition.
//!
//! That is the whole argument for the file: a `dyn` that the layer below it
//! instantiates has moved the type, not the coupling.
//!
//! It also gives the tests somewhere honest to go. The 75 behavioural tests in
//! `workspace_session.rs` deliberately build the real validator, the real
//! verifier and the real parser, because a permissive double would leave them
//! green while no longer exercising the rejections they exist to cover. Rather
//! than let each of them re-assemble that by hand — or let `application` grow
//! its own `::new()` calls back — they call the same function production calls.

use std::path::Path;
use std::sync::Arc;

use crate::application::workspace_session::WorkspaceCapabilities;
use crate::domain::traits::code_intelligence::CodeIntelligenceProvider;
use crate::domain::traits::code_verifier::CodeVerifier;
use crate::infrastructure::complexity::TreeSitterComplexity;
use crate::infrastructure::lsp::CompositeProvider;
use crate::infrastructure::parser::syntax_analysis::TreeSitterSyntaxAnalysis;
use crate::infrastructure::verification::RustVerifier;

/// The capabilities a deployment gets when nothing overrides them.
///
/// `path_policy` stays a parameter because it is the one part that is *not* a
/// default: the validator's allowed workspace comes from how the user invoked
/// us, and only the caller knows that.
pub fn default_capabilities(
    root: &Path,
    path_policy: Arc<dyn crate::application::ports::PathPolicy>,
) -> WorkspaceCapabilities {
    let intelligence: Arc<dyn CodeIntelligenceProvider> = Arc::new(CompositeProvider::new(root));
    let code_verifier: Arc<dyn CodeVerifier> = Arc::new(RustVerifier::new());
    let syntax: Arc<dyn crate::application::ports::SyntaxAnalysis> =
        Arc::new(TreeSitterSyntaxAnalysis::new());
    let complexity: Arc<dyn crate::application::ports::ComplexityAnalysis> =
        Arc::new(TreeSitterComplexity);

    WorkspaceCapabilities::new(path_policy, code_verifier, syntax, intelligence, complexity)
}
