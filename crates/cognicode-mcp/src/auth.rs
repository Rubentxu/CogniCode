//! M3.5: Bearer-token authentication primitives.
//!
//! Extracted from `cognicode-mcp/src/server.rs` as a library module so
//! the constant-time comparison logic is testable in isolation. The
//! `auth_middleware` itself lives next to the router in `server.rs` so
//! it can use the `Arc<HandlerContext>` state shared with the rest of
//! the HTTP pipeline; only the pure comparison function is exposed
//! here.

use axum::http::StatusCode;

/// Validate the `Authorization: Bearer <token>` header against an
/// expected token using constant-time comparison.
///
/// Returns `Ok(())` on match, `Err(UNAUTHORIZED)` on any mismatch or
/// missing/malformed header. The body of the request is NOT inspected.
///
/// The check is split out from the `auth_middleware` closure in
/// `server.rs` so the security-critical branch logic can be unit-tested
/// without a running router. Specifically we cover:
///
/// - exact match → accept
/// - wrong token → reject
/// - missing header → reject
/// - malformed (no scheme, truncated, lowercase scheme) → mixed
/// - empty bearer value with non-empty expected → reject
pub fn check_bearer_token(
    header_value: Option<&str>,
    expected: &str,
) -> Result<(), StatusCode> {
    // Accept both "Bearer " and "bearer " — RFC 7235 says the auth
    // scheme is case-insensitive. `eq_ignore_ascii_case` is OK here
    // because we're comparing a 7-byte literal, not the secret itself.
    let token = match header_value {
        Some(s) if s.len() > 7 && s.as_bytes()[..7].eq_ignore_ascii_case(b"Bearer ") => &s[7..],
        _ => return Err(StatusCode::UNAUTHORIZED),
    };
    use subtle::ConstantTimeEq;
    // `ct_eq` returns a `Choice` whose value is data-independent. We
    // branch on the choice itself (constant-time) and NOT on the
    // token contents, to preserve constant-time behaviour.
    let eq = token.as_bytes().ct_eq(expected.as_bytes());
    if bool::from(eq) {
        Ok(())
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_exact_match() {
        assert!(check_bearer_token(Some("Bearer secret123"), "secret123").is_ok());
    }

    #[test]
    fn rejects_wrong_token() {
        assert_eq!(
            check_bearer_token(Some("Bearer wrong-token"), "secret123"),
            Err(StatusCode::UNAUTHORIZED)
        );
    }

    #[test]
    fn rejects_missing_header() {
        assert_eq!(
            check_bearer_token(None, "secret123"),
            Err(StatusCode::UNAUTHORIZED)
        );
    }

    #[test]
    fn rejects_malformed_header_no_scheme() {
        // No "Bearer " prefix
        assert_eq!(
            check_bearer_token(Some("secret123"), "secret123"),
            Err(StatusCode::UNAUTHORIZED)
        );
    }

    #[test]
    fn accepts_lowercase_bearer_scheme() {
        // RFC 7235 says the auth scheme is case-insensitive.
        assert!(check_bearer_token(Some("bearer secret123"), "secret123").is_ok());
        assert!(check_bearer_token(Some("BEARER secret123"), "secret123").is_ok());
    }

    #[test]
    fn rejects_truncated_header() {
        // Just "Bearer " with no token — length > 7 check fails.
        assert_eq!(
            check_bearer_token(Some("Bearer "), "secret123"),
            Err(StatusCode::UNAUTHORIZED)
        );
        // Exactly "Bearer" with no trailing space.
        assert_eq!(
            check_bearer_token(Some("Bearer"), "secret123"),
            Err(StatusCode::UNAUTHORIZED)
        );
    }

    #[test]
    fn rejects_empty_token_against_nonempty_expected() {
        // Header present but token is empty.
        // "Bearer " has length 7, so the `s.len() > 7` guard rejects.
        assert_eq!(
            check_bearer_token(Some("Bearer "), "non-empty"),
            Err(StatusCode::UNAUTHORIZED)
        );
    }

    #[test]
    fn rejects_empty_bearer_even_when_expected_is_empty() {
        // The header must be longer than "Bearer " (7 bytes) for the
        // token extraction to fire. "Bearer " with no token is length
        // 7 and so is rejected regardless of the expected value.
        // The middleware already short-circuits on empty
        // `COGNICODE_MCP_AUTH_TOKEN`, so the empty-expected branch is
        // unreachable in practice; we just lock the rejection here.
        assert_eq!(
            check_bearer_token(Some("Bearer "), ""),
            Err(StatusCode::UNAUTHORIZED)
        );
    }
}
