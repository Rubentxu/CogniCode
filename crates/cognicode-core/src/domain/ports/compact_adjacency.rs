//! `CompactAdjacency` — the read-only, array-based adjacency of a projection.
//!
//! A projection answers its traversals from this value: dense node numbers
//! (`0..node_count`, the numbering of the projection), the outgoing edges of
//! every node stored contiguously (CSR) and the incoming edges stored
//! contiguously too (CSC). The per-edge facts a traversal may need — the
//! dependency type and the sanitised confidence — live in arrays parallel to
//! the outgoing edges, so the incoming side adds one `u32` per edge instead
//! of a second copy of them.
//!
//! The value is immutable once built. It is built once per projection and
//! lent to every reader (`&CompactAdjacency`); no query rebuilds it, and a
//! projection published to several readers shares it.
//!
//! # Order
//!
//! The edges of a node come out in the order the petgraph baseline walks
//! them: the most recently inserted first. The projection inserts its edges
//! sorted by source, target and dependency type, so the outgoing edges of a
//! node come out by descending target and dependency type, and the incoming
//! ones by descending source and dependency type. Keeping the order is what
//! lets an algorithm that accumulates floating point sums over an adjacency
//! (PageRank) publish the same scores through either representation.
//!
//! # Limits
//!
//! Nodes and edges are numbered with `u32`: a graph of more than
//! `u32::MAX` nodes or edges does not fit in memory on any host this runs
//! on, and [`CompactAdjacency::build`] refuses it rather than wrap.

use std::collections::{HashSet, VecDeque};
use std::hash::{BuildHasherDefault, Hasher};

use crate::domain::value_objects::DependencyType;

/// Which edges a traversal follows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EdgeSide {
    /// Follow edges from source to target (callees).
    Outgoing,
    /// Follow edges from target to source (callers).
    Incoming,
}

/// One edge as the adjacency stores it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CompactEdge {
    /// The node at the other end of the edge from the row being read.
    pub neighbour: u32,
    /// The dependency type of the edge.
    pub dependency_type: DependencyType,
    /// The sanitised confidence of the edge.
    pub confidence: f64,
}

/// Multiplicative hasher for dense `u32` node numbers: the numbers are
/// already well spread, so one multiplication is all a sparse set needs.
#[derive(Default)]
struct NodeHasher(u64);

impl Hasher for NodeHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.0 = (self.0 ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3);
        }
    }

    fn write_u32(&mut self, value: u32) {
        self.0 = u64::from(value).wrapping_mul(0x9e37_79b9_7f4a_7c15);
    }
}

/// Graphs up to this many nodes mark visited nodes in a bitset from the
/// start: the bitset is a few cache lines and a hash set would only add cost.
const DENSE_BELOW: usize = 4096;

/// A sparse set holds at most `node_count / SPARSE_RATIO` nodes before it
/// becomes a bitset, so the bitset is paid for only by a walk that has
/// already visited a fixed fraction of the graph.
const SPARSE_RATIO: usize = 64;

/// The nodes a traversal has visited.
///
/// A bounded walk on a large graph visits a few hundred nodes; allocating and
/// zeroing one flag per node of the graph for it would cost more than the
/// walk. The set starts as a hash set sized by what the walk visits and turns
/// into a bitset (one bit per node) once the walk has visited a fixed share of
/// the graph, so a walk costs in proportion to what it visits and an
/// unbounded one costs one bit per node.
#[derive(Debug)]
pub struct VisitedNodes {
    node_count: usize,
    repr: VisitedRepr,
}

#[derive(Debug)]
enum VisitedRepr {
    Sparse(HashSet<u32, BuildHasherDefault<NodeHasher>>),
    Dense(Vec<u64>),
}

impl VisitedNodes {
    /// An empty set over the nodes `0..node_count`.
    pub fn new(node_count: usize) -> Self {
        let repr = if node_count <= DENSE_BELOW {
            VisitedRepr::Dense(vec![0; node_count.div_ceil(64)])
        } else {
            VisitedRepr::Sparse(HashSet::default())
        };
        Self { node_count, repr }
    }

