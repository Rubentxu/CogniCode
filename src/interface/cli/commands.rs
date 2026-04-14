//! CLI Commands - Command-line interface implementations

use clap::{CommandFactory, Parser, Subcommand};
use tracing::info;
use std::path::PathBuf;
use std::time::Instant;
use crate::infrastructure::graph::{
    FullGraphStrategy, GraphStrategy, GraphStrategyFactory, LightweightStrategy, OnDemandStrategy,
    PerFileStrategy, TraversalDirection,
};
use crate::infrastructure::semantic::{OutlineNode, SymbolCodeService};
use crate::infrastructure::parser::Language;
use crate::domain::services::CallGraphAnalyzer;
use crate::domain::traits::code_intelligence::CodeIntelligenceProvider;

/// CLI arguments for CogniCode
#[derive(Debug, Parser)]
#[command(name = "cognicode")]
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
        /// The refactoring operation to perform
        #[arg(value_enum, default_value = "rename")]
        operation: RefactorOperation,
        /// Symbol to refactor
        symbol: String,
        /// New name (for rename operation)
        new_name: Option<String>,
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
            std::env::set_var("RUST_LOG", "debug");
        }

        match &cli.command {
            Some(CliCommand::Analyze { path }) => {
                info!("Analyzing code at: {}", path);
                // TODO: Implement analyze command
            }
            Some(CliCommand::Serve { port }) => {
                info!("Starting MCP server on port: {}", port);
                // TODO: Implement serve command
            }
            Some(CliCommand::Refactor { operation, symbol, new_name }) => {
                info!("Refactoring: {:?} - {} -> {:?}", operation, symbol, new_name);
                // TODO: Implement refactor command
            }
            Some(CliCommand::Index { command }) => {
                if let Err(e) = Self::execute_index(command).await {
                    eprintln!("Index command failed: {}", e);
                }
            }
            Some(CliCommand::Graph { command }) => {
                if let Err(e) = Self::execute_graph(command).await {
                    eprintln!("Graph command failed: {}", e);
                }
            }
            Some(CliCommand::Navigate { command }) => {
                if let Err(e) = Self::execute_navigate(command).await {
                    eprintln!("Navigate command failed: {}", e);
                }
            }
            Some(CliCommand::Doctor { format }) => {
                if let Err(e) = Self::execute_doctor(format).await {
                    eprintln!("Doctor command failed: {}", e);
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
                        println!("Index built successfully in {}ms using {} strategy",
                            elapsed, strategy_box.name());
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
                        println!("  {}:{}:{} ({})",
                            loc.file, loc.line, loc.column,
                            format!("{:?}", loc.symbol_kind));
                    }
                }
            }
            IndexCommand::Outline { file, include_private, include_tests } => {
                println!("Getting outline for: {}", file);

                let source = std::fs::read_to_string(file)?;
                let language = Language::Rust; // TODO: detect from file extension

                let outline = crate::infrastructure::semantic::build_outline(
                    &source, file, language, *include_private, *include_tests
                );

                println!("Found {} top-level symbols:", outline.len());
                print_outline_tree(&outline, 0);
            }
            IndexCommand::SymbolCode { file, line, column, include_doc: _ } => {
                println!("Getting symbol code for: {}:{}:{}", file, line, column);

                let service = SymbolCodeService::new();

                match service.get_symbol_code(file, *line, *column) {
                    Ok(code) => {
                        if let Some(doc) = &code.docstring {
                            println!("\n/// Docstring:\n{}", doc);
                        }
                        println!("\n/// Symbol code (lines {} - {}):", code.start_line, code.end_line);
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
            GraphCommand::OnDemand { symbol, depth, direction, path } => {
                let start = Instant::now();
                println!("Building on-demand subgraph for '{}' (depth={}, direction={})", symbol, depth, direction);

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
                println!("Root: {} ({}:{}:{})",
                    result.root_symbol.name(),
                    result.root_symbol.location().file(),
                    result.root_symbol.location().line(),
                    result.root_symbol.location().column());
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
            GraphCommand::Full { rebuild, path } => {
                let start = Instant::now();
                println!("Building full project graph at: {}{}",
                    path,
                    if *rebuild { " (rebuild)" } else { "" });

                let strategy = FullGraphStrategy::new();
                let dir = PathBuf::from(path);

                match strategy.build_full_graph(&dir) {
                    Ok(graph) => {
                        let elapsed = start.elapsed().as_millis();
                        println!("Full graph built in {}ms", elapsed);
                        println!("  Total symbols: {}", graph.symbol_count());
                        println!("  Total dependencies: {}", graph.edge_count());
                    }
                    Err(e) => {
                        eprintln!("Error building full graph: {}", e);
                        return Err(Box::new(e));
                    }
                }
            }
            GraphCommand::HotPaths { limit, min_fan_in, path } => {
                let start = Instant::now();
                println!("Finding hot paths in: {} (limit={}, min_fan_in={})", path, limit, min_fan_in);

                let dir = PathBuf::from(path);
                let strategy = FullGraphStrategy::new();

                let graph = match strategy.build_full_graph(&dir) {
                    Ok(g) => g,
                    Err(e) => {
                        eprintln!("Error building graph: {}", e);
                        return Err(Box::new(e));
                    }
                };

                let analyzer = CallGraphAnalyzer::new();
                let hot_paths = analyzer.find_hot_paths(&graph, *limit);

                let filtered: Vec<_> = hot_paths.into_iter()
                    .filter(|h| h.fan_in >= *min_fan_in)
                    .collect();

                println!("\nHot paths (most called functions):");
                println!("{:<40} {:>8} {:>8}  {}", "Function", "Fan-in", "Fan-out", "Location");
                println!("{}", "-".repeat(80));

                for hp in &filtered {
                    println!("{:<40} {:>8} {:>8}  {}:{}",
                        hp.symbol_name,
                        hp.fan_in,
                        hp.fan_out,
                        hp.file, hp.line);
                }

                let elapsed = start.elapsed().as_millis();
                println!("\nFound {} hot paths in {}ms", filtered.len(), elapsed);
            }
            GraphCommand::EntryPoints { path } => {
                let start = Instant::now();
                println!("Finding entry points in: {}", path);

                let dir = PathBuf::from(path);
                let strategy = FullGraphStrategy::new();

                let graph = match strategy.build_full_graph(&dir) {
                    Ok(g) => g,
                    Err(e) => {
                        eprintln!("Error building graph: {}", e);
                        return Err(Box::new(e));
                    }
                };

                let entry_ids = graph.roots();
                println!("\nEntry points (no incoming edges):");
                for id in entry_ids.iter().take(20) {
                    if let Some(sym) = graph.get_symbol(id) {
                        println!("  {} at {}:{}:{}",
                            sym.name(),
                            sym.location().file(),
                            sym.location().line(),
                            sym.location().column());
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

                let graph = match strategy.build_full_graph(&dir) {
                    Ok(g) => g,
                    Err(e) => {
                        eprintln!("Error building graph: {}", e);
                        return Err(Box::new(e));
                    }
                };

                let leaf_ids = graph.leaves();
                println!("\nLeaf functions (no outgoing edges):");
                for id in leaf_ids.iter().take(20) {
                    if let Some(sym) = graph.get_symbol(id) {
                        println!("  {} at {}:{}:{}",
                            sym.name(),
                            sym.location().file(),
                            sym.location().line(),
                            sym.location().column());
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

                let graph = match strategy.build_full_graph(&dir) {
                    Ok(g) => g,
                    Err(e) => {
                        eprintln!("Error building graph: {}", e);
                        return Err(Box::new(e));
                    }
                };

                let source_id = crate::domain::aggregates::call_graph::SymbolId::new(from.clone());
                let target_id = crate::domain::aggregates::call_graph::SymbolId::new(to.clone());

                match graph.find_path(&source_id, &target_id) {
                    Some(path_ids) => {
                        println!("\nPath found ({} hops):", path_ids.len());
                        for (i, id) in path_ids.iter().enumerate() {
                            if let Some(sym) = graph.get_symbol(id) {
                                println!("  {}. {} at {}:{}",
                                    i + 1,
                                    sym.name(),
                                    sym.location().file(),
                                    sym.location().line());
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

                let graph = match strategy.build_full_graph(&dir) {
                    Ok(g) => g,
                    Err(e) => {
                        eprintln!("Error building graph: {}", e);
                        return Err(Box::new(e));
                    }
                };

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
            GraphCommand::Hierarchy { symbol, depth, direction, path } => {
                let start = Instant::now();
                println!("Getting call hierarchy for '{}' (depth={}, direction={}) in: {}",
                    symbol, depth, direction, path);

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
                println!("Root: {} at {}:{}:{}",
                    result.root_symbol.name(),
                    result.root_symbol.location().file(),
                    result.root_symbol.location().line(),
                    result.root_symbol.location().column());

                println!("\nEntries by depth:");
                let mut by_depth: std::collections::HashMap<u32, Vec<_>> = std::collections::HashMap::new();
                for entry in &result.entries {
                    by_depth.entry(entry.depth).or_default().push(entry);
                }
                for depth in 1..=*depth {
                    if let Some(entries) = by_depth.get(&depth) {
                        println!("  Depth {}: {} entries", depth, entries.len());
                        for entry in entries.iter().take(5) {
                            println!("    - {} ({}) at {}:{}",
                                entry.symbol.name(),
                                format!("{:?}", entry.direction).to_lowercase(),
                                entry.symbol.location().file(),
                                entry.symbol.location().line());
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

                let graph = match strategy.build_full_graph(&dir) {
                    Ok(g) => g,
                    Err(e) => {
                        eprintln!("Error building graph: {}", e);
                        return Err(Box::new(e));
                    }
                };

                let analyzer = CallGraphAnalyzer::new();
                let complexity = analyzer.calculate_complexity(&graph);

                println!("\nComplexity Metrics:");
                println!("  Total symbols: {}", complexity.total_symbols);
                println!("  Total edges: {}", complexity.total_edges);
                println!("  Max depth: {}", complexity.max_depth);
                println!("  Cyclomatic complexity: {}", complexity.cyclomatic_complexity);
                println!("  High fan-out (>=10): {}", complexity.high_fan_out_count);
                println!("  Medium fan-out (5-9): {}", complexity.medium_fan_out_count);
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

                let graph = match strategy.build_full_graph(&dir) {
                    Ok(g) => g,
                    Err(e) => {
                        eprintln!("Error building graph: {}", e);
                        return Err(Box::new(e));
                    }
                };

                // Find all symbols that match the name
                let search_name = symbol.to_lowercase();
                let symbol_ids: Vec<_> = graph
                    .symbols()
                    .filter(|s| {
                        let name = s.name().to_lowercase();
                        let fqn = s.fully_qualified_name().to_lowercase();
                        name == search_name || fqn == search_name || name.contains(&search_name) || fqn.contains(&search_name)
                    })
                    .map(|s| crate::domain::aggregates::call_graph::SymbolId::new(s.fully_qualified_name()))
                    .collect();

                if symbol_ids.is_empty() {
                    println!("\nNo symbols found matching '{}'", symbol);
                    return Ok(());
                }

                println!("\nFound {} symbol(s) matching '{}'", symbol_ids.len(), symbol);

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
                } else if impacted_symbols.len() > 0 {
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
            ).into());
        }
        // rsplitn gives reversed order: column, line, file
        let column: u32 = parts[0].parse().map_err(|_| format!("Invalid column in '{}'", position))?;
        let line: u32 = parts[1].parse().map_err(|_| format!("Invalid line in '{}'", position))?;
        let file = parts[2].to_string();
        Ok((file, line, column))
    }

    /// Execute navigate subcommand
    async fn execute_navigate(command: &NavigateCommand) -> Result<(), Box<dyn std::error::Error>> {
        use crate::infrastructure::lsp::providers::CompositeProvider;
        use crate::domain::value_objects::Location;
        use std::path::Path;

        match command {
            NavigateCommand::Definition { position, path } => {
                let (file, line, column) = Self::parse_position(position)?;
                println!("Go to definition: {}:{}:{} (workspace: {})", file, line, column, path);
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
                println!("Hover info: {}:{}:{} (workspace: {})", file, line, column, path);
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
            NavigateCommand::References { position, include_declaration, path } => {
                let (file, line, column) = Self::parse_position(position)?;
                println!("Find references: {}:{}:{} (workspace: {})", file, line, column, path);
                println!("Connecting to LSP server...");

                let workspace = Path::new(path);
                let provider = CompositeProvider::new(workspace);
                let location = Location::new(file.clone(), line.saturating_sub(1), column);

                match provider.find_references(&location, *include_declaration).await {
                    Ok(refs) => {
                        if refs.is_empty() {
                            println!("No references found for {}:{}", file, line);
                        } else {
                            println!("Found {} reference(s):", refs.len());
                            for r in &refs {
                                let container = r.container.as_deref().unwrap_or("(unknown)");
                                println!("  {}:{}:{} [{:?}] in {}",
                                    r.location.file(),
                                    r.location.line() + 1,
                                    r.location.column(),
                                    r.reference_kind,
                                    container);
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
    async fn execute_doctor(format: &str) -> Result<(), Box<dyn std::error::Error>> {
        use crate::infrastructure::lsp::tool_checker::ToolAvailabilityChecker;

        let report = ToolAvailabilityChecker::doctor_report();

        match format {
            "json" => {
                println!("[");
                let last = report.len().saturating_sub(1);
                for (i, (lang, status)) in report.iter().enumerate() {
                    let comma = if i < last { "," } else { "" };
                    let path_str = status.binary_path.as_ref()
                        .map(|p| p.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    let version_str = status.version.as_deref().unwrap_or("");
                    println!(
                        "  {{\"language\":\"{}\",\"lsp\":\"{}\",\"available\":{},\"path\":\"{}\",\"version\":\"{}\",\"install_command\":\"{}\"}}{}",
                        lang.name(), lang.lsp_server_name(),
                        status.available, path_str, version_str,
                        status.install_command, comma
                    );
                }
                println!("]");
            }
            _ => {
                println!("LSP Server Availability Report");
                println!("{}", "=".repeat(60));
                println!("{:<12} {:<20} {:<10} {}", "Language", "LSP Server", "Status", "Path / Install");
                println!("{}", "-".repeat(60));
                for (lang, status) in &report {
                    let detail = if status.available {
                        status.binary_path.as_ref()
                            .map(|p| p.to_string_lossy().into_owned())
                            .unwrap_or_default()
                    } else {
                        format!("Install: {}", status.install_command)
                    };
                    let status_str = if status.available { "OK" } else { "MISSING" };
                    println!("{:<12} {:<20} {:<10} {}",
                        lang.name(), lang.lsp_server_name(), status_str, detail);
                    if status.available {
                        if let Some(v) = &status.version {
                            println!("{:<12} {:<20} {:<10} version: {}",
                                "", "", "", v);
                        }
                    }
                }
                println!("{}", "-".repeat(60));
                let available = report.iter().filter(|(_, s)| s.available).count();
                println!("  {}/{} LSP servers available", available, report.len());
            }
        }
        Ok(())
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

        let sig_info = node.signature.as_ref()
            .map(|s| format!(": {}", s))
            .unwrap_or_default();

        println!("{}{} ({}){}", prefix, node.name, format!("{:?}", node.kind), sig_info);

        if !node.children.is_empty() {
            print_outline_tree(&node.children, indent + 1);
        }
    }
}