pub mod composite;
pub mod fallback;
pub mod lsp;
pub mod scip_provider;

pub use composite::CompositeProvider;
pub use fallback::TreesitterFallbackProvider;
pub use lsp::LspIntelligenceProvider;
pub use scip_provider::ScipIntelligenceProvider;
