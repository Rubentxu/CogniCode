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
use crate::infrastructure::graph::GraphCache;
use crate::infrastructure::lsp::providers::composite::{CompositeProvider, TierPolicy};
use crate::infrastructure::parser::syntax_analysis::TreeSitterSyntaxAnalysis;
use crate::infrastructure::scip_loader::load_workspace_scip_index;
use crate::infrastructure::semantic::{SemanticSearchService, SymbolCodeService};
use crate::infrastructure::verification::RustVerifier;

/// The code-intelligence provider of a workspace.
///
/// A workspace that declares a SCIP index bound to it gets the S3 tier, the
/// one that answers from the index somebody produced. A workspace that
/// declares none keeps the S2/S1/S0 pipeline, and a workspace that declares
/// one the loader refuses keeps it too: a stale or foreign index is
/// reported and ignored, never answered from.
pub fn code_intelligence_provider(root: &Path) -> CompositeProvider {
    match load_workspace_scip_index(root) {
        Ok(Some(loaded)) => {
            CompositeProvider::with_scip_index(root, loaded.index, TierPolicy::all())
        }
        Ok(None) => CompositeProvider::new(root),
        Err(error) => {
            tracing::warn!(
                %error,
                "the declared SCIP index is not usable; continuing without the S3 tier"
            );
            CompositeProvider::new(root)
        }
    }
}

/// The capabilities a deployment gets when nothing overrides them.
///
/// `path_policy` stays a parameter because it is the one part that is *not* a
/// default: the validator's allowed workspace comes from how the user invoked
/// us, and only the caller knows that.
pub fn default_capabilities(
    root: &Path,
    path_policy: Arc<dyn crate::application::ports::PathPolicy>,
) -> WorkspaceCapabilities {
    let intelligence: Arc<dyn CodeIntelligenceProvider> =
        Arc::new(code_intelligence_provider(root));
    let code_verifier: Arc<dyn CodeVerifier> = Arc::new(RustVerifier::new());
    let syntax: Arc<dyn crate::application::ports::SyntaxAnalysis> =
        Arc::new(TreeSitterSyntaxAnalysis::new());
    let complexity: Arc<dyn crate::application::ports::ComplexityAnalysis> =
        Arc::new(TreeSitterComplexity);
    let graph: Arc<dyn crate::application::ports::SharedGraph> = Arc::new(GraphCache::new());
    let symbol_search: Arc<dyn crate::application::ports::SymbolSearch> =
        Arc::new(SemanticSearchService::new());
    let symbol_source: Arc<dyn crate::application::ports::SymbolSource> =
        Arc::new(SymbolCodeService::new());

    WorkspaceCapabilities {
        path_policy,
        code_verifier,
        syntax,
        intelligence,
        complexity,
        graph,
        symbol_search,
        symbol_source,
    }
}
