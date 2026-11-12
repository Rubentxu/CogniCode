//! TASK-223 — el refactor mide su impacto sobre el grafo del análisis.
//!
//! Hallazgo H9 (P1, fidelidad y rendimiento) de la auditoría del
//! 2026-11-12: `build_minimal_graph` reconstruía un grafo propio con un
//! `contained_walk` sin filtros —entraba en `target/` y en lo que
//! `.gitignore` declina—, inventaba los ids de los callees como
//! `nombre:0:0` (un nodo falso que funde a todos los homónimos y se
//! queda con las llamadas de todos), y se tragaba los errores de
//! lectura con `let _`.
//!
//! El refactor debe medir sobre el grafo que el análisis sostiene: el
//! mismo inventario de ficheros, los mismos ids, los mismos huecos. Los
//! números del código viejo, medidos antes del cambio (evidencia RED):
//!
//! - un rename se negaba porque `target/gen.rs` nombra el identificador;
//! - dos llamadas same-file a un símbolo contaban «(1 symbols
//!   impacted)»: los callers se perdían en el nodo falso;
//! - el rename sobre un fixture mediano (200 ficheros × 20 funciones)
//!   tardaba ~690 ms parseando el árbol entero.

use std::path::Path;

use cognicode_core::application::commands::{
    ChangeSignatureCommand, ParameterDefinition, RenameSymbolCommand,
};
use cognicode_core::application::dto::RefactorAction;
use cognicode_core::application::graph_snapshot::load_or_build_graph;
use cognicode_core::application::services::analysis_service::AnalysisService;
use cognicode_core::application::services::refactor_service::{RefactorBasis, RefactorService};
use cognicode_core::interface::mcp::handlers::HandlerContext;
use cognicode_core::interface::mcp::handlers::refactor_handlers::handle_safe_refactor;
use cognicode_core::interface::mcp::schemas::SafeRefactorInput;

/// El grafo del dueño, para los tests que llaman al servicio directamente.
fn basis_for(ws: &Path) -> RefactorBasis {
    let service = AnalysisService::new();
    let loaded = load_or_build_graph(&service, ws, None, None, false)
        .expect("the analysis graph of the fixture");
    RefactorBasis {
        graph: loaded.graph,
        coverage: loaded.coverage,
        workspace_root: ws.to_path_buf(),
    }
}

/// AC #1 — un fichero de un directorio que el inventario declina
/// (`target/`, un directorio de `.gitignore`) no participa en el
/// rename: ni en el impacto ni en la negativa.
///
/// Antes del cambio, el `first_use_outside` del rename recorría con
/// `contained_walk` —sin filtros— y se negaba con «it is also used in
/// …/target/gen.rs». Medido contra el código viejo (RED):
/// `InvalidParameter("rename of 'helper' refused: it is also used in
/// …/target/gen.rs, and rename only edits one file")`.
#[test]
fn a_rename_does_not_refuse_because_of_a_file_outside_the_source_inventory() {
    let dir = tempfile::tempdir().unwrap();
    let ws = dir.path().join("ws");
    // El walker honra .gitignore dentro de un repositorio: la presencia
    // de .git es lo que activa las reglas.
    std::fs::create_dir_all(ws.join(".git")).unwrap();
    std::fs::create_dir_all(ws.join("target")).unwrap();
    std::fs::create_dir_all(ws.join("generated")).unwrap();
    std::fs::write(
        ws.join("lib.rs"),
        "pub fn helper() {}\npub fn run() { helper(); }\n",
    )
    .unwrap();
    // El artefacto de build nombra el mismo identificador…
    std::fs::write(ws.join("target").join("gen.rs"), "fn helper() {}\n").unwrap();
    // …y un directorio que el .gitignore declina, también.
    std::fs::write(ws.join(".gitignore"), "generated/\n").unwrap();
    std::fs::write(ws.join("generated").join("out.rs"), "fn helper() {}\n").unwrap();

    let ctx = HandlerContext::builder()
        .with_working_dir(ws.clone())
        .build();
    let input = SafeRefactorInput {
        action: RefactorAction::Rename,
        target: "helper".to_string(),
        params: Some(serde_json::json!({
            "new_name": "renamed_helper",
            "file_path": ws.join("lib.rs").to_string_lossy(),
        })),
    };

    let result = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(handle_safe_refactor(&ctx, input))
        .expect("the handler answers");
    assert!(
        result.success,
        "a rename is not refused because of what target/ or .gitignore \
         hold; the answer was: {:?}",
        result.error_message
    );
}

