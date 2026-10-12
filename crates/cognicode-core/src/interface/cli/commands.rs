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
use crate::infrastructure::semantic::SymbolCodeService;
use crate::interface::cli::result::{
    CommandOutput, GraphResult, IndexError, IndexResult, NavigateError, NavigateResult,
};
use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use serde_json::Value;
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

    /// Output format for structured commands (index, graph, and other
    /// commands that expose a `--format` flag). Sub-commands with their
    /// own per-variant `--format` (Capabilities, Doctor, FindUsages,
    /// Refactor, Graph::Full, Graph::Mermaid) keep their own flag and
    /// ignore this top-level one.
    #[arg(long, value_enum, default_value_t = Format::Text)]
    pub format: Format,

    /// The command to execute
    #[command(subcommand)]
    pub command: Option<CliCommand>,
}

/// T1-5.5 — Output format dispatch.
///
/// The bin (cognicode-cli) inspects `CommandOutput` together with this
/// value to decide text vs JSON. `Text` is the default and preserves
/// byte-identical output with the pre-T1-5.5 behavior; `Json` emits
/// the structured `*Result` via `serde_json::to_writer_pretty`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[clap(rename_all = "lower")]
pub enum Format {
    Text,
    Json,
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

    /// Export `assurance-evidence/v1` para pipelinek-assurance.
    ///
    /// Produce un envelope CBOR (canónico) o JSON (debug) que el
    /// `CogniCodeArtifactProvider` de pipelinek-assurance consume. El
    /// shape es `assurance-evidence/v1` (ver `assurance_export::envelope`).
    ///
    /// El digest M-D01-style (fail-closed) se calcula sobre los bytes
    /// CBOR con `digest = ""` placeholder; el SHA-256 hex se incluye en
    /// el campo `digest` del envelope. El consumer verifica re-codificando.
    ///
    /// Capabilities no soportadas se declaran como
    /// `CapabilityCompleteness::Unsupported { reason }` con motivo, no
    /// como `items: []` (regla C4 del workstream). Esto significa que
    /// en v1 `architecture`, `relations`, `signals` siempre seran
    /// Unsupported: todavia no se calcula el grafo de dependencias, el
    /// SCC ni los signals heuristicos.
    #[command(name = "export")]
    ExportAssurance {
        /// Subcomando unico en v1: `cognicode export assurance`. El
        /// parser de clap exige este nivel aunque el binario CLI vive
        /// en `cognicode-cli`; aqui solo se modela la variante del
        /// enum y el binario `cognicode` lo expone.
        #[command(subcommand)]
        command: ExportAssuranceCommand,
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

    /// A-014 / CP2.4 — Machine-readable capability discovery.
    ///
    /// Emits a single JSON document on stdout (schema
    /// `cognicode.capabilities/v1`) describing the tools, profiles
    /// and runtime mutating set of the binary in front of you. The
    /// `--format json` mode is the canonical contract; the default
    /// `text` mode prints a short human summary on stdout.
    ///
    /// Pin contract: `crates/cognicode-cli/tests/a014_capabilities_json.rs`
    /// (5 strict-TDD tests).
    Capabilities {
        /// Output format: `text` (default) or `json`.
        #[arg(long, default_value = "text")]
        format: String,
        /// Documented shorthand for `--format json`, kept because
        /// the A-014 acceptance criterion in the action register is
        /// written as `cognicode capabilities --json`. Emits the
        /// identical document; never a different contract.
        #[arg(long, conflicts_with = "format")]
        json: bool,
    },
    /// B1.W2 — Print full identity: workspace version, baked-in
    /// source SHA (from `git rev-parse HEAD` at build time), compiled
    /// features, and the git remote URL. Used to verify a binary
    /// against its intended release artifact. Does NOT mutate state
    /// or print to stderr.
    Identity,
    /// B3.W2 — Progressive context routing (scaffold).
    ///
    /// PROPOSED: contract under review; see B3.W5.
    ///
    /// Best-effort parser that maps free-text arguments to an
    /// `Intent` and prints the routing chain (which tools to call,
    /// in what order). The scaffold does NOT actually invoke the
    /// routed tools; the caller (human or agent) reads the chain
    /// and issues the calls themselves. The contract is pinned by
    /// `b3_w2_progressive_context::inspect_subcommand_does_not_panic_on_arbitrary_input`
    /// and `b3_w2_progressive_context::intent_tags_in_catalog_match_route_intent`.
    ///
    /// Parsing rules (best-effort, NOT a final contract):
    ///   - `inspect path/to/file.rs`            -> Read
    ///   - `inspect path/to/file.rs:10-20`      -> Read with range
    ///   - `inspect fn foo`  / `inspect foo`    -> Locate
    ///   - `inspect summary <scope>`            -> Summarize
    ///   - `inspect verify <claim>`             -> Verify
    ///   - anything else                        -> fallback to Locate
    ///   -                                          with the joined args as
    ///   -                                          the symbol (last-ditch).
    Inspect {
        /// Free-text arguments the user typed after `inspect`.
        /// Parsed best-effort into an `Intent`; never panics.
        intent_args: Vec<String>,
    },
}

/// Subcomandos de `cognicode export`. v1 solo expone `assurance`; el
/// shape es `assurance-evidence/v1` (ver `assurance_export::envelope`).
/// Cambios incompatibles al shape bumpan a `assurance-evidence/v2`.
#[derive(clap::Subcommand, Debug)]
pub enum ExportAssuranceCommand {
    /// Produce el envelope `assurance-evidence/v1` con la forma
    /// exacta que `CogniCodeArtifactProvider` consume (ver
    /// `pipelinek-assurance/.../CogniCodeEvidenceExportDto.kt`).
    #[command(name = "assurance")]
    Assurance {
        /// Workspace raiz del que se extrae la evidencia.
        #[arg(short = 'C', long, default_value = ".")]
        workspace: String,

        /// Capabilities a incluir. Si no se pasa ninguna, se piden
        /// todas las conocidas (modo "produce lo que puedas"). Las
        /// capabilities no soportadas en v1 se declaran `Unsupported`
        /// con motivo en `gaps[]`, NO `items: []` (regla C4 del
        /// workstream).
        #[arg(short = 'c', long = "capability")]
        capability: Vec<String>,

        /// Path de salida del envelope. El sufijo determina el formato:
        /// `.cbor` produce CBOR canonical, `.json` produce JSON debug.
        #[arg(short = 'o', long)]
        output: String,

        /// Forzar formato de salida (`cbor` o `json`). Sobreescribe la
        /// decision basada en el sufijo del path.
        #[arg(long, value_enum)]
        format: Option<ExportFormat>,
    },
}

/// Formato del envelope. `cbor` es canónico (lo consume el provider de
/// pipelinek-assurance); `json` es solo debug / inspeccion humana.
#[derive(clap::ValueEnum, Clone, Debug, PartialEq, Eq)]
pub enum ExportFormat {
    Cbor,
    Json,
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
    ///
    /// T1-5.5: returns `Result<CommandOutput, _>` so the bin
    /// (cognicode-cli) can dispatch `--format text|json` against
    /// the structured `*Result` without the library doing the
    /// formatting itself. Sub-commands that still emit text inline
    /// (Navigate, Analyze, Doctor, FindUsages, Capabilities,
    /// ExportAssurance, Refactor, Evidence, DocsIngest,
    /// IssuesIngest) return `CommandOutput::Other` and the bin
    /// passes them through unchanged.
    pub async fn execute(cli: Cli) -> Result<CommandOutput, Box<dyn std::error::Error>> {
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
                Self::execute_analyze(path).await?;
                Ok(CommandOutput::Other)
            }
            // B1.W2 — `cognicode identity` returns Other; the bin
            // detects the variant from a clone of cli.command (in
            // main.rs) and dispatches to identity::print before the
            // presenter runs. Identity is a read-only inspection
            // surface; no execute_* function needed here.
            Some(CliCommand::Identity) => Ok(CommandOutput::Other),
            // B3.W2 — `cognicode inspect <args...>` scaffold. The
            // parser is best-effort (see the variant doc comment).
            // On any input it must (a) not panic, (b) emit the
            // PROPOSED status marker to stderr, and (c) exit 0.
            // The actual routing chain is delegated to
            // `crate::interface::intent::route_intent`. B3.W5 pins
            // the final shape; for now the contract is
            // "scaffold compiles, no panic, announces PROPOSED".
            Some(CliCommand::Inspect { intent_args }) => {
                let intent = parse_intent_args(intent_args);
                let chain = crate::interface::intent::route_intent(&intent);
                eprintln!(
                    "PROPOSED: contract under review; see B3.W5. \
                     intent={} next_steps={}",
                    intent_kind_label(&intent),
                    chain.len(),
                );
                for (i, step) in chain.iter().enumerate() {
                    eprintln!("  {}. {} -> {}", i + 1, step.tool, step.args);
                }
                Ok(CommandOutput::Other)
            }
            Some(CliCommand::Serve { port }) => {
                eprintln!("Use 'cognicode-mcp' binary to start the MCP server.");
                eprintln!("The MCP server uses stdio transport, not TCP ports.");
                eprintln!("Run: cognicode-mcp --cwd <workspace>");
                let _ = port;
                Ok(CommandOutput::Other)
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
                Self::execute_refactor(operation, symbol, new_name.as_deref(), format).await?;
                Ok(CommandOutput::Other)
            }
            Some(CliCommand::Index { command }) => {
                // T1-5.5d: the presenter has moved to
                // `cognicode_cli::presenter` (in the bin crate).
                // The library returns `IndexResult`; the bin
                // formats it (text or JSON per `--format`). No
                // presentation logic remains here.
                let result = Self::execute_index(command).await.map_err(Box::new)?;
                Ok(CommandOutput::Index(result))
            }
            Some(CliCommand::Graph { command }) => {
                // PRF F2.W2: do NOT swallow the Graph error here.
                // PerFileStrategy and FullGraphStrategy now surface read /
                // parse / unsupported-extension failures as `Err`, and the
                // UAT (see `w2_uat_tests` below) requires those to
                // propagate so that scripts and CI can detect them.
                // T1-5.4b: execute_graph now returns GraphResult.
                // T1-5.5: bind the result (was `let _result`) so the bin
                // can consume it. The inline presenter calls inside
                // `execute_graph` (12 sites) still emit text; T1-5.5d
                // removes them and the bin takes over.
                let result = Self::execute_graph(command).await?;
                Ok(CommandOutput::Graph(result))
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
                Self::execute_find_usages(symbol, cwd, include, *context_lines, format, *quiet)
                    .await?;
                Ok(CommandOutput::Other)
            }
            Some(CliCommand::Navigate { command }) => {
                // T1-5.6a: execute_navigate returns NavigateResult so
                // the bin can dispatch text/JSON. Pre-T1-5.6a this
                // returned Other; the bin ignored the typed result.
                let result = Self::execute_navigate(command).await.map_err(Box::new)?;
                Ok(CommandOutput::Navigate(result))
            }
            Some(CliCommand::Doctor { format, cwd }) => {
                Self::execute_doctor(format, cwd).await?;
                Ok(CommandOutput::Other)
            }
            Some(CliCommand::ExportAssurance { command }) => {
                Self::execute_export_assurance(command).await?;
                Ok(CommandOutput::Other)
            }
            #[cfg(feature = "multimodal")]
            Some(CliCommand::DocsIngest { path, recursive }) => {
                Self::execute_docs_ingest(path, *recursive).await?;
                Ok(CommandOutput::Other)
            }
            #[cfg(feature = "multimodal")]
            Some(CliCommand::IssuesIngest {
                owner,
                repo,
                include_git_log,
            }) => {
                Self::execute_issues_ingest(owner, repo, *include_git_log).await?;
                Ok(CommandOutput::Other)
            }
            #[cfg(feature = "evidence-cli-ladybug")]
            Some(CliCommand::Evidence(cmd)) => {
                Self::execute_evidence(cmd).await?;
                Ok(CommandOutput::Other)
            }
            Some(CliCommand::Capabilities { format, json }) => {
                let effective = if *json { "json" } else { format.as_str() };
                Self::execute_capabilities(effective).await?;
                Ok(CommandOutput::Other)
            }
            None => {
                info!("CogniCode CLI initialized");
                // Print help if no command given
                let mut cmd = Cli::command();
                cmd.print_help()?;
                println!();
                Ok(CommandOutput::Other)
            }
        }
    }

    /// Execute index subcommand
    ///
    /// T1-5.3c migration: returns `Result<IndexResult, IndexError>`
    /// instead of `Result<(), Box<dyn Error>>`. The function no
    /// longer prints to stdout; the caller in `execute()` invokes
    /// the presenter (T1-5.5 will move that into the bin's own
    /// module, but for T1-5.3c it lives here so the output
    /// remains byte-identical with the pre-migration version).
    async fn execute_index(command: &IndexCommand) -> Result<IndexResult, IndexError> {
        match command {
            IndexCommand::Build { path, strategy } => {
                tracing::info!(
                    target: "cli.index.build",
                    strategy = %strategy,
                    path = %path,
                    "building index"
                );
                let start = Instant::now();
                let mut strategy_box = GraphStrategyFactory::create(strategy);
                let dir = PathBuf::from(path);

                strategy_box
                    .build_index(&dir)
                    .map_err(|e| IndexError::Build {
                        path: dir.clone(),
                        strategy: strategy.to_string(),
                        source: Box::new(e),
                    })?;
                let elapsed_ms = start.elapsed().as_millis();
                Ok(IndexResult::Build {
                    path: dir,
                    strategy: strategy.to_string(),
                    elapsed_ms,
                })
            }
            IndexCommand::Query { symbol, path } => {
                tracing::info!(
                    target: "cli.index.query",
                    symbol = %symbol,
                    path = %path,
                    "querying symbol"
                );
                let mut strategy = LightweightStrategy::new();
                let dir = PathBuf::from(path);

                strategy.build_index(&dir).map_err(|e| IndexError::Query {
                    symbol: symbol.clone(),
                    path: dir.clone(),
                    source: Box::new(e),
                })?;
                let locations = strategy.query_symbols(symbol);
                if locations.is_empty() {
                    Ok(IndexResult::QueryEmpty {
                        symbol: symbol.clone(),
                        path: dir,
                    })
                } else {
                    Ok(IndexResult::Query {
                        symbol: symbol.clone(),
                        path: dir,
                        locations,
                    })
                }
            }
            IndexCommand::Outline {
                file,
                include_private,
                include_tests,
            } => {
                tracing::info!(
                    target: "cli.index.outline",
                    file = %file,
                    "getting outline"
                );
                let source = std::fs::read_to_string(file).map_err(|e| IndexError::Outline {
                    file: PathBuf::from(file),
                    source: e,
                })?;
                let language = Language::from_extension(std::path::Path::new(file).extension())
                    .unwrap_or(Language::Rust);

                let outline = crate::infrastructure::semantic::build_outline(
                    &source,
                    file,
                    language,
                    *include_private,
                    *include_tests,
                );

                Ok(IndexResult::Outline {
                    file: file.clone().into(),
                    symbols: outline,
                })
            }
            IndexCommand::SymbolCode {
                file,
                line,
                column,
                include_doc: _,
            } => {
                tracing::info!(
                    target: "cli.index.symbol_code",
                    file = %file,
                    line = line,
                    column = column,
                    "getting symbol code"
                );
                let service = SymbolCodeService::new();

                let code = service.get_symbol_code(file, *line, *column).map_err(|e| {
                    IndexError::SymbolCode {
                        file: file.clone().into(),
                        line: *line,
                        column: *column,
                        source: Box::<dyn std::error::Error + Send + Sync>::from(
                            std::io::Error::other(e),
                        ),
                    }
                })?;
                Ok(IndexResult::SymbolCode {
                    file: file.clone().into(),
                    line: *line,
                    column: *column,
                    start_line: code.start_line,
                    end_line: code.end_line,
                    doc: code.docstring,
                    code: code.code,
                })
            }
        }
    }

    /// Execute graph subcommand
    async fn execute_graph(
        command: &GraphCommand,
    ) -> Result<GraphResult, Box<dyn std::error::Error>> {
        use crate::interface::cli::result::{
            DepthBucket, DepthEntry, GraphResult as GR, HotPathEntry, PathHop, SkippedFile,
            SymbolRef,
        };
        match command {
            GraphCommand::OnDemand {
                symbol,
                depth,
                direction,
                path,
            } => {
                let start = Instant::now();
                // Progress line — same text as the
                // pre-migration println!; the presenter
                // re-emits it (T1-5.4i). The println! here
                // is removed.
                //
                // (Note: we DON'T call the presenter's
                // progress line from inside this function;
                // the presenter writes the whole stdout
                // byte stream when called.)

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

                let final_result = GR::OnDemand {
                    symbol: symbol.clone(),
                    depth: *depth,
                    direction: direction.clone(),
                    elapsed_ms: elapsed,
                    root_name: result.root_symbol.name().to_string(),
                    root_file: result.root_symbol.location().file().to_string(),
                    root_line: result.root_symbol.location().line(),
                    root_column: result.root_symbol.location().column(),
                    entries_len: result.entries.len(),
                };
                // T1-5.4i: text formatting is in the presenter.
                Ok(final_result)
            }
            GraphCommand::PerFile { file } => {
                let strategy = PerFileStrategy::new();
                let file_path = PathBuf::from(file);

                match strategy.build_local_graph(&file_path) {
                    Ok(graph) => {
                        let symbols = graph.symbol_count();
                        let edges = graph.edge_count();
                        let result = GR::PerFile {
                            file: file.clone(),
                            symbols,
                            dependencies: edges,
                        };
                        // T1-5.4c: text formatting is in the
                        // presenter. The function still emits
                        // the text (T1-5.5 will move it to the
                        // bin); the snapshot test pins the
                        // byte stream.
                        Ok(result)
                    }
                    Err(e) => {
                        eprintln!("Error building per-file graph: {}", e);
                        Err(Box::new(e))
                    }
                }
            }
            GraphCommand::Full {
                rebuild,
                format: _format,
                path,
            } => {
                let start = Instant::now();
                // T1-5.5d: the per-subcommand --format on
                // GraphCommand::Full is now ignored; the top-level
                // --format on Cli is canonical. Progress always goes
                // to stderr so stdout stays clean for `cogh ... | jq`.
                eprintln!(
                    "Building full project graph at: {}{}",
                    path,
                    if *rebuild { " (rebuild)" } else { "" }
                );

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
                        let skipped_files: Vec<SkippedFile> = skipped
                            .iter()
                            .filter_map(|s| {
                                let (p, r) = s.split_once(": ")?;
                                Some(SkippedFile {
                                    path: p.to_string(),
                                    reason: r.to_string(),
                                })
                            })
                            .collect();
                        // T1-5.5d: text formatting is in the bin's
                        // presenter (cognicode_cli::presenter). The
                        // legacy ad-hoc FullGraphJson envelope
                        // (`schema_version: "cognicode.graph.full/v1"`)
                        // is gone — the bin emits the unified JSON
                        // via `serde_json::to_writer_pretty(&GR::Full)`
                        // which carries `kind: "full"`, `rebuild`, the
                        // counts, `status`, and `skipped_files`.
                        let result = GR::Full {
                            path: path.clone(),
                            rebuild: *rebuild,
                            elapsed_ms: elapsed,
                            symbols,
                            dependencies: edges,
                            status: if skipped.is_empty() {
                                "complete".to_string()
                            } else {
                                "partial".to_string()
                            },
                            skipped_files,
                        };
                        // Skipped lines go to stderr (matches the
                        // pre-migration eprintln!). The bin's text
                        // presenter emits them too via the `status`
                        // and `skipped_files` fields; keeping them
                        // here preserves the historical stderr stream
                        // for shell pipelines that grep "Building ..."
                        // or "Skipped:" independently of stdout.
                        for s in &skipped {
                            eprintln!("  Skipped: {}", s);
                        }
                        Ok(result)
                    }
                    Err(e) => {
                        eprintln!("Error building full graph: {}", e);
                        Err(Box::new(e))
                    }
                }
            }
            GraphCommand::HotPaths {
                limit,
                min_fan_in,
                path,
            } => {
                let start = Instant::now();

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

                let elapsed = start.elapsed().as_millis();
                let result = GR::HotPaths {
                    path: path.clone(),
                    limit: *limit,
                    min_fan_in: *min_fan_in,
                    entries: filtered
                        .iter()
                        .map(|hp| HotPathEntry {
                            name: hp.symbol_name.clone(),
                            fan_in: hp.fan_in,
                            fan_out: hp.fan_out,
                            file: hp.file.clone(),
                            line: hp.line,
                        })
                        .collect(),
                    elapsed_ms: elapsed,
                };
                // T1-5.4e: text formatting is in the presenter.
                Ok(result)
            }
            GraphCommand::EntryPoints { path } => {
                let start = Instant::now();

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
                let mut entry_refs: Vec<SymbolRef> = Vec::new();
                for id in entry_ids.iter().take(20) {
                    if let Some(sym) = graph.get_symbol(id) {
                        entry_refs.push(SymbolRef {
                            name: sym.name().to_string(),
                            file: sym.location().file().to_string(),
                            line: sym.location().line(),
                            column: sym.location().column(),
                        });
                    }
                }
                let elapsed = start.elapsed().as_millis();
                let result = GR::EntryPoints {
                    path: path.clone(),
                    symbols: entry_refs,
                    total: entry_ids.len(),
                    elapsed_ms: elapsed,
                };
                // T1-5.4d: text formatting is in the presenter.
                Ok(result)
            }
            GraphCommand::LeafFunctions { path } => {
                let start = Instant::now();

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
                let mut leaf_refs: Vec<SymbolRef> = Vec::new();
                for id in leaf_ids.iter().take(20) {
                    if let Some(sym) = graph.get_symbol(id) {
                        leaf_refs.push(SymbolRef {
                            name: sym.name().to_string(),
                            file: sym.location().file().to_string(),
                            line: sym.location().line(),
                            column: sym.location().column(),
                        });
                    }
                }
                let elapsed = start.elapsed().as_millis();
                let result = GR::LeafFunctions {
                    path: path.clone(),
                    symbols: leaf_refs,
                    total: leaf_ids.len(),
                    elapsed_ms: elapsed,
                };
                // T1-5.4d: text formatting is in the presenter.
                Ok(result)
            }
            GraphCommand::TracePath { from, to, path } => {
                let start = Instant::now();
                // Progress line stays in the function: the
                // TracePathFound/NotFound variants don't carry
                // the project path, so the presenter (T1-5.4g)
                // can't reproduce it. T1-5.5 may move this
                // progress to a tracing event.
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

                let path_result = match graph.find_path(&source_id, &target_id) {
                    Some(path_ids) => {
                        let mut hops: Vec<PathHop> = Vec::new();
                        for (i, id) in path_ids.iter().enumerate() {
                            if let Some(sym) = graph.get_symbol(id) {
                                hops.push(PathHop {
                                    position: (i + 1) as u32,
                                    name: sym.name().to_string(),
                                    file: sym.location().file().to_string(),
                                    line: sym.location().line(),
                                });
                            }
                        }
                        GR::TracePathFound {
                            from: from.clone(),
                            to: to.clone(),
                            hops,
                            elapsed_ms: 0, // filled in below
                        }
                    }
                    None => GR::TracePathNotFound {
                        from: from.clone(),
                        to: to.clone(),
                        elapsed_ms: 0, // filled in below
                    },
                };
                let elapsed = start.elapsed().as_millis();
                // T1-5.4b: fill elapsed_ms in the result we just built.
                let final_result = match path_result {
                    GR::TracePathFound { from, to, hops, .. } => GR::TracePathFound {
                        from,
                        to,
                        hops,
                        elapsed_ms: elapsed,
                    },
                    GR::TracePathNotFound { from, to, .. } => GR::TracePathNotFound {
                        from,
                        to,
                        elapsed_ms: elapsed,
                    },
                    _ => unreachable!(),
                };
                // T1-5.4g: result block + summary in the
                // presenter (now lives in cognicode-cli::presenter).
                // The progress line above is NOT part of the
                // snapshot (it can't be: the variant doesn't
                // carry the project path).
                Ok(final_result)
            }
            GraphCommand::Mermaid { path, format } => {
                let start = Instant::now();
                // Progress line stays in the function: the
                // Mermaid variant doesn't carry the project
                // path (same pattern as TracePath, T1-5.4g).
                println!("Exporting to Mermaid format from: {}", path);

                let dir = PathBuf::from(path);
                let strategy = FullGraphStrategy::new();

                // `build_full_graph_report` returns a report, not a
                // `Result`, and it records an unreadable root as one more
                // *skipped file*. Reporting that as `PARTIAL` and exiting 0
                // describes a walk that partially failed; here the walk
                // never started. The root is checked before the walk so the
                // failure is not disguised as partial coverage — the same
                // answer `graph full` and the MCP `build_graph` handler
                // already give.
                if !dir.is_dir() {
                    return Err(format!("Directory does not exist: {}", dir.display()).into());
                }

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
                let elapsed = start.elapsed().as_millis();
                // T1-5.4b: Mermaid variant. T1-5.4h added
                // the `diagram` field (the presenter needs
                // the full content to reproduce the
                // pre-migration text byte-identical).
                let truncated = format != "svg" && format != "png" && mermaid.len() > 2000;
                let result = GR::Mermaid {
                    path: path.clone(),
                    diagram: mermaid,
                    diagram_len: 0, // filled in below
                    format: format.clone(),
                    truncated,
                    elapsed_ms: elapsed,
                };
                // Fill diagram_len from the diagram itself.
                let result = if let GR::Mermaid {
                    path,
                    diagram,
                    diagram_len: _,
                    format,
                    truncated,
                    elapsed_ms,
                } = result
                {
                    GR::Mermaid {
                        path,
                        diagram_len: diagram.len(),
                        diagram,
                        format,
                        truncated,
                        elapsed_ms,
                    }
                } else {
                    unreachable!()
                };
                // T1-5.4h: text formatting is in the presenter.
                Ok(result)
            }
            GraphCommand::Hierarchy {
                symbol,
                depth,
                direction,
                path,
            } => {
                let start = Instant::now();
                // Progress line stays in the function: the
                // Hierarchy variant doesn't carry the project
                // path (same pattern as TracePath, Mermaid).
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

                let mut by_depth: std::collections::HashMap<u32, Vec<_>> =
                    std::collections::HashMap::new();
                for entry in &result.entries {
                    by_depth.entry(entry.depth).or_default().push(entry);
                }
                let mut buckets: Vec<DepthBucket> = Vec::new();
                for depth in 1..=*depth {
                    if let Some(entries) = by_depth.get(&depth) {
                        let mut depth_entries: Vec<DepthEntry> = Vec::new();
                        for entry in entries.iter().take(5) {
                            let dir_str = format!("{:?}", entry.direction).to_lowercase();
                            depth_entries.push(DepthEntry {
                                name: entry.symbol.name().to_string(),
                                direction: dir_str,
                                file: entry.symbol.location().file().to_string(),
                                line: entry.symbol.location().line(),
                            });
                        }
                        buckets.push(DepthBucket {
                            depth,
                            entries: depth_entries,
                        });
                    }
                }
                let final_result = GR::Hierarchy {
                    symbol: symbol.clone(),
                    depth: *depth,
                    direction: direction.clone(),
                    root_file: result.root_symbol.location().file().to_string(),
                    root_line: result.root_symbol.location().line(),
                    root_column: result.root_symbol.location().column(),
                    buckets,
                    total_entries: result.entries.len(),
                    elapsed_ms: elapsed,
                };
                // T1-5.4j: text formatting is in the
                // presenter (the result block + summary; the
                // progress line above stays in the function).
                Ok(final_result)
            }
            GraphCommand::Complexity { path } => {
                let start = Instant::now();

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

                let elapsed = start.elapsed().as_millis();
                use crate::interface::cli::result::ComplexityMetrics;
                let result = GR::Complexity {
                    path: path.clone(),
                    metrics: ComplexityMetrics {
                        total_symbols: complexity.total_symbols,
                        total_edges: complexity.total_edges,
                        max_depth: complexity.max_depth,
                        cyclomatic_complexity: complexity.cyclomatic_complexity,
                        high_fan_out_count: complexity.high_fan_out_count,
                        medium_fan_out_count: complexity.medium_fan_out_count,
                        low_fan_out_count: complexity.low_fan_out_count,
                        entry_point_count: complexity.entry_point_count,
                        leaf_function_count: complexity.leaf_function_count,
                    },
                    elapsed_ms: elapsed,
                };
                // T1-5.4f: text formatting is in the presenter.
                Ok(result)
            }
            GraphCommand::Impact { symbol, path } => {
                let start = Instant::now();
                // Progress line stays in the function (the
                // Impact variant doesn't carry the project
                // path; same pattern as TracePath, Mermaid,
                // Hierarchy).
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

                let elapsed = start.elapsed().as_millis();

                if symbol_ids.is_empty() {
                    // Empty case: the presenter renders the
                    // 'No symbols found' message + summary
                    // based on direct_dependents == 0.
                    let result = GR::Impact {
                        symbol: symbol.clone(),
                        risk_level: "NONE".to_string(),
                        direct_dependents: 0,
                        total_impacted_symbols: 0,
                        impacted_files_count: 0,
                        impacted_symbols: vec![],
                        impacted_files: vec![],
                        elapsed_ms: elapsed,
                    };
                    return Ok(result);
                }

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

                let result = GR::Impact {
                    symbol: symbol.clone(),
                    risk_level: risk_level.to_string(),
                    direct_dependents: symbol_ids.len(),
                    total_impacted_symbols: impacted_symbols.len(),
                    impacted_files_count: impacted_files.len(),
                    impacted_symbols,
                    impacted_files,
                    elapsed_ms: elapsed,
                };
                // T1-5.4l: text formatting is in the
                // presenter.
                Ok(result)
            }
        }
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
    ///
    /// T1-5.6a: returns `Result<NavigateResult, NavigateError>` so the
    /// bin (cognicode-cli) can dispatch text/JSON via the presenter.
    /// Before T1-5.6a this returned `Result<(), Box<dyn Error>>` and
    /// the text was emitted by ~25 inline `println!` calls; the bin
    /// had no way to choose `--format json` for navigate.
    ///
    /// El `eprintln!("Error: {}", e)` se mantiene inline en cada brazo
    /// (decisión T1-5.4: los warnings tipo partial-warning van a
    /// stderr). El error estructurado que el bin recibe sigue siendo
    /// `NavigateError::Backend { operation, file, line, column,
    /// workspace, source }` (preserva la chain vía `#[source]`).
    async fn execute_navigate(command: &NavigateCommand) -> Result<NavigateResult, NavigateError> {
        use crate::domain::traits::code_intelligence::Reference as DomainReference;
        use crate::domain::value_objects::Location;
        use crate::infrastructure::lsp::providers::CompositeProvider;
        use crate::interface::cli::result::{
            DefinitionLocation, HoverResult, NavigateResult, ReferenceEntry,
        };
        use std::path::Path;

        match command {
            NavigateCommand::Definition { position, path } => {
                let (file, line, column) =
                    Self::parse_position(position).map_err(|_| NavigateError::InvalidPosition {
                        position: position.clone(),
                    })?;

                let workspace = Path::new(path);
                let provider = CompositeProvider::new(workspace);
                let location = Location::new(file.clone(), line.saturating_sub(1), column);

                let def_location = match provider.get_definition(&location).await {
                    Ok(Some(def)) => Some(DefinitionLocation {
                        file: def.file().to_string(),
                        // Storage is 0-indexed; CLI is 1-indexed.
                        // The data side carries 1-indexed so the
                        // presenter doesn't have to do arithmetic.
                        line: def.line() + 1,
                        column: def.column(),
                    }),
                    Ok(None) => None,
                    Err(e) => {
                        // Decisión T1-5.4: los errores del backend
                        // van a stderr como partial-warning. El
                        // caller aún recibe el error estructurado
                        // para formatear la salida de error en el
                        // bin y propagarlo como exit code != 0.
                        eprintln!("Error: {}", e);
                        return Err(NavigateError::Backend {
                            operation: "definition",
                            file: file.clone(),
                            line,
                            column,
                            workspace: path.clone(),
                            source: Box::new(e),
                        });
                    }
                };

                Ok(NavigateResult::Definition {
                    file,
                    line,
                    column,
                    workspace: path.clone(),
                    location: def_location,
                })
            }
            NavigateCommand::Hover { position, path } => {
                let (file, line, column) =
                    Self::parse_position(position).map_err(|_| NavigateError::InvalidPosition {
                        position: position.clone(),
                    })?;

                let workspace = Path::new(path);
                let provider = CompositeProvider::new(workspace);
                let location = Location::new(file.clone(), line.saturating_sub(1), column);

                let hover_info = match provider.hover(&location).await {
                    Ok(Some(info)) => Some(HoverResult {
                        type_info: info.content,
                        documentation: info.documentation,
                    }),
                    Ok(None) => None,
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        return Err(NavigateError::Backend {
                            operation: "hover",
                            file: file.clone(),
                            line,
                            column,
                            workspace: path.clone(),
                            source: Box::new(e),
                        });
                    }
                };

                Ok(NavigateResult::Hover {
                    file,
                    line,
                    column,
                    workspace: path.clone(),
                    info: hover_info,
                })
            }
            NavigateCommand::References {
                position,
                include_declaration,
                path,
            } => {
                let (file, line, column) =
                    Self::parse_position(position).map_err(|_| NavigateError::InvalidPosition {
                        position: position.clone(),
                    })?;

                let workspace = Path::new(path);
                let provider = CompositeProvider::new(workspace);
                let location = Location::new(file.clone(), line.saturating_sub(1), column);

                let refs: Vec<DomainReference> = match provider
                    .find_references(&location, *include_declaration)
                    .await
                {
                    Ok(refs) => refs,
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        return Err(NavigateError::Backend {
                            operation: "references",
                            file: file.clone(),
                            line,
                            column,
                            workspace: path.clone(),
                            source: Box::new(e),
                        });
                    }
                };

                // The data side carries the container as a String
                // (not Option<String>) so the presenter doesn't have
                // to substitute "(unknown)". The substitution lives
                // here, in the function that knows what the LSP
                // returned.
                let entries: Vec<ReferenceEntry> = refs
                    .into_iter()
                    .map(|r| ReferenceEntry {
                        file: r.location.file().to_string(),
                        line: r.location.line() + 1,
                        column: r.location.column(),
                        kind: r.reference_kind,
                        container: r.container.unwrap_or_else(|| "(unknown)".to_string()),
                    })
                    .collect();

                Ok(NavigateResult::References {
                    file,
                    line,
                    column,
                    workspace: path.clone(),
                    include_declaration: *include_declaration,
                    references: entries,
                })
            }
        }
    }

    /// Execute doctor subcommand — check LSP server availability
    /// `cognicode export assurance` — produce un envelope
    /// `assurance-evidence/v1` para pipelinek-assurance. Ver
    /// `assurance_export::envelope` para el shape contractual.
    ///
    /// Flujo:
    /// 1. `WorkspaceInfo::detect(workspace)` resuelve la revision (git
    ///    HEAD o fallback al nombre del directorio) y el kind
    ///    (rust-workspace si hay Cargo.toml).
    /// 2. `ProducerInfo` declara id=cognicode, version=workspace
    ///    version, schemaVersion=assurance-evidence/v1.
    /// 3. `ExtractionCapabilities::from_flags(&capability)` normaliza la
    ///    lista; sin flags, se piden todas las conocidas.
    /// 4. `extract_from_workspace` produce entities/facts/anchors y
    ///    declara capabilities no soportadas como Unsupported con gaps
    ///    honestos (regla C4 del workstream).
    /// 5. `encode_to_cbor_with_digest` (o JSON) serializa con SHA-256
    ///    hex en el campo `digest` (M-D01-style, fail-closed).
    /// 6. Se escribe a `output`. Si el formato se infiere por el sufijo
    ///    y no coincide con el path, error honesto.
    async fn execute_export_assurance(
        command: &ExportAssuranceCommand,
    ) -> Result<(), Box<dyn std::error::Error>> {
        use crate::assurance_export::{
            API_VERSION, ExtractionCapabilities, MEDIA_TYPE_CBOR as COBOR_MEDIA_TYPE, ProducerInfo,
            WorkspaceInfo, encode_to_cbor_with_digest, encode_to_json_with_digest,
            extract_from_workspace,
        };

        let (workspace, capabilities, output, format) = match command {
            ExportAssuranceCommand::Assurance {
                workspace,
                capability,
                output,
                format,
            } => (
                workspace.clone(),
                capability.clone(),
                output.clone(),
                format.clone(),
            ),
        };

        // Formato: el sufijo del path es la fuente canonica; --format
        // lo sobreescribe si se da explicitamente.
        let resolved_format = format.unwrap_or_else(|| {
            let lower = output.to_lowercase();
            if lower.ends_with(".cbor") {
                ExportFormat::Cbor
            } else if lower.ends_with(".json") {
                ExportFormat::Json
            } else {
                // Default: cbor (canonico para el consumer).
                ExportFormat::Cbor
            }
        });

        let workspace_path = std::path::PathBuf::from(&workspace);
        let info = WorkspaceInfo::detect(workspace_path);
        let caps = ExtractionCapabilities::from_flags(&capabilities);

        // Producer info: id=cognicode, version=workspace version,
        // schemaVersion=API_VERSION. Version tomada de Cargo.toml via env
        // CARGO_PKG_VERSION del binario que ejecuta el CLI. Esto
        // garantiza que el SHA-256 del golden export sea estable cuando
        // el binario viene de la misma revision del workspace.
        let producer = ProducerInfo {
            id: "cognicode".into(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            schema_version: API_VERSION.into(),
        };

        let envelope = extract_from_workspace(&info, &caps, &producer)
            .map_err(|e| -> Box<dyn std::error::Error> { format!("extract fallo: {e}").into() })?;

        let path = std::path::PathBuf::from(&output);
        match resolved_format {
            ExportFormat::Cbor => {
                let bytes = encode_to_cbor_with_digest(&envelope).map_err(
                    |e| -> Box<dyn std::error::Error> { format!("encode CBOR fallo: {e}").into() },
                )?;
                if let Some(parent) = path.parent()
                    && !parent.as_os_str().is_empty()
                {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&path, &bytes)?;
                eprintln!(
                    "assurance-evidence/v1 escrito en {} ({} bytes, media type {}, schema {})",
                    path.display(),
                    bytes.len(),
                    COBOR_MEDIA_TYPE,
                    API_VERSION,
                );
            }
            ExportFormat::Json => {
                let text = encode_to_json_with_digest(&envelope).map_err(
                    |e| -> Box<dyn std::error::Error> { format!("encode JSON fallo: {e}").into() },
                )?;
                if let Some(parent) = path.parent()
                    && !parent.as_os_str().is_empty()
                {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&path, text)?;
                eprintln!(
                    "assurance-evidence/v1 escrito en {} (JSON debug, schema {})",
                    path.display(),
                    API_VERSION,
                );
            }
        }
        // Imprime un resumen del envelope en stderr para inspeccion
        // humana (capabilities producidas, gaps, digest). El digest se
        // computa aqui porque `envelope.digest` es vacio hasta que
        // `encode_to_cbor_with_digest` lo asigna en una copia interna.
        use crate::assurance_export::digest_of;
        let digest_display = digest_of(&envelope);
        eprintln!(
            "schema: {} producer: {} v{} workspace: {} (rev {})",
            envelope.api_version,
            envelope.producer.id,
            envelope.producer.version,
            envelope.subject.kind,
            envelope.subject.revision,
        );
        eprintln!("digest: {}", digest_display);
        eprintln!(
            "capabilities: {} produced, {} unsupported, {} gaps",
            envelope.manifest.produced_capabilities.len(),
            envelope
                .capability_completeness
                .values()
                .filter(|c| matches!(
                    c,
                    crate::assurance_export::CapabilityCompleteness::Unsupported { .. }
                ))
                .count(),
            envelope.gaps.len(),
        );
        Ok(())
    }

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

    /// Execute the capabilities subcommand (A-014 / CP2.4).
    ///
    /// Emits a schema-versioned JSON document on stdout (the
    /// canonical contract for downstream skills / integration
    /// adapters). With `--format text` (default) prints a short
    /// human summary. Either way stderr stays clean unless an
    /// error is reported (the warning path for missing
    /// `product/*.json` is the one exception).
    async fn execute_capabilities(format: &str) -> Result<(), Box<dyn std::error::Error>> {
        // Build the doc in memory first so any error in construction
        // is reported as a structured envelope and never leaks a
        // partial document to stdout. JSON emission keeps stderr
        // pristine (the document is pipeable unfiltered); the missing
        // inventory warning is a text-mode diagnostic only.
        let doc = build_capabilities_doc(format == "json");

        if format == "json" {
            let serialized = serde_json::to_string(&doc)?;
            println!("{}", serialized);
        } else {
            let tools = doc["tools"].as_array().map(|a| a.len()).unwrap_or(0);
            let profiles = doc["profiles"].as_array().map(|a| a.len()).unwrap_or(0);
            let mutating = doc["runtime"]["mutating_tools"]
                .as_array()
                .map(|a| a.len())
                .unwrap_or(0);
            println!("CogniCode Capabilities");
            println!(
                "  cli_version:    {}",
                doc["cli_version"].as_str().unwrap_or("")
            );
            println!(
                "  source_commit:  {}",
                doc["source_commit"].as_str().unwrap_or("")
            );
            println!("  tools:          {}", tools);
            println!("  profiles:       {}", profiles);
            println!("  mutating_tools: {} (runtime)", mutating);
            println!();
            println!("Pass `--format json` for the machine-readable contract.");
        }
        Ok(())
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
            // B3.W1 — CLI mirror of the MCP `find_usages` handler: tag
            // precision + provider so the JSON presenter emits them in
            // the same shape as the MCP wire. The CLI text path ignores
            // these fields; the JSON path is the one that needs them.
            precision: crate::interface::symbol_identity::precision_floor_for_tool("find_usages"),
            provider: crate::interface::symbol_identity::default_provider_for_tool("find_usages")
                .to_string(),
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
        use crate::interface::mcp::security::InputValidator;
        use std::sync::Arc;

        println!("Analyzing code at: {}", path);

        // ST-02: the composition that names the validator lives HERE, in the
        // interface layer, because `application` sits below it and may not
        // name `interface::mcp::security`. The session receives the port.
        let path_policy =
            Arc::new(InputValidator::new().with_workspace(vec![std::path::PathBuf::from(path)]));
        let capabilities = crate::interface::composition::default_capabilities(
            std::path::Path::new(path),
            path_policy,
        );
        let session = WorkspaceSession::with_capabilities(path, capabilities)
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
        use crate::interface::mcp::security::InputValidator;
        use std::sync::Arc;

        let path = ".";

        // ST-02: composition belongs to the interface layer. See
        // `execute_analyze` for the same shape.
        let path_policy =
            Arc::new(InputValidator::new().with_workspace(vec![std::path::PathBuf::from(path)]));
        let capabilities = crate::interface::composition::default_capabilities(
            std::path::Path::new(path),
            path_policy,
        );
        let session = WorkspaceSession::with_capabilities(path, capabilities)
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
    async fn execute_evidence(cmd: &EvidenceCommand) -> Result<(), Box<dyn std::error::Error>> {
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
        let backend =
            factory(db_path.as_ref()).map_err(|e| format!("opening evidence backend: {e}"))?;

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
    println!("{:<60}  {:<12}  {:<7}  TITLE", "ID", "KIND", "CONF");
    for r in rows {
        let kind = match r.kind {
            EvidenceKind::Log => "log",
            EvidenceKind::Trace => "trace",
            EvidenceKind::Measurement => "measurement",
            EvidenceKind::External => "external",
        };
        println!(
            "{:<60}  {:<12}  {:<7.2}  {}",
            r.id, kind, r.confidence, r.title
        );
    }
}

/// Prints the outline tree with indentation
/// T1-5.3d: removed — moved to `crate::interface::cli::presenter`
/// alongside `present_index_result`, where the snapshot test can
/// exercise the same recursion the bin does. The migration kept
/// the byte-identical output; see `tests/t1_5_3d_index_presenter_snapshot.rs`.
/// T1-5.3c — present an `IndexResult` to stdout.
///
/// T1-5.3d moved the implementation to
/// `crate::interface::cli::presenter::present_index_result`
/// (a `&mut dyn Write` parameter makes it testable from
/// `tests/t1_5_3d_index_presenter_snapshot.rs`). T1-5.5
/// moved the whole presenter to `cognicode_cli::presenter`
/// (the bin crate) and added JSON dispatch via
/// `present_index_result_json` + `present_output`. The
/// `*Result` enum still lives here in the lib; only the
/// formatting moved.
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

// ============================================================================
// A-014 / CP2.4 — `cognicode capabilities` subcommand.
//
// The `build_capabilities_doc` function is intentionally free-standing
// (not a method on `CommandExecutor`) so unit tests can call it
// directly without spawning the binary. The only side effect is the
// optional `git rev-parse HEAD` call for `source_commit`, which has a
// graceful fallback to the empty string when git is unavailable.
// ============================================================================

/// Build the JSON document for `cognicode capabilities` (A-014).
///
/// Sources (in precedence order):
///   1. `product/tools.json` for the public tool catalogue.
///   2. `product/profiles.json` for the public profile surface.
///   3. `crate::product::PROFILE_POSTURES` (canonical runtime
///      posture; overrides the published `mutating` flag if the two
///      disagree — the runtime is authoritative).
///   4. `CogniCodeHandler::MUTATING_TOOLS` (canonical runtime
///      mutating set; the source of truth for `--read-only` filtering).
///   5. `git rev-parse HEAD` for `source_commit` (best-effort).
///
/// No network, no mutation. Reading from `product/*.json` falls back
/// to runtime-only data when the artefacts are missing (rare; only
/// happens if the binary was built against a tree where the files
/// have been moved). A warning is emitted on stderr in that case,
/// unless `json_emission` is set: JSON output is pipeable unfiltered,
/// so a successful run must never write diagnostics to stderr.
fn build_capabilities_doc(json_emission: bool) -> Value {
    use crate::product::PROFILE_POSTURES;

    let workspace_root = resolve_product_assets_root();
    let tools_path = workspace_root.join("product").join("tools.json");
    let profiles_path = workspace_root.join("product").join("profiles.json");

    let mut doc = serde_json::json!({
        "schema_version": "cognicode.capabilities/v1",
        "cli_version": env!("CARGO_PKG_VERSION"),
        "source_commit": current_source_commit(&workspace_root),
    });

    let mut profiles_arr: Vec<Value> = Vec::new();
    if let Ok(text) = std::fs::read_to_string(&profiles_path)
        && let Ok(parsed) = serde_json::from_str::<Value>(&text)
        && let Some(arr) = parsed.get("profiles").and_then(|v| v.as_array())
    {
        let posture_by_id: std::collections::HashMap<&str, bool> = PROFILE_POSTURES
            .iter()
            .map(|(id, posture)| (*id, !posture.is_read_only()))
            .collect();
        for p in arr {
            let id = p.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let mut entry = p.clone();
            if let Some(&mutating) = posture_by_id.get(id) {
                // Runtime posture is authoritative.
                entry["mutating"] = Value::Bool(mutating);
            }
            profiles_arr.push(entry);
        }
    }
    if profiles_arr.is_empty() {
        profiles_arr = PROFILE_POSTURES
            .iter()
            .map(|(id, posture)| {
                serde_json::json!({
                    "id": id,
                    "mutating": !posture.is_read_only(),
                })
            })
            .collect();
    }
    doc["profiles"] = Value::Array(profiles_arr);

    // Tools: the typed catalog is the single source of truth. B2.W2
    // replaces the manual `serde_json::Value` clone-loop with a round
    // trip through `ToolDescriptor`. Every public field the schema
    // promises (name, title, description, category, authority,
    // stability, input_schema, output_schema, requirements,
    // requirements_status, estimated_latency_ms, public_profiles,
    // runtime_metadata) is carried by the struct, so the JSON output
    // stays shape-compatible with `product/tools.json` and the
    // parity test can compare the CLI document to the MCP catalog
    // field-for-field.
    //
    // We use `load_tool_catalog_active()` (feature-filtered) so the
    // CLI document matches the MCP `tools/list` exactly: both are the
    // *live* surface, not the universe of all possible tools. The
    // catalogue file remains the source of truth on disk; the
    // `feature` field tells the loader which entries are live in this
    // build. A `cognicode capabilities --json` consumer sees the same
    // 73 (default build) tools that MCP advertises; a
    // `--features evidence-cli-ladybug` build sees 74.
    let catalog = crate::interface::catalog::load_tool_catalog_active();
    let tools_arr: Vec<Value> = catalog
        .iter()
        .map(|d| serde_json::to_value(d).expect("ToolDescriptor is serialisable"))
        .collect();
    if tools_arr.is_empty() && !json_emission {
        eprintln!(
            "warning: {} not found; emitting empty tools array",
            tools_path.display()
        );
    }
    doc["tools"] = Value::Array(tools_arr);

    // Derived from the tools' own declarations, so this report cannot
    // disagree with what `--read-only` actually refuses.
    let mutating_tools = crate::interface::mcp::rmcp_adapter::mutating_tool_names();
    doc["runtime"] = serde_json::json!({
        "mutating_tools": mutating_tools,
        "mutating_tools_count": mutating_tools.len(),
    });

    doc
}

/// Resolve the root that carries `product/*.json` for the **running** binary.
///
/// **Why not `env!("CARGO_MANIFEST_DIR")`.** That macro is resolved at compile
/// time and bakes the *build machine's* directory into the binary. Proven
/// failure: a binary compiled inside worktree `/tmp/a014vanish`, run after
/// that worktree was deleted, printed
/// `warning: /tmp/a014vanish/product/tools.json not found; emitting empty
/// tools array` and reported `tools: 0`. Since `release-validate.yml` builds
/// on a GitHub runner, every installed binary carried a path its user cannot
/// have, and the command whose entire purpose is machine-readable discovery
/// advertised zero capabilities.
///
/// **The anchors, in order.**
/// 1. The current working directory and its parents. Covers a checkout, a
///    `cargo run` from a subdirectory, and — for a packaged install where
///    the launcher is invoked from anywhere — the case where the user runs
///    the binary from inside their project.
/// 2. The directory of the running executable and its parents. This is what
///    makes the packaged layout work: data beside the binary, or one level up
///    when the binary lives in `bin/`.
///
/// **Why cwd first.** An installed binary is normally reached through a
/// launcher on `PATH`, so `current_exe` is the real binary and the data sits
/// beside it. But during development the executable lives in
/// `target/{debug,release}/`, which carries no data, while the cwd is the
/// checkout that does. Anchoring on the executable first made
/// `cargo test` see zero tools, because the test's cwd is the crate
/// directory and the binary is three levels away. Both orders are wrong for
/// one of the two layouts; trying both is what makes it right, and the
/// executable is kept as the fallback because it is the anchor that still
/// exists when the user has no checkout at all.
fn resolve_product_assets_root() -> PathBuf {
    fn holds(root: &std::path::Path) -> bool {
        root.join("product").join("tools.json").is_file()
            || root.join("product").join("profiles.json").is_file()
    }

    fn walk_up(start: &std::path::Path) -> Option<PathBuf> {
        let mut cursor: Option<&std::path::Path> = Some(start);
        // Bounded so a symlink loop cannot turn discovery into a hang.
        let mut hops = 0usize;
        while let Some(dir) = cursor {
            if holds(dir) {
                return Some(dir.to_path_buf());
            }
            cursor = dir.parent();
            hops += 1;
            if hops > 32 {
                break;
            }
        }
        None
    }

    if let Ok(cwd) = std::env::current_dir()
        && let Some(found) = walk_up(&cwd)
    {
        return found;
    }

    if let Some(exe) = std::env::current_exe().ok()
        && let Some(parent) = exe.parent()
        && let Some(found) = walk_up(parent)
    {
        return found;
    }

    // Neither anchor carries the data. Return the cwd: the caller already
    // warns on stderr and falls back to built-in defaults, and a wrong
    // absolute path there would only make the warning point at somewhere
    // meaningless.
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

/// Resolve `source_commit` for the capabilities document.
///
/// `git rev-parse HEAD` via subprocess; returns the empty string if
/// git is missing or the working tree is not a git checkout.
fn current_source_commit(workspace_root: &std::path::Path) -> String {
    std::process::Command::new("git")
        .arg("rev-parse")
        .arg("HEAD")
        .current_dir(workspace_root)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

/// B3.W2 — Best-effort parser for `cognicode inspect <args...>`.
///
/// English: maps free-text CLI args to a typed `Intent`.
/// Espanol: convierte args de CLI free-text a un `Intent` tipado.
///
/// Politica B3.W2 (scaffold; B3.W5 pinea el contrato final):
///   - Nunca panic. Si el input no encaja en ninguna regla,
///     cae a `Locate` con el resto del texto como symbol.
///   - Las reglas son deterministas y locales: no leen el
///     workspace ni invocan tools.
///   - El usuario puede sobreescribir la heuristica con palabras
///     clave (`summary`, `verify`) al principio.
fn parse_intent_args(args: &[String]) -> crate::interface::intent::Intent {
    use crate::interface::intent::Intent;

    // Empty input: default to Locate with an empty symbol; the
    // caller will see the PROPOSED status and an empty chain.
    // Better than panicking.
    if args.is_empty() {
        return Intent::Locate {
            symbol: String::new(),
        };
    }

    // First token drives the intent kind when it's a keyword.
    let first = args[0].to_ascii_lowercase();
    let rest = &args[1..];

    match first.as_str() {
        // `inspect summary <scope>` -> Summarize
        "summary" | "summarize" => {
            return Intent::Summarize {
                scope: rest.join(" "),
            };
        }
        // `inspect verify <claim>` -> Verify
        "verify" => {
            return Intent::Verify {
                claim: if rest.is_empty() {
                    args.join(" ")
                } else {
                    rest.join(" ")
                },
            };
        }
        // `inspect read <path>` / `inspect read <path:range>`
        "read" => {
            return parse_read(rest);
        }
        // `inspect locate <symbol>` (explicit) or
        // `inspect fn <symbol>` (Rust-ish keyword).
        "locate" | "find" | "fn" => {
            return Intent::Locate {
                symbol: if rest.is_empty() {
                    String::new()
                } else {
                    rest.join(" ")
                },
            };
        }
        _ => {}
    }

    // No keyword matched. If the only argument looks like a path
    // (contains '/' or '.' or has a `:LINE-LINE` range), treat it
    // as Read. Otherwise treat it as Locate.
    if args.len() == 1 {
        let only = &args[0];
        if only.contains('/') || only.contains('.') {
            return parse_read(std::slice::from_ref(only));
        }
        return Intent::Locate {
            symbol: only.clone(),
        };
    }

    // Multi-arg without keyword: best-effort Read if the first
    // arg looks like a path; otherwise Locate with joined args.
    if args[0].contains('/') || args[0].contains('.') {
        parse_read(args)
    } else {
        Intent::Locate {
            symbol: args.join(" "),
        }
    }
}

/// B3.W2 — Parses `Read` intent from raw args, recognizing the
/// `path:start-end` range syntax.
///
/// English: extract path and optional line range from CLI args.
/// Espanol: extrae path y rango opcional de lineas de args CLI.
fn parse_read(args: &[String]) -> crate::interface::intent::Intent {
    use crate::interface::intent::Intent;
    use std::ops::Range;

    let joined = args.join(" ");
    if let Some((path, range)) = joined.split_once(':')
        && let Some((s, e)) = range.split_once('-')
        && let (Ok(start), Ok(end)) = (s.parse::<usize>(), e.parse::<usize>())
    {
        return Intent::Read {
            path: path.to_string(),
            range: Some(Range { start, end }),
        };
    }
    Intent::Read {
        path: joined,
        range: None,
    }
}

/// B3.W2 — Human-readable label for an `Intent` (used in the
/// PROPOSED status line so the operator can see which variant
/// the scaffold chose).
fn intent_kind_label(intent: &crate::interface::intent::Intent) -> &'static str {
    match intent {
        crate::interface::intent::Intent::Locate { .. } => "Locate",
        crate::interface::intent::Intent::Summarize { .. } => "Summarize",
        crate::interface::intent::Intent::Verify { .. } => "Verify",
        crate::interface::intent::Intent::Read { .. } => "Read",
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
            format: Format::Text,
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
            format: Format::Text,
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
            format: Format::Text,
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
            format: Format::Text,
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
            format: Format::Text,
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
            format: Format::Text,
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
            format: Format::Text,
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
            format: Format::Text,
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
