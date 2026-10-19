//! H16 — Coherencia de clasificación entre las rutas que caminan el repo.
//!
//! MEDIDO 2026-10-19, antes de este fichero. `AnalysisService` tiene
//! tres rutas vivas que construyen el grafo del proyecto, y sobre un
//! repo Rust **sin una sola fuente sin soporte** (Cargo.toml, README.md,
//! dos `.rs`):
//!
//! ```text
//! build_project_graph           => Partial skipped=2   (README.md, Cargo.toml)
//! build_graph_per_file_with_report => Complete skipped=0
//! build_project_graph_async     => <no publica reporte>
//! ```
//!
//! Y con un `Other.kt` de verdad:
//!
//! ```text
//! build_project_graph           => Partial skipped=3   (README.md, Cargo.toml, Other.kt)
//! build_graph_per_file_with_report => Partial skipped=1   (Other.kt)
//! build_project_graph_async     => <no publica reporte>
//! ```
//!
//! ## Por qué esto ya no se vio
//!
//! El clasificador correcto existe desde K0 (`classify_walk_extension`:
//! `Analyze | KnownUnsupportedGap | LegitExclusion`) y tiene su contrato
//! (`h11_unified_classification_contract.rs`), donde el propio comentario
//! dice que un README **"MUST NOT appear in skipped"**. Pero dos rutas
//! no lo llaman: filtran por `language.is_none()`, que es un predicado
//! binario que aplasta las tres ramas a dos.
//!
//! Y cada ruta tenía su propio test (`h13` para per-file, `h12` para la
//! estrategia, `h13` para el sync). Cada una pasaba. La divergencia no
//! podia morir por tests: **nadie comparaba dos rutas entre si**.
//!
//! Ese es el hueco que este fichero cubre. La forma importa mas que la
//! lista de casos:
//!
//!   - **Ningun test por ruta.** Un test que fija el comportamiento de
//!     `build_project_graph` queda verde aunque `..._async` diga otra
//!     cosa. Este fichero exige que las tres digan LO MISMO.
//!   - **Un solo corpus, esperado explicito.** Si una ruta deja de
//!     publicar su reporte, el `None` falla la asercion: no hay forma
//!     de que "sin reporte" se lea como "sin problemas".
//!   - **La direccion peligrosa esta fijada por su nombre.** Un
//!     `Complete` falso es peor que un `Partial` falso, porque
//!     `mod.rs:1312` BORRA el snapshot durable cuando el build no es
//!     `Complete`. Un gap real que se vuelve silencio no lo detecta
//!     nadie; un asset que se vuelve gap solo molesta.

use cognicode_core::application::services::analysis_service::AnalysisService;
use cognicode_core::infrastructure::graph::per_file_graph::BuildStatus;

/// Repo Rust ordinario. Cero fuentes sin soporte: solo assets que no
/// analizamos A PROPOSITO y asi lo decimos en el clasificador.
fn cargo_repo(suffix: &str, with_kotlin_gap: bool) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "h16-{suffix}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    std::fs::write(dir.join("README.md"), "# Fixture\n\nUn proyecto normal.\n").unwrap();
    std::fs::write(
        dir.join("src").join("main.rs"),
        "mod core;\nfn main() { core::run(); }\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("src").join("core.rs"),
        "pub fn run() -> u32 { 1 }\n",
    )
    .unwrap();
    if with_kotlin_gap {
        std::fs::write(
            dir.join("src").join("Other.kt"),
            "class Other { fun x() = 1 }\n",
        )
        .unwrap();
    }
    dir
}

/// Nombres de fichero de los omitidos, ordenados: compara rutas sin
/// depender del camino absoluto del tempdir.
fn skipped_names(status: &BuildStatus) -> Vec<String> {
    match status {
        BuildStatus::Complete => Vec::new(),
        BuildStatus::Partial { skipped } => {
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
            names
        }
    }
}

fn is_complete(status: &BuildStatus) -> bool {
    matches!(status, BuildStatus::Complete)
}

/// Las tres rutas, con el reporte que publicaron. `None` se distingue
/// de `Complete` a proposito: no publicar NO es lo mismo que no haber
/// encontrado nada, y confundirlos es como este defecto se escondio.
async fn status_from_all_three(dir: &std::path::Path) -> Vec<(&'static str, Option<BuildStatus>)> {
    let sync = {
        let svc = AnalysisService::new();
        svc.build_project_graph(dir).expect("sync build");
        svc.get_last_build_report().map(|r| r.status)
    };

    let per_file = {
        let svc = AnalysisService::new();
        svc.build_graph_per_file_with_report(dir)
            .expect("per-file build");
        svc.get_last_build_report().map(|r| r.status)
    };

    let async_route = {
        let svc = AnalysisService::new();
        svc.build_project_graph_async(dir)
            .await
            .expect("async build");
        svc.get_last_build_report().map(|r| r.status)
    };

    vec![
        ("sync", sync),
        ("per_file", per_file),
        ("async", async_route),
    ]
}

