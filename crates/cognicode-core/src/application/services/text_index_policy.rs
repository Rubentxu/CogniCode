//! When a workspace's text index is worth building, and when it is not.
//!
//! The trigram index (`super::text_index`) narrows content searches, but it
//! is not free: it costs a cold pass over the corpus, and it retains heap in
//! proportion to what it indexed. Measured 2026-11-19 with
//! `crates/cognicode-core/benches/text_search.rs` (runs=2, queries/class=10,
//! `crates` corpus: 1473 files / 18.1 MiB):
//!
//! - warm selective queries improve roughly 25%–40% (p50 `lit rare`
//!   361–366 ms plain → 291 ms indexed; p50 `lit rare ci` 401–407 ms →
//!   289–290 ms; the CGH-090 card reported 4–10x on a quieter host);
//! - the cold first search costs 0.9 s and retains 15.9 MiB of heap,
//!   ≈ 0.9 B of index per byte of corpus (11338 B/file);
//! - a small corpus (`docs`: 67 files / 1.2 MiB) shows no gain worth the
//!   cold cost: plain p50s sit in the 9–12 ms range either way.
//!
//! So the index pays for itself only above a corpus size where reading
//! everything is measurably slow, and only while the heap it will retain
//! stays inside a budget the deployment can carry. This module is that
//! decision, pure: no clock, no environment, no file-system surprise — the
//! caller measures the corpus and asks. The reason a corpus is refused is
//! part of the answer, never a silent fall-through: a search that did not
//! get an index says why.
//!
//! The composition root (`interface::composition::default_text_index_config`)
//! is the one caller of [`TextIndexPolicy::config_for`]; the CLI's
//! `WorkspaceSession` and the MCP server's adapter both go through it, so
//! the knob is one rule for both surfaces.

use std::path::Path;

use ignore::WalkBuilder;

use crate::application::services::text_index::TextIndexConfig;

/// The smallest corpus, in bytes, the index is built for.
///
/// Below this, a full read is in the ten-millisecond range per query
/// (measured: 67 files / 1.2 MiB → plain p50 ≈ 9 ms, no class improved
/// under the index) and the cold build costs more than every warm query
/// saves. Two MiB sits above the corpus that showed no gain and well
/// below the 18 MiB corpus that showed a clear one.
pub const MIN_CORPUS_BYTES: u64 = 2 * 1024 * 1024;

/// The largest heap, in bytes, the policy will project for an index.
///
/// 256 MiB is a working set a session can hold without deciding for the
/// host; above it the deployment should say so explicitly by handing the
/// service a policy with a bigger budget, not get an index by surprise.
pub const MAX_PROJECTED_HEAP_BYTES: u64 = 268_435_456;

/// What a corpus looks like to the policy: the walk a content search would
/// make, reduced to the two numbers the decision reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CorpusMeasurement {
    /// Files the walk visits.
    pub files: usize,
    /// Their total size in bytes.
    pub bytes: u64,
}

/// Why a corpus does not get an index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkipReason {
    /// The corpus is smaller than `min_corpus_bytes`: reading it whole is
    /// cheaper than indexing it.
    BelowThreshold {
        /// What the corpus measured.
        measured_bytes: u64,
        /// The threshold it stayed under.
        min_corpus_bytes: u64,
    },
    /// The index would retain more than `budget_bytes`, projected at the
    /// measured ≈ 0.9 B per corpus byte (rounded up through the 9/8
    /// projection, so the projection never understates).
    OverBudget {
        /// What the corpus measured.
        measured_bytes: u64,
        /// The retained heap the corpus projects to.
        projected_heap_bytes: u64,
        /// The budget the projection exceeded.
        budget_bytes: u64,
    },
}

impl SkipReason {
    /// The reason in one sentence, for the log and for tests that assert on
    /// the honesty of the refusal.
    pub fn message(&self) -> String {
        match self {
            SkipReason::BelowThreshold {
                measured_bytes,
                min_corpus_bytes,
            } => format!(
                "corpus of {measured_bytes} bytes is below the \
                 {min_corpus_bytes}-byte index threshold: a full read is \
                 cheaper than a cold index",
            ),
            SkipReason::OverBudget {
                measured_bytes,
                projected_heap_bytes,
                budget_bytes,
            } => format!(
                "corpus of {measured_bytes} bytes projects {projected_heap_bytes} \
                 bytes of index heap, over the {budget_bytes}-byte budget",
            ),
        }
    }
}

/// Whether a corpus gets the index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextIndexDecision {
    /// Build the index for this corpus, with this configuration.
    Enabled(TextIndexConfig),
    /// Do not build it, and say why.
    Skipped(SkipReason),
}

/// The umbral and budget rule, as data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextIndexPolicy {
    /// Corpora smaller than this do not get an index.
    pub min_corpus_bytes: u64,
    /// Corpora whose projected index heap exceeds this do not get one.
    pub max_projected_heap_bytes: u64,
    /// The configuration an enabled index is built with.
    pub config: TextIndexConfig,
}

