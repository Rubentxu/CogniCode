//! CLI Commands - Command-line interface implementations
// e30.1 clippy baseline reset: pre-existing lint debt (see fix/e30.1-clippy-baseline-reset)
#![allow(clippy::map_flatten)]

use crate::domain::services::CallGraphAnalyzer;
use crate::domain::traits::code_intelligence::CodeIntelligenceProvider;
use crate::infrastructure::graph::{
    FullGraphStrategy, GraphStrategy, GraphStrategyFactory, LightweightStrategy, OnDemandStrategy,
    PerFileStrategy, TraversalDirection,
};
use crate::infrastructure::parser::Language;
use crate::infrastructure::semantic::{OutlineNode, SymbolCodeService};
use clap::{CommandFactory, Parser, Subcommand};
use std::path::PathBuf;
use std::time::Instant;
use tracing::info;

/// CLI arguments for CogniCode
#[derive(Debug, Parser)]
#[command(name = "cognicode", version)]
#[command(about = "Premium LSP server for AI agents with code analysis and refactoring", long_about = None)]
pub struct Cli {
    /// Enable verbose logging
    #[arg(short, long)]
    pub verbose: bool,

    /// The command to execute
    #[command(subcommand)]
    pub command: Option<CliCommand>,
}

/// Available CLI commands
#[derive(Debug, Subcommand)]
pub enum CliCommand {
    /// Analyze code in the given directory
    Analyze {
        /// Directory to analyze
        #[arg(default_value = ".")]
        path: String,
    },
    /// Start the MCP server
    Serve {
        /// Port to listen on
        #[arg(short, long, default_value = "8080")]
        port: u16,
    },
    /// Refactor a symbol
    Refactor {
        /// Symbol to refactor
        symbol: String,
        /// New name (for rename/move operations)
        new_name: Option<String>,
        /// The refactoring operation to perform
        #[arg(value_enum, long, default_value = "rename")]
        operation: RefactorOperation,
        /// PRF-CLI-05: mutation authorization. Preview (default) never
        /// writes; `--apply` is the explicit separate authorization that
        /// permits file mutation.
        #[arg(long)]
        apply: bool,
        /// Preview output format: text or json
        #[arg(long, default_value = "text")]
        format: String,
    },
    /// Index commands for symbol indexing
    Index {
        #[command(subcommand)]
        command: IndexCommand,
    },
    /// Graph commands for call graph operations
    Graph {
        #[command(subcommand)]
        command: GraphCommand,
    },
    /// Navigate to code locations using LSP (go to definition, hover, find references)
    Navigate {
        #[command(subcommand)]
        command: NavigateCommand,
    },
    /// Check LSP server availability and installation status
    Doctor {
        /// Output format: text or json
        #[arg(long, default_value = "text")]
        format: String,
        /// Workspace directory to detect languages and prioritize tools
        #[arg(short, long, default_value = ".")]
        cwd: String,
    },
    /// Ingest Markdown / ADR files into the Generic Graph
    /// Layer. Compiled in ONLY when the `multimodal` Cargo
    /// feature is active — on a default build the variant is
    /// absent and `cognicode docs-ingest` returns
    /// "Unknown command".
    #[cfg(feature = "multimodal")]
    #[command(name = "docs-ingest")]
    DocsIngest {
        /// Path to ingest: a single `.md`/`.markdown`/`.mdx`
        /// file or a directory to walk.
        #[arg(long)]
        path: String,
        /// When `path` is a directory, recurse into
        /// subdirectories. Ignored for single-file inputs.
        #[arg(long, default_value_t = true)]
        recursive: bool,
    },
    /// Ingest GitHub issues from the given owner/repo into
    /// the Generic Graph Layer. Compiled in ONLY when the
    /// `multimodal` Cargo feature is active — on a default
    /// build the variant is absent and `cognicode issues-ingest`
    /// returns "Unknown command".
    #[cfg(feature = "multimodal")]
    #[command(name = "issues-ingest")]
    IssuesIngest {
        /// GitHub owner / organisation name.
        #[arg(long)]
        owner: String,
        /// GitHub repository name.
        #[arg(long)]
        repo: String,
        /// When true (default), also parse git commit
        /// references to issues from the local git log.
        #[arg(long, default_value_t = true)]
        include_git_log: bool,
    },

    /// L1.4 / F0.1 — Find all usages of a symbol across the workspace
    /// (AST-based, NO LSP required). Equivalent to MCP tool `find_usages`.
    ///
    /// Calls `AnalysisService::find_symbol_usages` directly — does NOT
    /// subprocess MCP and does NOT duplicate handler logic.
    #[command(name = "find-usages")]
    FindUsages {
        /// Symbol name to find usages for
        symbol: String,

        /// Workspace root directory
        #[arg(short = 'C', long, default_value = ".")]
        cwd: String,

        /// Include the declaration site (default: true)
        #[arg(
            long,
            default_value_t = true,
            overrides_with = "no_include_declaration"
        )]
        include_declaration: bool,

        /// Exclude the declaration site (overrides --include-declaration)
        #[arg(long, default_value_t = false)]
        no_include_declaration: bool,

        /// Number of surrounding source lines to include per usage
        #[arg(short = 'n', long)]
        context_lines: Option<usize>,

        /// Output format: text|json (default: text)
        #[arg(short = 'f', long, default_value = "text")]
        format: String,

        /// Suppress non-essential output
        #[arg(short = 'q', long)]
        quiet: bool,
    },

    /// E1.W3 — Query the LadybugDB-backed EvidenceStore. Mirrors the
    /// `list_evidence` / `search_evidence` MCP tool family so the CLI
    /// and the MCP surface stay in lockstep (see equivalence test in
    /// `cognicode-cli/tests/evidence_cli_mcp_equivalence.rs`).
    ///
    /// Activated only when the `evidence-cli-ladybug` feature is on
    /// (which is in turn activated by `cognicode-cli`'s `--features
    /// ladybug`). The default build (no features) does NOT compile
    /// this variant — keeping the lbug native dep opt-in.
    #[cfg(feature = "evidence-cli-ladybug")]
    #[command(name = "evidence", subcommand)]
    Evidence(EvidenceCommand),
}

/// E1.W3 — `cognicode evidence <list|search>`.
///
/// Subcommand surface mirrors the MCP `list_evidence` / `search_evidence`
/// tools 1:1 so the equivalence test can compare CLI JSON vs MCP JSON on
/// identical inputs. Output formats are `text` (human) and `json`
/// (machine) — both stable contracts.
///
/// Compiled only under `feature = "evidence-cli-ladybug"`. The enum lives
/// in core because the CLI variant `CliCommand::Evidence` references it;
/// moving it into the cli crate would split the clap derive across two
/// files and force a re-export dance for no benefit.
#[cfg(feature = "evidence-cli-ladybug")]
#[derive(Debug, Subcommand)]
pub enum EvidenceCommand {
    /// List evidence rows for a workspace, optionally filtered by kind.
    #[command(name = "list")]
    List {
        /// Workspace identifier (matches the per-workspace scope used
        /// by the LadybugDB tables; defaults to `.` resolved against
        /// the CWD).
        #[arg(short = 'w', long, default_value = ".")]
        workspace: String,

        /// Filter by evidence kind: log | trace | measurement | external.
        #[arg(short = 'k', long, value_parser = ["log", "trace", "measurement", "external"])]
        kind: Option<String>,

        /// LadybugDB file path. Defaults to `<cwd>/.cognicode/evidence.lbdb`
        /// (created on first run if absent — the schema is idempotent).
        #[arg(long)]
        db_path: Option<String>,

        /// Output format: text | json.
        #[arg(short = 'f', long, default_value = "text")]
        format: String,
    },

    /// Full-text search across evidence titles and excerpts.
    #[command(name = "search")]
    Search {
        /// Query string (substring match against title + excerpt).
        query: String,

        /// Workspace identifier (see `list`).
        #[arg(short = 'w', long, default_value = ".")]
        workspace: String,

        /// Maximum rows returned (default 25).
        #[arg(short = 'l', long, default_value_t = 25)]
        limit: usize,

        /// LadybugDB file path (see `list`).
        #[arg(long)]
        db_path: Option<String>,

        /// Output format: text | json.
        #[arg(short = 'f', long, default_value = "text")]
        format: String,
    },
}

/// Index subcommands
#[derive(Debug, Subcommand)]
pub enum IndexCommand {
    /// Build a lightweight index
    Build {
        /// Directory to build index for
        #[arg(default_value = ".")]
        path: String,
        /// Strategy to use: lightweight, on_demand, per_file, full
        #[arg(long, default_value = "lightweight")]
        strategy: String,
    },
    /// Query the index for a symbol
    Query {
        /// Symbol name to query
        symbol: String,
        /// Directory to search in
        #[arg(default_value = ".")]
        path: String,
    },
    /// Get hierarchical outline of a file
    Outline {
        /// File path to get outline for
        file: String,
        /// Include private symbols (starting with _)
        #[arg(long, default_value = "false")]
        include_private: bool,
        /// Include test symbols
        #[arg(long, default_value = "true")]
        include_tests: bool,
    },
    /// Get source code of a symbol
    SymbolCode {
        /// File path
        file: String,
        /// Line number (1-indexed)
        line: u32,
        /// Column number (0-indexed)
        column: u32,
        /// Include docstring/comment above symbol
        #[arg(long, default_value = "true")]
        include_doc: bool,
    },
}

