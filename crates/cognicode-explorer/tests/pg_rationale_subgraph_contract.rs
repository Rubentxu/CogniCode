#![cfg(all(test, feature = "multimodal", feature = "postgres"))]

use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};

use cognicode_core::domain::aggregates::generic_graph::NodeId;
use cognicode_core::domain::ports::GraphRepository;
use cognicode_explorer::adapters::PgGraphRepository;
use sqlx::PgPool;

static UNIQ: AtomicU64 = AtomicU64::new(0);

async fn fresh_test_url() -> Option<(String, PgPool)> {
    let base = std::env::var("TEST_DATABASE_URL").ok()?;
    let n = UNIQ.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    let db_name = format!("cognicode_rationale_test_{pid}_{n}");

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

    let pool = sqlx::PgPool::connect(&test_url).await.ok()?;
    let repo =
        cognicode_core::infrastructure::persistence::PostgresRepository::from_pool(pool.clone());
    repo.run_migrations().await.ok()?;

    Some((test_url, pool))
}

fn rewrite_db_name(url: &str, new_name: &str) -> String {
    if let Some(at_idx) = url.rfind('@') {
        let (head, tail) = url.split_at(at_idx);
        if let Some(slash_idx) = tail.find('/') {
            let (host, _) = tail.split_at(slash_idx);
            return format!("{head}{host}/{new_name}");
        }
    }
    let trimmed = url.trim_end_matches('/');
    format!("{trimmed}/{new_name}")
}

macro_rules! pg_test {
    ($name:ident, |$url:ident: String, $pool:ident: PgPool| $body:block) => {
        #[tokio::test]
        async fn $name() {
            let Some(($url, $pool)) = fresh_test_url().await else {
                eprintln!("skipping {}: TEST_DATABASE_URL not set", stringify!($name));
                return;
            };
            async fn inner($url: String, $pool: PgPool) $body
            inner($url, $pool).await
        }
    };
}

async fn seed_rationale_fixture(pool: &PgPool) {
    sqlx::query(
        "INSERT INTO graph_nodes (id, kind, label, source_path, properties, workspace_id)
         VALUES
           ('ADR-001', 'decision', 'ADR-001', 'docs/adr/ADR-001.md', '{}'::jsonb, 'default'),
           ('doc-1', 'doc', 'Document', 'docs/adr/ADR-001.md', '{}'::jsonb, 'default'),
           ('issue-1', 'issue', 'Issue 1', NULL, '{}'::jsonb, 'default'),
           ('evidence-1', 'evidence', 'Evidence 1', NULL, '{}'::jsonb, 'default'),
           ('orphan', 'doc', 'Orphan', NULL, '{}'::jsonb, 'default')",
    )
    .execute(pool)
    .await
    .expect("insert graph_nodes");

    sqlx::query(
        "INSERT INTO graph_edges (source_id, target_id, kind, provenance, confidence, workspace_id, metadata)
         VALUES
           ('ADR-001', 'doc-1', 'justifies', 'Extracted', 1.0, 'default', '{}'::jsonb),
           ('doc-1', 'issue-1', 'cites', 'Inferred', 0.8, 'default', '{}'::jsonb),
           ('issue-1', 'evidence-1', 'resolves', 'Manual', 0.9, 'default', '{}'::jsonb),
           ('orphan', 'evidence-1', 'dependency.calls', 'Extracted', 1.0, 'default', '{}'::jsonb)",
    )
    .execute(pool)
    .await
    .expect("insert graph_edges");
}

pg_test!(
    rationale_subgraph_pg_happy_path,
    |_url: String, pool: PgPool| {
        seed_rationale_fixture(&pool).await;
        let repo = PgGraphRepository::new(pool.clone());

        let (nodes, edges, truncated) = repo
            .rationale_subgraph(&NodeId::new("ADR-001"), 3, 100)
            .await
            .expect("rationale_subgraph should succeed");

        assert!(!truncated, "full fixture should not truncate");

        let node_ids: HashSet<String> = nodes.iter().map(|n| n.id.as_str().to_string()).collect();
        assert!(node_ids.contains("ADR-001"));
        assert!(node_ids.contains("doc-1"));
        assert!(node_ids.contains("issue-1"));
        assert!(node_ids.contains("evidence-1"));
        assert!(
            !node_ids.contains("orphan"),
            "non-rationale edge must be excluded"
        );

        assert_eq!(edges.len(), 3, "must keep only rationale edges");
        assert!(edges.iter().any(|e| e.kind.to_string() == "justifies"));
        assert!(edges.iter().any(|e| e.kind.to_string() == "cites"));
        assert!(edges.iter().any(|e| e.kind.to_string() == "resolves"));
    }
);

pg_test!(
    rationale_subgraph_pg_truncates_and_drops_missing_endpoints,
    |_url: String, pool: PgPool| {
        seed_rationale_fixture(&pool).await;
        let repo = PgGraphRepository::new(pool.clone());

        let (nodes, edges, truncated) = repo
            .rationale_subgraph(&NodeId::new("ADR-001"), 3, 2)
            .await
            .expect("rationale_subgraph should succeed");

        assert!(truncated, "node cap should truncate traversal");
        assert_eq!(nodes.len(), 2, "focus + first retained target only");

        let kept: HashSet<String> = nodes.iter().map(|n| n.id.as_str().to_string()).collect();
        assert!(kept.contains("ADR-001"));
        assert!(kept.contains("doc-1"));
        assert!(
            edges
                .iter()
                .all(|e| { kept.contains(e.source.as_str()) && kept.contains(e.target.as_str()) })
        );
    }
);
