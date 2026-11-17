//! El goal `inspect`: qué declara un fichero.
//!
//! Parsea el fichero que se le nombra y publica los símbolos que declara.
//! No necesita el grafo entero: el fichero es la unidad de trabajo.
//!
//! ## De quién es la respuesta
//!
//! De la del servicio: el goal no extrae símbolos por su cuenta, pregunta al
//! dueño y publica su payload. La forma de cada símbolo es la que el
//! producto publica para todos los consumidores.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;

use crate::application::dto::SymbolDto;
use crate::application::services::analysis_service::AnalysisService;
use crate::interface::goals::GoalEnvironment;
use crate::interface::path_authorisation::authorise_workspace_file;

use super::compare::RevisionFactsCoverage;
use super::revision_support::produce_for_ref;

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
/// Input for the `inspect` goal
pub struct InspectInput {
    /// The file to inspect, relative to the workspace or absolute. Exactly one of
    /// `file` and `revision` is required.
    #[serde(default)]
    pub file: Option<String>,
    /// A Git revision to inspect instead: the facts assembled for it, their
    /// coverage and the files left without facts. Exactly one of `file` and
    /// `revision` is required.
    #[serde(default)]
    pub revision: Option<String>,
}

/// The facts of one revision, as the goal reports them.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InspectedRevision {
    /// The reference the caller supplied.
    pub requested_ref: String,
    /// The resolved immutable revision handle.
    pub revision: String,
    /// The digest of all tracked content of the revision.
    pub content_digest: String,
    /// Whether the revision snapshot was persisted for reuse across
    /// branches; `false` when the environment names no cache.
    pub persisted: bool,
    /// What the facts of the revision cover, gaps included.
    pub coverage: RevisionFactsCoverage,
}

/// La respuesta del goal `inspect`.
///
/// Los símbolos son los del servicio (`SymbolDto`), sin copiarlos: el goal
/// publica la forma que el producto publica para todos los consumidores.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalInspect {
    /// Una línea que resume la respuesta.
    pub summary: String,
    /// La versión del esquema de esta respuesta.
    pub schema_version: String,
    /// El goal que contesta.
    pub goal: String,
    /// El fichero que se ha inspeccionado; `null` al inspeccionar una revisión.
    pub file: Option<String>,
    /// Los símbolos que el fichero declara; vacío al inspeccionar una revisión.
    pub symbols: Vec<SymbolDto>,
    /// Los hechos de la revisión inspeccionada, si se pidió una.
    #[serde(default)]
    pub revision_facts: Option<InspectedRevision>,
}

/// La respuesta del goal `inspect` para ese fichero.
pub fn inspect(workspace: &Path, file: &str) -> Result<GoalInspect, String> {
    // El fichero se autoriza antes de leerlo: el goal habla del workspace
    // del llamante, y la misma autoridad que confina a los handlers del
    // MCP confina aquí — `../`, absolutas fuera y symlinks que escapan
    // son un rechazo que nombra el límite, no un accidente de I/O.
    let path = authorise_workspace_file(workspace, file)?;
    let service = AnalysisService::new();
    let symbols = service.get_file_symbols(&path).map_err(|error| {
        format!(
            "the file `{}` could not be read: {error}",
            path.to_string_lossy()
        )
    })?;

    let summary = format!(
        "`{}` declares {} symbol(s).",
        path.to_string_lossy(),
        symbols.len()
    );

    Ok(GoalInspect {
        summary,
        schema_version: "cognicode.goal.inspect/v1".to_string(),
        goal: "inspect".to_string(),
        file: Some(path.to_string_lossy().to_string()),
        symbols,
        revision_facts: None,
    })
}

/// The facts of one revision: what they cover and what they miss.
pub fn inspect_revision(
    workspace: &Path,
    cache_root: Option<&Path>,
    reference: &str,
) -> Result<GoalInspect, String> {
    let produced = produce_for_ref(workspace, cache_root, reference, true)?;
    let coverage = produced.coverage();
    let summary = format!(
        "Revision {} yields {} fact(s) over {} of {} analyzable file(s); coverage is {}.",
        produced.revision.handle().as_str(),
        coverage.facts,
        coverage.parsed_files + coverage.reused_files,
        coverage.analyzable_files,
        coverage.coverage
    );
    Ok(GoalInspect {
        summary,
        schema_version: "cognicode.goal.inspect/v1".to_string(),
        goal: "inspect".to_string(),
        file: None,
        symbols: Vec::new(),
        revision_facts: Some(InspectedRevision {
            requested_ref: reference.to_string(),
            revision: produced.revision.handle().as_str().to_string(),
            content_digest: produced.revision.content().as_hex().to_string(),
            persisted: produced.persisted,
            coverage,
        }),
    })
}

/// La implementación del goal: lee sus argumentos y devuelve su respuesta.
pub fn run(ctx: &GoalEnvironment, arguments: &Value) -> Result<Value, String> {
    let workspace = ctx.workspace;
    let input: InspectInput = serde_json::from_value(arguments.clone())
        .map_err(|error| format!("the arguments of the goal `inspect` are not valid: {error}"))?;
    let answer = match (&input.file, &input.revision) {
        (Some(file), None) => inspect(workspace, file)?,
        (None, Some(revision)) => inspect_revision(workspace, ctx.cache_root, revision)?,
        (Some(_), Some(_)) => {
            return Err(
                "the goal `inspect` takes either `file` or `revision`, not both".to_string(),
            );
        }
        (None, None) => {
            return Err("the goal `inspect` needs `file` or `revision`".to_string());
        }
    };
    serde_json::to_value(answer)
        .map_err(|error| format!("the answer of the goal `inspect` is not serialisable: {error}"))
}

