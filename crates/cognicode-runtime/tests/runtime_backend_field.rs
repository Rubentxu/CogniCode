//! Smoke test: verify the `Runtime` struct exposes the `backend`
//! field (the `PgBackend` trait object) after the e29-2
//! bootstrap-backend-migration.

#![cfg(feature = "postgres")]

#[test]
fn runtime_struct_has_backend_field() {
    // Compile-time check: `Runtime` must expose `pub backend:
    // Option<Arc<dyn PgBackend>>` so consumers can query which
    // storage backend is in use.
    fn _check(r: &cognicode_runtime::Runtime) {
        let _ = r.backend.as_ref().map(|_| ());
    }
    // This function compiles only if the field exists with the
    // expected type.
}
