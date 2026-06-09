//! CLI dispatch helper for the `cognicode-explorer-api` and
//! `cognicode-explorer-mcp` binaries.
//!
//! Resolves the storage backend from three signals with a fixed
//! precedence:
//!
//!   1. `--postgres <URL>` (explicit flag — highest precedence)
//!   2. `DATABASE_URL` env var (non-empty value)
//!   3. `--sqlite` flag (only honored when the `sqlite` feature
//!      is enabled; opt-in local mode)
//!
//! If none of the three are present, the helper returns an error so
//! the binary can fail fast with a clean message. This matches the
//! `explorer-postgres-bridge` MODIFIED Requirement 1 acceptance
//! criteria in the `postgres-default-config` spec.

/// Which storage backend the binary should connect to at startup.
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Backend {
    /// Connect to PostgreSQL using the given URL.
    Postgres(String),
    /// Open the local `.cognicode/cognicode.db` SQLite file.
    Sqlite,
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
    /// Whether `--sqlite` was passed. Only honored when the `sqlite`
    /// feature is enabled.
    pub sqlite_flag: bool,
}

impl ResolveInput {
    /// Build a `ResolveInput` with explicit values. `postgres_flag` is
    /// the raw `--postgres` value (None if not passed); `sqlite_flag`
    /// is the boolean `--sqlite` flag.
    pub fn new(postgres_flag: Option<String>, sqlite_flag: bool) -> Self {
        Self {
            postgres_flag,
            database_url: None,
            sqlite_flag,
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

    // 3. --sqlite opts out to local mode (feature-gated).
    #[cfg(feature = "sqlite")]
    if input.sqlite_flag {
        return Ok(Backend::Sqlite);
    }

    Err(
        "DATABASE_URL not set and no --sqlite flag provided — cannot start explorer. \
         Set DATABASE_URL=postgres://... or pass --sqlite (with --features sqlite)"
            .to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_postgres_flag_wins() {
        let i = ResolveInput::new(Some("postgres://x".into()), false).with_env("postgres://e");
        assert_eq!(resolve_backend(&i).unwrap(), Backend::Postgres("postgres://x".into()));
    }

    #[test]
    fn unit_env_wins_over_sqlite() {
        let i = ResolveInput::new(None, true).with_env("postgres://e");
        assert_eq!(resolve_backend(&i).unwrap(), Backend::Postgres("postgres://e".into()));
    }

    #[test]
    fn unit_empty_env_treated_as_unset() {
        let i = ResolveInput::new(None, true).with_env("");
        // No env, --sqlite passed → Sqlite wins (when feature on).
        #[cfg(feature = "sqlite")]
        assert_eq!(resolve_backend(&i).unwrap(), Backend::Sqlite);
        // Without sqlite feature, this errors.
        #[cfg(not(feature = "sqlite"))]
        assert!(resolve_backend(&i).is_err());
    }

    #[test]
    fn unit_no_inputs_errors() {
        let i = ResolveInput::new(None, false);
        assert!(resolve_backend(&i).is_err());
    }
}
