use crate::domain::aggregates::Symbol;
use crate::domain::traits::code_intelligence::{
    CallHierarchyCall, CallHierarchyItem, CodeIntelligenceError, CodeIntelligenceProvider,
    DocumentSymbol, DocumentSymbolKind, HoverInfo, HoverKind, PrepareRename, Reference,
    ReferenceKind, RenameEdits, TextEdit, TypeHierarchy, TypeHierarchyNode,
};
use crate::domain::value_objects::{Location, SourceRange};
use crate::infrastructure::lsp::process_manager::LspProcessManager;
use crate::infrastructure::parser::Language;
use serde_json::Value;
use std::path::Path;

/// Language-server code intelligence.
///
/// Serves the **S2** tier of the composite pipeline (LSI M4). Attempts are
/// readiness-gated by the composite; this provider itself keeps its original
/// six-op contract.
pub struct LspIntelligenceProvider {
    process_manager: LspProcessManager,
}

impl LspIntelligenceProvider {
    /// Stable provider identity used in tier diagnostics and provenance detail.
    pub const PROVIDER_ID: &'static str = "lsp";

    pub fn new(workspace_root: &Path) -> Self {
        Self {
            process_manager: LspProcessManager::new(workspace_root),
        }
    }

    pub fn process_manager(&self) -> &LspProcessManager {
        &self.process_manager
    }

    /// `prepare{Call,Type}Hierarchy`: the raw items (needed verbatim by the
    /// follow-up requests) paired with their parsed form.
    async fn prepare_hierarchy(
        &self,
        location: &Location,
        method: &str,
    ) -> Result<Vec<(Value, CallHierarchyItem)>, CodeIntelligenceError> {
        self.ensure_document_open(location.file()).await?;
        let language = Self::language_from_file(location.file())?;
        let params = Self::text_document_position_params(
            location.file(),
            location.line(),
            location.column(),
        )
        .await;
        let result = self
            .process_manager
            .request(language, method, Some(params))
            .await
            .map_err(|e| CodeIntelligenceError::LspError(e.to_string()))?;
        let parsed = parse_hierarchy_items(location.file(), &result)?;
        let raws = result.as_array().cloned().unwrap_or_default();
        Ok(raws.into_iter().zip(parsed).collect())
    }

