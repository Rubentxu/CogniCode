// crates/cognicode-cli/tests/qw08_crate_selector.rs
//
// CR-08 selector determinista de suites — test contractual.
//
// Este test ejercita `scripts/ci/select-suites.sh` directamente sobre
// entradas que imitan `git diff --name-only`. El selector NO corre
// en un job CI por ahora (PR-CI no expone "set of changed files" sin
// un step `dorny/paths-filter` extra); su contrato es lo que este test
// pinea. Si el script cambia su semántica o deja de responder en JSON,
// uno o más de estos tests fallan antes de que un PR rompa el silencio.
//
// Política L1.2.W — los tests contractuales viven en `cognicode-cli/tests/`,
// corren con `cargo test -p cognicode-cli --test <name>` y se invocan
// en CI como step dentro del job `merge-gate` (NO como job separado).
//
// Política de aislamiento — el script se invoca con un env aislado:
//   - HOME → tempdir (impide que /home/<user>/.config/git/ignore
//     o gitconfig global contaminen las reglas de fallback).
//     En realidad el script no consulta git, pero la regla `*.rs` para
//     paths slashed se basó en tests previos donde find usaba git.
//   - GIT_CONFIG_GLOBAL=/dev/null — paralelismo con QW-04.
//   - PATH del sistema (sin strip) para que bash pueda resolver el
//     shell builtin.

use std::path::PathBuf;

fn script_path() -> PathBuf {
    // El test corre desde el working dir del crate (CARGO_MANIFEST_DIR
    // apunta a crates/cognicode-cli). Subir al workspace root.
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // crates/cognicode-cli -> crates -> workspace_root
    p.pop();
    p.pop();
    p.push("scripts");
    p.push("ci");
    p.push("select-suites.sh");
    p
}

/// Run the script with the given paths and return (status, stdout).
fn run_select(paths: &str) -> (i32, String) {
    let script = script_path();
    assert!(
        script.exists(),
        "select-suites.sh must exist at {}",
        script.display()
    );

    let output = std::process::Command::new("bash")
        .arg(&script)
        .arg("--paths")
        .arg(paths)
        .env("HOME", "/tmp") // neutraliza gitconfig global real (no usado por el script)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("SELECT_PATHS", "")
        .output()
        .expect("select-suites.sh must spawn successfully");

    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let code = output.status.code().unwrap_or(-1);
    (code, stdout)
}

/// Extract a JSON field value (strategy, reason) from the output,
/// simple string-based extraction to keep dependencies minimal.
fn extract_json_field<'a>(stdout: &'a str, key: &str) -> Option<&'a str> {
    let needle = format!("\"{}\":", key);
    let after = stdout.find(&needle)?;
    let rest = &stdout[after + needle.len()..];
    let trimmed = rest.trim_start();
    // Caso especial: string "value"
    if let Some(stripped) = trimmed.strip_prefix('"') {
        let end = stripped.find('"')?;
        Some(&stripped[..end])
    } else if trimmed.starts_with('[') {
        // skip array; no necesario para los tests actuales
        None
    } else {
        None
    }
}

