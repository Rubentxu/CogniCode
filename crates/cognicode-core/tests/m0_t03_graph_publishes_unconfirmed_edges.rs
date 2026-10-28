//! M0.T03 — el grafo publica aristas que el almacén nunca confirmó.
//!
//! El defecto que esta unidad cierra: una relación que el almacén
//! rechazó se descartaba con `.ok()`. El fallo no se contaba en ninguna
//! parte y no se publicaba en ninguna parte; el log publicaba el número
//! de relaciones que se le habían *ofrecido*, y ese número incluía las
//! que el almacén nunca confirmó. Un grafo que dice que tiene una arista
//! que nadie guardó es un grafo que dice una falsehood sobre lo que sabe.
//!
//! El almacén del producto acepta toda relación, así que el camino de la
//! negativa era inalcanzable y nadie podía observarlo. El commit anterior
//! (el seam) abrió ese camino; aquí se mide el defecto contra él.
//!
//! Lo que este test afirma:
//!   - el número de relaciones que el producto publica es el que el
//!     almacén contiene, no el que se le ofreció;
//!   - una relación rechazada no desaparece: el build no se declara
//!     completo sobre un grafo al que le falta una relación;
//!   - el log dice lo mismo que la respuesta.

use cognicode_core::application::services::analysis_service::AnalysisService;
use cognicode_core::domain::aggregates::{CallGraph, Symbol, SymbolId};
use cognicode_core::domain::services::CycleDetectionResult;
use cognicode_core::domain::traits::{DependencyError, DependencyRepository};
use cognicode_core::domain::value_objects::DependencyType;
use cognicode_core::infrastructure::graph::PetGraphStore;
use cognicode_core::infrastructure::graph::per_file_graph::BuildStatus;
use std::collections::HashSet;
use std::io::Write;
use std::sync::{Arc, Mutex};

// ---------------------------------------------------------------------------
// Lo que el almacén sabe: cuántas relaciones le ofrecieron, cuántas
// guardó y cuáles rechazó.
// ---------------------------------------------------------------------------

#[derive(Default, Debug)]
struct Tally {
    offered: usize,
    accepted: usize,
    refused: Vec<String>,
}

/// Un almacén que rechaza la primera relación que le ofrecen y acepta
/// todas las demás.
///
/// Es el mínimo que un almacén puede hacer y que el almacén del producto
/// nunca hace: negarse. Un almacén que acepta todo no puede mostrar
/// qué hace el producto con una negativa, porque nunca la hay.
struct RefusingRepository {
    inner: PetGraphStore,
    tally: Arc<Mutex<Tally>>,
}

impl RefusingRepository {
    fn new(tally: Arc<Mutex<Tally>>) -> Self {
        Self {
            inner: PetGraphStore::new(),
            tally,
        }
    }
}

impl DependencyRepository for RefusingRepository {
    fn add_dependency(
        &mut self,
        source_id: &SymbolId,
        target_id: &SymbolId,
        dependency_type: DependencyType,
    ) -> Result<(), DependencyError> {
        {
            let mut tally = self.tally.lock().unwrap();
            tally.offered += 1;
            if tally.offered == 1 {
                // La primera relación que le ofrecen es la que rechaza.
                tally
                    .refused
                    .push(format!("{} -> {}", source_id.as_str(), target_id.as_str()));
                return Err(DependencyError::InvalidDependency(
                    "the repository refused this relation".to_string(),
                ));
            }
            tally.accepted += 1;
        }
        self.inner
            .add_dependency(source_id, target_id, dependency_type)
    }

    fn add_symbol(&mut self, id: &SymbolId, symbol: &Symbol) -> Result<(), DependencyError> {
        self.inner.add_symbol(id, symbol)
    }

    fn remove_symbol(&mut self, id: &SymbolId) -> Option<Symbol> {
        self.inner.remove_symbol(id)
    }

    fn get_symbol(&self, id: &SymbolId) -> Option<&Symbol> {
        self.inner.get_symbol(id)
    }

    fn get_all_symbols(&self) -> Vec<Symbol> {
        self.inner.get_all_symbols()
    }

    fn find_impact_scope(&self, id: &SymbolId) -> HashSet<SymbolId> {
        self.inner.find_impact_scope(id)
    }

    fn find_dependents(&self, id: &SymbolId) -> HashSet<SymbolId> {
        self.inner.find_dependents(id)
    }

    fn find_dependencies(&self, id: &SymbolId) -> HashSet<SymbolId> {
        self.inner.find_dependencies(id)
    }

    fn detect_cycles(&self) -> CycleDetectionResult {
        self.inner.detect_cycles()
    }

    fn has_path(&self, source: &SymbolId, target: &SymbolId) -> bool {
        self.inner.has_path(source, target)
    }

    fn get_call_graph(&self) -> CallGraph {
        self.inner.get_call_graph()
    }

    fn symbol_count(&self) -> usize {
        self.inner.symbol_count()
    }

    fn dependency_count(&self) -> usize {
        self.inner.dependency_count()
    }
}

// ---------------------------------------------------------------------------
// Los tres tests de este fichero comparten el estado global de `tracing`.
//
// La caché de interés de los callsites es del proceso, no del hilo: un
// `info!` que otro test del mismo binario ejecutó primero sin suscriptor
// queda registrado como "nunca" y no vuelve a emitirse. Los tres builds
// se serializan con un cerrojo para que ninguno mida el log de otro.
// ---------------------------------------------------------------------------