    /// Marks `node` as visited and says whether it was new.
    ///
    /// # Panics
    ///
    /// When `node >= node_count`.
    pub fn insert(&mut self, node: u32) -> bool {
        assert!((node as usize) < self.node_count, "node out of range");
        match &mut self.repr {
            VisitedRepr::Dense(words) => Self::set_bit(words, node),
            VisitedRepr::Sparse(set) => {
                let fresh = set.insert(node);
                if set.len() > self.node_count / SPARSE_RATIO {
                    let mut words = vec![0; self.node_count.div_ceil(64)];
                    for &seen in set.iter() {
                        Self::set_bit(&mut words, seen);
                    }
                    self.repr = VisitedRepr::Dense(words);
                }
                fresh
            }
        }
    }

    fn set_bit(words: &mut [u64], node: u32) -> bool {
        let word = &mut words[node as usize / 64];
        let bit = 1u64 << (node % 64);
        let fresh = *word & bit == 0;
        *word |= bit;
        fresh
    }
}

/// CSR/CSC adjacency over dense node numbers. See the
/// [module documentation](self) for the layout and the edge order.
#[derive(Debug, Clone, PartialEq)]
pub struct CompactAdjacency {
    node_count: usize,
    /// `out_offsets[u]..out_offsets[u + 1]` is the outgoing row of `u`.
    out_offsets: Box<[u32]>,
    out_targets: Box<[u32]>,
    out_kinds: Box<[DependencyType]>,
    out_confidence: Box<[f64]>,
    /// `in_offsets[v]..in_offsets[v + 1]` is the incoming row of `v`.
    in_offsets: Box<[u32]>,
    in_sources: Box<[u32]>,
    /// Position, in the outgoing arrays, of the edge of each incoming entry.
    in_edge: Box<[u32]>,
}

impl CompactAdjacency {
    /// Builds the adjacency of `node_count` nodes from the `edge_count`
    /// `edges`, given as `(source, target, dependency type, confidence)` in
    /// insertion order. The iterator is walked twice.
    ///
    /// Every endpoint must be below `node_count`, and `edges` must yield
    /// exactly `edge_count` items.
    ///
    /// # Panics
    ///
    /// When the nodes or the edges do not fit in a `u32`, or an endpoint is
    /// out of range: the caller is the projection, which numbers both.
    pub fn build(
        node_count: usize,
        edge_count: usize,
        edges: impl Iterator<Item = (usize, usize, DependencyType, f64)> + Clone,
    ) -> Self {
        let wide = |n: usize| u32::try_from(n).expect("a projection fits in u32 numbers");
        wide(node_count);
        wide(edge_count);

        let mut out_offsets = vec![0u32; node_count + 1];
        let mut in_offsets = vec![0u32; node_count + 1];
        for (source, target, _, _) in edges.clone() {
            out_offsets[source + 1] += 1;
            in_offsets[target + 1] += 1;
        }
        for node in 0..node_count {
            out_offsets[node + 1] += out_offsets[node];
            in_offsets[node + 1] += in_offsets[node];
        }

        // Each row is filled from its end, so reading it forward gives the
        // most recently inserted edge first, like the petgraph baseline.
        let mut out_fill: Vec<u32> = out_offsets[1..].to_vec();
        let mut in_fill: Vec<u32> = in_offsets[1..].to_vec();
        let mut out_targets = vec![0u32; edge_count];
        let mut out_kinds = vec![DependencyType::default(); edge_count];
        let mut out_confidence = vec![0f64; edge_count];
        let mut in_sources = vec![0u32; edge_count];
        let mut in_edge = vec![0u32; edge_count];
        for (source, target, kind, confidence) in edges {
            out_fill[source] -= 1;
            let slot = out_fill[source] as usize;
            out_targets[slot] = wide(target);
            out_kinds[slot] = kind;
            out_confidence[slot] = confidence;

            in_fill[target] -= 1;
            let back = in_fill[target] as usize;
            in_sources[back] = wide(source);
            in_edge[back] = slot as u32;
        }

        Self {
            node_count,
            out_offsets: out_offsets.into(),
            out_targets: out_targets.into(),
            out_kinds: out_kinds.into(),
            out_confidence: out_confidence.into(),
            in_offsets: in_offsets.into(),
            in_sources: in_sources.into(),
            in_edge: in_edge.into(),
        }
    }

    /// Number of nodes.
    pub fn node_count(&self) -> usize {
        self.node_count
    }

    /// Number of edges. Parallel edges and self loops count each.
    pub fn edge_count(&self) -> usize {
        self.out_targets.len()
    }

