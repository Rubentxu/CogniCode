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

// ===========================================================================
// La limpieza no puede cambiar el veredicto.
//
// MEDIDO 2026-10-03. El preflight de v0.101.2 certificó correctamente
// (passed=5934 failed=0, PREFLIGHT PASS, recibo emitido) y la lane salió
// con 64:
//
//     → PREFLIGHT PASS
//     mavis-trash: refusing to trash protected path '.../cognicode-preflight-O2ZYbs'
//     mavis-trash: '...' is the parent of the current working directory
//     Pipeline finished with FAILURE: shell exited with code 64
//
// El trap era `rm -rf "$WORK_DIR"`, y el script hace `cd "$WORK_DIR/clone"`
// en el stage 5: el trap corria desde dentro del directorio que borra. El
// estado de salida del trap sustituye al del script, asi que un PASS se
// convirtio en fallo por no haber limpiado.
//
// Estos tests ejercitan la funcion **extraida del script canonico**, no una
// copia: si alguien la mueve a otro fichero o la borra, el RED dice
// "no se encuentra" en vez de dar verde probando el camino eliminado.
// ===========================================================================

/// El texto de `cognicode_preflight_cleanup` tal y como vive en el script
/// canonico, entre su linea de cabecera y la llave que cierra el cuerpo.
fn cleanup_function_source() -> String {
    let text = fs::read_to_string(preflight_script()).expect("read preflight script");
    let lines: Vec<&str> = text.lines().collect();

    let start = lines
        .iter()
        .position(|l| l.starts_with("cognicode_preflight_cleanup()"))
        .unwrap_or_else(|| {
            panic!(
                "preflight-clean-clone.sh no define `cognicode_preflight_cleanup`. \
                 La funcion tiene que vivir en el script que la usa: un segundo \
                 fichero seria un segundo sitio donde la regla de limpieza puede \
                 quedar sin actualizar."
            )
        });

    let mut depth = 0i32;
    let mut started = false;
    let mut out = String::new();
    for line in &lines[start..] {
        for c in line.chars() {
            match c {
                '{' => {
                    depth += 1;
                    started = true;
                }
                '}' => depth -= 1,
                _ => {}
            }
        }
        out.push_str(line);
        out.push('\n');
        if started && depth == 0 {
            return out;
        }
    }
    panic!("cognicode_preflight_cleanup no cierra su cuerpo");
}

/// Un `rm` que se niega a borrar un directorio que sea **ancestro del cwd**,
/// y devuelve 64. Es la envoltura que produjo el fallo del 2026-10-03.
///
/// Este es el `rm` que hace la diferencia entre el bug y el arreglo:
/// con el trap viejo el cwd estaba dentro del clon al llegar aqui, asi que se
/// negaba; con la funcion actual el trap sale del directorio antes de borrar,
/// asi que la condicion no se cumple y el borrado ocurre.
const FAKE_RM_GUARDS_CWD: &str = r#"#!/usr/bin/env bash
target=""
for arg in "$@"; do
  case "$arg" in
    -*) continue ;;
    *) target="$arg" ;;
  esac
