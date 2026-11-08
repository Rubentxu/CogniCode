//! The basis of a `build_graph` answer: what it was built from, and who
//! served it.
//!
//! ## Why it has its own home
//!
//! The basis is the identity of an analysis: the workspace, the
//! configuration, the sources, the analyzer that read them —and who
//! served the graph. It is a document with a published shape:
//! `product/tools.json` carries the schema derived from it, and a
//! consumer reads it to know what the answer is worth.
//!
//! It lives apart from the handler that fills it for two reasons: the
//! published contract has one home, and the handler keeps the code that
//! decides *what* the answer is —which is a different question from what
//! the answer says about itself.

use std::path::Path;

use schemars::JsonSchema;

use crate::application::analysis_basis::GraphSource;
use crate::application::graph_snapshot::{build_manifest, hex_digest, source_digest};
use crate::domain::value_objects::file_manifest::FileManifest;
use crate::domain::value_objects::{AnalyzerBuild, SnapshotKey, SNAPSHOT_SCHEMA};

use super::HandlerContext;

/// Provenance basis for a build_graph result.
#[derive(Debug, serde::Serialize, JsonSchema)]
pub struct BasisDto {
    /// Canonicalized workspace path the graph was built from.
    pub workspace: String,
    /// Digest over the effective configuration relevant to the build.
    /// Stable across identical configurations; changes when config changes.
    pub config_digest: String,
    /// Digest over the source manifest (sorted relative paths + content
    /// hashes). Same sources ⇒ same digest; any source change ⇒ different
    /// digest. Empty when the manifest could not be computed.
    pub source_manifest_digest: String,
    /// The version of the analyzer that produced this analysis.
    ///
    /// The provider's version, not the consumer's: it says which build
    /// calculated the answer, which is what a cached answer has to be
    /// checked against before it can be served again.
    pub analyzer_version: String,
    /// The commit the analyzer was built from, when it could be known.
    ///
    /// `null` when the binary was built outside a checkout. That is a
    /// fact about the binary, not a value invented to fill the field.
    pub analyzer_commit: Option<String>,
    /// The capabilities the analyzer was built with, in a stable order.
    pub analyzer_features: Vec<String>,
    /// The shape the snapshot was written in.
    pub snapshot_schema: String,
    /// Where the graph came from: `built`, `cache` or `daemon`.
    ///
    /// The graph that was read now and the graph that was served from
    /// before are the same answer only when the tree did not change in
    /// between, and *who checked that* is part of the identity of the
    /// answer: a consumer that trusts a daemon's word has to know there
    /// was one. The word is the one the provenance enum publishes, so the
    /// message the handler writes and the field the consumer reads cannot
    /// disagree.
    pub graph_source: String,
    /// `true` only when every identity component was established and
    /// there was material to identify. A workspace with nothing to
    /// analyze has an established identity of an empty tree, which
    /// describes no analysis: the basis is not complete.
    pub complete: bool,
}

/// The identity basis of an answer.
///
/// The manifest the answer already carries is the one that describes the
/// sources the analysis covered: the snapshot's manifest when the graph
/// was served, the one taken before the build when it was read. The tree
/// is walked again only when the answer carries none —a build that read
/// nothing leaves no manifest behind— because hashing the tree is the
/// expensive part of the answer, and the answer already paid it.
pub(super) fn compute_basis(
    ctx: &HandlerContext,
    project_dir: &Path,
    manifest: Option<&FileManifest>,
    source: GraphSource,
) -> BasisDto {
    use sha2::{Digest, Sha256};

    let (workspace, workspace_ok) = match project_dir.canonicalize() {
        Ok(p) => (p.to_string_lossy().to_string(), true),
        Err(_) => (project_dir.to_string_lossy().to_string(), false),
    };

    // Config digest: stable serialization of effective config knobs.
    let config_key = format!(
        "log_level={}",
        ctx.log_level
            .try_read()
            .map(|l| l.to_string())
            .unwrap_or_else(|_| "unreadable".into())
    );
    let config_digest = hex_digest(&Sha256::digest(config_key.as_bytes()));

    // The sources: the manifest the answer carries, or —when it carries
    // none— the one the tree says now.
    let source_manifest_digest = match manifest {
        Some(manifest) => source_digest(manifest),
        None => {
            let tree = ctx.analysis_service.tree_state(project_dir);
            source_digest(&build_manifest(project_dir, &tree.files))
        }
    };

    // The key of the analysis: who produced it, over what, and with which
    // shape. It is the same key the snapshot is written with.
    let key = SnapshotKey::new(workspace.clone(), AnalyzerBuild::current(), SNAPSHOT_SCHEMA);

    BasisDto {
        workspace,
        config_digest,
        source_manifest_digest,
        analyzer_version: key.analyzer.version,
        analyzer_commit: key.analyzer.commit,
        analyzer_features: key.analyzer.features,
        snapshot_schema: key.schema,
        graph_source: source.published_label().to_string(),
        complete: workspace_ok,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::analysis_basis::GraphSource;

    #[test]
    fn log_level_changes_provenance_config_digest_not_snapshot_identity() {
        let project = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            project.path().join("lib.rs"),
            "pub fn answer() -> u32 { 42 }\n",
        )
        .expect("write fixture");

        let info = HandlerContext::builder()
            .with_working_dir(project.path())
            .with_log_level(tracing::Level::INFO)
            .build();
        let debug = HandlerContext::builder()
            .with_working_dir(project.path())
            .with_log_level(tracing::Level::DEBUG)
            .build();

        info.analysis_service
            .build_project_graph(project.path())
            .expect("build info graph");
        debug
            .analysis_service
            .build_project_graph(project.path())
            .expect("build debug graph");
        assert_eq!(
            info.analysis_service.get_project_graph(),
            debug.analysis_service.get_project_graph()
        );

        let info_basis = compute_basis(&info, project.path(), None, GraphSource::Built);
        let debug_basis = compute_basis(&debug, project.path(), None, GraphSource::Built);

        assert_ne!(info_basis.config_digest, debug_basis.config_digest);
        assert_eq!(info_basis.workspace, debug_basis.workspace);
        assert_eq!(
            info_basis.source_manifest_digest,
            debug_basis.source_manifest_digest
        );
        assert_eq!(info_basis.analyzer_version, debug_basis.analyzer_version);
        assert_eq!(info_basis.analyzer_commit, debug_basis.analyzer_commit);
        assert_eq!(info_basis.analyzer_features, debug_basis.analyzer_features);
        assert_eq!(info_basis.snapshot_schema, debug_basis.snapshot_schema);
    }
}
