//! Un rechazo llega al cliente como un rechazo, no como un fallo interno.
//!
//! El servicio de ficheros rechaza peticiones: un camino fuera del workspace,
//! un patron que no es una expresion regular, un fichero que no existe. Esos
//! rechazos son decisiones del producto, no caidas, y el cliente tiene que
//! poder distinguirlos.
//!
//! El defecto estaba en el camino **con** colector de metricas --y el
//! servidor MCP lo inicializa--: los handlers aplanaban el error a un fallo
//! interno, de modo que un rechazo deliberado llegaba al cliente como
//! "Internal error". Sin colector, la categoria se conservaba. El defecto era
//! invisible a la suite entera porque la suite prueba el camino que ya
//! funcionaba.
//!
//! Este test inicializa el colector para poder examinar ese camino, y afirma
//! sobre lo que el cliente recibe: la cadena que el protocolo publica.

use cognicode_core::infrastructure::telemetry::{get_global_metrics, init_global_metrics};
use cognicode_core::interface::mcp::error::InterfaceError;
use cognicode_core::interface::mcp::file_ops_handlers::{
    handle_edit_file, handle_list_files, handle_read_file, handle_search_content, handle_write_file,
};
use cognicode_core::interface::mcp::handlers::{HandlerContext, HandlerError};
use cognicode_core::interface::mcp::schemas::{
    EditFileInput, ListFilesInput, ReadFileInput, SearchContentInput, WriteFileInput,
};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

/// Un caso: una herramienta, la peticion que rechaza, y la categoria que el
/// cliente tiene que ver.
///
/// El identificador existe para que un fallo diga **que** caso fallo: una
/// tabla sin identificadores propios no distingue un caso de otro.
struct Case {
    id: &'static str,
    request: Value,
    /// La categoria del rechazo, tal y como la nombra el error de la
    /// aplicacion.
    category: &'static str,
}

/// La peticion de una herramienta, en el idioma del consumidor: JSON.
fn request<T: DeserializeOwned>(value: Value) -> T {
    serde_json::from_value(value).expect("la peticion se entiende")
}

/// Lo que el cliente recibe de un rechazo.
///
/// La cadena es la que el protocolo publica: el error de la interfaz que lo
/// entrega. Si el rechazo llega como un fallo interno, la cadena lo dice.
fn what_the_client_sees(error: HandlerError) -> String {
    InterfaceError::from(error).to_string()
}

/// Envia la peticion a su herramienta y devuelve lo que el cliente ve.
///
/// Devuelve un `Err` con el motivo cuando la peticion no fue rechazada: una
/// peticion que se acepta no demuestra nada sobre como llega un rechazo.
async fn client_sees(ctx: &HandlerContext, case: &Case) -> Result<String, String> {
    let rejected: Result<(), HandlerError> = match case.id {
        "read_file" => handle_read_file(ctx, request::<ReadFileInput>(case.request.clone()))
            .await
            .map(|_| ()),
        "write_file" => handle_write_file(ctx, request::<WriteFileInput>(case.request.clone()))
            .await
            .map(|_| ()),
        "edit_file" => handle_edit_file(ctx, request::<EditFileInput>(case.request.clone()))
            .await
            .map(|_| ()),
        "search_content" => {
            handle_search_content(ctx, request::<SearchContentInput>(case.request.clone()))
                .await
                .map(|_| ())
        }
        "list_files" => handle_list_files(ctx, request::<ListFilesInput>(case.request.clone()))
            .await
            .map(|_| ()),
        other => return Err(format!("no hay caso para la herramienta {other}")),
    };
    match rejected {
        Ok(()) => Err("la peticion deberia haberse rechazado".to_string()),
        Err(error) => Ok(what_the_client_sees(error)),
    }
}

/// Los casos: cada herramienta de ficheros, con la peticion que rechaza.
///
/// Cada herramienta tiene su caso: el defecto estaba en los cinco handlers, y
/// un caso por handler es lo que demuestra que ninguno se queda atras.
fn cases(workspace: &std::path::Path) -> Vec<Case> {
    let fuera = "../fuera-del-workspace";
    vec![
        Case {
            id: "read_file",
            request: json!({
                "path": workspace.join("no-existe.txt").to_str().unwrap(),
            }),
            category: "Invalid parameter",
        },
        Case {
            id: "write_file",
            request: json!({
                "path": fuera,
                "content": "contenido",
            }),
            category: "Invalid parameter",
        },
        Case {
            id: "edit_file",
            request: json!({
                "path": fuera,
                "edits": [{ "oldString": "a", "newString": "b" }],
            }),
            category: "Invalid parameter",
        },
        Case {
            id: "search_content",
            request: json!({
                "pattern": "[",
                "regex": true,
            }),
            category: "Invalid parameter",
        },
        Case {
            id: "list_files",
            request: json!({
                "path": fuera,
            }),
            category: "Invalid parameter",
        },
    ]
}