impl Default for TextIndexPolicy {
    fn default() -> Self {
        Self {
            min_corpus_bytes: MIN_CORPUS_BYTES,
            max_projected_heap_bytes: MAX_PROJECTED_HEAP_BYTES,
            config: TextIndexConfig::default(),
        }
    }
}

impl TextIndexPolicy {
    /// Measures what a content search would read under `root`: the same walk
    /// configuration the search itself builds (hidden and ignored files are
    /// not in the corpus because the search never goes near them). Walk
    /// errors end the count at what was seen — a corpus that cannot be
    /// measured whole is refused by its measurement, not assumed small.
    pub fn measure(root: &Path) -> CorpusMeasurement {
        let mut walker = WalkBuilder::new(root);
        walker
            .hidden(true)
            .git_ignore(true)
            .git_global(true)
            .parents(true)
            .require_git(false);
        let mut files = 0usize;
        let mut bytes = 0u64;
        for entry in walker.build().flatten() {
            if !entry.file_type().is_some_and(|kind| kind.is_file()) {
                continue;
            }
            files += 1;
            bytes += entry.metadata().map(|meta| meta.len()).unwrap_or(0);
        }
        CorpusMeasurement { files, bytes }
    }

    /// The heap an index over `bytes` of corpus is projected to retain.
    ///
    /// Measured at ≈ 0.9 B per corpus byte; the projection rounds the other
    /// way, to 9/8, so a corpus the policy calls affordable never turns out
    /// over budget at the sizes that were measured.
    pub fn projected_heap_bytes(&self, bytes: u64) -> u64 {
        bytes.saturating_mul(9) / 8
    }

    /// The pure decision: threshold first, then budget.
    pub fn decide(&self, measurement: CorpusMeasurement) -> TextIndexDecision {
        if measurement.bytes < self.min_corpus_bytes {
            return TextIndexDecision::Skipped(SkipReason::BelowThreshold {
                measured_bytes: measurement.bytes,
                min_corpus_bytes: self.min_corpus_bytes,
            });
        }
        let projected = self.projected_heap_bytes(measurement.bytes);
        if projected > self.max_projected_heap_bytes {
            return TextIndexDecision::Skipped(SkipReason::OverBudget {
                measured_bytes: measurement.bytes,
                projected_heap_bytes: projected,
                budget_bytes: self.max_projected_heap_bytes,
            });
        }
        TextIndexDecision::Enabled(self.config.clone())
    }

