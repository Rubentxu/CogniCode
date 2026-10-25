#[cfg(test)]
mod body {
    use crate::domain::aggregates::CallGraph;
    use std::path::Path;

    use crate::application::services::analysis_service::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_get_file_symbols_python() {
        let mut file = NamedTempFile::with_suffix(".py").unwrap();
        writeln!(file, "def hello():").unwrap();
        writeln!(file, "    pass").unwrap();
        writeln!(file).unwrap();
        writeln!(file, "class MyClass:").unwrap();
        writeln!(file, "    def method(self):").unwrap();
        writeln!(file, "        pass").unwrap();

        let service = AnalysisService::new();
        let symbols = service.get_file_symbols(file.path()).unwrap();

        // Should find hello function and MyClass
        let names: Vec<_> = symbols.iter().map(|s| s.name.clone()).collect();
        assert!(
            names.contains(&"hello".to_string()),
            "Should find hello function"
        );
        assert!(
            names.contains(&"MyClass".to_string()),
            "Should find MyClass"
        );
    }

    #[test]
    fn test_get_file_symbols_rust() {
        let mut file = NamedTempFile::with_suffix(".rs").unwrap();
        writeln!(file, "fn main() {{}}").unwrap();
        writeln!(file).unwrap();
        writeln!(file, "struct MyStruct {{}}").unwrap();

        let service = AnalysisService::new();
        let symbols = service.get_file_symbols(file.path()).unwrap();

        let names: Vec<_> = symbols.iter().map(|s| s.name.clone()).collect();
        assert!(
            names.contains(&"main".to_string()),
            "Should find main function"
        );
        assert!(
            names.contains(&"MyStruct".to_string()),
            "Should find MyStruct"
        );
    }

