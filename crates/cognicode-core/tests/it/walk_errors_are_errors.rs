//! Un error del recorrido se cuenta como error.
//!
//! Antes de este fichero, el recorrido que decide de qué ficheros se
//! construye el grafo entregaba sus errores con
//! `filter_map(|e| e.ok())`: un directorio que el proceso no puede
//! leer desaparecía del inventario sin dejar rastro. El build terminaba
//! `Complete` sobre un árbol que no había recorrido entero, y el
//! reporte no decía nada. Eso no es un detalle de diagnóstico: es una
//! afirmación falsa sobre la cobertura, del mismo género que el
//! `Complete` que un repo con un hueco real no puede decir.
//!
//! `Complete` es una promesa de dos cosas: que se leyó todo el árbol
//! y que toda referencia quedó atada a una declaración. Por eso un
//! `Partial` ya no significa sólo «hubo un error de recorrido»: puede
//! ser que el recorrido fue entero y que una llamada no se pudo atar.
//! La lista de omitidos es lo que distingue los dos casos, y es lo que
//! estos tests miran.
//!
//! El README y el `.kt` no son parte de este defecto — son una
//! decisión y un hueco, y ya se contaban bien (`h16`). Aquí se fijan
//! igual, para que el arreglo de este hueco no los rompa al revés.
//!
//! Este fichero mira lo que el CONSUMIDOR ve: el reporte de
//! construcción. El detalle de cómo el inventario separa el hueco real
//! de la exclusión legítima vive en `inventory_says_which_is_which.rs`.

use cognicode_core::application::services::analysis_service::AnalysisService;
use cognicode_core::infrastructure::graph::per_file_graph::BuildStatus;

/// Repo Rust normal: un `main.rs` que sí se analiza y un README que no
/// se analiza A PROPOSITO.
///
/// Sin `.kt` a propósito: el `.kt` es un hueco real y ya sale en
/// `skipped`, así que con él el estado sería `Partial` antes y después
/// y el test no distinguiría nada. Aquí lo que se mide es el error del
/// recorrido, y para eso el corpus tiene que estar limpio.
///
/// Y `run` no lo declara nadie: una llamada a un nombre que el proyecto
/// no declara no es una pérdida del proyecto —puede vivir fuera de él—,
/// así que no se publica como pendiente y el estado sigue hablando sólo
/// del recorrido.
fn cargo_repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path();
    std::fs::create_dir_all(root.join("src")).expect("src");
    std::fs::write(root.join("src").join("main.rs"), "fn main() { run(); }\n").expect("main.rs");
    std::fs::write(root.join("README.md"), "# Fixture\n\nUn proyecto normal.\n")
        .expect("README.md");
    dir
}

/// Deja un directorio que este proceso no puede leer y devuelve su ruta.
///
/// `mode 0o000` cierra el DAC al proceso que no es root. Si el proceso
/// es root, root salta los permisos de directorio y el recorrido SÍ
/// leería el árbol: entonces no hay ningún error que exigir, y el test
/// afirmaría una mentira. La sonda de abajo lo detecta y devuelve
/// `None`; ese `None` no es un fallo del test, es la razón explícita
/// de por qué este caso no se puede medir en esta máquina.
#[cfg(unix)]
fn lock_dir(root: &std::path::Path, name: &str) -> Option<std::path::PathBuf> {
    use std::os::unix::fs::PermissionsExt as _;

    let path = root.join(name);
    std::fs::create_dir_all(&path).expect("locked dir");
    std::fs::write(path.join("inside.rs"), "pub fn inside() {}\n").expect("inside.rs");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).expect("chmod 0o000");

    // Sonda: ¿el DAC está realmente cerrado para este proceso?
    if std::fs::read_dir(&path).is_ok() {
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
            .expect("restore perms");
        return None;
    }
    Some(path)
}

/// Devuelve el directorio a `0o755` para que el `TempDir` se pueda
/// borrar al final del test.
#[cfg(unix)]
fn unlock_dir(path: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt as _;
    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755));
}

#[cfg(unix)]
#[test]
fn un_recorrido_que_falla_no_puede_decir_complete() {
    let dir = cargo_repo();
    let Some(locked) = lock_dir(dir.path(), "locked") else {
        eprintln!(
            "SKIP C20: el DAC no se puede cerrar para este proceso (root lo salta); \
             no hay error de recorrido que exigir en esta máquina"
        );
        return;
    };

    let svc = AnalysisService::new();
    svc.build_project_graph(dir.path()).expect("sync build");
    let report = svc
        .get_last_build_report()
        .expect("la ruta publica reporte");

    let skipped = match &report.status {
        BuildStatus::Partial { skipped, .. } => skipped,
        BuildStatus::Complete => {
            unlock_dir(&locked);
            panic!(
                "el recorrido no pudo leer `locked/` y el build dice `Complete`. \
                 Un `Complete` sobre un árbol que no se recorrió entero es la misma \
                 mentira que un `Complete` sobre un repo con un hueco real: el \
                 grafo no describe lo que hay detrás de ese directorio, y el \
                 reporte tiene que decirlo."
            );
        }
    };

    assert!(
        skipped.iter().any(|s| s.path.ends_with("locked")),
        "el reporte tiene que NOMBRAR el directorio que el recorrido no pudo leer: \
         {:?}",
        skipped
    );

    unlock_dir(&locked);
}

