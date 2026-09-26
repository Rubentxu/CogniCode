//! QW-04 — preflight clean-clone contractual test.
//!
//! The script `scripts/ci/preflight-clean-clone.sh` exists but has
//! no contractual test and is not wired into any CI workflow. Its
//! real validation requires a 10-15 minute clean-clone + build +
//! workspace-test cycle (executed in `release.yml` once wired), so
//! the in-suite test focuses on the boundary that determines whether
//! the script is even runnable:
//!
//!   1. **Exists + executable** — the script file must be present
//!      and have the `+x` bit, otherwise the CI gate can never call
//!      it.
//!   2. **Returns non-zero on absent required tool** — pinea que
//!      `fail()` se ejecuta con `require_tool` cuando falta una
//!      dependencia crítica.
//!   3. **Quacks the JSON receipt schema** — la cabecera del recibo
//!      debe declarar los campos esperados por los consumidores
//!      (release.yml + el runbook C8-RECERTIFICATION). Validamos
//!      contra un grep del source (sin ejecutar el preflight, que
//!      tarda 10-15 min).
//!
//! El gate real (clean clone + workspace test contra el baseline
//! 5579/0/37) vive en `release.yml` y se valida en cada release.
//!
//! Históricamente, el C8 base se firmó sobre un working tree que
//! dependía de archivos no trackeados. El primer C8-R detectó esto
//! manualmente. Este test evita que el script desaparezca o pierda
//! su contrato sin que CI lo pille.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

mod common;

use common::repo_root;

/// Resolve the absolute path to the QW-04 preflight script.
fn preflight_script() -> PathBuf {
    repo_root().join("scripts/ci/preflight-clean-clone.sh")
}

#[test]
fn qw04_preflight_script_exists_and_is_executable() {
    let script = preflight_script();
    assert!(
        script.exists(),
        "QW-04 preflight script missing at {} — el gate pierde su \
         contrato. Re-crearlo o restaurar la versión pineada.",
        script.display()
    );
    let metadata = fs::metadata(&script).expect("stat preflight script");
    let mode = metadata.permissions().mode();
    assert_eq!(
        mode & 0o111,
        0o111,
        "QW-04 preflight script at {} does not have the executable \
         bit set (mode={:o}). Without +x, every CI caller that tries \
         to invoke it via `bash` may fail; the workflow was designed \
         around `bash <script>` so this is a real regression.",
        script.display(),
        mode
    );
}

#[test]
fn qw04_preflight_script_quacks_required_receipt_schema() {
    // Read the script and assert that the JSON receipt it emits
    // contains the fields the downstream `release.yml` and the
    // C8 recertification runbook consume. We grep the source
    // rather than executing the preflight because the real run
    // takes 10-15 minutes.
    let script = preflight_script();
    let source = fs::read_to_string(&script).expect("read preflight script");

    for required_field in [
        "\"sha\"",
        "\"tree_hash\"",
        "\"timestamp\"",
        "\"result\"",
        "\"stages\"",
        "\"baseline\"",
        "\"observed\"",
        "\"delta\"",
        "\"passed\"",
        "\"failed\"",
        "\"ignored\"",
        "\"tolerance\"",
        "\"log_file\"",
    ] {
        assert!(
            source.contains(required_field),
            "QW-04 preflight receipt missing required field `{required_field}`. \
             The release.yml JSON-parse step or the C8 runbook depends on it. \
             Update both the script and the consumers in lockstep."
        );
    }
}

#[test]
fn qw04_preflight_script_neutralizes_global_gitignore() {
    // Critical guarantee from the script header: the preflight
    // exports GIT_CONFIG_GLOBAL=/dev/null so the operator's global
    // ~/.config/git/ignore cannot silently exclude files that the
    // workspace actually requires. Without this, the script can
    // pass against the operator's machine but fail on a clean runner
    // (or vice versa), which is exactly the regression we are
    // guarding against.
    let script = preflight_script();
    let source = fs::read_to_string(&script).expect("read preflight script");
    assert!(
        source.contains("export GIT_CONFIG_GLOBAL=/dev/null"),
        "QW-04 preflight does not neutralise the operator's global \
         gitignore. Run it on a clean machine and the bin source \
         gate (QW-03) and any other file-existence check may give \
         different results from the operator's machine."
    );
}

