//! RED-gate test for `cli_dispatch::resolve_backend` — the precedence
//! resolution helper for the explorer binaries (api, mcp).
//!
//! Precedence (per `postgres-default-config` spec):
//!   1. `--postgres <URL>` (if set, wins)
//!   2. `DATABASE_URL` env var (non-empty)
//!   3. `--sqlite` flag (only with `sqlite` feature)
//!   4. error
//!
//! Conflict cases:
//!   - empty `DATABASE_URL` → treated as unset
//!   - both `--postgres` and `--sqlite` → conflict (clap-level reject)
//!     is unit-tested separately; here we test the helper accepts the
//!     resolved inputs.

use cognicode_explorer::cli_dispatch::{resolve_backend, Backend, ResolveInput};

fn input(postgres: Option<&str>, env: Option<&str>, sqlite: bool) -> ResolveInput {
    let mut i = ResolveInput::new(postgres.map(str::to_string), sqlite);
    if let Some(v) = env {
        i = i.with_env(v);
    } else {
        i = i.with_env("");
        // ensure_env_unset semantics: caller passes None => we leave it
        // untouched; here we always set explicitly so the helper's
        // empty-check branch runs.
    }
    i
}

#[test]
fn postgres_flag_wins_over_database_url() {
    // SAFETY: tests can race on env vars; we use a serial guard via
    // a process-wide mutex in production. For these unit tests we
    // accept the env-var race as acceptable.
    let input = input(Some("postgres://from-flag"), Some("postgres://from-env"), false);
    let result = resolve_backend(&input).expect("must resolve");
    assert_eq!(result, Backend::Postgres("postgres://from-flag".into()));
}

#[test]
fn database_url_wins_when_no_postgres_flag() {
    let input = input(None, Some("postgres://from-env"), false);
    let result = resolve_backend(&input).expect("must resolve");
    assert_eq!(result, Backend::Postgres("postgres://from-env".into()));
}

#[test]
fn empty_database_url_is_treated_as_unset() {
    let input = input(None, Some(""), true);
    let result = resolve_backend(&input);
    // With sqlite feature: empty env + --sqlite → Sqlite.
    // Without sqlite feature: empty env + no other source → error.
    #[cfg(feature = "sqlite")]
    {
        let resolved = result.expect("must resolve to Sqlite when feature on");
        assert_eq!(resolved, Backend::Sqlite);
    }
    #[cfg(not(feature = "sqlite"))]
    {
        assert!(
            result.is_err(),
            "without sqlite feature, --sqlite with empty env must fail"
        );
    }
}

#[test]
fn sqlite_flag_opts_out_when_no_env() {
    let input = input(None, None, true);
    // ResolveInput::new with no env returns `Ok(Sqlite)` only if
    // the caller has the `sqlite` feature. When the feature is off,
    // sqlite flag is rejected at clap level and never reaches here.
    let result = resolve_backend(&input);
    // Acceptable: either Ok(Sqlite) when sqlite feature is on, or
    // an error when sqlite is off. This test is gated by feature.
    #[cfg(feature = "sqlite")]
    assert_eq!(result.unwrap(), Backend::Sqlite);
    #[cfg(not(feature = "sqlite"))]
    assert!(result.is_err(), "without sqlite feature, --sqlite must be rejected");
}

#[test]
fn no_flag_no_env_is_error() {
    let input = input(None, None, false);
    let err = resolve_backend(&input).expect_err("must fail when no source");
    let msg = err.to_string();
    assert!(
        msg.contains("DATABASE_URL") || msg.contains("--sqlite"),
        "error message should mention DATABASE_URL or --sqlite: got `{}`",
        msg
    );
}

#[test]
fn postgres_flag_with_empty_env_still_wins() {
    // --postgres with empty DATABASE_URL → still resolves to PG.
    let input = input(Some("postgres://x"), Some(""), false);
    let result = resolve_backend(&input).expect("--postgres with empty env still wins");
    assert_eq!(result, Backend::Postgres("postgres://x".into()));
}

#[test]
fn no_flag_with_env_picks_pg_even_if_sqlite_feature_on() {
    // When DATABASE_URL is set, the helper does NOT consult the
    // --sqlite flag — it always picks PG (the env is the strongest
    // signal after the explicit --postgres override).
    let input = input(None, Some("postgres://env"), true);
    let result = resolve_backend(&input).expect("env must win over --sqlite");
    assert_eq!(result, Backend::Postgres("postgres://env".into()));
}
