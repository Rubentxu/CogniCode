//! Event-log identifiers (M7, cycle e63).

use serde::{Deserialize, Serialize};

/// Why a correlation id was rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CorrelationError {
    /// The value was empty or whitespace-only.
    Empty,
}

impl std::fmt::Display for CorrelationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("a correlation id must not be empty")
    }
}

impl std::error::Error for CorrelationError {}

/// Groups the events of one logical operation across actors.
///
/// A correlation id is opaque on purpose: it is a *handle* for "these events
/// belong together", not a semantic label. Two runs of the same detector are
/// different correlations even though they share every other field.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct CorrelationId(String);

impl CorrelationId {
    /// Validate and construct a correlation id.
    pub fn new(value: impl Into<String>) -> Result<Self, CorrelationError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(CorrelationError::Empty);
        }
        Ok(Self(value))
    }

    /// Borrow the raw value.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for CorrelationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for CorrelationId {
    type Error = CorrelationError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<CorrelationId> for String {
    fn from(value: CorrelationId) -> Self {
        value.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty_and_whitespace() {
        assert_eq!(CorrelationId::new(""), Err(CorrelationError::Empty));
        assert_eq!(CorrelationId::new("   "), Err(CorrelationError::Empty));
    }

    #[test]
    fn round_trips_serde_and_rejects_empty_on_the_wire() {
        let id = CorrelationId::new("corr-1").unwrap();
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, "\"corr-1\"");
        assert_eq!(serde_json::from_str::<CorrelationId>(&json).unwrap(), id);
        assert!(serde_json::from_str::<CorrelationId>("\"\"").is_err());
    }
}