    /// The callees of `node`, one entry per edge, in row order.
    ///
    /// # Panics
    ///
    /// When `node >= node_count()`.
    pub fn successors(&self, node: usize) -> &[u32] {
        &self.out_targets[self.out_range(node)]
    }

    /// The callers of `node`, one entry per edge, in row order.
    ///
    /// # Panics
    ///
    /// When `node >= node_count()`.
    pub fn predecessors(&self, node: usize) -> &[u32] {
        &self.in_sources[self.in_range(node)]
    }

    /// Number of outgoing edges of `node`.
    pub fn out_degree(&self, node: usize) -> usize {
        self.out_range(node).len()
    }

    /// Number of incoming edges of `node`.
    pub fn in_degree(&self, node: usize) -> usize {
        self.in_range(node).len()
    }

    /// The edges of `node` on `side`, in row order, with their facts.
    pub fn edges(&self, node: usize, side: EdgeSide) -> impl Iterator<Item = CompactEdge> + '_ {
        let (range, incoming) = match side {
            EdgeSide::Outgoing => (self.out_range(node), false),
            EdgeSide::Incoming => (self.in_range(node), true),
        };
        range.map(move |slot| {
            let (neighbour, edge) = if incoming {
                (self.in_sources[slot], self.in_edge[slot] as usize)
            } else {
                (self.out_targets[slot], slot)
            };
            CompactEdge {
                neighbour,
                dependency_type: self.out_kinds[edge],
                confidence: self.out_confidence[edge],
            }
        })
    }

    /// The outgoing rows as owned lists, in row order (the most recently
    /// inserted edge first), the shape the algorithm crate takes. One
    /// allocation per row: use [`Self::successors`] when a borrowed row is
    /// enough.
    pub fn out_lists(&self) -> Vec<Vec<usize>> {
        (0..self.node_count)
            .map(|node| widen(self.successors(node)))
            .collect()
    }

    /// The outgoing rows as owned lists in insertion order (the reverse of
    /// row order), which is the order a plain walk over the edge list of
    /// the projection produces.
    pub fn out_lists_in_insertion_order(&self) -> Vec<Vec<usize>> {
        (0..self.node_count)
            .map(|node| {
                self.successors(node)
                    .iter()
                    .rev()
                    .map(|&n| n as usize)
                    .collect()
            })
            .collect()
    }

    /// The incoming rows as owned lists, and the out degree of every node.
    pub fn in_lists(&self) -> (Vec<Vec<usize>>, Vec<usize>) {
        let lists = (0..self.node_count)
            .map(|node| widen(self.predecessors(node)))
            .collect();
        let degrees = (0..self.node_count)
            .map(|node| self.out_degree(node))
            .collect();
        (lists, degrees)
    }

    /// The neighbours of every node whatever the direction of the edge:
    /// every caller (one entry per edge) and then, in insertion order, the
    /// callees that are not listed yet.
    pub fn undirected_lists(&self) -> Vec<Vec<usize>> {
        (0..self.node_count)
            .map(|node| {
                let mut row = widen(self.predecessors(node));
                for &callee in self.successors(node).iter().rev() {
                    let callee = callee as usize;
                    if !row.contains(&callee) {
                        row.push(callee);
                    }
                }
                row
            })
            .collect()
    }

    /// The nodes reached from `start` along `side` within `max_depth`
    /// hops, in breadth-first discovery order, `start` not included unless
    /// `revisit_start` lets a cycle bring it back.
    ///
    /// `revisit_start` reproduces the behaviour of the reverse impact
    /// radius, whose visited set does not hold the root: a root on a cycle
    /// is listed among its own callers. The forward reach marks the root as
    /// visited and never lists it. `max_depth == 0` reaches nothing.
    ///
    /// # Panics
    ///
    /// When `start >= node_count()`.
    pub fn reach(
        &self,
        start: usize,
        side: EdgeSide,
        max_depth: usize,
        revisit_start: bool,
    ) -> Vec<u32> {
        let mut found = Vec::new();
        if max_depth == 0 {
            return found;
        }
        let mut seen = VisitedNodes::new(self.node_count);
        if !revisit_start {
            seen.insert(start as u32);
        }
        let mut queue: VecDeque<(u32, usize)> = VecDeque::new();
        queue.push_back((start as u32, 0));
        while let Some((node, depth)) = queue.pop_front() {
            if depth >= max_depth {
                continue;
            }
            for &next in self.row(node as usize, side) {
                if seen.insert(next) {
                    found.push(next);
                    queue.push_back((next, depth + 1));
                }
            }
        }
        found
    }

    /// Whether a directed path leads from `from` to `to`. A node reaches
    /// itself.
    ///
    /// # Panics
    ///
    /// When either node is out of range.
    pub fn connects(&self, from: usize, to: usize) -> bool {
        if from == to {
            return true;
        }
        let mut seen = VisitedNodes::new(self.node_count);
        seen.insert(from as u32);
        let mut stack = vec![from as u32];
        while let Some(node) = stack.pop() {
            for &next in self.successors(node as usize) {
                if next as usize == to {
                    return true;
                }
                if seen.insert(next) {
                    stack.push(next);
                }
            }
        }
        false
    }

    /// Bytes the arrays occupy, for capacity accounting.
    pub fn heap_bytes(&self) -> usize {
        use std::mem::size_of;
        (self.out_offsets.len() + self.in_offsets.len()) * size_of::<u32>()
            + (self.out_targets.len() + self.in_sources.len() + self.in_edge.len())
                * size_of::<u32>()
            + self.out_kinds.len() * size_of::<DependencyType>()
            + self.out_confidence.len() * size_of::<f64>()
    }

    fn row(&self, node: usize, side: EdgeSide) -> &[u32] {
        match side {
            EdgeSide::Outgoing => self.successors(node),
            EdgeSide::Incoming => self.predecessors(node),
        }
    }

    fn out_range(&self, node: usize) -> std::ops::Range<usize> {
        self.out_offsets[node] as usize..self.out_offsets[node + 1] as usize
    }

    fn in_range(&self, node: usize) -> std::ops::Range<usize> {
        self.in_offsets[node] as usize..self.in_offsets[node + 1] as usize
    }
}