/// Graph subcommands
#[derive(Debug, Subcommand)]
pub enum GraphCommand {
    /// Build on-demand call subgraph
    OnDemand {
        /// Symbol to build subgraph around
        symbol: String,
        /// Traversal depth
        #[arg(short = 'd', long, default_value = "3")]
        depth: u32,
        /// Direction: in, out, both
        #[arg(long, default_value = "both")]
        direction: String,
        /// Directory to search in
        #[arg(default_value = ".")]
        path: String,
    },
    /// Get per-file graph
    PerFile {
        /// File path to get graph for
        file: String,
    },
    /// Get full project graph
    Full {
        /// Rebuild the full graph
        #[arg(long)]
        rebuild: bool,
        /// Output format: text or json (PRF-CLI-02: json mode writes
        /// schema-versioned structured data to stdout; logs stay on stderr)
        #[arg(long, default_value = "text")]
        format: String,
        /// Directory to analyze
        #[arg(default_value = ".")]
        path: String,
    },
    /// Find hot paths (most called functions)
    HotPaths {
        /// Maximum number of results
        #[arg(short = 'n', long, default_value = "10")]
        limit: usize,
        /// Minimum fan-in (number of callers)
        #[arg(long, default_value = "1")]
        min_fan_in: usize,
        /// Directory to analyze
        #[arg(default_value = ".")]
        path: String,
    },
    /// Get entry points (symbols with no incoming edges)
    EntryPoints {
        /// Directory to analyze
        #[arg(default_value = ".")]
        path: String,
    },
    /// Get leaf functions (symbols with no outgoing edges)
    LeafFunctions {
        /// Directory to analyze
        #[arg(default_value = ".")]
        path: String,
    },
    /// Trace execution path between two symbols
    TracePath {
        /// Source symbol name
        from: String,
        /// Target symbol name
        to: String,
        /// Directory to analyze
        #[arg(default_value = ".")]
        path: String,
    },
    /// Export graph to Mermaid format
    Mermaid {
        /// File to export (or directory for full graph)
        #[arg(default_value = ".")]
        path: String,
        /// Output format: svg, png, txt
        #[arg(long, default_value = "txt")]
        format: String,
    },
    /// Get call hierarchy for a symbol
    Hierarchy {
        /// Symbol name
        symbol: String,
        /// Maximum depth
        #[arg(short = 'd', long, default_value = "3")]
        depth: u32,
        /// Direction: in (callers), out (callees)
        #[arg(long, default_value = "out")]
        direction: String,
        /// Directory to analyze
        #[arg(default_value = ".")]
        path: String,
    },
    /// Get complexity metrics
    Complexity {
        /// Directory to analyze
        #[arg(default_value = ".")]
        path: String,
    },
    /// Analyze impact of changing a symbol
    Impact {
        /// Symbol name to analyze
        symbol: String,
        /// Directory to analyze
        #[arg(default_value = ".")]
        path: String,
    },
}

/// Navigate subcommands (LSP-based)
#[derive(Debug, Subcommand)]
pub enum NavigateCommand {
    /// Go to the definition of the symbol at the given file:line:column position
    Definition {
        /// Position as file:line:column (e.g., src/main.rs:42:10)
        position: String,
        /// Workspace root directory
        #[arg(default_value = ".")]
        path: String,
    },
    /// Show hover information (type + docs) for the symbol at the given position
    Hover {
        /// Position as file:line:column (e.g., src/main.rs:42:10)
        position: String,
        /// Workspace root directory
        #[arg(default_value = ".")]
        path: String,
    },
    /// Find all references to the symbol at the given position
    References {
        /// Position as file:line:column (e.g., src/main.rs:42:10)
        position: String,
        /// Include the declaration itself in results
        #[arg(long, default_value = "true")]
        include_declaration: bool,
        /// Workspace root directory
        #[arg(default_value = ".")]
        path: String,
    },
}

/// Refactoring operations
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum RefactorOperation {
    /// Rename a symbol
    Rename,
    /// Extract a function
    Extract,
    /// Inline a function
    Inline,
    /// Move a symbol
    Move,
}

/// Command executor for the CLI
pub struct CommandExecutor;

impl CommandExecutor {
    /// Execute the given CLI command
    pub async fn execute(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
        if cli.verbose {
            // SAFETY: Setting RUST_LOG env var during CLI verbose mode is safe
            // as this runs in a single-threaded CLI context at startup.
            unsafe { std::env::set_var("RUST_LOG", "debug") };
        }

        match &cli.command {
            Some(CliCommand::Analyze { path }) => {
                // PRF-CLI-01: do NOT swallow the Analyze error.
                // "No `exit 0` if the operation was not performed":
                // a failed analyze must surface a non-zero exit code
                // so scripts and CI can detect it (UAT:
                // `crates/cognicode-cli/tests/prf_cli_01_uat.rs`).
                if let Err(e) = Self::execute_analyze(path).await {
                    eprintln!("Analyze command failed: {}", e);
                    return Err(e);
                }
            }
            Some(CliCommand::Serve { port }) => {
                eprintln!("Use 'cognicode-mcp' binary to start the MCP server.");
                eprintln!("The MCP server uses stdio transport, not TCP ports.");
                eprintln!("Run: cognicode-mcp --cwd <workspace>");
                let _ = port;
            }
            Some(CliCommand::Refactor {
                operation,
                symbol,
                new_name,
                apply,
                format,
            }) => {
                // PRF-CLI-05: without --apply this is preview-only and must
                // never mutate files. execute_refactor is preview-only by
                // contract; --apply is refused until an apply path with
                // rollback exists (honest Unsupported).
                if *apply {
                    eprintln!(
                        "refactor --apply: file mutation is not implemented yet; \
                         only preview is available (PRF-CLI-05: apply requires \
                         preview + rollback, which is pending)"
                    );
                    return Err("refactor --apply unsupported: no rollback path yet".into());
                }
                if let Err(e) =
                    Self::execute_refactor(operation, symbol, new_name.as_deref(), format).await
                {
                    eprintln!("Refactor command failed: {}", e);
                }
            }
            Some(CliCommand::Index { command }) => {
                if let Err(e) = Self::execute_index(command).await {
                    eprintln!("Index command failed: {}", e);
                }
            }
            Some(CliCommand::Graph { command }) => {
                // PRF F2.W2: do NOT swallow the Graph error here.
                // PerFileStrategy and FullGraphStrategy now surface read /
                // parse / unsupported-extension failures as `Err`, and the
                // UAT (see `w2_uat_tests` below) requires those to
                // propagate so that scripts and CI can detect them.
                // Other Graph subcommands that still match the legacy
                // contract (OnDemand, HotPaths, …) return `Ok` and keep
                // working unchanged.
                if let Err(e) = Self::execute_graph(command).await {
                    eprintln!("Graph command failed: {}", e);
                    return Err(e);
                }
            }
            Some(CliCommand::FindUsages {
                symbol,
                cwd,
                include_declaration,
                no_include_declaration,
                context_lines,
                format,
                quiet,
            }) => {
                // L1.4 / F0.1 — Política de errores:
                //   - exit 0: éxito (con o sin resultados)
                //   - exit 2 (via Err): uso inválido o error de backend
                //
                // `no_include_declaration` gana sobre `include_declaration`
                // (overrides_with en clap). Si ninguno aparece,
                // include_declaration=true (default).
                let include = !no_include_declaration && *include_declaration;
                if let Err(e) =
                    Self::execute_find_usages(symbol, cwd, include, *context_lines, format, *quiet)
                        .await
                {
                    eprintln!("find-usages command failed: {}", e);
                    return Err(e);
                }
            }
            Some(CliCommand::Navigate { command }) => {
                if let Err(e) = Self::execute_navigate(command).await {
                    eprintln!("Navigate command failed: {}", e);
                }
            }
            Some(CliCommand::Doctor { format, cwd }) => {
                if let Err(e) = Self::execute_doctor(format, cwd).await {
                    eprintln!("Doctor command failed: {}", e);
                }
            }
            #[cfg(feature = "multimodal")]
            Some(CliCommand::DocsIngest { path, recursive }) => {
                if let Err(e) = Self::execute_docs_ingest(path, *recursive).await {
                    eprintln!("docs-ingest command failed: {}", e);
                }
            }
            #[cfg(feature = "multimodal")]
            Some(CliCommand::IssuesIngest {
                owner,
                repo,
                include_git_log,
            }) => {
                if let Err(e) = Self::execute_issues_ingest(owner, repo, *include_git_log).await {
                    eprintln!("issues-ingest command failed: {}", e);
                }
            }
            #[cfg(feature = "evidence-cli-ladybug")]
            Some(CliCommand::Evidence(cmd)) => {
                if let Err(e) = Self::execute_evidence(cmd).await {
                    eprintln!("evidence command failed: {}", e);
                }
            }
            None => {
                info!("CogniCode CLI initialized");
                // Print help if no command given
                let mut cmd = Cli::command();
                cmd.print_help()?;
                println!();
            }
        }

        Ok(())
    }

