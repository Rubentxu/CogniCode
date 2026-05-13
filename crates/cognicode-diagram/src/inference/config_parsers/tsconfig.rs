//! TypeScript tsconfig.json parser for path aliases and project structure

use std::path::Path;
use serde::{Deserialize, Serialize};

/// Parser for tsconfig.json files
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TsConfigInfo {
    /// The directory containing tsconfig.json
    pub base_url: Option<String>,
    /// Path aliases (e.g., "@/components" -> "src/components")
    pub paths: Vec<(String, String)>,
    /// Whether strict mode is enabled
    pub strict: bool,
    /// Compiler target (e.g., "ES2020")
    pub target: Option<String>,
    /// Module system (e.g., "ESNext", "CommonJS")
    pub module: Option<String>,
}

impl TsConfigInfo {
    /// Parse tsconfig.json file
    pub fn parse(path: &Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let value: serde_json::Value = serde_json::from_str(&content)?;

        let base_url = value["compilerOptions"]["baseUrl"]
            .as_str()
            .map(|s| s.to_string());

        let mut paths = Vec::new();
        if let Some(paths_obj) = value["compilerOptions"]["paths"].as_object() {
            for (alias, targets) in paths_obj {
                if let Some(targets_arr) = targets.as_array() {
                    if let Some(first_target) = targets_arr.first() {
                        if let Some(target_str) = first_target.as_str() {
                            paths.push((alias.clone(), target_str.to_string()));
                        }
                    }
                }
            }
        }

        let strict = value["compilerOptions"]["strict"]
            .as_bool()
            .unwrap_or(false);

        let target = value["compilerOptions"]["target"]
            .as_str()
            .map(|s| s.to_string());

        let module = value["compilerOptions"]["module"]
            .as_str()
            .map(|s| s.to_string());

        Ok(TsConfigInfo {
            base_url,
            paths,
            strict,
            target,
            module,
        })
    }

    /// Resolve a path alias to its actual path
    /// e.g., "@/components/Button" -> "src/components/Button"
    pub fn resolve_alias(&self, alias: &str, _base_dir: &Path) -> Option<String> {
        // Handle @/ pattern - check paths first
        if alias.starts_with("@/") {
            // Check if we have a matching path entry for @/components -> src/components
            let rest = &alias[2..]; // Remove @/
            for (alias_prefix, target_prefix) in &self.paths {
                // alias_prefix might be "@/components" or just "components"
                let prefix_to_check = alias_prefix.strip_prefix("@/").unwrap_or(alias_prefix);
                if rest.starts_with(prefix_to_check) {
                    let remainder = &rest[prefix_to_check.len()..];
                    return Some(format!("{}{}", target_prefix, remainder));
                }
            }
            // Fall back to base_url
            if let Some(ref base_url) = self.base_url {
                return Some(format!("{}/{}", base_url, rest));
            }
            // Default to src/
            return Some(format!("src/{}", rest));
        }

        // Handle other aliases
        for (alias_prefix, target_prefix) in &self.paths {
            if alias.starts_with(alias_prefix.as_str()) {
                let rest = &alias[alias_prefix.len()..];
                return Some(format!("{}{}", target_prefix, rest));
            }
        }

        None
    }

    /// Get the src directory based on tsconfig
    pub fn get_src_dir(&self) -> String {
        self.base_url.clone().unwrap_or_else(|| "src".to_string())
    }
}

/// Detect if a project is a Next.js project
pub fn detect_nextjs(project_dir: &Path) -> bool {
    // Check for Next.js specific files/directories
    let next_config = project_dir.join("next.config.js").exists()
        || project_dir.join("next.config.ts").exists()
        || project_dir.join("next.config.mjs").exists();

    let has_app_dir = project_dir.join("app").is_dir();
    let has_pages_dir = project_dir.join("pages").is_dir();

    next_config && (has_app_dir || has_pages_dir)
}

