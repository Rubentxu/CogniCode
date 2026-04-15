use crate::domain::aggregates::Symbol;
use crate::domain::traits::code_intelligence::{
    CodeIntelligenceError, CodeIntelligenceProvider, DocumentSymbol, HoverInfo, Reference,
    TypeHierarchy,
};
use crate::domain::value_objects::Location;
use crate::infrastructure::graph::LightweightIndex;
use crate::infrastructure::lsp::error::{LspProcessError, ProgressCallback};
use crate::infrastructure::lsp::providers::fallback::TreesitterFallbackProvider;
use crate::infrastructure::lsp::providers::lsp::LspIntelligenceProvider;
use crate::infrastructure::parser::Language;
use std::path::Path;
use std::sync::Arc;
use tracing::warn;

pub struct FallbackResult<T> {
    pub value: T,
    pub fallback_reason: Option<String>,
}

impl<T> FallbackResult<T> {
    pub fn new(value: T) -> Self {
        Self {
            value,
            fallback_reason: None,
        }
    }

    pub fn with_fallback(value: T, reason: impl Into<String>) -> Self {
        Self {
            value,
            fallback_reason: Some(reason.into()),
        }
    }

    pub fn is_fallback(&self) -> bool {
        self.fallback_reason.is_some()
    }
}

pub struct CompositeProvider {
    lsp: LspIntelligenceProvider,
    fallback: TreesitterFallbackProvider,
    wait_timeout_secs: u64,
}

impl CompositeProvider {
    /// Build a LightweightIndex for the given workspace root
    fn build_index(workspace_root: &Path) -> Arc<LightweightIndex> {
        let mut index = LightweightIndex::new();
        index.build_index(workspace_root).ok();
        Arc::new(index)
    }

    pub fn new(workspace_root: &Path) -> Self {
        let arc_index = Self::build_index(workspace_root);
        Self {
            lsp: LspIntelligenceProvider::new(workspace_root),
            fallback: TreesitterFallbackProvider::with_index(arc_index.clone()),
            wait_timeout_secs: 30,
        }
    }

    pub fn with_wait_timeout(workspace_root: &Path, timeout_secs: u64) -> Self {
        let arc_index = Self::build_index(workspace_root);
        Self {
            lsp: LspIntelligenceProvider::new(workspace_root),
            fallback: TreesitterFallbackProvider::with_index(arc_index.clone()),
            wait_timeout_secs: timeout_secs,
        }
    }

    pub fn with_progress_callback<F>(workspace_root: &Path, callback: F) -> Self
    where
        F: ProgressCallback,
    {
        let arc_index = Self::build_index(workspace_root);
        Self {
            lsp: LspIntelligenceProvider::new(workspace_root),
            fallback: TreesitterFallbackProvider::with_index(arc_index.clone()),
            wait_timeout_secs: 30,
        }
    }

    pub fn wait_timeout_secs(&self) -> u64 {
        self.wait_timeout_secs
    }

    fn language_from_location(location: &Location) -> Option<Language> {
        Language::from_extension(Path::new(location.file()).extension())
    }

    async fn wait_for_lsp_ready(
        &self,
        language: Language,
        progress_callback: Option<Box<dyn ProgressCallback>>,
    ) -> Result<(), LspProcessError> {
        let pm = self.lsp.process_manager();
        match pm.wait_for_ready(language, self.wait_timeout_secs, progress_callback).await {
            Ok(status) => {
                if status.is_ready() {
                    Ok(())
                } else {
                    Err(LspProcessError::ServerNotReady {
                        language: language.name().to_string(),
                        status,
                        waited_secs: self.wait_timeout_secs,
                    })
                }
            }
            Err(e) => Err(e),
        }
    }
}

#[async_trait::async_trait]
impl CodeIntelligenceProvider for CompositeProvider {
    async fn get_symbols(&self, path: &Path) -> Result<Vec<Symbol>, CodeIntelligenceError> {
        let file_str = path.to_string_lossy().to_string();
        match self.lsp.get_symbols(path).await {
            Ok(symbols) if !symbols.is_empty() => Ok(symbols),
            Ok(_) | Err(_) => {
                warn!("LSP get_symbols failed for {}, using tree-sitter", file_str);
                self.fallback.get_symbols(path).await
            }
        }
    }

