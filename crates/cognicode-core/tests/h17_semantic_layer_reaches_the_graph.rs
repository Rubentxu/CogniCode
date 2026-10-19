//! H17 — la capa semántica tiene que llegar al grafo que consulta el usuario.
//!
//! MEDIDO 2026-10-19 sobre `PerFileGraphCache::merge_with_report` con un
//! `main.tf` de dos bloques. Símbolos que salen:
//!
//! ```text
//! .../main.tf:resource:1  name="resource"
//! .../main.tf:resource:5  name="resource"
//! .../main.tf:bucket:2    name="bucket"
//! .../main.tf:bucket:6    name="bucket"
//! ```
//!
//! Cuatro símbolos, ninguno con la identidad del bloque. HCL está
//! anunciado en `product/languages.json` y su bloque produce dos
//! símbolos: la palabra clave y el nombre del atributo.
//!
//! ## Por qué
//!
//! `LanguageConfig::semantic_handler` existe y está cableado —para HCL y
//! para Ansible— pero se invoca en UN solo sitio:
//! `application/ingest/extractor.rs:238`. El camino que construye el
//! grafo (`infrastructure/graph/per_file_graph.rs`, vía
//! `TreeSitterParser::new`) nunca lo consulta. Son dos autoridades para
//! la misma pregunta («qué es un símbolo en este lenguaje») y la
//! segunda gana porque es la que se pregunta.
//!
//! Esto no es un defecto de HCL: es que un seam construido para ser
//! opcional se quedó opcional en un sentido que nadie eligió. H15
//! (PR #387) comprobó que el parser entregaba la identidad del bloque —
//! sobre `extract_file`, el seam— y su comentario daba por hecho que
//! llegar al grafo era consecuencia. No lo era. Este fichero mueve la
//! ley del seam al producto.
//!
//! ## La forma
//!
//! El cuarto test no es sobre Terraform: es sobre un lenguaje que NO
//! tiene capa semántica. El seam es `Option`, y un `Option` que nadie
//! ejercita en su rama `None` no es un seam: es un `if` que se puede
//! colar en el camino caliente sin que ninguna suite lo note. Este
//! fichero obliga a que ambos lados estén probados.

use cognicode_core::domain::aggregates::call_graph::CallGraph;
use cognicode_core::infrastructure::graph::per_file_graph::PerFileGraphCache;

const TF_ONE_BLOCK: &str = r#"
resource "aws_s3_bucket" "only" {
  bucket = "x"
}
"#;

const TF_TWO_BLOCKS: &str = r#"
resource "aws_s3_bucket" "app_bucket" {
  bucket = "x"
}

resource "aws_s3_bucket" "audit_bucket" {
  bucket = "y"
}
"#;

/// Nombres de símbolo del grafo, ordenados: compara sin depender del
/// tempdir.
fn symbol_names(graph: &CallGraph) -> Vec<String> {
    let mut names: Vec<String> = graph
        .symbol_ids()
        .map(|(_, s)| s.name().to_string())
        .collect();
    names.sort();
    names
}

fn graph_for(dir: &std::path::Path, files: &[(&str, &str)]) -> CallGraph {
    let mut paths = Vec::new();
    for (name, content) in files {
        let p = dir.join(name);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).expect("mkdir");
        }
        std::fs::write(&p, content).expect("write fixture");
        paths.push(p);
    }
    let cache = PerFileGraphCache::with_project_dir(dir.to_string_lossy().into_owned());
    let borrowed: Vec<&std::path::Path> = paths.iter().map(|p| p.as_path()).collect();
    cache.merge_with_report(&borrowed).graph
}

/// La ley central: el bloque llega al grafo con su identidad de dominio,
/// no con la palabra clave de su cabecera.
///
/// El conjunto es EXACTO, no una coincidencia: un símbolo de más
/// significa que algo del camino genético sobrevive mezclado con la capa
/// semántica, que es exactamente el defecto que este seam elimina.
#[test]
fn a_terraform_block_reaches_the_graph_with_its_domain_identity() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let graph = graph_for(tmp.path(), &[("main.tf", TF_ONE_BLOCK)]);

    let names = symbol_names(&graph);
    assert_eq!(
        names,
        vec!["aws_s3_bucket.only".to_string()],
        "un bloque HCL debe llegar al grafo como exactamente un símbolo con su \
         identidad de dominio. Un símbolo llamado `resource` significa que el camino \
         del grafo sigue preguntando al parser genérico; uno de más significa que \
         las dos autoridades se están mezclando."
    );
}

/// Un bloque, un símbolo. La extracción genérica entrega el nodo del
/// bloque dos veces (misma línea, misma columna), y el handler lo
/// convertía en dos nodos con el MISMO id: el conteo de símbolos de un
/// repo de IaC salía inflado y sin que nada lo delatara.
#[test]
fn one_terraform_block_is_one_symbol() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let graph = graph_for(tmp.path(), &[("main.tf", TF_ONE_BLOCK)]);

    let occurrences = symbol_names(&graph)
        .iter()
        .filter(|n| *n == "aws_s3_bucket.only")
        .count();
    assert_eq!(
        occurrences, 1,
        "un bloque produce un símbolo; se contaron {occurrences}. \
         La extracción genérica entrega el nodo del bloque dos veces y, \
         sin deduplicar, el grafo cuenta cada bloque de dos."
    );
}

/// Dos bloques del mismo tipo no son el mismo símbolo — la razón por la
/// que H15 arregló la etiqueta, retomada sobre el grafo.
#[test]
fn two_terraform_blocks_of_the_same_type_stay_distinct() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let graph = graph_for(tmp.path(), &[("main.tf", TF_TWO_BLOCKS)]);

    let names = symbol_names(&graph);
    assert!(
        names.iter().any(|n| n == "aws_s3_bucket.app_bucket")
            && names.iter().any(|n| n == "aws_s3_bucket.audit_bucket"),
        "dos recursos del mismo tipo deben llegar como dos símbolos; se obtuvo {names:?}"
    );
}

/// La frontera. Un lenguaje SIN capa semántica no puede enterarse de que
/// existe: mismo símbolo, misma arista, mismo nombre. Este test no es
/// sobre Terraform a propósito — es la garantía de que el seam no se ha
/// colado en el camino caliente de los otros veintisiete lenguajes.
#[test]
fn a_language_without_a_semantic_layer_is_untouched() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let rust = "pub fn alpha() -> u32 { 1 }\npub fn caller() -> u32 { alpha() }\n";
    let graph = graph_for(tmp.path(), &[("lib.rs", rust)]);

    let names = symbol_names(&graph);
    assert!(
        names.iter().any(|n| n == "alpha") && names.iter().any(|n| n == "caller"),
        "el camino genérico debe seguir produciendo sus símbolos; se obtuvo {names:?}"
    );
    assert!(
        graph.edge_count() >= 1,
        "`caller` llama a `alpha`: la arista genérica debe seguir existiendo; \
         se contaron {} aristas",
        graph.edge_count()
    );
}