/// Detect if a project is a React project
pub fn detect_react(project_dir: &Path) -> bool {
    // Check package.json for react dependency
    let pkg_path = project_dir.join("package.json");
    if let Ok(content) = std::fs::read_to_string(&pkg_path) {
        if let Ok(pkg) = serde_json::from_str::<serde_json::Value>(&content) {
            let deps = pkg.get("dependencies").or(pkg.get("peerDependencies"));
            if let Some(deps_obj) = deps.and_then(|d| d.as_object()) {
                return deps_obj.contains_key("react") || deps_obj.contains_key("react-dom");
            }
        }
    }
    false
}

/// Detect if a project uses Vite
pub fn detect_vite(project_dir: &Path) -> bool {
    project_dir.join("vite.config.ts").exists()
        || project_dir.join("vite.config.js").exists()
        || project_dir.join("vite.config.mjs").exists()
}

/// Detect project type
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JsProjectType {
    NextJs,
    React,
    Vue,
    Angular,
    Svelte,
    NestJS,
    Express,
    Generic,
}

impl JsProjectType {
    pub fn detect(project_dir: &Path) -> Self {
        // Check package.json
        let pkg_path = project_dir.join("package.json");
        if let Ok(content) = std::fs::read_to_string(&pkg_path) {
            if let Ok(pkg) = serde_json::from_str::<serde_json::Value>(&content) {
                let deps = pkg.get("dependencies")
                    .or(pkg.get("devDependencies"))
                    .and_then(|d| d.as_object());

                if let Some(deps) = deps {
                    if deps.contains_key("next") {
                        return JsProjectType::NextJs;
                    }
                    if deps.contains_key("@nestjs/core") {
                        return JsProjectType::NestJS;
                    }
                    if deps.contains_key("react") {
                        return JsProjectType::React;
                    }
                    if deps.contains_key("vue") {
                        return JsProjectType::Vue;
                    }
                    if deps.contains_key("@angular/core") {
                        return JsProjectType::Angular;
                    }
                    if deps.contains_key("svelte") {
                        return JsProjectType::Svelte;
                    }
                    if deps.contains_key("express") {
                        return JsProjectType::Express;
                    }
                }
            }
        }

        JsProjectType::Generic
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_parse_tsconfig() {
        let temp_dir = TempDir::new().unwrap();
        let tsconfig_path = temp_dir.path().join("tsconfig.json");

        std::fs::write(
            &tsconfig_path,
            r#"{
                "compilerOptions": {
                    "baseUrl": ".",
                    "paths": {
                        "@/components/*": ["src/components/*"],
                        "@/utils/*": ["src/utils/*"]
                    },
                    "strict": true,
                    "target": "ES2020",
                    "module": "ESNext"
                }
            }"#,
        )
        .unwrap();

        let info = TsConfigInfo::parse(&tsconfig_path).unwrap();

        assert_eq!(info.base_url, Some(".".to_string()));
        assert!(info.strict);
        assert_eq!(info.target, Some("ES2020".to_string()));
        assert_eq!(info.paths.len(), 2);
    }

    #[test]
    fn test_resolve_alias() {
        let mut info = TsConfigInfo::default();
        info.base_url = Some(".".to_string());
        info.paths = vec![
            ("@/components".to_string(), "src/components".to_string()),
            ("@/utils".to_string(), "src/utils".to_string()),
        ];

        let base_dir = std::path::Path::new("/project");

        assert_eq!(
            info.resolve_alias("@/components/Button", base_dir),
            Some("src/components/Button".to_string())
        );
        assert_eq!(
            info.resolve_alias("@/utils/helpers", base_dir),
            Some("src/utils/helpers".to_string())
        );
    }

    #[test]
    fn test_project_type_detection() {
        let temp_dir = TempDir::new().unwrap();
        let pkg_path = temp_dir.path().join("package.json");

        // Test Next.js detection
        std::fs::write(
            &pkg_path,
            r#"{
                "dependencies": {
                    "next": "^13.0.0",
                    "react": "^18.0.0"
                }
            }"#,
        )
        .unwrap();

        std::fs::write(temp_dir.path().join("next.config.js"), "").unwrap();
        std::fs::create_dir(temp_dir.path().join("app")).unwrap();

        assert_eq!(
            JsProjectType::detect(temp_dir.path()),
            JsProjectType::NextJs
        );
    }
}
