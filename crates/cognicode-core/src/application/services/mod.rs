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
// Casar un patron contra el texto de un fichero, sin abrir ninguno.
pub(crate) mod content_search;
// Lo que el servicio recuerda de cada fichero leido (TASK-228).
pub(crate) mod file_facts_cache;
pub mod file_operations;
pub mod graph_analytics;
// Las preguntas del grafo, contestadas una vez (TASK-78).
pub mod graph_insights;
pub mod graph_queries;
pub mod impact_analysis;
pub mod impact_of;
pub mod lsp_proxy_service;
pub mod refactor_service;
pub mod relation_candidates;
// Lo que un recorrido no pudo leer, publicado como hueco (TASK-258).
pub(crate) mod unread_entries;
// La busqueda de usos de un simbolo y lo que cubrio.
pub mod usage_search;