static BUILD_LOCK: Mutex<()> = Mutex::new(());

/// Adquiere el cerrojo de build sin poisoned: un test que falla no puede
/// impedir que los otros dos midan.
fn build_lock() -> std::sync::MutexGuard<'static, ()> {
    BUILD_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

// ---------------------------------------------------------------------------
// Fixture: tres relaciones distintas, para que el número sea countable.
// ---------------------------------------------------------------------------

fn fixture(dir: &std::path::Path) {
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname = \"m0-t03-refusing-fixture\"\nversion = \"0.0.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("src").join("main.rs"),
        "fn main() { helper(); other(); }\nfn helper() -> u32 { 1 }\nfn other() -> u32 { helper() }\n",
    )
    .unwrap();
}

// ---------------------------------------------------------------------------
// Ley 1: el número publicado es el que el almacén contiene.
// ---------------------------------------------------------------------------

#[test]
fn the_published_relation_count_is_the_one_the_repository_holds() {
    let _guard = build_lock();
    let tmp = tempfile::tempdir().unwrap();
    fixture(tmp.path());

    let tally = Arc::new(Mutex::new(Tally::default()));
    let service = AnalysisService::new().with_repository_factory({
        let tally = tally.clone();
        move || Box::new(RefusingRepository::new(tally.clone())) as Box<dyn DependencyRepository>
    });
    service.build_project_graph(tmp.path()).unwrap();

    let tally = tally.lock().unwrap();
    assert!(
        tally.offered > 0,
        "el fixture debe ofrecer relaciones al almacén; si no, la comparación es vacua"
    );
    assert_eq!(
        tally.accepted + tally.refused.len(),
        tally.offered,
        "toda relación ofrecida está o bien guardada o bien rechazada: ninguna se pierde en el camino"
    );
    assert_eq!(
        tally.refused.len(),
        1,
        "el almacén debe haber rechazado exactamente una relación"
    );

    let published = service.get_graph_stats().edge_count;
    assert_eq!(
        published, tally.accepted,
        "el número de relaciones publicado debe ser el que el almacén contiene, \
         no el que se le ofreció (se ofrecieron {})",
        tally.offered
    );
}

// ---------------------------------------------------------------------------
// Ley 2: una relación rechazada no se descarta en silencio.
// ---------------------------------------------------------------------------

#[test]
fn a_refused_relation_is_published_and_not_swallowed() {
    let _guard = build_lock();
    let tmp = tempfile::tempdir().unwrap();
    fixture(tmp.path());

    let tally = Arc::new(Mutex::new(Tally::default()));
    let service = AnalysisService::new().with_repository_factory({
        let tally = tally.clone();
        move || Box::new(RefusingRepository::new(tally.clone())) as Box<dyn DependencyRepository>
    });
    service.build_project_graph(tmp.path()).unwrap();

    let refused = tally.lock().unwrap().refused.len();
    assert_eq!(refused, 1, "el almacén debe haber rechazado una relación");

    let report = service
        .get_last_build_report()
        .expect("un build que publica un grafo debe publicar su reporte");
    assert!(
        !matches!(report.status, BuildStatus::Complete),
        "una relación que el almacén rechazó no puede desaparecer en silencio: \
         el build no puede declararse completo sobre un grafo al que le falta esa relación"
    );
}

// ---------------------------------------------------------------------------
// Ley 3: el log dice lo mismo que la respuesta.
// ---------------------------------------------------------------------------

#[derive(Clone, Default)]
struct LogSink(Arc<Mutex<Vec<u8>>>);

struct LogWriter(Arc<Mutex<Vec<u8>>>);

impl Write for LogWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for LogSink {
    type Writer = LogWriter;

    fn make_writer(&'a self) -> Self::Writer {
        LogWriter(self.0.clone())
    }
}

/// El número de relaciones que el log dice haber resuelto.
fn logged_relation_count(log: &str) -> Option<usize> {
    let stage = log.find("stage=resolve")?;
    let after = &log[stage..];
    let label = after.find("relationships")?;
    let before = after[..label].trim_end();
    let digits: String = before
        .chars()
        .rev()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    let digits: String = digits.chars().rev().collect();
    digits.parse().ok()
}

#[test]
fn the_log_publishes_the_same_relation_count_as_the_response() {
    let _guard = build_lock();
    let tmp = tempfile::tempdir().unwrap();
    fixture(tmp.path());

    let tally = Arc::new(Mutex::new(Tally::default()));
    let service = AnalysisService::new().with_repository_factory({
        let tally = tally.clone();
        move || Box::new(RefusingRepository::new(tally.clone())) as Box<dyn DependencyRepository>
    });

    let sink = LogSink::default();
    let subscriber = tracing_subscriber::fmt()
        .with_writer(sink.clone())
        .with_ansi(false)
        .with_max_level(tracing::Level::INFO)
        .finish();
    tracing::subscriber::with_default(subscriber, || {
        service.build_project_graph(tmp.path()).unwrap();
    });

    let log = String::from_utf8_lossy(&sink.0.lock().unwrap()).to_string();
    let logged = logged_relation_count(&log).unwrap_or_else(|| {
        panic!(
            "el build debe registrar cuántas relaciones resolvió; log capturado: {:?}",
            log
        )
    });
    let published = service.get_graph_stats().edge_count;

    assert_eq!(
        logged, published,
        "el log dice lo mismo que la respuesta: el log publicó {} relaciones y \
         la respuesta publica {}",
        logged, published
    );
}
