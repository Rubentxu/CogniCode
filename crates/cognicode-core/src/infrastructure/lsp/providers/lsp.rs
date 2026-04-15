use crate::domain::aggregates::Symbol;
use crate::domain::traits::code_intelligence::{
    CodeIntelligenceError, CodeIntelligenceProvider, DocumentSymbol, DocumentSymbolKind,
    HoverInfo, HoverKind, Reference, ReferenceKind, TypeHierarchy,
};
use crate::domain::value_objects::Location;
use crate::infrastructure::lsp::process_manager::LspProcessManager;
use crate::infrastructure::parser::Language;
use serde_json::Value;
use std::path::Path;

pub struct LspIntelligenceProvider {
    process_manager: LspProcessManager,
}

impl LspIntelligenceProvider {
    pub fn new(workspace_root: &Path) -> Self {
        Self {
            process_manager: LspProcessManager::new(workspace_root),
        }
    }

    pub fn process_manager(&self) -> &LspProcessManager {
        &self.process_manager
    }

    async fn ensure_document_open(&self, file_path: &str) -> Result<(), CodeIntelligenceError> {
        let language = Self::language_from_file(file_path)?;
        let content = std::fs::read_to_string(file_path)
            .map_err(|e| CodeIntelligenceError::FileNotFound(e.to_string()))?;

        self.process_manager
            .open_document(language, file_path, &content)
            .await
            .map_err(|e| CodeIntelligenceError::LspError(e.to_string()))
    }

    fn language_from_file(file_path: &str) -> Result<Language, CodeIntelligenceError> {
        Language::from_extension(Path::new(file_path).extension())
            .ok_or_else(|| CodeIntelligenceError::LanguageNotSupported(file_path.to_string()))
    }

    async fn text_document_position_params(
        file_path: &str,
        line: u32,
        column: u32,
    ) -> Value {
        serde_json::json!({
            "textDocument": { "uri": format!("file://{}", file_path) },
            "position": {
                "line": line.saturating_sub(1),
                "character": column.saturating_sub(1)
            }
        })
    }

}

#[async_trait::async_trait]
impl CodeIntelligenceProvider for LspIntelligenceProvider {
    async fn get_symbols(&self, path: &Path) -> Result<Vec<Symbol>, CodeIntelligenceError> {
        let language = Self::language_from_file(&path.to_string_lossy())?;
        let file_uri = format!("file://{}", path.display());
        let params = serde_json::json!({ "textDocument": { "uri": file_uri } });

        let result = self
            .process_manager
            .request(language, "textDocument/documentSymbol", Some(params))
            .await
            .map_err(|e| CodeIntelligenceError::LspError(e.to_string()))?;

        if result.is_null() || !result.is_array() {
            return Ok(Vec::new());
        }

        let symbols: Vec<Symbol> = result
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|item| {
                let name = item.get("name")?.as_str()?.to_string();
                let kind_num = item.get("kind")?.as_u64()?;
                let range = item.get("range")?;
                let start = range.get("start")?;
                let line = start.get("line")?.as_u64()? as u32 + 1;
                let character = start.get("character")?.as_u64()? as u32 + 1;
                Some(Symbol::new(
                    name,
                    crate::domain::value_objects::SymbolKind::from_lsp_kind(kind_num),
                    Location::new(path.to_string_lossy().to_string(), line, character),
                ))
            })
            .collect();