    #[test]
    fn test_get_file_symbols_unsupported_extension() {
        let mut file = NamedTempFile::with_suffix(".xyz").unwrap();
        writeln!(file, "some content").unwrap();

        let service = AnalysisService::new();
        let result = service.get_file_symbols(file.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_symbol_locations_have_correct_file_path() {
        let mut file = NamedTempFile::with_suffix(".py").unwrap();
        writeln!(file, "def hello():").unwrap();
        writeln!(file, "    pass").unwrap();

        let service = AnalysisService::new();
        let symbols = service.get_file_symbols(file.path()).unwrap();

        assert!(!symbols.is_empty());
        assert_eq!(symbols[0].file_path, file.path().to_str().unwrap());
    }

    #[test]
    fn test_build_project_graph() {
        // Create a temp directory with Python files
        let temp_dir = tempfile::tempdir().unwrap();
        let temp_path = temp_dir.path();

        // Create first Python file
        let file1_path = temp_path.join("module1.py");
        std::fs::write(
            &file1_path,
            r#"
    def a():
    b()
    c()

    def b():
    c()

    def c():
    pass
    "#,
        )
        .unwrap();

        // Create second Python file
        let file2_path = temp_path.join("module2.py");
        std::fs::write(
            &file2_path,
            r#"
    def d():
    a()
    c()
    "#,
        )
        .unwrap();

        let service = AnalysisService::new();
        service.build_project_graph(temp_path).unwrap();

        let graph = service.get_project_graph();

        // The graph should have symbols from both files
        assert!(
            graph.symbol_count() >= 4,
            "Should have at least 4 symbols (a, b, c, d)"
        );
    }

    #[test]
    fn test_check_cycles_empty_graph() {
        let service = AnalysisService::new();
        let graph = CallGraph::new();

        let result = service.check_cycles(&graph);
        assert!(!result.has_cycles);
        assert!(result.cycles.is_empty());
    }

    #[test]
    fn test_check_cycles_with_cycle() {
        use crate::domain::aggregates::Symbol;
        use crate::domain::value_objects::{DependencyType, Location, SymbolKind};

        let service = AnalysisService::new();
        let mut graph = CallGraph::new();

        // Create a -> b -> a cycle
        let a = Symbol::new("a", SymbolKind::Function, Location::new("test.rs", 1, 1));
        let b = Symbol::new("b", SymbolKind::Function, Location::new("test.rs", 2, 1));

        let id_a = graph.add_symbol(a);
        let id_b = graph.add_symbol(b);

        graph
            .add_dependency(&id_a, &id_b, DependencyType::Calls)
            .unwrap();
        graph
            .add_dependency(&id_b, &id_a, DependencyType::Calls)
            .unwrap();

        let result = service.check_cycles(&graph);
        assert!(result.has_cycles);
        assert_eq!(result.cycles.len(), 1);
    }

    #[test]
    fn test_analyze_impact_no_dependents() {
        use crate::domain::aggregates::Symbol;
        use crate::domain::value_objects::{Location, SymbolKind};

        let service = AnalysisService::new();
        let graph = CallGraph::new();

        let symbol = Symbol::new(
            "orphan",
            SymbolKind::Function,
            Location::new("test.rs", 1, 1),
        );
        let report = service.analyze_impact(&symbol, &graph);

        assert_eq!(report.direct_dependents, 0);
        assert_eq!(report.transitive_dependents, 0);
        assert_eq!(
            report.impact_level,
            crate::domain::services::ImpactLevel::Minimal
        );
    }

    #[test]
    fn test_analyze_impact_with_dependents() {
        use crate::domain::aggregates::Symbol;
        use crate::domain::value_objects::{DependencyType, Location, SymbolKind};

        let service = AnalysisService::new();
        let mut graph = CallGraph::new();

        let main = Symbol::new("main", SymbolKind::Function, Location::new("main.rs", 1, 1));
        let helper = Symbol::new(
            "helper",
            SymbolKind::Function,
            Location::new("helper.rs", 5, 1),
        );

        let main_id = graph.add_symbol(main);
        let helper_id = graph.add_symbol(helper);

        graph
            .add_dependency(&main_id, &helper_id, DependencyType::Calls)
            .unwrap();

        let helper_symbol = Symbol::new(
            "helper",
            SymbolKind::Function,
            Location::new("helper.rs", 5, 1),
        );
        let report = service.analyze_impact(&helper_symbol, &graph);

        assert_eq!(report.direct_dependents, 1);
        assert!(report.impact_level <= crate::domain::services::ImpactLevel::Medium);
    }

    #[test]
    fn test_calculate_complexity_simple() {
        let service = AnalysisService::new();

        let report = service.calculate_complexity("test_func", &[], 1);

        assert_eq!(report.symbol_name, "test_func");
        assert_eq!(report.cyclomatic, 1);
        assert_eq!(report.risk, crate::domain::services::ComplexityRisk::Low);
    }

    #[test]
    fn test_calculate_complexity_with_decisions() {
        use crate::domain::services::DecisionPoint;

        let service = AnalysisService::new();

        let decision_points = vec![DecisionPoint::If, DecisionPoint::While, DecisionPoint::For];
        let report = service.calculate_complexity("complex_func", &decision_points, 1);

        assert_eq!(report.cyclomatic, 4); // 1 base + 3 decision points
        assert_eq!(report.decision_point_count, 3);
    }

    #[test]
    fn test_is_safe_to_change() {
        use crate::domain::aggregates::Symbol;
        use crate::domain::services::ImpactThreshold;
        use crate::domain::value_objects::{DependencyType, Location, SymbolKind};

        let service = AnalysisService::new();
        let mut graph = CallGraph::new();

        let main = Symbol::new("main", SymbolKind::Function, Location::new("main.rs", 1, 1));
        let helper = Symbol::new(
            "helper",
            SymbolKind::Function,
            Location::new("helper.rs", 5, 1),
        );

        let main_id = graph.add_symbol(main);
        let helper_id = graph.add_symbol(helper);

        graph
            .add_dependency(&main_id, &helper_id, DependencyType::Calls)
            .unwrap();

        let helper_symbol = Symbol::new(
            "helper",
            SymbolKind::Function,
            Location::new("helper.rs", 5, 1),
        );

        // With moderate threshold, should be safe
        let is_safe =
            service.is_safe_to_change(&helper_symbol, &graph, ImpactThreshold::moderate());
        assert!(is_safe);

        // With very strict threshold, may not be safe
        let strict = ImpactThreshold {
            max_level: crate::domain::services::ImpactLevel::Minimal,
            max_dependents: 0,
        };
        let is_safe_strict = service.is_safe_to_change(&helper_symbol, &graph, strict);
        assert!(!is_safe_strict);
    }

    #[test]
    fn test_find_minimal_feedback_set_no_cycles() {
        use crate::domain::aggregates::Symbol;
        use crate::domain::value_objects::{DependencyType, Location, SymbolKind};

        let service = AnalysisService::new();
        let mut graph = CallGraph::new();

        // Linear graph: a -> b -> c (no cycles)
        let a = Symbol::new("a", SymbolKind::Function, Location::new("test.rs", 1, 1));
        let b = Symbol::new("b", SymbolKind::Function, Location::new("test.rs", 2, 1));
        let c = Symbol::new("c", SymbolKind::Function, Location::new("test.rs", 3, 1));

        let id_a = graph.add_symbol(a);
        let id_b = graph.add_symbol(b);
        let id_c = graph.add_symbol(c);

        graph
            .add_dependency(&id_a, &id_b, DependencyType::Calls)
            .unwrap();
        graph
            .add_dependency(&id_b, &id_c, DependencyType::Calls)
            .unwrap();

        let feedback_set = service.find_minimal_feedback_set(&graph);
        assert!(feedback_set.is_empty());
    }

    #[test]
    fn test_find_minimal_feedback_set_with_cycle() {
        use crate::domain::aggregates::Symbol;
        use crate::domain::value_objects::{DependencyType, Location, SymbolKind};

        let service = AnalysisService::new();
        let mut graph = CallGraph::new();

        // a -> b -> a cycle
        let a = Symbol::new("a", SymbolKind::Function, Location::new("test.rs", 1, 1));
        let b = Symbol::new("b", SymbolKind::Function, Location::new("test.rs", 2, 1));

        let id_a = graph.add_symbol(a);
        let id_b = graph.add_symbol(b);

        graph
            .add_dependency(&id_a, &id_b, DependencyType::Calls)
            .unwrap();
        graph
            .add_dependency(&id_b, &id_a, DependencyType::Calls)
            .unwrap();

        let feedback_set = service.find_minimal_feedback_set(&graph);
        // Should find at least one symbol to break the cycle
        assert!(!feedback_set.is_empty());
    }

    #[test]
    fn test_full_analysis_workflow() {
        use std::io::Write;
        use tempfile::TempDir;

        let service = AnalysisService::new();
        let temp_dir = TempDir::new().unwrap();

        // Create a test Rust file with multiple symbols and call relationships
        let rust_file_path = temp_dir.path().join("test_lib.rs");
        let mut file = std::fs::File::create(&rust_file_path).unwrap();
        writeln!(file, "pub fn helper_function() -> i32 {{").unwrap();
        writeln!(file, "    return 42;").unwrap();
        writeln!(file, "}}").unwrap();
        writeln!(file).unwrap();
        writeln!(file, "pub fn main_function() -> i32 {{").unwrap();
        writeln!(file, "    let x = helper_function();").unwrap();
        writeln!(file, "    if x > 0 {{").unwrap();
        writeln!(file, "        return x;").unwrap();
        writeln!(file, "    }}").unwrap();
        writeln!(file, "    return 0;").unwrap();
        writeln!(file, "}}").unwrap();
        drop(file);

        // 1. TEST: get_file_symbols - extract symbols from a file
        let symbols = service.get_file_symbols(&rust_file_path).unwrap();
        let symbol_names: Vec<_> = symbols.iter().map(|s| s.name.clone()).collect();
        assert!(
            symbol_names.contains(&"helper_function".to_string()),
            "Should find helper_function"
        );
        assert!(
            symbol_names.contains(&"main_function".to_string()),
            "Should find main_function"
        );

        // 2. TEST: build_project_graph - build call graph for the project
        service.build_project_graph(temp_dir.path()).unwrap();
        let graph = service.get_project_graph();
        assert!(
            graph.symbol_count() >= 2,
            "Graph should have at least 2 symbols"
        );

        // 3. TEST: check_cycles - detect cycles in the graph
        let cycle_result = service.check_cycles(&graph);
        assert!(
            !cycle_result.has_cycles,
            "Simple linear calls should not have cycles"
        );

        // 4. TEST: analyze_impact - analyze impact of changing a symbol
        //
        // Use the symbol the graph holds rather than constructing one by hand.
        // FQN is the key analyze_impact looks up, and the graph records the
        // absolute path and a 0-indexed line: this fixture's helper_function
        // is keyed `/tmp/.../test_lib.rs:helper_function:0`, while a hand-built
        // `Symbol::new("helper_function", ..., "test_lib.rs", 1, 4)` produces
        // `test_lib.rs:helper_function:1` and matches nothing. The edge itself
        // is fine -- the lookup key was wrong, which is why the old `>= 0`
        // assertion hid it: it passed on a report that found no dependents at
        // all, under a comment asserting the opposite.
        let helper_symbol = graph
            .symbols()
            .find(|s| s.name() == "helper_function")
            .cloned()
            .expect("the graph must contain helper_function");
        let impact_report = service.analyze_impact(&helper_symbol, &graph);
        assert_eq!(
            impact_report.direct_dependents, 1,
            "main_function is the only caller of helper_function in this fixture"
        );
        assert!(
            impact_report.transitive_dependents >= impact_report.direct_dependents,
            "a direct dependent is also a transitive dependent; got transitive={} \
             direct={}",
            impact_report.transitive_dependents,
            impact_report.direct_dependents
        );

        // 5. TEST: calculate_complexity - calculate cyclomatic complexity
        use crate::domain::services::DecisionPoint;
        let decision_points = vec![DecisionPoint::If];
        let complexity_report = service.calculate_complexity("main_function", &decision_points, 2);
        assert_eq!(complexity_report.symbol_name, "main_function");
        // Formula: base(1) + decision_points(1) + exit_points(2) - 1 = 3
        assert_eq!(
            complexity_report.cyclomatic, 3,
            "Should have cyclomatic complexity of 3 for 1 if and 2 exit points"
        );
        assert_eq!(
            complexity_report.risk,
            crate::domain::services::ComplexityRisk::Low
        );

        // 6. TEST: is_safe_to_change - check if it's safe to refactor
        let threshold = crate::domain::services::ImpactThreshold::conservative();
        let is_safe = service.is_safe_to_change(&helper_symbol, &graph, threshold);
        // With low impact, should be safe
        assert!(
            impact_report.impact_level <= crate::domain::services::ImpactLevel::Medium || is_safe
        );

        // 7. TEST: find_minimal_feedback_set - find symbols to break cycles
        let feedback_set = service.find_minimal_feedback_set(&graph);
        // Without cycles, feedback set should be empty
        assert!(
            feedback_set.is_empty(),
            "Without cycles, feedback set should be empty"
        );
    }

    #[test]
    #[ignore = "integration: scans entire project via build_project_graph (>5 min, not appropriate for normal suite). End-to-end path 'build_project_graph → graph → get_file_symbols' is bounded-covered by test_debug_call_relationships_in_real_code (single file, 0.1s) and test_lightweight_index_real_project_benchmark + test_on_demand_graph_real_project_benchmark (full cognicode-core scan, ≤15s). See M0.12 audit 2026-09-27."]
    fn test_real_code_analysis_workflow() {
        let service = AnalysisService::new();
        let project_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));

        // 1. Build graph from real source code
        service
            .build_project_graph(&project_root)
            .expect("Should build project graph");
        let graph = service.get_project_graph();

        // Verify we got real symbols from the codebase
        let symbol_count = graph.symbol_count();
        assert!(
            symbol_count > 100,
            "Should find many symbols in real codebase, found {}",
            symbol_count
        );
        println!("[REAL CODE] Found {} symbols in codebase", symbol_count);

        // 2. Get file symbols from a real file
        let real_file = project_root.join("src/application/services/analysis_service.rs");
        let file_symbols = service
            .get_file_symbols(&real_file)
            .expect("Should get file symbols");
        assert!(!file_symbols.is_empty(), "Should find symbols in real file");
        println!(
            "[REAL CODE] Found {} symbols in analysis_service.rs",
            file_symbols.len()
        );

        // Find a function to analyze
        let target_symbol = file_symbols
            .iter()
            .find(|s| s.name == "build_project_graph")
            .expect("Should find build_project_graph function");

        println!(
            "[REAL CODE] Target symbol: {} at {}:{}:{}",
            target_symbol.name, target_symbol.file_path, target_symbol.line, target_symbol.column
        );

        // 3. Check for cycles
        let cycle_result = service.check_cycles(&graph);
        println!(
            "[REAL CODE] Cycle detection: has_cycles={}, total_sccs={}",
            cycle_result.has_cycles, cycle_result.total_sccs
        );

        // 4. Analyze impact of a real symbol
        //
        // Take the symbol from the graph, not one rebuilt from the file-scan
        // record. FQN is the key analyze_impact looks up, and it is assembled
        // from the exact path and line the graph recorded; a symbol rebuilt
        // from a separately-parsed location can key differently and silently
        // match nothing, which is what the old `>= 0` assertion permitted.
        let symbol_for_impact = graph
            .symbols()
            .find(|s| s.name() == "build_project_graph")
            .cloned()
            .expect("the project graph must contain build_project_graph");
        let impact_report = service.analyze_impact(&symbol_for_impact, &graph);
        println!(
            "[REAL CODE] Impact analysis: direct={}, transitive={}, level={:?}",
            impact_report.direct_dependents,
            impact_report.transitive_dependents,
            impact_report.impact_level
        );

        // Whether this real symbol has dependents depends on the real code, so
        // pinning a count here would pin nothing. Two things must hold: the
        // graph really is queryable by this symbol, and the transitive set
        // contains the direct one. `>= 0` was true of every usize and held
        // even if impact analysis returned an empty, incoherent report.
        assert!(
            graph
                .get_symbol(&crate::domain::aggregates::call_graph::SymbolId::new(
                    symbol_for_impact.fully_qualified_name()
                ))
                .is_some(),
            "a symbol taken from the graph must be resolvable in that graph; \
             otherwise impact analysis is reporting on nothing"
        );
        assert!(
            impact_report.transitive_dependents >= impact_report.direct_dependents,
            "a direct dependent is also a transitive dependent, so the transitive \
             count cannot be lower; got transitive={} direct={}",
            impact_report.transitive_dependents,
            impact_report.direct_dependents
        );

        // 5. Test safety check
        let threshold = crate::domain::services::ImpactThreshold::conservative();
        let is_safe = service.is_safe_to_change(&symbol_for_impact, &graph, threshold.clone());
        println!(
            "[REAL CODE] Safety check: is_safe={} (threshold={:?})",
            is_safe, threshold.max_level
        );

        // 6. Find feedback set (for breaking cycles if any)
        let feedback_set = service.find_minimal_feedback_set(&graph);
        println!(
            "[REAL CODE] Minimal feedback set size: {}",
            feedback_set.len()
        );

        // 7. Test call graph traversal - find a symbol with callers
        let symbols_with_callers: Vec<_> = graph
            .symbols()
            .filter(|s| {
                let sid =
                    crate::domain::aggregates::call_graph::SymbolId::new(s.fully_qualified_name());
                !graph.callers(&sid).is_empty()
            })
            .take(5)
            .collect();

        println!(
            "[REAL CODE] Symbols with callers: {}",
            symbols_with_callers.len()
        );
        for sym in &symbols_with_callers {
            let sid =
                crate::domain::aggregates::call_graph::SymbolId::new(sym.fully_qualified_name());
            let callers = graph.callers(&sid);
            println!("  - {} ({} callers)", sym.name(), callers.len());
        }

        // Verify we can traverse the graph
        assert!(graph.symbol_count() > 0, "Graph should have symbols");
        println!(
            "[REAL CODE] Graph stats: symbols={}, edges={}, sccs={}",
            graph.symbol_count(),
            graph.edge_count(),
            cycle_result.total_sccs
        );

        // Note: edges may be 0 if call relationships weren't detected
        // This can happen if tree-sitter queries don't match the code patterns
        if graph.edge_count() == 0 {
            println!(
                "[REAL CODE] WARNING: No edges detected. Call relationships may not be parsed."
            );
        }
    }

