//! The provider identities are the port's own vocabulary.
//!
//! ## The hole
//!
//! The application attributes an observation to the provider that served it,
//! and every adapter declares the identity it answers under. Those were two
//! literals that had to be kept in step by hand: a provider that renamed
//! itself kept its old name in the provenance of every observation it
//! produced, and nothing failed.
//!
//! ## What this file measures
//!
//! The identities live in the port (`domain::traits::code_intelligence`),
//! and the adapters export them. This test crosses the layer boundary on
//! purpose — it is the only place where both sides can be seen at once —
//! and says that an adapter that declared a name of its own would fail
//! here, instead of quietly serving a second truth.

use cognicode_core::domain::traits::code_intelligence::provider_ids;
use cognicode_core::infrastructure::lsp::providers::fallback::TreesitterFallbackProvider;
use cognicode_core::infrastructure::lsp::providers::lsp::LspIntelligenceProvider;
use cognicode_core::infrastructure::lsp::providers::scip_provider::ScipIntelligenceProvider;

#[test]
fn every_adapter_exports_the_identity_the_port_declares() {
    assert_eq!(
        LspIntelligenceProvider::PROVIDER_ID,
        provider_ids::LSP,
        "the LSP provider answers under another name than the one the port declares"
    );
    assert_eq!(
        TreesitterFallbackProvider::PROVIDER_ID,
        provider_ids::TREE_SITTER,
        "the tree-sitter provider answers under another name than the one the port declares"
    );
    assert_eq!(
        TreesitterFallbackProvider::LOCAL_RESOLVER_PROVIDER_ID,
        provider_ids::LOCAL_RESOLVER,
        "the local resolver answers under another name than the one the port declares"
    );
    assert_eq!(
        ScipIntelligenceProvider::PROVIDER_ID,
        provider_ids::SCIP,
        "the SCIP provider answers under another name than the one the port declares"
    );
}

#[test]
fn every_identity_is_a_name_of_its_own() {
    // Two providers answering under the same name would make the
    // attribution of an observation ambiguous: the name would not say
    // which one served it.
    let identities = [
        provider_ids::LSP,
        provider_ids::TREE_SITTER,
        provider_ids::LOCAL_RESOLVER,
        provider_ids::SCIP,
    ];
    for (index, identity) in identities.iter().enumerate() {
        assert!(
            !identity.trim().is_empty(),
            "a provider identity that is not a name identifies nothing"
        );
        for other in identities.iter().skip(index + 1) {
            assert_ne!(
                identity, other,
                "two providers answer under the same name: the attribution of an \
                 observation would not say which one served it"
            );
        }
    }
}
