# Tasks: Flexible Graph Construction Strategies

## Phase 1: Lightweight Index

- [ ] 1.1 Create `src/infrastructure/graph/lightweight_index.rs` with `LightweightIndex` struct and `SymbolLocation` struct
- [ ] 1.2 Implement `build_index()` method that scans files and extracts symbol definitions
- [ ] 1.3 Implement `find_symbol()` method to find locations by symbol name
- [ ] 1.4 Implement `find_in_file()` method to find all symbols defined in a file

## Phase 2: Symbol Index Service

- [ ] 2.1 Create `src/infrastructure/graph/symbol_index.rs` with `SymbolIndex` service
- [ ] 2.2 Implement `build()` method to construct index from directory
- [ ] 2.3 Implement `query()` method to find symbol locations
- [ ] 2.4 Add cache management

## Phase 3: On-Demand Graph Builder

- [ ] 3.1 Create `src/infrastructure/graph/on_demand_graph.rs` with `OnDemandGraphBuilder`
- [ ] 3.2 Implement `build_for_symbol()` to build subgraph centered on a symbol
- [ ] 3.3 Implement `build_for_path()` to build subgraph for tracing path between two symbols

## Phase 4: Per-File Graph Cache

- [ ] 4.1 Create `src/infrastructure/graph/per_file_graph.rs` with `PerFileGraphCache`
- [ ] 4.2 Implement `get_or_build()` to get graph for file, building if needed
- [ ] 4.3 Implement `merge()` to merge multiple file graphs

## Phase 5: Graph Strategy Trait and Implementations

- [ ] 5.1 Create `src/infrastructure/graph/strategy.rs` with `GraphStrategy` trait
- [ ] 5.2 Implement `LightweightStrategy`
- [ ] 5.3 Implement `OnDemandStrategy`
- [ ] 5.4 Implement `PerFileStrategy`
- [ ] 5.5 Implement `FullGraphStrategy`

## Phase 6: AnalysisService Modifications

- [ ] 6.1 Add `symbol_index()` method returning LightweightIndex
- [ ] 6.2 Add `query_call_hierarchy(symbol, depth, direction)` using on-demand approach
- [ ] 6.3 Add `build_full_graph()` method for explicit full graph build
- [ ] 6.4 Update `mod.rs` to export new modules
