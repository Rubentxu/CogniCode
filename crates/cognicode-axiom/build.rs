//! Build script: auto-discovers rule modules in the catalog directory
//! for automatic module declaration generation.

use std::env;
use std::fs;
use std::path::Path;

fn main() {
    let rules_dir = Path::new("src/rules");

    if !rules_dir.exists() {
        return;
    }

    println!("cargo:rerun-if-changed=src/rules/");

    // Collect all .rs files in the rules directory
    let mut modules = Vec::new();
    if let Ok(entries) = fs::read_dir(rules_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && path.extension().map(|e| e == "rs").unwrap_or(false) {
                if let Some(stem) = path.file_stem() {
                    let name = stem.to_string_lossy().to_string();
                    if name != "mod" {
                        modules.push(name);
                    }
                }
            }
        }
    }

    // Also check for catalog/ subdirectory
    let catalog_dir = rules_dir.join("catalog");
    if catalog_dir.exists() {
        println!("cargo:rerun-if-changed=src/rules/catalog/");
        if let Ok(entries) = fs::read_dir(&catalog_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() && path.extension().map(|e| e == "rs").unwrap_or(false) {
                    if let Some(stem) = path.file_stem() {
                        let name = format!("catalog/{}", stem.to_string_lossy().to_string());
                        modules.push(name);
                    }
                }
            }
        }
    }

    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("rules_auto.rs");

    let module_code: String = modules
        .iter()
        .map(|m| format!("pub mod {};\n", m))
        .collect();

    fs::write(&dest_path, module_code).unwrap();
}