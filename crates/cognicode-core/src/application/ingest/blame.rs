//! Git blame enrichment — attaches `last_author`, `author_email`, and
//! `codeowners` to symbol nodes based on `git blame` of the symbol's first line.
//!
//! Feature-gated behind `ownership`. Uses `git` CLI via std::process::Command.

use crate::application::ingest::codeowners::CodeOwnersMap;
use crate::application::ingest::types::ExtractionResult;

/// Enrich `result.nodes` with blame authorship and CODEOWNERS data.
///
/// Silently skips errors (non-git repos, binary files, unparseable, etc.)
#[cfg(feature = "ownership")]
pub fn enrich_with_blame(
    result: &mut ExtractionResult,
    root: &std::path::Path,
    codeowners: &CodeOwnersMap,
) {
    use std::path::Path;

    // Check if this is a git repo — git rev-parse will fail if not
    let is_git_repo = std::process::Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .current_dir(root)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if !is_git_repo {
        return;
    }

    for node in &mut result.nodes {
        // Skip nodes without a source path
        let source_path = match &node.source_path {
            Some(p) => p.clone(),
            None => continue,
        };

        // Compute repo-relative path
        let rel_path = match source_path.strip_prefix(root) {
            Ok(p) => p,
            Err(_) => continue,
        };
        let rel_path_str = rel_path.to_string_lossy();

        // Get the first line number from properties
        let line: u32 = match node.properties.get("line") {
            Some(v) => v.parse().unwrap_or(1),
            None => 1,
        };

        // Run git blame on this file at the specific line
        let (author_name, author_email) = match git_blame_author(root, &rel_path_str, line) {
            Some(pair) => pair,
            None => continue,
        };

        // Set blame properties on the node
        node.properties
            .insert("last_author".to_string(), author_name);
        node.properties
            .insert("author_email".to_string(), author_email);

        // Set CODEOWNERS property
        let owners = codeowners.owners_for(&rel_path_str);
        if !owners.is_empty() {
            node.properties
                .insert("codeowners".to_string(), owners.join(","));
        }
    }
}

/// Returns the author name and email for the given file at the given line.
/// Returns `None` on any error (binary file, no such path, etc.).
#[cfg(feature = "ownership")]
fn git_blame_author(
    repo_root: &std::path::Path,
    rel_path: &str,
    line: u32,
) -> Option<(String, String)> {
    // git blame --line-porcelain gives us author name and email in one shot
    let output = std::process::Command::new("git")
        .args([
            "blame",
            "--line-porcelain",
            "-L",
            &format!("{},{}", line, line),
            "--",
            rel_path,
        ])
        .current_dir(repo_root)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut author_name: Option<String> = None;
    let mut author_email: Option<String> = None;

    for line in stdout.lines() {
        if let Some(name) = line.strip_prefix("author ") {
            author_name = Some(name.to_string());
        } else if let Some(email) = line.strip_prefix("author-mail ") {
            // Strip <> from email like <test@example.com>
            author_email = Some(email.trim_matches(|c| c == '<' || c == '>').to_string());
        }
        if author_name.is_some() && author_email.is_some() {
            break;
        }
    }

    match (author_name, author_email) {
        (Some(name), Some(email)) => Some((name, email)),
        _ => None,
    }
}