    async fn calls(
        &self,
        location: &Location,
        method: &str,
        key: &str,
    ) -> Result<Vec<CallHierarchyCall>, CodeIntelligenceError> {
        let prepared = self
            .prepare_hierarchy(location, "textDocument/prepareCallHierarchy")
            .await?;
        let Some((raw, _)) = prepared.first() else {
            return Ok(Vec::new());
        };
        let language = Self::language_from_file(location.file())?;
        let result = self
            .process_manager
            .request(language, method, Some(serde_json::json!({ "item": raw })))
            .await
            .map_err(|e| CodeIntelligenceError::LspError(e.to_string()))?;
        parse_calls(&result, key)
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

    async fn text_document_position_params(file_path: &str, line: u32, column: u32) -> Value {
        serde_json::json!({
            "textDocument": { "uri": format!("file://{}", file_path) },
            "position": {
                "line": line.saturating_sub(1),
                "character": column.saturating_sub(1)
            }
        })
    }
}

/// `textDocument/rename` parameters: the position plus the new name.
fn rename_params(file_path: &str, line: u32, column: u32, new_name: &str) -> Value {
    serde_json::json!({
        "textDocument": { "uri": format!("file://{}", file_path) },
        "position": {
            "line": line.saturating_sub(1),
            "character": column.saturating_sub(1)
        },
        "newName": new_name
    })
}

fn bad_shape(what: &str) -> CodeIntelligenceError {
    CodeIntelligenceError::Internal(format!("Malformed {what} in LSP response"))
}

fn location_of(file: &str, position: &Value) -> Option<Location> {
    let line = position.get("line")?.as_u64()? as u32 + 1;
    let character = position.get("character")?.as_u64()? as u32 + 1;
    Some(Location::new(file.to_string(), line, character))
}

fn range_of(file: &str, range: &Value) -> Option<SourceRange> {
    Some(SourceRange::new(
        location_of(file, range.get("start")?)?,
        location_of(file, range.get("end")?)?,
    ))
}

fn path_of_uri(uri: &str) -> &str {
    uri.strip_prefix("file://").unwrap_or(uri)
}

/// Reads a `prepareRename` answer. `Ok(None)` = the server says "not renamable".
fn parse_prepare_rename(
    file_path: &str,
    result: &Value,
) -> Result<Option<PrepareRename>, CodeIntelligenceError> {
    if result.is_null() {
        return Ok(None);
    }
    if result.get("start").is_some() {
        let range = range_of(file_path, result).ok_or_else(|| bad_shape("range"))?;
        return Ok(Some(PrepareRename {
            range: Some(range),
            placeholder: None,
        }));
    }
    if let Some(range) = result.get("range") {
        let range = range_of(file_path, range).ok_or_else(|| bad_shape("range"))?;
        let placeholder = result
            .get("placeholder")
            .and_then(Value::as_str)
            .map(str::to_string);
        return Ok(Some(PrepareRename {
            range: Some(range),
            placeholder,
        }));
    }
    if result.get("defaultBehavior").and_then(Value::as_bool) == Some(true) {
        return Ok(Some(PrepareRename {
            range: None,
            placeholder: None,
        }));
    }
    Err(bad_shape("prepareRename result"))
}

fn text_edits(
    file: &str,
    edits: &Value,
    out: &mut Vec<TextEdit>,
) -> Result<(), CodeIntelligenceError> {
    for edit in edits.as_array().ok_or_else(|| bad_shape("edit list"))? {
        let range = edit
            .get("range")
            .and_then(|r| range_of(file, r))
            .ok_or_else(|| bad_shape("edit range"))?;
        let new_text = edit
            .get("newText")
            .and_then(Value::as_str)
            .ok_or_else(|| bad_shape("edit newText"))?
            .to_string();
        out.push(TextEdit { range, new_text });
    }
    Ok(())
}

/// Reads a `WorkspaceEdit` (`changes` and `documentChanges`).
///
/// File create/rename/delete operations are not edits: they are counted in
/// `skipped_resource_operations` so the caller can tell the result is partial.
fn parse_workspace_edit(result: &Value) -> Result<Option<RenameEdits>, CodeIntelligenceError> {
    if result.is_null() {
        return Ok(None);
    }
    let mut out = RenameEdits::default();
    if let Some(changes) = result.get("changes").and_then(Value::as_object) {
        for (uri, edits) in changes {
            text_edits(path_of_uri(uri), edits, &mut out.edits)?;
        }
    }
    if let Some(changes) = result.get("documentChanges").and_then(Value::as_array) {
        for change in changes {
            if change.get("kind").is_some() {
                out.skipped_resource_operations += 1;
                continue;
            }
            let uri = change
                .pointer("/textDocument/uri")
                .and_then(Value::as_str)
                .ok_or_else(|| bad_shape("documentChanges uri"))?;
            let edits = change
                .get("edits")
                .ok_or_else(|| bad_shape("documentChanges edits"))?;
            text_edits(path_of_uri(uri), edits, &mut out.edits)?;
        }
    }
    Ok(Some(out))
}

/// One hierarchy item, positioned on its name (`selectionRange`).
fn parse_item(item: &Value) -> Result<CallHierarchyItem, CodeIntelligenceError> {
    let name = item
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| bad_shape("hierarchy item"))?;
    let uri = item
        .get("uri")
        .and_then(Value::as_str)
        .ok_or_else(|| bad_shape("hierarchy item"))?;
    let kind = item
        .get("kind")
        .and_then(Value::as_u64)
        .ok_or_else(|| bad_shape("hierarchy item"))?;
    let at = item
        .get("selectionRange")
        .or_else(|| item.get("range"))
        .and_then(|r| r.get("start"))
        .and_then(|p| location_of(path_of_uri(uri), p))
        .ok_or_else(|| bad_shape("hierarchy item"))?;
    Ok(CallHierarchyItem {
        name: name.to_string(),
        kind: crate::domain::value_objects::SymbolKind::from_lsp_kind(kind),
        location: at,
        detail: item
            .get("detail")
            .and_then(Value::as_str)
            .map(str::to_string),
    })
}

/// Reads the items of a `prepare*Hierarchy` answer. `null` = nothing there.
fn parse_hierarchy_items(
    _file_path: &str,
    result: &Value,
) -> Result<Vec<CallHierarchyItem>, CodeIntelligenceError> {
    if result.is_null() {
        return Ok(Vec::new());
    }
    result
        .as_array()
        .ok_or_else(|| bad_shape("hierarchy items"))?
        .iter()
        .map(parse_item)
        .collect()
}

