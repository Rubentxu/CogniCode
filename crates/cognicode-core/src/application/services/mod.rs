//! Application Services module

pub mod analysis_service;
// CU-1.2 — producción del puerto `Analyze` sobre `AnalysisService`.
// B2.W1 dejó la firma; este módulo la implementa para las ops de grafo.
pub mod analytics_oracle_harness;
pub mod analyze;
pub mod context_compressor;
pub mod file_operations;
pub mod graph_analytics;
pub mod graph_insights;
pub mod impact_analysis;
pub mod lsp_proxy_service;
pub mod refactor_service;
pub mod relation_candidates;