#[cfg(feature = "ownership")]
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn init_git_repo(dir: &std::path::Path) {
        let output = std::process::Command::new("git")
            .args(["init"])
            .current_dir(dir)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git init failed: {:?}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn make_commit(dir: &std::path::Path) {
        // Add files
        let add_output = std::process::Command::new("git")
            .args(["add", "."])
            .current_dir(dir)
            .output()
            .unwrap();
        assert!(add_output.status.success(), "git add failed");

        // Commit with -c flag to override user.name/email
        let commit_output = std::process::Command::new("git")
            .args([
                "-C",
                &dir.to_string_lossy(),
                "-c",
                "user.name=Test User",
                "-c",
                "user.email=test@example.com",
                "commit",
                "-m",
                "initial commit",
            ])
            .current_dir(dir)
            .output()
            .unwrap();
        assert!(
            commit_output.status.success(),
            "git commit failed: {:?}",
            String::from_utf8_lossy(&commit_output.stderr)
        );
    }

    #[test]
    fn enrich_with_blame_non_git_repo_is_noop() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();

        // Create a file but NOT a git repo
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/main.rs"), "fn main() {}\n").unwrap();

        let mut result = ExtractionResult::ok(
            root.join("src/main.rs"),
            "abc123".to_string(),
            vec![
                crate::domain::aggregates::GraphNode::builder(
                    "test-node",
                    crate::domain::value_objects::NodeKind::Symbol(
                        crate::domain::value_objects::SymbolKind::Function,
                    ),
                )
                .source_path(root.join("src/main.rs"))
                .property("line", "1")
                .build(),
            ],
            vec![],
        );

        let codeowners = CodeOwnersMap::parse(root);
        enrich_with_blame(&mut result, root, &codeowners);

        // Should be unchanged - no git repo
        let node = &result.nodes[0];
        assert!(!node.properties.contains_key("last_author"));
        assert!(!node.properties.contains_key("author_email"));
    }

    #[test]
    fn enrich_with_blame_sets_last_author_and_email() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();

        init_git_repo(root);
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/main.rs"), "fn main() {}\n").unwrap();
        make_commit(root);

        let mut result = ExtractionResult::ok(
            root.join("src/main.rs"),
            "abc123".to_string(),
            vec![
                crate::domain::aggregates::GraphNode::builder(
                    "test-node",
                    crate::domain::value_objects::NodeKind::Symbol(
                        crate::domain::value_objects::SymbolKind::Function,
                    ),
                )
                .source_path(root.join("src/main.rs"))
                .property("line", "1")
                .build(),
            ],
            vec![],
        );

        let codeowners = CodeOwnersMap::parse(root);
        enrich_with_blame(&mut result, root, &codeowners);

        let node = &result.nodes[0];
        assert_eq!(
            node.properties.get("last_author"),
            Some(&"Test User".to_string()),
            "last_author should be set from git blame"
        );
        assert_eq!(
            node.properties.get("author_email"),
            Some(&"test@example.com".to_string()),
            "author_email should be set from git blame"
        );
    }

    #[test]
    fn enrich_with_blame_attaches_codeowners_to_symbols() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();

        init_git_repo(root);

        // Create .github/CODEOWNERS
        fs::create_dir_all(root.join(".github")).unwrap();
        fs::write(root.join(".github/CODEOWNERS"), "src/main.rs @alice @bob\n").unwrap();

        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/main.rs"), "fn main() {}\n").unwrap();
        make_commit(root);

        let mut result = ExtractionResult::ok(
            root.join("src/main.rs"),
            "abc123".to_string(),
            vec![
                crate::domain::aggregates::GraphNode::builder(
                    "test-node",
                    crate::domain::value_objects::NodeKind::Symbol(
                        crate::domain::value_objects::SymbolKind::Function,
                    ),
                )
                .source_path(root.join("src/main.rs"))
                .property("line", "1")
                .build(),
            ],
            vec![],
        );

        let codeowners = CodeOwnersMap::parse(root);
        enrich_with_blame(&mut result, root, &codeowners);

        let node = &result.nodes[0];
        assert_eq!(
            node.properties.get("codeowners"),
            Some(&"alice,bob".to_string()),
            "codeowners should be set from CODEOWNERS file"
        );
    }

    #[test]
    fn enrich_with_blame_uses_first_line_author() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();

        init_git_repo(root);
        fs::create_dir_all(root.join("src")).unwrap();
        // Multi-line file
        fs::write(
            root.join("src/main.rs"),
            "fn main() {\n    println!(\"hello\");\n}\n",
        )
        .unwrap();
        make_commit(root);

        let mut result = ExtractionResult::ok(
            root.join("src/main.rs"),
            "abc123".to_string(),
            vec![
                crate::domain::aggregates::GraphNode::builder(
                    "test-node",
                    crate::domain::value_objects::NodeKind::Symbol(
                        crate::domain::value_objects::SymbolKind::Function,
                    ),
                )
                .source_path(root.join("src/main.rs"))
                .property("line", "1") // First line attribution
                .build(),
            ],
            vec![],
        );

        let codeowners = CodeOwnersMap::parse(root);
        enrich_with_blame(&mut result, root, &codeowners);

        let node = &result.nodes[0];
        assert_eq!(
            node.properties.get("last_author"),
            Some(&"Test User".to_string()),
            "Should use first line author"
        );
    }
}
