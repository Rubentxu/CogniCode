//! RED-gate test for `cli_dispatch::resolve_backend` — the precedence
//! resolution helper for the explorer binaries (api, mcp).
//!
//! Precedence (per `postgres-default-config` spec):
//!   1. `--postgres <URL>` (if set, wins)
//!   2. `DATABASE_URL` env var (non-empty)
//!   3. error
//!
//! Conflict cases:
//!   - empty `DATABASE_URL` → treated as unset
//!   - both `--postgres` and `--sqlite` → conflict (clap-level reject)
//!     is unit-tested separately; here we test the helper accepts the
//!     resolved inputs.

use cognicode_explorer::cli_dispatch::{Backend, ResolveInput, resolve_backend};

fn input(postgres: Option<&str>, env: Option<&str>) -> ResolveInput {
    let mut i = ResolveInput::new(postgres.map(str::to_string));
    if let Some(v) = env {
        i = i.with_env(v);
    } else {
        i = i.with_env("");
    }
    i
}

#[test]
fn postgres_flag_wins_over_database_url() {
    let input = input(Some("postgres://from-flag"), Some("postgres://from-env"));
    let result = resolve_backend(&input).expect("must resolve");
    assert_eq!(result, Backend::Postgres("postgres://from-flag".into()));
}

#[test]
fn database_url_wins_when_no_postgres_flag() {
    let input = input(None, Some("postgres://from-env"));
    let result = resolve_backend(&input).expect("must resolve");
    assert_eq!(result, Backend::Postgres("postgres://from-env".into()));
}

#[test]
fn no_flag_no_env_is_error() {
    let input = input(None, None);
    let err = resolve_backend(&input).expect_err("must fail when no source");
    let msg = err.to_string();
    assert!(
        msg.contains("DATABASE_URL"),
        "error message should mention DATABASE_URL: got `{}`",
        msg
    );
}

#[test]
fn postgres_flag_with_empty_env_still_wins() {
    // --postgres with empty DATABASE_URL → still resolves to PG.
    let input = input(Some("postgres://x"), Some(""));
    let result = resolve_backend(&input).expect("--postgres with empty env still wins");
    assert_eq!(result, Backend::Postgres("postgres://x".into()));
}

#[test]
fn no_flag_with_env_picks_pg() {
    // When DATABASE_URL is set, the helper picks PG.
    let input = input(None, Some("postgres://env"));
    let result = resolve_backend(&input).expect("env must resolve to PG");
    assert_eq!(result, Backend::Postgres("postgres://env".into()));
}
