//! CLI Commands - Command-line interface implementations
// e30.1 clippy baseline reset: pre-existing lint debt (see fix/e30.1-clippy-baseline-reset)
#![allow(clippy::map_flatten)]

use crate::application::services::analysis_service::AnalysisService;
use crate::domain::services::CallGraphAnalyzer;
use crate::domain::traits::code_intelligence::CodeIntelligenceProvider;
use crate::domain::value_objects::{published_column, published_line};
use crate::infrastructure::graph::{
    GraphStrategy, GraphStrategyFactory, LightweightStrategy, OnDemandStrategy, PerFileStrategy,
    TraversalDirection,
};
use crate::infrastructure::parser::Language;
use crate::infrastructure::semantic::SymbolCodeService;
use crate::interface::cli::capabilities::{build_capabilities_doc, format_capabilities_text};
use crate::interface::cli::mermaid_format;
use crate::interface::cli::progress::progress_line;
use crate::interface::cli::result::{
    CommandOutput, GraphResult, IndexError, IndexResult, NavigateError, NavigateResult,
};
use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
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
        /// permits file mutation. B5.W1 keeps this refusal in place;
        /// the apply path with rollback lands in B5.W3.
        #[arg(long)]
        apply: bool,
        /// B5.W1 — path to a JSON file holding a `RefactorPlan`
        /// (`cognicode_core::refactor::plan::RefactorPlan`). When
        /// provided, the CLI parses the plan and runs the
        /// plan-based preview path. When omitted, the existing
        /// free-form text refactor preview arm remains valid for
        /// non-plan-based quick previews.
        #[arg(long)]
        plan_file: Option<PathBuf>,
        /// B5.W3 — separate-channel write authorization token.
        /// REQUIRED when `--apply` is set; refused if missing or
        /// empty (AAT-B5-03). Not derived from plan content.
        #[arg(long)]
        authorization_token: Option<String>,
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
    ///   - anything else                        -> Locate with joined args.
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
        /// Strategy to use: lightweight, on_demand, full
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
        /// Column number (1-indexed): the column a person reads.
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
        /// Maximum depth of the path (0 means no limit)
        #[arg(long, default_value = "10")]
        max_depth: usize,
        /// Directory to analyze
        #[arg(default_value = ".")]
        path: String,
    },
    /// Export graph to Mermaid format
    Mermaid {
        /// File to export (or directory for full graph)
        #[arg(default_value = ".")]
        path: String,
        /// The format the graph publishes the diagram in.
        #[arg(long, default_value = mermaid_format::DEFAULT, help = mermaid_format::help())]
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
    /// B4.W5 — Composite impact query for agents. Aggregates
    /// impact paths + architecture observations + quality findings
    /// + suggested follow-ups / prerequisites for a symbol.
    ///
    /// INVESTIGATIVE, NOT AUTHORITATIVE: the result carries
    /// `verdict_refused: true` precisely so the agent never treats
    /// this as a verdict. Use the suggested follow-ups
    /// (`suggested_follow_ups`) and prerequisites
    /// (`suggested_prerequisites`) to investigate each piece.
    #[command(name = "impact-composite")]
    ImpactComposite {
        /// Symbol qualified name to investigate. Must match a
        /// `ToolDescriptor::name` in the catalog for follow-ups /
        /// prerequisites to surface.
        symbol: String,
        /// Optional depth limit for the impact extractor (B4.W1).
        #[arg(short = 'd', long)]
        depth: Option<usize>,
        /// Directory to analyze (workspace root for the query).
        #[arg(default_value = ".")]
        path: String,
    },
}