fn extract_suites(stdout: &str) -> Vec<String> {
    let start = stdout
        .find("\"suites\":")
        .expect("stdout must declare suites");
    let rest = &stdout[start + "\"suites\":".len()..];
    let trimmed = rest.trim_start();
    assert!(trimmed.starts_with('['), "suites must be an array");
    let end = trimmed.find(']').expect("suite array must close");
    let array = &trimmed[1..end];
    if array.is_empty() {
        return Vec::new();
    }
    array
        .split(',')
        .map(|s| s.trim().trim_matches('"').to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

// -------------------- tests --------------------

#[test]
fn qw08_crate_selector_returns_changed_for_core_path() {
    let (code, stdout) = run_select("crates/cognicode-core/src/lib.rs");
    assert_eq!(code, 0, "exit code 0 expected; got {code}, out={stdout}");
    assert_eq!(
        extract_json_field(&stdout, "strategy"),
        Some("changed"),
        "strategy for core path must be 'changed'"
    );
    let suites = extract_suites(&stdout);
    assert_eq!(suites, vec!["core".to_string()]);
}

#[test]
fn qw08_crate_selector_returns_explorer_and_core_for_explorer_path() {
    let (code, stdout) = run_select("crates/cognicode-explorer/src/architecture.rs");
    assert_eq!(code, 0);
    assert_eq!(extract_json_field(&stdout, "strategy"), Some("changed"));
    let suites = extract_suites(&stdout);
    assert!(
        suites.contains(&"explorer".to_string()),
        "explorer suite must be present: {suites:?}"
    );
    assert!(
        suites.contains(&"core".to_string()),
        "core suite must be present (arch codegen): {suites:?}"
    );
}

#[test]
fn qw08_crate_selector_returns_mcp_and_core_for_mcp_path() {
    let (code, stdout) = run_select("crates/cognicode-mcp/src/rpc.rs");
    assert_eq!(code, 0);
    let suites = extract_suites(&stdout);
    assert!(suites.contains(&"mcp".to_string()));
    assert!(suites.contains(&"core".to_string()));
}

#[test]
fn qw08_crate_selector_returns_cli_for_cli_path() {
    let (code, stdout) = run_select("crates/cognicode-cli/src/main.rs");
    assert_eq!(code, 0);
    assert_eq!(extract_json_field(&stdout, "strategy"), Some("changed"));
    let suites = extract_suites(&stdout);
    assert_eq!(suites, vec!["cli".to_string()]);
}

#[test]
fn qw08_crate_selector_returns_ladybug_for_ladybug_path() {
    let (code, stdout) = run_select("crates/cognicode-ladybug/src/store.rs");
    assert_eq!(code, 0);
    let suites = extract_suites(&stdout);
    assert_eq!(suites, vec!["ladybug".to_string()]);
}

#[test]
fn qw08_crate_selector_falls_back_on_workflow_change() {
    let (code, stdout) = run_select(".github/workflows/pr-ci.yml");
    assert_eq!(code, 0);
    assert_eq!(
        extract_json_field(&stdout, "strategy"),
        Some("fallback"),
        "workflow changes must trigger fallback"
    );
    let suites = extract_suites(&stdout);
    assert!(
        suites.len() >= 5,
        "fallback must include all suites; got {suites:?}"
    );
}

#[test]
fn qw08_crate_selector_falls_back_on_crate_cargo_toml_change() {
    let (code, stdout) = run_select("crates/cognicode-core/Cargo.toml");
    assert_eq!(code, 0);
    assert_eq!(extract_json_field(&stdout, "strategy"), Some("fallback"));
}

#[test]
fn qw08_crate_selector_falls_back_on_workspace_cargo_toml() {
    let (code, stdout) = run_select("Cargo.toml");
    assert_eq!(code, 0);
    assert_eq!(extract_json_field(&stdout, "strategy"), Some("fallback"));
}

#[test]
fn qw08_crate_selector_noop_for_sandbox_paths() {
    let (code, stdout) = run_select("sandbox/scripts/foo.py");
    assert_eq!(code, 0);
    assert_eq!(
        extract_json_field(&stdout, "strategy"),
        Some("noop"),
        "sandbox must be noop; got: {stdout}"
    );
    let suites = extract_suites(&stdout);
    assert!(
        suites.is_empty(),
        "noop strategy must have empty suites; got {suites:?}"
    );
}

#[test]
fn qw08_crate_selector_noop_for_metadata_files() {
    let (code, stdout) = run_select("README.md");
    assert_eq!(code, 0);
    assert_eq!(extract_json_field(&stdout, "strategy"), Some("noop"));
}

#[test]
fn qw08_crate_selector_falls_back_when_paths_empty() {
    let (code, stdout) = run_select("");
    assert_eq!(code, 0, "empty paths must exit 0 (safe default)");
    assert_eq!(
        extract_json_field(&stdout, "strategy"),
        Some("fallback"),
        "empty paths must fallback to full suite"
    );
}

#[test]
fn qw08_crate_selector_unions_suites_for_multiple_paths() {
    let (code, stdout) = run_select("crates/cognicode-core/src/a.rs crates/cognicode-mcp/src/b.rs");
    assert_eq!(code, 0);
    let suites = extract_suites(&stdout);
    assert!(suites.contains(&"core".to_string()));
    assert!(suites.contains(&"mcp".to_string()));
    // cli NO debe aparecer
    assert!(!suites.contains(&"cli".to_string()));
}

#[test]
fn qw08_crate_selector_unions_when_noop_mixed_with_real_change() {
    let (code, stdout) = run_select("README.md crates/cognicode-core/src/foo.rs");
    assert_eq!(code, 0);
    let strategy = extract_json_field(&stdout, "strategy").unwrap_or("");
    assert_ne!(
        strategy, "noop",
        "si hay cambios reales, la estrategia no debe ser noop"
    );
    let suites = extract_suites(&stdout);
    assert!(
        suites.contains(&"core".to_string()),
        "core change must drive suite inclusion even with noop co-traveler: {suites:?}"
    );
}

#[test]
fn qw08_crate_selector_falls_back_on_unknown_root_dotrs() {
    let (code, stdout) = run_select("foo.rs");
    assert_eq!(code, 0);
    assert_eq!(extract_json_field(&stdout, "strategy"), Some("fallback"));
}
