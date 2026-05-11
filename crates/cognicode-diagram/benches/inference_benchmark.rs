//! Inference benchmark — measures deployment and ER inference performance

use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};
use std::path::Path;
use tempfile::TempDir;

use cognicode_diagram::inference::deployment_inference::infer_deployment;
use cognicode_diagram::inference::er_inference::infer_er_diagram;
use cognicode_diagram::model::deployment::DeploymentModel;
use cognicode_diagram::model::er_types::ErModel;

fn create_test_docker_project() -> TempDir {
    let temp_dir = TempDir::new().unwrap();

    // Create a Dockerfile
    let dockerfile = r#"FROM rust:1.70 AS builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bullseye-slim
EXPOSE 8080 9090
ENV RUST_LOG=info
COPY --from=builder /app/target/release/myapp /app/
CMD ["/app/myapp"]
"#;
    std::fs::write(temp_dir.path().join("Dockerfile"), dockerfile).unwrap();

    // Create docker-compose.yml
    let compose = r#"version: '3.8'
services:
  web:
    build: .
    ports:
      - "8080:8080"
      - "9090:9090"
    environment:
      - RUST_LOG=info
    depends_on:
      - db
      - redis
  db:
    image: postgres:15
    volumes:
      - db_data:/var/lib/postgresql/data
    environment:
      - POSTGRES_PASSWORD=secret
  redis:
    image: redis:7-alpine
    ports:
      - "6379:6379"
volumes:
  db_data:
networks:
  default:
    name: app_network
"#;
    std::fs::write(temp_dir.path().join("docker-compose.yml"), compose).unwrap();

    temp_dir
}

fn create_test_sql_project() -> TempDir {
    let temp_dir = TempDir::new().unwrap();

    // Create a SQL migrations directory
    let migrations_dir = temp_dir.path().join("migrations");
    std::fs::create_dir(&migrations_dir).unwrap();

    let sql_content = r#"
CREATE TABLE users (
    id SERIAL PRIMARY KEY,
    email VARCHAR(255) NOT NULL UNIQUE,
    password_hash VARCHAR(255) NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE posts (
    id SERIAL PRIMARY KEY,
    title VARCHAR(255) NOT NULL,
    content TEXT,
    author_id INTEGER REFERENCES users(id),
    published BOOLEAN DEFAULT FALSE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE comments (
    id SERIAL PRIMARY KEY,
    body TEXT NOT NULL,
    post_id INTEGER REFERENCES posts(id),
    author_id INTEGER REFERENCES users(id),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE tags (
    id SERIAL PRIMARY KEY,
    name VARCHAR(50) NOT NULL UNIQUE
);

CREATE TABLE post_tags (
    post_id INTEGER REFERENCES posts(id),
    tag_id INTEGER REFERENCES tags(id),
    PRIMARY KEY (post_id, tag_id)
);
"#;
    std::fs::write(migrations_dir.join("001_initial.sql"), sql_content).unwrap();

    temp_dir
}

fn bench_deployment_inference(c: &mut Criterion) {
    let temp_dir = create_test_docker_project();

    c.bench_function("deployment_inference", |b| {
        b.iter(|| {
            let _ = infer_deployment(temp_dir.path());
        });
    });

    // Also benchmark cached scenario (second call with same data)
    let _ = infer_deployment(temp_dir.path()); // warmup/prime
    c.bench_function("deployment_inference_cached", |b| {
        b.iter(|| {
            let _ = infer_deployment(temp_dir.path());
        });
    });
}

fn bench_er_inference(c: &mut Criterion) {
    let temp_dir = create_test_sql_project();

    c.bench_function("er_inference", |b| {
        b.iter(|| {
            let _ = infer_er_diagram(temp_dir.path());
        });
    });

    // Cached scenario
    let _ = infer_er_diagram(temp_dir.path()); // warmup
    c.bench_function("er_inference_cached", |b| {
        b.iter(|| {
            let _ = infer_er_diagram(temp_dir.path());
        });
    });
}

fn bench_inference_levels(c: &mut Criterion) {
    // L1: Context inference (deployment - external systems)
    // L2: Container inference (docker-compose services)
    // L3: Component inference (container details)

    let mut group = c.benchmark_group("inference_levels");

    let temp_dir = create_test_docker_project();
    let project_path = temp_dir.path();

    // L1 - basic deployment detection
    group.bench_function("L1_deployment_detection", |b| {
        b.iter(|| {
            let _ = infer_deployment(project_path);
        });
    });

    // L2 - ER diagram inference
    let sql_dir = create_test_sql_project();
    group.bench_function("L2_er_inference", |b| {
        b.iter(|| {
            let _ = infer_er_diagram(sql_dir.path());
        });
    });

    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(10);
    targets = bench_deployment_inference, bench_er_inference, bench_inference_levels
}
criterion_main!(benches);
