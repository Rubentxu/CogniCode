//! Application Services module

pub mod analysis_service;
// Una escritura que deja el fichero entero o no lo deja: el temporal se
// crea en exclusiva, se borra si no se publica, y el contenido es durable
// antes de que el destino exista.
pub mod atomic_file_writer;
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