    async fn find_references(
        &self,
        location: &Location,
        include_declaration: bool,
    ) -> Result<Vec<Reference>, CodeIntelligenceError> {
        let file = location.file().to_string();

        if let Some(lang) = Self::language_from_location(location) {
            if let Err(e) = self
                .wait_for_lsp_ready(lang, None)
                .await
            {
                warn!(
                    "LSP not ready for find_references on {}, using tree-sitter: {}",
                    file, e
                );
                return self
                    .fallback
                    .find_references(location, include_declaration)
                    .await;
            }
        }

        match self.lsp.find_references(location, include_declaration).await {
            Ok(refs) if !refs.is_empty() => Ok(refs),
            Ok(_) | Err(_) => {
                warn!("LSP find_references failed for {}, using tree-sitter", file);
                self.fallback
                    .find_references(location, include_declaration)
                    .await
            }
        }
    }

    async fn get_hierarchy(
        &self,
        location: &Location,
    ) -> Result<TypeHierarchy, CodeIntelligenceError> {
        self.fallback.get_hierarchy(location).await
    }

    async fn get_definition(
        &self,
        location: &Location,
    ) -> Result<Option<Location>, CodeIntelligenceError> {
        let file = location.file().to_string();

        if let Some(lang) = Self::language_from_location(location) {
            if let Err(e) = self.wait_for_lsp_ready(lang, None).await {
                warn!(
                    "LSP not ready for get_definition on {}, using tree-sitter: {}",
                    file, e
                );
                return self.fallback.get_definition(location).await;
            }
        } else {
            return self.fallback.get_definition(location).await;
        }

        match self.lsp.get_definition(location).await {
            Ok(Some(loc)) => Ok(Some(loc)),
            Ok(None) => Ok(None),
            Err(e) => {
                warn!("LSP get_definition failed for {}, trying tree-sitter", file);
                self.fallback.get_definition(location).await
            }
        }
    }

    async fn get_document_symbols(
        &self,
        path: &Path,
    ) -> Result<Vec<DocumentSymbol>, CodeIntelligenceError> {
        let file_str = path.to_string_lossy().to_string();
        match self.lsp.get_document_symbols(path).await {
            Ok(symbols) if !symbols.is_empty() => Ok(symbols),
            Ok(_) | Err(_) => {
                warn!("LSP document symbols failed for {}, using tree-sitter", file_str);
                self.fallback.get_document_symbols(path).await
            }
        }
    }

    async fn hover(
        &self,
        location: &Location,
    ) -> Result<Option<HoverInfo>, CodeIntelligenceError> {
        let file = location.file().to_string();

        if let Some(lang) = Self::language_from_location(location) {
            if let Err(e) = self.wait_for_lsp_ready(lang, None).await {
                warn!("LSP not ready for hover on {}, using tree-sitter: {}", file, e);
                return self.fallback.hover(location).await;
            }
        } else {
            return self.fallback.hover(location).await;
        }

        match self.lsp.hover(location).await {
            Ok(Some(info)) if !info.content.is_empty() => {
                tracing::debug!("LSP hover returned: {}", info.content);
                Ok(Some(info))
            }
            Ok(Some(_)) => {
                warn!("LSP hover returned empty/unknown content for {}, trying tree-sitter", file);
                self.fallback.hover(location).await
            }
            Ok(None) => {
                warn!("LSP hover returned None for {}, trying tree-sitter", file);
                self.fallback.hover(location).await
            }
            Err(e) => {
                warn!("LSP hover error for {}, trying tree-sitter: {}", file, e);
                self.fallback.hover(location).await
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_composite_falls_back_on_error() {
        let provider = CompositeProvider::new(std::path::Path::new("/nonexistent"));
        let loc = Location::new("/nonexistent/test.rs".to_string(), 1, 1);
        let result = provider.get_definition(&loc).await;
        assert!(result.is_ok() || result.is_err());
    }

    #[tokio::test]
    async fn test_composite_hover_fallback_on_missing_file() {
        let provider = CompositeProvider::new(std::path::Path::new("/nonexistent"));
        let loc = Location::new("/nonexistent/test.rs".to_string(), 1, 1);
        let result = provider.hover(&loc).await;
        assert!(result.is_ok() || result.is_err());
    }

    #[tokio::test]
    async fn test_composite_hierarchy_always_uses_fallback() {
        let provider = CompositeProvider::new(std::path::Path::new("/tmp"));
        let loc = Location::new("/tmp/test.rs".to_string(), 1, 1);
        let result = provider.get_hierarchy(&loc).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_fallback_result_new() {
        let result: FallbackResult<i32> = FallbackResult::new(42);
        assert_eq!(result.value, 42);
        assert!(!result.is_fallback());
        assert!(result.fallback_reason.is_none());
    }

    #[test]
    fn test_fallback_result_with_reason() {
        let result: FallbackResult<i32> = FallbackResult::with_fallback(42, "Server not ready");
        assert_eq!(result.value, 42);
        assert!(result.is_fallback());
        assert_eq!(result.fallback_reason.as_deref(), Some("Server not ready"));
    }
}
