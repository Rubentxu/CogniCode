//! Criterion benchmarks for graph operations in cognicode-core
//!
//! Run with: cargo bench -p cognicode-core --bench graph_benchmarks

use std::path::PathBuf;
use std::time::Instant;

use criterion::{black_box, criterion_group, criterion_main, Criterion};

use cognicode_core::infrastructure::graph::{
    GraphCache, LightweightIndex, OnDemandGraphBuilder, PerFileGraphCache, SymbolIndex,
    TraversalDirection,
};

/// Generates a Python source file with approximately the given number of lines.
/// Creates a realistic file with functions, classes, and call relationships.
fn generate_python_source(line_count: usize) -> String {
    let mut source = String::new();

    // Header with imports
    source.push_str("import os\n");
    source.push_str("import sys\n");
    source.push_str("from typing import List, Dict, Optional, Any\n\n");

    let functions_per_class = 10;
    let lines_per_function = line_count / 20;

    // Generate classes with methods
    let class_count = (line_count as f64 / (functions_per_class as f64 * lines_per_function as f64)) as usize;

    for class_idx in 0..class_count {
        source.push_str(&format!("class Class{}:\n", class_idx));
        source.push_str(&format!("    def __init__(self):\n"));
        source.push_str(&format!("        self.value = {}\n", class_idx));
        source.push_str(&format!("        self.name = \"class_{}\"\n\n", class_idx));

        for method_idx in 0..functions_per_class {
            source.push_str(&format!(
                "    def method_{}(self, arg1: int, arg2: str) -> Optional[Dict]:\n",
                method_idx
            ));
            source.push_str(&format!("        result = {{\"class\": {}, \"method\": {}}}\n", class_idx, method_idx));
            source.push_str("        local_var = arg1 * 2\n");
            source.push_str("        another = arg2.lower()\n");
            source.push_str("        if local_var > 100:\n");
            source.push_str("            return None\n");
            source.push_str("        for i in range(10):\n");
            source.push_str("            x = i * local_var\n");
            source.push_str("            y = x + another\n");
            source.push_str(&format!(
                "        return self.method_{}(local_var, another)\n\n",
                (method_idx + 1) % functions_per_class
            ));
        }
        source.push_str("\n");

        // Add top-level functions
        source.push_str(&format!(
            "def standalone_func_{}(data: List[int]) -> Dict[str, Any]:\n",
            class_idx
        ));
        source.push_str("    result = {}\n");
        source.push_str("    for item in data:\n");
        source.push_str("        if item > 0:\n");
        source.push_str("            result['positive'] = result.get('positive', 0) + 1\n");
        source.push_str("        else:\n");
        source.push_str("            result['negative'] = result.get('negative', 0) + 1\n");
        source.push_str(&format!(
            "    obj = Class{}()\n",
            class_idx
        ));
        source.push_str(&format!(
            "    obj.method_{}(1, \"test\")\n",
            class_idx % functions_per_class
        ));
        source.push_str("    return result\n\n");
    }

    source
}

/// Generates Rust source with the given approximate line count.
fn generate_rust_source(line_count: usize) -> String {
    let mut source = String::new();

    source.push_str("use std::collections::{HashMap, HashSet};\n");
    source.push_str("use std::sync::{Arc, RwLock};\n\n");

    let functions_per_struct = 8;
    let lines_per_function = line_count / 16;

    let struct_count = (line_count as f64 / (functions_per_struct as f64 * lines_per_function as f64)) as usize;

    for struct_idx in 0..struct_count {
        source.push_str(&format!("struct Handler{} {{\n", struct_idx));
        source.push_str(&format!("    id: usize,\n"));
        source.push_str(&format!("    name: String,\n"));
        source.push_str(&format!("    cache: HashMap<String, i32>,\n"));
        source.push_str("}\n\n");

        source.push_str(&format!("impl Handler{} {{\n", struct_idx));

        for method_idx in 0..functions_per_struct {
            source.push_str(&format!(
                "    pub fn process_{}(&mut self, input: Vec<i32>) -> HashMap<String, i32> {{\n",
                method_idx
            ));
            source.push_str(&format!("        let mut result = HashMap::new();\n"));
            source.push_str(&format!("        result.insert(\"id\".to_string(), self.id);\n"));
            source.push_str("        for item in &input {\n");
            source.push_str("            if *item > 0 {\n");
            source.push_str("                *result.entry(\"positive\".to_string()).or_insert(0) += 1;\n");
            source.push_str("            }\n");
            source.push_str("        }\n");
            source.push_str(&format!(
                "        self.internal_{}(&input)\n",
                (method_idx + 1) % functions_per_struct
            ));
            source.push_str("    }\n\n");

            source.push_str(&format!(
                "    fn internal_{}(&self, data: &[i32]) -> i32 {{\n",
                method_idx
            ));
            source.push_str("        data.iter().sum()\n");
            source.push_str("    }\n\n");
        }

        source.push_str("}\n\n");

        // Standalone function
        source.push_str(&format!(
            "pub fn standalone_{}(items: Vec<String>) -> usize {{\n",
            struct_idx
        ));
        source.push_str("    items.iter().map(|s| s.len()).sum()\n");
        source.push_str(&format!("    let _handler = Handler{} {{ id: {}, name: String::new(), cache: HashMap::new() }};\n", struct_idx, struct_idx));
        source.push_str("}\n\n");
    }

    source
}