        Ok(symbols)
    }

    async fn find_references(
        &self,
        location: &Location,
        include_declaration: bool,
    ) -> Result<Vec<Reference>, CodeIntelligenceError> {
        self.ensure_document_open(location.file()).await?;
        
        let language = Self::language_from_file(location.file())?;
        let mut params = Self::text_document_position_params(location.file(), location.line(), location.column()).await;
        params["context"] = serde_json::json!({ "includeDeclaration": include_declaration });

        let result = self
            .process_manager
            .request(language, "textDocument/references", Some(params))
            .await
            .map_err(|e| CodeIntelligenceError::LspError(e.to_string()))?;

        if result.is_null() || !result.is_array() {
            return Ok(Vec::new());
        }

        let refs: Vec<Reference> = result
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|item| {
                let uri = item.get("uri")?.as_str()?.to_string();
                let range = item.get("range")?;
                let start = range.get("start")?;
                let line = start.get("line")?.as_u64()? as u32 + 1;
                let character = start.get("character")?.as_u64()? as u32 + 1;
                Some(Reference {
                    location: Location::new(uri, line, character),
                    reference_kind: ReferenceKind::Read,
                    container: None,
                })
            })
            .collect();

        Ok(refs)
    }

    async fn get_hierarchy(
        &self,
        _location: &Location,
    ) -> Result<TypeHierarchy, CodeIntelligenceError> {
        Err(CodeIntelligenceError::Internal(
            "Type hierarchy via LSP not yet implemented".to_string(),
        ))
    }

    async fn get_definition(
        &self,
        location: &Location,
    ) -> Result<Option<Location>, CodeIntelligenceError> {
        self.ensure_document_open(location.file()).await?;
        
        let language = Self::language_from_file(location.file())?;
        let params = Self::text_document_position_params(location.file(), location.line(), location.column()).await;

        let result = self
            .process_manager
            .request(language, "textDocument/definition", Some(params))
            .await
            .map_err(|e| CodeIntelligenceError::LspError(e.to_string()))?;

        if result.is_null() {
            return Ok(None);
        }

        let items = if result.is_array() {
            result.as_array().unwrap().clone()
        } else {
            vec![result]
        };

        if let Some(first) = items.first() {
            let uri = first.get("uri").and_then(|v| v.as_str()).ok_or_else(|| {
                CodeIntelligenceError::Internal("Missing uri in definition response".to_string())
            })?.to_string();
            let range = first.get("range").ok_or_else(|| {
                CodeIntelligenceError::Internal("Missing range in definition response".to_string())
            })?;
            let start = range.get("start").ok_or_else(|| {
                CodeIntelligenceError::Internal("Missing start in range".to_string())
            })?;
            let line = start.get("line").and_then(|v| v.as_u64()).ok_or_else(|| {
                CodeIntelligenceError::Internal("Missing line in start".to_string())
            })? as u32 + 1;
            let character = start.get("character").and_then(|v| v.as_u64()).ok_or_else(|| {
                CodeIntelligenceError::Internal("Missing character in start".to_string())
            })? as u32 + 1;
            return Ok(Some(Location::new(uri, line, character)));
        }

        Ok(None)
    }

    async fn get_document_symbols(
        &self,
        path: &Path,
    ) -> Result<Vec<DocumentSymbol>, CodeIntelligenceError> {
        let symbols = self.get_symbols(path).await?;
        Ok(symbols
            .iter()
            .map(|s| DocumentSymbol {
                symbol: s.clone(),
                document_kind: DocumentSymbolKind::Function,
                range: crate::domain::value_objects::SourceRange::new(
                    s.location().clone(),
                    s.location().clone(),
                ),
                children: Vec::new(),
            })
            .collect())
    }

    async fn hover(
        &self,
        location: &Location,
    ) -> Result<Option<HoverInfo>, CodeIntelligenceError> {
        self.ensure_document_open(location.file()).await?;
        
        let language = Self::language_from_file(location.file())?;
        let params = Self::text_document_position_params(location.file(), location.line(), location.column()).await;

        let delays_ms = [1000, 2000, 3000];
        
        for (attempt, delay_ms) in delays_ms.iter().enumerate() {
            let result = self
                .process_manager
                .request(language, "textDocument/hover", Some(params.clone()))
                .await
                .map_err(|e| CodeIntelligenceError::LspError(e.to_string()))?;

            if !result.is_null() {
                let contents = &result["contents"];
                let content = match contents {
                    Value::String(s) => s.clone(),
                    Value::Object(obj) => obj
                        .get("value")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    Value::Array(arr) => arr
                        .iter()
                        .filter_map(|v| match v {
                            Value::String(s) => Some(s.as_str()),
                            Value::Object(obj) => obj.get("value").and_then(|v| v.as_str()),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join("\n\n"),
                    _ => {
                        if attempt < delays_ms.len() - 1 {
                            tokio::time::sleep(std::time::Duration::from_millis(*delay_ms)).await;
                            continue;
                        }
                        return Ok(None);
                    }
                };

                if !content.is_empty() {
                    return Ok(Some(HoverInfo {
                        content,
                        documentation: None,
                        kind: HoverKind::Mixed,
                    }));
                }
            }
            
            if attempt < delays_ms.len() - 1 {
                tokio::time::sleep(std::time::Duration::from_millis(*delay_ms)).await;
            }
        }
        
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use crate::infrastructure::lsp::providers::lsp::LspIntelligenceProvider;
    use crate::infrastructure::parser::Language;

    #[test]
    fn test_language_from_file_python() {
        let result = LspIntelligenceProvider::language_from_file("/path/to/file.py");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Language::Python);
    }

    #[test]
    fn test_language_from_file_rust() {
        let result = LspIntelligenceProvider::language_from_file("/path/to/file.rs");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Language::Rust);
    }

    #[test]
    fn test_language_from_file_javascript() {
        let result = LspIntelligenceProvider::language_from_file("/path/to/file.js");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Language::JavaScript);
    }

    #[test]
    fn test_language_from_file_typescript() {
        let result = LspIntelligenceProvider::language_from_file("/path/to/file.ts");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Language::TypeScript);
    }

    #[test]
    fn test_language_from_file_typescript_jsx() {
        let result = LspIntelligenceProvider::language_from_file("/path/to/file.tsx");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Language::TypeScript);
    }

    #[test]
    fn test_language_from_file_unsupported() {
        let result = LspIntelligenceProvider::language_from_file("/path/to/file.txt");
        assert!(result.is_err());
    }

    #[test]
    fn test_language_from_file_no_extension() {
        let result = LspIntelligenceProvider::language_from_file("/path/to/Makefile");
        assert!(result.is_err());
    }

    #[test]
    fn test_language_from_file_case_insensitive() {
        assert!(LspIntelligenceProvider::language_from_file("/path/to/file.PY").is_ok());
        assert!(LspIntelligenceProvider::language_from_file("/path/to/file.RS").is_ok());
    }
}
