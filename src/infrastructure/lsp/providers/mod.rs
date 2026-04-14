pub mod composite;
pub mod fallback;
pub mod lsp;

pub use composite::CompositeProvider;
pub use fallback::TreesitterFallbackProvider;
pub use lsp::LspIntelligenceProvider;