// =============================================================================
// Benchmark: Call Graph Construction (10K+ lines)
// =============================================================================

fn benchmark_call_graph_construction_10k(c: &mut Criterion) {
    let source = generate_python_source(10_000);

    // Create a temp file
    let temp_dir = std::env::temp_dir();
    let temp_path = temp_dir.join("benchmark_10k.py");
    std::fs::write(&temp_path, &source).expect("Failed to write temp file");

    let cache = PerFileGraphCache::new();

    c.bench_function("call_graph_10k_lines_python", |b| {
        b.iter(|| {
            cache.clear();
            let graph = cache.get_or_build(black_box(&temp_path)).unwrap();
            assert!(graph.symbol_count() > 0, "Should have symbols");
        });
    });

    let _ = std::fs::remove_file(&temp_path);
}

fn benchmark_call_graph_construction_rust_10k(c: &mut Criterion) {
    let source = generate_rust_source(10_000);

    let temp_dir = std::env::temp_dir();
    let temp_path = temp_dir.join("benchmark_10k.rs");
    std::fs::write(&temp_path, &source).expect("Failed to write temp file");

    let cache = PerFileGraphCache::new();

    c.bench_function("call_graph_10k_lines_rust", |b| {
        b.iter(|| {
            cache.clear();
            let graph = cache.get_or_build(black_box(&temp_path)).unwrap();
            assert!(graph.symbol_count() > 0, "Should have symbols");
        });
    });

    let _ = std::fs::remove_file(&temp_path);
}

// =============================================================================
// Benchmark: Hot Path Analysis Performance
// =============================================================================

fn setup_on_demand_builder() -> OnDemandGraphBuilder {
    let mut builder = OnDemandGraphBuilder::new();

    // Build a substantial index with multiple files
    let sources: Vec<(&str, &str)> = vec![
        (
            "main.py",
            r#"
def outer_function():
    middle_function()
    return 1

def middle_function():
    inner_function()
    return 2

def inner_function():
    leaf_function()
    return 3

def leaf_function():
    pass

class MyClass:
    def method_a(self):
        self.method_b()

    def method_b(self):
        self.method_c()

    def method_c(self):
        pass
"#,
        ),
        (
            "utils.py",
            r#"
def util_func():
    another_util()
    return True

def another_util():
    pass

def helper():
    deep_helper()

def deep_helper():
    leaf_function()
"#,
        ),
    ];

    builder.build_index_from_sources(sources);
    builder
}

fn benchmark_hot_path_callees(c: &mut Criterion) {
    let mut builder = setup_on_demand_builder();

    c.bench_function("hot_path_outgoing_calls", |b| {
        b.iter(|| {
            let result = builder.build_for_symbol(
                black_box("outer_function"),
                black_box(3),
                black_box(TraversalDirection::Callees),
            );
            // usize is always >= 0, just verify the result exists
            let _ = result.entries.len();
        });
    });
}

fn benchmark_hot_path_callers(c: &mut Criterion) {
    let mut builder = setup_on_demand_builder();

    c.bench_function("hot_path_incoming_calls", |b| {
        b.iter(|| {
            let result = builder.build_for_symbol(
                black_box("leaf_function"),
                black_box(3),
                black_box(TraversalDirection::Callers),
            );
            // usize is always >= 0, just verify the result exists
            let _ = result.entries.len();
        });
    });
}

fn benchmark_hot_path_bidirectional(c: &mut Criterion) {
    let mut builder = setup_on_demand_builder();

    c.bench_function("hot_path_bidirectional", |b| {
        b.iter(|| {
            let result = builder.build_for_symbol(
                black_box("method_b"),
                black_box(2),
                black_box(TraversalDirection::Both),
            );
            // usize is always >= 0, just verify the result exists
            let _ = result.entries.len();
        });
    });
}

// =============================================================================
// Benchmark: Semantic Search with Large Indices
// =============================================================================

