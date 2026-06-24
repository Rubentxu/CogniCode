//! CLI dispatch helper for the `cognicode-explorer-api` and
//! `cognicode-explorer-mcp` binaries.
//!
//! Resolves the storage backend from two signals with a fixed
//! precedence:
//!
//!   1. `--postgres <URL>` (explicit flag — highest precedence)
//!   2. `DATABASE_URL` env var (non-empty value)
//!
//! If neither is present, the helper returns an error so the binary
//! can fail fast with a clean message. This matches the
//! `explorer-postgres-bridge` MODIFIED Requirement 1 acceptance
//! criteria in the `postgres-default-config` spec.

/// Which storage backend the binary should connect to at startup.
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Backend {
    /// Connect to PostgreSQL using the given URL.
    Postgres(String),
}

/// Input bundle for [`resolve_backend`] — split out so tests can drive
/// the helper without touching process-level clap args or env vars.
#[derive(Debug, Clone, Default)]
pub struct ResolveInput {
    /// The value of `--postgres <URL>`, if passed.
    pub postgres_flag: Option<String>,
    /// The value of `DATABASE_URL`, if any. An empty string is
    /// treated as "unset" by [`resolve_backend`].
    pub database_url: Option<String>,
}

impl ResolveInput {
    /// Build a `ResolveInput` with explicit values. `postgres_flag` is
    /// the raw `--postgres` value (None if not passed).
    pub fn new(postgres_flag: Option<String>) -> Self {
        Self {
            postgres_flag,
            database_url: None,
        }
    }

    /// Override the `DATABASE_URL` value (use empty string to simulate
    /// "set but empty", `None` to leave unset).
    pub fn with_env(mut self, value: impl Into<String>) -> Self {
        self.database_url = Some(value.into());
        self
    }
}

/// Resolve the storage backend per the precedence table above.
///
/// Errors are returned as plain `String` so the binary can print them
/// without dragging in a richer error type — these are fatal startup
/// errors anyway.
pub fn resolve_backend(input: &ResolveInput) -> Result<Backend, String> {
    // 1. --postgres <URL> wins.
    if let Some(url) = &input.postgres_flag {
        if !url.is_empty() {
            return Ok(Backend::Postgres(url.clone()));
        }
    }

    // 2. DATABASE_URL (non-empty) wins.
    if let Some(url) = &input.database_url {
        if !url.is_empty() {
            return Ok(Backend::Postgres(url.clone()));
        }
    }

    Err("DATABASE_URL not set — cannot start explorer. \
         Set DATABASE_URL=postgres://... or pass --postgres <URL>"
        .to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_postgres_flag_wins() {
        let i = ResolveInput::new(Some("postgres://x".into())).with_env("postgres://e");
        assert_eq!(
            resolve_backend(&i).unwrap(),
            Backend::Postgres("postgres://x".into())
        );
    }

    #[test]
    fn unit_env_wins_over_postgres() {
        let i = ResolveInput::new(None).with_env("postgres://e");
        assert_eq!(
            resolve_backend(&i).unwrap(),
            Backend::Postgres("postgres://e".into())
        );
    }

    #[test]
    fn unit_no_inputs_errors() {
        let i = ResolveInput::new(None);
        assert!(resolve_backend(&i).is_err());
    }
}
