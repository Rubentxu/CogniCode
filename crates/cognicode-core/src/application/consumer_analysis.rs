//! CU-1.2 — Consumer analysis facade.
//!
//! Una única superficie de entrada para consumidores (CLI, MCP, SDK via
//! librería) que resuelve capacidades ANTES de ejecutar y delega la
//! ejecución SIEMPRE en el puerto `Analyze` (B2.W1). La facade:
//!
//! 1. **Resolución pura**: `CapabilityRequest` → `CapabilityPlan` sin
//!    I/O. Un workspace sin material para el grafo resuelve
//!    `Unsupported` (o `Partial` con gaps) que dice qué faltó, jamás
//!    `Complete` con grafo vacío (SPEC-CU-001.5).
//! 2. **Delegación**: la ejecución pasa por el `&dyn Analyze` inyectado.
//!    La facade nunca llama `AnalysisService` directamente (hexagonal).
//! 3. **Sin segundo ADT**: el resultado es el `AnalysisOutcome<T>`
//!    upstream tal cual (SPEC-CU-001.2).
//! 4. **Provenance del proveedor**: `provider_version()` expone la
//!    versión de `cognicode-core` desde su build real, no la del
//!    consumidor (SPEC-CU-001.6).
//!
//! Diseño de la resolución: la detección de material es deliberada-
//! mente barata — la misma clasificación que usa el scan
//! (`classify_file`: es material el fichero que un lenguaje del
//! registro reclama) sobre el recorrido con la política del grafo,
//! detenido en la primera fuente. Es la misma señal que decide qué
//! ficheros lee el producto; una lista propia de extensiones sería una
//! segunda respuesta a la misma pregunta. Un `Partial` aquí no impide
//! ejecutar: significa "ejecuta sabiendo que el backend de grafo puede
//! no tener material"; el outcome real lo decide el puerto.

use std::path::{Path, PathBuf};

use crate::application::analysis_basis::{
    AnalysisBasis, AnalysisOp, AnalysisOutcome, AnalysisValue, Analyze,
};
use crate::application::ingest::{FileType, classify_file};

// El adaptador de producción vive en `services::analyze` (donde B2.W1
// dijo que viviría). Se reexporta aquí para que el consumidor importe
// TODO desde `application::consumer_analysis`.
pub use crate::application::services::analyze::ServiceAnalyzer;

/// Petición tipada de capacidades de análisis (smart constructor).
///
/// `for_graph` construye la petición canónica del consumidor Guard:
/// grafo completo + complejidad sobre el basis dado.
#[derive(Debug, Clone)]
pub struct CapabilityRequest {
    basis: AnalysisBasis,
    capabilities: Vec<Capability>,
}

impl CapabilityRequest {
    /// Smart constructor: rechaza workspace raíz vacío.
    pub fn new(workspace_root: PathBuf) -> Result<Self, InvalidRequest> {
        if workspace_root.as_os_str().is_empty() {
            return Err(InvalidRequest::EmptyWorkspaceRoot);
        }
        Ok(Self {
            basis: AnalysisBasis::for_workspace(workspace_root),
            capabilities: Vec::new(),
        })
    }

    /// Petición de capacidades de grafo sobre un basis existente.
    pub fn for_graph(basis: &AnalysisBasis) -> Self {
        Self {
            basis: basis.clone(),
            capabilities: vec![Capability::Graph, Capability::Complexity],
        }
    }

    pub fn basis(&self) -> &AnalysisBasis {
        &self.basis
    }

    pub fn capabilities(&self) -> &[Capability] {
        &self.capabilities
    }

    pub fn with_capability(mut self, capability: Capability) -> Self {
        self.capabilities.push(capability);
        self
    }
}

/// Capability pedida al proveedor. ADT cerrado: no strings libres.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Capability {
    Graph,
    Complexity,
}

impl Capability {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Graph => "graph",
            Self::Complexity => "complexity",
        }
    }
}

/// Error de construcción de `CapabilityRequest`.
#[derive(Debug, thiserror::Error)]
pub enum InvalidRequest {
    #[error("workspace root no puede estar vacío")]
    EmptyWorkspaceRoot,
}

