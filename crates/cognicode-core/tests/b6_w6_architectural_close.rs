//! B6.W6 — Cierre arquitectónico (C8-R base).
//!
//! B6.W6 — Architectural closure (C8-R foundation).
//!
//! Que cierra este WU:
//!   - El ratchet soft (4000 líneas) y el hard cap (6000 líneas) sobre
//!     `commands.rs` y `rmcp_adapter.rs` quedan pineados con un test
//!     que lee el filesystem. Hard cap = 1.5x soft cap.
//!   - The soft ratchet (4000 lines) and hard cap (6000 lines) on
//!     `commands.rs` and `rmcp_adapter.rs` are pinned by a test that
//!     reads the filesystem. Hard cap = 1.5x soft cap.
//!   - La separacion delgada CLI/MCP se mantiene: `cognicode-cli` no
//!     depende de `cognicode-mcp`, y viceversa.
//!   - The thin CLI/MCP separation holds: `cognicode-cli` does not
//!     depend on `cognicode-mcp`, and vice versa.
//!   - Los modulos B4.W1..W5 no re-implementan tipos B2
//!     (`AnalysisBasis`, `Severity`); los reusan por path o `pub use`.
//!   - The B4.W1..W5 modules do not re-implement B2 types
//!     (`AnalysisBasis`, `Severity`); they re-use them by path or
//!     `pub use`.
//!
//! Politica B6.W6 (NO se debilita bajo ninguna circunstancia):
//!   - El ratchet soft no se promociona a hard cap sin evidencia de
//!     connascence medida y un contrato preservado.
//!   - The soft ratchet is not promoted to a hard cap without measured
//!     connascence evidence and a preserved contract.
//!   - Cualquier tipo nuevo que compita con `AnalysisBasis` o
//!     `Severity` debe abrir un ADR nuevo antes de fusionar.
//!   - Any new type competing with `AnalysisBasis` or `Severity` must
//!     open a new ADR before merging.
//!
//! Referencia: `docs/roadmap/adr/ADR-012-st04-cli-mcp-separation.md`.

use std::fs;
use std::path::{Path, PathBuf};

/// Repo root (parent of `crates/cognicode-core`).
/// Raiz del repo (padre de `crates/cognicode-core`).
fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates/cognicode-core has a parent")
        .parent()
        .expect("crates/ has a parent")
        .to_path_buf()
}

/// Soft cap y hard cap del ratchet sobre coordinadores grandes.
/// Soft and hard cap of the ratchet on large coordinators.
///
/// 4000 lines = soft cap (warning emitted; no split justified).
/// 4000 lineas = soft cap (warning emitido; sin division justificada).
/// 6000 lines = hard cap (build fails; review mandatory).
/// 6000 lineas = hard cap (build falla; revision obligatoria).
const SOFT_CAP_LINES: usize = 4000;
const HARD_CAP_LINES: usize = 6000;

/// Lines of a file or 0 if the file does not exist.
/// Lineas de un fichero o 0 si el fichero no existe.
fn line_count(path: &Path) -> usize {
    let Ok(text) = fs::read_to_string(path) else {
        return 0;
    };
    if text.is_empty() {
        0
    } else {
        text.lines().count()
    }
}

/// CLI coordinator: `cognicode-core/src/interface/cli/commands.rs`.
/// Coordinador CLI: `cognicode-core/src/interface/cli/commands.rs`.
fn cli_coordinator_path() -> PathBuf {
    repo_root().join("crates/cognicode-core/src/interface/cli/commands.rs")
}

/// MCP coordinator: `cognicode-core/src/interface/mcp/rmcp_adapter.rs`.
/// Coordinador MCP: `cognicode-core/src/interface/mcp/rmcp_adapter.rs`.
fn mcp_coordinator_path() -> PathBuf {
    repo_root().join("crates/cognicode-core/src/interface/mcp/rmcp_adapter.rs")
}

