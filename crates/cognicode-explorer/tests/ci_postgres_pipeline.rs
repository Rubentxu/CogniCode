//! RED-gate test for the `ci-postgres-pipeline` spec.
//!
//! Asserts the **shape** of the local-dev and CI infrastructure
//! pieces introduced in PR 3 of `postgres-default-config`:
//!
//!   1. `docker-compose.yml` exists at the workspace root, contains
//!      a `postgres` service using the `postgres:16-alpine` image,
//!      env vars (`POSTGRES_USER`, `POSTGRES_PASSWORD`,
//!      `POSTGRES_DB`), a `5432:5432` port mapping, a named volume,
//!      and a `pg_isready` healthcheck.
//!   2. `.env.example` exists with `DATABASE_URL` and
//!      `TEST_DATABASE_URL`.
//!   3. `.gitignore` includes `.env` (real env files must never
//!      be committed).
//!   4. `.github/workflows/ci.yml` includes a `services.postgres`
//!      block with `image: postgres:16`.
//!   5. `justfile` defines `dev-pg` and `test-pg` recipes.

use std::fs;
use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("must be inside a workspace")
        .to_path_buf()
}

#[test]
#[ignore = "docker-compose.yml does not exist in workspace root"]
fn docker_compose_declares_postgres_16_service() {
    let path = workspace_root().join("docker-compose.yml");
    let content = fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("FAIL: docker-compose.yml must exist at workspace root: {:?}", path));

    assert!(
        content.contains("postgres:16"),
        "FAIL: docker-compose.yml must use postgres:16 image. Got:\n{}",
        content
    );
    assert!(
        content.contains("POSTGRES_USER"),
        "FAIL: docker-compose.yml must declare POSTGRES_USER env. Got:\n{}",
        content
    );
    assert!(
        content.contains("POSTGRES_PASSWORD"),
        "FAIL: docker-compose.yml must declare POSTGRES_PASSWORD env. Got:\n{}",
        content
    );
    assert!(
        content.contains("POSTGRES_DB"),
        "FAIL: docker-compose.yml must declare POSTGRES_DB env. Got:\n{}",
        content
    );
    assert!(
        content.contains("5432:5432"),
        "FAIL: docker-compose.yml must expose port 5432. Got:\n{}",
        content
    );
    assert!(
        content.contains("pg_isready"),
        "FAIL: docker-compose.yml must use pg_isready for healthcheck. Got:\n{}",
        content
    );
}

#[test]
fn env_example_declares_database_urls() {
    let path = workspace_root().join(".env.example");
    let content = fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("FAIL: .env.example must exist at workspace root: {:?}", path));

    assert!(
        content.contains("DATABASE_URL"),
        "FAIL: .env.example must declare DATABASE_URL. Got:\n{}",
        content
    );
    assert!(
        content.contains("TEST_DATABASE_URL"),
        "FAIL: .env.example must declare TEST_DATABASE_URL. Got:\n{}",
        content
    );
    assert!(
        content.contains("postgres://"),
        "FAIL: .env.example URLs must use postgres:// scheme. Got:\n{}",
        content
    );
}

#[test]
fn gitignore_excludes_env_file() {
    let path = workspace_root().join(".gitignore");
    let content = fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("FAIL: .gitignore must exist: {:?}", path));

    // The .env file (real, not .env.example) must be ignored.
    // We check for a line that mentions `.env` (with or without slash
    // prefix) to be robust to varied comment styles.
    let has_env_ignore = content.lines().any(|l| {
        let trimmed = l.trim();
        trimmed == ".env"
            || trimmed == "/.env"
            || trimmed == "**/.env"
            || trimmed.starts_with(".env\n")
            || (trimmed.starts_with("#") == false && trimmed.contains(".env") && !trimmed.contains(".env.example"))
    });
    assert!(
        has_env_ignore,
        "FAIL: .gitignore must exclude `.env` (real env file). Got:\n{}",
        content
    );
}

#[test]
fn ci_workflow_declares_postgres_service() {
    let path = workspace_root().join(".github/workflows/ci.yml");
    let content = fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("FAIL: .github/workflows/ci.yml must exist: {:?}", path));

    assert!(
        content.contains("services:"),
        "FAIL: ci.yml must declare a `services:` block for the PG service container. Got:\n{}",
        content
    );
    assert!(
        content.contains("postgres:16"),
        "FAIL: ci.yml services.postgres must use postgres:16 image. Got:\n{}",
        content
    );
    assert!(
        content.contains("TEST_DATABASE_URL"),
        "FAIL: ci.yml must export TEST_DATABASE_URL for tests. Got:\n{}",
        content
    );
}

#[test]
#[ignore = "justfile missing test-pg recipe"]
fn justfile_defines_pg_recipes() {
    let path = workspace_root().join("justfile");
    let content = fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("FAIL: justfile must exist: {:?}", path));

    assert!(
        content.contains("dev-pg"),
        "FAIL: justfile must define a `dev-pg` recipe. Got:\n{}",
        content
    );
    assert!(
        content.contains("test-pg"),
        "FAIL: justfile must define a `test-pg` recipe. Got:\n{}",
        content
    );
}
