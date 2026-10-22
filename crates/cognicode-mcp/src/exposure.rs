//! Exposure rules for the HTTP transport of the MCP server.
//!
//! The server speaks JSON-RPC over stdio by default. Started as
//! `cognicode-mcp-server`, it speaks the same protocol over HTTP, and that
//! is the transport other machines can reach. The rules about who may
//! reach it live here, in one place, so they can be read and tested
//! without starting a server.
//!
//! Two rules, and they are the same rule seen from two sides:
//!
//! - **A server nobody else can reach needs no credential.** A loopback
//!   address only answers from the machine itself.
//! - **A server other machines can reach needs one.** Serving a mutable
//!   tool over HTTP without a credential means that anybody who can open
//!   a socket can act with the authority of the user who started the
//!   process.
//!
//! So the address the server listens on is loopback by default, and
//! asking for a wider one without providing a token is refused at
//! startup instead of being served in the clear.

use std::net::{IpAddr, SocketAddr};

/// The address the HTTP transport listens on when nothing else is asked for.
///
/// Loopback, because the default has to be the safe one: a server that
/// waits to be told to be reachable does not surprise anybody, while one
/// that waits to be told to be unreachable already exposed itself.
pub const DEFAULT_LISTEN: &str = "127.0.0.1:9847";

/// The environment variable that carries the bearer token.
pub const AUTH_TOKEN_VARIABLE: &str = "COGNICODE_MCP_AUTH_TOKEN";

/// Whether an address can only be reached from this machine.
pub fn is_loopback(listen: &SocketAddr) -> bool {
    match listen.ip() {
        IpAddr::V4(v4) => v4.is_loopback(),
        IpAddr::V6(v6) => v6.is_loopback(),
    }
}

/// The bearer token this process was started with, if any.
///
/// An empty or absent token means the same thing: no credential. The
/// value is read once, at startup, because the rule that depends on it is
/// a rule about the configuration of a running server, and a server whose
/// credential appears halfway through its life is a server that was
/// exposed before it was protected.
///
/// Rotating the token is a restart: change the variable and start the
/// server again.
pub fn configured_token() -> Option<String> {
    match std::env::var(AUTH_TOKEN_VARIABLE) {
        Ok(token) if !token.trim().is_empty() => Some(token),
        _ => None,
    }
}

/// Whether the server may start with this address and this credential.
///
/// Fails closed: an address other machines can reach without a credential
/// is refused, and the message says what to do about it.
pub fn ensure_safe_exposure(listen: &SocketAddr, token: Option<&str>) -> Result<(), String> {
    if token.is_some_and(|value| !value.trim().is_empty()) {
        return Ok(());
    }
    if is_loopback(listen) {
        return Ok(());
    }
    Err(format!(
        "refusing to serve {listen} without a credential: an address other \
         machines can reach exposes the tools, including the ones that write. \
         Set {AUTH_TOKEN_VARIABLE} to a secret value, or listen on a loopback \
         address such as {DEFAULT_LISTEN}."
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn addr(text: &str) -> SocketAddr {
        text.parse().expect("a valid socket address")
    }

    #[test]
    fn the_default_address_is_reachable_only_from_this_machine() {
        assert!(
            is_loopback(&addr(DEFAULT_LISTEN)),
            "the default address has to be loopback, or the server is exposed \
             before anybody asks for it"
        );
    }

    #[test]
    fn a_loopback_address_needs_no_credential() {
        assert!(ensure_safe_exposure(&addr("127.0.0.1:9847"), None).is_ok());
        assert!(ensure_safe_exposure(&addr("[::1]:9847"), None).is_ok());
    }

    #[test]
    fn a_wider_address_without_a_credential_is_refused() {
        let refused = ensure_safe_exposure(&addr("0.0.0.0:9847"), None);
        assert!(
            refused.is_err(),
            "a server other machines can reach, with no credential, is the \
             configuration that must not start"
        );
        let message = refused.expect_err("the refusal explains itself");
        assert!(
            message.contains(AUTH_TOKEN_VARIABLE),
            "the refusal has to say which variable to set: {message}"
        );
    }

    #[test]
    fn a_wider_address_with_a_credential_is_allowed() {
        assert!(ensure_safe_exposure(&addr("0.0.0.0:9847"), Some("secret")).is_ok());
        assert!(ensure_safe_exposure(&addr("192.168.1.10:9847"), Some("secret")).is_ok());
    }

    #[test]
    fn an_empty_credential_is_no_credential() {
        assert!(ensure_safe_exposure(&addr("0.0.0.0:9847"), Some("")).is_err());
        assert!(ensure_safe_exposure(&addr("0.0.0.0:9847"), Some("   ")).is_err());
    }
}
