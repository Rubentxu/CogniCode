//! Who is on the other end of this connection.
//!
//! `HandlerContext` carried this as three public fields — `client_name`,
//! `client_version`, `client_protocol_version` — each an `Option<String>`,
//! each independently settable. That is not three optional values; it is one
//! optional value with three attributes. The difference is not cosmetic:
//! three fields admit states that have no meaning ("named `cognicode-cli`,
//! no version"), and every one of those states is a branch some reader has to
//! consider. Here there is one way to be unknown and no way to be half-known.
//!
//! It lives in `domain` rather than beside the MCP handler because "which
//! client is this, if any" is a fact about the caller, not a detail of the
//! transport that happens to report it. The same reasoning moved
//! `TraversalDirection` out of the graph store in ST-02: the question is
//! vocabulary, the transport that asked it is an adapter.

/// The identity of a connected client, if it announced one.
///
/// Equality is over all three attributes, so "same client" means the same
/// thing everywhere and cannot be answered two ways.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClientIdentity {
    name: Option<String>,
    version: Option<String>,
    protocol_version: Option<String>,
}

impl ClientIdentity {
    /// No client announced itself.
    ///
    /// This is the only spelling of "unknown": there is no way to construct a
    /// `ClientIdentity` that is unknown *and* carries a name.
    pub fn unknown() -> Self {
        Self::default()
    }

    /// Builder method for setting the client name.
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Builder method for setting the client version.
    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.version = Some(version.into());
        self
    }

    /// Builder method for setting the negotiated protocol version.
    pub fn with_protocol_version(mut self, version: impl Into<String>) -> Self {
        self.protocol_version = Some(version.into());
        self
    }

    /// The client's self-reported name, if it gave one.
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// The client's self-reported version, if it gave one.
    pub fn version(&self) -> Option<&str> {
        self.version.as_deref()
    }

    /// The negotiated MCP protocol version, if one was established.
    pub fn protocol_version(&self) -> Option<&str> {
        self.protocol_version.as_deref()
    }

    /// Whether this context has a connected client at all.
    ///
    /// A client that announced nothing is unknown, not empty-but-present;
    /// the distinction matters to handlers that behave differently for a
    /// known caller than for none.
    pub fn is_known(&self) -> bool {
        self.name.is_some() || self.version.is_some() || self.protocol_version.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::ClientIdentity;

    #[test]
    fn unknown_is_unknown_and_is_the_default() {
        // Both spellings of "no client" are the same value, so a context that
        // never set anything compares equal to one built explicitly.
        assert_eq!(ClientIdentity::unknown(), ClientIdentity::default());
        assert!(!ClientIdentity::unknown().is_known());
    }

    #[test]
    fn a_client_that_announced_anything_is_known() {
        // Each attribute alone is enough. A peer that sent only a protocol
        // version negotiated a real conversation; calling it unknown would
        // make handlers behave as if nobody were connected.
        assert!(
            ClientIdentity::unknown()
                .with_name("cognicode-cli")
                .is_known()
        );
        assert!(ClientIdentity::unknown().with_version("0.98.1").is_known());
        assert!(
            ClientIdentity::unknown()
                .with_protocol_version("2024-11")
                .is_known()
        );
    }

    #[test]
    fn attributes_round_trip_and_compare_by_value() {
        let a = ClientIdentity::unknown()
            .with_name("cognicode-cli")
            .with_version("0.98.1")
            .with_protocol_version("2024-11");
        let b = a.clone();
        assert_eq!(a, b);

        // Equality is over every attribute, so two clients that differ in one
        // field are two clients. Each variant is built from `a` rather than
        // from `b`, because the `with_*` builders consume the receiver.
        assert_ne!(a, b.with_version("0.98.2"));
        assert_ne!(a, a.clone().with_name("other-client"));
        assert_ne!(a, a.clone().with_protocol_version("2025-03"));
    }

    #[test]
    fn accessors_report_absent_attributes_as_none() {
        let id = ClientIdentity::unknown().with_name("cognicode-cli");
        assert_eq!(id.name(), Some("cognicode-cli"));
        assert_eq!(id.version(), None);
        assert_eq!(id.protocol_version(), None);
    }
}
