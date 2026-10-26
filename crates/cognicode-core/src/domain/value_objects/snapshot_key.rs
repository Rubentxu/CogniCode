//! La identidad de un análisis: quién lo produjo, sobre qué y con qué forma.
//!
//! ## Por qué existe
//!
//! El producto persiste un snapshot del grafo y lo vuelve a servir cuando
//! las fuentes no han cambiado. Lo que el snapshot guardaba era el grafo y
//! el manifiesto de contenido; lo que no guardaba era quién lo había
//! calculado. Sin ese dato, un binario distinto —otra versión, otro commit,
//! otras capacidades— sirve el snapshot como si fuera suyo: el consumidor
//! recibe un grafo que calculó otro analizador, con otra ontología y otras
//! reglas, y nada en la respuesta lo dice.
//!
//! ## Qué es la clave
//!
//! La clave es la pregunta "¿esto sigue valiendo?" hecha comparable:
//!
//! - **qué se miró**: el workspace canónico;
//! - **quién lo miró**: el analizador —su versión, su commit y sus
//!   capacidades—;
//! - **con qué forma se escribió**: el esquema del snapshot.
//!
//! ## Qué NO es la clave, y por qué
//!
//! El contenido de los ficheros —qué bytes se leyeron— **no** está en la
//! clave, y no es un olvido: el snapshot guarda el manifiesto, y el
//! manifiesto ya se compara fichero a fichero cuando el producto decide si
//! el snapshot sigue valiendo. Meter aquí el mismo digest sería una segunda
//! fuente de verdad sobre los mismos bytes, y las dos se podrían separar.
//!
//! Lo mismo con la revisión del workspace analizado: el análisis es una
//! función de los bytes y del analizador, así que la revisión es redundante
//! una vez que los bytes están fijados, y pedirla sería invalidar la caché
//! en cada commit sin cambiar ninguna respuesta.

use serde::{Deserialize, Serialize};

/// El esquema del snapshot persistido.
///
/// El número dice con qué forma se escribió el fichero, y con eso decide
/// si se puede leer: un snapshot de otra forma no se decodea, y por eso no
/// se puede servir como si fuera de esta.
pub const SNAPSHOT_SCHEMA: &str = "cognicode.graph.cache/v4";

/// El analizador que produce un análisis.
///
/// Es la identidad del **proveedor**: el crate que calculates el grafo. No
/// es la del consumidor que pregunta, que no dice nada sobre cómo se
/// calculó la respuesta.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalyzerBuild {
    /// La versión del crate que hace el análisis.
    pub version: String,
    /// El commit exacto del que se construyó, cuando se pudo saber.
    ///
    /// `None` cuando el binario se construyó fuera de un checkout —un
    /// tarball de release, por ejemplo—. Es un hecho, no un hueco: el
    /// binario no tiene de dónde saberlo.
    pub commit: Option<String>,
    /// Las capacidades con las que se construyó, ordenadas.
    ///
    /// Las capacidades son parte de la identidad porque cambian lo que el
    /// analizador puede ver: un binario con la capa multimodal y otro sin
    /// ella no calculan lo mismo sobre los mismos bytes.
    pub features: Vec<String>,
}

impl AnalyzerBuild {
    /// La identidad del analizador que está en ejecución.
    pub fn current() -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION").to_string(),
            commit: option_env!("GIT_SHA")
                .map(str::trim)
                .filter(|sha| !sha.is_empty())
                .map(str::to_string),
            features: enabled_features(),
        }
    }
}

/// Las capacidades con las que se compiló el analizador.
///
/// La lista es explícita porque el lenguaje no deja preguntar en tiempo de
/// ejecución qué features se activaron. Cada nombre está declarado en
/// `Cargo.toml`: si se borra una feature de ahí, el `cfg!` deja de existir
/// y el compilador lo dice. Si se **añade** una feature y no se nombra
/// aquí, un contrato de CI se pone rojo —un analizador que no declara su
/// capacidad no se puede identificar—.
fn enabled_features() -> Vec<String> {
    let mut features: Vec<String> = Vec::new();
    if cfg!(feature = "evidence-cli-ladybug") {
        features.push("evidence-cli-ladybug".to_string());
    }
    if cfg!(feature = "evidence-kernel") {
        features.push("evidence-kernel".to_string());
    }
    if cfg!(feature = "multimodal") {
        features.push("multimodal".to_string());
    }
    if cfg!(feature = "ownership") {
        features.push("ownership".to_string());
    }
    if cfg!(feature = "persistence") {
        features.push("persistence".to_string());
    }
    if cfg!(feature = "program-analysis-server") {
        features.push("program-analysis-server".to_string());
    }
    if cfg!(feature = "rig") {
        features.push("rig".to_string());
    }
    if cfg!(feature = "test-support") {
        features.push("test-support".to_string());
    }
    features.sort();
    features
}