#[cfg(unix)]
#[test]
fn el_error_del_recorrido_no_es_un_fichero_del_grafo() {
    let dir = cargo_repo();
    let Some(locked) = lock_dir(dir.path(), "locked") else {
        eprintln!("SKIP C20: el DAC no se puede cerrar para este proceso");
        return;
    };

    let svc = AnalysisService::new();
    svc.build_project_graph(dir.path()).expect("sync build");
    let report = svc
        .get_last_build_report()
        .expect("la ruta publica reporte");

    let skipped = match &report.status {
        BuildStatus::Partial { skipped, .. } => skipped,
        BuildStatus::Complete => {
            unlock_dir(&locked);
            panic!("el recorrido falló y el build dice `Complete`");
        }
    };

    assert!(
        !skipped.iter().any(|s| s.path.ends_with("inside.rs")),
        "lo que hay detrás del directorio ilegible NO está en el grafo: el \
         recorrido no lo vio, y no puede aparecer como si lo hubiera visto. \
         Omitidos: {:?}",
        skipped
    );
    assert!(
        !skipped.iter().any(|s| s.path.ends_with("README.md")),
        "un README está fuera del alcance por política: es una decisión, no un \
         hueco, y no debe aparecer en `skipped`. Omitidos: {:?}",
        skipped
    );

    unlock_dir(&locked);
}

#[cfg(unix)]
#[tokio::test]
async fn las_tres_rutas_cuentan_el_mismo_error_del_recorrido() {
    // La coherencia, enunciada sola. Sin esto, cada test anterior podría
    // pasar en una ruta distinta y el defecto seguiría vivo en las otras
    // dos — que es exactamente como murió el defecto que vino antes.
    let dir = cargo_repo();
    let Some(locked) = lock_dir(dir.path(), "locked") else {
        eprintln!("SKIP C20: el DAC no se puede cerrar para este proceso");
        return;
    };

    let sync = {
        let svc = AnalysisService::new();
        svc.build_project_graph(dir.path()).expect("sync build");
        svc.get_last_build_report().map(|r| r.status)
    };
    let per_file = {
        let svc = AnalysisService::new();
        svc.build_graph_per_file_with_report(dir.path())
            .expect("per-file build");
        svc.get_last_build_report().map(|r| r.status)
    };
    let async_route = {
        let svc = AnalysisService::new();
        svc.build_project_graph_async(dir.path())
            .await
            .expect("async build");
        svc.get_last_build_report().map(|r| r.status)
    };

    let describe = |name: &str, status: &Option<BuildStatus>| match status {
        None => format!("{name}=<no publica reporte>"),
        Some(BuildStatus::Complete) => format!("{name}=Complete"),
        Some(BuildStatus::Partial { skipped, .. }) => {
            let mut names: Vec<String> = skipped
                .iter()
                .map(|s| {
                    std::path::Path::new(&s.path)
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| s.path.clone())
                })
                .collect();
            names.sort();
            format!("{name}=Partial{names:?}")
        }
    };

    let rendered = format!(
        "{} | {} | {}",
        describe("sync", &sync),
        describe("per_file", &per_file),
        describe("async", &async_route)
    );

    for (name, status) in [
        ("sync", &sync),
        ("per_file", &per_file),
        ("async", &async_route),
    ] {
        let status = status.as_ref().unwrap_or_else(|| {
            unlock_dir(&locked);
            panic!(
                "la ruta {name} no publicó BuildReport; sin reporte no hay cobertura \
                 que afirmar. Medido: {rendered}"
            )
        });
        match status {
            BuildStatus::Partial { skipped, .. } => assert!(
                skipped.iter().any(|s| s.path.ends_with("locked")),
                "la ruta {name} tiene que contar el error del recorrido. Medido: {rendered}"
            ),
            BuildStatus::Complete => {
                unlock_dir(&locked);
                panic!(
                    "la ruta {name} dice `Complete` sobre un árbol que no pudo \
                     recorrer entero. Medido: {rendered}"
                );
            }
        }
    }

    unlock_dir(&locked);
}