    #[test]
    fn test_debug_call_relationships_in_real_code() {
        use crate::domain::traits::DependencyRepository;
        use crate::infrastructure::parser::{Language, TreeSitterParser};

        let parser = TreeSitterParser::new(Language::Rust).unwrap();
        let real_file = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("src/application/services/analysis_service.rs");

        let source = std::fs::read_to_string(&real_file).expect("Should read real file");

        let relationships = parser
            .find_call_relationships(&source, &real_file.to_string_lossy())
            .expect("Should find relationships");

        println!(
            "[DEBUG] Found {} call relationships in analysis_service.rs",
            relationships.len()
        );

        for (i, (caller, callee)) in relationships.iter().take(20).enumerate() {
            println!("[DEBUG] {}. {} -> {}", i + 1, caller.name(), callee);
        }

        // Now test with a real PetGraphStore to see if edges get added
        let mut store = crate::infrastructure::graph::PetGraphStore::new();

        // Add symbols first
        for (caller, _) in &relationships {
            let caller_id =
                crate::domain::aggregates::call_graph::SymbolId::new(caller.fully_qualified_name());
            store
                .add_dependency(
                    &caller_id,
                    &caller_id,
                    crate::domain::value_objects::DependencyType::Defines,
                )
                .ok();
        }

        let symbols_in_store = store.get_all_symbols();
        println!("[DEBUG] Symbols in store: {}", symbols_in_store.len());
        for (i, sym) in symbols_in_store.iter().take(10).enumerate() {
            println!(
                "[DEBUG]   {}. {} (fqn: {})",
                i + 1,
                sym.name(),
                sym.fully_qualified_name()
            );
        }

        // Now try to find a callee
        let callee_to_find = "WalkDir";
        let found = store
            .get_all_symbols()
            .into_iter()
            .find(|s| s.name() == callee_to_find);
        println!(
            "[DEBUG] Looking for '{}': found={}",
            callee_to_find,
            found.is_some()
        );
        if let Some(f) = found {
            println!(
                "[DEBUG]   Found: {} with fqn: {}",
                f.name(),
                f.fully_qualified_name()
            );
        }

        // Re-enabled by M0.12 (#[ignore] audit 2026-09-27):
        // this test parses analysis_service.rs (~3000+ lines) end-to-end
        // and verifies call relationships + symbol store integration.
        // Was #[ignore]d under "integration: parses 1400+ line real
        // source file" — but it is bounded (single file, ~0.1s) and
        // pins the contract that find_call_relationships + PetGraphStore
        // work on real source. Lesson 79.
        assert!(
            !relationships.is_empty(),
            "Should find at least some call relationships in real code (got {})",
            relationships.len()
        );
        assert!(
            !symbols_in_store.is_empty(),
            "PetGraphStore should hold ≥1 symbol after adding relationships (got {})",
            symbols_in_store.len()
        );
    }