/// Read the dependency block of a `Cargo.toml` and return the
/// declared workspace-path dependencies (any key whose value starts
/// with `../cognicode-`).
///
/// Lee el bloque de dependencias de un `Cargo.toml` y devuelve las
/// dependencias declaradas como workspace-path (cualquier clave cuyo
/// valor empiece por `../cognicode-`).
fn workspace_path_deps(cargo_toml: &Path) -> Vec<String> {
    let text = fs::read_to_string(cargo_toml)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", cargo_toml.display()));
    let mut deps = Vec::new();
    let mut in_deps = false;
    for raw_line in text.lines() {
        let line = raw_line.split('#').next().unwrap_or("").trim();
        if line.starts_with('[') {
            // [dependencies], [dev-dependencies], [build-dependencies].
            // We only count [dependencies] (the first occurrence).
            in_deps = line == "[dependencies]";
            continue;
        }
        if !in_deps || line.is_empty() {
            continue;
        }
        // Entry forms (we treat all of them as workspace-path deps):
        //   - `name.workspace = true` (resolves via [workspace.dependencies])
        //   - `name = { workspace = true }` (inline workspace-resolved)
        //   - `name = { path = "../cognicode-X" }` (explicit path)
        // Registry version (`name = "x.y.z"`) is excluded because it
        // does not create a workspace-path coupling — the value is a
        // quoted version, not a `{ workspace = true }` or `path = ...`.
        if let Some((raw_name, value)) = line.split_once('=') {
            let raw_name = raw_name.trim();
            let value = value.trim();
            // Heuristic: workspace-path deps use either an inline
            // block `{ ... }` form OR the legacy `.workspace = true`
            // (in which case `value` is literally `true`). Registry
            // version deps use a quoted version string as the value.
            let is_workspace_attr =
                raw_name.ends_with(".workspace") || value.contains("workspace = true");
            let is_path_attr =
                value.contains("../cognicode-") || value.contains("path = \"../cognicode-");
            let is_inline_block = value.starts_with('{');
            // Build the canonical crate name (strip `.workspace`).
            let canonical = raw_name
                .strip_suffix(".workspace")
                .unwrap_or(raw_name)
                .to_string();
            if canonical.starts_with("cognicode-")
                && (is_workspace_attr || is_path_attr || is_inline_block)
            {
                deps.push(canonical);
            }
        }
    }
    deps
}

// =============================================================================
// Test 1 — cli_does_not_depend_on_mcp_runtime
// =============================================================================

/// `cognicode-cli/Cargo.toml` declares `cognicode-core` as a
/// workspace dependency. It must NOT declare `cognicode-mcp`: that
/// would couple the CLI runtime to the MCP server, the very split
/// ST-04 that ADR-012 says is not justified.
///
/// `cognicode-cli/Cargo.toml` declara `cognicode-core` como
/// dependencia del workspace. NO debe declarar `cognicode-mcp`:
/// eso acoplaría el runtime CLI al servidor MCP, precisamente la
/// division ST-04 que ADR-012 dice no esta justificada.
#[test]
fn cli_does_not_depend_on_mcp_runtime() {
    let cargo = repo_root().join("crates/cognicode-cli/Cargo.toml");
    let deps = workspace_path_deps(&cargo);
    assert!(
        deps.iter().any(|d| d == "cognicode-core"),
        "cognicode-cli must depend on cognicode-core (workspace path). \
         Found: {deps:?}"
    );
    assert!(
        !deps.iter().any(|d| d == "cognicode-mcp"),
        "cognicode-cli must NOT depend on cognicode-mcp — that would \
         couple the CLI runtime to the MCP server. ST-04 separation is \
         NOT justified per ADR-012. Found: {deps:?}"
    );
}

// =============================================================================
// Test 2 — mcp_does_not_depend_on_cli_runtime
// =============================================================================

/// Mirror of test 1 in the other direction. `cognicode-mcp` declares
/// `cognicode-core` (the runtime it actually serves) and must NOT
/// declare `cognicode-cli`. The two crates coexist as independent
/// binaries; their shared surface is `cognicode-core`.
///
/// Espejo del test 1 en el otro sentido. `cognicode-mcp` declara
/// `cognicode-core` (el runtime al que sirve) y NO debe declarar
/// `cognicode-cli`. Los dos crates coexisten como binarios
/// independientes; su superficie compartida es `cognicode-core`.
#[test]
fn mcp_does_not_depend_on_cli_runtime() {
    let cargo = repo_root().join("crates/cognicode-mcp/Cargo.toml");
    let deps = workspace_path_deps(&cargo);
    assert!(
        deps.iter().any(|d| d == "cognicode-core"),
        "cognicode-mcp must depend on cognicode-core (workspace path). \
         Found: {deps:?}"
    );
    assert!(
        !deps.iter().any(|d| d == "cognicode-cli"),
        "cognicode-mcp must NOT depend on cognicode-cli — the CLI and \
         MCP runtimes share cognicode-core, not each other. ST-04 \
         separation is NOT justified per ADR-012. Found: {deps:?}"
    );
}