/// Plan resuelto ANTES de ejecutar. El vocabulario es deliberadamente
/// el de `OutcomeState` restringido a la fase de planificación: aquí no
/// hay `Failed` (aún no se ejecutó nada) ni `Unknown` (la señal de
/// disponibilidad es determinista para las capabilities actuales).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapabilityPlan {
    /// Todas las capabilities pedidas tienen material en el workspace.
    Complete { capabilities: Vec<Capability> },
    /// Algunas capabilities no tienen material; `gaps` nombra cuáles.
    Partial {
        capabilities: Vec<Capability>,
        gaps: Vec<String>,
    },
    /// Ninguna capability pedida es proveíble.
    Unsupported { reason: String },
}

impl CapabilityPlan {
    /// Los gaps del plan; vacío si `Complete`.
    pub fn gaps(&self) -> &[String] {
        match self {
            Self::Complete { .. } => &[],
            Self::Partial { gaps, .. } => gaps,
            Self::Unsupported { .. } => &[],
        }
    }
}

/// Facade de consumo. Mantiene el puerto inyectado y resuelve planes
/// puros; no posee estado mutable de análisis.
pub struct ConsumerAnalysisFacade<'a> {
    analyze: &'a dyn Analyze,
}

impl<'a> ConsumerAnalysisFacade<'a> {
    pub fn new(analyze: &'a dyn Analyze) -> Self {
        Self { analyze }
    }

    /// Resolución PURA de la petición en un plan. Sin I/O de análisis:
    /// la única inspección es estructural (¿hay material para el
    /// backend de grafo en el workspace raíz?).
    pub fn resolve(request: &CapabilityRequest) -> CapabilityPlan {
        let root = request.basis().workspace_root();
        let missing = match workspace_graph_material(root) {
            GraphMaterial::Present => None,
            GraphMaterial::NoReadableSources => Some(format!(
                "the files under {} are in no language the product reads; \
                 graph-based capabilities are unavailable",
                root.display()
            )),
            GraphMaterial::NoFiles => Some(format!(
                "no files found under {}; graph-based capabilities are unavailable",
                root.display()
            )),
        };

        let mut available: Vec<Capability> = Vec::new();
        let mut gaps: Vec<String> = Vec::new();
        for capability in request.capabilities() {
            match &missing {
                None => available.push(*capability),
                Some(reason) => gaps.push(reason.clone()),
            }
        }

        if available.is_empty() {
            return CapabilityPlan::Unsupported {
                reason: missing.unwrap_or_else(|| "no capabilities were requested".to_string()),
            };
        }
        if gaps.is_empty() {
            return CapabilityPlan::Complete {
                capabilities: available,
            };
        }
        CapabilityPlan::Partial {
            capabilities: available,
            gaps,
        }
    }

    /// Ejecuta una operación a través del puerto inyectado. El
    /// `AnalysisOutcome` upstream se devuelve TAL CUAL: sin
    /// re-envoltura, sin conversión de estados.
    pub fn analyze(
        &self,
        _request: &CapabilityRequest,
        op: AnalysisOp,
    ) -> AnalysisOutcome<AnalysisValue> {
        // La facade no re-valida el op: el puerto es el dueño de la
        // semántica de ejecución. La única responsabilidad aquí es que
        // el basis viaje con la petición.
        let request_basis = _request.basis();
        self.analyze.execute(request_basis, op)
    }

    /// Versión del proveedor (`cognicode-core`) desde su build real.
    /// SPEC-CU-001.6: el consumidor NO usa su propio
    /// `CARGO_PKG_VERSION` para identificar al proveedor.
    pub fn provider_version() -> &'static str {
        env!("CARGO_PKG_VERSION")
    }
}

/// La señal estructural del material del backend de grafo.
///
/// No es un booleano porque «no hay material» tiene dos causas que el
/// plan no puede mezclar: un árbol sin ficheros no es un árbol cuyos
/// ficheros están en un lenguaje que el producto no lee. El motivo del
/// plan dice cuál de las dos pasó.
enum GraphMaterial {
    /// Hay fuentes en lenguajes que el producto lee.
    Present,
    /// Hay ficheros, pero ninguno es código en un lenguaje del registro.
    NoReadableSources,
    /// El recorrido no vio ningún fichero: workspace vacío o inexistente.
    NoFiles,
}