    /// The config a corpus earns, or `None` with the reason in the log.
    ///
    /// This is the one call composition roots make: the CLI session and the
    /// MCP server answer the same corpus the same way. The refusal is never
    /// silence — a deployment that gets no index is told why.
    pub fn config_for(&self, root: &Path) -> Option<TextIndexConfig> {
        match self.decide(Self::measure(root)) {
            TextIndexDecision::Enabled(config) => Some(config),
            TextIndexDecision::Skipped(reason) => {
                tracing::info!("text index not activated: {}", reason.message());
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::ports::{PathPolicy, PathPolicyError};
    use std::fs;
    use std::path::PathBuf;
    use std::sync::Arc;

    /// A policy that lets every path through: these tests are about whether
    /// the service got an index, not about which paths a workspace admits.
    struct OpenPolicy;

    impl PathPolicy for OpenPolicy {
        fn validate_path(&self, _path: &Path) -> Result<(), PathPolicyError> {
            Ok(())
        }

        fn resolve_for_creation(&self, path: &Path) -> Result<PathBuf, PathPolicyError> {
            Ok(path.to_path_buf())
        }
    }

    /// Writes `count` files of `size` bytes under a fresh tempdir root.
    fn corpus(count: usize, size: usize) -> (tempfile::TempDir, CorpusMeasurement) {
        let dir = tempfile::tempdir().expect("tempdir");
        for i in 0..count {
            fs::write(
                dir.path().join(format!("f{i:04}.txt")),
                vec![b'a' + (i % 26) as u8; size],
            )
            .expect("file");
        }
        let measurement = TextIndexPolicy::measure(dir.path());
        (dir, measurement)
    }

    #[test]
    fn a_corpus_below_the_threshold_is_refused_with_the_reason() {
        let (_dir, measurement) = corpus(3, 1024);
        let policy = TextIndexPolicy::default();
        assert!(measurement.bytes < policy.min_corpus_bytes);
        match policy.decide(measurement) {
            TextIndexDecision::Skipped(reason) => {
                assert!(matches!(reason, SkipReason::BelowThreshold { .. }));
                assert!(
                    reason.message().contains("below"),
                    "the refusal says why: {}",
                    reason.message()
                );
            }
            TextIndexDecision::Enabled(_) => panic!("a toy corpus must not earn an index"),
        }
    }

    #[test]
    fn a_corpus_over_the_threshold_within_budget_activates() {
        // 17 files x 128 KiB = 2.125 MiB: over the 2 MiB threshold,
        // projecting 2.39 MiB of heap, far under the 256 MiB budget.
        let (_dir, measurement) = corpus(17, 128 * 1024);
        let policy = TextIndexPolicy::default();
        assert!(measurement.bytes >= policy.min_corpus_bytes);
        match policy.decide(measurement) {
            TextIndexDecision::Enabled(config) => assert_eq!(config, policy.config),
            TextIndexDecision::Skipped(reason) => {
                panic!("an affordable corpus must activate: {}", reason.message())
            }
        }
    }

    #[test]
    fn a_corpus_over_the_budget_is_refused_with_the_reason() {
        // Same corpus as the activating test, but a policy whose budget is
        // smaller than the projection: the refusal must name the budget.
        let (_dir, measurement) = corpus(16, 128 * 1024);
        let policy = TextIndexPolicy {
            max_projected_heap_bytes: 2 * 1024 * 1024,
            ..TextIndexPolicy::default()
        };
        let projected = policy.projected_heap_bytes(measurement.bytes);
        assert!(projected > policy.max_projected_heap_bytes);
        match policy.decide(measurement) {
            TextIndexDecision::Skipped(
                reason @ SkipReason::OverBudget {
                    measured_bytes,
                    projected_heap_bytes,
                    budget_bytes,
                },
            ) => {
                assert_eq!(measured_bytes, measurement.bytes);
                assert_eq!(projected_heap_bytes, projected);
                assert_eq!(budget_bytes, policy.max_projected_heap_bytes);
                assert!(
                    reason.message().contains("over the"),
                    "the refusal names the budget: {}",
                    reason.message()
                );
            }
            other => panic!("an over-budget corpus must be refused, got {other:?}"),
        }
    }

    #[test]
    fn the_threshold_is_checked_before_the_budget() {
        // A corpus under both bounds refuses for the first reason only: it
        // never reached a size worth projecting.
        let (_dir, measurement) = corpus(2, 512);
        let policy = TextIndexPolicy {
            min_corpus_bytes: 4096,
            max_projected_heap_bytes: 1024,
            ..TextIndexPolicy::default()
        };
        assert!(matches!(
            policy.decide(measurement),
            TextIndexDecision::Skipped(SkipReason::BelowThreshold { .. })
        ));
    }

    #[test]
    fn the_walk_measures_what_a_search_would_read() {
        let dir = tempfile::tempdir().expect("tempdir");
        fs::create_dir(dir.path().join("src")).expect("dir");
        fs::write(dir.path().join("src/keep.txt"), vec![b'x'; 4096]).expect("file");
        fs::write(dir.path().join("ignored.bin"), vec![b'y'; 1 << 20]).expect("file");
        fs::write(dir.path().join(".gitignore"), "ignored.bin\n").expect("ignore");
        let measurement = TextIndexPolicy::measure(dir.path());
        assert_eq!(
            measurement.files, 1,
            "only keep.txt: the walk reads .gitignore as a rule file, \
             not as corpus — the same walk a search builds"
        );
        assert_eq!(measurement.bytes, 4096);
    }

    #[test]
    fn config_for_follows_the_decision_end_to_end() {
        // The corpus the composition root would hand to the service: over
        // the threshold, within budget, so `config_for` yields the index
        // and a search through the service carries its report.
        let (dir, measurement) = corpus(17, 128 * 1024);
        let policy = TextIndexPolicy::default();
        let config = policy
            .config_for(dir.path())
            .expect("a 2.1 MiB corpus earns an index");
        let service = crate::application::services::file_operations::FileOperationsService::new(
            dir.path().to_string_lossy().to_string(),
            Arc::new(OpenPolicy),
            Arc::new(crate::infrastructure::verification::RustVerifier::new()),
            Arc::new(
                crate::infrastructure::parser::syntax_analysis::TreeSitterSyntaxAnalysis::new(),
            ),
        )
        .with_text_index(config);
        assert_eq!(
            service.text_index_config(),
            Some(&policy.config),
            "the service carries the policy it was wired with"
        );
        let result = service
            .search_content(crate::application::dto::SearchContentRequest::new(
                "xxxxneedle",
            ))
            .expect("search");
        let report = result
            .text_index
            .expect("a corpus over the threshold gets a report");
        assert!(report.used, "the wired index narrows: {:?}", report.reason);
        assert!(measurement.bytes >= policy.min_corpus_bytes);
    }

    #[test]
    fn a_refused_corpus_leaves_the_service_unindexed_and_says_no_report() {
        let (dir, _measurement) = corpus(3, 1024);
        let policy = TextIndexPolicy::default();
        assert!(
            policy.config_for(dir.path()).is_none(),
            "a toy corpus earns no index"
        );
    }
}