/// La identidad de un análisis.
///
/// Es un valor: dos claves iguales describen el mismo análisis, y se
/// comparan con `==`. No tiene identidad propia —no hay dos claves
/// distintas del mismo análisis— porque no hay ningún otro sitio donde
/// crearla.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotKey {
    /// El workspace canónico: qué se miró.
    pub workspace: String,
    /// El analizador: quién lo miró.
    pub analyzer: AnalyzerBuild,
    /// El esquema del snapshot: con qué forma se escribió.
    pub schema: String,
}

impl SnapshotKey {
    /// Construye una clave.
    ///
    /// Función pura: los mismos datos producen la misma clave, y la clave
    /// no depende de nada que no sea un dato. No lee el disco, no llama al
    /// reloj y no pregunta al sistema: quien construye la clave es el que
    /// trae los datos.
    pub fn new(
        workspace: impl Into<String>,
        analyzer: AnalyzerBuild,
        schema: impl Into<String>,
    ) -> Self {
        Self {
            workspace: workspace.into(),
            analyzer,
            schema: schema.into(),
        }
    }

    /// La clave del análisis que produce el analizador de ahora, sobre el
    /// workspace dado.
    pub fn for_current_analyzer(workspace: impl Into<String>) -> Self {
        Self::new(workspace, AnalyzerBuild::current(), SNAPSHOT_SCHEMA)
    }

    /// ¿El otro análisis lo produjo el mismo analizador, sobre el mismo
    /// workspace y con el mismo esquema?
    ///
    /// Es la pregunta que se puede responder sin leer el árbol, y la que
    /// decide si un snapshot puede servir **antes** de comprobar sus
    /// fuentes. El contenido no se compara aquí: lo compara el manifiesto,
    /// que ya está en el snapshot y que se revisa fichero a fichero.
    pub fn same_producer_as(&self, other: &SnapshotKey) -> bool {
        self.workspace == other.workspace
            && self.analyzer == other.analyzer
            && self.schema == other.schema
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn analyzer(version: &str) -> AnalyzerBuild {
        AnalyzerBuild {
            version: version.to_string(),
            commit: Some("abc1234".to_string()),
            features: vec!["persistence".to_string()],
        }
    }

    #[test]
    fn the_same_data_produces_the_same_key() {
        let a = SnapshotKey::new("/ws", analyzer("1.0.0"), "schema");
        let b = SnapshotKey::new("/ws", analyzer("1.0.0"), "schema");
        assert_eq!(a, b);
        assert!(a.same_producer_as(&b));
    }

    #[test]
    fn another_version_is_another_key() {
        let a = SnapshotKey::new("/ws", analyzer("1.0.0"), "schema");
        let b = SnapshotKey::new("/ws", analyzer("1.0.1"), "schema");
        assert_ne!(a, b);
        assert!(!a.same_producer_as(&b));
    }

    #[test]
    fn another_commit_is_another_key() {
        let mut other = analyzer("1.0.0");
        other.commit = Some("def5678".to_string());
        let a = SnapshotKey::new("/ws", analyzer("1.0.0"), "schema");
        let b = SnapshotKey::new("/ws", other, "schema");
        assert!(!a.same_producer_as(&b));
    }

    #[test]
    fn another_capability_set_is_another_key() {
        let mut other = analyzer("1.0.0");
        other.features.push("multimodal".to_string());
        let a = SnapshotKey::new("/ws", analyzer("1.0.0"), "schema");
        let b = SnapshotKey::new("/ws", other, "schema");
        assert!(!a.same_producer_as(&b));
    }

    #[test]
    fn another_workspace_is_another_key() {
        let a = SnapshotKey::new("/ws-a", analyzer("1.0.0"), "schema");
        let b = SnapshotKey::new("/ws-b", analyzer("1.0.0"), "schema");
        assert!(!a.same_producer_as(&b));
    }

    #[test]
    fn another_schema_is_another_key() {
        let a = SnapshotKey::new("/ws", analyzer("1.0.0"), "schema-a");
        let b = SnapshotKey::new("/ws", analyzer("1.0.0"), "schema-b");
        assert!(!a.same_producer_as(&b));
    }

    #[test]
    fn the_capabilities_are_declared_in_a_stable_order() {
        let features = enabled_features();
        let mut sorted = features.clone();
        sorted.sort();
        assert_eq!(features, sorted);
        assert!(
            !features.is_empty(),
            "un analizador sin ninguna capacidad declarada no se puede identificar"
        );
    }

    #[test]
    fn the_analyzer_names_its_own_version() {
        let current = AnalyzerBuild::current();
        assert_eq!(current.version, env!("CARGO_PKG_VERSION"));
    }
}