done
[ -n "$target" ] || exit 0
case "$(pwd -P)/" in
  "$(cd "$target" 2>/dev/null && pwd -P)"/*)
    echo "FAKE_RM: refusing to trash protected path '$target'" >&2
    exit 64
    ;;
esac
exec /usr/bin/rm "$@"
"#;

/// Un `rm` que se niega siempre, sin mirar el cwd. Modela el otro modo de
/// fallo de una limpieza —permisos, un fichero abierto, una envoltura que
/// prohibe el path— donde el borrado falla pero el proceso deberia conservar
/// el veredicto que ya habia decidido.
const FAKE_RM_ALWAYS_REFUSES: &str = r#"#!/usr/bin/env bash
echo "FAKE_RM: refusing to remove (simulated permission failure)" >&2
exit 64
"#;

/// Que `rm` poner delante del PATH del subproceso.
#[derive(Clone, Copy, PartialEq)]
enum RmMode {
    /// El `rm` real del sistema.
    Real,
    /// Se niega solo si el cwd esta dentro del target.
    GuardsCwd,
    /// Se niega siempre.
    AlwaysRefuses,
}

impl RmMode {
    fn script(self) -> Option<&'static str> {
        match self {
            RmMode::Real => None,
            RmMode::GuardsCwd => Some(FAKE_RM_GUARDS_CWD),
            RmMode::AlwaysRefuses => Some(FAKE_RM_ALWAYS_REFUSES),
        }
    }
}

/// Reproduce la forma del preflight en la parte que importa: instala el
/// trap, se situa dentro del clon (el stage 5), y sale con `status`.
///
/// Devuelve el codigo de salida real del proceso.
fn run_preflight_shape(status: i32, rm: RmMode) -> (i32, String, PathBuf) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().to_path_buf();
    fs::create_dir_all(root.join("clone")).expect("clone");
    fs::create_dir_all(root.join("bin")).expect("bin");

    if let Some(body) = rm.script() {
        let path = root.join("bin/rm");
        fs::write(&path, body).expect("write fake rm");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("chmod rm");
    }

    // El script que se ejecuta es la funcion real + el trap real, con el
    // cwd dentro del clon: exactamente el estado en que se comporto el fallo.
    let script = root.join("preflight-shape.sh");
    let body = format!(
        "#!/usr/bin/env bash\n\
         set -uo pipefail\n\
         {function}\n\
         WORK_DIR=\"{work}\"\n\
         trap 'cognicode_preflight_cleanup \"$WORK_DIR\"' EXIT\n\
         cd \"$WORK_DIR/clone\"\n\
         exit {status}\n",
        function = cleanup_function_source(),
        work = root.display(),
        status = status,
    );
    fs::write(&script, body).expect("write shape");
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).expect("chmod");

    let path = std::env::join_paths([
        root.join("bin"),
        PathBuf::from("/usr/bin"),
        PathBuf::from("/bin"),
    ])
    .expect("PATH");
    let out = Command::new("bash")
        .arg(&script)
        .env("PATH", path)
        .env("HOME", &root)
        .env("TMPDIR", &root)
        .output()
        .expect("run preflight shape");

    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    let code = out.status.code().unwrap_or(-1);
    // Se conserva el tempdir para poder inspeccionar si el clon sobrevivio;
    // el llamante lo borra.
    std::mem::forget(tmp);
    (code, stderr, root)
}

#[test]
fn a_passing_preflight_stays_passing_when_cleanup_refuses() {
    let (code, stderr, root) = run_preflight_shape(0, RmMode::AlwaysRefuses);
    assert_eq!(
        code, 0,
        "un preflight que certifica PASS tiene que salir con 0 aunque la limpieza \
         se niegue. stderr:\n{stderr}"
    );
    // La prueba de que la limpieza se niego es el aviso de la propia funcion,
    // no el mensaje del `rm` simulado: el `rm` corre con `2>/dev/null`, y su
    // salida se descarta a proposito para que el stderr del gate no se llene
    // de la salida de la herramienta de borrado. El aviso propio es la
    // evidencia, y ademas es la que ve el operador.
    assert!(
        stderr.contains("no se pudo limpiar"),
        "una limpieza que falla tiene que decirse en voz alta, no desaparecer: \
         un aviso silencioso es indistinguible de que no hubiera clon que \
         limpiar, que es como se acumularon 17G de directorios huerfanos. \
         stderr:\n{stderr}"
    );
    assert!(
        !stderr.contains("FAKE_RM"),
        "la salida del `rm` se descarta con 2>/dev/null a proposito; si aparece, \
         la funcion cambio de politica. stderr:\n{stderr}"
    );
    assert!(
        root.exists(),
        "con el `rm` negandose, el clon tiene que sobrevivir: si desapareciera, \
         este test no estaria probando la negativa."
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn a_failing_preflight_stays_failing() {
    let (code, stderr, root) = run_preflight_shape(3, RmMode::AlwaysRefuses);
    assert_eq!(
        code, 3,
        "la limpieza no puede convertir un fallo en exito, ni al reves: un \
         preflight que sale con 3 tiene que seguir saliendo con 3. stderr:\n{stderr}"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn the_cleanup_leaves_the_directory_before_removing_it() {
    // Este es el test que reproduce el incidente del 2026-10-03. El `rm` se
    // niega cuando el cwd esta dentro del target, que es exactamente la
    // condicion que cumplia el trap viejo. La funcion actual sale del
    // directorio antes de borrar, asi que la negativa no llega a producirse.
    //
    // Con el trap viejo este test daba exit 64 con un PASS ya certificado.
    let (code, stderr, root) = run_preflight_shape(0, RmMode::GuardsCwd);

    assert_eq!(
        code, 0,
        "la funcion tiene que salir del directorio antes de borrarlo: el `rm` \
         protegido se niega a borrar un ancestro del cwd, y ese refusal con \
         status 64 es el fallo medido el 2026-10-03. stderr:\n{stderr}"
    );
    assert!(
        !stderr.contains("FAKE_RM"),
        "el `rm` protegido se niega, o sea que el cwd seguia dentro del \
         directorio que se iba a borrar. stderr:\n{stderr}"
    );
    assert!(
        !root.exists(),
        "el clon temporal sobrevive: {} sigue existiendo",
        root.display()
    );
}

#[test]
fn the_temp_clone_is_actually_removed() {
    // Sin ninguna obstruccion, la limpieza tiene que funcionar de verdad: la
    // propiedad anterior seria trivial si el directorio nunca se borrara.
    let (code, stderr, root) = run_preflight_shape(0, RmMode::Real);
    assert_eq!(code, 0, "stderr:\n{stderr}");
    assert!(
        !root.exists(),
        "el clon temporal sobrevive a la limpieza: {} sigue existiendo",
        root.display()
    );
}

#[test]
fn the_preflight_traps_the_function_and_not_a_raw_removal() {
    let text = fs::read_to_string(preflight_script()).expect("read preflight script");
    let live: String = text
        .lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n");

    assert!(
        live.contains("trap 'cognicode_preflight_cleanup \"$WORK_DIR\"' EXIT"),
        "el trap debe llamar a la funcion de cleanup, no a un `rm` directo: \
         el `rm` directo es exactamente el defecto que salio con 64."
    );
    assert!(
        !live.contains("trap 'rm -rf \"$WORK_DIR\"' EXIT"),
        "el trap crudo `rm -rf` ha vuelto: borra el directorio desde dentro de si \
         mismo y su estado sustituye al del script."
    );
}

#[test]
fn the_cleanup_reads_the_status_before_anything_else() {
    let f = cleanup_function_source();
    let after_open = f.split_once('{').expect("la funcion abre su cuerpo").1;
    let first = after_open
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty() && !l.starts_with('#'))
        .unwrap_or_default();

    assert!(
        first.contains("$?"),
        "la primera sentencia de la funcion tiene que leer `$?`: cualquier \
         comando anterior sobrescribiria el estado de salida que se quiere \
         conservar. Primera sentencia: {first:?}"
    );
}

#[test]
fn a_removal_failure_is_reported_and_not_structurally_swallowed() {
    let f = cleanup_function_source();
    let rm_line = f
        .lines()
        .find(|l| l.contains("rm -rf"))
        .unwrap_or_else(|| panic!("la funcion deberia borrar el work_dir: {f}"));

    assert!(
        rm_line.contains("if !"),
        "el borrado tiene que ir dentro de un `if !`: un `rm -rf` a pelo \
         abortaria con `set -e` y convertiria la limpieza en el veredicto. \
         Linea: {rm_line:?}"
    );
    assert!(
        f.contains("no se pudo limpiar"),
        "un borrado que falla tiene que dejar un aviso: sin el, el operador no \
         sabe que hay 17G de clones huerfanos en el volumen."
    );
}

#[test]
fn the_cleanup_lives_in_the_preflight_and_not_in_a_sibling_script() {
    // El Mandato es explicito: el fix pertenece al script existente, y un
    // segundo fichero seria un segundo sitio donde la regla puede quedar
    // desactualizada. Este test no mira que el fichero no exista —eso lo
    // haria dependiente del estado del arbol— sino que la funcion usada por
    // el trap este DEFINIDA en el propio preflight, que es la propiedad que
    // importa.
    let text = fs::read_to_string(preflight_script()).expect("read preflight script");
    assert!(
        text.contains("cognicode_preflight_cleanup() {"),
        "la funcion de cleanup deberia estar definida en preflight-clean-clone.sh."
    );
    assert!(
        !text.contains("source \"$SCRIPT_DIR/preflight-cleanup.sh\""),
        "el preflight vuelve a cargar la limpieza desde un fichero aparte."
    );
}

// ===========================================================================
// El mismo defecto, un stage mas abajo: el UAT de instalacion.
//
// MEDIDO 2026-10-04. La lane de v0.101.8 buildo los dos targets, genero los
// SBOM, aplo el staging —la limpieza de la raiz funciono: los seis tarballs
// sucios de 0.101.4/0.101.5 desaparecieron y quedaron tres de 0.101.8— y
// produjo un candidato cuyos once artefactos verifican contra SHA256SUMS. Luego
// salio:
//
//     Pipeline finished with FAILURE: shell exited with code 64
//
// Y el UAT que lo precedia habia pasado:
//
//     PASS: published-layout CLI + MCP + skills install/update/reshim/uninstall
//
// El script era `release-install-smoke.sh`, con el trap viejo:
//
//     cleanup() { ... rm -rf "$TMP"; }
//     trap cleanup EXIT
//
// El script reexporta HOME y XDG_DATA_HOME dentro de `$TMP`, y corre el CLI con
// el cwd ahi dentro. Asi que la limpieza se lanzaba desde el directorio que
// borraba, con el HOME dentro del directorio que borraba, y con la papelera de
// la envoltura de recuperacion DENTRO del temporal: un arbol de 70 MB que
// contiene su propia basura y que ya no se puede mover. El 64 de la envoltura
// sustituyo al veredicto, que era un PASS.
//
// Es el mismo defecto que el de arriba, en el hermano que B2 se dejo atras: la
// stage 1 paso porque el preflight ya estaba arreglado, y esta murio porque el
// otro no. Por eso los tests viven aqui y no en un fichero nuevo: la propiedad
// —la limpieza no puede ser el veredicto— tiene un dueno, y este dueno vigila
// los dos scripts que la implementan.
// ===========================================================================

fn install_smoke_script() -> PathBuf {
    repo_root().join("scripts/ci/release-install-smoke.sh")
}

/// El texto de `cognicode_install_smoke_cleanup` tal y como vive en el script
/// canonico. Mismo extractor que el del preflight, y por el mismo motivo: si
/// alguien mueve o borra la funcion, el RED dice "no se encuentra" en vez de
/// dar verde probando una copia.
fn install_smoke_cleanup_source() -> String {
    let text = fs::read_to_string(install_smoke_script()).expect("read install-smoke script");
    let lines: Vec<&str> = text.lines().collect();

    let start = lines
        .iter()
        .position(|l| l.starts_with("cognicode_install_smoke_cleanup()"))
        .unwrap_or_else(|| {
            panic!(
                "release-install-smoke.sh no define `cognicode_install_smoke_cleanup`. \
                 La funcion tiene que vivir en el script que la usa: un segundo \
                 fichero seria un segundo sitio donde la regla de limpieza puede \
                 quedar sin actualizar."
            )
        });

    let mut depth = 0i32;
    let mut started = false;
    let mut out = String::new();
    for line in &lines[start..] {
        for c in line.chars() {
            match c {
                '{' => {
                    depth += 1;
                    started = true;
                }
                '}' => depth -= 1,
                _ => {}
            }
        }
        out.push_str(line);
        out.push('\n');
        if started && depth == 0 {
            break;
        }
    }
    out
}

/// Reproduce la forma del UAT en la parte que importa: el trap corre con el
/// cwd dentro de `$TMP`, con HOME y XDG_DATA_HOME dentro de `$TMP` — que es
/// como estaba cuando la lane salio con 64— y el script sale con `status`.
fn run_install_smoke_shape(status: i32, rm: RmMode) -> (i32, String, PathBuf) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().to_path_buf();
    fs::create_dir_all(root.join("home")).expect("home");
    fs::create_dir_all(root.join("bin")).expect("bin");

    if let Some(body) = rm.script() {
        let path = root.join("bin/rm");
        fs::write(&path, body).expect("write fake rm");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("chmod rm");
    }

    let script = root.join("smoke-shape.sh");
    let body = format!(
        "#!/usr/bin/env bash\n\
         set -uo pipefail\n\
         {function}\n\
         TMP=\"{work}\"\n\
         SERVER_PID=\"\"\n\
         ORIGINAL_HOME=\"{outer_home}\"\n\
         ORIGINAL_XDG_DATA_HOME=\"{outer_home}/.local/share\"\n\
         trap 'cognicode_install_smoke_cleanup' EXIT\n\
         export HOME=\"$TMP/home\"\n\
         export XDG_DATA_HOME=\"$HOME/.local/share\"\n\
         cd \"$TMP/home\" || exit 1\n\
         exit {status}\n",
        function = install_smoke_cleanup_source(),
        work = root.display(),
        outer_home = root.join("outer-home").display(),
        status = status,
    );
    fs::write(&script, body).expect("write shape");
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).expect("chmod");

    let path = std::env::join_paths([
        root.join("bin"),
        PathBuf::from("/usr/bin"),
        PathBuf::from("/bin"),
    ])
    .expect("PATH");
    let out = Command::new("bash")
        .arg(&script)
        .env("PATH", path)
        .env("HOME", root.join("outer-home"))
        .env("XDG_DATA_HOME", root.join("outer-home/.local/share"))
        .output()
        .expect("run install-smoke shape");

    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    let code = out.status.code().unwrap_or(-1);
    std::mem::forget(tmp);
    (code, stderr, root)
}

#[test]
fn a_passing_install_smoke_stays_passing_when_cleanup_refuses() {
    let (code, stderr, root) = run_install_smoke_shape(0, RmMode::AlwaysRefuses);
    assert_eq!(
        code, 0,
        "un UAT que pasa tiene que salir con 0 aunque la limpieza se niegue. \
         Salir con 64 fue exactamente lo que mato la lane de v0.101.8. \
         stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("no se pudo limpiar"),
        "una limpieza que falla tiene que decirse en voz alta: es la unica \
         evidencia que distingue 'el UAT fallo' de 'no borre una carpeta'. \
         stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("FAKE_RM"),
        "el motivo del borrado se imprime a proposito en este script, y es una \
         diferencia deliberada con el preflight: alli el `rm` corre con \
         `2>/dev/null` porque su stderr va al log de una stage que ya ha \
         certificado. Aqui el UAT es lo que el operador lee cuando falla, y un \
         aviso que no dice por que no se pudo borrar obliga a reproducir la \
         corrida entera. stderr:\n{stderr}"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn a_failing_install_smoke_stays_failing() {
    let (code, _stderr, root) = run_install_smoke_shape(3, RmMode::Real);
    assert_eq!(
        code, 3,
        "arreglar la limpieza no puede tapar un UAT que de verdad falla: ese es \
         el otro lado de la misma propiedad."
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn the_install_smoke_cleanup_reads_the_status_first() {
    let f = install_smoke_cleanup_source();
    let after_open = f.split_once('{').expect("la funcion abre su cuerpo").1;
    let first = after_open
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty() && !l.starts_with('#'))
        .unwrap_or_default();

    assert!(
        first.contains("$?"),
        "la primera sentencia tiene que leer `$?`: el `kill` del servidor MCP va \
         justo despues y sobrescribiria el estado que se quiere conservar. \
         Primera sentencia: {first:?}"
    );
}

#[test]
fn the_install_smoke_removal_cannot_become_the_verdict() {
    let f = install_smoke_cleanup_source();
    let rm_line = f
        .lines()
        .find(|l| l.contains("rm -rf"))
        .unwrap_or_else(|| panic!("la funcion deberia borrar el temporal: {f}"));

    assert!(
        rm_line.contains("if !"),
        "el borrado tiene que ir dentro de un `if !`: a pelo, con `set -e`, su \
         estado seria el del script. Linea: {rm_line:?}"
    );
    assert!(
        f.contains("no se pudo limpiar"),
        "un borrado que falla tiene que dejar aviso: sin el, 70 MB por corrida \
         se acumulan sin que nadie lo note."
    );
}

#[test]
fn the_install_smoke_cleanup_puts_the_trash_back_outside_the_temporary() {
    // La segunda mitad del defecto, y la que no hacia falta ver para arreglar.
    //
    // Devolver solo el HOME no basta. Con HOME fuera pero XDG_DATA_HOME
    // todavia dentro del temporal, la envoltura deja de negarse y pasa a
    // fallar con `failed to trash`: el arbol contiene su propio
    // `.local/share/Trash`, y un arbol que contiene su propia papelera no se
    // puede mover. MEDIDO: con las dos variables fuera, `rc=0` y BORRADO; con
    // una sola, el temporal sobrevive.
    let f = install_smoke_cleanup_source();

    assert!(
        f.contains("HOME=\"$ORIGINAL_HOME\""),
        "la limpieza tiene que devolver HOME: mientras apunte dentro del \
         temporal, la envoltura se niega a moverlo por ser su ancestro."
    );
    assert!(
        f.contains("XDG_DATA_HOME=\"$ORIGINAL_XDG_DATA_HOME\"")
            || f.contains("unset XDG_DATA_HOME"),
        "la limpieza tiene que devolver XDG_DATA_HOME, no solo HOME: la \
         envoltura trastera en `$XDG_DATA_HOME/Trash`, y si esa variable sigue \
         dentro del temporal, el temporal contiene su propia basura y no se \
         puede borrar. De ahi los 70 MB por corrida."
    );
}

#[test]
fn the_install_smoke_trap_calls_the_function_and_not_a_bare_rm() {
    let text = fs::read_to_string(install_smoke_script()).expect("read install-smoke script");
    let live = text
        .lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n");

    assert!(
        live.contains("trap 'cognicode_install_smoke_cleanup' EXIT"),
        "el trap debe llamar a la funcion de cleanup, no a un `rm` directo: el \
         `rm` directo es el defecto que salio con 64."
    );
    assert!(
        !live.contains("trap cleanup EXIT"),
        "el trap crudo ha vuelto: borra el temporal desde dentro de si mismo y \
         su estado sustituye al del script."
    );
    assert!(
        live.contains("cognicode_install_smoke_cleanup() {"),
        "la funcion de cleanup deberia estar definida en release-install-smoke.sh."
    );
}
