//! Las superficies por fichero dicen lo que no ataron.
//!
//! MEDIDO, antes de este fichero. El grafo por fichero sabe qué
//! referencias no ató —la caché las guarda y `merge_with_report` las
//! publica—, pero `build_local_graph` devuelve solo el grafo, y de ahí el
//! manejador `get_per_file_graph` del MCP y la orden `graph per-file` de
//! la CLI responden con símbolos y dependencias sin decir nada de ellas.
//!
//! Sobre el fixture real de Terraform, el grafo por fichero de `main.tf`
//! ató 37 referencias que no puede atar (`var.environment`,
//! `var.vpc_cidr`, …): el fichero las enuncia y su declaración vive en
//! otro fichero. Las dos superficies por fichero las respondían como
//! cero referencias pendientes, que es lo mismo que decir que el fichero
//! no enuncia ninguna.
//!
//! Las dos superficies tienen que decir lo mismo sobre el mismo
//! fichero. La cobertura de relaciones se mide en el grafo del proyecto
//! y no en el del fichero, así que dos superficies pueden contestar
//! cosas distintas sobre la misma pregunta — y la cobertura de un
//! fichero se lee en las dos.
//!
//! ## Lo que se exige
//!
//! Que cada superficie publique las referencias pendientes del fichero,
//! con su origen (dónde se escribe) y su motivo (por qué no se ató), y
//! que las dos publiquen lo mismo. El contrato JSON es el que viaja:
//! `pending_references`, el mismo nombre que usan el grafo completo y
//! `MergeCoverage`.

use cognicode_core::interface::cli::commands::{
    Cli, CliCommand, CommandExecutor, Format, GraphCommand,
};
use cognicode_core::interface::cli::result::CommandOutput;
use cognicode_core::interface::mcp::handlers::{HandlerContext, handle_get_per_file_graph};
use cognicode_core::interface::mcp::schemas::GetPerFileGraphInput;
use serde_json::Value;
use std::path::{Path, PathBuf};

/// El fixture real, copiado a un directorio de trabajo para que el
/// repositorio quede limpio. El contenido es el versionado: estos tests
/// corren contra lo que el repositorio realmente tiene.
fn copy_fixture(dir: &Path) -> PathBuf {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../sandbox/fixtures/terraform-iac");
    let target = dir.join("terraform-iac");
    copy_tree(&source, &target);
    target
}

fn copy_tree(source: &Path, target: &Path) {
    std::fs::create_dir_all(target).expect("mkdir");
    for entry in std::fs::read_dir(source).expect("read fixture") {
        let entry = entry.expect("entry");
        let path = entry.path();
        let destination = target.join(entry.file_name());
        if path.is_dir() {
            copy_tree(&path, &destination);
        } else {
            std::fs::copy(&path, &destination).expect("copy fixture");
        }
    }
}

/// Las referencias pendientes que publica una superficie.
fn published_pending(value: &Value) -> Vec<Value> {
    value["pending_references"]
        .as_array()
        .cloned()
        .unwrap_or_default()
}

/// Cada referencia dice dónde se escribe y por qué no se ató.
fn assert_each_reference_says_its_origin_and_its_reason(pending: &[Value]) {
    assert!(
        !pending.is_empty(),
        "la superficie tiene que publicar las referencias que no ató; publicó una lista vacía, \
         que es lo mismo que decir que el fichero no enuncia ninguna"
    );
    for reference in pending {
        assert!(
            reference["target"].as_str().is_some_and(|t| !t.is_empty()),
            "una referencia sin nombre no dice nada: {:?}",
            reference
        );
        assert!(
            reference["reason"].as_str().is_some(),
            "una referencia sin motivo no se puede	atender: {:?}",
            reference
        );
        assert!(
            reference["file"]
                .as_str()
                .is_some_and(|f| f.ends_with("main.tf")),
            "una referencia sin origen no se puede localizar: {:?}",
            reference
        );
    }
}

#[tokio::test]
async fn la_superficie_mcp_publica_las_referencias_que_no_ato() {
    let dir = tempfile::tempdir().expect("tempdir");
    let fixtures = copy_fixture(dir.path());
    let main = fixtures.join("main.tf");

    let ctx = HandlerContext::builder()
        .with_working_dir(dir.path())
        .build();

    let answer = handle_get_per_file_graph(
        &ctx,
        GetPerFileGraphInput {
            file_path: main.to_string_lossy().into_owned(),
        },
    )
    .await
    .expect("la superficie por fichero responde");

    let json = serde_json::to_value(&answer).expect("la respuesta es JSON");
    let pending = published_pending(&json);
    assert_each_reference_says_its_origin_and_its_reason(&pending);
}

#[tokio::test]
async fn la_superficie_de_la_cli_publica_las_referencias_que_no_ato() {
    let dir = tempfile::tempdir().expect("tempdir");
    let fixtures = copy_fixture(dir.path());
    let main = fixtures.join("main.tf");

    let output = CommandExecutor::execute(Cli {
        verbose: false,
        format: Format::Json,
        command: Some(CliCommand::Graph {
            command: GraphCommand::PerFile {
                file: main.to_string_lossy().into_owned(),
            },
        }),
    })
    .await
    .expect("la orden por fichero responde");

    let CommandOutput::Graph(result) = output else {
        panic!("la orden por fichero devuelve un resultado de grafo");
    };
    let json = serde_json::to_value(&result).expect("el resultado es JSON");
    let pending = published_pending(&json);
    assert_each_reference_says_its_origin_and_its_reason(&pending);
}

#[tokio::test]
async fn las_dos_superficies_dicen_lo_mismo_sobre_el_mismo_fichero() {
    let dir = tempfile::tempdir().expect("tempdir");
    let fixtures = copy_fixture(dir.path());
    let main = fixtures.join("main.tf");
    let file = main.to_string_lossy().into_owned();

    let ctx = HandlerContext::builder()
        .with_working_dir(dir.path())
        .build();
    let answer = handle_get_per_file_graph(
        &ctx,
        GetPerFileGraphInput {
            file_path: file.clone(),
        },
    )
    .await
    .expect("la superficie por fichero responde");
    let mcp = published_pending(&serde_json::to_value(&answer).expect("la respuesta es JSON"));

    let output = CommandExecutor::execute(Cli {
        verbose: false,
        format: Format::Json,
        command: Some(CliCommand::Graph {
            command: GraphCommand::PerFile { file },
        }),
    })
    .await
    .expect("la orden por fichero responde");
    let CommandOutput::Graph(result) = output else {
        panic!("la orden por fichero devuelve un resultado de grafo");
    };
    let cli = published_pending(&serde_json::to_value(&result).expect("el resultado es JSON"));

    assert!(
        !mcp.is_empty(),
        "si ninguna de las dos superficies publica nada, no hay nada que comparar: la pregunta \
         sobre el fichero se queda sin respuesta"
    );
    assert_eq!(
        mcp, cli,
        "las dos superficies por fichero tienen que decir lo mismo sobre el mismo fichero: \
         una cuenta lo que la otra no cuenta, y el consumidor no sabe cuál"
    );
}
