//! `cogh::version` — version reporting.

use anyhow::Result;

use crate::layout::CognicodeHome;

const COGH_VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn cmd_version(home: &CognicodeHome) -> Result<()> {
    let cog_ver = std::fs::read_to_string(home.tracker_version())
        .ok()
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "(no version pinned)".to_string());
    println!("cogh {} (managing CogniCode {})", COGH_VERSION, cog_ver);
    Ok(())
}

#[cfg(test)]
mod tests {
    // Version is reported via env!(CARGO_PKG_VERSION); no test needed.
}
