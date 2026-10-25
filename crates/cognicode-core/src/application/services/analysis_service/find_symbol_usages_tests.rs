#[cfg(test)]
mod body {
    use crate::application::services::analysis_service::*;
    use std::fs;
    use std::path::PathBuf;

    fn fixture_corpus() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "cognicode-usages-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(dir.join("src")).unwrap();
        fs::create_dir_all(dir.join("target")).unwrap();

        fs::write(
            dir.join("src/lib.rs"),
            "fn alpha() -> i32 {\n    42\n}\n\nfn main() {\n    let x = alpha();\n    let y = alpha();\n    println!(\"{} {}\", x, y);\n}\n",
        )
        .unwrap();
        fs::write(dir.join("target/distractor.rs"), "fn alpha() {}").unwrap();

        dir
    }

    fn cleanup(dir: &PathBuf) {
        let _ = fs::remove_dir_all(dir);
    }

    // R1.1: include_declaration=true returns >= 3 occurrences,
    // exactly one flagged is_definition.
    #[test]
    fn r1_1_include_declaration_true_one_definition() {
        let dir = fixture_corpus();
        let service = AnalysisService::new();
        let usages = service
            .find_symbol_usages(UsageSearchParams {
                project_dir: dir.clone(),
                symbol_name: "alpha".to_string(),
                include_declaration: true,
                context_lines: None,
                first_only_definition: true,
            })
            .unwrap();
        cleanup(&dir);

        assert!(
            usages.len() >= 3,
            "expected >= 3 usages, got {}",
            usages.len()
        );
        let defs = usages.iter().filter(|u| u.is_definition).count();
        assert_eq!(defs, 1, "expected exactly one definition, got {}", defs);
    }

    // R1.2: include_declaration=false excludes definition occurrences.
    #[test]
    fn r1_2_exclude_declaration_omits_definition() {
        let dir = fixture_corpus();
        let service = AnalysisService::new();
        let usages = service
            .find_symbol_usages(UsageSearchParams {
                project_dir: dir.clone(),
                symbol_name: "alpha".to_string(),
                include_declaration: false,
                context_lines: None,
                first_only_definition: true,
            })
            .unwrap();
        cleanup(&dir);

        assert!(
            usages.iter().all(|u| !u.is_definition),
            "no definition should be present"
        );
        assert_eq!(usages.len(), 2, "only the two call sites remain");
    }

    // R1.3: files under target/, .git/, node_modules/ are never reported.
    #[test]
    fn r1_3_skip_dirs_never_reported() {
        let dir = fixture_corpus();
        let service = AnalysisService::new();
        let usages = service
            .find_symbol_usages(UsageSearchParams {
                project_dir: dir.clone(),
                symbol_name: "alpha".to_string(),
                include_declaration: true,
                context_lines: None,
                first_only_definition: true,
            })
            .unwrap();
        cleanup(&dir);

        assert!(
            usages.iter().all(|u| !u.file.contains("target/")),
            "no usage may come from target/"
        );
        assert!(
            usages.iter().all(|u| u.file.ends_with("src/lib.rs")),
            "only src/lib.rs may be reported"
        );
    }

    // R1.4: context_lines=Some(n) populates context; None leaves it absent.
    #[test]
    fn r1_4_context_lines_populated_or_absent() {
        let dir = fixture_corpus();
        let service = AnalysisService::new();

        let with_ctx = service
            .find_symbol_usages(UsageSearchParams {
                project_dir: dir.clone(),
                symbol_name: "alpha".to_string(),
                include_declaration: true,
                context_lines: Some(1),
                first_only_definition: true,
            })
            .unwrap();
        let without_ctx = service
            .find_symbol_usages(UsageSearchParams {
                project_dir: dir.clone(),
                symbol_name: "alpha".to_string(),
                include_declaration: true,
                context_lines: None,
                first_only_definition: true,
            })
            .unwrap();
        cleanup(&dir);

        assert!(with_ctx.iter().all(|u| u.context_lines.is_some()));
        let def = with_ctx.iter().find(|u| u.is_definition).unwrap();
        // Definition line: "fn alpha() -> i32 {" ; before is empty (line 0),
        // after holds line 2.
        assert_eq!(
            def.context_lines.as_ref().unwrap().current,
            "fn alpha() -> i32 {"
        );
        assert_eq!(def.context_lines.as_ref().unwrap().after, vec!["    42"]);
        assert!(without_ctx.iter().all(|u| u.context_lines.is_none()));
    }
}
