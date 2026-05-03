//! File tracking (BLAKE3 hashes, change detection)

use regex::Regex;
use rusqlite::{Connection, params};
use std::path::Path;
use crate::types::FileState;

pub struct FileStore {
    db: Connection,
}

impl FileStore {
    pub fn new(db: Connection) -> Self {
        Self { db }
    }

    pub fn hash_file(path: &std::path::Path) -> Option<String> {
        let content = std::fs::read(path).ok()?;
        let hash = blake3::hash(&content);
        Some(hash.to_hex().to_string())
    }

    pub fn is_changed(&self, path: &str) -> bool {
        let stored_hash: Option<String> = self.db.query_row(
            "SELECT hash FROM file_states WHERE path = ?1", params![path], |row| row.get(0)
        ).ok();
        match stored_hash {
            Some(h) => Self::hash_file(std::path::Path::new(path)).map(|current| current != h).unwrap_or(true),
            None => true, // New file
        }
    }

    pub fn update(&self, path: &str, issues_count: usize) {
        if let Some(hash) = Self::hash_file(std::path::Path::new(path)) {
            self.db.execute(
                "INSERT OR REPLACE INTO file_states (path, hash, issues_count, last_analyzed) VALUES (?1, ?2, ?3, ?4)",
                params![path, hash, issues_count as i64, chrono::Utc::now().to_rfc3339()],
            ).ok();
        }
    }

    pub fn get_state(&self, path: &str) -> Option<FileState> {
        self.db.query_row(
            "SELECT hash, issues_count, last_analyzed FROM file_states WHERE path = ?1",
            params![path], |row| Ok(FileState {
                hash: row.get(0)?, issues_count: row.get::<_, i64>(1)? as usize, last_analyzed: row.get(2)?
            })
        ).ok()
    }

    /// Extract imports from a source file (lightweight — just parse `use`/`import`/`require` statements)
    pub fn extract_imports(source: &str, file_path: &str) -> Vec<String> {
        let mut imports = Vec::new();
        let dir = Path::new(file_path).parent().unwrap_or(Path::new("."));

        for line in source.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            // JS/TS: import { X } from './module' or import X from './module'
            // Must check BEFORE Java because JS imports also end with semicolon
            if trimmed.starts_with("import ") && (trimmed.contains("from '") || trimmed.contains("from \"")) {
                if let Some(module) = Self::extract_js_import(trimmed, dir) {
                    imports.push(module);
                }
            }
            // Java: import com.package.Class; (ends with semicolon, no "from")
            else if trimmed.starts_with("import ") && trimmed.ends_with(';') {
                if let Some(module) = Self::extract_java_import(trimmed, dir) {
                    imports.push(module);
                }
            }
            // Python: from module import Type / import module
            else if trimmed.starts_with("from ") || trimmed.starts_with("import ") {
                if let Some(module) = Self::extract_python_import(trimmed, dir) {
                    imports.push(module);
                }
            }
            // Rust: use crate::module::Type;
            else if trimmed.starts_with("use ") {
                if let Some(module) = Self::extract_rust_import(trimmed, dir) {
                    imports.push(module);
                }
            }
        }
        imports
    }

    fn extract_rust_import(line: &str, _dir: &Path) -> Option<String> {
        // "use crate::module::Type;" → resolve to "src/module.rs" or "src/module/mod.rs"
        let re = Regex::new(r"use\s+(crate::|super::)?(\w+)").ok()?;
        let cap = re.captures(line)?;
        let module = cap.get(2)?.as_str();
        Some(format!("src/{}.rs", module))
    }

    fn extract_python_import(line: &str, _dir: &Path) -> Option<String> {
        let re = Regex::new(r"(?:from|import)\s+(\w+)").ok()?;
        let cap = re.captures(line)?;
        let module = cap.get(1)?.as_str();
        Some(format!("{}.py", module))
    }

    fn extract_js_import(line: &str, dir: &Path) -> Option<String> {
        // import { X } from './module' → resolve relative
        let re = Regex::new(r#"from\s+['"](\./[^'"]+)['"]"#).ok()?;
        let cap = re.captures(line)?;
        let relative = cap.get(1)?.as_str();
        let resolved = dir.join(relative);
        Some(resolved.to_string_lossy().to_string())
    }

    fn extract_java_import(line: &str, _dir: &Path) -> Option<String> {
        // import com.package.Class; → resolve to com/package/Class.java
        let re = Regex::new(r"import\s+([\w.]+)").ok()?;
        let cap = re.captures(line)?;
        let path = cap.get(1)?.as_str().replace('.', "/");
        Some(format!("{}.java", path))
    }

    /// Update the import table for a source file
    pub fn update_imports(&self, source_file: &str, imports: &[String]) {
        // Clear old imports for this file
        self.db.execute(
            "DELETE FROM file_imports WHERE source_file = ?1",
            params![source_file],
        ).ok();
        // Insert new imports
        for imported in imports {
            self.db.execute(
                "INSERT OR IGNORE INTO file_imports (source_file, imported_file) VALUES (?1, ?2)",
                params![source_file, imported],
            ).ok();
        }
    }

    /// Get files that import the given file
    pub fn get_dependents(&self, imported_file: &str) -> Vec<String> {
        let mut stmt = match self.db.prepare(
            "SELECT source_file FROM file_imports WHERE imported_file = ?1"
        ) {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };
        let rows = stmt.query_map(params![imported_file], |row| row.get::<_, String>(0)).ok();
        match rows {
            Some(r) => r.filter_map(|row| row.ok()).collect(),
            None => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_rust_import() {
        let source = r#"
use crate::user::User;
use std::collections::HashMap;
use super::parent::Module;
"#;
        let imports = FileStore::extract_imports(source, "src/handler.rs");
        assert!(imports.iter().any(|i| i.contains("user.rs")));
    }

    #[test]
    fn test_extract_python_import() {
        let source = r#"
from typing import List
import os
from mymodule import something
"#;
        let imports = FileStore::extract_imports(source, "script.py");
        assert!(imports.contains(&"typing.py".to_string()));
        assert!(imports.contains(&"os.py".to_string()));
        assert!(imports.contains(&"mymodule.py".to_string()));
    }

    #[test]
    fn test_extract_js_import() {
        let source = r#"
import { useState } from 'react';
import { helper } from './utils';
import styles from './styles.css';
"#;
        let imports = FileStore::extract_imports(source, "src/components/Button.js");
        // JS imports with 'from' and relative path
        assert!(imports.iter().any(|i| i.contains("utils")));
        assert!(imports.iter().any(|i| i.contains("styles.css")));
    }

    #[test]
    fn test_extract_java_import() {
        let source = r#"
import java.util.List;
import com.example.MyClass;
"#;
        let imports = FileStore::extract_imports(source, "src/Main.java");
        assert!(imports.contains(&"java/util/List.java".to_string()));
        assert!(imports.contains(&"com/example/MyClass.java".to_string()));
    }
}