// =============================================================================
// Test 3 — large_coordinators_have_a_size_ratchet
// =============================================================================

/// Ratchet on `commands.rs` and `rmcp_adapter.rs`.
///
/// Below the soft cap (4000): no warning, no failure.
/// Above soft, below hard: warning emitted to stderr; build passes.
/// Above hard cap (6000): build fails.
///
/// Ratchet sobre `commands.rs` y `rmcp_adapter.rs`.
///
/// Por debajo del soft cap (4000): sin warning, sin fallo.
/// Sobre el soft, bajo el hard: warning emitido a stderr; build pasa.
/// Sobre el hard cap (6000): build falla.
#[test]
fn large_coordinators_have_a_size_ratchet() {
    let cli_path = cli_coordinator_path();
    let mcp_path = mcp_coordinator_path();

    assert!(
        cli_path.exists(),
        "CLI coordinator not found at {}. Repository layout changed \
         without updating this test.",
        cli_path.display()
    );
    assert!(
        mcp_path.exists(),
        "MCP coordinator not found at {}. Repository layout changed \
         without updating this test.",
        mcp_path.display()
    );

    let cli_lines = line_count(&cli_path);
    let mcp_lines = line_count(&mcp_path);

    // Soft cap: warning. The build keeps passing; the warning is the
    // signal that a future maintainer should measure connascence.
    if cli_lines > SOFT_CAP_LINES && cli_lines <= HARD_CAP_LINES {
        eprintln!(
            "[B6.W6 ratchet] WARNING: {} = {} lines (soft cap = {}, \
             hard cap = {}). Measure connascence before splitting; see \
             ADR-012.",
            cli_path.display(),
            cli_lines,
            SOFT_CAP_LINES,
            HARD_CAP_LINES
        );
    }
    if mcp_lines > SOFT_CAP_LINES && mcp_lines <= HARD_CAP_LINES {
        eprintln!(
            "[B6.W6 ratchet] WARNING: {} = {} lines (soft cap = {}, \
             hard cap = {}). Measure connascence before splitting; see \
             ADR-012.",
            mcp_path.display(),
            mcp_lines,
            SOFT_CAP_LINES,
            HARD_CAP_LINES
        );
    }

    // Hard cap: build fails. No warning — a coordinator at >6000 lines
    // is a measured problem, not a polite reminder.
    assert!(
        cli_lines <= HARD_CAP_LINES,
        "CLI coordinator {} has {} lines (hard cap = {}). The soft cap \
         is {}; crossing the hard cap means connascence was NOT \
         measured before growth. See ADR-012 §Decisión 2.",
        cli_path.display(),
        cli_lines,
        HARD_CAP_LINES,
        SOFT_CAP_LINES
    );
    assert!(
        mcp_lines <= HARD_CAP_LINES,
        "MCP coordinator {} has {} lines (hard cap = {}). The soft cap \
         is {}; crossing the hard cap means connascence was NOT \
         measured before growth. See ADR-012 §Decisión 2.",
        mcp_path.display(),
        mcp_lines,
        HARD_CAP_LINES,
        SOFT_CAP_LINES
    );
}

// =============================================================================
// Test 4 — B4_modules_dont_reimplement_b2_types
// =============================================================================

