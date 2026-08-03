//! Integration test: the runtime's `PgBackend` trait object and
//! cross-backend compatibility surface.
//!
//! e29-7 task-7: the postgres arm (`PostgresBackend`) was dropped per
//! the spec coherence amendment — the concrete postgres backend no
//! longer exists. The trait is now implemented by `LadybugPgBackend`
//! only, and the escape-hatch `as_postgres_repo` method is gone
//! (verified by the R2 compile-fail test below).

use std::path::PathBuf;
use std::sync::Arc;

use cognicode_core::domain::ports::{CallGraphStore, QualityStore, ViewSpecStore};
use cognicode_runtime::{LadybugPgBackend, PgBackend};

/// `PgBackend` is a Send+Sync trait object implemented by the lbug-side
/// adapter. This is the foundation for the ladybug cutover where the
/// runtime picks the canonical backend at build time.
#[test]
fn pg_backend_trait_object_supports_ladybug_backend() {
    let _ladybug: Box<dyn PgBackend> = Box::new(LadybugPgBackend::new(
        None::<Arc<dyn QualityStore>>,
        None::<Arc<dyn ViewSpecStore>>,
        None::<Arc<dyn CallGraphStore>>,
    ));
}

/// R2 (moved from the task-1 RED scaffolding): compile-fail —
/// `PgBackend::as_postgres_repo` must be ABSENT from the trait. The
/// runtime's call sites use `self.pg_repo` directly, so the
/// escape-hatch method that exposed the concrete `PostgresRepository`
/// no longer exists.
#[test]
fn r2_as_postgres_repo_is_absent_from_pg_backend_trait() {
    let ui = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("ui");
    let t = trybuild::TestCases::new();
    t.compile_fail(
        ui.join("r2_as_postgres_repo_absent.rs")
            .to_str()
            .expect("ui path is valid utf8"),
    );
}
