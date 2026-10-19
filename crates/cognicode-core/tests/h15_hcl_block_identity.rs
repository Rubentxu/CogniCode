//! HCL: un bloque debe conservar su tipo y su nombre.
//!
//! MEDIDO 2026-10-19 sobre `sandbox/fixtures/terraform-iac/main.tf` con el
//! código anterior a este fichero: las 144 etiquetas de bloque eran
//! literalmente la palabra clave. `resource "aws_s3_bucket" "app_bucket"
//! { ... }` producía la etiqueta `"resource"`, y el tipo y el nombre
//! desaparecían ANTES de que `terraform_handler` pudiera verlos.
//!
//! Tres consecuencias, todas medidas, ninguna hipotetica:
//!
//!  1. `is_terraform_file` busca `"resource "` (con espacio). Una etiqueta
//!     `"resource"` sola no lo contiene, asi que el handler devolvia el
//!     resultado intacto: nunca se ejecutaba.
//!  2. Todos los bloques `resource` del fichero comparten etiqueta, luego
//!     comparten identidad salvo la linea. Dos recursos distintos son el
//!     mismo simbolo.
//!  3. Las aristas que si salian eran del camino generico
//!     (`call_types: ["expression"]`) y eran ruido: se observaron
//!     `Unresolved("environment}-alerts\"")` y `Unresolved("ecs_service_name\n
//!     }")`, que son contenidos de literales y fragmentos de fuente
//!     tratados como destinos de llamada.
//!
//! El arreglo no reescribe `terraform_handler`: le devuelve el nombre que
//! el parser ya habia destruido.

use cognicode_core::application::ingest::extractor::extract_file;
use cognicode_core::infrastructure::parser::language_config::HCL_CONFIG;

fn labels_for(source: &str) -> Vec<String> {
    let result = extract_file(&HCL_CONFIG, std::path::Path::new("main.tf"), source, "h15");
    result.nodes.iter().map(|n| n.label.clone()).collect()
}

#[test]
fn an_hcl_block_keeps_its_type_and_name() {
    let labels = labels_for(
        r#"
resource "aws_s3_bucket" "app_bucket" {
  bucket = "x"
}
"#,
    );
    assert!(
        labels
            .iter()
            .any(|l| l.contains("aws_s3_bucket") && l.contains("app_bucket")),
        "un bloque HCL debe conservar tipo y nombre; se obtuvo {labels:?}"
    );
}

#[test]
fn two_resources_of_the_same_type_are_not_the_same_symbol() {
    let labels = labels_for(
        r#"
resource "aws_s3_bucket" "app_bucket" {
  bucket = "x"
}

resource "aws_s3_bucket" "audit_bucket" {
  bucket = "y"
}
"#,
    );
    let has_app = labels.iter().any(|l| l.contains("app_bucket"));
    let has_audit = labels.iter().any(|l| l.contains("audit_bucket"));
    assert!(
        has_app && has_audit,
        "dos recursos del mismo tipo deben conservar nombres distintos; se obtuvo {labels:?}"
    );
}

#[test]
fn the_terraform_handler_becomes_reachable_and_unquotes() {
    // El guardia de `is_terraform_file` exige la palabra clave seguida de
    // espacio, asi que con la etiqueta completa el handler se ejecuta.
    // Su salida observable es la identidad IaC `tipo.nombre`, ya SIN
    // comillas: el literal HCL es `"aws_s3_bucket"`, y quedarse con las
    // comillas contaminaba el label y el NodeId.
    //
    // Antes de este cambio el guardia no pasaba nunca (la etiqueta era
    // solo "resource"), asi que esta asercion no podia observarse.
    let labels = labels_for(
        r#"
resource "aws_s3_bucket" "app_bucket" {
  bucket = "x"
}
"#,
    );
    assert!(
        labels.iter().any(|l| l == "aws_s3_bucket.app_bucket"),
        "el handler debe producir la identidad IaC sin comillas; se obtuvo {labels:?}"
    );
    assert!(
        !labels.iter().any(|l| l.contains('"')),
        "ninguna identidad debe conservar las comillas del literal HCL; se obtuvo {labels:?}"
    );
}

#[test]
fn the_real_fixture_yields_resolvable_block_identities() {
    // El fixture real, no un fragmento inventado: es el que la auditoria
    // nombro como prueba de la relacion `aws_s3_bucket_versioning ->
    // aws_s3_bucket`.
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../sandbox/fixtures/terraform-iac/main.tf");
    let Ok(source) = std::fs::read_to_string(&fixture) else {
        // El fixture no viaja en todos los empaquetados: no se inventa un fallo.
        return;
    };
    let labels = labels_for(&source);
    assert!(
        labels
            .iter()
            .any(|l| l.contains("aws_s3_bucket_versioning")),
        "el fixture real debe producir la identidad del recurso versionado; \
         se obtuvo {} etiquetas, ninguna con ese nombre",
        labels.len()
    );
    assert!(
        labels.iter().any(|l| l.contains("app_bucket")),
        "el fixture real debe producir la identidad del bucket de origen"
    );
}