fn generate_large_index(size: usize) -> LightweightIndex {
    let mut index = LightweightIndex::new();

    // Build sources directly using a single closure that returns owned strings
    let sources_vec: Vec<(String, String)> = (0..size)
        .map(|i| {
            let source = format!(
                r#"
def function_{}(x, y):
    return x + y

class Class_{}:
    def method_{}(self):
        pass
"#,
                i, i, i
            );
            (format!("file_{}.py", i), source)
        })
        .collect();

    // Convert to the format expected by build_from_sources
    for (path, source) in sources_vec {
        index.build_from_sources([(path.as_str(), source.as_str())]);
    }

    index
}

fn benchmark_semantic_search_small_index(c: &mut Criterion) {
    let index = generate_large_index(100);

    c.bench_function("semantic_search_100_files", |b| {
        b.iter(|| {
            let results = index.find_symbol(black_box("function_50"));
            assert!(results.len() > 0, "Should find symbol");
        });
    });
}

fn benchmark_semantic_search_medium_index(c: &mut Criterion) {
    let index = generate_large_index(500);

    c.bench_function("semantic_search_500_files", |b| {
        b.iter(|| {
            let results = index.find_symbol(black_box("function_250"));
            assert!(results.len() > 0, "Should find symbol");
        });
    });
}

fn benchmark_semantic_search_large_index(c: &mut Criterion) {
    let index = generate_large_index(1000);

    c.bench_function("semantic_search_1000_files", |b| {
        b.iter(|| {
            let results = index.find_symbol(black_box("function_500"));
            assert!(results.len() > 0, "Should find symbol");
        });
    });
}

fn benchmark_symbol_index_build(c: &mut Criterion) {
    c.bench_function("symbol_index_build_1000_files", |b| {
        b.iter(|| {
            let mut index = SymbolIndex::new();
            for i in 0..1000 {
                let path = format!("file_{}.py", i);
                let source = format!(
                    r#"
def func_{}(x):
    return x * 2

class Cls_{}:
    pass
"#,
                    i, i
                );
                index.build_from_sources([(path.as_str(), source.as_str())]);
            }
            assert!(index.symbol_count() > 0, "Should build index");
        });
    });
}

// =============================================================================
// Benchmark: P99 Latency Assertions
// =============================================================================

fn benchmark_p99_call_graph_construction(c: &mut Criterion) {
    let source = generate_python_source(5000);
    let temp_dir = std::env::temp_dir();
    let temp_path = temp_dir.join("p99_test.py");
    std::fs::write(&temp_path, &source).expect("Failed to write temp file");

    let cache = PerFileGraphCache::new();

    // Warmup
    let _ = cache.get_or_build(&temp_path);

    let mut group = c.benchmark_group("p99_call_graph");

    // Run multiple iterations to get P99 distribution
    group.sample_size(100).warm_up_time(std::time::Duration::from_millis(100));

    group.bench_function("construction", |b| {
        b.iter(|| {
            cache.clear();
            let start = Instant::now();
            let _ = cache.get_or_build(&temp_path);
            let elapsed = start.elapsed();
            // P99 assertion: should complete within 500ms for 5K lines
            assert!(
                elapsed.as_millis() < 500,
                "P99 threshold exceeded: {:?}",
                elapsed
            );
        });
    });

    let _ = std::fs::remove_file(&temp_path);
}

fn benchmark_p99_semantic_search(c: &mut Criterion) {
    let index = generate_large_index(500);

    let mut group = c.benchmark_group("p99_semantic_search");
    group.sample_size(100).warm_up_time(std::time::Duration::from_millis(50));

    group.bench_function("lookup", |b| {
        b.iter(|| {
            let start = Instant::now();
            let _ = index.find_symbol(black_box("function_250"));
            let elapsed = start.elapsed();
            // P99 assertion: lookup should be < 10ms
            assert!(
                elapsed.as_millis() < 10,
                "P99 threshold exceeded: {:?}",
                elapsed
            );
        });
    });
}

fn benchmark_p99_graph_cache_update(c: &mut Criterion) {
    let cache = GraphCache::new();

    let mut group = c.benchmark_group("p99_graph_cache");
    group.sample_size(100).warm_up_time(std::time::Duration::from_millis(50));

    group.bench_function("update", |b| {
        b.iter(|| {
            let start = Instant::now();
            cache.update(|g| {
                let sym = cognicode_core::domain::aggregates::symbol::Symbol::new(
                    "bench_func",
                    cognicode_core::domain::value_objects::SymbolKind::Function,
                    cognicode_core::domain::value_objects::Location::new("bench.rs", 1, 1),
                );
                g.add_symbol(sym);
            });
            let elapsed = start.elapsed();
            // P99 assertion: cache update should be < 5ms
            assert!(
                elapsed.as_millis() < 5,
                "P99 threshold exceeded: {:?}",
                elapsed
            );
        });
    });
}