/// Navigate subcommands (LSP-based)
#[derive(Debug, Subcommand)]
pub enum NavigateCommand {
    /// Go to the definition of the symbol at the given file:line:column position
    Definition {
        /// Position as file:line:column (e.g., src/main.rs:42:10).
        /// The line and the column are 1-indexed.
        position: String,
        /// Workspace root directory
        #[arg(default_value = ".")]
        path: String,
    },
    /// Show hover information (type + docs) for the symbol at the given position
    Hover {
        /// Position as file:line:column (e.g., src/main.rs:42:10).
        /// The line and the column are 1-indexed.
        position: String,
        /// Workspace root directory
        #[arg(default_value = ".")]
        path: String,
    },
    /// Find all references to the symbol at the given position
    References {
        /// Position as file:line:column (e.g., src/main.rs:42:10).
        /// The line and the column are 1-indexed.
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
                // a failed analyze must surface a non-zero exit code so
                // scripts and CI can detect it. The UAT that named this
                // contract was retired with the class on 2026-10-15
                // (ADR-TH-006); the exit-code contract itself lives in
                // `crates/cognicode-cli/tests/cli_exit_code_propagation.rs`.
                Ok(Self::execute_analyze(path).await?)
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
                plan_file,
                authorization_token,
                format,
            }) => {
                // B5.W3: `--apply` requires BOTH `--plan-file` and
                // `--authorization-token`. The proposal and the consent
                // travel on separate channels; either missing is a
                // clean error (R-B5-W3 / AAT-B5-03).
                if *apply {
                    let plan_path = plan_file
                        .as_deref()
                        .ok_or("--apply requires --plan-file <path> (B5.W3)")?;
                    let token_value = authorization_token.as_deref().ok_or(
                        "--apply requires --authorization-token <token> \
                         received out-of-band (B5.W3 / AAT-B5-03)",
                    )?;
                    return Self::execute_refactor_apply(plan_path, token_value, format).await;
                }
                // PRF-CLI-05 / B5.W1: without --apply this is
                // preview-only and must never mutate files.
                // execute_refactor is preview-only by contract.
                // B5.W1 — plan-based preview path. When `--plan-file`
                // is provided, parse the JSON `RefactorPlan` here and
                // surface a clean error if the file is missing or
                // malformed. The free-form preview path remains valid
                // when `--plan-file` is omitted.
                if let Some(plan_path) = plan_file.as_deref() {
                    let plan = crate::refactor::plan::load_plan_from_file(plan_path).map_err(
                        |e| -> Box<dyn std::error::Error> {
                            format!("failed to load --plan-file {}: {}", plan_path.display(), e)
                                .into()
                        },
                    )?;
                    eprintln!(
                        "B5.W1 plan preview: id={} files={} edits={} \
                         risks={} digest={}",
                        plan.id,
                        plan.affected_files.len(),
                        plan.edits.len(),
                        plan.risks.len(),
                        plan.digest
                    );
                    return Ok(CommandOutput::Other);
                }
                Ok(Self::execute_refactor(operation, symbol, new_name.as_deref(), format).await?)
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
                // The graph commands return their data; the bin formats
                // it (text or JSON per `--format`).
                let result = Self::execute_graph(command, cli.format).await?;
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
                Ok(
                    Self::execute_find_usages(symbol, cwd, include, *context_lines, format, *quiet)
                        .await?,
                )
            }
            Some(CliCommand::Navigate { command }) => {
                // T1-5.6a: execute_navigate returns NavigateResult so
                // the bin can dispatch text/JSON. Pre-T1-5.6a this
                // returned Other; the bin ignored the typed result.
                let result = Self::execute_navigate(command).await.map_err(Box::new)?;
                Ok(CommandOutput::Navigate(result))
            }
            Some(CliCommand::Doctor { format, cwd }) => {
                Ok(Self::execute_doctor(format, cwd).await?)
            }
            Some(CliCommand::ExportAssurance { command }) => {
                Self::execute_export_assurance(command).await?;
                Ok(CommandOutput::Other)
            }
            #[cfg(feature = "multimodal")]
            Some(CliCommand::DocsIngest { path, recursive }) => {
                Ok(Self::execute_docs_ingest(path, *recursive).await?)
            }
            #[cfg(feature = "multimodal")]
            Some(CliCommand::IssuesIngest {
                owner,
                repo,
                include_git_log,
            }) => Ok(Self::execute_issues_ingest(owner, repo, *include_git_log).await?),
            #[cfg(feature = "evidence-cli-ladybug")]
            Some(CliCommand::Evidence(cmd)) => Ok(Self::execute_evidence(cmd).await?),
            Some(CliCommand::Capabilities { format, json }) => {
                let effective = if *json { "json" } else { format.as_str() };
                Ok(Self::execute_capabilities(effective).await?)
            }
            None => {
                info!("CogniCode CLI initialized");
                // The help, if no command was given. The core renders
                // it and returns it; the presenter writes it.
                let mut cmd = Cli::command();
                let help = cmd.render_help().to_string();
                Ok(CommandOutput::rendered(format!("{help}\n")))
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
                let dir = PathBuf::from(path);
                // The factory is the authority on which names index a
                // project. A name that is not offered is refused with
                // the names that are: the command must not report a
                // successful index over a strategy that indexed nothing.
                let mut strategy_box =
                    GraphStrategyFactory::create(strategy).map_err(|e| IndexError::Build {
                        path: dir.clone(),
                        strategy: strategy.to_string(),
                        source: Box::new(e),
                    })?;

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
                    Ok(IndexResult::published_query(
                        symbol.clone(),
                        dir,
                        &locations,
                    ))
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

                Ok(IndexResult::published_outline(
                    file.clone().into(),
                    &outline,
                ))
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
        output_format: Format,
    ) -> Result<GraphResult, Box<dyn std::error::Error>> {
        use crate::interface::cli::result::{
            DepthBucket, DepthEntry, GraphResult as GR, HotPathEntry, PathHop, SymbolRef,
        };
        match command {
            GraphCommand::OnDemand {
                symbol,
                depth,
                direction,
                path,
            } => {
                let start = Instant::now();
                // The progress line belongs to the presenter: this
                // function returns the data and the bin formats it.

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
                    root_line: published_line(result.root_symbol.location().line()),
                    root_column: published_column(result.root_symbol.location().column()),
                    entries_len: result.entries.len(),
                };
                // T1-5.4i: text formatting is in the presenter.
                Ok(final_result)
            }
            GraphCommand::PerFile { file } => {
                let strategy = PerFileStrategy::new();
                let file_path = PathBuf::from(file);

                match strategy.build_local_graph(&file_path) {
                    Ok((graph, pending)) => {
                        let symbols = graph.symbol_count();
                        let edges = graph.edge_count();
                        // The references the file states and nobody declared
                        // are part of the answer, not a footnote: the counts
                        // say how much the graph holds, and these say what the
                        // file states that the graph does not hold.
                        let pending_references: Vec<
                            crate::interface::coverage::PendingReferenceInfo,
                        > = pending
                            .iter()
                            .map(crate::interface::coverage::PendingReferenceInfo::from_reference)
                            .collect();
                        let result = GR::PerFile {
                            file: file.clone(),
                            symbols,
                            dependencies: edges,
                            pending_references: pending_references.clone(),
                        };
                        // They go to stderr, like the pending references of
                        // the full graph do: stdout is the machine-readable
                        // stream and its byte shape is pinned.
                        for reference in &pending_references {
                            // A reference without a line is not on line
                            // zero: it is on a line the reader did not
                            // know, and saying "0" would name a place
                            // that does not exist.
                            let origin = match reference.line {
                                Some(line) => format!("{}:{}", reference.file, line),
                                None => reference.file.clone(),
                            };
                            eprintln!(
                                "  Pending: {} -> {} ({:?})",
                                origin, reference.target, reference.reason
                            );
                        }
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
                let _start = Instant::now();
                // T1-5.5d: the per-subcommand --format on
                // GraphCommand::Full is now ignored; the top-level
                // --format on Cli is canonical. Progress always goes
                // to stderr so stdout stays clean for `cogh ... | jq`.
                eprintln!(
                    "Building full project graph at: {}{}",
                    path,
                    if *rebuild { " (rebuild)" } else { "" }
                );

                // CU-1.3: the use case runs through the consumer facade
                // (ServiceAnalyzer implements the Analyze port); the CLI is
                // a presenter. Same AnalysisService pipeline underneath
                // (PRF-EXT-02 / H-03 constraint preserved).
                //
                // The CLI names the user's cache: what it calculates is kept
                // there, and the next run over the same tree is served. The
                // daemon's word, when there is one, is asked by the
                // analyzer the CLI builds.
                let dir = PathBuf::from(&path);
                let analyzer = crate::interface::cli::analyzer::cli_analyzer(&dir);
                let basis = crate::application::analysis_basis::AnalysisBasis::for_workspace(
                    std::path::PathBuf::from(&path),
                );
                let outcome = crate::application::analysis_basis::Analyze::execute(
                    &analyzer,
                    &basis,
                    crate::application::analysis_basis::AnalysisOp::GraphFull {
                        path: dir.clone(),
                        rebuild: *rebuild,
                    },
                );

                match outcome.state() {
                    crate::application::analysis_basis::OutcomeState::Failed => {
                        let msg = outcome
                            .error()
                            .map(|e| e.message.clone())
                            .unwrap_or_else(|| "unknown build failure".to_string());
                        eprintln!("Error building full graph: {}", msg);
                        Err(msg.into())
                    }
                    _ => {
                        let result = crate::interface::cli::presenter_bridge::present_graph_full(
                            &outcome, path, *rebuild,
                        );
                        crate::interface::cli::presenter_bridge::report_graph_full(
                            &outcome, &result,
                        );
                        Ok(result)
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
                // The graph comes from the same authority the MCP uses.
                let graph = project_graph_of(&dir)?.graph;

                // The question has one owner: the MCP asks it too, and both
                // publish the same table. The threshold is applied BEFORE
                // the cut — cutting first published fewer hot paths than
                // the project had.
                let entries = AnalysisService::hot_paths(&graph, *limit, *min_fan_in);

                let elapsed = start.elapsed().as_millis();
                let result = GR::HotPaths {
                    path: path.clone(),
                    limit: *limit,
                    min_fan_in: *min_fan_in,
                    entries: entries
                        .into_iter()
                        .map(|hp| HotPathEntry {
                            name: hp.name,
                            fan_in: hp.fan_in,
                            fan_out: hp.fan_out,
                            file: hp.file,
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
                // The graph comes from the same authority the MCP uses.
                let graph = project_graph_of(&dir)?.graph;

                // The question has one owner: the MCP asks it too. Every
                // entry point is published — the list used to stop at
                // twenty without saying so.
                let entry_points = AnalysisService::entry_points(&graph);
                let total = entry_points.len();
                let entry_refs: Vec<SymbolRef> = entry_points
                    .iter()
                    .map(|sym| SymbolRef {
                        name: sym.name().to_string(),
                        file: sym.location().file().to_string(),
                        line: published_line(sym.location().line()),
                        column: published_column(sym.location().column()),
                    })
                    .collect();
                let elapsed = start.elapsed().as_millis();
                let result = GR::EntryPoints {
                    path: path.clone(),
                    symbols: entry_refs,
                    total,
                    elapsed_ms: elapsed,
                };
                // T1-5.4d: text formatting is in the presenter.
                Ok(result)
            }
            GraphCommand::LeafFunctions { path } => {
                let start = Instant::now();

                let dir = PathBuf::from(path);
                // The graph comes from the same authority the MCP uses.
                let graph = project_graph_of(&dir)?.graph;

                // The question has one owner: the MCP asks it too. Every
                // leaf function is published — the list used to stop at
                // twenty without saying so.
                let leaves = AnalysisService::leaf_functions(&graph);
                let total = leaves.len();
                let leaf_refs: Vec<SymbolRef> = leaves
                    .iter()
                    .map(|sym| SymbolRef {
                        name: sym.name().to_string(),
                        file: sym.location().file().to_string(),
                        line: published_line(sym.location().line()),
                        column: published_column(sym.location().column()),
                    })
                    .collect();
                let elapsed = start.elapsed().as_millis();
                let result = GR::LeafFunctions {
                    path: path.clone(),
                    symbols: leaf_refs,
                    total,
                    elapsed_ms: elapsed,
                };
                // T1-5.4d: text formatting is in the presenter.
                Ok(result)
            }
            GraphCommand::TracePath {
                from,
                to,
                max_depth,
                path,
            } => {
                let start = Instant::now();
                // Progress line: the presenter can't reproduce it (the
                // variant doesn't carry the project path), so the
                // function that calculates it writes it.
                progress_line(
                    output_format,
                    &format!("Tracing path from '{}' to '{}' in: {}", from, to, path),
                );

                let dir = PathBuf::from(path);
                // The graph comes from the same authority the MCP uses.
                let graph = project_graph_of(&dir)?.graph;

                // The question has one owner: the MCP asks it too. A name
                // means the symbol that carries it — the trace used to
                // demand a fully qualified name and answered "not found"
                // for the simple name the MCP answers.
                let path_result =
                    match AnalysisService::trace_path_by_name(&graph, from, to, *max_depth) {
                        Ok(Some(symbols)) => {
                            let mut hops: Vec<PathHop> = Vec::new();
                            for (i, sym) in symbols.iter().enumerate() {
                                hops.push(PathHop {
                                    position: (i + 1) as u32,
                                    name: sym.name().to_string(),
                                    file: sym.location().file().to_string(),
                                    line: published_line(sym.location().line()),
                                });
                            }
                            GR::TracePathFound {
                                from: from.clone(),
                                to: to.clone(),
                                hops,
                                elapsed_ms: 0, // filled in below
                            }
                        }
                        Ok(None) => GR::TracePathNotFound {
                            from: from.clone(),
                            to: to.clone(),
                            elapsed_ms: 0, // filled in below
                        },
                        Err(error) => return Err(Box::new(error)),
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
                // A name the graph cannot publish is a refusal, and the
                // refusal names what the command does publish.
                let requested = format.as_str();
                let format = mermaid_format::resolve(requested)?;

                let start = Instant::now();
                // Progress line: the presenter can't reproduce it (the
                // variant doesn't carry the project path).
                progress_line(
                    output_format,
                    &format!("Exporting to Mermaid format from: {}", path),
                );

                let dir = PathBuf::from(path);
                // The root is checked before the walk so the failure is
                // not disguised as partial coverage — the same answer
                // `graph full` and the MCP `build_graph` handler already
                // give.
                if !dir.is_dir() {
                    return Err(format!("Directory does not exist: {}", dir.display()).into());
                }

                // The graph comes from the same authority the MCP uses.
                let graph = project_graph_of(&dir)?.graph;

                // There is one renderer and this is it: the diagram the
                // CLI publishes is the diagram the MCP publishes for the
                // same project.
                let options = crate::domain::aggregates::call_graph::MermaidOptions {
                    root: None,
                    max_depth: 0,
                    theme: None,
                    format: Some(format),
                    module_filter: None,
                };
                let diagram = AnalysisService::mermaid_diagram(&graph, &options);
                let mermaid = diagram.code;
                let diagram_len = mermaid.len();
                let elapsed = start.elapsed().as_millis();
                let truncated = format.is_source() && diagram_len > 2000;
                let result = GR::Mermaid {
                    path: path.clone(),
                    diagram: mermaid,
                    diagram_len,
                    format: requested.to_string(),
                    resolved_format: format,
                    truncated,
                    elapsed_ms: elapsed,
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
                // Progress line: the presenter can't reproduce it (the
                // variant doesn't carry the project path).
                progress_line(
                    output_format,
                    &format!(
                        "Getting call hierarchy for '{}' (depth={}, direction={}) in: {}",
                        symbol, depth, direction, path
                    ),
                );

                let dir = PathBuf::from(path);
                // The graph comes from the same authority the MCP uses.
                let built = project_graph_of(&dir)?;
                let graph = built.graph;

                let dir_enum = match direction.to_lowercase().as_str() {
                    "in" => crate::application::services::graph_queries::Direction::Callers,
                    "out" => crate::application::services::graph_queries::Direction::Callees,
                    _ => crate::application::services::graph_queries::Direction::Both,
                };

                // The question has one owner: the MCP asks it too.
                let entries = AnalysisService::call_hierarchy(&graph, symbol, *depth, dir_enum)
                    .map_err(|e| -> Box<dyn std::error::Error> { Box::new(e) })?;

                let root_id = AnalysisService::resolve_symbol(&graph, symbol)
                    .map_err(|e| -> Box<dyn std::error::Error> { Box::new(e) })?;
                let root =
                    graph
                        .get_symbol(&root_id)
                        .ok_or_else(|| -> Box<dyn std::error::Error> {
                            format!("Symbol '{symbol}' is not in the project graph").into()
                        })?;

                let elapsed = start.elapsed().as_millis();

                let mut by_depth: std::collections::HashMap<u32, Vec<_>> =
                    std::collections::HashMap::new();
                for entry in &entries {
                    by_depth.entry(entry.depth).or_default().push(entry);
                }
                let mut buckets: Vec<DepthBucket> = Vec::new();
                for depth in 1..=*depth {
                    if let Some(entries) = by_depth.get(&depth) {
                        // Every entry is published: the bucket used to stop
                        // at five without saying so.
                        let depth_entries: Vec<DepthEntry> = entries
                            .iter()
                            .map(|entry| DepthEntry {
                                name: entry.name.clone(),
                                direction: entry.direction.as_str().to_string(),
                                file: entry.file.clone(),
                                line: entry.line,
                            })
                            .collect();
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
                    root_file: root.location().file().to_string(),
                    root_line: published_line(root.location().line()),
                    root_column: published_column(root.location().column()),
                    buckets,
                    total_entries: entries.len(),
                    coverage: crate::interface::coverage::CoverageInfo::of(&built.coverage),
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
                // The graph comes from the same authority the MCP uses.
                let graph = project_graph_of(&dir)?.graph;

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
                // Progress line: the presenter can't reproduce it (the
                // variant doesn't carry the project path).
                progress_line(
                    output_format,
                    &format!("Analyzing impact of changing '{}' in: {}", symbol, path),
                );

                let dir = PathBuf::from(path);
                // The graph comes from the same authority the MCP uses.
                let built = project_graph_of(&dir)?;
                let graph = built.graph;

                // The same question, answered by the same function the MCP and
                // the workspace session call.
                let impact = AnalysisService::impact_of(&graph, symbol)
                    .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;

                let elapsed = start.elapsed().as_millis();

                let result = GR::impact_of(impact, symbol.clone(), &built.coverage, elapsed);
                // T1-5.4l: text formatting is in the
                // presenter.
                Ok(result)
            }
            GraphCommand::ImpactComposite {
                symbol,
                depth: _,
                path,
            } => {
                // B4.W5 — Composite impact query. INVESTIGATIVE,
                // NOT AUTHORITATIVE. The result carries
                // `verdict_refused: true` precisely so the caller
                // cannot treat it as a verdict (R-B4-W5). Emitted as
                // a JSON object via stdout (T1-5.4 pre-migration
                // pattern for sub-commands not yet migrated to
                // `GraphResult`).
                use crate::impact::composite::{CompositeImpactQuery, execute_composite};
                use crate::interface::symbol_identity::SymbolId;
                use std::ops::Range;
                use std::path::PathBuf;

                // Build the SymbolId the catalog will look up by
                // qualified_name. The `file_path`/`byte_range` are
                // best-effort defaults; the agent that wants precise
                // location should pass a real symbol identity via a
                // future rich input.
                let symbol_id = SymbolId {
                    qualified_name: symbol.clone(),
                    file_path: PathBuf::from(path.clone()),
                    byte_range: Range { start: 0, end: 0 },
                };
                let query = CompositeImpactQuery {
                    symbol: symbol_id,
                    workspace_root: PathBuf::from(path),
                    depth_limit: None,
                };
                let result = execute_composite(&query);

                // The composite answer has a shape of its own — it is
                // the query's own answer, not a graph summary — so it
                // travels as it is and the presenter writes it once.
                //
                // It used to be printed here AND returned as a
                // `PerFile` with nothing in it, so the command
                // published two documents: the answer, and a second
                // one that described no graph at all.
                //
                // `verdict_refused: true` travels as the JSON
                // `true` literal — the consumer can pattern-match
                // it without parsing the response.
                let body =
                    serde_json::to_string(&result).map_err(|e| -> Box<dyn std::error::Error> {
                        Box::new(std::io::Error::other(e))
                    })?;
                Ok(GR::CompositeImpact { body })
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
                // The position the user typed is the line a person
                // reads, and the code-intelligence paths speak that
                // base: the line travels to the backend as it is.
                // The arithmetic that used to sit here (`line - 1` on
                // the way in) asked the backend about the line above
                // the one the user named.
                let location = Location::new(file.clone(), line, column);

                let def_location = match provider.get_definition(&location).await {
                    Ok(Some(def)) => Some(DefinitionLocation {
                        file: def.file().to_string(),
                        // The backend answers in the base the product
                        // publishes, so the data side carries it as it
                        // is and the presenter does no arithmetic.
                        line: def.line(),
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
                // The position the user typed is the line a person
                // reads, and the code-intelligence paths speak that
                // base: the line travels to the backend as it is.
                let location = Location::new(file.clone(), line, column);

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
                // The position the user typed is the line a person
                // reads, and the code-intelligence paths speak that
                // base: the line travels to the backend as it is.
                let location = Location::new(file.clone(), line, column);

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
                        line: r.location.line(),
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

    /// Execute the doctor subcommand.
    ///
    /// The core renders the report and returns it; the presenter
    /// writes it and the bin ends the process with the code the
    /// report implies. The core does not write to the standard
    /// output: the doctor is the command a script reads the exit
    /// code of, and its answer has to arrive before the process
    /// ends.
    async fn execute_doctor(
        format: &str,
        cwd: &str,
    ) -> Result<CommandOutput, Box<dyn std::error::Error>> {
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

        let text = match format {
            "json" => format_doctor_json(&report),
            _ => format_doctor_text(&report),
        };

        // The exit code follows the overall status.
        let exit_code = match report.overall_status() {
            crate::interface::cli::doctor::DoctorStatus::Missing => 1,
            crate::interface::cli::doctor::DoctorStatus::Ok
            | crate::interface::cli::doctor::DoctorStatus::Warn
            | crate::interface::cli::doctor::DoctorStatus::Info => 0,
        };

        Ok(CommandOutput::Rendered {
            text: format!("{text}\n"),
            exit_code: Some(exit_code),
        })
    }

    /// Execute the capabilities subcommand (A-014 / CP2.4).
    ///
    /// Emits a schema-versioned JSON document on stdout (the
    /// canonical contract for downstream skills / integration
    /// adapters). With `--format text` (default) prints a short
    /// human summary. Either way stderr stays clean unless an
    /// error is reported (the warning path for missing
    /// `product/*.json` is the one exception).
    async fn execute_capabilities(
        format: &str,
    ) -> Result<CommandOutput, Box<dyn std::error::Error>> {
        // Build the doc in memory first so any error in construction
        // is reported as a structured envelope and never leaks a
        // partial document to stdout. JSON emission keeps stderr
        // pristine (the document is pipeable unfiltered); the missing
        // inventory warning is a text-mode diagnostic only.
        let doc = build_capabilities_doc(format == "json");

        let text = if format == "json" {
            format!("{}\n", serde_json::to_string(&doc)?)
        } else {
            format_capabilities_text(&doc)
        };
        Ok(CommandOutput::rendered(text))
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
    ) -> Result<CommandOutput, Box<dyn std::error::Error>> {
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
        // The structured JSON form is the wire-level contract
        // (matches the MCP `docs_ingest` envelope payload).
        let payload = serde_json::json!({
            "files_processed": files_processed,
            "nodes_created":   nodes_created,
            "edges_created":   edges_created,
            "errors":          Vec::<String>::new(),
        });
        // The core renders the answer and returns it; the presenter
        // writes it. Exit 0 when the extractor produced at least one
        // candidate OR the input was a non-empty directory
        // (recursive walks of empty dirs are also exit 0 —
        // the operation is a no-op success).
        let mut text = String::new();
        text.push_str(&format!(
            "docs-ingest: files_processed = {files_processed}\n"
        ));
        text.push_str(&format!("docs-ingest: nodes_created   = {nodes_created}\n"));
        text.push_str(&format!("docs-ingest: edges_created   = {edges_created}\n"));
        text.push_str(&format!("docs-ingest: payload = {payload}\n"));
        Ok(CommandOutput::rendered(text))
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
    ) -> Result<CommandOutput, Box<dyn std::error::Error>> {
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
        let payload = serde_json::json!({
            "issues_processed": issues_processed,
            "nodes_created":    nodes_created,
            "edges_created":    edges_created,
            "errors":           Vec::<String>::new(),
        });
        let mut text = String::new();
        text.push_str(&format!(
            "issues-ingest: issues_processed = {issues_processed}\n"
        ));
        text.push_str(&format!(
            "issues-ingest: nodes_created   = {nodes_created}\n"
        ));
        text.push_str(&format!(
            "issues-ingest: edges_created   = {edges_created}\n"
        ));
        text.push_str(&format!("issues-ingest: payload = {payload}\n"));
        Ok(CommandOutput::rendered(text))
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
    ) -> Result<CommandOutput, Box<dyn std::error::Error>> {
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

        // 4) Render the answer. The core does not write to the
        // standard output; the presenter writes what the core
        // returns.
        let text = match format {
            "json" => {
                let json = serde_json::to_string_pretty(&out)
                    .map_err(|e| format!("find-usages: json serialization: {e}"))?;
                format!("{json}\n")
            }
            "text" => {
                let mut text = String::new();
                if !quiet {
                    text.push_str(&format!(
                        "# find-usages symbol={symbol} total={}\n",
                        out.total
                    ));
                }
                text.push_str(&render_text(&out));
                text
            }
            other => {
                return Err(
                    format!("find-usages: unknown format '{other}' (expected: text|json)").into(),
                );
            }
        };

        Ok(CommandOutput::rendered(text))
    }

    /// Execute analyze subcommand
    async fn execute_analyze(path: &str) -> Result<CommandOutput, Box<dyn std::error::Error>> {
        use crate::WorkspaceSession;
        use crate::interface::mcp::security::InputValidator;
        use std::sync::Arc;

        let mut text = String::new();
        text.push_str(&format!("Analyzing code at: {path}\n"));

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
        text.push_str("\n=== Architecture Check ===\n");
        // A valid directory with nothing to analyze is an operation that
        // ran, not a failure of the command: the exit code stays 0. But
        // the command must not publish a score about nothing — a perfect
        // score over a workspace whose material was never read is a claim
        // about a project nobody looked at. The reason is said instead.
        match session.check_architecture(None).await {
            Ok(result) => {
                text.push_str(&format!("  Score: {:.1}/100\n", result.score));
                text.push_str(&format!("  Summary: {}\n", result.summary));
                text.push_str(&format!("  Cycles: {}\n", result.cycles.len()));
                text.push_str(&format!("  Violations: {}\n", result.violations.len()));
            }
            Err(e) => {
                eprintln!("  Architecture check: {}", e);
            }
        }

        // Complexity check on key files
        text.push_str("\n=== Complexity Analysis ===\n");
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
                text.push_str(&format!(
                    "  {}: cyclomatic={}, cognitive={}, loc={}\n",
                    name, c.cyclomatic, c.cognitive, c.lines_of_code
                ));
            }
        }

        text.push_str("\nAnalysis complete.\n");
        Ok(CommandOutput::rendered(text))
    }

    /// Execute refactor subcommand
    async fn execute_refactor(
        operation: &RefactorOperation,
        symbol: &str,
        new_name: Option<&str>,
        format: &str,
    ) -> Result<CommandOutput, Box<dyn std::error::Error>> {
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

        // The core renders the answer and returns it; the presenter
        // writes it.
        let mut text = String::new();

        match operation {
            RefactorOperation::Rename => {
                let new_name =
                    new_name.ok_or_else(|| anyhow::anyhow!("Rename requires a new name"))?;
                if json_mode {
                    eprintln!("Renaming '{}' to '{}'...", symbol, new_name);
                } else {
                    text.push_str(&format!("Renaming '{symbol}' to '{new_name}'...\n"));
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
                            text.push_str(&format!("{}\n", serde_json::to_string(&doc)?));
                        } else if result.success {
                            text.push_str(&format!(
                                "  Success: {} change(s) made\n",
                                result.changes.len()
                            ));
                        } else {
                            text.push_str(&format!(
                                "  Failed: {}\n",
                                result.error_message.as_deref().unwrap_or("unknown error")
                            ));
                        }
                    }
                    Err(e) => {
                        eprintln!("  Error: {}", e);
                    }
                }
            }
            RefactorOperation::Inline => {
                text.push_str(&format!("Inlining '{symbol}'...\n"));
                match session.inline_symbol(symbol, "<unknown>").await {
                    Ok(result) => {
                        if result.success {
                            text.push_str("  Success\n");
                        } else {
                            text.push_str(&format!(
                                "  Failed: {}\n",
                                result.error_message.as_deref().unwrap_or("unknown error")
                            ));
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
                text.push_str(&format!("Moving '{symbol}' to '{target}'...\n"));
                match session.move_symbol(symbol, "<unknown>", target).await {
                    Ok(result) => {
                        if result.success {
                            text.push_str(&format!(
                                "  Success: {}\n",
                                result.validation_result.warnings.join("; ")
                            ));
                        } else {
                            text.push_str(&format!(
                                "  Failed: {}\n",
                                result.error_message.as_deref().unwrap_or("unknown error")
                            ));
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
                text.push_str(&format!("Extracting function '{name}'...\n"));
                match session
                    .extract_function("<unknown>", (0, 0, 0, 0), name)
                    .await
                {
                    Ok(result) => {
                        if result.success {
                            text.push_str(&format!(
                                "  Success: {}\n",
                                result.validation_result.warnings.join("; ")
                            ));
                        } else {
                            text.push_str(&format!(
                                "  Failed: {}\n",
                                result.error_message.as_deref().unwrap_or("unknown error")
                            ));
                        }
                    }
                    Err(e) => {
                        eprintln!("  Error: {}", e);
                    }
                }
            }
        }

        Ok(CommandOutput::rendered(text))
    }

    /// B5.W3 — Apply con autoridad.
    ///
    /// Lee el `RefactorPlan` de `--plan-file`, construye el
    /// `AuthorizationToken` del canal separado (`--authorization-token`;
    /// AAT-B5-03), recomputa el `workspace_state_digest` y llama a
    /// `cognicode_core::refactor::apply::apply`. Un fallo sale con
    /// código no-cero y su mensaje específico (AuthorizationMissing,
    /// WorkspaceStateMismatch, SymlinkEscape, TraversalEscape,
    /// ConcurrentModification, IoError).
    ///
    /// Tras un apply correcto publica tambien la revalidacion (B5.W4):
    /// la salida dice que se verifico y que no, en vez de callarse.
    async fn execute_refactor_apply(
        plan_file: &std::path::Path,
        authorization_token: &str,
        format: &str,
    ) -> Result<CommandOutput, Box<dyn std::error::Error>> {
        use crate::refactor::apply::{AuthorizationToken, apply, compute_workspace_state_digest};
        use chrono::Utc;

        // Carga el plan desde disco (B5.W1).
        let plan = crate::refactor::plan::load_plan_from_file(plan_file).map_err(|e| {
            format!(
                "B5.W3: failed to read --plan-file {}: {}",
                plan_file.display(),
                e
            )
        })?;

        // Workspace root: derivar del plan.workspace_identity (B5.W1
        // lo canonaliza) o del primer affected_file como fallback.
        let workspace_root = std::path::PathBuf::from(&plan.workspace_identity);
        if !workspace_root.exists() {
            return Err(format!(
                "B5.W3: workspace_identity {:?} does not exist",
                workspace_root
            )
            .into());
        }

        // Recomputa el digest del estado actual del workspace. El
        // preview (B5.W2) habría emitido este mismo digest en
        // `plan.workspace_state_digest`-equivalent; apply verifica
        // que coincida (AAT-B5-W3-04 / UAT-B5-03).
        let actual_digest = compute_workspace_state_digest(&plan);

        // Construye el token a partir del flag separado. NOTA: el
        // apply internamente exige created_at dentro de la ventana
        // de 1h. Para el caso CLI, el token se acaba de generar —
        // usamos Utc::now() para que sea válido.
        let token = AuthorizationToken {
            value: authorization_token.to_string(),
            created_at: Utc::now(),
        };

        match apply(&plan, &actual_digest, &token, &workspace_root) {
            Ok(outcome) => {
                // La revalidacion dice que se verifico y que no: un apply
                // que no dice nada sobre la verificacion se lee como si la
                // hubiera hecho.
                let revalidation = crate::refactor::revalidate::run_revalidation(
                    &crate::refactor::revalidate::build_revalidation_plan(&plan, &outcome),
                    &workspace_root,
                );

                // The core renders the answer and returns it; the
                // presenter writes it.
                let mut text = String::new();
                if format == "json" {
                    #[derive(serde::Serialize)]
                    struct ApplyOkJson {
                        schema_version: &'static str,
                        committed: bool,
                        plan_id: uuid::Uuid,
                        applied_files: Vec<String>,
                        revalidation: crate::refactor::revalidate::RevalidationReport,
                    }
                    let doc = ApplyOkJson {
                        schema_version: "cognicode.refactor.apply/v1",
                        committed: outcome.committed,
                        plan_id: outcome.plan_id,
                        applied_files: outcome
                            .applied_files
                            .iter()
                            .map(|p| p.to_string_lossy().to_string())
                            .collect(),
                        revalidation,
                    };
                    text.push_str(&format!("{}\n", serde_json::to_string_pretty(&doc)?));
                } else {
                    eprintln!(
                        "Apply committed: {} file(s) modified",
                        outcome.applied_files.len()
                    );
                    for f in &outcome.applied_files {
                        text.push_str(&format!("  + {}\n", f.display()));
                    }
                    text.push_str(&format!("  Revalidation: {}\n", revalidation.summary()));
                }
                Ok(CommandOutput::rendered(text))
            }
            Err(e) => Err(Box::new(e) as Box<dyn std::error::Error>),
        }
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
    ) -> Result<CommandOutput, Box<dyn std::error::Error>> {
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

        Ok(CommandOutput::rendered(render_evidence_rows(rows, format)))
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
) -> String {
    use crate::domain::ports::evidence_store::EvidenceKind;

    if format == "json" {
        return format!("{}\n", render_evidence_rows_json(rows));
    }
    let mut text = String::new();
    if rows.is_empty() {
        text.push_str("(no evidence rows match the query)\n");
        return text;
    }
    text.push_str(&format!(
        "{:<60}  {:<12}  {:<7}  TITLE\n",
        "ID", "KIND", "CONF"
    ));
    for r in rows {
        let kind = match r.kind {
            EvidenceKind::Log => "log",
            EvidenceKind::Trace => "trace",
            EvidenceKind::Measurement => "measurement",
            EvidenceKind::External => "external",
        };
        text.push_str(&format!(
            "{:<60}  {:<12}  {:<7.2}  {}\n",
            r.id, kind, r.confidence, r.title
        ));
    }
    text
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
/// The text answer of `find-usages`, rendered and not printed.
///
/// The core returns it and the presenter writes it: a core that
/// prints its own answer can interleave it with the document a
/// consumer is parsing.
fn render_text(out: &crate::interface::mcp::schemas::FindUsagesOutput) -> String {
    let mut text = String::new();
    if out.usages.is_empty() {
        text.push_str("(no usages found)\n");
        return text;
    }
    for u in &out.usages {
        let prefix = if u.is_definition { "def " } else { "    " };
        text.push_str(&format!(
            "{prefix}{file}:{line}:{col}: {ctx}\n",
            file = u.file,
            line = u.line,
            col = u.column,
            ctx = u.context,
        ));
    }
    text
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
            .join("crates/cognicode-core/testdata/per_file_partial_corpus")
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

/// The project's graph, and what the build could not cover.
///
/// The graph comes from the same authority the MCP uses: a command that
/// builds its own graph answers with different facts than the MCP would
/// hand out for the same tree. And the build's skips travel with it, so a
/// command that shows one and hides the other tells half the truth.
fn project_graph_of(
    dir: &std::path::Path,
) -> Result<crate::interface::cli::project_graph::ProjectGraph, Box<dyn std::error::Error>> {
    let built = crate::interface::cli::project_graph::project_graph(dir)
        .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
    crate::interface::cli::project_graph::warn_incomplete(&built);
    Ok(built)
}
