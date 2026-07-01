//! Contract test for the Pin Evidence endpoint (ADR-005 E21-2).
//!
//! Tests the `POST /api/investigations/:id/evidence` REST handler
//! with a real PostgreSQL database.

#![cfg(all(test, feature = "postgres"))]

use std::sync::atomic::{AtomicU64, Ordering};

use axum::{
    body::Body,
    http::{HeaderMap, Method, StatusCode},
    Router,
};
use cognicode_core::infrastructure::persistence::PostgresRepository;
use serde_json::json;
use sqlx::PgPool;
use tower::ServiceExt;

static UNIQ: AtomicU64 = AtomicU64::new(0);

/// Build a fresh per-test PostgreSQL database.
async fn fresh_test_url() -> Option<(String, PgPool)> {
    let base = std::env::var("TEST_DATABASE_URL").ok()?;
    let n = UNIQ.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    let db_name = format!("cognicode_pin_evidence_test_{pid}_{n}");

    let admin_url = base.clone();
    let test_url = rewrite_db_name(&admin_url, &db_name);

    let admin = sqlx::PgPool::connect(&admin_url).await.ok()?;
    let _ = sqlx::query(&format!("DROP DATABASE IF EXISTS \"{db_name}\""))
        .execute(&admin)
        .await;
    sqlx::query(&format!("CREATE DATABASE \"{db_name}\""))
        .execute(&admin)
        .await
        .ok()?;
    admin.close().await;

    let pool = sqlx::PgPool::connect(&test_url).await.ok()?;

    // Run the embedded schemas.
    let m0013 = include_str!("../../cognicode-core/src/infrastructure/persistence/m0013_investigation.sql");
    let m0014 = include_str!("../../cognicode-core/src/infrastructure/persistence/m0014_investigation_sessions.sql");

    for sql in [m0013, m0014] {
        for stmt in sql.split(';') {
            let stmt = stmt.trim();
            if stmt.is_empty() {
                continue;
            }
            sqlx::query(stmt).execute(&pool).await.ok()?;
        }
    }

    Some((test_url, pool))
}

fn rewrite_db_name(base: &str, db_name: &str) -> String {
    use regex::Regex;
    let re = Regex::new(r"^postgres://([^@]+)@([^/]+)(/[^?]*)?").unwrap();
    re.replace(base, format!(r"postgres://$1@$2/{}", db_name)).to_string()
}

async fn setup_app() -> (Router, PgPool) {
    let Some((test_url, pool)) = fresh_test_url().await else {
        panic!("TEST_DATABASE_URL must be set");
    };

    let repo = PostgresRepository::new(pool.clone());
    cognicode_explorer::api::router_with_state(
        cognicode_explorer::api::ApiState::builder()
            .with_repository(repo)
            .build(),
    )
    .await
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL"]
async fn pin_evidence_missing_investigation() {
    let (app, _pool) = setup_app().await;

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method(Method::POST)
                .uri("/api/investigations/nonexistent/evidence")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "object_id": "symbol:foo:bar:1",
                        "view_id": "overview",
                        "note": "Test note"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL"]
async fn pin_evidence_creates_evidence_record() {
    let (app, pool) = setup_app().await;

    // Create an investigation first
    let inv_id = uuid::Uuid::new_v4().to_string();
    let ws_id = "test-workspace";

    sqlx::query(
        r#"
        INSERT INTO investigations (
            id, workspace_id, title, goal, status,
            entry_point, panes, evidence, artifacts, narrative,
            related_adrs, created_at, updated_at
        ) VALUES (
            $1, $2, $3, $4, 'active', NULL, '[]'::jsonb,
            '[]'::jsonb, '[]'::jsonb, '', '{}'::jsonb,
            now(), now()
        )
        "#,
    )
    .bind(&inv_id)
    .bind(ws_id)
    .bind("Test Investigation")
    .bind("Test goal")
    .execute(&pool)
    .await
    .unwrap();

    // Pin evidence
    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method(Method::POST)
                .uri(&format!("/api/investigations/{}/evidence", inv_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "object_id": "symbol:UserService:create:15",
                        "view_id": "call_graph",
                        "note": "This is the main user creation function"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // Verify evidence was created
    let evidence_rows: Vec<(String, String, String)> = sqlx::query_as(
        r#"
        SELECT object_id, view_id, note
        FROM evidence
        WHERE investigation_id = $1
        ORDER BY pinned_at DESC
        LIMIT 1
        "#,
    )
    .bind(&inv_id)
    .fetch_all(&pool)
    .await
    .unwrap();

    assert_eq!(evidence_rows.len(), 1);
    let (object_id, view_id, note) = &evidence_rows[0];
    assert_eq!(object_id, "symbol:UserService:create:15");
    assert_eq!(view_id, Some("call_graph"));
    assert_eq!(note, "This is the main user creation function");
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL"]
async fn pin_evidence_updates_investigation_updated_at() {
    let (app, pool) = setup_app().await;

    // Create an investigation
    let inv_id = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        r#"
        INSERT INTO investigations (
            id, workspace_id, title, goal, status,
            entry_point, panes, evidence, artifacts, narrative,
            related_adrs, created_at, updated_at
        ) VALUES (
            $1, $2, $3, $4, 'active', NULL, '[]'::jsonb,
            '[]'::jsonb, '[]'::jsonb, '', '{}'::jsonb,
            now(), now()
        )
        "#,
    )
    .bind(&inv_id)
    .bind("test-workspace")
    .bind("Test Investigation")
    .bind("Test goal")
    .execute(&pool)
    .await
    .unwrap();

    // Get initial updated_at as string
    let initial_updated: String = sqlx::query_scalar(
        "SELECT updated_at::text FROM investigations WHERE id = $1",
    )
    .bind(&inv_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    // Wait a bit to ensure updated_at would change
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    // Pin evidence
    let _ = app
        .oneshot(
            axum::http::Request::builder()
                .method(Method::POST)
                .uri(&format!("/api/investigations/{}/evidence", inv_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "object_id": "symbol:test:1",
                        "note": "Test"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // Verify updated_at was updated
    let updated_after: String = sqlx::query_scalar(
        "SELECT updated_at::text FROM investigations WHERE id = $1",
    )
    .bind(&inv_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_ne!(initial_updated, updated_after);
    assert!(updated_after > initial_updated);
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL"]
async fn pin_evidence_nullable_view_id() {
    let (app, pool) = setup_app().await;

    // Create an investigation
    let inv_id = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        r#"
        INSERT INTO investigations (
            id, workspace_id, title, goal, status,
            entry_point, panes, evidence, artifacts, narrative,
            related_adrs, created_at, updated_at
        ) VALUES (
            $1, $2, $3, $4, 'active', NULL, '[]'::jsonb,
            '[]'::jsonb, '[]'::jsonb, '', '{}'::jsonb,
            now(), now()
        )
        "#,
    )
    .bind(&inv_id)
    .bind("test-workspace")
    .bind("Test Investigation")
    .bind("Test goal")
    .execute(&pool)
    .await
    .unwrap();

    // Pin evidence with null view_id
    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method(Method::POST)
                .uri(&format!("/api/investigations/{}/evidence", inv_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "object_id": "symbol:test:1",
                        "note": "Test"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // Verify evidence has null view_id
    let view_id: Option<String> = sqlx::query_scalar(
        "SELECT view_id FROM evidence WHERE investigation_id = $1 LIMIT 1",
    )
    .bind(&inv_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(view_id, None);
}