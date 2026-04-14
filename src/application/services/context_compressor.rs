//! Context Compression Service for AI agents
//!
//! Transforms verbose JSON responses into compressed natural language summaries
//! to reduce token usage while preserving critical information.

use crate::domain::aggregates::call_graph::CallGraph;
use crate::interface::mcp::schemas::{
    AnalyzeImpactOutput, CallEntry, GetCallHierarchyOutput, GetFileSymbolsOutput, RiskLevel,
    SymbolInfo, SymbolKind,
};

/// Service for compressing JSON responses into natural language summaries
pub struct ContextCompressorService;

impl ContextCompressorService {
    /// Creates a new ContextCompressorService
    pub fn new() -> Self {
        Self
    }

    /// Compresses file symbols into a natural language summary
    ///
    /// Example output:
    /// "order_service.rs: 4 functions (process_order, validate, compute_total, save).
    ///  process_order calls validate + compute_total. No external deps."
    pub fn compress_symbols(
        &self,
        output: &GetFileSymbolsOutput,
        graph: Option<&CallGraph>,
    ) -> String {
        let file_path = &output.file_path;
        let symbols = &output.symbols;

        if symbols.is_empty() {
            return format!("{}: No symbols found", file_path);
        }

        let mut functions = Vec::new();
        let mut classes = Vec::new();
        let mut structs = Vec::new();
        let mut methods = Vec::new();

        for sym in symbols {
            match sym.kind {
                SymbolKind::Function => functions.push(sym),
                SymbolKind::Class => classes.push(sym),
                SymbolKind::Struct => structs.push(sym),
                SymbolKind::Method => methods.push(sym),
                _ => {}
            }
        }

        let functions: Vec<_> = functions;
        let classes: Vec<_> = classes;
        let structs: Vec<_> = structs;
        let methods: Vec<_> = methods;

        let mut summary = format!("{}: ", file_path);

        // Add counts by kind
        if !functions.is_empty() {
            let names: Vec<_> = functions
                .iter()
                .map(|f| format!("{}@{}", f.name, f.location.line))
                .collect();
            summary.push_str(&format!(
                "{} functions ({})",
                functions.len(),
                names.join(", ")
            ));
        }

        if !methods.is_empty() {
            if !functions.is_empty() {
                summary.push_str(". ");
            }
            let names: Vec<_> = methods
                .iter()
                .map(|m| {
                    format!(
                        "{}.{}@{}",
                        extract_type_name(&output.file_path),
                        m.name,
                        m.location.line
                    )
                })
                .collect();
            summary.push_str(&format!("{} methods ({})", methods.len(), names.join(", ")));
        }

        if !classes.is_empty() {
            if !functions.is_empty() || !methods.is_empty() {
                summary.push_str(". ");
            }
            summary.push_str(&format!(
                "{} classes ({})",
                classes.len(),
                classes
                    .iter()
                    .map(|c| c.name.clone())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }

        if !structs.is_empty() {
            if !functions.is_empty() || !methods.is_empty() || !classes.is_empty() {
                summary.push_str(". ");
            }
            summary.push_str(&format!(
                "{} structs ({})",
                structs.len(),
                structs
                    .iter()
                    .map(|s| s.name.clone())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }

        // Add call relationships if graph is available
        if let Some(g) = graph {
            let call_relationships = self.extract_call_relationships(symbols, g);
            if !call_relationships.is_empty() {
                summary.push_str(&format!(". {}", call_relationships));
            }
        }

        summary
    }

    /// Compresses call hierarchy into a natural language summary
    pub fn compress_call_hierarchy(&self, output: &GetCallHierarchyOutput) -> String {
        let symbol = &output.symbol;
        let calls = &output.calls;
        let metadata = &output.metadata;

        if calls.is_empty() {
            return format!(
                "{}: No callers/callees found (completed in {}ms)",
                symbol, metadata.analysis_time_ms
            );
        }

        let mut summary = format!(
            "{}: {} call(s) found (completed in {}ms)\n",
            symbol, metadata.total_calls, metadata.analysis_time_ms
        );

        // Group by file
        let mut by_file: std::collections::HashMap<String, Vec<&CallEntry>> =
            std::collections::HashMap::new();
        for call in calls {
            by_file.entry(call.file.clone()).or_default().push(call);
        }

        for (file, file_calls) in by_file {
            let call_details: Vec<_> = file_calls
                .iter()
                .map(|c| format!("{}(@{})", c.symbol, c.line))
                .collect();
            summary.push_str(&format!("  {}: {}\n", file, call_details.join(", ")));
        }

        summary.trim().to_string()
    }

    /// Compresses impact analysis into a natural language summary
    pub fn compress_impact(&self, output: &AnalyzeImpactOutput) -> String {
        let symbol = &output.symbol;
        let risk = &output.risk_level;
        let files = &output.impacted_files;
        let symbols = &output.impacted_symbols;

        let risk_str = match risk {
            RiskLevel::Low => "LOW",
            RiskLevel::Medium => "MEDIUM",
            RiskLevel::High => "HIGH",
            RiskLevel::Critical => "CRITICAL",
        };

        let mut summary = format!("{}: {} risk. ", symbol, risk_str);

        if files.is_empty() {
            summary.push_str("No impacted files.");
        } else {
            summary.push_str(&format!("{} file(s) would be affected: ", files.len()));
            // Show first 5 files
            let display_files: Vec<_> = files
                .iter()
                .take(5)
                .map(|f| {
                    std::path::Path::new(f)
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| f.clone())
                })
                .collect();

            if files.len() > 5 {
                summary.push_str(&format!(
                    "{}[+{} more]",
                    display_files.join(", "),
                    files.len() - 5
                ));
            } else {
                summary.push_str(&display_files.join(", "));
            }
        }

        if !symbols.is_empty() && symbols.len() <= 10 {
            summary.push_str(&format!(
                ". {} symbol(s): {}",
                symbols.len(),
                symbols.join(", ")
            ));
        } else if symbols.len() > 10 {
            summary.push_str(&format!(". {} symbol(s)", symbols.len()));
        }

        summary
    }

    #[allow(dead_code)]
    /// Extracts file name from path
    fn extract_file_name(file_path: &str) -> String {
        std::path::Path::new(file_path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| file_path.to_string())
    }

    /// Extracts call relationships between symbols
    fn extract_call_relationships(&self, symbols: &[SymbolInfo], graph: &CallGraph) -> String {
        let mut relationships = Vec::new();

        for symbol in symbols.iter().filter(|s| {
            matches!(
                s.kind,
                crate::interface::mcp::schemas::SymbolKind::Function
                    | crate::interface::mcp::schemas::SymbolKind::Method
            )
        }) {
            // Find what this symbol calls
            let symbol_id = crate::domain::aggregates::call_graph::SymbolId::new(format!(
                "{}:{}:{}",
                symbol.location.file, symbol.location.line, symbol.location.column
            ));

            let callees: Vec<_> = graph
                .callees(&symbol_id)
                .iter()
                .filter_map(|(id, _)| graph.get_symbol(id).map(|s| s.name().to_string()))
                .collect();

            if !callees.is_empty() {
                relationships.push(format!("{} calls {}", symbol.name, callees.join(" + ")));
            }
        }

        if relationships.is_empty() {
            String::new()
        } else {
            relationships.join(". ")
        }
    }
}

impl Default for ContextCompressorService {
    fn default() -> Self {
        Self::new()
    }
}

/// Extracts the type name from a file path (for methods)
fn extract_type_name(file_path: &str) -> String {
    let path = std::path::Path::new(file_path);
    let stem = path.file_stem().unwrap_or_default().to_string_lossy();

    // Convert snake_case or kebab-case to PascalCase
    let mut result = String::new();
    for part in stem.split(|c: char| c == '_' || c == '-') {
        let mut chars = part.chars();
        if let Some(first) = chars.next() {
            result.push(first.to_ascii_uppercase());
            result.extend(chars);
        }
    }

    if result.is_empty() {
        stem.to_string()
    } else {
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interface::mcp::schemas::{SourceLocation, SymbolKind};

    #[test]
    fn test_compress_empty_symbols() {
        let service = ContextCompressorService::new();
        let output = GetFileSymbolsOutput {
            file_path: "test.rs".to_string(),
            symbols: vec![],
        };

        let result = service.compress_symbols(&output, None);
        assert_eq!(result, "test.rs: No symbols found");
    }

    #[test]
    fn test_compress_functions_only() {
        let service = ContextCompressorService::new();
        let output = GetFileSymbolsOutput {
            file_path: "order_service.rs".to_string(),
            symbols: vec![
                SymbolInfo {
                    name: "process_order".to_string(),
                    kind: SymbolKind::Function,
                    location: SourceLocation {
                        file: "order_service.rs".to_string(),
                        line: 42,
                        column: 0,
                    },
                    signature: Some("fn process_order(id: i32)".to_string()),
                },
                SymbolInfo {
                    name: "validate".to_string(),
                    kind: SymbolKind::Function,
                    location: SourceLocation {
                        file: "order_service.rs".to_string(),
                        line: 58,
                        column: 0,
                    },
                    signature: Some("fn validate(order: Order)".to_string()),
                },
            ],
        };

        let result = service.compress_symbols(&output, None);
        assert!(result.contains("order_service.rs"));
        assert!(result.contains("2 functions"));
        assert!(result.contains("process_order@42"));
        assert!(result.contains("validate@58"));
    }

    #[test]
    fn test_compress_call_hierarchy() {
        let service = ContextCompressorService::new();
        let output = GetCallHierarchyOutput {
            symbol: "process_order".to_string(),
            calls: vec![
                CallEntry {
                    symbol: "validate".to_string(),
                    file: "order_service.rs".to_string(),
                    line: 58,
                    column: 0,
                    confidence: 1.0,
                },
                CallEntry {
                    symbol: "compute_total".to_string(),
                    file: "order_service.rs".to_string(),
                    line: 72,
                    column: 0,
                    confidence: 1.0,
                },
            ],
            metadata: crate::interface::mcp::schemas::AnalysisMetadata {
                total_calls: 2,
                analysis_time_ms: 15,
            },
        };

        let result = service.compress_call_hierarchy(&output);
        assert!(result.contains("process_order"));
        assert!(result.contains("2 call(s) found"));
        assert!(result.contains("validate(@58)"));
        assert!(result.contains("compute_total(@72)"));
    }

    #[test]
    fn test_compress_impact_low_risk() {
        let service = ContextCompressorService::new();
        let output = AnalyzeImpactOutput {
            symbol: "helper_func".to_string(),
            impacted_files: vec!["main.rs".to_string()],
            impacted_symbols: vec!["main".to_string()],
            risk_level: RiskLevel::Low,
            summary: "Low impact analysis".to_string(),
        };

        let result = service.compress_impact(&output);
        assert!(result.contains("LOW risk"));
        assert!(result.contains("1 file(s) would be affected"));
    }

    #[test]
    fn test_compress_impact_high_risk() {
        let service = ContextCompressorService::new();
        let output = AnalyzeImpactOutput {
            symbol: "Config".to_string(),
            impacted_files: vec![
                "main.rs".to_string(),
                "lib.rs".to_string(),
                "config.rs".to_string(),
            ],
            impacted_symbols: vec![
                "main".to_string(),
                "init".to_string(),
                "load_config".to_string(),
            ],
            risk_level: RiskLevel::High,
            summary: "High impact analysis".to_string(),
        };

        let result = service.compress_impact(&output);
        assert!(result.contains("HIGH risk"));
        assert!(result.contains("3 file(s) would be affected"));
    }

    #[test]
    fn test_extract_type_name() {
        assert_eq!(extract_type_name("order_service.rs"), "OrderService");
        assert_eq!(
            extract_type_name("my_module/order_handler.rs"),
            "OrderHandler"
        );
        assert_eq!(extract_type_name("order-service.rs"), "OrderService");
    }

    #[test]
    fn test_compression_reduces_tokens_python() {
        let service = ContextCompressorService::new();

        // Real Python code with many symbols
        let output = GetFileSymbolsOutput {
            file_path: "order_service.py".to_string(),
            symbols: vec![
                SymbolInfo {
                    name: "process_order".to_string(),
                    kind: SymbolKind::Function,
                    location: SourceLocation {
                        file: "order_service.py".to_string(),
                        line: 10,
                        column: 0,
                    },
                    signature: Some(
                        "def process_order(order_id: int, items: list) -> dict:".to_string(),
                    ),
                },
                SymbolInfo {
                    name: "validate_order".to_string(),
                    kind: SymbolKind::Function,
                    location: SourceLocation {
                        file: "order_service.py".to_string(),
                        line: 50,
                        column: 0,
                    },
                    signature: Some("def validate_order(order: dict) -> bool:".to_string()),
                },
                SymbolInfo {
                    name: "compute_total".to_string(),
                    kind: SymbolKind::Function,
                    location: SourceLocation {
                        file: "order_service.py".to_string(),
                        line: 90,
                        column: 0,
                    },
                    signature: Some("def compute_total(items: list) -> float:".to_string()),
                },
                SymbolInfo {
                    name: "save_order".to_string(),
                    kind: SymbolKind::Function,
                    location: SourceLocation {
                        file: "order_service.py".to_string(),
                        line: 130,
                        column: 0,
                    },
                    signature: Some("def save_order(order: dict) -> int:".to_string()),
                },
                SymbolInfo {
                    name: "send_confirmation".to_string(),
                    kind: SymbolKind::Function,
                    location: SourceLocation {
                        file: "order_service.py".to_string(),
                        line: 170,
                        column: 0,
                    },
                    signature: Some(
                        "def send_confirmation(order_id: int, email: str) -> None:".to_string(),
                    ),
                },
            ],
        };

        let compressed = service.compress_symbols(&output, None);

        // The compressed output should be significantly shorter than the raw JSON
        let original_json_len = serde_json::to_string(&output).unwrap().len();
        let compressed_len = compressed.len();

        // Compression should reduce size by at least 50%
        assert!(
            compressed_len < original_json_len / 2,
            "Compression should reduce size significantly. Original: {}, Compressed: {}",
            original_json_len,
            compressed_len
        );

        // But should still contain key information
        assert!(compressed.contains("order_service.py"));
        assert!(compressed.contains("5 functions"));
        assert!(compressed.contains("process_order@10"));
    }

    #[test]
    fn test_compression_reduces_tokens_rust() {
        let service = ContextCompressorService::new();

        // Real Rust code with structs and impl blocks
        let output = GetFileSymbolsOutput {
            file_path: "order_service.rs".to_string(),
            symbols: vec![
                SymbolInfo {
                    name: "Order".to_string(),
                    kind: SymbolKind::Struct,
                    location: SourceLocation {
                        file: "order_service.rs".to_string(),
                        line: 5,
                        column: 0,
                    },
                    signature: Some("struct Order { id, items, total }".to_string()),
                },
                SymbolInfo {
                    name: "OrderService".to_string(),
                    kind: SymbolKind::Struct,
                    location: SourceLocation {
                        file: "order_service.rs".to_string(),
                        line: 15,
                        column: 0,
                    },
                    signature: Some("struct OrderService { database, queue }".to_string()),
                },
                SymbolInfo {
                    name: "process".to_string(),
                    kind: SymbolKind::Method,
                    location: SourceLocation {
                        file: "order_service.rs".to_string(),
                        line: 25,
                        column: 0,
                    },
                    signature: Some("fn process(&self, order: Order) -> Result<()>".to_string()),
                },
                SymbolInfo {
                    name: "validate".to_string(),
                    kind: SymbolKind::Method,
                    location: SourceLocation {
                        file: "order_service.rs".to_string(),
                        line: 45,
                        column: 0,
                    },
                    signature: Some("fn validate(&self, order: &Order) -> bool".to_string()),
                },
                SymbolInfo {
                    name: "calculate_total".to_string(),
                    kind: SymbolKind::Method,
                    location: SourceLocation {
                        file: "order_service.rs".to_string(),
                        line: 65,
                        column: 0,
                    },
                    signature: Some(
                        "fn calculate_total(&self, items: &[Item]) -> Money".to_string(),
                    ),
                },
            ],
        };

        let compressed = service.compress_symbols(&output, None);

        let original_json_len = serde_json::to_string(&output).unwrap().len();
        let compressed_len = compressed.len();

        // Compression should reduce size significantly
        assert!(
            compressed_len < original_json_len / 2,
            "Compression should reduce size. Original: {}, Compressed: {}",
            original_json_len,
            compressed_len
        );

        // Should contain key information
        assert!(compressed.contains("order_service.rs"));
        assert!(compressed.contains("Order"));
        assert!(compressed.contains("OrderService"));
    }

    #[test]
    fn test_compress_call_hierarchy_multi_level() {
        let service = ContextCompressorService::new();

        // Multi-level call hierarchy
        let output = GetCallHierarchyOutput {
            symbol: "main".to_string(),
            calls: vec![
                // Level 1 callers
                CallEntry {
                    symbol: "init".to_string(),
                    file: "main.rs".to_string(),
                    line: 10,
                    column: 0,
                    confidence: 1.0,
                },
                CallEntry {
                    symbol: "process".to_string(),
                    file: "main.rs".to_string(),
                    line: 15,
                    column: 0,
                    confidence: 1.0,
                },
                // Level 2 callers (from different file)
                CallEntry {
                    symbol: "cleanup".to_string(),
                    file: "utils.rs".to_string(),
                    line: 50,
                    column: 0,
                    confidence: 0.95,
                },
            ],
            metadata: crate::interface::mcp::schemas::AnalysisMetadata {
                total_calls: 3,
                analysis_time_ms: 25,
            },
        };

        let result = service.compress_call_hierarchy(&output);

        assert!(result.contains("main"));
        assert!(result.contains("3 call(s) found"));
        assert!(result.contains("25ms"));
        assert!(result.contains("main.rs"));
        assert!(result.contains("utils.rs"));
    }

    #[test]
    fn test_compress_impact_critical_risk() {
        let service = ContextCompressorService::new();

        // Critical risk with many impacted files
        let output = AnalyzeImpactOutput {
            symbol: "DatabaseConnection".to_string(),
            impacted_files: vec![
                "main.rs".to_string(),
                "lib.rs".to_string(),
                "db/mod.rs".to_string(),
                "db/connection.rs".to_string(),
                "services/user.rs".to_string(),
                "services/order.rs".to_string(),
                "services/payment.rs".to_string(),
            ],
            impacted_symbols: vec![
                "main".to_string(),
                "init".to_string(),
                "UserService".to_string(),
                "OrderService".to_string(),
                "PaymentProcessor".to_string(),
            ],
            risk_level: RiskLevel::Critical,
            summary: "Critical impact analysis".to_string(),
        };

        let result = service.compress_impact(&output);

        assert!(result.contains("CRITICAL risk"));
        assert!(result.contains("7 file(s) would be affected"));
        assert!(result.contains("[+2 more]")); // Only shows first 5
        assert!(result.contains("5 symbol(s)"));
    }

    #[test]
    fn test_compress_impact_no_impacted_files() {
        let service = ContextCompressorService::new();

        let output = AnalyzeImpactOutput {
            symbol: "unused_helper".to_string(),
            impacted_files: vec![],
            impacted_symbols: vec![],
            risk_level: RiskLevel::Low,
            summary: "No impact".to_string(),
        };

        let result = service.compress_impact(&output);

        assert!(result.contains("LOW risk"));
        assert!(result.contains("No impacted files"));
    }

    #[test]
    fn test_compress_symbols_mixed_kinds() {
        let service = ContextCompressorService::new();

        let output = GetFileSymbolsOutput {
            file_path: "mixed.rs".to_string(),
            symbols: vec![
                SymbolInfo {
                    name: "Config".to_string(),
                    kind: SymbolKind::Struct,
                    location: SourceLocation {
                        file: "mixed.rs".to_string(),
                        line: 5,
                        column: 0,
                    },
                    signature: None,
                },
                SymbolInfo {
                    name: "init".to_string(),
                    kind: SymbolKind::Function,
                    location: SourceLocation {
                        file: "mixed.rs".to_string(),
                        line: 20,
                        column: 0,
                    },
                    signature: None,
                },
                SymbolInfo {
                    name: "process".to_string(),
                    kind: SymbolKind::Method,
                    location: SourceLocation {
                        file: "mixed.rs".to_string(),
                        line: 35,
                        column: 0,
                    },
                    signature: None,
                },
                SymbolInfo {
                    name: "Handler".to_string(),
                    kind: SymbolKind::Class,
                    location: SourceLocation {
                        file: "mixed.rs".to_string(),
                        line: 50,
                        column: 0,
                    },
                    signature: None,
                },
            ],
        };

        let result = service.compress_symbols(&output, None);

        assert!(result.contains("mixed.rs"));
        assert!(result.contains("1 functions"));
        assert!(result.contains("1 methods"));
        assert!(result.contains("1 classes"));
        assert!(result.contains("1 structs"));
    }
}
