//! Un workspace, un nombre.
//!
//! La ingesta y la caché de grafo derivaban la identidad del workspace
//! con dos criterios distintos: `workspace_id_for_path` hacía hash de
//! la ruta TAL COMO SE ESCRIBIÓ (DefaultHasher, sin canonicalizar) y
//! `workspace_key` hacía hash de la ruta CANÓNICA (SHA-256). El mismo
//! proyecto abierto por dos rutas que acaban en el mismo sitio tenía
//! dos identidades en la ingesta y una en la caché: dos respuestas a
//! una pregunta que sólo tiene una.
//!
//! Este fichero era el que señalaba la divergencia; la derivación
//! duplicada se retiró con su único consumidor — la fachada del
//! explorer retirado con la descomisión de TASK-177 — y lo que queda
//! aquí guardando es la propiedad del dueño único: el mismo proyecto,
//! por cualquier ruta que llegue al mismo sitio, es una entrada.

#[cfg(unix)]
mod routes {
    use std::path::PathBuf;

    /// El mismo proyecto alcanzado por dos rutas: la directa y un enlace.
    pub fn same_project_two_routes() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let dir = tempfile::tempdir().expect("tempdir");
        let direct = dir.path().join("project");
        std::fs::create_dir_all(&direct).expect("project dir");
        let link = dir.path().join("project-link");
        std::os::unix::fs::symlink(&direct, &link).expect("symlink");
        (dir, direct, link)
    }
}

#[cfg(unix)]
#[test]
fn la_caché_da_una_entrada_al_mismo_proyecto_por_cualquier_ruta() {
    use cognicode_core::application::graph_cache::{snapshot_path, workspace_key};

    let (_dir, direct, link) = routes::same_project_two_routes();
    let cache = _dir.path().join("cache");

    assert_eq!(
        workspace_key(&direct),
        workspace_key(&link),
        "el mismo proyecto, por ruta directa o por enlace, tiene una sola clave"
    );
    assert_eq!(
        snapshot_path(&cache, &direct),
        snapshot_path(&cache, &link),
        "el snapshot del proyecto es uno, sea cual sea la ruta con la que \
         se llegó"
    );
}
