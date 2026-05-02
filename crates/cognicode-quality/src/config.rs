//! Configuration types for cognicode-quality
//!
//! Loads from ~/.cognicode/config/cognicode-quality.yaml

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityConfig {
    pub analysis: AnalysisConfig,
    pub linters: LintersConfig,
    pub gates: GatesConfig,
    pub cache: CacheConfig,
    pub performance: PerformanceConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisConfig {
    #[serde(default = "default_max_complexity")]
    pub max_complexity: u32,
    #[serde(default = "default_max_depth")]
    pub max_depth: u32,
    #[serde(default = "default_max_duplications")]
    pub max_duplications_percent: f64,
    #[serde(default = "default_max_debt")]
    pub max_debt_minutes: u64,
    #[serde(default = "default_max_issues")]
    pub max_issues_per_file: usize,
    #[serde(default = "default_early_term")]
    pub early_termination_threshold: usize,
}

fn default_max_complexity() -> u32 { 10 }
fn default_max_depth() -> u32 { 4 }
fn default_max_duplications() -> f64 { 5.0 }
fn default_max_debt() -> u64 { 60 }
fn default_max_issues() -> usize { 100 }
fn default_early_term() -> usize { 10 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LintersConfig {
    pub clippy: LinterConfig,
    #[serde(default)]
    pub eslint: Option<LinterConfig>,
    #[serde(default)]
    pub semgrep: Option<LinterConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinterConfig {
    pub enabled: bool,
    #[serde(default)]
    pub args: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatesConfig {
    #[serde(default = "default_gate_path")]
    pub default: String,
    #[serde(default)]
    pub deploy: Option<String>,
}

fn default_gate_path() -> String { "./quality-gates/default.yaml".to_string() }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    #[serde(default = "default_cache_dir")]
    pub directory: String,
    #[serde(default = "default_cache_size")]
    pub max_size_mb: u64,
}

fn default_cache_dir() -> String { "~/.cognicode/cache".to_string() }
fn default_cache_size() -> u64 { 512 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    #[serde(default = "default_concurrent")]
    pub max_concurrent_analyses: usize,
    #[serde(default = "default_parse_cache")]
    pub parse_cache_size: usize,
}

fn default_concurrent() -> usize { 4 }
fn default_parse_cache() -> usize { 10000 }

impl Default for QualityConfig {
    fn default() -> Self {
        Self {
            analysis: AnalysisConfig {
                max_complexity: 10,
                max_depth: 4,
                max_duplications_percent: 5.0,
                max_debt_minutes: 60,
                max_issues_per_file: 100,
                early_termination_threshold: 10,
            },
            linters: LintersConfig {
                clippy: LinterConfig { enabled: true, args: vec!["--".to_string(), "-W".to_string(), "clippy::all".to_string()] },
                eslint: None,
                semgrep: None,
            },
            gates: GatesConfig { default: "./quality-gates/default.yaml".to_string(), deploy: None },
            cache: CacheConfig { directory: "~/.cognicode/cache".to_string(), max_size_mb: 512 },
            performance: PerformanceConfig { max_concurrent_analyses: 4, parse_cache_size: 10000 },
        }
    }
}

impl QualityConfig {
    /// Load config from default location
    pub fn load() -> Self {
        let config_path = dirs_next::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".cognicode/config/cognicode-quality.yaml");

        if config_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&config_path) {
                if let Ok(config) = serde_yaml::from_str(&content) {
                    return config;
                }
            }
        }
        Self::default()
    }

    /// Write default config to disk
    pub fn write_default() -> std::io::Result<()> {
        if let Some(home) = dirs_next::home_dir() {
            let config_dir = home.join(".cognicode/config");
            std::fs::create_dir_all(&config_dir)?;
            let config_path = config_dir.join("cognicode-quality.yaml");
            let yaml = serde_yaml::to_string(&Self::default())
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
            std::fs::write(&config_path, yaml)?;
        }
        Ok(())
    }
}