/// B4.W1..W5 (impact, architecture, findings, composite_impact,
/// assurance_export::workspace_extractor) MUST NOT redefine
/// `AnalysisBasis` or `Severity`. They re-use them via `crate::*`
/// imports or `pub use` re-exports. A re-declaration creates two
/// sources of truth for the same domain concept, the lesson
/// AGENTS.md cites for `EvidenceStore`.
///
/// B4.W1..W5 (impact, architecture, findings, composite_impact,
/// assurance_export::workspace_extractor) NO deben redefinir
/// `AnalysisBasis` ni `Severity`. Los reusan via imports `crate::*`
/// o re-exports `pub use`. Una re-declaracion crea dos fuentes de
/// verdad para el mismo concepto de dominio, la leccion que
/// AGENTS.md cita para `EvidenceStore`.
// El prefijo `B4_` es la convencion de trazabilidad que enlaza este test
// con su ciclo, no un descuido de estilo. Renombrarlo perderia el enlace, asi
// que la excepcion se acota a esta firma en vez de desactivar el lint en
// todo el fichero.
#[allow(non_snake_case)]
#[test]
fn B4_modules_dont_reimplement_b2_types() {
    let search_dirs = [
        "crates/cognicode-core/src/impact",
        "crates/cognicode-core/src/architecture",
        "crates/cognicode-core/src/findings",
        "crates/cognicode-core/src/assurance_export",
    ];
    let root = repo_root();
    let mut redefined: Vec<String> = Vec::new();

    for dir in search_dirs {
        let path = root.join(dir);
        let Ok(entries) = fs::read_dir(&path) else {
            continue;
        };
        for entry in entries.flatten() {
            let p = entry.path();
            if p.extension().and_then(|e| e.to_str()) != Some("rs") {
                continue;
            }
            let Ok(text) = fs::read_to_string(&p) else {
                continue;
            };
            // We pin `pub struct AnalysisBasis` (with body opening)
            // because only a definition needs a new ADR.
            if text.lines().any(|l| {
                let t = l.trim();
                t.starts_with("pub struct AnalysisBasis")
                    && (t.ends_with('{') || t.ends_with(';') || t.contains('<'))
            }) {
                redefined.push(format!(
                    "{} redefines `pub struct AnalysisBasis` — must \
                     re-use via `crate::application::analysis_basis::AnalysisBasis`.",
                    p.display()
                ));
            }
            // We pin `pub enum Severity` (with body opening) the same
            // way. `architecture::diagnostic` is the canonical home.
            if p.to_string_lossy().contains("architecture/diagnostic") {
                continue; // canonical home — exempt.
            }
            if text.lines().any(|l| {
                let t = l.trim();
                t.starts_with("pub enum Severity") && (t.ends_with('{') || t.contains('<'))
            }) {
                redefined.push(format!(
                    "{} redefines `pub enum Severity` — must re-use via \
                     `crate::architecture::diagnostic::Severity`.",
                    p.display()
                ));
            }
        }
    }

    assert!(
        redefined.is_empty(),
        "{} B4 module(s) re-implement B2 types. Re-use instead of \
         re-declare per ADR-012 §Decisión 1: {redefined:?}",
        redefined.len()
    );
}

// =============================================================================
// Test 5 — b2_w1_analysis_basis_is_the_only_basis
// =============================================================================

/// Pin that `pub struct AnalysisBasis` is defined exactly once in
/// the whole `cognicode-core` source tree. A second declaration is
/// a silent divergence that ADR-012 says cannot exist.
///
/// Pinea que `pub struct AnalysisBasis` esta definido exactamente
/// una vez en todo el arbol `cognicode-core`. Una segunda
/// declaracion es una divergencia silenciosa que ADR-012 dice
/// no debe existir.
#[test]
fn b2_w1_analysis_basis_is_the_only_basis() {
    let root = repo_root().join("crates/cognicode-core/src");
    let mut hits: Vec<String> = Vec::new();
    walk_rs(&root, &mut |p| {
        if let Ok(text) = fs::read_to_string(p) {
            for line in text.lines() {
                let t = line.trim();
                // Match `pub struct AnalysisBasis {`, optionally with
                // generic bounds: `pub struct AnalysisBasis<T> {`.
                if t.starts_with("pub struct AnalysisBasis")
                    && (t.ends_with('{') || t.ends_with(';') || t.contains('<'))
                {
                    hits.push(p.display().to_string());
                    break;
                }
            }
        }
    });
    assert_eq!(
        hits.len(),
        1,
        "expected exactly one `pub struct AnalysisBasis` definition, \
         found {}: {hits:?}. A second declaration creates two sources \
         of truth (the lesson AGENTS.md cites for `EvidenceStore`).",
        hits.len()
    );
    assert!(
        hits[0].ends_with("application/analysis_basis.rs"),
        "the canonical `AnalysisBasis` must live in \
         `application/analysis_basis.rs`. Found at: {}",
        hits[0]
    );
}

// =============================================================================
// Test 6 — impact_graph_reuses_symbol_identity
// =============================================================================