/// Señal estructural: ¿hay material para el backend de grafo?
///
/// El grafo del proyecto recorre el árbol entero, así que la señal tiene
/// que mirar donde el grafo mira. Un proyecto cuyas fuentes viven en un
/// subdirectorio tiene material aunque en la raíz no haya nada, y un
/// manifiesto sin fuentes no tiene ninguno: prometer un grafo que va a
/// salir vacío es prometer un grafo que no existe.
///
/// Qué ficheros cuentan es la MISMA decisión que toma el scan: un
/// fichero es material cuando `classify_file` lo clasifica como código,
/// es decir, cuando un lenguaje del registro lo reclama. Una segunda
/// lista de extensiones sería una segunda respuesta a esa pregunta, y
/// las dos se separarían sin que nadie lo notara. El recorrido es el
/// mismo que usa el grafo —los ficheros ocultos y los ignorados quedan
/// fuera, los enlaces simbólicos no se siguen— y se detiene en la
/// primera fuente que encuentra.
fn workspace_graph_material(root: &Path) -> GraphMaterial {
    let mut saw_any_file = false;
    let has_material = crate::infrastructure::walk_policy::contained_ignore_walk(root)
        .hidden(true)
        .git_ignore(true)
        .git_exclude(true)
        .build()
        .filter_map(|entry| entry.ok())
        .filter(crate::infrastructure::walk_policy::ignore_is_real_file)
        .any(|entry| {
            saw_any_file = true;
            let (file_type, _) = classify_file(entry.path());
            file_type == FileType::Code
        });
    if has_material {
        GraphMaterial::Present
    } else if saw_any_file {
        GraphMaterial::NoReadableSources
    } else {
        GraphMaterial::NoFiles
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_root_is_rejected() {
        assert!(CapabilityRequest::new(PathBuf::from("")).is_err());
    }

    #[test]
    fn resolve_is_pure_for_missing_workspace() {
        let basis = AnalysisBasis::for_workspace(PathBuf::from("/nonexistent/xyz"));
        let request = CapabilityRequest::for_graph(&basis);
        let plan = ConsumerAnalysisFacade::resolve(&request);
        assert!(matches!(plan, CapabilityPlan::Unsupported { .. }));
    }

    fn plan_for(root: &std::path::Path) -> CapabilityPlan {
        let basis = AnalysisBasis::for_workspace(root.to_path_buf());
        let request = CapabilityRequest::for_graph(&basis);
        ConsumerAnalysisFacade::resolve(&request)
    }

    /// Un proyecto Rust con las fuentes en un subdirectorio tiene material:
    /// el grafo lo va a leer igual, así que el plan no puede negarse a
    /// ofrecer la capacidad.
    #[test]
    fn a_nested_rust_project_has_material() {
        let temp = tempfile::TempDir::new().expect("temporary directory");
        let backend = temp.path().join("backend");
        std::fs::create_dir_all(backend.join("src")).expect("create the nested project");
        std::fs::write(
            backend.join("Cargo.toml"),
            "[package]\nname = \"backend\"\n",
        )
        .expect("write the manifest");
        std::fs::write(backend.join("src/main.rs"), "fn main() {}\n").expect("write the source");

        let plan = plan_for(temp.path());
        assert!(
            !matches!(plan, CapabilityPlan::Unsupported { .. }),
            "un proyecto Rust anidado tiene material y el grafo lo lee: el plan no puede negarle la capacidad"
        );
    }

    /// Un manifiesto sin fuentes no puede prometer un grafo: el grafo
    /// saldría vacío, y un grafo vacío no es un grafo.
    #[test]
    fn a_manifest_without_sources_cannot_resolve_complete() {
        let temp = tempfile::TempDir::new().expect("temporary directory");
        std::fs::write(temp.path().join("Cargo.toml"), "").expect("write an empty manifest");

        let plan = plan_for(temp.path());
        assert!(
            !matches!(plan, CapabilityPlan::Complete { .. }),
            "un manifiesto sin fuentes no puede resolver Complete: el grafo saldría vacío"
        );
    }

    /// Un proyecto con fuentes Rust y sin manifiesto sí tiene material.
    #[test]
    fn rust_sources_without_a_manifest_have_material() {
        let temp = tempfile::TempDir::new().expect("temporary directory");
        std::fs::write(temp.path().join("main.rs"), "fn main() {}\n").expect("write the source");

        let plan = plan_for(temp.path());
        assert!(
            matches!(plan, CapabilityPlan::Complete { .. }),
            "las fuentes Rust son material, tengan o no manifiesto"
        );
    }

    /// Un proyecto Kotlin tiene material: el producto lo lee — el registro
    /// de lenguajes es la única lista que lo dice — y el grafo lo consume.
    /// La presencia de `.rs` no puede ser el indicador de disponibilidad:
    /// la señal es la misma que decide qué ficheros son código, no un tipo
    /// de fichero concreto.
    #[test]
    fn a_kotlin_project_has_material() {
        let temp = tempfile::TempDir::new().expect("temporary directory");
        std::fs::write(temp.path().join("main.kt"), "fun main() {}\n").expect("write the source");

        let plan = plan_for(temp.path());
        assert!(
            matches!(plan, CapabilityPlan::Complete { .. }),
            "un proyecto Kotlin tiene material y el grafo lo lee: el plan no \
             puede negarle la capacidad por no haber `.rs`. Plan: {plan:?}"
        );
    }

    /// Un segundo lenguaje fuera de Rust: la señal es el registro, no un
    /// conjunto de extensiones escrito a mano que un mutante podría
    /// recortar a uno.
    #[test]
    fn a_python_project_has_material() {
        let temp = tempfile::TempDir::new().expect("temporary directory");
        std::fs::write(temp.path().join("app.py"), "def main():\n    pass\n")
            .expect("write the source");

        let plan = plan_for(temp.path());
        assert!(
            matches!(plan, CapabilityPlan::Complete { .. }),
            "un proyecto Python tiene material: Plan: {plan:?}"
        );
    }

    /// Un workspace sin fuentes Rust no puede servir el backend de grafo.
    #[test]
    fn a_workspace_without_sources_the_product_reads_is_unsupported() {
        let temp = tempfile::TempDir::new().expect("temporary directory");
        std::fs::write(temp.path().join("README.md"), "# nothing\n").expect("write a file");

        let plan = plan_for(temp.path());
        assert!(
            matches!(plan, CapabilityPlan::Unsupported { .. }),
            "sin material para el grafo el backend no se puede ofrecer"
        );
    }

    /// El motivo dice QUÉ faltó: los ficheros que hay no se leen en
    /// ningún lenguaje del producto. «No Rust sources» era una respuesta
    /// sobre un lenguaje, no sobre el material.
    #[test]
    fn the_reason_says_what_was_missing_not_just_that_something_was() {
        let temp = tempfile::TempDir::new().expect("temporary directory");
        std::fs::write(temp.path().join("README.md"), "# nothing\n").expect("write a file");

        let plan = plan_for(temp.path());
        match plan {
            CapabilityPlan::Unsupported { reason } => assert!(
                reason.contains("no language the product reads"),
                "el motivo nombra el material que faltó, no un lenguaje: {reason}"
            ),
            other => panic!("esperaba Unsupported, obtuve {other:?}"),
        }
    }

    /// Y cuando el recorrido no vio ningún fichero, el motivo también lo
    /// distingue: no es lo mismo un árbol sin ficheros que un árbol con
    /// ficheros ilegibles.
    #[test]
    fn a_workspace_with_no_files_at_all_says_so() {
        let temp = tempfile::TempDir::new().expect("temporary directory");

        let plan = plan_for(temp.path());
        match plan {
            CapabilityPlan::Unsupported { reason } => assert!(
                reason.contains("no files found"),
                "un árbol sin ficheros declara que no había ficheros: {reason}"
            ),
            other => panic!("esperaba Unsupported, obtuve {other:?}"),
        }
    }
}

#[cfg(test)]
mod spy_tests {
    use super::*;
    use crate::application::analysis_basis::AnalysisError;

    struct StubAnalyze;

    impl Analyze for StubAnalyze {
        fn execute(
            &self,
            _basis: &AnalysisBasis,
            _op: AnalysisOp,
        ) -> AnalysisOutcome<AnalysisValue> {
            AnalysisOutcome::failed(AnalysisError::generic("stub"))
        }
    }

    #[test]
    fn facade_returns_port_outcome_verbatim() {
        let stub = StubAnalyze;
        let facade = ConsumerAnalysisFacade::new(&stub);
        let basis = AnalysisBasis::for_workspace(PathBuf::from("."));
        let request = CapabilityRequest::for_graph(&basis);
        let outcome = facade.analyze(
            &request,
            AnalysisOp::GraphFull {
                path: PathBuf::from("."),
                rebuild: false,
            },
        );
        assert!(outcome.error().is_some());
    }
}