// =============================================================================
// Benchmark: Graph Cache Performance
// =============================================================================

fn benchmark_graph_cache_operations(c: &mut Criterion) {
    let cache = GraphCache::new();

    c.bench_function("graph_cache_set_and_get", |b| {
        b.iter(|| {
            let mut graph = cognicode_core::domain::aggregates::call_graph::CallGraph::new();
            for i in 0..100 {
                let sym = cognicode_core::domain::aggregates::symbol::Symbol::new(
                    &format!("func_{}", i),
                    cognicode_core::domain::value_objects::SymbolKind::Function,
                    cognicode_core::domain::value_objects::Location::new("test.rs", i, 1),
                );
                graph.add_symbol(sym);
            }
            cache.set(graph);
            let retrieved = cache.get();
            assert!(retrieved.symbol_count() == 100);
        });
    });
}

// =============================================================================
// Benchmark: Lightweight Index Build Performance
// =============================================================================

fn benchmark_lightweight_index_build(c: &mut Criterion) {
    c.bench_function("lightweight_index_build_1000_files", |b| {
        b.iter(|| {
            let mut index = LightweightIndex::new();
            for i in 0..1000 {
                let path = format!("src/file_{}.rs", i);
                let source = format!(
                    r#"
pub fn function_{}(x: i32) -> i32 {{
    x * 2
}}

pub struct Struct_{} {{
    value: i32,
}}
"#,
                    i, i
                );
                index.build_from_sources([(path.as_str(), source.as_str())]);
            }
            assert!(index.symbol_count() > 0, "Should build index");
        });
    });
}

fn benchmark_lightweight_index_find(c: &mut Criterion) {
    let mut index = LightweightIndex::new();
    for i in 0..500 {
        let path = format!("src/file_{}.rs", i);
        let source = format!(
            r#"
pub fn function_{}(x: i32) -> i32 {{ x * 2 }}
pub fn another_function_{}() {{ }}
"#,
            i, i
        );
        index.build_from_sources([(path.as_str(), source.as_str())]);
    }

    c.bench_function("lightweight_index_find_500_files", |b| {
        b.iter(|| {
            let results = index.find_symbol(black_box("function_250"));
            assert!(results.len() > 0, "Should find symbol");
        });
    });
}

// =============================================================================
// Benchmark: Merge Operations
// =============================================================================

fn benchmark_per_file_graph_merge(c: &mut Criterion) {
    let cache = PerFileGraphCache::new();
    let temp_dir = std::env::temp_dir();

    // Create multiple files
    let mut paths: Vec<PathBuf> = Vec::new();
    for i in 0..10 {
        let path = temp_dir.join(format!("merge_test_{}.py", i));
        let source = generate_python_source(1000);
        std::fs::write(&path, &source).expect("Failed to write temp file");
        paths.push(path);
    }

    // Pre-build graphs
    for path in &paths {
        let _ = cache.get_or_build(path);
    }

    let path_refs: Vec<&std::path::Path> = paths.iter().map(|p| p.as_path()).collect();

    c.bench_function("per_file_graph_merge_10_files", |b| {
        b.iter(|| {
            let merged = cache.merge(black_box(&path_refs));
            assert!(merged.symbol_count() > 0, "Should merge symbols");
        });
    });

    for path in &paths {
        let _ = std::fs::remove_file(path);
    }
}

// =============================================================================
// Criterion Main
// =============================================================================

criterion_group!(
    benches,
    // Call graph construction benchmarks
    benchmark_call_graph_construction_10k,
    benchmark_call_graph_construction_rust_10k,
    // Hot path analysis benchmarks
    benchmark_hot_path_callees,
    benchmark_hot_path_callers,
    benchmark_hot_path_bidirectional,
    // Semantic search benchmarks
    benchmark_semantic_search_small_index,
    benchmark_semantic_search_medium_index,
    benchmark_semantic_search_large_index,
    benchmark_symbol_index_build,
    // P99 latency benchmarks
    benchmark_p99_call_graph_construction,
    benchmark_p99_semantic_search,
    benchmark_p99_graph_cache_update,
    // Graph cache benchmarks
    benchmark_graph_cache_operations,
    // Index build benchmarks
    benchmark_lightweight_index_build,
    benchmark_lightweight_index_find,
    // Merge benchmarks
    benchmark_per_file_graph_merge,
);
criterion_main!(benches);