fn widen(row: &[u32]) -> Vec<usize> {
    row.iter().map(|&n| n as usize).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    use DependencyType::{Calls, Imports};

    /// 0 -> 1 (Calls), 0 -> 1 (Imports), 1 -> 2, 2 -> 1, 3 -> 3, in insertion order.
    fn sample() -> CompactAdjacency {
        let edges = [
            (0, 1, Calls, 1.0),
            (0, 1, Imports, 0.5),
            (1, 2, Calls, 0.9),
            (2, 1, Calls, 0.7),
            (3, 3, Calls, 1.0),
        ];
        CompactAdjacency::build(5, edges.len(), edges.iter().copied())
    }

    #[test]
    fn an_empty_adjacency_has_no_nodes_and_no_edges() {
        let adjacency = CompactAdjacency::build(0, 0, std::iter::empty());
        assert_eq!(adjacency.node_count(), 0);
        assert_eq!(adjacency.edge_count(), 0);
        assert!(adjacency.out_lists().is_empty());
        assert!(adjacency.undirected_lists().is_empty());
    }

    #[test]
    fn a_node_without_edges_has_empty_rows() {
        let adjacency = sample();
        assert!(adjacency.successors(4).is_empty());
        assert!(adjacency.predecessors(4).is_empty());
        assert_eq!(adjacency.out_degree(4), 0);
        assert_eq!(adjacency.in_degree(4), 0);
    }

    #[test]
    fn rows_come_out_newest_edge_first() {
        let adjacency = sample();
        // 0 -> 1 was inserted twice: the later (Imports) edge leads.
        let kinds: Vec<DependencyType> = adjacency
            .edges(0, EdgeSide::Outgoing)
            .map(|edge| edge.dependency_type)
            .collect();
        assert_eq!(kinds, [Imports, Calls]);
        assert_eq!(adjacency.successors(0), [1, 1]);
        assert_eq!(adjacency.out_lists_in_insertion_order()[0], [1, 1]);
    }

    #[test]
    fn parallel_edges_and_loops_count_once_each() {
        let adjacency = sample();
        assert_eq!(adjacency.edge_count(), 5);
        assert_eq!(adjacency.in_degree(1), 3);
        assert_eq!(adjacency.out_degree(3), 1);
        assert_eq!(adjacency.in_degree(3), 1);
        assert_eq!(adjacency.successors(3), [3]);
    }

    #[test]
    fn the_incoming_side_pairs_each_caller_with_its_own_edge_facts() {
        let adjacency = sample();
        let incoming: Vec<(u32, DependencyType, f64)> = adjacency
            .edges(1, EdgeSide::Incoming)
            .map(|e| (e.neighbour, e.dependency_type, e.confidence))
            .collect();
        // Callers of 1, newest first: 2 (Calls 0.7), then 0 twice (Imports, Calls).
        assert_eq!(
            incoming,
            [(2, Calls, 0.7), (0, Imports, 0.5), (0, Calls, 1.0)]
        );
    }

    #[test]
    fn reach_lists_in_breadth_first_order_and_honours_the_depth() {
        let adjacency = sample();
        assert_eq!(
            adjacency.reach(0, EdgeSide::Outgoing, 0, false),
            Vec::<u32>::new()
        );
        assert_eq!(adjacency.reach(0, EdgeSide::Outgoing, 1, false), [1]);
        assert_eq!(
            adjacency.reach(0, EdgeSide::Outgoing, usize::MAX, false),
            [1, 2]
        );
        assert_eq!(
            adjacency.reach(2, EdgeSide::Incoming, usize::MAX, false),
            [1, 0]
        );
    }

    #[test]
    fn a_cycle_brings_the_root_back_only_when_asked() {
        let adjacency = sample();
        assert_eq!(
            adjacency.reach(1, EdgeSide::Outgoing, usize::MAX, false),
            [2]
        );
        assert_eq!(
            adjacency.reach(1, EdgeSide::Outgoing, usize::MAX, true),
            [2, 1]
        );
    }

    #[test]
    fn connects_follows_edge_direction_and_a_node_reaches_itself() {
        let adjacency = sample();
        assert!(adjacency.connects(0, 2));
        assert!(!adjacency.connects(2, 0));
        assert!(adjacency.connects(4, 4));
        assert!(adjacency.connects(3, 3));
        assert!(!adjacency.connects(3, 0));
    }

    #[test]
    fn undirected_rows_list_callers_then_the_callees_not_listed_yet() {
        let adjacency = sample();
        let rows = adjacency.undirected_lists();
        // 1: callers 2, 0, 0 (one entry per edge), then callee 2 is already listed.
        assert_eq!(rows[1], [2, 0, 0]);
        // 0: no callers, callees (insertion order) 1.
        assert_eq!(rows[0], [1]);
    }

    #[test]
    fn the_adjacency_adds_one_u32_per_edge_for_the_incoming_side() {
        let adjacency = sample();
        let nodes = adjacency.node_count();
        let edges = adjacency.edge_count();
        let expected = 2 * (nodes + 1) * 4
            + edges * (4 + 4 + 4)
            + edges * std::mem::size_of::<DependencyType>()
            + edges * 8;
        assert_eq!(adjacency.heap_bytes(), expected);
    }

    #[test]
    #[should_panic]
    fn an_endpoint_out_of_range_is_refused() {
        let edges = [(0usize, 7usize, Calls, 1.0)];
        CompactAdjacency::build(2, 1, edges.iter().copied());
    }

    #[test]
    fn visited_nodes_report_first_insertion_in_both_representations() {
        for node_count in [10, DENSE_BELOW + 1, 100_000] {
            let mut visited = VisitedNodes::new(node_count);
            let step = (node_count / 7).max(1);
            let nodes: Vec<u32> = (0..node_count as u32).step_by(step).collect();
            for &node in &nodes {
                assert!(visited.insert(node), "{node} is new in {node_count}");
            }
            for &node in &nodes {
                assert!(!visited.insert(node), "{node} is known in {node_count}");
            }
        }
    }

    #[test]
    fn visited_nodes_keep_their_marks_when_the_set_turns_into_a_bitset() {
        let node_count = 100_000;
        let mut visited = VisitedNodes::new(node_count);
        assert!(matches!(visited.repr, VisitedRepr::Sparse(_)));
        let until = (node_count / SPARSE_RATIO + 10) as u32;
        for node in 0..until {
            assert!(visited.insert(node * 7 % node_count as u32));
        }
        assert!(matches!(visited.repr, VisitedRepr::Dense(_)));
        for node in 0..until {
            assert!(!visited.insert(node * 7 % node_count as u32));
        }
    }

    #[test]
    #[should_panic]
    fn visited_nodes_refuse_a_node_out_of_range() {
        VisitedNodes::new(10).insert(10);
    }
}