/// La respuesta del goal `inspect` en texto.
pub fn render(answer: &Value) -> String {
    let inspected: GoalInspect = serde_json::from_value(answer.clone())
        .expect("the answer of the goal `inspect` is the answer its own type describes");
    let mut text = String::new();
    text.push_str(&inspected.summary);
    text.push('\n');
    for symbol in &inspected.symbols {
        text.push_str(&format!(
            "  {}:{} {} ({})\n",
            symbol.file_path, symbol.line, symbol.name, symbol.kind
        ));
    }
    if let Some(revision) = &inspected.revision_facts {
        for gap in &revision.coverage.gaps {
            text.push_str(&format!("  no facts: {} ({})\n", gap.path, gap.reason));
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_file_named_is_the_file_inspected() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("main.rs"), "fn helper() {}\nfn main() {}").expect("write");

        let answer = inspect(dir.path(), "main.rs").expect("the goal answers");

        assert_eq!(answer.symbols.len(), 2);
        assert!(answer.file.expect("a file answer").ends_with("main.rs"));
        assert!(
            answer.symbols.iter().any(|symbol| symbol.name == "helper"),
            "the goal publishes what the file declares"
        );
    }

    #[test]
    fn a_file_that_cannot_be_read_says_so() {
        let dir = tempfile::tempdir().expect("tempdir");

        let answer = inspect(dir.path(), "nowhere.rs");

        assert!(
            answer.is_err(),
            "a file that is not there is not an empty answer: {answer:?}"
        );
    }

    // TASK-218 — the file the caller names is confined to the workspace:
    // `../`, an absolute path outside and a symlinked escape are all
    // refused, and the refusal says the path left the workspace (not an
    // I/O accident).
    #[test]
    fn a_file_outside_the_workspace_is_not_inspected() {
        let parent = tempfile::tempdir().expect("tempdir");
        let work = parent.path().join("work");
        let outside = parent.path().join("outside");
        std::fs::create_dir(&work).expect("work dir");
        std::fs::create_dir(&outside).expect("outside dir");
        std::fs::write(outside.join("secret.rs"), "fn secret() {}").expect("write");

        let relative = inspect(&work, "../outside/secret.rs");
        let absolute = inspect(&work, &outside.join("secret.rs").to_string_lossy());

        assert!(
            relative.is_err(),
            "a relative escape is not inspected: {relative:?}"
        );
        assert!(
            absolute.is_err(),
            "an absolute path outside is not inspected: {absolute:?}"
        );
        // La absoluta es el caso de frontera pura: el rechazo dice qué
        // mundo se dejó, y no es un accidente de I/O.
        let message = absolute.expect_err("already asserted");
        assert!(
            message.contains("outside the workspace"),
            "the refusal names the workspace boundary: {message}"
        );

        #[cfg(unix)]
        {
            std::os::unix::fs::symlink("../outside", work.join("link")).expect("symlink");
            let via_link = inspect(&work, "link/secret.rs");
            assert!(
                via_link.is_err(),
                "a symlink that escapes is not inspected: {via_link:?}"
            );
        }
    }

    fn repository(files: &[(&str, &[u8])]) -> tempfile::TempDir {
        let directory = tempfile::tempdir().expect("repository");
        let root = directory.path();
        let git = |args: &[&str]| {
            let no_configuration = root.join("no-git-configuration");
            let output = std::process::Command::new("git")
                .env("PATH", "/usr/bin:/bin")
                .arg("-C")
                .arg(root)
                .args(args)
                .env("GIT_CONFIG_GLOBAL", &no_configuration)
                .env("GIT_CONFIG_SYSTEM", &no_configuration)
                .output()
                .expect("git is installed");
            assert!(output.status.success(), "git {args:?} failed");
        };
        git(&["init", "--quiet"]);
        git(&["config", "user.name", "CogniCode tests"]);
        git(&["config", "user.email", "tests@example.invalid"]);
        for (name, body) in files {
            std::fs::write(root.join(name), body).expect("write file");
        }
        git(&["add", "-A"]);
        git(&["commit", "--quiet", "-m", "base"]);
        directory
    }

    #[test]
    fn a_revision_is_inspected_through_its_facts_and_gaps() {
        let repo = repository(&[("a.rs", b"fn a() {}\n"), ("bad.rs", &[0xff, 0xfe, 0x9f])]);
        let cache = tempfile::tempdir().expect("cache");

        let answer = inspect_revision(repo.path(), Some(cache.path()), "HEAD").expect("inspect");

        let facts = answer.revision_facts.expect("revision facts");
        assert!(answer.file.is_none() && answer.symbols.is_empty());
        assert!(facts.persisted);
        assert_eq!(facts.coverage.coverage, "partial");
        assert_eq!(facts.coverage.gaps[0].path, "bad.rs");
        assert!(facts.coverage.facts > 0);
    }

    #[test]
    fn exactly_one_of_file_and_revision_is_required() {
        let dir = tempfile::tempdir().expect("tempdir");
        let ctx = GoalEnvironment::new(dir.path(), None);

        let neither = run(&ctx, &serde_json::json!({}));
        let both = run(
            &ctx,
            &serde_json::json!({"file": "a.rs", "revision": "HEAD"}),
        );

        assert!(neither.expect_err("neither is refused").contains("needs"));
        assert!(both.expect_err("both are refused").contains("not both"));
    }
}