#[test]
fn qw04_preflight_script_fails_when_required_tool_is_missing() {
    // El script usa `require_tool` para git, cargo, perl, python3.
    // Simulamos tool ausente quitando el directorio de cargo del PATH
    // del subproceso bash. Importante:
    //   - Capturar cargo_dir ANTES de cualquier strip (necesitamos
    //     resolver `which cargo` con el PATH del test runner completo).
    //   - Limpiar env vars (CARGO_HOME, RUSTUP_HOME, RUSTUP_TOOLCHAIN,
    //     CARGO) que podrían ofrecer un camino indirecto a cargo/rustc.
    //   - NO quitar `bash`, `git`, `perl`, `python3` del PATH — esos
    //     requieren resolverse para que `require_tool` los pase ANTES
    //     de detectar la falta de cargo.
    let script = preflight_script();

    // 1. Capturar cargo_dir con el PATH del proceso actual (antes de strip).
    //    Importante: usamos `which cargo` para resolver el cargo que ESTÉ
    //    en el PATH real del test runner. rustup puede setear CARGO env
    //    a un binario que NO está en el PATH; el script usa `command -v
    //    cargo` que SOLO mira PATH, por lo que debemos strip'pear el cargo
    //    del PATH, no el de rustup.
    let cargo_path = std::env::var_os("PATH")
        .and_then(|p| {
            for component in std::env::split_paths(&p) {
                let candidate = component.join("cargo");
                if candidate.is_file() {
                    // Found one in PATH. Prefer this over $CARGO env.
                    return Some(candidate);
                }
            }
            None
        })
        .or_else(|| {
            Command::new("which")
                .arg("cargo")
                .output()
                .ok()
                .filter(|o| o.status.success())
                .map(|o| PathBuf::from(String::from_utf8_lossy(&o.stdout).trim()))
        })
        .expect("locate cargo binary in PATH (test setup)");
    let cargo_dir = cargo_path
        .parent()
        .expect("cargo's parent dir")
        .to_path_buf();
    eprintln!("qw04-debug: cargo at {cargo_path:?}, stripping {cargo_dir:?}");

    // 2. Tmpdir vacío para prepender al PATH (vacío pero presente).
    let tmp = std::env::temp_dir().join(format!(
        "qw04-empty-path-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&tmp).expect("mkdir tmp");

    // 3. Construir PATH sin cargo_dir, prependiendo el tmp vacío.
    let real_path = std::env::var_os("PATH").unwrap_or_default();
    let mut stripped = std::ffi::OsString::new();
    for component in std::env::split_paths(&real_path) {
        if component == cargo_dir {
            continue;
        }
        if !stripped.is_empty() {
            stripped.push(":");
        }
        stripped.push(component);
    }
    let mut path_minus_cargo = std::ffi::OsString::from(&tmp);
    path_minus_cargo.push(":");
    path_minus_cargo.push(&stripped);

    // 4. Spawn bash con PATH sin cargo y env vars de toolchain limpias.
    let out = Command::new("bash")
        .arg(&script)
        .env("PATH", &path_minus_cargo)
        .env("RANDOM_GIT_COMMITTER_DISABLED", "1")
        .env_remove("CARGO")
        .env_remove("CARGO_HOME")
        .env_remove("RUSTUP_HOME")
        .env_remove("RUSTUP_TOOLCHAIN")
        .output()
        .expect("spawn preflight");

    let stderr = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        !out.status.success(),
        "QW-04 preflight did NOT fail when cargo was missing from PATH. \
         stdout:\n{stdout}\nstderr:\n{stderr}"
    );

    let combined = format!("{stdout}\n{stderr}");
    assert!(
        combined.contains("herramienta requerida no disponible") || combined.contains("cargo"),
        "QW-04 preflight failure mode did not name the missing tool. \
         expected 'herramienta requerida no disponible' or 'cargo'. \
         stdout:\n{stdout}\nstderr:\n{stderr}"
    );

    fs::remove_dir_all(&tmp).ok();
}

use std::os::unix::fs::PermissionsExt;