    #[test]
    #[ignore = "integration: scans entire project via build_project_graph (>5 min). The enhanced features (entry_points, dead_code, hot_paths, etc.) are bounded-covered by other unit tests in this file; the full graph stats pin is in test_lightweight_index_real_project_benchmark (10708 symbols, 28337 locations). See M0.12 audit 2026-09-27."]
    fn test_enhanced_call_graph_features() {
        let service = AnalysisService::new();
        let project_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));

        // Build the project graph first
        service
            .build_project_graph(&project_root)
            .expect("Should build project graph");
        let graph = service.get_project_graph();

        println!(
            "[ENHANCED] Graph stats: symbols={}, edges={}",
            graph.symbol_count(),
            graph.edge_count()
        );

        // 1. Test get_entry_points (symbols with no incoming edges)
        let entry_points = service.get_entry_points();
        println!("[ENHANCED] Entry points found: {}", entry_points.len());
        assert!(!entry_points.is_empty(), "Should find some entry points");
        for ep in entry_points.iter().take(5) {
            println!("  - {} in {}", ep.name, ep.file_path);
        }

        // 2. Test get_leaf_functions (symbols with no outgoing edges)
        let leaf_functions = service.get_leaf_functions();
        println!("[ENHANCED] Leaf functions found: {}", leaf_functions.len());
        assert!(
            !leaf_functions.is_empty(),
            "Should find some leaf functions"
        );
        for lf in leaf_functions.iter().take(5) {
            println!("  - {} in {}", lf.name, lf.file_path);
        }

        // 3. Test trace_path - find a path between two symbols
        // Find two symbols that might be connected
        if graph.symbol_count() > 1 {
            let symbols: Vec<_> = graph.symbols().take(2).collect();
            if symbols.len() == 2 {
                let source = symbols[0].name();
                let target = symbols[1].name();
                println!(
                    "[ENHANCED] Trying trace_path from '{}' to '{}'",
                    source, target
                );
                let path_result = service.trace_path(source, target, 0);
                match path_result {
                    Ok(Some(path)) => {
                        println!("[ENHANCED] Path found with {} hops", path.len());
                    }
                    Ok(None) => {
                        println!(
                            "[ENHANCED] No path found between '{}' and '{}'",
                            source, target
                        );
                    }
                    Err(e) => {
                        println!("[ENHANCED] Error tracing path: {:?}", e);
                    }
                }
            }
        }

        // 4. Test fan_in and fan_out on CallGraph directly
        if let Some(symbol) = graph.symbols().next() {
            let sid =
                crate::domain::aggregates::call_graph::SymbolId::new(symbol.fully_qualified_name());
            let fan_in = graph.fan_in(&sid);
            let fan_out = graph.fan_out(&sid);
            println!(
                "[ENHANCED] Symbol '{}': fan_in={}, fan_out={}",
                symbol.name(),
                fan_in,
                fan_out
            );
        }

        // 5. Test to_mermaid export (just verify it doesn't crash)
        let mermaid = graph.to_mermaid("Test Graph");
        println!("[ENHANCED] Mermaid export length: {} chars", mermaid.len());
        assert!(
            mermaid.contains("flowchart"),
            "Mermaid export should contain 'flowchart'"
        );
        if mermaid.len() < 500 {
            println!("[ENHANCED] Mermaid preview:\n{}", mermaid);
        }

        // 6. Test CallGraphAnalyzer hot paths
        use crate::domain::services::CallGraphAnalyzer;
        let analyzer = CallGraphAnalyzer::new();
        let hot_paths = analyzer.find_hot_paths(&graph, 10);
        println!("[ENHANCED] Hot paths found: {}", hot_paths.len());
        for hp in hot_paths.iter().take(5) {
            println!("  - {} (fan_in={})", hp.symbol_name, hp.fan_in);
        }

        // 7. Test complexity metrics
        let complexity = analyzer.calculate_complexity(&graph);
        println!(
            "[ENHANCED] Complexity: symbols={}, edges={}, max_depth={}",
            complexity.total_symbols, complexity.total_edges, complexity.max_depth
        );

        // Verify basic assertions
        assert!(graph.symbol_count() > 0, "Graph should have symbols");
    }

    #[test]
    fn test_traverse_callees_and_callers() {
        use crate::domain::aggregates::{CallGraph, Symbol};
        use crate::domain::value_objects::{DependencyType, Location, SymbolKind};

        let mut graph = CallGraph::new();

        // Create: a -> b -> c, and a -> d
        let a = Symbol::new("a", SymbolKind::Function, Location::new("test.rs", 1, 1));
        let b = Symbol::new("b", SymbolKind::Function, Location::new("test.rs", 2, 1));
        let c = Symbol::new("c", SymbolKind::Function, Location::new("test.rs", 3, 1));
        let d = Symbol::new("d", SymbolKind::Function, Location::new("test.rs", 4, 1));

        let id_a = graph.add_symbol(a);
        let id_b = graph.add_symbol(b);
        let id_c = graph.add_symbol(c);
        let id_d = graph.add_symbol(d);

        graph
            .add_dependency(&id_a, &id_b, DependencyType::Calls)
            .unwrap();
        graph
            .add_dependency(&id_b, &id_c, DependencyType::Calls)
            .unwrap();
        graph
            .add_dependency(&id_a, &id_d, DependencyType::Calls)
            .unwrap();

        // Test traverse_callees with depth=1 from 'a'
        let callees_depth1 = graph.traverse_callees(&id_a, 1);
        println!(
            "[TRAVERSE] callees of 'a' with depth=1: {:?}",
            callees_depth1.len()
        );
        for ce in &callees_depth1 {
            println!("  - {} at {}:{}", ce.symbol_name, ce.file, ce.line);
        }
        assert!(
            callees_depth1.len() == 2,
            "Should have 2 direct callees (b and d)"
        );

        // Test traverse_callees with depth=2 from 'a'
        let callees_depth2 = graph.traverse_callees(&id_a, 2);
        println!(
            "[TRAVERSE] callees of 'a' with depth=2: {:?}",
            callees_depth2.len()
        );
        for ce in &callees_depth2 {
            println!("  - {} at {}:{}", ce.symbol_name, ce.file, ce.line);
        }
        assert!(
            callees_depth2.len() >= 2,
            "Should have at least 2 callees (b, d, c)"
        );

        // Test traverse_callers with depth=1 from 'c'
        let callers_c = graph.traverse_callers(&id_c, 1);
        println!(
            "[TRAVERSE] callers of 'c' with depth=1: {:?}",
            callers_c.len()
        );
        assert!(callers_c.len() == 1, "Should have 1 direct caller (b)");

        // Test traverse_callers with depth=2 from 'c'
        let callers_c_depth2 = graph.traverse_callers(&id_c, 2);
        println!(
            "[TRAVERSE] callers of 'c' with depth=2: {:?}",
            callers_c_depth2.len()
        );
        assert!(
            callers_c_depth2.len() >= 1,
            "Should have at least 1 caller (a through b)"
        );

        // Test fan_in and fan_out
        assert_eq!(
            graph.fan_in(&id_a),
            0,
            "a should have fan_in=0 (no callers)"
        );
        assert_eq!(
            graph.fan_out(&id_a),
            2,
            "a should have fan_out=2 (calls b and d)"
        );
        assert_eq!(
            graph.fan_in(&id_c),
            1,
            "c should have fan_in=1 (called by b)"
        );
        assert_eq!(
            graph.fan_out(&id_c),
            0,
            "c should have fan_out=0 (no callees)"
        );
    }

    #[test]
    fn test_mermaid_export() {
        use crate::domain::aggregates::{CallGraph, Symbol};
        use crate::domain::value_objects::{DependencyType, Location, SymbolKind};

        let mut graph = CallGraph::new();

        let a = Symbol::new("main", SymbolKind::Function, Location::new("main.rs", 1, 1));
        let b = Symbol::new(
            "process",
            SymbolKind::Function,
            Location::new("main.rs", 10, 1),
        );
        let c = Symbol::new(
            "validate",
            SymbolKind::Function,
            Location::new("util.rs", 5, 1),
        );

        let id_a = graph.add_symbol(a);
        let id_b = graph.add_symbol(b);
        let id_c = graph.add_symbol(c);

        graph
            .add_dependency(&id_a, &id_b, DependencyType::Calls)
            .unwrap();
        graph
            .add_dependency(&id_b, &id_c, DependencyType::Calls)
            .unwrap();

        let mermaid = graph.to_mermaid("Test Call Graph");

        println!("[MERMAID] Generated diagram:\n{}", mermaid);

        assert!(mermaid.contains("flowchart"), "Should contain 'flowchart'");
        assert!(mermaid.contains("main"), "Should contain 'main' node");
        assert!(mermaid.contains("process"), "Should contain 'process' node");
        assert!(
            mermaid.contains("validate"),
            "Should contain 'validate' node"
        );
        assert!(mermaid.contains("-->"), "Should contain edges");
    }

    // =========================================================================
    // H4.2 - no invented edges
    // =========================================================================

    /// MEDIDO (RED): este proyecto producía una arista `main → helper`.
    ///
    /// `helper` está declarada en `alpha.rs` **y** en `beta.rs`. El
    /// constructorFiltered resolvía contra un `HashMap<String, SymbolId>`
    /// plano, donde el segundo `insert` pisa al primero: el `get` devolvía
    /// siempre exactamente un símbolo y la arista se dibujaba contra el
    /// homónimo que sobrevivió por azar de orden de recorrido.
    ///
    /// Eso es la arista falsa que toda la ley de H4/K2 prohíbe, y que la
    /// ruta `build_project_graph` ya evita usando `GlobalSymbolIndex`.
    /// La ley se cumple por construcción cuando solo `Resolved` nombra
    /// un destino: aquí no hay regla que desambigue dos homónimos, así
    /// que no puede haber destino.
    #[test]
    fn h42_filtered_graph_invents_no_edge_against_a_homonym() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();
        for (file, body) in [
            ("alpha.rs", "pub fn helper() -> i32 { 1 }"),
            ("beta.rs", "pub fn helper() -> i32 { 2 }"),
            ("main.rs", "pub fn caller() -> i32 { helper() }"),
        ] {
            std::fs::write(root.join(file), body).unwrap();
        }

        let service = AnalysisService::new();
        let report = service
            .build_project_graph_filtered(root, &[root])
            .expect("filtered graph should build");
        let graph = &report.graph;

        let helper_edges = graph
            .all_dependencies()
            .filter(|(_, target, _)| {
                graph
                    .get_symbol(target)
                    .map(|s| s.name().eq_ignore_ascii_case("helper"))
                    .unwrap_or(false)
            })
            .count();

        assert_eq!(
            helper_edges, 0,
            "two homonyms with no rule to disambiguate must produce no edge, \
             got {helper_edges}"
        );
    }

    /// H4.2, second site. `build_project_graph_async` was a separate
    /// implementation with the same last-writer-wins `HashMap`, and it is the
    /// one MCP `reparse_on_edit` reaches. A guard that only exercised the
    /// filtered builder would leave that one unprotected, so the contract is
    /// stated once and checked at both entry points.
    #[tokio::test]
    async fn h42_async_graph_invents_no_edge_against_a_homonym() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();
        for (file, body) in [
            ("alpha.rs", "pub fn helper() -> i32 { 1 }"),
            ("beta.rs", "pub fn helper() -> i32 { 2 }"),
            ("main.rs", "pub fn caller() -> i32 { helper() }"),
        ] {
            std::fs::write(root.join(file), body).unwrap();
        }

        let service = AnalysisService::new();
        service
            .build_project_graph_async(root)
            .await
            .expect("async graph should build");

        let graph = service.graph_cache().get();
        let helper_edges = graph
            .all_dependencies()
            .filter(|(_, target, _)| {
                graph
                    .get_symbol(target)
                    .map(|s| s.name().eq_ignore_ascii_case("helper"))
                    .unwrap_or(false)
            })
            .count();

        assert_eq!(
            helper_edges, 0,
            "two homonyms with no rule to disambiguate must produce no edge, \
             got {helper_edges}"
        );
    }

    // =========================================================================
    // P4.4 - build_project_graph_async tests
    // =========================================================================

    #[tokio::test]
    async fn test_build_project_graph_async_completes_without_blocking() {
        use tempfile::TempDir;
        use tokio::time::{Duration, timeout};

        // Create a temp directory with Python files
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        // Create a Python file
        let file_path = temp_path.join("module.py");
        std::fs::write(
            &file_path,
            r#"
    def a():
    b()

    def b():
    pass
    "#,
        )
        .unwrap();

        let service = AnalysisService::new();

        // Spawn the async build and verify it completes within 5 seconds
        // (If it blocked the tokio runtime, this would hang or take much longer)
        let result = timeout(
            Duration::from_secs(5),
            service.build_project_graph_async(temp_path),
        )
        .await;

        assert!(
            result.is_ok(),
            "build_project_graph_async should complete within timeout"
        );
        assert!(
            result.unwrap().is_ok(),
            "build_project_graph_async should succeed"
        );

        // Verify the graph was actually built
        let graph = service.get_project_graph();
        assert!(
            graph.symbol_count() >= 2,
            "Graph should have at least 2 symbols (a, b)"
        );
    }

    #[tokio::test]
    async fn test_build_project_graph_async_returns_handle_for_await() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        // Create a simple Python file
        let file_path = temp_path.join("test.py");
        std::fs::write(&file_path, "def hello(): pass\n").unwrap();

        let service = AnalysisService::new();

        // The method should be awaitable directly
        let result = service.build_project_graph_async(temp_path).await;
        assert!(
            result.is_ok(),
            "build_project_graph_async should be awaitable"
        );
    }

    // =========================================================================
    // P3.3 - build_project_graph_filtered tests
    // =========================================================================

    #[test]
    fn test_build_project_graph_filtered_single_dir() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        // Create module_a with a file
        let module_a = temp_path.join("module_a");
        std::fs::create_dir(&module_a).unwrap();
        let file_a = module_a.join("mod.rs");
        std::fs::write(&file_a, "pub fn func_a() {}").unwrap();

        // Create module_b with a file (should be excluded)
        let module_b = temp_path.join("module_b");
        std::fs::create_dir(&module_b).unwrap();
        let file_b = module_b.join("mod.rs");
        std::fs::write(&file_b, "pub fn func_b() {}").unwrap();

        let service = AnalysisService::new();

        // Build filtered graph for only module_a
        let include_dirs = vec![temp_path.join("module_a")];
        let include_dir_refs: Vec<&Path> = include_dirs.iter().map(|p| p.as_path()).collect();
        let report = service
            .build_project_graph_filtered(temp_path, &include_dir_refs)
            .unwrap();

        // The graph should have symbols from module_a
        let symbols: Vec<_> = report.graph.symbols().collect();
        assert!(
            !symbols.is_empty(),
            "Filtered graph should have symbols from module_a"
        );

        // All symbols should be from module_a
        for symbol in &symbols {
            let file = symbol.location().file();
            assert!(
                file.contains("module_a"),
                "Symbol {} should be from module_a, got: {}",
                symbol.name(),
                file
            );
        }
    }

    #[test]
    fn test_build_project_graph_filtered_nonexistent_dir() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        // Create a file but no nonexistent_dir
        let file = temp_path.join("mod.rs");
        std::fs::write(&file, "pub fn func() {}").unwrap();

        let service = AnalysisService::new();

        // Build filtered graph for a nonexistent directory should return empty graph
        let include_dirs = vec![temp_path.join("nonexistent_dir")];
        let include_dir_refs: Vec<&Path> = include_dirs.iter().map(|p| p.as_path()).collect();
        let report = service
            .build_project_graph_filtered(temp_path, &include_dir_refs)
            .unwrap();

        // Should return empty graph (no symbols)
        assert_eq!(
            report.graph.symbol_count(),
            0,
            "Filtered graph for nonexistent dir should be empty"
        );
    }

    #[test]
    fn test_build_project_graph_filtered_multiple_dirs() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        // Create module_a
        let module_a = temp_path.join("module_a");
        std::fs::create_dir(&module_a).unwrap();
        let file_a = module_a.join("mod.rs");
        std::fs::write(&file_a, "pub fn func_a() {}").unwrap();

        // Create module_b
        let module_b = temp_path.join("module_b");
        std::fs::create_dir(&module_b).unwrap();
        let file_b = module_b.join("mod.rs");
        std::fs::write(&file_b, "pub fn func_b() {}").unwrap();

        // Create module_c (should be excluded)
        let module_c = temp_path.join("module_c");
        std::fs::create_dir(&module_c).unwrap();
        let file_c = module_c.join("mod.rs");
        std::fs::write(&file_c, "pub fn func_c() {}").unwrap();

        let service = AnalysisService::new();

        // Build filtered graph for module_a and module_b (exclude module_c)
        let include_dirs = vec![temp_path.join("module_a"), temp_path.join("module_b")];
        let include_dir_refs: Vec<&Path> = include_dirs.iter().map(|p| p.as_path()).collect();
        let report = service
            .build_project_graph_filtered(temp_path, &include_dir_refs)
            .unwrap();

        // The graph should have symbols from module_a and module_b
        let symbols: Vec<_> = report.graph.symbols().collect();
        assert!(
            !symbols.is_empty(),
            "Filtered graph should have symbols from module_a and module_b"
        );

        // No symbols should be from module_c
        for symbol in &symbols {
            let file = symbol.location().file();
            assert!(
                !file.contains("module_c"),
                "Symbol {} should NOT be from module_c, got: {}",
                symbol.name(),
                file
            );
        }
    }

    #[test]
    fn test_build_project_graph_filtered_does_not_modify_cached_graph() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        // Create a file in src
        let src = temp_path.join("src");
        std::fs::create_dir(&src).unwrap();
        let file = src.join("mod.rs");
        std::fs::write(&file, "pub fn func() {}").unwrap();

        let service = AnalysisService::new();

        // Build full graph first
        service.build_project_graph(temp_path).unwrap();
        let cached_count = service.get_project_graph().symbol_count();

        // Build filtered graph for a subdirectory
        let include_dirs = vec![temp_path.join("src")];
        let include_dir_refs: Vec<&Path> = include_dirs.iter().map(|p| p.as_path()).collect();
        let _filtered_graph = service
            .build_project_graph_filtered(temp_path, &include_dir_refs)
            .unwrap();

        // The cached graph should be unchanged
        assert_eq!(
            service.get_project_graph().symbol_count(),
            cached_count,
            "Cached graph should not be modified by build_project_graph_filtered"
        );
    }

    /// F2.W7 — RED → GREEN tests.
    ///
    /// The bug that motivated F2.W7: the legacy resolver inside
    /// `AnalysisService::build_project_graph` (the path used by the
    /// real `cognicode-mcp` binary) used a flat `name_lower → SymbolId`
    /// map with a lexicographic-FQN tie-break. F2.W5 fixed the same
    /// bug in `FullGraphStrategy` / `PerFileStrategy`, but those
    /// strategies are NOT what the binary invokes. The binary goes
    /// through `analysis_service::build_project_graph` and so the
    /// bug was still live for the end user.
    ///
    /// The corpus `crates/cognicode-core/testdata/cross_file_scope_aware/`
    /// exercises five resolution rules on a small Rust crate; see
    /// `cross_file_scope_aware/CORPUS.md` for the per-call expected
    /// destination.
    ///
    /// **FQN convention**: in CogniCode, `Symbol::fully_qualified_name()`
    /// is the string `<file_path>:<symbol_name>:<line>`. So the
    /// `caller_in_lib` symbol registered by the parser has FQN
    /// `<corpus>/src/lib.rs:caller_in_lib:5`, not just `caller_in_lib`.
    /// The tests assert on the trailing segments to stay readable.
    mod w7_scope_aware_resolution_tests {
        use crate::application::services::analysis_service::*;
        use crate::domain::aggregates::call_graph::SymbolId;

        fn corpus_path() -> std::path::PathBuf {
            let manifest = std::env::var("CARGO_MANIFEST_DIR")
                .expect("CARGO_MANIFEST_DIR must be set during cargo test");
            let crate_root = std::path::PathBuf::from(manifest);
            let workspace_root = crate_root
                .ancestors()
                .nth(2)
                .expect("workspace root has at least 2 ancestors")
                .to_path_buf();
            workspace_root.join("crates/cognicode-core/testdata/cross_file_scope_aware")
        }

        /// Build the project graph and return the FQN of the unique
        /// outgoing target whose name == `callee_leaf`. Returns `None`
        /// when no such target exists — the resolver dropped the edge
        /// honestly, which is the right behaviour per D33.
        fn unique_target_fqn_for(callee_leaf: &str) -> Option<String> {
            let service = AnalysisService::new();
            let path = corpus_path();
            service
                .build_project_graph(&path)
                .expect("build_project_graph should succeed on the fixture");
            let graph = service.get_project_graph();
            let caller_id = graph
                .symbols()
                .find(|s| s.name() == "caller_in_lib")
                .map(|s| SymbolId::new(s.fully_qualified_name()))
                .expect("caller_in_lib must exist in the fixture graph");
            let mut targets: Vec<String> = graph
                .dependencies(&caller_id)
                .filter_map(|(tid, _)| {
                    let sym = graph.get_symbol(tid)?;
                    if sym.name() == callee_leaf {
                        Some(sym.fully_qualified_name().to_string())
                    } else {
                        None
                    }
                })
                .collect();
            targets.sort();
            targets.dedup();
            targets.first().cloned()
        }

        fn outgoing_target_names_for_caller_in_lib() -> Vec<String> {
            let service = AnalysisService::new();
            let path = corpus_path();
            service
                .build_project_graph(&path)
                .expect("build_project_graph should succeed on the fixture");
            let graph = service.get_project_graph();
            let caller_id = graph
                .symbols()
                .find(|s| s.name() == "caller_in_lib")
                .map(|s| SymbolId::new(s.fully_qualified_name()))
                .expect("caller_in_lib must exist in the fixture graph");
            graph
                .dependencies(&caller_id)
                .filter_map(|(tid, _)| graph.get_symbol(tid).map(|s| s.name().to_string()))
                .collect()
        }

        #[test]
        fn w7_single_candidate_cross_file_resolves_to_nested_callee() {
            // Rule 1: 1 candidate globally → use it.
            let target = unique_target_fqn_for("callee_in_nested");
            let fqn = target.expect("callee_in_nested should resolve");
            assert!(
                fqn.contains("nested/mod.rs:callee_in_nested"),
                "expected target to live in nested/mod.rs, got {:?}",
                fqn
            );
        }

        #[test]
        fn w7_homonym_in_callers_file_resolves_locally() {
            // Rule 2: multiple candidates, one lives in caller's file
            // → use the local one.
            let target = unique_target_fqn_for("local_helper");
            let fqn = target.expect("local_helper should resolve to the local one");
            assert!(
                fqn.contains("src/lib.rs:local_helper:"),
                "expected caller-in-file rule to pick local lib.rs::local_helper, got {:?}",
                fqn
            );
        }

        #[test]
        fn w7_single_candidate_cross_file_resolves_to_ambig_compute() {
            // Rule 3: 1 candidate globally → use it.
            let target = unique_target_fqn_for("compute");
            let fqn = target.expect("compute should resolve");
            assert!(
                fqn.contains("ambig/mod.rs:compute"),
                "expected target to live in ambig/mod.rs, got {:?}",
                fqn
            );
        }

        #[test]
        fn w7_homonym_three_way_resolves_to_callers_file_local() {
            // Rule 4: the case the old FQN-lexicographic rule got
            // wrong (it picked `ambig::shared_name`).
            let target = unique_target_fqn_for("shared_name");
            let fqn = target.expect("shared_name should resolve to the local one");
            assert!(
                fqn.contains("src/lib.rs:shared_name:"),
                "expected caller-in-file rule to pick local lib.rs::shared_name, got {:?}",
                fqn
            );
        }

        #[test]
        fn w7_two_way_homonym_no_caller_file_honest_drop() {
            // Rule: caller is in src/lib.rs; callee name lives in
            // BOTH src/nested/mod.rs and src/ambig/mod.rs. Neither is
            // the caller's file, so the "same file" rule does not
            // apply. Both candidates share the crate root, so the
            // "same crate root" rule applies — but with 2 candidates
            // it is genuinely ambiguous, so per D33 the resolver
            // MUST drop the edge rather than picking one.
            let target = unique_target_fqn_for("two_way_ambig");
            assert!(
                target.is_none(),
                "expected two_way_ambig to be dropped (genuine ambiguity, no caller-file anchor), got {:?}",
                target
            );
        }

        #[test]
        fn w7_no_invented_edges_outside_crate_root() {
            // Sanity: caller_in_lib has exactly 4 outgoing edges
            // after F2.W7 — one per resolved call site
            // (two_way_ambig is dropped per D33).
            let names = outgoing_target_names_for_caller_in_lib();
            assert_eq!(
                names.len(),
                4,
                "expected exactly 4 outgoing edges from caller_in_lib (5 call sites minus the 1 ambiguous drop), got {:?}",
                names
            );
            for n in &names {
                assert!(
                    matches!(
                        n.as_str(),
                        "callee_in_nested" | "local_helper" | "compute" | "shared_name"
                    ),
                    "unexpected outgoing target: {}",
                    n
                );
            }
        }
    }

    /// PRF-ANA-07 — corpus with massive homonym collisions.
    ///
    /// `cross_file_scope_aware/` (F2.W7) covered the 1-3 homonym case.
    /// PRF-ANA-07 attacks the next tier: a project with **51 symbols
    /// sharing the short name `init`** (50 sibling files + 1 local).
    /// The visibility rule must still pick the local even when the
    /// global candidate count is large.
    mod prf_ana_07_massive_collision_tests {
        use crate::application::services::analysis_service::*;
        use crate::domain::aggregates::call_graph::SymbolId;

        fn corpus_path() -> std::path::PathBuf {
            let manifest = std::env::var("CARGO_MANIFEST_DIR")
                .expect("CARGO_MANIFEST_DIR must be set during cargo test");
            let crate_root = std::path::PathBuf::from(manifest);
            let workspace_root = crate_root
                .ancestors()
                .nth(2)
                .expect("workspace root has at least 2 ancestors")
                .to_path_buf();
            workspace_root.join("crates/cognicode-core/testdata/massive_collision_corpus")
        }

        /// Build the project graph and return the FQN of every outgoing
        /// target whose name == `callee_leaf` from `caller_in_lib`. The
        /// set is sorted and deduped to make assertions order-stable.
        fn outgoing_targets_for_caller(callee_leaf: &str) -> Vec<String> {
            let service = AnalysisService::new();
            let path = corpus_path();
            service
                .build_project_graph(&path)
                .expect("build_project_graph should succeed on the fixture");
            let graph = service.get_project_graph();
            let caller_id = graph
                .symbols()
                .find(|s| s.name() == "caller_in_lib")
                .map(|s| SymbolId::new(s.fully_qualified_name()))
                .expect("caller_in_lib must exist in the fixture graph");
            let mut targets: Vec<String> = graph
                .dependencies(&caller_id)
                .filter_map(|(tid, _)| {
                    let sym = graph.get_symbol(tid)?;
                    if sym.name() == callee_leaf {
                        Some(sym.fully_qualified_name().to_string())
                    } else {
                        None
                    }
                })
                .collect();
            targets.sort();
            targets.dedup();
            targets
        }

        #[test]
        fn mass_collision_same_name_picks_local() {
            // Visibility rule under stress: 51 candidates globally,
            // exactly one local. The resolver must NOT pick any of the
            // 50 sibling modules; it must pick the local.
            //
            // If a future regression reintroduces "first inserted" or
            // "lex-FQN-min" as the tie-break, this test fails because
            // the chosen target's FQN will not contain "lib.rs".
            let targets = outgoing_targets_for_caller("init");
            assert_eq!(
                targets.len(),
                1,
                "expected exactly 1 outgoing 'init' target from caller_in_lib, got {:?}",
                targets
            );
            let fqn = &targets[0];
            assert!(
                fqn.contains("lib.rs:init:"),
                "expected the chosen init to live in lib.rs (visibility anchor), got {:?}",
                fqn
            );
            // Sanity: NONE of the targets is one of the sibling modules.
            for sib in &["d1::", "d7::", "d25::", "d50::"] {
                assert!(
                    !fqn.contains(sib),
                    "resolved to a sibling homonym ({}) instead of local: {:?}",
                    sib,
                    fqn
                );
            }
        }

        #[test]
        fn single_candidate_cross_file_resolves_to_unique_sibling() {
            // Control: when exactly one file declares `compute`, the
            // single-candidate rule picks it, even in a corpus with
            // 51 other symbols in the same crate. This exercises
            // index construction at scale — a regression in
            // `GlobalSymbolIndex::resolve` that mishandled the
            // 1-candidate path would surface here.
            let targets = outgoing_targets_for_caller("compute");
            assert_eq!(
                targets.len(),
                1,
                "expected exactly 1 outgoing 'compute' target from caller_in_lib, got {:?}",
                targets
            );
            let fqn = &targets[0];
            assert!(
                fqn.contains("sibling_unique_compute.rs:compute:"),
                "expected compute to live in sibling_unique_compute.rs, got {:?}",
                fqn
            );
        }

        #[test]
        fn index_size_matches_corpus() {
            // Sanity: with 50 sibling `init` + 1 local + the caller +
            // `sibling_unique_compute::compute`, the global symbol index
            // must hold at least 51 entries (51 `init`s) and total
            // count > 50. This pinea that index construction is not
            // de-duplicating aggressively (which would be wrong:
            // each `init` is a distinct SymbolId in a distinct file).
            //
            // FQN format is `"{file}:{name}:{line}"` (per
            // `SymbolFqn::from_legacy_side`), so the d1 sibling's
            // `init` is `d1.rs:init:0`, not `d1::init`.
            let service = AnalysisService::new();
            let path = corpus_path();
            service
                .build_project_graph(&path)
                .expect("build_project_graph should succeed on the fixture");
            let graph = service.get_project_graph();
            let init_targets: Vec<String> = graph
                .symbols()
                .filter(|s| s.name() == "init")
                .map(|s| s.fully_qualified_name().to_string())
                .collect();
            assert!(
                init_targets.len() >= 51,
                "expected >= 51 'init' symbols (1 local + 50 sibling), got {}: {:?}",
                init_targets.len(),
                init_targets
            );
            // The local `init` is present. Its FQN is `src/lib.rs:init:0`.
            assert!(
                init_targets.iter().any(|f| f.contains("lib.rs:init:")),
                "expected a local 'init' (lib.rs) in the symbol list, got {:?}",
                init_targets
            );
            // The sibling `init`s are present (spot check d1, d25, d50).
            for sib in &["d1.rs:init:", "d25.rs:init:", "d50.rs:init:"] {
                assert!(
                    init_targets.iter().any(|f| f.contains(sib)),
                    "expected sibling homonym {} to be present in symbol list, got {:?}",
                    sib,
                    init_targets
                );
            }
        }
    }

    /// F2.W8 — RED tests for silent errors in `build_project_graph`.
    ///
    /// The bug F2.W8 addresses: `AnalysisService::build_project_graph`
    /// (the path the real `cognicode-mcp` binary uses) silently drops
    /// files that fail to read, fail to initialize the parser, or fail
    /// during AST extraction. The only externally observable signal of
    /// the omission today is `GraphCoverageMetrics::coverage_percent`,
    /// which is a derived ratio and gives the user no list of which
    /// files were dropped or why.
    ///
    /// F2.W2 introduced `BuildReport` / `SkippedFile` / `SkipReason`
    /// for `PerFileStrategy::merge_with_report` — but that API lives
    /// in `infrastructure/graph/` and is NOT used by `analysis_service`.
    /// The corpus `crates/cognicode-core/testdata/silent_errors_corpus/` is created
    /// alongside this test so the failure modes are reproducible.
    mod w8_silent_errors_tests {
        use crate::application::dto::GraphCoverageMetrics;
        use crate::application::services::analysis_service::*;

        fn corpus_path() -> std::path::PathBuf {
            let manifest = std::env::var("CARGO_MANIFEST_DIR")
                .expect("CARGO_MANIFEST_DIR must be set during cargo test");
            let crate_root = std::path::PathBuf::from(manifest);
            let workspace_root = crate_root
                .ancestors()
                .nth(2)
                .expect("workspace root has at least 2 ancestors")
                .to_path_buf();
            workspace_root.join("crates/cognicode-core/testdata/silent_errors_corpus")
        }

        fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
            std::fs::create_dir_all(dst)?;
            for entry in std::fs::read_dir(src)? {
                let entry = entry?;
                let from = entry.path();
                let to = dst.join(entry.file_name());
                let file_type = entry.file_type()?;
                if file_type.is_dir() {
                    copy_dir_recursive(&from, &to)?;
                } else if file_type.is_file() {
                    std::fs::copy(&from, &to)?;
                }
            }
            Ok(())
        }

        /// Build the corpus into a private temp directory, mutate
        /// permissions / contents so that one file becomes unreadable
        /// and another becomes UTF-8-invalid, then build the graph
        /// and return (coverage_metrics, parsed_symbol_names).
        fn build_graph_with_corruption(
            unreadable_filename: &str,
            invalid_utf8_filename: &str,
        ) -> (GraphCoverageMetrics, Vec<String>) {
            use std::os::unix::fs::PermissionsExt;

            let src_corpus = corpus_path();
            let tmp = tempfile::tempdir().expect("tempdir");
            let dst = tmp.path().join("corpus");
            copy_dir_recursive(&src_corpus, &dst).expect("copy fixture");

            let unreadable = dst.join("src").join(unreadable_filename);
            std::fs::set_permissions(&unreadable, std::fs::Permissions::from_mode(0o000))
                .expect("set_permissions 000");

            let invalid = dst.join("src").join(invalid_utf8_filename);
            std::fs::write(&invalid, [0xFFu8, 0xFE, 0xFD, 0xFC]).expect("write invalid bytes");

            let service = AnalysisService::new();
            service
                .build_project_graph(&dst)
                .expect("build_project_graph should return Ok even with skipped files");

            let coverage = service
                .get_coverage_metrics()
                .expect("coverage metrics should be populated after a successful build");
            let graph = service.get_project_graph();
            let mut names: Vec<String> = graph.symbols().map(|s| s.name().to_string()).collect();
            names.sort();
            names.dedup();
            (coverage, names)
        }

        #[test]
        fn w8_unreadable_file_is_silently_dropped() {
            let (coverage, names) = build_graph_with_corruption("unreadable.rs", "invalid_utf8.rs");
            assert!(
                coverage.parsed_files >= 1,
                "expected at least 1 file parsed (ok.rs), got {} (coverage={}%)",
                coverage.parsed_files,
                coverage.coverage_percent
            );
            // The corpus contains 3 .rs files. After corruption 2
            // should be dropped, leaving exactly 1 parsed (ok.rs).
            assert_eq!(
                coverage.parsed_files, 1,
                "expected exactly 1 file parsed after corruption, got {} (coverage={}%)",
                coverage.parsed_files, coverage.coverage_percent
            );
            assert!(
                names.contains(&"normal_function".to_string()),
                "expected ok.rs::normal_function in the graph"
            );
        }

        #[test]
        fn w8_invalid_utf8_file_is_silently_dropped() {
            let (coverage, _names) =
                build_graph_with_corruption("unreadable.rs", "invalid_utf8.rs");
            assert!(
                coverage.coverage_percent < 100.0,
                "expected coverage below 100% when 2 of 3 source files are corrupted, got {}",
                coverage.coverage_percent
            );
        }

        #[test]
        fn w8_build_report_enumerates_skipped_files() {
            // Today `AnalysisService` exposes no way to obtain the
            // list of skipped files + reasons. After F2.W8 it must.
            // This test pins the public surface. The RED is that
            // this won't compile until we add `get_last_build_report`.
            let service = AnalysisService::new();
            let dst = corpus_path();
            service
                .build_project_graph(&dst)
                .expect("build_project_graph should succeed on unmodified corpus");
            let report = service
                .get_last_build_report()
                .expect("get_last_build_report must return Some after a build");
            use crate::infrastructure::graph::per_file_graph::BuildStatus;
            match report.status {
                BuildStatus::Complete => {
                    // ok.rs is the only source file — every build
                    // should land on Complete.
                }
                BuildStatus::Partial { skipped, .. } => {
                    panic!(
                        "expected Complete status with 1 valid source file, got Partial with {} skipped",
                        skipped.len()
                    );
                }
            }
        }
    }

    /// F2.W9 — mtime preserved: a file whose BYTES change but whose
    /// mtime stays the same must NOT be served stale from file_cache.
    mod w9_mtime_tests {
        use crate::application::services::analysis_service::*;

        fn corpus_path() -> std::path::PathBuf {
            let manifest = std::env::var("CARGO_MANIFEST_DIR")
                .expect("CARGO_MANIFEST_DIR must be set during cargo test");
            let crate_root = std::path::PathBuf::from(manifest);
            let workspace_root = crate_root
                .ancestors()
                .nth(2)
                .expect("workspace root has at least 2 ancestors")
                .to_path_buf();
            workspace_root.join("crates/cognicode-core/testdata/silent_errors_corpus")
        }

        fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
            std::fs::create_dir_all(dst)?;
            for entry in std::fs::read_dir(src)? {
                let entry = entry?;
                let from = entry.path();
                let to = dst.join(entry.file_name());
                let file_type = entry.file_type()?;
                if file_type.is_dir() {
                    copy_dir_recursive(&from, &to)?;
                } else if file_type.is_file() {
                    std::fs::copy(&from, &to)?;
                }
            }
            Ok(())
        }

        #[test]
        fn w9_content_change_with_preserved_mtime_invalidates_cache() {
            let src_corpus = corpus_path();
            let tmp = tempfile::tempdir().expect("tempdir");
            let dst = tmp.path().join("corpus");
            copy_dir_recursive(&src_corpus, &dst).expect("copy fixture");

            let target = dst.join("src").join("ok.rs");
            let original_mtime = std::fs::metadata(&target)
                .expect("stat ok.rs")
                .modified()
                .expect("mtime");

            let service = AnalysisService::new();

            // Build #1: parse the pristine corpus. normal_function
            // returns 42.
            service.build_project_graph(&dst).expect("first build");
            let graph1 = service.get_project_graph();
            assert!(
                graph1.symbols().any(|s| s.name() == "normal_function"),
                "build #1 must contain normal_function"
            );

            // Mutate bytes but RESTORE the original mtime. Many
            // editors and refactoring tools write this way.
            std::fs::write(&target, "pub fn renamed_function() -> i32 { 43 }\n")
                .expect("rewrite ok.rs");
            let f = std::fs::File::options()
                .write(true)
                .open(&target)
                .expect("open for mtime restore");
            f.set_modified(original_mtime).expect("set_modified");
            drop(f);

            // Sanity: mtime really is preserved.
            let restored_mtime = std::fs::metadata(&target)
                .expect("stat ok.rs after rewrite")
                .modified()
                .expect("mtime");
            assert_eq!(
                original_mtime, restored_mtime,
                "test setup must preserve mtime"
            );

            // Build #2: the cache must NOT serve the stale entry —
            // content changed, so renamed_function must appear.
            service.build_project_graph(&dst).expect("second build");
            let graph2 = service.get_project_graph();
            assert!(
                graph2.symbols().any(|s| s.name() == "renamed_function"),
                "build #2 must see renamed_function: content changed even \
                 though mtime was preserved; stale cache entry was served"
            );
            assert!(
                !graph2.symbols().any(|s| s.name() == "normal_function"),
                "build #2 must NOT contain normal_function (it was renamed)"
            );
        }
    }

    /// PRF audit H-01 (operator 2026-09-22) — cache invalidation by content,
    /// not only mtime + size. This test pins the requirement that
    /// file_cache must invalidate when **bytes change but BOTH mtime and
    /// size are preserved**. The existing F2.W9 test only covers the
    /// case where mtime is preserved; if the editor also rewrites the
    /// file at the same size (a common refactor pattern: rename a symbol
    /// in place), the cache will return stale symbols.
    ///
    /// RED today: cache key is `(mtime, size)` (line 705-706), so any
    /// edit at same mtime + same size returns the cached entry. The
    /// test below asserts that the cache does NOT serve stale symbols
    /// in that scenario; it will fail until the cache key incorporates
    /// a content-derived signal (e.g. SHA-256 of bytes) — but the choice
    /// of signal is the operator's decision (see RELEASE-CANDIDATE §3
    /// H-01 and PRF-ANA-03 in RECONCILIATION-MATRIX).
    mod h01_cache_content_hash_tests {
        use crate::application::services::analysis_service::*;

        fn corpus_path() -> std::path::PathBuf {
            let manifest = std::env::var("CARGO_MANIFEST_DIR")
                .expect("CARGO_MANIFEST_DIR must be set during cargo test");
            let crate_root = std::path::PathBuf::from(manifest);
            let workspace_root = crate_root
                .ancestors()
                .nth(2)
                .expect("workspace root has at least 2 ancestors")
                .to_path_buf();
            workspace_root.join("crates/cognicode-core/testdata/silent_errors_corpus")
        }

        fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
            std::fs::create_dir_all(dst)?;
            for entry in std::fs::read_dir(src)? {
                let entry = entry?;
                let from = entry.path();
                let to = dst.join(entry.file_name());
                let file_type = entry.file_type()?;
                if file_type.is_dir() {
                    copy_dir_recursive(&from, &to)?;
                } else if file_type.is_file() {
                    std::fs::copy(&from, &to)?;
                }
            }
            Ok(())
        }

        #[test]
        fn h01_byte_change_with_same_mtime_and_same_size_must_invalidate_cache() {
            let src_corpus = corpus_path();
            let tmp = tempfile::tempdir().expect("tempdir");
            let dst = tmp.path().join("corpus");
            copy_dir_recursive(&src_corpus, &dst).expect("copy fixture");

            let target = dst.join("src").join("ok.rs");
            let original_mtime = std::fs::metadata(&target)
                .expect("stat ok.rs")
                .modified()
                .expect("mtime");
            let original_size = std::fs::metadata(&target).expect("stat ok.rs").len();

            let service = AnalysisService::new();

            // Build #1: pristine corpus. normal_function is the only
            // function in ok.rs.
            service.build_project_graph(&dst).expect("first build");
            let graph1 = service.get_project_graph();
            assert!(
                graph1.symbols().any(|s| s.name() == "normal_function"),
                "build #1 must contain normal_function"
            );
            assert!(
                !graph1.symbols().any(|s| s.name() == "renamedfunction"),
                "build #1 must NOT contain renamedfunction yet"
            );

            // Mutate bytes AND preserve BOTH mtime AND size. The new
            // function name `renamedfunction` has the same length (15
            // chars) as `normal_function`, and the indentation/spacing
            // is preserved byte-for-byte so the file size is identical.
            std::fs::write(
                &target,
                "// ok.rs — file that parses successfully.\npub fn renamedfunction() -> i32 {\n    42\n}\n",
            )
            .expect("rewrite ok.rs at same size");
            // Restore mtime so cache invalidation cannot rely on mtime
            // changing.
            let f = std::fs::File::options()
                .write(true)
                .open(&target)
                .expect("open for mtime restore");
            f.set_modified(original_mtime).expect("set_modified");
            drop(f);

            // Sanity: BOTH mtime AND size really are preserved.
            let restored = std::fs::metadata(&target)
                .expect("stat ok.rs after rewrite")
                .modified()
                .expect("mtime");
            let restored_size = std::fs::metadata(&target)
                .expect("stat ok.rs after rewrite")
                .len();
            assert_eq!(original_mtime, restored, "test setup must preserve mtime");
            assert_eq!(
                original_size, restored_size,
                "test setup must preserve size (got {} vs original {})",
                restored_size, original_size
            );

            // Build #2: the cache must NOT serve the stale entry.
            // Content changed (renamed_function vs normal_function)
            // even though mtime AND size are both preserved.
            service.build_project_graph(&dst).expect("second build");
            let graph2 = service.get_project_graph();
            assert!(
                graph2.symbols().any(|s| s.name() == "renamedfunction"),
                "build #2 must see renamedfunction: content changed even \
                 though mtime AND size were preserved; stale cache entry \
                 was served. H-01 requires invalidation by content."
            );
            assert!(
                !graph2.symbols().any(|s| s.name() == "normal_function"),
                "build #2 must NOT contain normal_function (it was renamed \
                 at the same size and same mtime)"
            );
        }
    }

    // ============================================================================
    // U15 — cobertura honesta en corpus mixto.
    //
    // MEDIDO 2026-10-19. Esta prueba fijaba la regla ORIGINAL de U15
    // (2026-09-23): todo archivo sin parser cuenta como hueco, asi que
    // un `.txt` o un `.cob` obligaban a `Partial`. K0/H1.1 (2026-10-18,
    // 25 dias posterior, y la decision vigente en ROADMAP §11) la
    // sustituyo por una distincion de tres ramas: un lenguaje fuente
    // conocido-sin-soporte es un hueco; un asset es una DECISION.
    //
    // La preocupacion de U15 —"no digas Complete mientras descartas cosas
    // en silencio"— sigue siendo valida y sigue fijada aqui. Lo que
    // cambia es que se responde con una exclusion con nombre en vez de
    // con un hueco inflado. MEDIDO antes del arreglo: un repo Rust
    // normal (Cargo.toml + README.md, cero fuentes sin soporte) salia
    // `Partial skipped=2` por dos assets, y como `mod.rs:1312` borra el
    // snapshot durable cuando el build no es Complete, ese repo nunca
    // persistia nada.
    //
    // No se borra U15: se reemplaza por la version vigente de su
    // preocupacion, que es mas barata (una asercion) y mas discriminante
    // (distingue las dos clases en vez de contarlas).
    // ============================================================================
    #[test]
    fn test_u15_a_source_language_gap_is_reported_and_an_asset_is_not() {
        let temp_dir = tempfile::tempdir().unwrap();
        let temp_path = temp_dir.path();

        // 1 archivo soportado (Python)
        std::fs::write(temp_path.join("supported.py"), "def a():\n    pass\n").unwrap();

        // 1 hueco real: fuente en un lenguaje que conocemos y no hemos
        // conectado (el puente K0, hasta que K1 aterrice).
        std::fs::write(temp_path.join("Main.kt"), "class Main { fun x() = 1 }\n").unwrap();

        // 2 assets: fuera del alcance por decision, no por carencia.
        std::fs::write(
            temp_path.join("unsupported.cob"),
            "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. HELLO.\n",
        )
        .unwrap();
        std::fs::write(
            temp_path.join("notes.txt"),
            "this file has no parser at all\n",
        )
        .unwrap();

        let service = AnalysisService::new();
        service.build_project_graph(temp_path).unwrap();

        let report = service
            .get_last_build_report()
            .expect("build_project_graph debe poblar last_build_report");

        use crate::infrastructure::graph::per_file_graph::BuildStatus;
        let skipped = match &report.status {
            BuildStatus::Complete => panic!(
                "U15: hay un `.kt` sin registrar y el build se declara Complete. \
                 Eso si es soporte silencioso: un `.kt` es fuente real en un \
                 lenguaje conocido-sin-soporte y debe aparecer en skipped. \
                 BuildReport: {report:?}"
            ),
            BuildStatus::Partial { skipped, .. } => skipped
                .iter()
                .map(|s| {
                    std::path::Path::new(&s.path)
                        .file_name()
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .to_string()
                })
                .collect::<Vec<_>>(),
        };

        assert!(
            skipped.iter().any(|n| n == "Main.kt"),
            "U15: el hueco real debe aparecer en skipped. Reported: {skipped:?}"
        );
        for asset in ["unsupported.cob", "notes.txt"] {
            assert!(
                !skipped.iter().any(|n| n == asset),
                "U15: `{asset}` es un asset, no una fuente. Contarlo infla el hueco \
                 y degrada el build a Partial sin motivo. Reported: {skipped:?}"
            );
        }
    }
}
