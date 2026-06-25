//! SonarQube API Rule Scraper
//!
//! Downloads rule metadata from SonarCloud's public API and converts to RuleCatalog format.
//! No API key needed for public access (limited rate).

#[cfg(feature = "scraper")]
use crate::rules::importer::{ImportedRule, RuleCatalog, RuleParameter};

#[cfg(feature = "scraper")]
use std::collections::HashMap;

/// Scrapes SonarCloud API for rules of a given language
#[cfg(feature = "scraper")]
pub struct SonarQubeScraper {
    client: reqwest::blocking::Client,
    base_url: String,
}

#[cfg(feature = "scraper")]
impl SonarQubeScraper {
    pub fn new() -> Self {
        Self {
            client: reqwest::blocking::Client::new(),
            base_url: "https://sonarcloud.io/api/rules/search".to_string(),
        }
    }

    /// Fetch rules for a specific language
    pub fn fetch_rules(&self, language: &str, rule_types: &[&str]) -> Result<RuleCatalog, String> {
        let mut all_rules = Vec::new();
        let mut page = 1;

        loop {
            let types_param = rule_types.join(",");
            let url = format!(
                "{}?languages={}&types={}&p={}&ps=500&f=name,severity,type,lang,htmlDesc,params,tags",
                self.base_url, language, types_param, page
            );

            let response = self.client
                .get(&url)
                .header("Accept", "application/json")
                .send()
                .map_err(|e| format!("HTTP error: {}", e))?;

            let json: serde_json::Value = response.json()
                .map_err(|e| format!("JSON error: {}", e))?;

            let rules = json["rules"].as_array()
                .ok_or("No rules array")?;

            if rules.is_empty() { break; }

            for rule in rules {
                let params: Vec<RuleParameter> = rule["params"].as_array()
                    .map(|arr| arr.iter().map(|p| RuleParameter {
                        name: p["key"].as_str().unwrap_or("").to_string(),
                        description: p["htmlDesc"].as_str().unwrap_or("").to_string(),
                        default_value: p["defaultValue"].as_str().map(|s| s.to_string()),
                        param_type: p["type"].as_str().unwrap_or("STRING").to_string(),
                    }).collect())
                    .unwrap_or_default();

                all_rules.push(ImportedRule {
                    rule_id: rule["key"].as_str().unwrap_or("").to_string(),
                    name: rule["name"].as_str().unwrap_or("").to_string(),
                    severity: rule["severity"].as_str().unwrap_or("MAJOR").to_string(),
                    rule_type: rule["type"].as_str().unwrap_or("CODE_SMELL").to_string(),
                    language: rule["lang"].as_str().unwrap_or(language).to_string(),
                    description: strip_html(rule["htmlDesc"].as_str().unwrap_or("")),
                    parameters: params,
                    tags: rule["tags"].as_array()
                        .map(|arr| arr.iter().filter_map(|t| t.as_str().map(|s| s.to_string())).collect())
                        .unwrap_or_default(),
                });
            }

            // Check if there are more pages
            let total = json["total"].as_u64().unwrap_or(0) as usize;
            if page * 500 >= total { break; }
            page += 1;

            // Rate limiting
            std::thread::sleep(std::time::Duration::from_secs(1));
        }

        Ok(RuleCatalog {
            version: "1.0".to_string(),
            source: "sonarcloud-api".to_string(),
            exported_at: chrono::Utc::now().to_rfc3339(),
            rules: all_rules,
        })
    }

    /// Fetch only top-priority rules (BLOCKER + CRITICAL)
    pub fn fetch_priority_rules(&self, language: &str) -> Result<RuleCatalog, String> {
        let catalog = self.fetch_rules(language, &["BUG", "VULNERABILITY", "SECURITY_HOTSPOT"])?;
        let priority: Vec<ImportedRule> = catalog.rules.into_iter()
            .filter(|r| r.severity == "BLOCKER" || r.severity == "CRITICAL")
            .collect();
        Ok(RuleCatalog {
            version: catalog.version,
            source: catalog.source,
            exported_at: catalog.exported_at,
            rules: priority,
        })
    }
}

#[cfg(feature = "scraper")]
fn strip_html(text: &str) -> String {
    let re = regex::Regex::new(r"<[^>]*>").unwrap();
    re.replace_all(text, "").to_string()
}

/// Command-line entry point for rule scraping
#[cfg(feature = "scraper")]
pub fn scrape_command(language: &str, output_path: &str, priority_only: bool) -> Result<(), String> {
    let scraper = SonarQubeScraper::new();

    let catalog = if priority_only {
        scraper.fetch_priority_rules(language)?
    } else {
        scraper.fetch_rules(language, &["BUG", "VULNERABILITY", "CODE_SMELL", "SECURITY_HOTSPOT"])?
    };

    let json = serde_json::to_string_pretty(&catalog)
        .map_err(|e| format!("Serialization error: {}", e))?;

    std::fs::write(output_path, json)
        .map_err(|e| format!("Write error: {}", e))?;

    println!("Scraped {} rules to {}", catalog.rules.len(), output_path);
    Ok(())
}

#[cfg(not(feature = "scraper"))]
/// Stub when scraper feature is not enabled
pub fn scrape_command(_language: &str, _output_path: &str, _priority_only: bool) -> Result<(), String> {
    Err("Scraper feature not enabled. Add --features scraper to enable.".to_string())
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "scraper")]
    use super::*;

    #[cfg(feature = "scraper")]
    #[test]
    fn test_strip_html() {
        assert_eq!(strip_html("<p>hello</p>"), "hello");
        assert_eq!(strip_html("no html"), "no html");
    }

    #[test]
    fn test_strip_html_without_feature() {
        // When feature is off, strip_html is not available but scrape_command exists as stub
        let result = scrape_command("rust", "/tmp/test.json", false);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Scraper feature not enabled. Add --features scraper to enable.");
    }
}