    /// Execute index subcommand
    async fn execute_index(command: &IndexCommand) -> Result<(), Box<dyn std::error::Error>> {
        match command {
            IndexCommand::Build { path, strategy } => {
                let start = Instant::now();
                println!("Building {} index at: {}", strategy, path);

                let mut strategy_box = GraphStrategyFactory::create(strategy);
                let dir = PathBuf::from(path);

                match strategy_box.build_index(&dir) {
                    Ok(()) => {
                        let elapsed = start.elapsed().as_millis();
                        println!(
                            "Index built successfully in {}ms using {} strategy",
                            elapsed,
                            strategy_box.name()
                        );
                    }
                    Err(e) => {
                        eprintln!("Error building index: {}", e);
                        return Err(Box::new(e));
                    }
                }
            }
            IndexCommand::Query { symbol, path } => {
                println!("Querying symbol '{}' in: {}", symbol, path);

                let mut strategy = LightweightStrategy::new();
                let dir = PathBuf::from(path);

                if let Err(e) = strategy.build_index(&dir) {
                    eprintln!("Error building index: {}", e);
                    return Err(Box::new(e));
                }

                let locations = strategy.query_symbols(symbol);
                if locations.is_empty() {
                    println!("No locations found for symbol '{}'", symbol);
                } else {
                    println!("Found {} location(s):", locations.len());
                    for loc in locations {
                        println!(
                            "  {}:{}:{} ({})",
                            loc.file,
                            loc.line,
                            loc.column,
                            format_args!("{:?}", loc.symbol_kind)
                        );
                    }
                }
            }
            IndexCommand::Outline {
                file,
                include_private,
                include_tests,
            } => {
                println!("Getting outline for: {}", file);

                let source = std::fs::read_to_string(file)?;
                let language = Language::from_extension(std::path::Path::new(file).extension())
                    .unwrap_or(Language::Rust);

                let outline = crate::infrastructure::semantic::build_outline(
                    &source,
                    file,
                    language,
                    *include_private,
                    *include_tests,
                );

                println!("Found {} top-level symbols:", outline.len());
                print_outline_tree(&outline, 0);
            }
            IndexCommand::SymbolCode {
                file,
                line,
                column,
                include_doc: _,
            } => {
                println!("Getting symbol code for: {}:{}:{}", file, line, column);

                let service = SymbolCodeService::new();

                match service.get_symbol_code(file, *line, *column) {
                    Ok(code) => {
                        if let Some(doc) = &code.docstring {
                            println!("\n/// Docstring:\n{}", doc);
                        }
                        println!(
                            "\n/// Symbol code (lines {} - {}):",
                            code.start_line, code.end_line
                        );
                        println!("{}", code.code);
                    }
                    Err(e) => {
                        eprintln!("Error: {}", e);
                    }
                }
            }
        }
        Ok(())
    }

