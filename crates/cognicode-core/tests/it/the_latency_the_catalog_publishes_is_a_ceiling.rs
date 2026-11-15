//! La latencia que el catálogo publica es un TECHO, y el techo de una
//! herramienta que recorre el workspace cubre el recorrido medido.
//!
//! ## El defecto
//!
//! `estimated_latency_ms` no decía qué era. El catálogo declaraba 40 ms
//! para `search_content` y 20 ms para `list_files`, y las dos herramientas
//! hacen el MISMO recorrido del árbol: sobre este repositorio, el recorrido
//! lee 3 057 ficheros y tarda 645 ms. El número publicaba una latencia
//! nominal, y un consumidor que lo leyera como un coste real decidiría
//! mal: esperaría 40 ms por un trabajo que tarda 645.
//!
//! ## La decisión
//!
//! El número es un TECHO —el presupuesto que el producto concede al
//! trabajo de la herramienta—, y no el coste típico de una llamada. Un
//! coste típico depende del tamaño del workspace y de la máquina, así que
//! un número publicado como coste típico sería un número que el producto no
//! puede cumplir en el siguiente workspace. El techo es un presupuesto, y un
//! presupuesto es el mismo en toda máquina.
//!
//! ## Qué se mide
//!
//! 1. El techo de las dos herramientas que recorren el workspace cubre el
//!    recorrido medido (645 ms sobre este repositorio).
//! 2. Las dos declaran el MISMO techo: hacen el mismo recorrido, y dos
//!    números distintos para el mismo trabajo serían dos verdades.
//! 3. La herramienta que dice de dónde sale su número lo dice de verdad:
//!    la base nombra la medición.
//!
//! ## Lo que el tope no es
//!
//! No es una promesa de que el trabajo tarde eso: es el techo que el
//! despacho concede, y el plazo del despacho lo cubre. El contrato de que
//! el plazo cubre el techo declarado vive en el servidor (`cognicode-mcp`),
//! donde está la ventana de cada categoría.

use cognicode_core::interface::catalog::load_tool_catalog;

/// El recorrido medido del workspace, en milisegundos.
///
/// MEDIDO el 2026-11-14 sobre este repositorio: el recorrido del árbol lee
/// 3 057 ficheros y tarda 645 ms. La cifra es una constante del test, no
/// una medición que la suite repita: una medición de latencia depende del
/// hardware y de la carga, y un test que depende de eso no es determinista.
const MEASURED_WALK_MS: u32 = 645;

/// Las dos herramientas que recorren el workspace.
const WORKSPACE_WALKS: [&str; 2] = ["list_files", "search_content"];

/// La herramienta que recorre el workspace publica un techo que cubre el
/// recorrido medido.
#[test]
fn the_workspace_walks_publish_a_ceiling_that_covers_the_measured_walk() {
    let catalog = load_tool_catalog();
    for name in WORKSPACE_WALKS {
        let tool = catalog
            .iter()
            .find(|tool| tool.name == name)
            .unwrap_or_else(|| panic!("the catalogue declares `{name}`"));
        assert!(
            tool.estimated_latency_ms >= MEASURED_WALK_MS,
            "`{name}` publishes a ceiling of {}ms and the walk of the workspace measures \
             {MEASURED_WALK_MS}ms: the ceiling has to cover the work it bounds",
            tool.estimated_latency_ms
        );
    }
}

/// Las dos herramientas que hacen el mismo recorrido declaran el mismo
/// techo: dos números para el mismo trabajo son dos verdades.
#[test]
fn the_two_workspace_walks_declare_the_same_ceiling() {
    let catalog = load_tool_catalog();
    let ceilings: Vec<u32> = WORKSPACE_WALKS
        .iter()
        .map(|name| {
            catalog
                .iter()
                .find(|tool| &tool.name == name)
                .unwrap_or_else(|| panic!("the catalogue declares `{name}`"))
                .estimated_latency_ms
        })
        .collect();
    assert_eq!(
        ceilings[0], ceilings[1],
        "the two tools walk the same tree, so they publish the same ceiling"
    );
}

/// La base de un techo medido nombra la medición: un número que no dice de
/// dónde sale es una suposición, y una suposición publicada como techo es
/// justo lo que esta tarea quita de en medio.
#[test]
fn the_base_of_a_measured_ceiling_names_the_measurement() {
    let catalog = load_tool_catalog();
    for name in WORKSPACE_WALKS {
        let tool = catalog
            .iter()
            .find(|tool| tool.name == name)
            .unwrap_or_else(|| panic!("the catalogue declares `{name}`"));
        let basis = tool
            .latency_basis
            .as_deref()
            .unwrap_or_else(|| panic!("`{name}` publishes a measured ceiling and says so"));
        assert!(
            basis.contains("measured") && basis.contains("ms"),
            "the basis of `{name}` says where the number comes from: {basis:?}"
        );
    }
}