fn describe(name: &str, status: &Option<BuildStatus>) -> String {
    match status {
        None => format!("{name}=<no publica reporte>"),
        Some(s) => format!("{name}={s:?}"),
    }
}

#[tokio::test]
async fn a_repo_without_unsupported_sources_is_complete_on_every_route() {
    // El caso que mas caro sale: aqui el estado parcialmente del sync
    // hace que `mod.rs:1312` borre el snapshot durable de un repo que
    // no tiene ni un hueco real.
    let dir = cargo_repo("clean", false);

    let statuses = status_from_all_three(&dir).await;
    let rendered: Vec<String> = statuses.iter().map(|(n, s)| describe(n, s)).collect();

    for (name, status) in &statuses {
        let status = status.as_ref().unwrap_or_else(|| {
            panic!(
                "la ruta {name} no publico BuildReport: sin reporte no hay cobertura que \
                 afirmar, y `None` no puede leerse como `Complete`. Medido: {}",
                rendered.join(" | ")
            )
        });
        assert!(
            is_complete(status),
            "un repo sin fuentes sin soporte es Complete en las tres rutas. \
             Se obtuvo: {}",
            rendered.join(" | ")
        );
    }

    std::fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn a_readme_is_never_reported_as_a_coverage_gap_on_any_route() {
    // La asercion discriminante: nombra el archivo que un clasificador
    // correcto NO puede reportar como hueco. Si alguien reintroduce el
    // filtro binario, este nombre reaparece y el test cae.
    let dir = cargo_repo("asset", false);

    for (name, status) in status_from_all_three(&dir).await {
        let status = status.as_ref().unwrap_or_else(|| {
            panic!("la ruta {name} no publico BuildReport; sin el, nada se comprueba")
        });
        let names = skipped_names(status);
        for asset in ["README.md", "Cargo.toml"] {
            assert!(
                !names.iter().any(|n| n == asset),
                "`{asset}` esta fuera del alcance por politica: no es un hueco, \
                 asi que no debe aparecer en `skipped`. Ruta {name} lo reporto. \
                 Los tres lados de la ley estan en h11; repetirla aqui es \
                 deliberado, porque h11 mira el clasificador y esto mira las \
                 rutas que pueden NO llamarlo."
            );
        }
    }

    std::fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn a_real_language_gap_survives_on_every_route() {
    // La direccion opuesta. Si arreglar lo anterior colgara un `.kt`
    // por el camino, el repo pasaria a `Complete` y el gap K0
    // desapareceria del reporte. Esto es lo que NO puede pasar.
    let dir = cargo_repo("gap", true);

    for (name, status) in status_from_all_three(&dir).await {
        let status = status.as_ref().unwrap_or_else(|| {
            panic!(
                "la ruta {name} no publico BuildReport; un `.kt` sin registrar es \
                 justamente el caso que mas necesita que alguien mire"
            )
        });
        assert!(
            !is_complete(status),
            "un `.kt` es una fuente real en un lenguaje conocido-sin-soporte: \
             es un hueco honesto y la ruta {name} lo reporta como complete"
        );
        assert_eq!(
            skipped_names(status),
            vec!["Other.kt".to_string()],
            "la ruta {name} debe reportar EXACTAMENTE el hueco real, ni uno mas \
             ni uno menos"
        );
    }

    std::fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn every_route_publishes_a_report_for_the_same_corpus() {
    // La coherencia, enunciada sola. Sin esto los tres tests anteriores
    // podrian pasar cada uno en una ruta distinta.
    let dir = cargo_repo("coherence", true);

    let statuses = status_from_all_three(&dir).await;
    let (sync, per_file, async_route) = (
        statuses[0].1.as_ref(),
        statuses[1].1.as_ref(),
        statuses[2].1.as_ref(),
    );

    assert!(
        sync.is_some() && per_file.is_some() && async_route.is_some(),
        "las tres rutas deben publicar BuildReport; sin el, un consumidor que \
         llama a la ultima ruta que corrio lee el reporte de la anterior"
    );
    assert_eq!(
        skipped_names(sync.expect("sync")),
        skipped_names(per_file.expect("per_file")),
        "sync y per-file deben ver el mismo corpus igual"
    );
    assert_eq!(
        skipped_names(sync.expect("sync")),
        skipped_names(async_route.expect("async")),
        "sync y async deben ver el mismo corpus igual: hoy la async filtra en \
         silencio y no registra nada, asi que su numero de omitidos no es \
         comparable con el de las otras dos"
    );

    std::fs::remove_dir_all(&dir).ok();
}