/// El colector de metricas tiene que estar presente.
///
/// El defecto solo existe en el camino con colector. Si este test corriera sin
/// el, pasaria por el camino que ya funcionaba y no probaria nada: por eso
/// el fallo tiene que decir que el colector no estaba.
fn metrics_are_counting() {
    // El colector se inicializa una vez por proceso: un segundo intento dice
    // "ya inicializado", y eso aqui no es un fallo.
    let _ = init_global_metrics();
    assert!(
        get_global_metrics().is_some(),
        "el colector de metricas tiene que estar presente: sin el, la prueba \
         pasaria por el camino que ya funcionaba y no probaria nada"
    );
}

/// Reparte la peticion entre las herramientas y dice que fallo en cada caso.
///
/// Todos los fallos se terminan juntos: una tabla que se para en el primer caso
/// esconde los demas, y lo que hay que ver es si el defecto es de un handler
/// o de todos.
async fn examine(ctx: &HandlerContext, cases: &[Case]) -> Vec<String> {
    let mut fallos = Vec::new();
    for case in cases {
        let text = match client_sees(ctx, case).await {
            Ok(text) => text,
            Err(reason) => {
                fallos.push(format!("{}: {reason}", case.id));
                continue;
            }
        };
        if !text.contains(case.category) {
            fallos.push(format!(
                "{}: el rechazo llega al cliente como {text:?}, y deberia llegar \
                 con su categoria ({})",
                case.id, case.category
            ));
        }
        if text.contains("Internal error") {
            fallos.push(format!(
                "{}: el rechazo llega al cliente como un fallo interno: {text:?}. Un \
                 rechazo es una decision del producto, no una caida",
                case.id
            ));
        }
    }
    fallos
}

#[tokio::test]
async fn a_rejection_arrives_at_the_client_as_a_rejection() {
    metrics_are_counting();

    let workspace = tempfile::tempdir().expect("workspace");
    std::fs::write(workspace.path().join("notas.txt"), "contenido\n").expect("notas");
    let ctx = HandlerContext::builder()
        .with_working_dir(workspace.path().to_path_buf())
        .build();

    let cases = cases(workspace.path());
    assert_eq!(
        cases.len(),
        5,
        "los cinco casos estan: una tabla que no selecciona ninguno no es PASS"
    );
    let ids: std::collections::BTreeSet<&str> = cases.iter().map(|c| c.id).collect();
    assert_eq!(
        ids.len(),
        cases.len(),
        "cada caso tiene su propio identificador: si dos comparten, uno no se \
         esta probando"
    );

    let fallos = examine(&ctx, &cases).await;

    assert!(
        fallos.is_empty(),
        "un rechazo tiene que llegar al cliente como un rechazo:\n  - {}",
        fallos.join("\n  - ")
    );
}

/// La misma garantia, vista desde el otro lado: lo que el cliente recibe no
/// es un error que envuelve la categoria, sino la categoria.
///
/// El handler no puede inventar una categoria que el servicio no dio, ni
/// esconderla debajo de un "Internal error" que dice que el producto se
/// rompio.
#[tokio::test]
async fn the_category_the_client_sees_is_the_one_the_service_produced() {
    metrics_are_counting();

    let workspace = tempfile::tempdir().expect("workspace");
    let ctx = HandlerContext::builder()
        .with_working_dir(workspace.path().to_path_buf())
        .build();

    let cases = cases(workspace.path());
    let mut fallos = Vec::new();
    for case in &cases {
        let text = match client_sees(&ctx, case).await {
            Ok(text) => text,
            Err(reason) => {
                fallos.push(format!("{}: {reason}", case.id));
                continue;
            }
        };
        // La categoria viaja en el texto que el protocolo publica, y no solo
        // en el mensaje: "Invalid parameter: X" nombra la categoria y el
        // detalle; "Internal error: Invalid parameter: X" la esconde.
        let (category, detail) = match text.strip_prefix("application: ") {
            Some(rest) => match rest.split_once(": ") {
                Some((category, detail)) => (category, detail),
                None => ("", rest),
            },
            None => ("", text.as_str()),
        };
        if category.is_empty() {
            fallos.push(format!(
                "{}: el cliente no recibe una categoria: {text:?}",
                case.id
            ));
        }
        if detail.starts_with("Invalid parameter") {
            fallos.push(format!(
                "{}: el cliente recibe un error interno que envuelve la categoria: \
                 {text:?}",
                case.id
            ));
        }
    }

    assert!(
        fallos.is_empty(),
        "la categoria del rechazo tiene que llegar al cliente:\n  - {}",
        fallos.join("\n  - ")
    );
}