/// The B4.W1 `impact` module MUST consume `SymbolId` from B3.W1
/// (`interface::symbol_identity`) rather than declaring its own
/// identifier type. B3.W1 already pinned the contract; B4.W1
/// inherits it.
///
/// El modulo B4.W1 `impact` debe consumir `SymbolId` de B3.W1
/// (`interface::symbol_identity`) en vez de declarar su propio
/// tipo de identificador. B3.W1 ya pino el contrato; B4.W1 lo
/// hereda.
#[test]
fn impact_graph_reuses_symbol_identity() {
    let root = repo_root();
    let module_dir = root.join("crates/cognicode-core/src/impact");
    let Ok(entries) = fs::read_dir(&module_dir) else {
        panic!("impact module not found at {}", module_dir.display());
    };
    let mut any_consumer = false;
    let mut re_declares: Vec<String> = Vec::new();
    for entry in entries.flatten() {
        let p = entry.path();
        if p.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let Ok(text) = fs::read_to_string(&p) else {
            continue;
        };
        // Consumes the canonical type.
        if text.contains("interface::symbol_identity::SymbolId")
            || text.contains("crate::interface::symbol_identity")
            || text.contains("SymbolId")
        {
            any_consumer = true;
        }
        // Re-declares a competing type. We accept the words
        // `SymbolId` (it's the canonical name) and pin any other
        // struct that ends in `Id` and lives in this module —
        // because B3.W1 set the precedent that identifiers are
        // shared, not module-local.
        for line in text.lines() {
            let t = line.trim();
            if t.starts_with("pub struct ") && t.ends_with("Id {") && !t.contains("SymbolId") {
                re_declares.push(format!("{}: {t}", p.display()));
            }
        }
    }
    assert!(
        any_consumer,
        "impact module must consume SymbolId from interface::symbol_identity. \
         No reference found in any impact/*.rs file."
    );
    assert!(
        re_declares.is_empty(),
        "impact module re-declares identifier-like types. Re-use \
         SymbolId per B3.W1 contract: {re_declares:?}"
    );
}

// =============================================================================
// Test 7 — architecture_diagnostic_publishes_severity
// =============================================================================

/// The B4.W2 `architecture::diagnostic` module MUST be the only
/// place in `cognicode-core` that publishes `pub enum Severity`.
/// Other modules (B4.W1, B4.W3, B4.W5) import it via
/// `crate::architecture::diagnostic::Severity` or via the
/// `pub use` re-export at `architecture/mod.rs`.
///
/// El modulo B4.W2 `architecture::diagnostic` debe ser el unico
/// lugar en `cognicode-core` que publica `pub enum Severity`.
/// Otros modulos (B4.W1, B4.W3, B4.W5) lo importan via
/// `crate::architecture::diagnostic::Severity` o via la
/// re-exportacion `pub use` en `architecture/mod.rs`.
#[test]
fn architecture_diagnostic_publishes_severity() {
    let root = repo_root().join("crates/cognicode-core/src");
    let mut hits: Vec<String> = Vec::new();
    walk_rs(&root, &mut |p| {
        if p.to_string_lossy().contains("architecture/diagnostic") {
            return; // canonical home — exempt.
        }
        if let Ok(text) = fs::read_to_string(p) {
            for line in text.lines() {
                let t = line.trim();
                if t.starts_with("pub enum Severity") && (t.ends_with('{') || t.contains('<')) {
                    hits.push(p.display().to_string());
                    break;
                }
            }
        }
    });
    assert!(
        hits.is_empty(),
        "{} file(s) outside architecture/diagnostic redefine \
         `pub enum Severity`. Re-use via \
         `crate::architecture::diagnostic::Severity`: {hits:?}",
        hits.len()
    );
}

// =============================================================================
// Helpers (no tests below this line)
// =============================================================================

/// Walk a directory tree recursively and invoke `visit` on every
/// regular file. Silently ignores unreadable entries (the assertion
/// surface is the user's tree, not the helper's IO errors).
///
/// Recorre un arbol de directorios e invoca `visit` sobre cada
/// fichero regular. Ignora silenciosamente entradas no leibles (la
/// superficie de asercion es el arbol del usuario, no los errores
/// de IO del helper).
fn walk_rs(root: &Path, visit: &mut dyn FnMut(&Path)) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            walk_rs(&p, visit);
        } else if p.extension().and_then(|e| e.to_str()) == Some("rs") {
            visit(&p);
        }
    }
}
