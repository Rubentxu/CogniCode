/// Vulnerable TLS configuration - triggers S4834
pub fn create_ssl_config() -> String {
    // verify = false - disabled certificate verification
    let config = r#"
    SSL_CONFIG {
        verify: false,
    }
    "#;
    config.to_string()
}

/// Vulnerable with NoVerify
pub fn no_verify_ssl() -> String {
    let config = "SSL: NoVerify";
    config.to_string()
}

/// Vulnerable with ALLOW_SELF_SIGNED
pub fn allow_self_signed() -> String {
    let config = "ALLOW_SELF_SIGNED = true";
    config.to_string()
}

/// Vulnerable with danger_accept_invalid_certs
pub fn danger_accept() -> String {
    let config = "danger_accept_invalid_certs(true)";
    config.to_string()
}