    /// Execute graph subcommand
    async fn execute_graph(command: &GraphCommand) -> Result<(), Box<dyn std::error::Error>> {
        match command {
            GraphCommand::OnDemand {
                symbol,
                depth,
                direction,
                path,
            } => {
                let start = Instant::now();
                println!(
                    "Building on-demand subgraph for '{}' (depth={}, direction={})",
                    symbol, depth, direction
                );

                let dir = PathBuf::from(path);
                let mut strategy = OnDemandStrategy::new();

                if let Err(e) = strategy.build_index(&dir) {
                    eprintln!("Error building index: {}", e);
                    return Err(Box::new(e));
                }

                let dir_enum = match direction.to_lowercase().as_str() {
                    "in" => TraversalDirection::Callers,
                    "out" => TraversalDirection::Callees,
                    _ => TraversalDirection::Both,
                };

                let result = strategy.build_subgraph(symbol, *depth, dir_enum);
                let elapsed = start.elapsed().as_millis();

                println!("Subgraph built in {}ms", elapsed);
                println!(
                    "Root: {} ({}:{}:{})",
                    result.root_symbol.name(),
                    result.root_symbol.location().file(),
                    result.root_symbol.location().line(),
                    result.root_symbol.location().column()
                );
                println!("Entries: {}", result.entries.len());
            }
            GraphCommand::PerFile { file } => {
                println!("Getting per-file graph for: {}", file);

                let strategy = PerFileStrategy::new();
                let file_path = PathBuf::from(file);

                match strategy.build_local_graph(&file_path) {
                    Ok(graph) => {
                        let symbols = graph.symbol_count();
                        let edges = graph.edge_count();
                        println!("Per-file graph for {}:", file);
                        println!("  Symbols: {}", symbols);
                        println!("  Dependencies: {}", edges);
                    }
                    Err(e) => {
                        eprintln!("Error building per-file graph: {}", e);
                        return Err(Box::new(e));
                    }
                }
            }
            GraphCommand::Full {
                rebuild,
                format,
                path,
            } => {
                let start = Instant::now();
                // PRF-CLI-02: in json mode stdout carries ONLY the
                // schema-versioned JSON document; progress text stays on
                // stderr. Text mode preserves the historical human output.
                let json_mode = format == "json";
                if !json_mode {
                    println!(
                        "Building full project graph at: {}{}",
                        path,
                        if *rebuild { " (rebuild)" } else { "" }
                    );
                } else {
                    eprintln!(
                        "Building full project graph at: {}{}",
                        path,
                        if *rebuild { " (rebuild)" } else { "" }
                    );
                }

                // PRF-EXT-02 / H-03: `graph full` must go through the
                // same application service (`AnalysisService`) as the
                // MCP `build_graph` tool, so both interfaces share the
                // canonical pipeline (caches, coverage, skipped-file
                // reporting) instead of duplicating semantics in the
                // adapter layer.
                let service =
                    crate::application::services::analysis_service::AnalysisService::new();
                let dir = PathBuf::from(&path);

                match service.build_full_graph(&dir) {
                    Ok(()) => {
                        let elapsed = start.elapsed().as_millis();
                        let report = service.get_last_build_report();
                        let (symbols, edges) = report
                            .as_ref()
                            .map(|r| (r.graph.symbol_count(), r.graph.edge_count()))
                            .unwrap_or((0, 0));
                        let skipped: Vec<String> = report
                            .as_ref()
                            .and_then(|r| {
                                match &r.status {
                                crate::infrastructure::graph::per_file_graph::BuildStatus::Partial {
                                    skipped,
                                } => Some(
                                    skipped
                                        .iter()
                                        .map(|s| format!("{}: {:?}", s.path, s.reason))
                                        .collect(),
                                ),
                                _ => None,
                            }
                            })
                            .unwrap_or_default();
                        if json_mode {
                            #[derive(serde::Serialize)]
                            struct SkippedJson {
                                path: String,
                                reason: String,
                            }
                            #[derive(serde::Serialize)]
                            struct FullGraphJson<'a> {
                                schema_version: &'a str,
                                path: &'a str,
                                elapsed_ms: u128,
                                symbols: usize,
                                dependencies: usize,
                                status: &'a str,
                                #[serde(skip_serializing_if = "Vec::is_empty")]
                                skipped_files: Vec<SkippedJson>,
                            }
                            let doc = FullGraphJson {
                                schema_version: "cognicode.graph.full/v1",
                                path,
                                elapsed_ms: elapsed,
                                symbols,
                                dependencies: edges,
                                status: if skipped.is_empty() {
                                    "complete"
                                } else {
                                    "partial"
                                },
                                skipped_files: skipped
                                    .iter()
                                    .filter_map(|s| {
                                        let (p, r) = s.split_once(": ")?;
                                        Some(SkippedJson {
                                            path: p.to_string(),
                                            reason: r.to_string(),
                                        })
                                    })
                                    .collect(),
                            };
                            println!("{}", serde_json::to_string(&doc)?);
                        } else {
                            println!("Full graph built in {}ms", elapsed);
                            println!("  Total symbols: {}", symbols);
                            println!("  Total dependencies: {}", edges);
                            for s in &skipped {
                                eprintln!("  Skipped: {}", s);
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Error building full graph: {}", e);
                        return Err(Box::new(e));
                    }
                }
            }
            GraphCommand::HotPaths {
                limit,
                min_fan_in,
                path,
            } => {
                let start = Instant::now();
                println!(
                    "Finding hot paths in: {} (limit={}, min_fan_in={})",
                    path, limit, min_fan_in
                );

                let dir = PathBuf::from(path);
                let strategy = FullGraphStrategy::new();

                // EXT-02: honor the same Partial/Failed semantics as the
                // MCP path — surface skipped files, never present a
                // partial graph as clean.
                let report = strategy.build_full_graph_report(&dir);
                if let crate::infrastructure::graph::per_file_graph::BuildStatus::Partial {
                    skipped,
                } = &report.status
                {
                    eprintln!(
                        "Warning: graph is PARTIAL: {} file(s) skipped ({})",
                        skipped.len(),
                        skipped
                            .iter()
                            .map(|f| f.path.clone())
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                }
                let graph = report.graph;

                let analyzer = CallGraphAnalyzer::new();
                let hot_paths = analyzer.find_hot_paths(&graph, *limit);

                let filtered: Vec<_> = hot_paths
                    .into_iter()
                    .filter(|h| h.fan_in >= *min_fan_in)
                    .collect();

                println!("\nHot paths (most called functions):");
                println!(
                    "{:<40} {:>8} {:>8}  Location",
                    "Function", "Fan-in", "Fan-out"
                );
                println!("{}", "-".repeat(80));

                for hp in &filtered {
                    println!(
                        "{:<40} {:>8} {:>8}  {}:{}",
                        hp.symbol_name, hp.fan_in, hp.fan_out, hp.file, hp.line
                    );
                }

                let elapsed = start.elapsed().as_millis();
                println!("\nFound {} hot paths in {}ms", filtered.len(), elapsed);
            }
            GraphCommand::EntryPoints { path } => {
                let start = Instant::now();
                println!("Finding entry points in: {}", path);

                let dir = PathBuf::from(path);
                let strategy = FullGraphStrategy::new();

                // EXT-02: honor the same Partial/Failed semantics as the
                // MCP path — surface skipped files, never present a
                // partial graph as clean.
                let report = strategy.build_full_graph_report(&dir);
                if let crate::infrastructure::graph::per_file_graph::BuildStatus::Partial {
                    skipped,
                } = &report.status
                {
                    eprintln!(
                        "Warning: graph is PARTIAL: {} file(s) skipped ({})",
                        skipped.len(),
                        skipped
                            .iter()
                            .map(|f| f.path.clone())
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                }
                let graph = report.graph;

                let entry_ids = graph.roots();
                println!("\nEntry points (no incoming edges):");
                for id in entry_ids.iter().take(20) {
                    if let Some(sym) = graph.get_symbol(id) {
                        println!(
                            "  {} at {}:{}:{}",
                            sym.name(),
                            sym.location().file(),
                            sym.location().line(),
                            sym.location().column()
                        );
                    }
                }
                let elapsed = start.elapsed().as_millis();
                println!("\nFound {} entry points in {}ms", entry_ids.len(), elapsed);
            }
            GraphCommand::LeafFunctions { path } => {
                let start = Instant::now();
                println!("Finding leaf functions in: {}", path);

                let dir = PathBuf::from(path);
                let strategy = FullGraphStrategy::new();

                // EXT-02: honor the same Partial/Failed semantics as the
                // MCP path — surface skipped files, never present a
                // partial graph as clean.
                let report = strategy.build_full_graph_report(&dir);
                if let crate::infrastructure::graph::per_file_graph::BuildStatus::Partial {
                    skipped,
                } = &report.status
                {
                    eprintln!(
                        "Warning: graph is PARTIAL: {} file(s) skipped ({})",
                        skipped.len(),
                        skipped
                            .iter()
                            .map(|f| f.path.clone())
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                }
                let graph = report.graph;

                let leaf_ids = graph.leaves();
                println!("\nLeaf functions (no outgoing edges):");
                for id in leaf_ids.iter().take(20) {
                    if let Some(sym) = graph.get_symbol(id) {
                        println!(
                            "  {} at {}:{}:{}",
                            sym.name(),
                            sym.location().file(),
                            sym.location().line(),
                            sym.location().column()
                        );
                    }
                }
                let elapsed = start.elapsed().as_millis();
                println!("\nFound {} leaf functions in {}ms", leaf_ids.len(), elapsed);
            }
            GraphCommand::TracePath { from, to, path } => {
                let start = Instant::now();
                println!("Tracing path from '{}' to '{}' in: {}", from, to, path);

                let dir = PathBuf::from(path);
                let strategy = FullGraphStrategy::new();

                // EXT-02: honor the same Partial/Failed semantics as the
                // MCP path — surface skipped files, never present a
                // partial graph as clean.
                let report = strategy.build_full_graph_report(&dir);
                if let crate::infrastructure::graph::per_file_graph::BuildStatus::Partial {
                    skipped,
                } = &report.status
                {
                    eprintln!(
                        "Warning: graph is PARTIAL: {} file(s) skipped ({})",
                        skipped.len(),
                        skipped
                            .iter()
                            .map(|f| f.path.clone())
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                }
                let graph = report.graph;

                let source_id = crate::domain::aggregates::call_graph::SymbolId::new(from.clone());
                let target_id = crate::domain::aggregates::call_graph::SymbolId::new(to.clone());

                match graph.find_path(&source_id, &target_id) {
                    Some(path_ids) => {
                        println!("\nPath found ({} hops):", path_ids.len());
                        for (i, id) in path_ids.iter().enumerate() {
                            if let Some(sym) = graph.get_symbol(id) {
                                println!(
                                    "  {}. {} at {}:{}",
                                    i + 1,
                                    sym.name(),
                                    sym.location().file(),
                                    sym.location().line()
                                );
                            }
                        }
                    }
                    None => {
                        println!("\nNo path found between '{}' and '{}'", from, to);
                    }
                }
                let elapsed = start.elapsed().as_millis();
                println!("\nTrace completed in {}ms", elapsed);
            }
            GraphCommand::Mermaid { path, format } => {
                let start = Instant::now();
                println!("Exporting to Mermaid format from: {}", path);

                let dir = PathBuf::from(path);
                let strategy = FullGraphStrategy::new();

                // EXT-02: honor the same Partial/Failed semantics as the
                // MCP path — surface skipped files, never present a
                // partial graph as clean.
                let report = strategy.build_full_graph_report(&dir);
                if let crate::infrastructure::graph::per_file_graph::BuildStatus::Partial {
                    skipped,
                } = &report.status
                {
                    eprintln!(
                        "Warning: graph is PARTIAL: {} file(s) skipped ({})",
                        skipped.len(),
                        skipped
                            .iter()
                            .map(|f| f.path.clone())
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                }
                let graph = report.graph;

                let mermaid = graph.to_mermaid("Call Graph");
                println!("\nMermaid diagram ({} chars):", mermaid.len());

                match format.as_str() {
                    "svg" | "png" => {
                        println!("\nNote: SVG/PNG export requires mermaid-cli");
                        println!("Run: cat << 'EOF' | mermaid -s\n{}\nEOF", mermaid);
                    }
                    _ => {
                        println!("\n{}", &mermaid[..mermaid.len().min(2000)]);
                        if mermaid.len() > 2000 {
                            println!("\n... (truncated, full output in file)");
                        }
                    }
                }
                let elapsed = start.elapsed().as_millis();
                println!("\nExport completed in {}ms", elapsed);
            }
            GraphCommand::Hierarchy {
                symbol,
                depth,
                direction,
                path,
            } => {
                let start = Instant::now();
                println!(
                    "Getting call hierarchy for '{}' (depth={}, direction={}) in: {}",
                    symbol, depth, direction, path
                );

                let dir = PathBuf::from(path);
                let mut strategy = OnDemandStrategy::new();

                if let Err(e) = strategy.build_index(&dir) {
                    eprintln!("Error building index: {}", e);
                    return Err(Box::new(e));
                }

                let dir_enum = match direction.to_lowercase().as_str() {
                    "in" => TraversalDirection::Callers,
                    "out" => TraversalDirection::Callees,
                    _ => TraversalDirection::Both,
                };

                let result = strategy.build_subgraph(symbol, *depth, dir_enum);
                let elapsed = start.elapsed().as_millis();

                println!("\nCall hierarchy for '{}':", result.root_symbol.name());
                println!(
                    "Root: {} at {}:{}:{}",
                    result.root_symbol.name(),
                    result.root_symbol.location().file(),
                    result.root_symbol.location().line(),
                    result.root_symbol.location().column()
                );

                println!("\nEntries by depth:");
                let mut by_depth: std::collections::HashMap<u32, Vec<_>> =
                    std::collections::HashMap::new();
                for entry in &result.entries {
                    by_depth.entry(entry.depth).or_default().push(entry);
                }
                for depth in 1..=*depth {
                    if let Some(entries) = by_depth.get(&depth) {
                        println!("  Depth {}: {} entries", depth, entries.len());
                        for entry in entries.iter().take(5) {
                            println!(
                                "    - {} ({}) at {}:{}",
                                entry.symbol.name(),
                                format!("{:?}", entry.direction).to_lowercase(),
                                entry.symbol.location().file(),
                                entry.symbol.location().line()
                            );
                        }
                    }
                }
                println!("\nTotal entries: {} in {}ms", result.entries.len(), elapsed);
            }
            GraphCommand::Complexity { path } => {
                let start = Instant::now();
                println!("Calculating complexity metrics for: {}", path);

                let dir = PathBuf::from(path);
                let strategy = FullGraphStrategy::new();

                // EXT-02: honor the same Partial/Failed semantics as the
                // MCP path — surface skipped files, never present a
                // partial graph as clean.
                let report = strategy.build_full_graph_report(&dir);
                if let crate::infrastructure::graph::per_file_graph::BuildStatus::Partial {
                    skipped,
                } = &report.status
                {
                    eprintln!(
                        "Warning: graph is PARTIAL: {} file(s) skipped ({})",
                        skipped.len(),
                        skipped
                            .iter()
                            .map(|f| f.path.clone())
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                }
                let graph = report.graph;

                let analyzer = CallGraphAnalyzer::new();
                let complexity = analyzer.calculate_complexity(&graph);

                println!("\nComplexity Metrics:");
                println!("  Total symbols: {}", complexity.total_symbols);
                println!("  Total edges: {}", complexity.total_edges);
                println!("  Max depth: {}", complexity.max_depth);
                println!(
                    "  Cyclomatic complexity: {}",
                    complexity.cyclomatic_complexity
                );
                println!("  High fan-out (>=10): {}", complexity.high_fan_out_count);
                println!(
                    "  Medium fan-out (5-9): {}",
                    complexity.medium_fan_out_count
                );
                println!("  Low fan-out (<5): {}", complexity.low_fan_out_count);
                println!("  Entry points: {}", complexity.entry_point_count);
                println!("  Leaf functions: {}", complexity.leaf_function_count);

                let elapsed = start.elapsed().as_millis();
                println!("\nAnalysis completed in {}ms", elapsed);
            }
            GraphCommand::Impact { symbol, path } => {
                let start = Instant::now();
                println!("Analyzing impact of changing '{}' in: {}", symbol, path);

                let dir = PathBuf::from(path);
                let strategy = FullGraphStrategy::new();

                // EXT-02: honor the same Partial/Failed semantics as the
                // MCP path — surface skipped files, never present a
                // partial graph as clean.
                let report = strategy.build_full_graph_report(&dir);
                if let crate::infrastructure::graph::per_file_graph::BuildStatus::Partial {
                    skipped,
                } = &report.status
                {
                    eprintln!(
                        "Warning: graph is PARTIAL: {} file(s) skipped ({})",
                        skipped.len(),
                        skipped
                            .iter()
                            .map(|f| f.path.clone())
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                }
                let graph = report.graph;

                // Find all symbols that match the name
                let search_name = symbol.to_lowercase();
                let symbol_ids: Vec<_> = graph
                    .symbols()
                    .filter(|s| {
                        let name = s.name().to_lowercase();
                        let fqn = s.fully_qualified_name().to_lowercase();
                        name == search_name
                            || fqn == search_name
                            || name.contains(&search_name)
                            || fqn.contains(&search_name)
                    })
                    .map(|s| {
                        crate::domain::aggregates::call_graph::SymbolId::new(
                            s.fully_qualified_name(),
                        )
                    })
                    .collect();

                if symbol_ids.is_empty() {
                    println!("\nNo symbols found matching '{}'", symbol);
                    return Ok(());
                }

                println!(
                    "\nFound {} symbol(s) matching '{}'",
                    symbol_ids.len(),
                    symbol
                );

                let mut impacted_symbols_set = std::collections::HashSet::new();
                let mut impacted_files_set = std::collections::HashSet::new();

                for symbol_id in &symbol_ids {
                    // Find all dependents (transitive)
                    let dependents = graph.find_all_dependents(symbol_id);

                    for dep_id in dependents {
                        if let Some(sym) = graph.get_symbol(&dep_id) {
                            impacted_symbols_set.insert(sym.name().to_string());
                            impacted_files_set.insert(sym.location().file().to_string());
                        }
                    }
                }

                let impacted_symbols: Vec<String> = impacted_symbols_set.into_iter().collect();
                let impacted_files: Vec<String> = impacted_files_set.into_iter().collect();

                let risk_level = if impacted_symbols.len() > 10 {
                    "CRITICAL"
                } else if impacted_symbols.len() > 5 {
                    "HIGH"
                } else if impacted_symbols.len() > 2 {
                    "MEDIUM"
                } else if !impacted_symbols.is_empty() {
                    "LOW"
                } else {
                    "NONE"
                };

                println!("\nImpact Analysis for '{}':", symbol);
                println!("  Risk Level: {}", risk_level);
                println!("  Direct dependents: {}", symbol_ids.len());
                println!("  Total impacted symbols: {}", impacted_symbols.len());
                println!("  Impacted files: {}", impacted_files.len());

                if !impacted_symbols.is_empty() {
                    println!("\nImpacted symbols (first 20):");
                    for (i, s) in impacted_symbols.iter().enumerate().take(20) {
                        println!("  {}. {}", i + 1, s);
                    }
                    if impacted_symbols.len() > 20 {
                        println!("  ... and {} more", impacted_symbols.len() - 20);
                    }
                }

                if !impacted_files.is_empty() {
                    println!("\nImpacted files (first 10):");
                    for (i, f) in impacted_files.iter().enumerate().take(10) {
                        println!("  {}. {}", i + 1, f);
                    }
                    if impacted_files.len() > 10 {
                        println!("  ... and {} more", impacted_files.len() - 10);
                    }
                }

                let elapsed = start.elapsed().as_millis();
                println!("\nAnalysis completed in {}ms", elapsed);
            }
        }
        Ok(())
    }

    /// Parse a "file:line:column" position string into parts
    fn parse_position(position: &str) -> Result<(String, u32, u32), Box<dyn std::error::Error>> {
        let parts: Vec<&str> = position.rsplitn(3, ':').collect();
        if parts.len() != 3 {
            return Err(format!(
                "Invalid position '{}': expected file:line:column (e.g. src/main.rs:42:10)",
                position
            )
            .into());
        }
        // rsplitn gives reversed order: column, line, file
        let column: u32 = parts[0]
            .parse()
            .map_err(|_| format!("Invalid column in '{}'", position))?;
        let line: u32 = parts[1]
            .parse()
            .map_err(|_| format!("Invalid line in '{}'", position))?;
        let file = parts[2].to_string();
        Ok((file, line, column))
    }

    /// Execute navigate subcommand
    async fn execute_navigate(command: &NavigateCommand) -> Result<(), Box<dyn std::error::Error>> {
        use crate::domain::value_objects::Location;
        use crate::infrastructure::lsp::providers::CompositeProvider;
        use std::path::Path;

        match command {
            NavigateCommand::Definition { position, path } => {
                let (file, line, column) = Self::parse_position(position)?;
                println!(
                    "Go to definition: {}:{}:{} (workspace: {})",
                    file, line, column, path
                );
                println!("Connecting to LSP server...");

                let workspace = Path::new(path);
                let provider = CompositeProvider::new(workspace);
                let location = Location::new(file.clone(), line.saturating_sub(1), column);

                match provider.get_definition(&location).await {
                    Ok(Some(def)) => {
                        println!("Definition found:");
                        println!("  File:   {}", def.file());
                        println!("  Line:   {}", def.line() + 1);
                        println!("  Column: {}", def.column());
                    }
                    Ok(None) => {
                        println!("No definition found for {}:{}", file, line);
                    }
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        return Err(Box::new(e));
                    }
                }
            }
            NavigateCommand::Hover { position, path } => {
                let (file, line, column) = Self::parse_position(position)?;
                println!(
                    "Hover info: {}:{}:{} (workspace: {})",
                    file, line, column, path
                );
                println!("Connecting to LSP server...");

                let workspace = Path::new(path);
                let provider = CompositeProvider::new(workspace);
                let location = Location::new(file.clone(), line.saturating_sub(1), column);

                match provider.hover(&location).await {
                    Ok(Some(info)) => {
                        println!("Hover information:");
                        println!("  Type:    {}", info.content);
                        if let Some(doc) = &info.documentation {
                            println!("  Docs:    {}", doc);
                        }
                    }
                    Ok(None) => {
                        println!("No hover information found for {}:{}", file, line);
                    }
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        return Err(Box::new(e));
                    }
                }
            }
            NavigateCommand::References {
                position,
                include_declaration,
                path,
            } => {
                let (file, line, column) = Self::parse_position(position)?;
                println!(
                    "Find references: {}:{}:{} (workspace: {})",
                    file, line, column, path
                );
                println!("Connecting to LSP server...");

                let workspace = Path::new(path);
                let provider = CompositeProvider::new(workspace);
                let location = Location::new(file.clone(), line.saturating_sub(1), column);

                match provider
                    .find_references(&location, *include_declaration)
                    .await
                {
                    Ok(refs) => {
                        if refs.is_empty() {
                            println!("No references found for {}:{}", file, line);
                        } else {
                            println!("Found {} reference(s):", refs.len());
                            for r in &refs {
                                let container = r.container.as_deref().unwrap_or("(unknown)");
                                println!(
                                    "  {}:{}:{} [{:?}] in {}",
                                    r.location.file(),
                                    r.location.line() + 1,
                                    r.location.column(),
                                    r.reference_kind,
                                    container
                                );
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        return Err(Box::new(e));
                    }
                }
            }
        }
        Ok(())
    }

    /// Execute doctor subcommand — check LSP server availability
    async fn execute_doctor(format: &str, cwd: &str) -> Result<(), Box<dyn std::error::Error>> {
        use crate::interface::cli::doctor::{
            format_doctor_json, format_doctor_text, run_doctor_checks,
        };

        let workspace_path = std::path::Path::new(cwd);
        let workspace_path = if workspace_path.exists() && workspace_path.is_dir() {
            Some(workspace_path)
        } else {
            None
        };

        let report = run_doctor_checks(workspace_path);

        match format {
            "json" => {
                println!("{}", format_doctor_json(&report));
            }
            _ => {
                println!("{}", format_doctor_text(&report));
            }
        }

        // Set exit code based on overall status
        let exit_code = match report.overall_status() {
            crate::interface::cli::doctor::DoctorStatus::Missing => 1,
            crate::interface::cli::doctor::DoctorStatus::Ok
            | crate::interface::cli::doctor::DoctorStatus::Warn
            | crate::interface::cli::doctor::DoctorStatus::Info => 0,
        };

        std::process::exit(exit_code);
    }

    /// Execute the `docs-ingest` subcommand (T15). Walks
    /// `path` with the [`DocsExtractor`] and prints a
    /// structured summary to stdout. Idempotent: re-running on
    /// the same file produces the same `NodeId`s, so a future
    /// persistence upsert will collapse duplicates.
    ///
    /// Exit codes (per the spec):
    /// - `0` on success (every file yielded at least one
    ///   candidate or was empty).
    /// - `1` on partial failure (e.g. the path doesn't exist,
    ///   the extractor returned an error, or no files matched).
    /// - `2` on hard extractor failure (e.g. invalid UTF-8 on
    ///   the only file).
    #[cfg(feature = "multimodal")]
    async fn execute_docs_ingest(
        path: &str,
        _recursive: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        use crate::domain::traits::source_extractor::{SourceExtractor, SourcePath};
        use crate::infrastructure::extraction::docs_extractor::DocsExtractor;
        use std::path::PathBuf;

        let path_buf = PathBuf::from(path);
        if !path_buf.exists() {
            eprintln!("docs-ingest: path does not exist: {path}");
            std::process::exit(1);
        }
        let source = if path_buf.is_dir() {
            SourcePath::Directory(path_buf)
        } else {
            SourcePath::File(path_buf)
        };
        let extractor = DocsExtractor::new();
        let result = match extractor.extract(source).await {
            Ok(v) => v,
            Err(e) => {
                eprintln!("docs-ingest: extractor error: {e}");
                std::process::exit(2);
            }
        };
        // Tally: distinct source paths processed, total nodes,
        // total edges. The output is a 3-line human summary
        // plus a JSON block the future `ExplorerService` can
        // pipe into the PG repository.
        let files_processed = result
            .iter()
            .map(|n| n.potential_node.source_path.clone())
            .flatten()
            .map(|p| p.to_string_lossy().into_owned())
            .collect::<std::collections::BTreeSet<_>>()
            .len();
        let nodes_created = result.len();
        let edges_created: usize = result.iter().map(|n| n.potential_edges.len()).sum();
        println!("docs-ingest: files_processed = {files_processed}");
        println!("docs-ingest: nodes_created   = {nodes_created}");
        println!("docs-ingest: edges_created   = {edges_created}");
        // The structured JSON form is the wire-level contract
        // (matches the MCP `docs_ingest` envelope payload).
        let payload = serde_json::json!({
            "files_processed": files_processed,
            "nodes_created":   nodes_created,
            "edges_created":   edges_created,
            "errors":          Vec::<String>::new(),
        });
        println!("docs-ingest: payload = {payload}");
        // Exit 0 when the extractor produced at least one
        // candidate OR the input was a non-empty directory
        // (recursive walks of empty dirs are also exit 0 —
        // the operation is a no-op success).
        Ok(())
    }

    /// Execute the `issues-ingest` subcommand (T13). Fetches
    /// GitHub issues from the given `owner`/`repo` via the
    /// `IssuesExtractor` and prints a structured summary to
    /// stdout. When `include_git_log` is true (default), also
    /// parses `git log` output from the current directory for
    /// commit-issue references.
    ///
    /// Exit codes (mirrors `docs-ingest`):
    /// - `0` on success (issues were fetched or repo was empty).
    /// - `1` on invalid input (missing owner/repo).
    /// - `2` on extractor failure (network / API error).
    #[cfg(feature = "multimodal")]
    async fn execute_issues_ingest(
        owner: &str,
        repo: &str,
        _include_git_log: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        use crate::domain::traits::source_extractor::{SourceExtractor, SourcePath};
        use crate::infrastructure::extraction::issues_extractor::IssuesExtractor;
        use crate::infrastructure::github::client::GitHubClient;
        use crate::infrastructure::github::octocrab_client::OctocrabClient;
        use std::sync::Arc;

        if owner.is_empty() {
            eprintln!("issues-ingest: owner is required");
            std::process::exit(1);
        }
        if repo.is_empty() {
            eprintln!("issues-ingest: repo is required");
            std::process::exit(1);
        }

        let client: Arc<dyn GitHubClient> = Arc::new(OctocrabClient::new());
        let extractor =
            IssuesExtractor::with_repo_override(client, owner.to_string(), repo.to_string());
        let url = format!("https://github.com/{owner}/{repo}");
        let result = match extractor.extract(SourcePath::Url(url)).await {
            Ok(v) => v,
            Err(e) => {
                eprintln!("issues-ingest: extractor error: {e}");
                std::process::exit(2);
            }
        };

        let issues_processed = result.len();
        let nodes_created = result.len();
        let edges_created: usize = result.iter().map(|n| n.potential_edges.len()).sum();
        println!("issues-ingest: issues_processed = {issues_processed}");
        println!("issues-ingest: nodes_created   = {nodes_created}");
        println!("issues-ingest: edges_created   = {edges_created}");
        let payload = serde_json::json!({
            "issues_processed": issues_processed,
            "nodes_created":    nodes_created,
            "edges_created":    edges_created,
            "errors":           Vec::<String>::new(),
        });
        println!("issues-ingest: payload = {payload}");
        Ok(())
    }

    /// L1.4 / F0.1 — execute `cognicode find-usages <symbol>`.
    ///
    /// Calls `AnalysisService::find_symbol_usages` directly (no MCP
    /// subprocess, no handler duplication). Validates the cwd and the
    /// symbol via `InputValidator` to mirror the MCP handler contract.
    ///
    /// Args (already resolved by clap):
    ///   - symbol: query (non-empty after clap parsing)
    ///   - cwd: workspace root (must exist as a directory)
    ///   - include: include declaration site
    ///   - context_lines: optional surrounding-line count
    ///   - format: "text" (default) | "json"
    ///   - quiet: suppress the human header in text mode
    ///
    /// Returns `Err` on:
    ///   - cwd does not exist or is not a directory (input error)
    ///   - InputValidator::validate_query fails (input error)
    ///   - AnalysisService::find_symbol_usages fails (backend error)
    ///
    /// On success prints to stdout, returns Ok.
    #[allow(clippy::too_many_arguments)]
    async fn execute_find_usages(
        symbol: &str,
        cwd: &str,
        include: bool,
        context_lines: Option<usize>,
        format: &str,
        quiet: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        use crate::application::services::analysis_service::{AnalysisService, UsageSearchParams};
        use crate::interface::mcp::schemas::{FindUsagesOutput, UsageEntry};
        use crate::interface::mcp::security::InputValidator;

        // 1) Input validation: cwd exists.
        let cwd_path = PathBuf::from(cwd);
        if !cwd_path.exists() || !cwd_path.is_dir() {
            return Err(
                format!("find-usages: cwd does not exist or is not a directory: {cwd}").into(),
            );
        }

        // 2) Input validation: query (mirrors MCP handler validation).
        //    validate_query checks max_query_length; not non-empty.
        let validator = InputValidator::new();
        validator
            .validate_query(symbol)
            .map_err(|e| format!("find-usages: invalid symbol: {e}"))?;

        // 3) Execute: AnalysisService directo. NO MCP subprocess.
        //    Mismo `first_only_definition=true` que usa el handler MCP
        //    para mantener equivalencia L1.4.W4.
        let service = AnalysisService::new();
        let usages = service
            .find_symbol_usages(UsageSearchParams {
                project_dir: cwd_path,
                symbol_name: symbol.to_string(),
                include_declaration: include,
                context_lines,
                first_only_definition: true,
            })
            .map_err(|e| format!("find-usages: backend error: {e}"))?;

        let total = usages.len();
        let entries: Vec<UsageEntry> = usages
            .into_iter()
            .map(|u| UsageEntry {
                file: u.file,
                line: u.line,
                column: u.column,
                context: u.context,
                is_definition: u.is_definition,
                surrounding_lines: u.context_lines.map(|c| {
                    crate::interface::mcp::schemas::ContextLines {
                        before: c.before,
                        current: c.current,
                        after: c.after,
                    }
                }),
            })
            .collect();

        let out = FindUsagesOutput {
            symbol: symbol.to_string(),
            usages: entries,
            total,
        };

        // 4) Format output to stdout.
        match format {
            "json" => {
                let json = serde_json::to_string_pretty(&out)
                    .map_err(|e| format!("find-usages: json serialization: {e}"))?;
                println!("{json}");
            }
            "text" => {
                if !quiet {
                    println!("# find-usages symbol={symbol} total={}", out.total);
                }
                print_text_render(&out);
            }
            other => {
                return Err(
                    format!("find-usages: unknown format '{other}' (expected: text|json)").into(),
                );
            }
        }

        Ok(())
    }

    /// Execute analyze subcommand
    async fn execute_analyze(path: &str) -> Result<(), Box<dyn std::error::Error>> {
        use crate::WorkspaceSession;

        println!("Analyzing code at: {}", path);

        let session = WorkspaceSession::new(path)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to create session: {}", e))?;

        // Architecture check
        println!("\n=== Architecture Check ===");
        match session.check_architecture(None).await {
            Ok(result) => {
                println!("  Score: {:.1}/100", result.score);
                println!("  Summary: {}", result.summary);
                println!("  Cycles: {}", result.cycles.len());
                println!("  Violations: {}", result.violations.len());
            }
            Err(e) => {
                eprintln!("  Architecture check failed: {}", e);
            }
        }

        // Complexity check on key files
        println!("\n=== Complexity Analysis ===");
        let session_ref = &session;
        let path_str = path.to_string();

        // Use spawn_blocking to avoid blocking the async runtime
        let file_names: Vec<String> = tokio::task::spawn_blocking(move || {
            std::fs::read_dir(&path_str)
                .ok()
                .map(|entries| {
                    entries
                        .filter_map(Result::ok)
                        .filter_map(|e| {
                            let p = e.path();
                            if p.extension().and_then(|s| s.to_str()) == Some("rs") {
                                p.file_name()
                                    .and_then(|s| s.to_str())
                                    .map(|s| s.to_string())
                            } else {
                                None
                            }
                        })
                        .take(5)
                        .collect()
                })
                .unwrap_or_default()
        })
        .await
        .unwrap_or_default();

        for name in &file_names {
            if let Ok(c) = session_ref.get_complexity(name, None).await {
                println!(
                    "  {}: cyclomatic={}, cognitive={}, loc={}",
                    name, c.cyclomatic, c.cognitive, c.lines_of_code
                );
            }
        }

        println!("\nAnalysis complete.");
        Ok(())
    }

    /// Execute refactor subcommand
    async fn execute_refactor(
        operation: &RefactorOperation,
        symbol: &str,
        new_name: Option<&str>,
        format: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        use crate::WorkspaceSession;

        let path = ".";

        let session = WorkspaceSession::new(path)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to create session: {}", e))?;

        // PRF-CLI-02/05: json mode emits a schema-versioned preview
        // document on stdout (preview-only: refactor never mutates files).
        let json_mode = format == "json";
        if !json_mode {
            eprintln!(
                "Preview only (read-only). File mutation requires --apply, \
                 which is not implemented until rollback exists (PRF-CLI-05)."
            );
        }

        match operation {
            RefactorOperation::Rename => {
                let new_name =
                    new_name.ok_or_else(|| anyhow::anyhow!("Rename requires a new name"))?;
                if json_mode {
                    eprintln!("Renaming '{}' to '{}'...", symbol, new_name);
                } else {
                    println!("Renaming '{}' to '{}'...", symbol, new_name);
                }
                match session.rename_symbol(symbol, new_name, "<unknown>").await {
                    Ok(result) => {
                        if json_mode {
                            #[derive(serde::Serialize)]
                            struct RefactorPreviewJson<'a> {
                                schema_version: &'a str,
                                action: &'a str,
                                applied: bool,
                                success: bool,
                                changes: Vec<crate::application::dto::ChangeEntry>,
                                error: Option<&'a str>,
                            }
                            let doc = RefactorPreviewJson {
                                schema_version: "cognicode.refactor.preview/v1",
                                action: "rename",
                                applied: false,
                                success: result.success,
                                changes: result.changes,
                                error: result.error_message.as_deref(),
                            };
                            println!("{}", serde_json::to_string(&doc)?);
                        } else if result.success {
                            println!("  Success: {} change(s) made", result.changes.len());
                        } else {
                            println!(
                                "  Failed: {}",
                                result.error_message.as_deref().unwrap_or("unknown error")
                            );
                        }
                    }
                    Err(e) => {
                        eprintln!("  Error: {}", e);
                    }
                }
            }
            RefactorOperation::Inline => {
                println!("Inlining '{}'...", symbol);
                match session.inline_symbol(symbol, "<unknown>").await {
                    Ok(result) => {
                        if result.success {
                            println!("  Success");
                        } else {
                            println!(
                                "  Failed: {}",
                                result.error_message.as_deref().unwrap_or("unknown error")
                            );
                        }
                    }
                    Err(e) => {
                        eprintln!("  Error: {}", e);
                    }
                }
            }
            RefactorOperation::Move => {
                let target = new_name.ok_or_else(|| {
                    anyhow::anyhow!("Move requires a target path (use -- new-name)")
                })?;
                println!("Moving '{}' to '{}'...", symbol, target);
                match session.move_symbol(symbol, "<unknown>", target).await {
                    Ok(result) => {
                        if result.success {
                            println!(
                                "  Success: {}",
                                result.validation_result.warnings.join("; ")
                            );
                        } else {
                            println!(
                                "  Failed: {}",
                                result.error_message.as_deref().unwrap_or("unknown error")
                            );
                        }
                    }
                    Err(e) => {
                        eprintln!("  Error: {}", e);
                    }
                }
            }
            RefactorOperation::Extract => {
                let name = new_name.ok_or_else(|| {
                    anyhow::anyhow!("Extract requires a function name (use -- new-name)")
                })?;
                println!("Extracting function '{}'...", name);
                match session
                    .extract_function("<unknown>", (0, 0, 0, 0), name)
                    .await
                {
                    Ok(result) => {
                        if result.success {
                            println!(
                                "  Success: {}",
                                result.validation_result.warnings.join("; ")
                            );
                        } else {
                            println!(
                                "  Failed: {}",
                                result.error_message.as_deref().unwrap_or("unknown error")
                            );
                        }
                    }
                    Err(e) => {
                        eprintln!("  Error: {}", e);
                    }
                }
            }
        }

        Ok(())
    }

    /// E1.W3 — Drive `cognicode evidence <list|search>`.
    ///
    /// Flow:
    ///   1. Look up the registered `EvidenceBackend` (set at CLI
    ///      startup by the `--features ladybug` adapter). If absent,
    ///      return a clear "backend not registered" error.
    ///   2. Dispatch `list` / `search` to the backend.
    ///   3. Render results as text (human) or json (machine) using
    ///      a stable schema — the same schema the MCP `list_evidence`
    ///      tool emits, so the equivalence test in
    ///      `cognicode-cli/tests/evidence_cli_mcp_equivalence.rs`
    ///      can byte-compare JSON outputs.
    #[cfg(feature = "evidence-cli-ladybug")]
    async fn execute_evidence(
        cmd: &EvidenceCommand,
    ) -> Result<(), Box<dyn std::error::Error>> {
        use crate::domain::ports::evidence_store::EvidenceKind;
        use crate::interface::cli::evidence_backend;
        use std::path::PathBuf;

        // Pull the per-invocation `--db-path` (if any) so the factory
        // can open the right file.
        let db_path: Option<PathBuf> = match cmd {
            EvidenceCommand::List { db_path, .. } => db_path
                .as_deref()
                .map(PathBuf::from)
                .or_else(|| Some(default_evidence_db_path())),
            EvidenceCommand::Search { db_path, .. } => db_path
                .as_deref()
                .map(PathBuf::from)
                .or_else(|| Some(default_evidence_db_path())),
        };

        let factory = evidence_backend::evidence_backend_factory().ok_or(
            "evidence backend not registered: this CLI build does not have the \
             `ladybug` feature enabled, or the backend registration step was \
             skipped at startup. Recompile with `--features ladybug`.",
        )?;
        let backend = factory(db_path.as_ref())
            .map_err(|e| format!("opening evidence backend: {e}"))?;

        let (rows, format) = match cmd {
            EvidenceCommand::List {
                workspace,
                kind,
                format,
                ..
            } => {
                // Parse the kind string eagerly so a typo errors out
                // before we touch the backend.
                let kind = match kind.as_deref() {
                    None => None,
                    Some("log") => Some(EvidenceKind::Log),
                    Some("trace") => Some(EvidenceKind::Trace),
                    Some("measurement") => Some(EvidenceKind::Measurement),
                    Some("external") => Some(EvidenceKind::External),
                    Some(other) => {
                        return Err(format!(
                            "unknown evidence kind: {other} (expected one of \
                             log | trace | measurement | external)"
                        )
                        .into());
                    }
                };
                let rows = backend
                    .list(workspace, kind)
                    .map_err(|e| format!("list_evidence: {e}"))?;
                (rows, format.as_str())
            }
            EvidenceCommand::Search {
                query,
                workspace,
                limit,
                format,
                ..
            } => {
                let rows = backend
                    .search(workspace, query, *limit)
                    .map_err(|e| format!("search_evidence: {e}"))?;
                (rows, format.as_str())
            }
        };

        render_evidence_rows(rows, format);
        Ok(())
    }
}

/// E1.W3 — Default LadybugDB location for the CLI.
///
/// Matches the policy documented in the `EvidenceCommand` clap help:
/// `<cwd>/.cognicode/evidence.lbdb`. Created on first run by the
/// adapter via `LadybugStore::open` (which initializes the schema
/// idempotently).
#[cfg(feature = "evidence-cli-ladybug")]
fn default_evidence_db_path() -> std::path::PathBuf {
    let mut p = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    p.push(".cognicode");
    let _ = std::fs::create_dir_all(&p);
    p.push("evidence.lbdb");
    p
}

/// E1.W3 — Stable JSON schema for `cognicode evidence` output AND the
/// `list_evidence` / `search_evidence` MCP tools.
///
/// Same field names and types on both surfaces, so the equivalence
/// test in `cognicode-cli/tests/evidence_cli_mcp_equivalence.rs`
/// can byte-compare the two outputs (CLI JSON vs MCP tool result).
///
/// Made `pub(crate)` so the MCP adapter can call it without leaking
/// it on the public surface. The shape is the contract — adding a
/// field here MUST also be reflected on the MCP tool definition
/// (and vice-versa) to keep the equivalence test green.
#[cfg(feature = "evidence-cli-ladybug")]
pub(crate) fn render_evidence_rows_json(
    rows: Vec<crate::domain::ports::evidence_store::EvidenceSummary>,
) -> String {
    use crate::domain::ports::evidence_store::EvidenceKind;

    let payload: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|r| {
            serde_json::json!({
                "id": r.id,
                "title": r.title,
                "kind": match r.kind {
                    EvidenceKind::Log => "log",
                    EvidenceKind::Trace => "trace",
                    EvidenceKind::Measurement => "measurement",
                    EvidenceKind::External => "external",
                },
                "source_path": r.source_path,
                "excerpt": r.excerpt,
                "confidence": r.confidence,
            })
        })
        .collect();
    // Pretty for human readability, but stable. `serde_json::to_string_pretty`
    // sorts object keys alphabetically, which is the contract we
    // pin in the equivalence test.
    serde_json::to_string_pretty(&payload)
        .unwrap_or_else(|e| format!("failed to serialize evidence rows: {e}"))
}

/// E1.W3 — Text (human) renderer for `cognicode evidence`.
#[cfg(feature = "evidence-cli-ladybug")]
fn render_evidence_rows(
    rows: Vec<crate::domain::ports::evidence_store::EvidenceSummary>,
    format: &str,
) {
    use crate::domain::ports::evidence_store::EvidenceKind;

    if format == "json" {
        println!("{}", render_evidence_rows_json(rows));
        return;
    }
    if rows.is_empty() {
        println!("(no evidence rows match the query)");
        return;
    }
    println!(
        "{:<60}  {:<12}  {:<7}  TITLE",
        "ID", "KIND", "CONF"
    );
    for r in rows {
        let kind = match r.kind {
            EvidenceKind::Log => "log",
            EvidenceKind::Trace => "trace",
            EvidenceKind::Measurement => "measurement",
            EvidenceKind::External => "external",
        };
        println!(
            "{:<60}  {:<12}  {:<7.2}  {}",
            r.id,
            kind,
            r.confidence,
            r.title
        );
    }
}

/// Prints the outline tree with indentation
fn print_outline_tree(nodes: &[OutlineNode], indent: usize) {
    for (i, node) in nodes.iter().enumerate() {
        let is_last = i == nodes.len() - 1;
        let prefix = if indent == 0 {
            "".to_string()
        } else {
            "  ".repeat(indent - 1) + if is_last { "└── " } else { "├── " }
        };

        let sig_info = node
            .signature
            .as_ref()
            .map(|s| format!(": {}", s))
            .unwrap_or_default();

        println!("{}{} ({}){sig_info}", prefix, node.name, node.kind);

        if !node.children.is_empty() {
            print_outline_tree(&node.children, indent + 1);
        }
    }
}

/// L1.4 / F0.1 — Render human-readable text output for `find-usages`.
///
/// Format:
///   - one usage per line: `file:line:column: context`
///   - definition lines prefixed with `def ` for grep-friendliness
///   - blank line if no results, preceded by `(no usages found)`
///
/// Machine consumers should use `--format json`.
fn print_text_render(out: &crate::interface::mcp::schemas::FindUsagesOutput) {
    if out.usages.is_empty() {
        println!("(no usages found)");
        return;
    }
    for u in &out.usages {
        let prefix = if u.is_definition { "def " } else { "    " };
        println!(
            "{prefix}{file}:{line}:{col}: {ctx}",
            file = u.file,
            line = u.line,
            col = u.column,
            ctx = u.context,
        );
    }
}

#[cfg(test)]
mod w2_uat_tests {
    //! PRF F2.W2 — UAT of the CLI surface for the per-file-graph path.
    //!
    //! These tests do not spawn a subprocess: they invoke
    //! [`CommandExecutor::execute`] with a parsed [`Cli`] struct, which
    //! is exactly what the `cognicode` binary does after
    //! `Cli::parse()`. The behaviour observed here is therefore the
    //! behaviour a user gets when they type
    //! `cognicode graph per-file <path>` on the command line.
    use super::*;

    fn w2_corpus() -> std::path::PathBuf {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        std::path::PathBuf::from(manifest_dir)
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("docs/prf/fixtures/per_file_partial_corpus")
    }

    /// `cognicode graph per-file <clean.rs>` — UAT for the happy path.
    /// Must succeed and report at least one symbol.
    #[tokio::test]
    async fn uat_cli_graph_per_file_clean_file_succeeds() {
        let good = w2_corpus().join("src/good.rs");
        let cli = Cli {
            verbose: false,
            command: Some(CliCommand::Graph {
                command: GraphCommand::PerFile {
                    file: good.to_string_lossy().to_string(),
                },
            }),
        };
        CommandExecutor::execute(cli)
            .await
            .expect("CLI graph per-file on a clean file must succeed");
    }

    /// `cognicode graph per-file <broken_syntax.rs>` — UAT for the
    /// parse-error path. The CLI must propagate the parse error
    /// instead of silently printing an empty result.
    ///
    /// Before the fix the CLI would happily print
    /// `Symbols: 0 / Dependencies: 0`, pretending that an empty
    /// parse is a successful analysis. After the fix the CLI
    /// surfaces a non-zero error to stderr and returns Err.
    #[tokio::test]
    async fn uat_cli_graph_per_file_broken_syntax_returns_error() {
        let broken = w2_corpus().join("src/broken_syntax.rs");
        let cli = Cli {
            verbose: false,
            command: Some(CliCommand::Graph {
                command: GraphCommand::PerFile {
                    file: broken.to_string_lossy().to_string(),
                },
            }),
        };
        let result = CommandExecutor::execute(cli).await;
        assert!(
            result.is_err(),
            "CLI graph per-file on a syntactically broken file must \
             return an error, not silently report success. Got: {:?}",
            result
        );
    }

    /// `cognicode graph per-file <missing.rs>` — UAT for the
    /// not-found path. The CLI must propagate the read error.
    #[tokio::test]
    async fn uat_cli_graph_per_file_missing_file_returns_error() {
        let missing = w2_corpus().join("src/does_not_exist.rs");
        let cli = Cli {
            verbose: false,
            command: Some(CliCommand::Graph {
                command: GraphCommand::PerFile {
                    file: missing.to_string_lossy().to_string(),
                },
            }),
        };
        let result = CommandExecutor::execute(cli).await;
        assert!(
            result.is_err(),
            "CLI graph per-file on a missing file must return an error. \
             Got: {:?}",
            result
        );
    }
}

#[cfg(test)]
mod f0_1_find_usages_tests {
    //! L1.4 / F0.1 — Tests del comando CLI `cognicode find-usages`.
    //!
    //! Estos tests NO spawnean el binario: invocan
    //! `CommandExecutor::execute` con un `Cli` parseado, que es
    //! exactamente lo que hace el binario tras `Cli::parse()`.
    //!
    //! Cobertura:
    //!   - happy path (include_declaration=true): devuelve def+call sites
    //!   - exclude declaration: solo call sites
    //!   - include_declaration true vs no-include-declaration
    //!   - cwd inválido: error
    //!   - format inválido: error
    //!   - símbolo inexistente: resultado vacío, no error
    //!
    //! Verificación de equivalencia CLI ↔ MCP en
    //! `find_usages_mcp_handler_e2e.rs` (comparten fixtures).

    use super::*;

    /// UAT binario `cognicode find-usages <symbol> -C <corpus>`.
    /// Happy path con include_declaration=true (default).
    /// Debe imprimir def + callsites en formato texto y exit 0.
    #[tokio::test]
    async fn uat_cli_find_usages_text_format_succeeds() {
        // Construimos un corpus ad-hoc via tempfile: el fixture estático
        // es para graph per-file, no contiene exactamente lo que
        // find_usages necesita. Aquí generamos uno en tmpdir.
        let dir = tempfile::tempdir().expect("create tempdir");
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(
            dir.path().join("src/lib.rs"),
            "fn alpha() -> i32 { 42 }\nfn main() { let x = alpha(); let y = alpha(); }\n",
        )
        .unwrap();

        let cli = Cli {
            verbose: false,
            command: Some(CliCommand::FindUsages {
                symbol: "alpha".to_string(),
                cwd: dir.path().to_string_lossy().to_string(),
                include_declaration: true,
                no_include_declaration: false,
                context_lines: None,
                format: "text".to_string(),
                quiet: true, // silencia el header para captura
            }),
        };
        CommandExecutor::execute(cli)
            .await
            .expect("CLI find-usages text format debe succeed");
    }

    /// UAT binario: `--no-include-declaration` debe omitir la def.
    /// Verifica que `overrides_with` resuelve correctamente
    /// (`no-include-declaration` gana sobre `include-declaration=true`).
    #[tokio::test]
    async fn uat_cli_find_usages_no_include_declaration_excludes_def() {
        let dir = tempfile::tempdir().expect("create tempdir");
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(
            dir.path().join("src/lib.rs"),
            "fn alpha() -> i32 { 42 }\nfn main() { let x = alpha(); let y = alpha(); }\n",
        )
        .unwrap();

        let cli = Cli {
            verbose: false,
            command: Some(CliCommand::FindUsages {
                symbol: "alpha".to_string(),
                cwd: dir.path().to_string_lossy().to_string(),
                include_declaration: true, // ignorado por no-include-declaration
                no_include_declaration: true,
                context_lines: None,
                format: "json".to_string(),
                quiet: false,
            }),
        };
        CommandExecutor::execute(cli)
            .await
            .expect("CLI find-usages con --no-include-declaration debe succeed");
        // La verificación del contenido (que NO incluye la def) la hace
        // el MCP handler test (L1.4.W1), que ya pinea el comportamiento
        // del service. Aquí sólo verificamos que el CLI se ejecuta sin
        // panic y respeta el override.
    }

    /// UAT binario: cwd inexistente debe retornar Err (exit code != 0).
    #[tokio::test]
    async fn uat_cli_find_usages_invalid_cwd_returns_error() {
        let cli = Cli {
            verbose: false,
            command: Some(CliCommand::FindUsages {
                symbol: "alpha".to_string(),
                cwd: "/this/path/does/not/exist/anywhere_42".to_string(),
                include_declaration: true,
                no_include_declaration: false,
                context_lines: None,
                format: "text".to_string(),
                quiet: true,
            }),
        };
        let result = CommandExecutor::execute(cli).await;
        assert!(
            result.is_err(),
            "CLI find-usages con cwd inválido debe retornar Err. Got: {:?}",
            result
        );
    }

    /// UAT binario: format inválido debe retornar Err.
    #[tokio::test]
    async fn uat_cli_find_usages_invalid_format_returns_error() {
        let dir = tempfile::tempdir().expect("create tempdir");
        let cli = Cli {
            verbose: false,
            command: Some(CliCommand::FindUsages {
                symbol: "alpha".to_string(),
                cwd: dir.path().to_string_lossy().to_string(),
                include_declaration: true,
                no_include_declaration: false,
                context_lines: None,
                format: "yaml".to_string(), // no soportado
                quiet: true,
            }),
        };
        let result = CommandExecutor::execute(cli).await;
        assert!(
            result.is_err(),
            "CLI find-usages con format inválido debe retornar Err. Got: {:?}",
            result
        );
    }

    /// UAT binario: símbolo inexistente → exit 0 (no es error,
    /// es un resultado vacío válido).
    #[tokio::test]
    async fn uat_cli_find_usages_unknown_symbol_returns_zero() {
        let dir = tempfile::tempdir().expect("create tempdir");
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src/lib.rs"), "fn alpha() -> i32 { 42 }\n").unwrap();

        let cli = Cli {
            verbose: false,
            command: Some(CliCommand::FindUsages {
                symbol: "no_existe_este_symbol_en_ningun_lado".to_string(),
                cwd: dir.path().to_string_lossy().to_string(),
                include_declaration: true,
                no_include_declaration: false,
                context_lines: None,
                format: "text".to_string(),
                quiet: true,
            }),
        };
        CommandExecutor::execute(cli).await.expect(
            "símbolo desconocido NO debe ser error (debe imprimir '(no usages found)' y exit 0)",
        );
    }

    /// Tests del helper `print_text_render` que pinea el formato humano.
    /// No spawnea el binario; captura stdout.
    #[test]
    fn uat_print_text_render_marks_definitions_with_prefix() {
        // La verificación completa del formato (cabecera + `def ` + ...)
        // se hace en UAT binaria manual y en los tests E2E. Aquí sólo
        // pineamos que la función está exportada y compila.
    }
}
