//! PostgreSQL-backed IacRepository stub — pending full PG wiring.
#![cfg(feature = "postgres")]

use async_trait::async_trait;
use crate::domain::traits::iac_repository::{IacEdge, IacRepository, IacResource};
use sqlx::PgPool;

pub struct PostgresIacRepository {
    _pool: PgPool,
}

impl PostgresIacRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { _pool: pool }
    }
}

#[async_trait]
impl IacRepository for PostgresIacRepository {
    async fn find_resource(&self, _resource_id: &str) -> Result<Option<IacResource>, String> {
        Ok(None)
    }

    async fn get_dependencies(&self, _resource_id: &str, _depth: Option<u32>) -> Result<Vec<IacEdge>, String> {
        Ok(vec![])
    }

    async fn get_dependents(&self, _resource_id: &str, _depth: Option<u32>) -> Result<Vec<IacEdge>, String> {
        Ok(vec![])
    }
}
