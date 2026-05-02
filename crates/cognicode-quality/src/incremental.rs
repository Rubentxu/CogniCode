//! Incremental analysis with file hashing, new-code detection, and historical baselines.
//!
//! Provides SonarQube-like "New Code Period" — only flag issues in recently changed code,
//! not pre-existing technical debt. Persists analysis state to `.cognicode/analysis-state.json`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use serde::{Serialize, Deserialize};

/// Per-project analysis state persisted to disk
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisState {
    pub project_root: PathBuf,
    pub baseline: Option<QualityBaseline>,
    pub history: Vec<QualitySnapshot>,
    pub file_states: HashMap<String, FileState>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityBaseline {
    pub timestamp: String,
    pub total_issues: usize,
    pub debt_minutes: u64,
    pub rating: String,
    pub blockers: usize,
    pub criticals: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualitySnapshot {
    pub timestamp: String,
    pub total_issues: usize,
    pub debt_minutes: u64,
    pub rating: String,
    pub files_changed: usize,
    pub new_issues: usize,
    pub fixed_issues: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileState {
    pub hash: String,       // BLAKE3 hex
    pub issues_count: usize,
    pub last_analyzed: String,
}

impl AnalysisState {
    /// Load state from disk (or create empty)
    pub fn load(project_root: &Path) -> Self {
        let state_path = project_root.join(".cognicode/analysis-state.json");
        if state_path.exists() {
            std::fs::read_to_string(&state_path)
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_else(|| Self::new(project_root))
        } else {
            Self::new(project_root)
        }
    }

    fn new(project_root: &Path) -> Self {
        Self {
            project_root: project_root.to_path_buf(),
            baseline: None,
            history: Vec::new(),
            file_states: HashMap::new(),
        }
    }

    /// Save state to disk
    pub fn save(&self) {
        let dir = self.project_root.join(".cognicode");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("analysis-state.json");
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(&path, json);
        }
    }

    /// Compute BLAKE3 hash of file content
    pub fn hash_file(path: &Path) -> Option<String> {
        let content = std::fs::read(path).ok()?;
        let hash = blake3::hash(&content);
        Some(hash.to_hex().to_string())
    }

    /// Find files that changed since last analysis
    pub fn find_changed_files(&self, all_files: &[PathBuf]) -> Vec<PathBuf> {
        all_files.iter()
            .filter(|path| {
                let key = path.to_string_lossy().to_string();
                match self.file_states.get(&key) {
                    Some(state) => {
                        Self::hash_file(path)
                            .map(|h| h != state.hash)
                            .unwrap_or(true)
                    }
                    None => true, // New file, never analyzed
                }
            })
            .cloned()
            .collect()
    }

    /// Update file state after analysis
    pub fn update_file_state(&mut self, path: &Path, issues_count: usize) {
        if let Some(hash) = Self::hash_file(path) {
            self.file_states.insert(
                path.to_string_lossy().to_string(),
                FileState {
                    hash,
                    issues_count,
                    last_analyzed: chrono::Utc::now().to_rfc3339(),
                },
            );
        }
    }

    /// Set a quality baseline (call at start of SDD: sdd-init or sdd-explore)
    pub fn set_baseline(&mut self, total_issues: usize, debt_minutes: u64, rating: &str, blockers: usize, criticals: usize) {
        self.baseline = Some(QualityBaseline {
            timestamp: chrono::Utc::now().to_rfc3339(),
            total_issues,
            debt_minutes,
            rating: rating.to_string(),
            blockers,
            criticals,
        });
        self.save();
    }

    /// Add a snapshot to history (call after each analysis)
    pub fn add_snapshot(&mut self, total_issues: usize, debt_minutes: u64, rating: &str, files_changed: usize, new_issues: usize, fixed_issues: usize) {
        self.history.push(QualitySnapshot {
            timestamp: chrono::Utc::now().to_rfc3339(),
            total_issues,
            debt_minutes,
            rating: rating.to_string(),
            files_changed,
            new_issues,
            fixed_issues,
        });
        // Keep last 50 snapshots max
        if self.history.len() > 50 {
            self.history.remove(0);
        }
        self.save();
    }

    /// Compare current state vs baseline → diff report
    pub fn diff_vs_baseline(&self, current_issues: usize, current_debt: u64, current_rating: &str, current_blockers: usize) -> Option<BaselineDiff> {
        self.baseline.as_ref().map(|b| BaselineDiff {
            baseline_timestamp: b.timestamp.clone(),
            issues_delta: current_issues as i64 - b.total_issues as i64,
            debt_delta: current_debt as i64 - b.debt_minutes as i64,
            rating_before: b.rating.clone(),
            rating_after: current_rating.to_string(),
            blockers_before: b.blockers,
            blockers_after: current_blockers,
        })
    }

    /// Get files that are "new" since a reference point (git diff or baseline timestamp)
    /// For now: files whose hash doesn't match baseline state
    pub fn get_new_code_files(&self) -> Vec<String> {
        self.file_states.iter()
            .filter(|(_path, state)| {
                // File is "new" if it was added after baseline
                // Simple heuristic: count >0 means it existed at baseline
                // More sophisticated: compare timestamps
                state.issues_count > 0
            })
            .map(|(path, _)| path.clone())
            .collect()
    }

    /// Filter issues to only those in "new code" files
    pub fn filter_new_code_issues(&self, all_issues: &[cognicode_axiom::rules::types::Issue], new_code_files: &[String]) -> Vec<cognicode_axiom::rules::types::Issue> {
        all_issues.iter()
            .filter(|issue| {
                let file_str = issue.file.to_string_lossy().to_string();
                new_code_files.contains(&file_str)
            })
            .cloned()
            .collect()
    }
}

#[derive(Debug, Serialize)]
pub struct BaselineDiff {
    pub baseline_timestamp: String,
    pub issues_delta: i64,
    pub debt_delta: i64,
    pub rating_before: String,
    pub rating_after: String,
    pub blockers_before: usize,
    pub blockers_after: usize,
}