/// Reads `incomingCalls` (`key` = "from") or `outgoingCalls` (`key` = "to").
fn parse_calls(result: &Value, key: &str) -> Result<Vec<CallHierarchyCall>, CodeIntelligenceError> {
    if result.is_null() {
        return Ok(Vec::new());
    }
    result
        .as_array()
        .ok_or_else(|| bad_shape("calls"))?
        .iter()
        .map(|call| {
            let item = parse_item(call.get(key).ok_or_else(|| bad_shape("call"))?)?;
            let file = item.location.file().to_string();
            let call_sites = call
                .get("fromRanges")
                .and_then(Value::as_array)
                .map(|ranges| {
                    ranges
                        .iter()
                        .map(|r| range_of(&file, r).ok_or_else(|| bad_shape("call range")))
                        .collect::<Result<Vec<_>, _>>()
                })
                .transpose()?
                .unwrap_or_default();
            Ok(CallHierarchyCall { item, call_sites })
        })
        .collect()
}

/// Reads a `supertypes`/`subtypes` answer as direct (distance 0) nodes.
fn parse_type_nodes(result: &Value) -> Result<Vec<TypeHierarchyNode>, CodeIntelligenceError> {
    Ok(parse_hierarchy_items("", result)?
        .into_iter()
        .map(|i| TypeHierarchyNode {
            symbol: Symbol::new(i.name, i.kind, i.location),
            distance: 0,
        })
        .collect())
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
        let mut params = Self::text_document_position_params(
            location.file(),
            location.line(),
            location.column(),
        )
        .await;
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
        location: &Location,
    ) -> Result<TypeHierarchy, CodeIntelligenceError> {
        let raw = self
            .prepare_hierarchy(location, "textDocument/prepareTypeHierarchy")
            .await?;
        let root = raw.first().ok_or_else(|| {
            CodeIntelligenceError::InvalidLocation(format!(
                "no type at {}:{}:{}",
                location.file(),
                location.line(),
                location.column()
            ))
        })?;
        let language = Self::language_from_file(location.file())?;
        let params = serde_json::json!({ "item": root.0 });
        let supers = self
            .process_manager
            .request(language, "typeHierarchy/supertypes", Some(params.clone()))
            .await
            .map_err(|e| CodeIntelligenceError::LspError(e.to_string()))?;
        let subs = self
            .process_manager
            .request(language, "typeHierarchy/subtypes", Some(params))
            .await
            .map_err(|e| CodeIntelligenceError::LspError(e.to_string()))?;
        Ok(TypeHierarchy {
            symbol: Symbol::new(root.1.name.clone(), root.1.kind, root.1.location.clone()),
            parents: parse_type_nodes(&supers)?,
            children: parse_type_nodes(&subs)?,
        })
    }

    async fn get_definition(
        &self,
        location: &Location,
    ) -> Result<Option<Location>, CodeIntelligenceError> {
        self.ensure_document_open(location.file()).await?;

        let language = Self::language_from_file(location.file())?;
        let params = Self::text_document_position_params(
            location.file(),
            location.line(),
            location.column(),
        )
        .await;

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
            let uri = first
                .get("uri")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    CodeIntelligenceError::Internal(
                        "Missing uri in definition response".to_string(),
                    )
                })?
                .to_string();
            let range = first.get("range").ok_or_else(|| {
                CodeIntelligenceError::Internal("Missing range in definition response".to_string())
            })?;
            let start = range.get("start").ok_or_else(|| {
                CodeIntelligenceError::Internal("Missing start in range".to_string())
            })?;
            let line = start.get("line").and_then(|v| v.as_u64()).ok_or_else(|| {
                CodeIntelligenceError::Internal("Missing line in start".to_string())
            })? as u32
                + 1;
            let character = start
                .get("character")
                .and_then(|v| v.as_u64())
                .ok_or_else(|| {
                    CodeIntelligenceError::Internal("Missing character in start".to_string())
                })? as u32
                + 1;
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

    async fn hover(&self, location: &Location) -> Result<Option<HoverInfo>, CodeIntelligenceError> {
        self.ensure_document_open(location.file()).await?;

        let language = Self::language_from_file(location.file())?;
        let params = Self::text_document_position_params(
            location.file(),
            location.line(),
            location.column(),
        )
        .await;

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

    async fn prepare_rename(
        &self,
        location: &Location,
    ) -> Result<Option<PrepareRename>, CodeIntelligenceError> {
        self.ensure_document_open(location.file()).await?;
        let language = Self::language_from_file(location.file())?;
        let params = Self::text_document_position_params(
            location.file(),
            location.line(),
            location.column(),
        )
        .await;
        let result = self
            .process_manager
            .request(language, "textDocument/prepareRename", Some(params))
            .await
            .map_err(|e| CodeIntelligenceError::LspError(e.to_string()))?;
        parse_prepare_rename(location.file(), &result)
    }

    async fn rename(
        &self,
        location: &Location,
        new_name: &str,
    ) -> Result<Option<RenameEdits>, CodeIntelligenceError> {
        if new_name.trim().is_empty() {
            return Err(CodeIntelligenceError::ParseError(
                "the new name is empty".to_string(),
            ));
        }
        self.ensure_document_open(location.file()).await?;
        let language = Self::language_from_file(location.file())?;
        let params = rename_params(
            location.file(),
            location.line(),
            location.column(),
            new_name,
        );
        let result = self
            .process_manager
            .request(language, "textDocument/rename", Some(params))
            .await
            .map_err(|e| CodeIntelligenceError::LspError(e.to_string()))?;
        parse_workspace_edit(&result)
    }

    async fn incoming_calls(
        &self,
        location: &Location,
    ) -> Result<Vec<CallHierarchyCall>, CodeIntelligenceError> {
        self.calls(location, "callHierarchy/incomingCalls", "from")
            .await
    }

    async fn outgoing_calls(
        &self,
        location: &Location,
    ) -> Result<Vec<CallHierarchyCall>, CodeIntelligenceError> {
        self.calls(location, "callHierarchy/outgoingCalls", "to")
            .await
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

#[cfg(test)]
mod rename_tests {
    use super::*;
    use serde_json::json;

    fn range(sl: u32, sc: u32, el: u32, ec: u32) -> Value {
        json!({"start": {"line": sl, "character": sc}, "end": {"line": el, "character": ec}})
    }

    #[test]
    fn rename_params_are_one_based_in_and_zero_based_out_with_the_new_name() {
        let p = rename_params("/w/a.cs", 3, 5, "Renamed");
        assert_eq!(p["textDocument"]["uri"], "file:///w/a.cs");
        assert_eq!(p["position"], json!({"line": 2, "character": 4}));
        assert_eq!(p["newName"], "Renamed");
    }

    #[test]
    fn prepare_rename_null_means_not_renamable() {
        assert_eq!(parse_prepare_rename("/w/a.cs", &Value::Null).unwrap(), None);
    }

    #[test]
    fn prepare_rename_reads_a_bare_range() {
        let got = parse_prepare_rename("/w/a.cs", &range(1, 4, 1, 9))
            .unwrap()
            .unwrap();
        let r = got.range.unwrap();
        assert_eq!((r.start().line(), r.start().column()), (2, 5));
        assert_eq!((r.end().line(), r.end().column()), (2, 10));
        assert_eq!(r.start().file(), "/w/a.cs");
        assert_eq!(got.placeholder, None);
    }

    #[test]
    fn prepare_rename_reads_range_with_placeholder() {
        let v = json!({"range": range(0, 0, 0, 3), "placeholder": "foo"});
        let got = parse_prepare_rename("/w/a.cs", &v).unwrap().unwrap();
        assert_eq!(got.placeholder.as_deref(), Some("foo"));
        assert_eq!(got.range.unwrap().end().column(), 4);
    }

    #[test]
    fn prepare_rename_default_behavior_has_no_range() {
        let got = parse_prepare_rename("/w/a.cs", &json!({"defaultBehavior": true}))
            .unwrap()
            .unwrap();
        assert_eq!(
            got,
            PrepareRename {
                range: None,
                placeholder: None
            }
        );
    }

    #[test]
    fn prepare_rename_rejects_an_unrecognised_shape() {
        assert!(parse_prepare_rename("/w/a.cs", &json!({"nope": 1})).is_err());
    }

    #[test]
    fn workspace_edit_changes_map_become_edits_with_paths() {
        let v = json!({"changes": {"file:///w/a.cs": [
            {"range": range(0, 1, 0, 4), "newText": "X"},
            {"range": range(4, 0, 4, 3), "newText": "X"}
        ]}});
        let got = parse_workspace_edit(&v).unwrap().unwrap();
        assert_eq!(got.skipped_resource_operations, 0);
        assert_eq!(got.edits.len(), 2);
        let e: &TextEdit = &got.edits[0];
        assert_eq!(e.range.start().file(), "/w/a.cs");
        assert_eq!((e.range.start().line(), e.range.start().column()), (1, 2));
        assert_eq!(e.new_text, "X");
    }

    #[test]
    fn workspace_edit_document_changes_are_read_and_resource_ops_are_counted() {
        let v = json!({"documentChanges": [
            {"textDocument": {"uri": "file:///w/b.kt", "version": 1},
             "edits": [{"range": range(0, 0, 0, 1), "newText": "Y"}]},
            {"kind": "rename", "oldUri": "file:///w/b.kt", "newUri": "file:///w/c.kt"},
            {"kind": "create", "uri": "file:///w/d.kt"}
        ]});
        let got = parse_workspace_edit(&v).unwrap().unwrap();
        assert_eq!(got.edits.len(), 1);
        assert_eq!(got.edits[0].range.start().file(), "/w/b.kt");
        assert_eq!(got.skipped_resource_operations, 2);
    }

    #[test]
    fn workspace_edit_null_means_the_server_declined() {
        assert_eq!(parse_workspace_edit(&Value::Null).unwrap(), None);
    }

    #[test]
    fn an_edit_without_a_range_is_an_error_not_a_silent_drop() {
        let v = json!({"changes": {"file:///w/a.cs": [{"newText": "X"}]}});
        assert!(parse_workspace_edit(&v).is_err());
    }
}

#[cfg(test)]
mod hierarchy_tests {
    use super::*;
    use crate::domain::value_objects::SymbolKind;
    use serde_json::json;

    fn item(name: &str, line: u32) -> Value {
        json!({
            "name": name, "kind": 12, "detail": "fn()",
            "uri": "file:///w/a.rs",
            "range": {"start": {"line": line, "character": 0}, "end": {"line": line + 3, "character": 1}},
            "selectionRange": {"start": {"line": line, "character": 3}, "end": {"line": line, "character": 7}}
        })
    }

    #[test]
    fn a_null_prepare_answer_is_an_empty_list() {
        assert!(
            parse_hierarchy_items("/w/a.rs", &Value::Null)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn a_prepared_item_points_at_its_name_with_one_based_positions() {
        let items = parse_hierarchy_items("/w/a.rs", &json!([item("foo", 4)])).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name, "foo");
        assert_eq!(items[0].kind, SymbolKind::from_lsp_kind(12));
        assert_eq!(items[0].detail.as_deref(), Some("fn()"));
        assert_eq!(
            items[0].location,
            Location::new("/w/a.rs".to_string(), 5, 4)
        );
    }

    #[test]
    fn an_item_without_a_uri_is_malformed_not_dropped() {
        let mut bad = item("foo", 1);
        bad.as_object_mut().unwrap().remove("uri");
        assert!(parse_hierarchy_items("/w/a.rs", &json!([bad])).is_err());
    }

    #[test]
    fn incoming_calls_read_the_caller_from_and_the_call_sites() {
        let result = json!([{
            "from": item("caller", 9),
            "fromRanges": [{"start": {"line": 10, "character": 2}, "end": {"line": 10, "character": 5}}]
        }]);
        let calls = parse_calls(&result, "from").unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].item.name, "caller");
        assert_eq!(calls[0].call_sites.len(), 1);
        assert_eq!(calls[0].call_sites[0].start().line(), 11);
    }

    #[test]
    fn outgoing_calls_read_the_callee_from_to() {
        let result = json!([{ "to": item("callee", 2), "fromRanges": [] }]);
        let calls = parse_calls(&result, "to").unwrap();
        assert_eq!(calls[0].item.name, "callee");
        assert!(calls[0].call_sites.is_empty());
    }

    #[test]
    fn a_call_without_its_other_end_is_malformed() {
        let result = json!([{ "fromRanges": [] }]);
        assert!(parse_calls(&result, "from").is_err());
    }

    #[test]
    fn null_calls_are_an_empty_list() {
        assert!(parse_calls(&Value::Null, "from").unwrap().is_empty());
    }

    #[test]
    fn type_hierarchy_answers_become_direct_nodes() {
        let nodes = parse_type_nodes(&json!([item("Base", 0), item("Other", 7)])).unwrap();
        assert_eq!(nodes.len(), 2);
        assert_eq!(nodes[0].symbol.name(), "Base");
        assert_eq!(nodes[0].distance, 0);
        assert_eq!(nodes[1].symbol.location().line(), 8);
    }
}
