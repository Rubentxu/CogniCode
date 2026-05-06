/// Clean SSL configuration with proper verification
pub fn create_safe_ssl_config() -> String {
    let config = r#"
    SSL_CONFIG {
        verify: true,
        cert_path: "/path/to/cert.pem",
    }
    "#;
    config.to_string()
}

/// Clean configuration
pub fn normal_config() -> String {
    let config = "verify: true";
    config.to_string()
}

/// Another clean example
pub fn safe_connection() -> String {
    let config = "SSL with Certificate verification enabled";
    config.to_string()
}
