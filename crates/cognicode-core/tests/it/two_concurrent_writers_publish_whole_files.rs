//! Dos escritores concurrentes publican ficheros enteros.
//!
//! La escritura atómica garantiza que el destino nunca se observa a
//! medias: el cambio de nombre publica el contenido entero o nada. Lo
//! que NO garantiza es la actualización condicional: la versión se
//! juzga antes del cambio de nombre, y entre ese juicio y el renombrado
//! hay una ventana en la que otro escritor puede publicar. Dos
//! escritores que declaran la misma versión base pueden pasar los dos
//! el juicio y publicar los dos — gana el último, y el que pierde no
//! se entera.
//!
//! Este fichero caracteriza lo que pasa HOY, para que la decisión de
//! si hace falta lock se tome con el comportamiento medido y no con la
//! intuición (TASK-209): publicación atómica bajo concurrencia, sí;
//! actualización condicional, no — es best-effort aguas arriba.

use std::fs;
use std::path::Path;
use std::sync::Arc;

use cognicode_core::application::dto::WriteFileRequest;
use cognicode_core::application::services::file_operations::FileOperationsService;
use cognicode_core::domain::findings::sha256_hex as digest;
use cognicode_core::infrastructure::parser::syntax_analysis::TreeSitterSyntaxAnalysis;
use cognicode_core::infrastructure::verification::RustVerifier;
use cognicode_core::interface::mcp::security::InputValidator;

fn service(root: &Path) -> FileOperationsService {
    FileOperationsService::new(
        root.to_string_lossy().to_string(),
        Arc::new(InputValidator::new().with_workspace(vec![root.to_path_buf()])),
        Arc::new(RustVerifier::new()),
        Arc::new(TreeSitterSyntaxAnalysis::new()),
    )
}

/// El plazo de la regla 7: la escritura es un fichero local, el camino
/// sano tarda milisegundos; si un escritor no contesta en 10 s, el test
/// falla con el plazo y no cuelga la suite.
const WRITER_DEADLINE: std::time::Duration = std::time::Duration::from_secs(10);

#[test]
fn dos_escritores_concurrentes_dejan_el_fichero_entero_de_uno_de_ellos() {
    let dir = tempfile::TempDir::new().expect("a temporary directory");
    let file = dir.path().join("codigo.rs");
    fs::write(&file, "fn base() {}\n").expect("the base file is written");

    // Los dos escritores leyeron la misma versión base y declaran la misma.
    let base_version = digest(&fs::read(&file).expect("the base is readable"));
    let content_a = "fn escritor_a() {}\n";
    let content_b = "fn escritor_b() {}\n";

    let barrier = Arc::new(std::sync::Barrier::new(2));
    let (tx, rx) = std::sync::mpsc::channel();

    for (name, content) in [("a", content_a), ("b", content_b)] {
        let barrier = Arc::clone(&barrier);
        let tx = tx.clone();
        let file = file.clone();
        let dir = dir.path().to_path_buf();
        let base = base_version.clone();
        std::thread::spawn(move || {
            barrier.wait();
            let mut request =
                WriteFileRequest::new(file.to_string_lossy().to_string(), content.to_string());
            request.expected_content_hash = Some(base);
            let outcome = service(&dir).write_file(request);
            let _ = tx.send((name, outcome.is_ok()));
        });
    }
    drop(tx);

    // Los dos resultados llegan, o el plazo muere y el test lo dice.
    let mut oks = 0;
    for _ in 0..2 {
        match rx.recv_timeout(WRITER_DEADLINE) {
            Ok((_, ok)) => {
                if ok {
                    oks += 1;
                }
            }
            Err(_) => panic!(
                "un escritor no contestó en {WRITER_DEADLINE:?}: la escritura \
                 local no tiene por qué colgar"
            ),
        }
    }

    let final_content = fs::read_to_string(&file).expect("the file is readable");
    assert!(
        oks >= 1,
        "al menos un escritor publicó: el fichero no puede quedarse sin escritor"
    );
    assert!(
        final_content == content_a || final_content == content_b,
        "el fichero publicado es el contenido ENTERO de uno de los dos \
         escritores: la publicación atómica no deja nunca un fichero a \
         medias ni entrelazado. Quedó: {final_content:?}"
    );
    // La actualización condicional NO está garantizada: los dos writers
    // declararon la misma versión base y aun así el contenido de uno de
    // ellos fue sustituido sin que su llamada se lo dijera. Ese es el
    // comportamiento caracterizado en TASK-209, no un fallo de este test.
}