/// AC #2 — el impacto cuenta los callers reales del símbolo y no el
/// nodo falso `nombre:0:0` que fundaba a todos los homónimos.
///
/// Fixture: `worker` en `lib.rs` con DOS llamadas same-file, y un
/// homónimo `worker` en `other.rs`. El grafo del análisis resuelve cada
/// llamada al worker de su propio fichero, así que el impacto de
/// `lib.rs:worker` son sus dos callers — y el homónimo no entra.
///
/// Medido contra el código viejo (RED): «(1 symbols impacted)» — las
/// dos llamadas se perdían en el nodo falso `worker:0:0`.
#[test]
fn the_impact_counts_the_real_callers_and_not_a_fused_placeholder() {
    let dir = tempfile::tempdir().unwrap();
    let ws = dir.path().join("ws");
    std::fs::create_dir_all(&ws).unwrap();
    std::fs::write(
        ws.join("lib.rs"),
        "pub fn worker() {}\npub fn call_a() { worker(); }\npub fn call_b() { worker(); }\n",
    )
    .unwrap();
    std::fs::write(ws.join("other.rs"), "pub fn worker() {}\n").unwrap();

    let basis = basis_for(&ws);
    let service = RefactorService::new();
    let command = ChangeSignatureCommand {
        function_name: "worker".to_string(),
        file_path: ws.join("lib.rs").to_string_lossy().to_string(),
        new_parameters: vec![ParameterDefinition {
            name: "x".to_string(),
            type_annotation: Some("i32".to_string()),
            default_value: None,
        }],
    };

    let preview = service
        .change_signature(command, &basis)
        .expect("the preview of a symbol the graph holds");
    assert!(
        preview.risk_assessment.contains("(2 symbols impacted)"),
        "the impact counts the two real callers of lib.rs:worker, and \
         neither the homonym nor a fused placeholder; got: {}",
        preview.risk_assessment
    );
    // El homónimo no entra en el impacto: es otro símbolo.
    assert!(
        !preview
            .symbols_affected
            .iter()
            .any(|s| s.contains("other.rs")),
        "the homonym in other.rs is not in the impact of lib.rs:worker: {:?}",
        preview.symbols_affected
    );
}

/// AC #3 — el preview declara la base del grafo que contestó: la
/// palabra de la cobertura del dueño, y los ficheros que el árbol
/// tiene y el grafo no leyó. Un fichero ilegible es un hueco que se
/// nombra, no un silencio.
///
/// `broken.rs` no es UTF-8 válido: la lectura falla y la construcción
/// lo lista como hueco (`SkippedFile` con razón de lectura).
#[test]
fn an_unreadable_file_is_a_named_hole_in_the_preview() {
    let dir = tempfile::tempdir().unwrap();
    let ws = dir.path().join("ws");
    std::fs::create_dir_all(&ws).unwrap();
    std::fs::write(
        ws.join("lib.rs"),
        "pub fn helper() {}\npub fn run() { helper(); }\n",
    )
    .unwrap();
    // Bytes que no son UTF-8: el fichero existe, el árbol lo camina, la
    // lectura falla.
    std::fs::write(ws.join("broken.rs"), [0x66, 0x6e, 0x20, 0xff, 0xfe]).unwrap();

    let basis = basis_for(&ws);
    let service = RefactorService::new();
    let command = RenameSymbolCommand::new(
        "helper",
        "renamed_helper",
        ws.join("lib.rs").to_string_lossy().to_string(),
    );

    let preview = service
        .rename_symbol(command, &basis)
        .expect("the rename of a symbol used only in its own file");
    assert_eq!(
        preview.coverage_state, "partial",
        "a graph with an unreadable file is not complete"
    );
    assert!(
        preview
            .unreadable_files
            .iter()
            .any(|f| f.ends_with("broken.rs")),
        "the unreadable file is named as the hole it is: {:?}",
        preview.unreadable_files
    );
}

/// AC #3 (positivo) — sobre un árbol limpio, el preview declara la
/// cobertura que el dueño midió: «complete», sin huecos.
#[test]
fn a_clean_tree_declares_a_complete_basis() {
    let dir = tempfile::tempdir().unwrap();
    let ws = dir.path().join("ws");
    std::fs::create_dir_all(&ws).unwrap();
    std::fs::write(
        ws.join("lib.rs"),
        "pub fn helper() {}\npub fn run() { helper(); }\n",
    )
    .unwrap();

    let basis = basis_for(&ws);
    let service = RefactorService::new();
    let command = RenameSymbolCommand::new(
        "helper",
        "renamed_helper",
        ws.join("lib.rs").to_string_lossy().to_string(),
    );

    let preview = service.rename_symbol(command, &basis).unwrap();
    assert_eq!(preview.coverage_state, "complete");
    assert!(
        preview.unreadable_files.is_empty(),
        "a complete coverage names no holes: {:?}",
        preview.unreadable_files
    );
}

/// AC #4 — el coste de un rename sobre un fixture mediano, medido con
/// el grafo ya servido (el coste del rename, no el de la primera
/// construcción). Antes del cambio: ~690 ms parseando el árbol entero
/// con un walk sin filtros (evidencia RED). Fuera de la suite porque
/// mide tiempo.
#[test]
#[ignore]
fn rename_cost_on_a_medium_fixture() {
    let dir = tempfile::tempdir().unwrap();
    let ws = dir.path().join("ws");
    std::fs::create_dir_all(&ws).unwrap();
    for i in 0..200 {
        let mut src = String::new();
        for j in 0..20 {
            src.push_str(&format!(
                "pub fn f{i}_{j}() {{ f{i}_{other_j}(); }}\n",
                other_j = (j + 1) % 20
            ));
        }
        if i == 0 {
            src.push_str("pub fn helper() {}\n");
        }
        std::fs::write(ws.join(format!("mod{i}.rs")), src).unwrap();
    }

    let basis = basis_for(&ws);
    let service = RefactorService::new();
    let mut samples = Vec::new();
    for k in 0..5 {
        let command = RenameSymbolCommand::new(
            "helper",
            format!("renamed_helper_{k}"),
            ws.join("mod0.rs").to_string_lossy().to_string(),
        );
        let start = std::time::Instant::now();
        let preview = service.rename_symbol(command, &basis).unwrap();
        let elapsed = start.elapsed();
        assert_eq!(preview.coverage_state, "complete");
        samples.push(elapsed);
    }
    samples.sort();
    println!("rename on a medium fixture (200 files x 20 fns):");
    for (i, s) in samples.iter().enumerate() {
        println!("  sample {i}: {s:?}");
    }
    println!("median: {:?}", samples[2]);
}
