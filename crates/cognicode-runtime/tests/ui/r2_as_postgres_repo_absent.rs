//! Compile-fail scenario R2 (e29-7 task-6).
//!
//! `PgBackend::as_postgres_repo` must be REMOVED from the trait. The
//! runtime's call sites migrate to `self.pg_repo` directly, so the
//! escape-hatch method that exposes the concrete `PostgresRepository`
//! through the backend abstraction no longer exists.
//!
//! This file must NOT compile: calling `b.as_postgres_repo()` on a
//! `&dyn PgBackend` must be a compile error.

use cognicode_runtime::PgBackend;

fn main() {
    let _ = |b: &dyn PgBackend| {
        let _ = b.as_postgres_repo();
    };
}
