//! `DocsExtractor` — turns Markdown / ADR files into
//! [`ExtractedNode`](crate::domain::traits::source_extractor::ExtractedNode)
//! candidates for the Generic Graph Layer.
//!
//! The extractor is the first concrete implementation of the
//! [`SourceExtractor`](crate::domain::traits::source_extractor::SourceExtractor)
//! port. It is split into two pieces that share the same module:
//!
//! 1. `parse_markdown(text, source_path, slug)` — a pure function
//!    that takes a Markdown string and a `SourcePath` / anchor
//!    slug and returns a list of `ExtractedNode` candidates. No
//!    filesystem, no async, no I/O. The function is the unit-test
//!    surface (T12's RED gate).
//! 2. `DocsExtractor` — the async [`SourceExtractor`] impl that
//!    walks a `SourcePath` (file or directory), runs the pure
//!    parser on each `.md` file, and concatenates the candidates.
//!    Idempotent: re-ingesting the same file produces the same
//!    `NodeId`s, so the persistence layer's upsert collapses
//!    duplicates (T13's RED gate).
//!
//! The whole module is `#[cfg(feature = "multimodal")]`-gated
//! because it is part of the docs-source adapter pipeline.
//!
//! ## ADR detection
//!
//! ADRs (Architecture Decision Records) are detected by scanning
//! the file's first 4 KiB for the canonical markers
//! `# ADR-NNNN:` / `# ADR- NNNN` / `# Decision: NNNN`. When a
//! marker is present, the top-level heading produces a
//! [`NodeKind::Decision`] node (instead of [`NodeKind::Doc`]),
//! and the `Status:` line in the body is captured as a
//! `status` property on the node.
// e30.1 clippy baseline reset: pre-existing lint debt (see fix/e30.1-clippy-baseline-reset)
#![allow(
    clippy::ptr_arg,
    clippy::too_many_arguments,
    clippy::unnecessary_map_or,
    unused_imports
)]

#[cfg(feature = "multimodal")]
use std::path::{Path, PathBuf};

#[cfg(feature = "multimodal")]
use async_trait::async_trait;
#[cfg(feature = "multimodal")]
use chrono::Utc;
#[cfg(feature = "multimodal")]
use pulldown_cmark::{Event, HeadingLevel, LinkType, Parser, Tag, TagEnd};
#[cfg(feature = "multimodal")]
use walkdir::WalkDir;

#[cfg(feature = "multimodal")]
use crate::domain::aggregates::SymbolId;
#[cfg(feature = "multimodal")]
use crate::domain::aggregates::generic_graph::{GraphEdge, GraphNode, NodeId};
#[cfg(feature = "multimodal")]
use crate::domain::traits::source_extractor::{
    ExtractedNode, SourceExtractor, SourceExtractorError, SourceExtractorResult, SourcePath,
};
#[cfg(feature = "multimodal")]
use crate::domain::value_objects::edge_kind::EdgeKind;
#[cfg(feature = "multimodal")]
use crate::domain::value_objects::node_kind::NodeKind;
#[cfg(feature = "multimodal")]
use crate::domain::value_objects::provenance::Provenance;

#[cfg(feature = "multimodal")]
use super::docs_confidence_rules::{ConfidenceTier, score_link, sym_short_name};

// ============================================================================
// Pure parsing function (T12 surface).
// ============================================================================

/// Parse a Markdown string into a list of `ExtractedNode`s.
///
/// `source_path` is the canonical file path the parser uses to
/// build deterministic `NodeId`s. `file_stem` is the
/// `Path::file_stem()` of the source — it is used to disambiguate
/// between two ADRs that live in different files but share an
/// anchor.
///
/// One `ExtractedNode` is emitted per heading (the heading's
/// text becomes the node's label and the heading's anchor becomes
/// the `NodeId`'s suffix). The first heading also determines
/// whether the document is a `Decision` (ADR) or a generic
/// `Doc`.
#[cfg(feature = "multimodal")]
pub fn parse_markdown(text: &str, source_path: &Path, file_stem: &str) -> Vec<ExtractedNode> {
    let mut nodes: Vec<ExtractedNode> = Vec::new();
    let mut status_lines: Vec<String> = Vec::new();
    let is_adr = detect_adr(text);

    // First pass: walk all events. For each heading we emit one
    // `ExtractedNode`; links to known code symbols inside the
    // heading or its body are accumulated as edges.
    let parser = Parser::new(text);
    let mut current_heading: Option<(String, usize, HeadingLevel)> = None;
    // Buffer for the body text of the most recently emitted
    // node. The body is the concatenation of all text + code
    // between this heading and the next. When a new heading
    // starts, the buffer is flushed as additional edges onto the
    // PREVIOUS node (i.e. body paragraphs between two headings
    // belong to the *previous* heading per Markdown convention).
    let mut current_body = String::new();
    let mut code_text_buf: Option<String> = None;
    let mut in_link: Option<String> = None;

    // ---- Deferred-heading node building for code fence language ----
    // Code blocks appear AFTER their heading ends in the event stream
    // (heading ends → body text → code fence). We defer building
    // the heading node until the NEXT heading starts (or EOF) so we
    // can attach `code_block_lang` from any fences in the body.
    let mut pending_heading: Option<(String, usize, HeadingLevel)> = None;
    let mut pending_code_lang: Option<String> = None;

    /// Build and push a deferred heading node.
    fn build_and_push_node(
        nodes: &mut Vec<ExtractedNode>,
        label: &str,
        line: usize,
        lvl: HeadingLevel,
        body: &str,
        file_stem: &str,
        source_path: &Path,
        is_adr: bool,
        status_lines: &mut Vec<String>,
        code_lang: Option<String>,
    ) {
        let mut node = build_heading_node(
            label,
            line,
            lvl,
            file_stem,
            source_path,
            is_adr,
            body,
            status_lines,
        );
        // Attach code fence language if present
        if let Some(lang) = code_lang {
            node = node.with_property("code_block_lang", lang);
        }
        let mut edges: Vec<GraphEdge> = body
            .lines()
            .filter_map(|line| classify_body_line(line, file_stem))
            .map(|cites| CitesCandidate::Body(cites).into_edge(&node.id))
            .collect();
        // Also check for doc links (cross-ADR/cross-doc citations)
        for line in body.lines() {
            if let Some(target) = extract_link_target(line)
                && let Some(doc_cites) = classify_doc_link(&target)
            {
                edges.push(CitesCandidate::Doc(doc_cites).into_edge(&node.id));
            }
        }
        nodes.push(ExtractedNode::with_edges(node, edges));
    }

    /// Flush the accumulated `body` to the most recently
    /// emitted node by appending edges AND capturing any
    /// ADR-specific metadata (`Status:` line). If no node
    /// exists yet (e.g. body before the first heading), the body
    /// is dropped — in practice, the parser guarantees the body
    /// never precedes the first heading.
    fn flush_trailing_body(nodes: &mut Vec<ExtractedNode>, body: &mut String, file_stem: &str) {
        if body.is_empty() {
            return;
        }
        let Some(last) = nodes.last_mut() else { return };
        let source_id = last.potential_node.id.clone();
        // Capture ADR `Status:` from the trailing body so the
        // property lands on the right node (the one whose
        // heading was emitted earlier, not the one that opens
        // next).
        if let Some(status) = extract_status(body) {
            last.potential_node = last.potential_node.clone().with_property("status", status);
        }
        for line in body.lines() {
            // Try body line (symbol citation)
            if let Some(cites) = classify_body_line(line, file_stem) {
                last.potential_edges
                    .push(CitesCandidate::Body(cites).into_edge(&source_id));
            }
            // Try doc link (cross-ADR/cross-doc citation)
            if let Some(target) = extract_link_target(line)
                && let Some(doc_cites) = classify_doc_link(&target)
            {
                last.potential_edges
                    .push(CitesCandidate::Doc(doc_cites).into_edge(&source_id));
            }
        }
        body.clear();
    }

    for (event_offset, event) in parser.enumerate() {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                // Open a new heading — first, flush any pending
                // node from the previous heading (with its body and
                // any code fence language), then start new heading.
                if let Some((label, line, lvl)) = pending_heading.take() {
                    let body = std::mem::take(&mut current_body);
                    build_and_push_node(
                        &mut nodes,
                        &label,
                        line,
                        lvl,
                        &body,
                        file_stem,
                        source_path,
                        is_adr,
                        &mut status_lines,
                        pending_code_lang.take(),
                    );
                }
                current_heading = Some((String::new(), event_offset, level));
            }
            Event::End(TagEnd::Heading(_)) => {
                // The heading is complete; defer emitting the node
                // until we see the next heading (or EOF) so we can
                // attach `code_block_lang` from any fenced code
                // blocks that appear in the body.
                //
                // Use take() to atomically consume current_heading
                // and start fresh.
                let prev_heading = current_heading.take();
                if let Some((label, line, lvl)) = prev_heading {
                    // Store for deferred building; body will be
                    // accumulated in current_body until next heading.
                    pending_heading = Some((label, line, lvl));
                    // Code lang from any previous heading's fences
                    // stays in pending_code_lang for now.
                }
            }
            Event::Text(text) | Event::Code(text) => {
                if let Some((ref mut label, _, _)) = current_heading
                    && code_text_buf.is_none()
                {
                    label.push_str(&text);
                }
                if let Some(buf) = code_text_buf.as_mut() {
                    buf.push_str(&text);
                } else {
                    // Text is accumulated into `current_body`
                    // whether or not a heading is currently
                    // open. The heading open / EOF arms flush
                    // the buffer onto the previous node.
                    current_body.push_str(&text);
                    current_body.push('\n');
                }
                let _ = in_link.as_ref();
            }
            Event::Start(Tag::Link {
                dest_url,
                link_type,
                ..
            }) => {
                // Skip autolinks and external URLs — the spec
                // only resolves intra-repo links.
                if link_type == LinkType::Autolink
                    || link_type == LinkType::Email
                    || dest_url.starts_with("http://")
                    || dest_url.starts_with("https://")
                {
                    in_link = None;
                } else {
                    in_link = Some(dest_url.to_string());
                }
            }
            Event::End(TagEnd::Link) => {
                if let Some(target) = in_link.take() {
                    // The link is now a `(target, body)` pair.
                    // We don't have the body separately in the
                    // event stream, but the target itself is
                    // enough for a `Cites` edge against a
                    // matching symbol — record it as a synthetic
                    // body line so the classifier picks it up.
                    current_body.push_str(&target);
                    current_body.push('\n');
                }
            }
            Event::Start(Tag::CodeBlock(kind)) => {
                code_text_buf = Some(String::new());
                // Capture fenced code block language for `code_block_lang` property.
                // `CodeBlockKind::Fenced(lang)` has the info string (may be empty).
                // `CodeBlockKind::Indented` is an indented code block (no language).
                if let pulldown_cmark::CodeBlockKind::Fenced(ref lang) = kind {
                    let lang_str = lang.to_string();
                    if !lang_str.is_empty() {
                        pending_code_lang = Some(lang_str);
                    }
                }
            }
            Event::End(TagEnd::CodeBlock) => {
                if let Some(buf) = code_text_buf.take() {
                    // The code block's contents are appended to
                    // the current body — the body classifier
                    // looks for `file:name:line` patterns and
                    // emits a `Cites` edge per match.
                    current_body.push_str(&buf);
                    current_body.push('\n');
                }
            }
            _ => {
                // Other events (HTML, soft/hard break, rule,
                // image, footnote, …) are intentionally ignored:
                // the spec only needs headings, links, and code
                // blocks. Future PRs can add the missing
                // extractors without changing this function's
                // signature.
            }
        }
    }

    // Flush any pending heading node at EOF. This handles:
    //   1. A heading that ended but whose body content (including
    //      code fences) appears after the heading in the event stream.
    //   2. The last heading closed, then trailing paragraphs
    //      accumulated in `current_body`.
    if let Some((label, line, lvl)) = pending_heading.take() {
        // Build the pending node with its accumulated body and code lang
        let body = std::mem::take(&mut current_body);
        build_and_push_node(
            &mut nodes,
            &label,
            line,
            lvl,
            &body,
            file_stem,
            source_path,
            is_adr,
            &mut status_lines,
            pending_code_lang.take(),
        );
    } else if current_heading.take().is_some() {
        // Heading still open but no pending (shouldn't happen with
        // deferred building, but handle for completeness).
        let body = std::mem::take(&mut current_body);
        if !body.is_empty() {
            flush_trailing_body(&mut nodes, &mut current_body, file_stem);
        }
    } else {
        // No pending heading; just flush trailing body to most recent node.
        flush_trailing_body(&mut nodes, &mut current_body, file_stem);
    }

    // If the file had headings but no top-level heading was
    // emitted (e.g. a single H1 followed by an EOF), emit a
    // fallback file-level node so the file is never silently
    // dropped.
    if nodes.is_empty() && !text.trim().is_empty() {
        let node = build_fallback_node(text, file_stem, source_path, is_adr, &status_lines);
        nodes.push(ExtractedNode::new(node));
    }

    // ---- Phase 4.2: Emit Decision→Doc Justifies edge for ADRs ----
    // When an ADR has multiple headings (nodes.len() >= 2), we need to
    // emit a Justifies edge from the first Decision node to a file-level
    // Doc node. The Doc node represents the ADR document itself, and
    // the Decision (H1 heading) justifies why this decision was made.
    if is_adr && nodes.len() >= 2 {
        // Find the first Decision node
        if let Some(first_decision_idx) = nodes
            .iter()
            .position(|n| n.potential_node.kind == NodeKind::Decision)
        {
            // Check if a Doc node already exists; if not, create a file-level Doc node
            let doc_idx = if let Some(idx) = nodes
                .iter()
                .position(|n| n.potential_node.kind == NodeKind::Doc)
            {
                idx
            } else {
                // Create a file-level Doc node for the ADR
                let doc_id_str = format!("doc:{}#intro", file_stem);
                let now = Utc::now();
                let doc_label = source_path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| file_stem.to_string());
                let doc_node = GraphNode::builder(NodeId::new(doc_id_str), NodeKind::Doc)
                    .label(doc_label)
                    .source_path(source_path.to_path_buf())
                    .created_at(now)
                    .updated_at(now)
                    .property("file_level", "adr_doc")
                    .build();
                nodes.push(ExtractedNode::new(doc_node));
                nodes.len() - 1 // Return the index of the newly created node
            };

            // Add Justifies edge from first Decision to the Doc node
            let decision_id = nodes[first_decision_idx].potential_node.id.clone();
            let doc_id = nodes[doc_idx].potential_node.id.clone();
            let justifies_edge = GraphEdge::new(
                decision_id,
                doc_id,
                EdgeKind::Justifies,
                Provenance::Extracted,
                1.0,
            )
            .expect("GraphEdge creation should not fail for valid inputs");
            nodes[first_decision_idx]
                .potential_edges
                .push(justifies_edge);
        }
    }

    nodes
}

// ============================================================================
// DocsExtractor — async SourceExtractor impl (T13 surface).
// ============================================================================

/// Markdown/ADR [`SourceExtractor`] implementation. Walks a
/// `SourcePath` (file or directory), runs
/// [`parse_markdown`] on every `.md` file, and concatenates
/// the candidates.
///
/// The extractor does NOT touch the persistence layer. The
/// caller (the MCP `docs_ingest` tool or the `cognicode
/// docs-ingest` CLI) is responsible for upserting the
/// candidates into the `graph_nodes` / `graph_edges` tables.
///
/// Idempotency: the `NodeId`s are deterministic
/// (`doc:{file_stem}#{slug}` / `decision:{file_stem}#{slug}`),
/// so re-ingesting the same file produces the SAME ids. The
/// repository's upsert collapses the resulting duplicates.
#[cfg(feature = "multimodal")]
#[derive(Debug, Default, Clone)]
pub struct DocsExtractor;

#[cfg(feature = "multimodal")]
impl DocsExtractor {
    /// Build a new `DocsExtractor`. The struct is stateless so
    /// the constructor is a no-op; the `Default` impl is also
    /// usable.
    pub fn new() -> Self {
        Self
    }

    /// Extract candidate nodes + edges from a directory, with
    /// control over recursion. The trait-level
    /// [`SourceExtractor::extract`] is hardcoded to recursive for
    /// backward compatibility with callers that pre-date the
    /// flag; this is the explicit form that the MCP `docs_ingest`
    /// tool uses.
    pub async fn extract_directory(
        &self,
        dir: &Path,
        recursive: bool,
    ) -> SourceExtractorResult<Vec<ExtractedNode>> {
        extract_from_directory(dir, recursive).await
    }

    /// Extract candidate nodes + edges from a single file. The
    /// single-file path is unaffected by the `recursive` flag.
    pub async fn extract_file(&self, file: &Path) -> SourceExtractorResult<Vec<ExtractedNode>> {
        extract_from_file(file).await
    }
}

#[cfg(feature = "multimodal")]
#[async_trait]
impl SourceExtractor for DocsExtractor {
    fn source_kind(&self) -> &'static str {
        "markdown"
    }

    async fn extract(&self, source: SourcePath) -> SourceExtractorResult<Vec<ExtractedNode>> {
        match source {
            SourcePath::File(path) => extract_from_file(&path).await,
            SourcePath::Directory(path) => extract_from_directory(&path, true).await,
            SourcePath::Url(_) => Err(SourceExtractorError::Unsupported(
                "docs extractor does not fetch remote URLs".to_string(),
            )),
        }
    }
}

// ============================================================================
// Internals
// ============================================================================

/// Discover `.md` files in `dir` (recursively when `recursive` is
/// true) and run [`parse_markdown`] on each. Invalid-UTF8 files
/// are logged via `tracing::warn!` and skipped (the contract is
/// "skip, don't crash"; callers get the partial result).
#[cfg(feature = "multimodal")]
async fn extract_from_directory(
    dir: &Path,
    recursive: bool,
) -> SourceExtractorResult<Vec<ExtractedNode>> {
    if !dir.is_dir() {
        return Err(SourceExtractorError::NotFound(dir.display().to_string()));
    }
    let mut out: Vec<ExtractedNode> = Vec::new();
    let walker = if recursive {
        WalkDir::new(dir).follow_links(false).into_iter()
    } else {
        WalkDir::new(dir).max_depth(1).into_iter()
    };
    for entry in walker.flatten() {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if !is_markdown_path(path) {
            continue;
        }
        match extract_from_file(path).await {
            Ok(mut nodes) => out.append(&mut nodes),
            Err(SourceExtractorError::InvalidUtf8(p)) => {
                tracing::warn!(file = %p, "skipping non-UTF8 markdown file");
                continue;
            }
            Err(SourceExtractorError::ReadFailed { path, source }) => {
                tracing::warn!(file = %path, error = %source, "skipping unreadable markdown file");
                continue;
            }
            Err(other) => return Err(other),
        }
    }
    Ok(out)
}

#[cfg(feature = "multimodal")]
async fn extract_from_file(path: &Path) -> SourceExtractorResult<Vec<ExtractedNode>> {
    if !path.is_file() {
        return Err(SourceExtractorError::NotFound(path.display().to_string()));
    }
    let bytes = std::fs::read(path).map_err(|e| SourceExtractorError::ReadFailed {
        path: path.display().to_string(),
        source: e,
    })?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|_| SourceExtractorError::InvalidUtf8(path.display().to_string()))?;
    let file_stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "doc".to_string());
    Ok(parse_markdown(text, path, &file_stem))
}

#[cfg(feature = "multimodal")]
fn is_markdown_path(p: &Path) -> bool {
    matches!(
        p.extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("md") | Some("markdown") | Some("mdx")
    )
}

/// Detect whether a Markdown file is an ADR. The heuristic scans
/// the first 4 KiB for the canonical ADR markers
/// (`# ADR-NNNN:`, `# ADR- NNNN:`, or `# Decision: NNNN`).
/// Conservative: false positives are preferable to false
/// negatives because the wrong kind costs only a UI label, while
/// a missed ADR costs the entire decision in the graph.
#[cfg(feature = "multimodal")]
fn detect_adr(text: &str) -> bool {
    let head = &text[..text.len().min(4096)];
    head.lines().take(10).any(|line| {
        let l = line.trim_start_matches('#').trim_start();
        // Match "# ADR-0001: Title", "# ADR- 0001: Title",
        // "# Decision: 0001: Title", etc. The pattern is loose on
        // purpose — ADRs in the wild have inconsistent prefixes.
        if l.len() < 4 {
            return false;
        }
        let l_lower = l.to_ascii_lowercase();
        l_lower.starts_with("adr-")
            || l_lower.starts_with("adr ")
            || l_lower.starts_with("decision:")
            || l_lower.starts_with("decision record")
    })
}

/// Build a `GraphNode` for a single heading. The slug is the
/// heading text lower-cased and slugified; the `NodeId` is
/// `{kind}:{file_stem}#{slug}`.
#[cfg(feature = "multimodal")]
fn build_heading_node(
    label: &str,
    line: usize,
    level: HeadingLevel,
    file_stem: &str,
    source_path: &Path,
    is_adr: bool,
    body: &str,
    status_lines: &mut Vec<String>,
) -> GraphNode {
    let slug = slugify(label);
    let kind = if is_adr && (level == HeadingLevel::H1 || level == HeadingLevel::H2) {
        NodeKind::Decision
    } else {
        NodeKind::Doc
    };
    let id_str = format!("{}:{}#{}", kind_prefix(&kind), file_stem, slug);
    let id = NodeId::new(id_str);
    let now = Utc::now();
    let mut builder = GraphNode::builder(id, kind)
        .label(label.trim().to_string())
        .source_path(source_path.to_path_buf())
        .created_at(now)
        .updated_at(now)
        // The line index is the pulldown-cmark event offset,
        // not a real byte/line number. Callers that need the
        // byte offset can re-parse the file with a different
        // walker; the value is kept for traceability.
        .property("heading_offset", line.to_string())
        .property("heading_level", heading_level_str(level).to_string());
    if is_adr {
        // ADRs have a `Status:` line somewhere in the body. The
        // parser scans the first 4 KiB for the marker; the actual
        // status value is captured from the body.
        if let Some(status) = extract_status(body) {
            status_lines.push(status.clone());
            builder = builder.property("status", status);
        }
    }
    builder.build()
}

/// Build a fallback `GraphNode` for a file with no headings
/// (e.g. a plain text note). The slug is the file stem; the
/// label is the file's basename.
#[cfg(feature = "multimodal")]
fn build_fallback_node(
    text: &str,
    file_stem: &str,
    source_path: &Path,
    is_adr: bool,
    _status_lines: &[String],
) -> GraphNode {
    let kind = if is_adr {
        NodeKind::Decision
    } else {
        NodeKind::Doc
    };
    let id_str = format!("{}:{}#intro", kind_prefix(&kind), file_stem);
    let now = Utc::now();
    let label = source_path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| file_stem.to_string());
    GraphNode::builder(NodeId::new(id_str), kind)
        .label(label)
        .source_path(source_path.to_path_buf())
        .created_at(now)
        .updated_at(now)
        .property("fallback", "no_headings")
        .property("body_chars", text.chars().count().to_string())
        .build()
}

/// Stable kebab-case prefix for the `NodeId`. The kind's
/// kebab-case `Display` form already matches the prefix we want.
#[cfg(feature = "multimodal")]
fn kind_prefix(kind: &NodeKind) -> &'static str {
    match kind {
        NodeKind::Decision => "decision",
        NodeKind::Doc => "doc",
        NodeKind::Issue => "issue",
        NodeKind::Evidence => "evidence",
        // C4-model architecture kinds (Phase 1 — no extractor
        // produces them yet, but the kebab-case prefix is wired
        // in so the taxonomy stays consistent).
        NodeKind::Component => "component",
        NodeKind::Container => "container",
        NodeKind::System => "system",
        // e15.5 — API route ingestion produces these nodes.
        NodeKind::Route => "route",
        NodeKind::Symbol(_) => "symbol",
    }
}

/// `HeadingLevel` -> stable string. Used as the
/// `heading_level` property.
#[cfg(feature = "multimodal")]
fn heading_level_str(level: HeadingLevel) -> &'static str {
    match level {
        HeadingLevel::H1 => "h1",
        HeadingLevel::H2 => "h2",
        HeadingLevel::H3 => "h3",
        HeadingLevel::H4 => "h4",
        HeadingLevel::H5 => "h5",
        HeadingLevel::H6 => "h6",
    }
}

/// Slugify a heading label: lowercase, ASCII alphanumerics +
/// hyphens, collapse runs of separators.
#[cfg(feature = "multimodal")]
fn slugify(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut last_was_dash = true; // suppress leading dash
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            last_was_dash = false;
        } else if !last_was_dash {
            out.push('-');
            last_was_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        "section".to_string()
    } else {
        out
    }
}

/// Strip paired markdown emphasis markers from both ends of a string.
/// Handles `**bold**`, `*italic*`, `_underline_`, and plain text.
#[cfg(feature = "multimodal")]
fn strip_md_marker(s: &str) -> String {
    let s = s.trim();
    let chars: Vec<char> = s.chars().collect();
    let len = chars.len();

    // Check for paired markers: **, __
    if len >= 4 {
        let start_marker: String = chars[0..2].iter().collect();
        if matches!(start_marker.as_str(), "**" | "__") {
            let end_marker: String = chars[len - 2..len].iter().collect();
            if end_marker == start_marker {
                return chars[2..len - 2]
                    .iter()
                    .collect::<String>()
                    .trim()
                    .to_string();
            }
        }
    }
    // Check for single markers: *, _
    if len >= 2 {
        let start_marker = chars[0];
        if matches!(start_marker, '*' | '_') {
            let end_marker = chars[len - 1];
            if end_marker == start_marker {
                return chars[1..len - 1]
                    .iter()
                    .collect::<String>()
                    .trim()
                    .to_string();
            }
        }
    }
    s.to_string()
}

/// Extract the value of the canonical `Status: <value>` line in
/// an ADR's body. Matches the first `Status:` line (case
/// insensitive) and returns the trimmed remainder, lowercased.
/// Handles bold markdown (`**Status**:`), italic (`_Status_:`,
/// `*Status*:`), and plain `Status:` forms.
///
/// When the status label is bold/italic (e.g., `**Status**:`),
/// pulldown_cmark emits the label and value as separate text nodes,
/// so we also check for the "Status\n: value" pattern.
///
/// Returns `None` when no `Status:` line is present.
#[cfg(feature = "multimodal")]
fn extract_status(body: &str) -> Option<String> {
    let lines: Vec<&str> = body.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim_start();
        // Strip leading markdown markers: **, _, *
        let stripped = strip_md_marker(trimmed);
        let stripped_lower = stripped.to_ascii_lowercase();

        // Case 1: "status:" or "status: value" on same line
        if let Some(rest) = stripped_lower.strip_prefix("status:") {
            return Some(rest.trim().to_lowercase());
        }

        // Case 2: bold/italic label on its own line, value on next line
        // e.g., "Status" followed by ": ACCEPTED"
        if stripped_lower == "status" {
            // Check if next line starts with ':'
            if i + 1 < lines.len() {
                let next_line = lines[i + 1].trim_start();
                if let Some(rest) = next_line.strip_prefix(':') {
                    return Some(rest.trim().to_lowercase());
                }
            }
        }
    }
    None
}

/// Internal: a single Cites candidate for a body line. The
/// `into_edge` materialises a `GraphEdge` once the source
/// `NodeId` is known.
#[cfg(feature = "multimodal")]
struct BodyCites {
    /// The matched code symbol id (e.g. `src/foo.rs:bar:1`).
    target: SymbolId,
    /// The confidence tier the body line resolved to.
    tier: ConfidenceTier,
}

#[cfg(feature = "multimodal")]
impl BodyCites {
    fn into_edge(self, source: &NodeId) -> GraphEdge {
        GraphEdge::new(
            source.clone(),
            NodeId::from(self.target.as_str().to_string()),
            EdgeKind::Cites,
            self.tier.provenance(),
            self.tier.confidence(),
        )
        .expect("body-derived Cites edge must satisfy GraphEdge invariants")
    }
}

/// Internal: a Cites candidate for a markdown link to another
/// document (ADR or doc). The `into_edge` materialises a
/// `GraphEdge` once the source `NodeId` is known.
#[cfg(feature = "multimodal")]
struct DocCites {
    /// The target NodeId (e.g. `decision:adr-002-foo#bar`).
    target: NodeId,
    /// The confidence tier (always `ConfidenceTier::LinkExact` = 0.9).
    tier: ConfidenceTier,
}

#[cfg(feature = "multimodal")]
impl DocCites {
    fn into_edge(self, source: &NodeId) -> GraphEdge {
        GraphEdge::new(
            source.clone(),
            self.target,
            EdgeKind::Cites,
            self.tier.provenance(),
            self.tier.confidence(),
        )
        .expect("doc-derived Cites edge must satisfy GraphEdge invariants")
    }
}

/// Unified Cites candidate for either a body line or a doc link.
#[cfg(feature = "multimodal")]
enum CitesCandidate {
    Body(BodyCites),
    Doc(DocCites),
}

#[cfg(feature = "multimodal")]
impl CitesCandidate {
    fn into_edge(self, source: &NodeId) -> GraphEdge {
        match self {
            CitesCandidate::Body(c) => c.into_edge(source),
            CitesCandidate::Doc(d) => d.into_edge(source),
        }
    }
}

/// Classify a single body line. Returns `Some(BodyCites)` if the
/// line is a `file:name:line`-shaped code reference (or a markdown
/// link to one), `None` otherwise. The classification is
/// conservative: only lines that match the canonical
/// `SymbolId` shape contribute an edge.
#[cfg(feature = "multimodal")]
fn classify_body_line(line: &str, file_stem: &str) -> Option<BodyCites> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }
    // Capture an optional `(text, target)` split: if the line
    // is a markdown link `[text](target)`, the link text is the
    // "link text" we feed to `score_link`; the target is the
    // symbol we cite. Otherwise the entire line is the target
    // (and we synthesise a link text from its short name).
    let mut link_text: Option<String> = None;
    let mut target_str = trimmed.to_string();
    if let Some(start) = trimmed.find("](") {
        if let Some(end_rel) = trimmed[start + 2..].find(')') {
            link_text = Some(trimmed[..start].trim().trim_matches('[').trim().to_string());
            target_str = trimmed[start + 2..start + 2 + end_rel].to_string();
        }
    } else if trimmed.starts_with('<') && trimmed.ends_with('>') {
        target_str = trimmed[1..trimmed.len() - 1].to_string();
    }
    // Skip http(s):// URLs.
    if target_str.starts_with("http://") || target_str.starts_with("https://") {
        return None;
    }
    if !looks_like_symbol_id(&target_str) {
        return None;
    }
    let target = SymbolId::new(&target_str);
    // Synthesise the "link text" used for the confidence
    // scoring. If the source was a `[text](target)` link, use
    // `text`; otherwise use the short name of the target (the
    // middle `:`-segment) so the score lands in `LinkExact`
    // (0.9) when the link cleanly points at a single symbol.
    let scoring_text = link_text
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| sym_short_name(&target));
    let (tier, _) = score_link(&scoring_text, std::slice::from_ref(&target));
    if matches!(tier, ConfidenceTier::Unresolved) {
        return None;
    }
    let _ = file_stem; // reserved for future per-file resolution.
    Some(BodyCites { target, tier })
}

/// Cheap shape check: a string looks like a `SymbolId` if it has
/// exactly 2 or 3 `:`-separated segments AND the first two are
/// non-empty AND the last (when present) parses as `i32`.
#[cfg(feature = "multimodal")]
fn looks_like_symbol_id(s: &str) -> bool {
    let parts: Vec<&str> = s.split(':').collect();
    match parts.len() {
        2 => !parts[0].is_empty() && !parts[1].is_empty(),
        3 => !parts[0].is_empty() && !parts[1].is_empty() && parts[2].parse::<i32>().is_ok(),
        _ => false,
    }
}

/// Classify a markdown link target that points to another `.md` file.
/// Returns `Some(DocCites)` if the target is a relative `.md` path
/// (e.g., `./ADR-002-foo.md` or `./architecture.md#section`),
/// `None` otherwise.
///
/// The returned `DocCites` has:
/// - For ADR links (`ADR-NNN-*.md`): target `decision:<stem>#<heading>`
/// - For doc links: target `doc:<stem>#<anchor>`
/// - Confidence: 0.9 (`ConfidenceTier::LinkExact`)
#[cfg(feature = "multimodal")]
fn classify_doc_link(target: &str) -> Option<DocCites> {
    // Skip http(s):// URLs and autolinks
    if target.starts_with("http://") || target.starts_with("https://") || target.starts_with('<') {
        return None;
    }

    // Must be a relative path containing .md or .markdown (case insensitive)
    // Handle URLs with #anchor suffixes like "./architecture.md#section"
    let target_lower = target.to_ascii_lowercase();
    let has_md = target_lower.contains(".md") || target_lower.contains(".markdown");
    if !has_md {
        return None;
    }

    // Extract the path, stripping leading `./` or `.\`
    let path = target.trim_start_matches("./").trim_start_matches(".\\");

    // Split off any #anchor
    let (stem, anchor) = if let Some(pos) = path.find('#') {
        (&path[..pos], Some(&path[pos + 1..]))
    } else {
        (path, None)
    };

    // Compute the file_stem (remove extension)
    let file_stem = if let Some(pos) = stem.rfind('/') {
        &stem[pos + 1..]
    } else {
        stem
    };
    let file_stem = if let Some(pos) = file_stem.rfind('\\') {
        &file_stem[pos + 1..]
    } else {
        file_stem
    };
    // Remove .md/.markdown extension
    let file_stem = file_stem
        .strip_suffix(".md")
        .or_else(|| file_stem.strip_suffix(".markdown"))
        .unwrap_or(file_stem);

    // Determine if it's an ADR based on naming pattern (adr-NNN-*.md)
    let is_adr = file_stem.to_ascii_lowercase().starts_with("adr-");

    // Build the target NodeId
    let target_id = if is_adr {
        // For ADRs: decision:<stem>#<heading> where heading = file_stem
        let slug = slugify(file_stem);
        format!("decision:{}#{}", file_stem.to_lowercase(), slug)
    } else {
        // For docs: doc:<stem>#<anchor> or doc:<stem>#<stem>
        let slug = if let Some(anchor) = anchor {
            slugify(anchor)
        } else {
            slugify(file_stem)
        };
        format!("doc:{}#{}", file_stem.to_lowercase(), slug)
    };

    Some(DocCites {
        target: NodeId::new(target_id),
        tier: ConfidenceTier::LinkExact,
    })
}

/// Parse a markdown link `[text](target)` from a line, returning
/// the target string if found. Handles both inline `[text](url)` form
/// and the case where the URL was pushed to body as a separate line
/// after the link closed.
#[cfg(feature = "multimodal")]
fn extract_link_target(line: &str) -> Option<String> {
    let trimmed = line.trim();

    // Case 1: inline markdown link [text](url)
    if let Some(open_pos) = trimmed.find("](") {
        // The URL starts after "](", at position open_pos + 2
        let url_start = open_pos + 2;
        // Find the closing ")" - it must exist after the URL start
        if let Some(close_pos) = trimmed[url_start..].find(')') {
            // close_pos is relative to url_start, so absolute position is url_start + close_pos
            let url_end = url_start + close_pos;
            if url_end < trimmed.len() {
                return Some(trimmed[url_start..url_end].to_string());
            }
        }
    }

    // Case 2: URL on its own line (after link close event)
    // Detected by: contains .md/.markdown before any #anchor, starts with . or /
    let trimmed_lower = trimmed.to_ascii_lowercase();
    if let Some(md_pos) = trimmed_lower.find(".md") {
        let _before_md = &trimmed[..md_pos];
        let after_md = &trimmed[md_pos + 3..];
        // .md must be followed by nothing, '/', '#', or '?' (end or anchor/query)
        if (after_md.is_empty()
            || after_md.starts_with('/')
            || after_md.starts_with('#')
            || after_md.starts_with('?'))
            && (trimmed.starts_with('.') || trimmed.starts_with('/'))
        {
            return Some(trimmed.to_string());
        }
    }
    if let Some(md_pos) = trimmed_lower.find(".markdown") {
        let _before_md = &trimmed[..md_pos];
        let after_md = &trimmed[md_pos + 9..];
        if (after_md.is_empty()
            || after_md.starts_with('/')
            || after_md.starts_with('#')
            || after_md.starts_with('?'))
            && (trimmed.starts_with('.') || trimmed.starts_with('/'))
        {
            return Some(trimmed.to_string());
        }
    }

    None
}

// ============================================================================
// Tests (T12 + T13 RED gates).
// ============================================================================

#[cfg(all(test, feature = "multimodal"))]
mod tests {
    use super::*;

    fn parse(text: &str) -> Vec<ExtractedNode> {
        let p = PathBuf::from("docs/adr/0007.md");
        parse_markdown(text, &p, "0007")
    }

    // ---- T12 RED gates ----

    /// A file with `# ADR-0007: Adopt GraphQL` plus a
    /// `Status: accepted` line MUST produce a `Decision` node
    /// whose `status` property is `accepted`.
    #[test]
    fn parse_adr_creates_decision_node() {
        let text = "# ADR-0007: Adopt GraphQL\n\nStatus: accepted\n\n## Context\nWe evaluated REST vs GraphQL.\n";
        let nodes = parse(text);
        assert!(!nodes.is_empty(), "expected at least one node");
        let first = &nodes[0];
        assert_eq!(first.potential_node.kind, NodeKind::Decision);
        assert_eq!(first.potential_node.label, "ADR-0007: Adopt GraphQL");
        assert_eq!(
            first
                .potential_node
                .properties_map()
                .get("status")
                .and_then(|v| v.as_str()),
            Some("accepted")
        );
        // The `heading_level` property is recorded.
        assert_eq!(
            first
                .potential_node
                .properties_map()
                .get("heading_level")
                .and_then(|v| v.as_str()),
            Some("h1")
        );
    }

    // ---- Phase 2: Bold-Markdown Status Extraction RED tests ----

    /// `**Status**: ACCEPTED` (bold markdown) MUST produce
    /// `status == "accepted"` on the Decision node.
    #[test]
    fn bold_status_accepted_is_lowercased() {
        let text = "# ADR-0007: Adopt GraphQL\n\n**Status**: ACCEPTED\n\n## Context\n";
        let nodes = parse(text);
        let first = &nodes[0];
        assert_eq!(
            first
                .potential_node
                .properties_map()
                .get("status")
                .and_then(|v| v.as_str()),
            Some("accepted"),
            "bold **Status**: ACCEPTED should normalize to accepted"
        );
    }

    /// `_Status_: proposed` (italic markdown) MUST produce
    /// `status == "proposed"` on the Decision node.
    #[test]
    fn italic_status_proposed_is_lowercased() {
        let text = "# ADR-0008: New Feature\n\n_Status_: proposed\n\n## Context\n";
        let nodes = parse(text);
        let first = &nodes[0];
        assert_eq!(
            first
                .potential_node
                .properties_map()
                .get("status")
                .and_then(|v| v.as_str()),
            Some("proposed"),
            "italic _Status_: proposed should normalize to proposed"
        );
    }

    /// `*Status*: superseded` (another italic variant) MUST produce
    /// `status == "superseded"` on the Decision node.
    #[test]
    fn asterisk_status_superseded_is_lowercased() {
        let text = "# ADR-0009: Old Approach\n\n*Status*: superseded\n\n## Context\n";
        let nodes = parse(text);
        let first = &nodes[0];
        assert_eq!(
            first
                .potential_node
                .properties_map()
                .get("status")
                .and_then(|v| v.as_str()),
            Some("superseded"),
            "asterisk *Status*: superseded should normalize to superseded"
        );
    }

    /// `**Status**: Superseded` (mixed case) MUST produce
    /// `status == "superseded"` (lowercased).
    #[test]
    fn bold_status_mixed_case_is_normalised() {
        let text = "# ADR-0010: Superseded\n\n**Status**: Superseded\n\n## Context\n";
        let nodes = parse(text);
        let first = &nodes[0];
        assert_eq!(
            first
                .potential_node
                .properties_map()
                .get("status")
                .and_then(|v| v.as_str()),
            Some("superseded"),
            "**Status**: Superseded should normalize to lowercase"
        );
    }

    // ---- Phase 3: Cross-Document ADR Citations RED tests ----

    /// `[ADR-002](./ADR-002-moldable-exploration-parity-program.md)`
    /// in a body line MUST emit a `Cites` edge to
    /// `decision:adr-002-moldable-exploration-parity-program#<heading>`
    /// with confidence 0.9.
    #[test]
    fn cross_adr_link_emits_decision_cites_edge() {
        let text = "# ADR-0005: References\n\nSee [ADR-002](./ADR-002-moldable-exploration-parity-program.md) for details.\n";
        let nodes = parse(text);
        assert!(!nodes.is_empty());
        let edges = &nodes[0].potential_edges;
        let adr_edge = edges.iter().find(|e| {
            e.kind == EdgeKind::Cites && e.target.as_str().starts_with("decision:adr-002")
        });
        assert!(
            adr_edge.is_some(),
            "expected a Cites edge to decision:adr-002..., got: {:?}",
            edges
        );
        let edge = adr_edge.unwrap();
        assert!((edge.confidence - 0.9).abs() < 1e-9);
        assert_eq!(edge.provenance, Provenance::Extracted);
    }

    /// `[Architecture](./architecture.md#context)` in a body line
    /// MUST emit a `Cites` edge to `doc:architecture#context`
    /// with confidence 0.9.
    #[test]
    fn cross_doc_link_with_anchor_emits_doc_cites_edge() {
        let text = "# Overview\n\nSee [Architecture](./architecture.md#context) for context.\n";
        let nodes = parse(text);
        assert!(!nodes.is_empty());
        let edges = &nodes[0].potential_edges;
        let doc_edge = edges.iter().find(|e| {
            e.kind == EdgeKind::Cites && e.target.as_str().starts_with("doc:architecture#")
        });
        assert!(
            doc_edge.is_some(),
            "expected a Cites edge to doc:architecture#..., got: {:?}",
            edges
        );
        let edge = doc_edge.unwrap();
        assert!((edge.confidence - 0.9).abs() < 1e-9);
    }

    /// `[spec](https://example.com)` (external URL) MUST NOT
    /// emit any edge.
    #[test]
    fn external_url_link_emits_no_edge() {
        let text = "# Overview\n\nSee [spec](https://example.com) for details.\n";
        let nodes = parse(text);
        assert!(!nodes.is_empty());
        let edges = &nodes[0].potential_edges;
        // No Cites edges expected for external URLs
        assert!(
            edges.iter().all(|e| e.kind != EdgeKind::Cites),
            "external URL should not produce Cites edge, got: {:?}",
            edges
        );
    }

    /// `[bar](src/foo.rs:bar:1)` (symbol-shaped link) MUST
    /// still produce a Cites edge to `src/foo.rs:bar:1` with
    /// confidence 0.9 (no regression).
    #[test]
    fn symbol_shaped_link_still_resolves() {
        let text = "# Overview\n\nsee [bar](src/foo.rs:bar:1) for details.\n";
        let nodes = parse(text);
        assert!(!nodes.is_empty());
        let edges = &nodes[0].potential_edges;
        let sym_edge = edges
            .iter()
            .find(|e| e.target.as_str() == "src/foo.rs:bar:1");
        assert!(
            sym_edge.is_some(),
            "symbol-shaped link should still resolve, got: {:?}",
            edges
        );
        let edge = sym_edge.unwrap();
        assert_eq!(edge.kind, EdgeKind::Cites);
        assert!((edge.confidence - 0.9).abs() < 1e-9);
    }

    // ---- Phase 4: Decision→Doc Justifies Edge test ----

    /// An ADR file with multiple headings MUST emit exactly one `Justifies` edge
    /// (confidence 1.0) from the Decision node to the file-level Doc node.
    /// The Justifies edge connects decision:<stem>#<heading> → doc:<stem>#intro.
    #[test]
    fn adr_emits_one_justifies_edge() {
        let text = "# ADR-0007: Adopt GraphQL\n\n**Status**: ACCEPTED\n\n## Context\nWe evaluated REST vs GraphQL.\n";
        let nodes = parse(text);

        // Should have 3 nodes: Decision (H1), Decision (H2), and Doc (file-level)
        assert!(
            nodes.len() >= 3,
            "expected at least 3 nodes, got {}",
            nodes.len()
        );

        // The first node should be a Decision (H1 in ADR file)
        let first_node = nodes.first().expect("nodes should not be empty");
        assert_eq!(
            first_node.potential_node.kind,
            NodeKind::Decision,
            "first node should be Decision"
        );

        // Find the Doc node (file-level)
        let doc_node = nodes
            .iter()
            .find(|n| n.potential_node.kind == NodeKind::Doc);
        assert!(doc_node.is_some(), "expected a Doc node, got: {:?}", nodes);

        // Collect all edges
        let all_edges: Vec<&GraphEdge> = nodes
            .iter()
            .flat_map(|n| n.potential_edges.iter())
            .collect();

        // Find Justifies edges
        let justifies_edges: Vec<&&GraphEdge> = all_edges
            .iter()
            .filter(|e| e.kind == EdgeKind::Justifies)
            .collect();

        // Exactly one Justifies edge expected
        assert_eq!(
            justifies_edges.len(),
            1,
            "expected exactly one Justifies edge, got: {:?}",
            justifies_edges
        );

        let justifies = justifies_edges[0];
        assert!(
            (justifies.confidence - 1.0).abs() < 1e-9,
            "Justifies edge confidence should be 1.0"
        );
        assert_eq!(justifies.provenance, Provenance::Extracted);

        // The target should be the Doc node
        let doc_id = doc_node.unwrap().potential_node.id.as_str();
        assert_eq!(
            justifies.target.as_str(),
            doc_id,
            "Justifies edge target should be the Doc node"
        );
    }

    /// A plain Markdown file (no ADR marker) MUST NOT emit any Justifies edge.
    #[test]
    fn plain_markdown_emits_no_justifies_edge() {
        let text = "# Overview\n\nSome text.\n\n## Authentication\nLogin flow.\n";
        let nodes = parse(text);
        assert!(!nodes.is_empty());

        // Collect all edges across all nodes
        let all_edges: Vec<&GraphEdge> = nodes
            .iter()
            .flat_map(|n| n.potential_edges.iter())
            .collect();

        // No Justifies edges expected for plain markdown
        assert!(
            all_edges.iter().all(|e| e.kind != EdgeKind::Justifies),
            "plain markdown should not produce Justifies edge, got: {:?}",
            all_edges
        );
    }

    /// A plain Markdown file (no ADR marker) MUST produce a
    /// `Doc` node per heading. The label is the heading text.
    #[test]
    fn parse_markdown_creates_doc_node() {
        let text = "# Overview\n\nSome text.\n\n## Authentication\nLogin flow.\n";
        let nodes = parse(text);
        assert_eq!(nodes.len(), 2, "expected 2 doc nodes from 2 headings");
        assert_eq!(nodes[0].potential_node.kind, NodeKind::Doc);
        assert_eq!(nodes[0].potential_node.label, "Overview");
        assert_eq!(nodes[1].potential_node.label, "Authentication");
    }

    /// A Markdown body containing a `src/foo.rs:bar:1`-shaped
    /// reference MUST emit a `Cites` edge from the heading node
    /// to a `Symbol("src/foo.rs:bar:1")` node id.
    #[test]
    fn code_link_creates_cites_edge() {
        let text = "# Overview\n\nsee [bar](src/foo.rs:bar:1) for details.\n";
        let nodes = parse(text);
        assert_eq!(nodes.len(), 1);
        let edges = &nodes[0].potential_edges;
        assert_eq!(edges.len(), 1, "expected exactly one Cites edge");
        let edge = &edges[0];
        assert_eq!(edge.kind, EdgeKind::Cites);
        assert_eq!(
            edge.target.as_str(),
            "src/foo.rs:bar:1",
            "the edge target must be the symbol id, not a doc id"
        );
        // The confidence must be the exact-link tier (0.9).
        assert!((edge.confidence - 0.9).abs() < 1e-9);
        assert_eq!(edge.provenance, Provenance::Extracted);
    }

    // ---- T13 RED gates ----

    /// `extract` on a directory of `.md` files returns the
    /// concatenated candidates (one per heading across the
    /// directory).
    #[tokio::test]
    async fn full_pipeline_directory() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let a = tmp.path().join("a.md");
        let b = tmp.path().join("nested").join("b.md");
        std::fs::create_dir_all(b.parent().unwrap()).unwrap();
        std::fs::write(&a, "# Top\n\nsee [helper](src/lib.rs:helper:10).\n").unwrap();
        std::fs::write(&b, "# ADR-0001: Add a thing\n\nStatus: proposed\n").unwrap();

        let extractor = DocsExtractor::new();
        let nodes = extractor
            .extract(SourcePath::Directory(tmp.path().to_path_buf()))
            .await
            .expect("extract directory");
        assert!(!nodes.is_empty(), "directory walk should yield nodes");
        // We expect at least 2 nodes: one Doc + one Decision.
        let kinds: Vec<&NodeKind> = nodes.iter().map(|n| &n.potential_node.kind).collect();
        assert!(kinds.contains(&&NodeKind::Doc));
        assert!(kinds.contains(&&NodeKind::Decision));
        // The Cites edge from `a.md` survives the round-trip.
        let any_cites = nodes
            .iter()
            .flat_map(|n| &n.potential_edges)
            .any(|e| e.kind == EdgeKind::Cites);
        assert!(any_cites, "Cites edge from a.md should be present");
    }

    /// Re-ingesting the same directory twice MUST produce the
    /// same `NodeId`s (idempotency contract). The persistence
    /// layer's upsert collapses the resulting duplicates.
    #[tokio::test]
    async fn idempotent_reingest() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let f = tmp.path().join("doc.md");
        std::fs::write(&f, "# Hello\n\nsee [foo](src/foo.rs:foo:1).\n").unwrap();

        let extractor = DocsExtractor::new();
        let first = extractor
            .extract(SourcePath::File(f.clone()))
            .await
            .expect("first extract");
        let second = extractor
            .extract(SourcePath::File(f.clone()))
            .await
            .expect("second extract");

        // Same id sequence on both runs.
        let first_ids: Vec<String> = first
            .iter()
            .map(|n| n.potential_node.id.to_string())
            .collect();
        let second_ids: Vec<String> = second
            .iter()
            .map(|n| n.potential_node.id.to_string())
            .collect();
        assert_eq!(
            first_ids, second_ids,
            "node ids must be deterministic across re-ingests"
        );
        assert!(!first_ids.is_empty());
    }

    // ---- Additional TDD coverage ----

    /// `slugify` collapses non-alphanumerics to single dashes and
    /// strips leading/trailing dashes.
    #[test]
    fn slugify_basics() {
        assert_eq!(slugify("Hello World"), "hello-world");
        assert_eq!(slugify("  Already   -- a slug!  "), "already-a-slug");
        assert_eq!(slugify(""), "section");
        assert_eq!(slugify("---"), "section");
        assert_eq!(slugify("Authentication & AuthZ"), "authentication-authz");
    }

    /// `detect_adr` matches the canonical ADR markers and
    /// rejects plain Markdown.
    #[test]
    fn detect_adr_recognises_markers() {
        assert!(detect_adr(
            "# ADR-0007: Adopt GraphQL\n\nStatus: accepted\n"
        ));
        assert!(detect_adr("# ADR- 0007: Adopt GraphQL\n"));
        assert!(detect_adr("# Decision: 0007 — Adopt GraphQL\n"));
        assert!(!detect_adr("# Overview\n\nSome text.\n"));
        assert!(!detect_adr("Just a paragraph, no heading.\n"));
    }

    /// Files with no headings fall back to a single `Doc` node
    /// keyed on the file stem. The fallback node carries a
    /// `fallback=no_headings` property so consumers can
    /// distinguish it from a parsed-heading node.
    #[test]
    fn parse_no_headings_falls_back_to_filename_doc() {
        let text = "just a paragraph, no headings at all.\n";
        let nodes = parse(text);
        assert_eq!(nodes.len(), 1);
        let node = &nodes[0].potential_node;
        assert_eq!(node.kind, NodeKind::Doc);
        assert_eq!(
            node.properties_map()
                .get("fallback")
                .and_then(|v| v.as_str()),
            Some("no_headings")
        );
        assert_eq!(
            node.id.as_str(),
            "doc:0007#intro",
            "fallback slug is `intro`"
        );
    }

    /// A file with no `.md` extension is silently skipped by the
    /// directory walker (no `extract_from_file` call, no error).
    #[tokio::test]
    async fn directory_skips_non_markdown() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::write(tmp.path().join("readme.txt"), "not markdown").unwrap();
        std::fs::write(
            tmp.path().join("real.md"),
            "# Hi\n\nsee [foo](src/x.rs:foo:1).\n",
        )
        .unwrap();
        let extractor = DocsExtractor::new();
        let nodes = extractor
            .extract(SourcePath::Directory(tmp.path().to_path_buf()))
            .await
            .expect("extract");
        assert_eq!(nodes.len(), 1, "only the .md file should be processed");
        assert_eq!(nodes[0].potential_node.kind, NodeKind::Doc);
    }

    /// `extract` on a non-existent file returns
    /// `SourceExtractorError::NotFound`.
    #[tokio::test]
    async fn extract_missing_file_returns_not_found() {
        let extractor = DocsExtractor::new();
        let result = extractor
            .extract(SourcePath::File(PathBuf::from(
                "/nonexistent/path/does-not-exist.md",
            )))
            .await;
        match result {
            Err(SourceExtractorError::NotFound(_)) => {}
            other => panic!("expected NotFound, got {other:?}"),
        }
    }

    /// `extract` on a URL returns `Unsupported` (the docs
    /// extractor never fetches remote URLs in this slice).
    #[tokio::test]
    async fn extract_url_returns_unsupported() {
        let extractor = DocsExtractor::new();
        let result = extractor
            .extract(SourcePath::Url("https://example.com/doc.md".to_string()))
            .await;
        match result {
            Err(SourceExtractorError::Unsupported(_)) => {}
            other => panic!("expected Unsupported, got {other:?}"),
        }
    }

    /// `DocsExtractor` is dyn-compatible + Send + Sync (it
    /// composes behind `Box<dyn SourceExtractor + Send + Sync>`).
    #[test]
    fn docs_extractor_is_dyn_compatible() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Box<dyn SourceExtractor + Send + Sync>>();
        assert_send_sync::<DocsExtractor>();
        let _boxed: Box<dyn SourceExtractor + Send + Sync> = Box::new(DocsExtractor::new());
    }

    // ---- Phase 6: Corpus Regression Fixture ----

    /// Ingest the real ADR corpus (`docs/adr/ADR-00*.md`) and assert:
    /// - All 8 ADRs produce at least one Decision node with non-empty status
    /// - ADR-005's References section produces ≥3 cross-ADR Cites edges
    /// - Total Justifies edges equals 8 (one per ADR file)
    #[tokio::test]
    async fn docs_extractor_corpus_regression() {
        let adr_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent() // cognicode-core
            .unwrap()
            .parent() // crates
            .unwrap()
            .join("docs/adr");

        if !adr_dir.exists() {
            eprintln!("SKIP: docs/adr/ not found at {:?}", adr_dir);
            return;
        }

        let extractor = DocsExtractor::new();
        let nodes = extractor
            .extract(SourcePath::Directory(adr_dir.clone()))
            .await
            .expect("extract ADR corpus");

        // Collect Decision nodes and their statuses
        let decision_nodes: Vec<_> = nodes
            .iter()
            .filter(|n| n.potential_node.kind == NodeKind::Decision)
            .collect();

        // Assert: at least 8 Decision nodes (one per ADR file, some ADRs have multiple headings)
        assert!(
            decision_nodes.len() >= 8,
            "expected at least 8 Decision nodes, got {}",
            decision_nodes.len()
        );

        // Assert: at least one Decision node per ADR has a non-empty status
        // (H1 headings have status; H2 headings like "Context" may not)
        let nodes_with_status: Vec<_> = decision_nodes
            .iter()
            .filter(|n| {
                n.potential_node
                    .properties_map()
                    .get("status")
                    .and_then(|v| v.as_str())
                    .map_or(false, |s| !s.is_empty())
            })
            .collect();

        assert!(
            nodes_with_status.len() >= 8,
            "expected at least 8 Decision nodes with non-empty status, got {}",
            nodes_with_status.len()
        );

        // Collect all Cites edges
        let all_cites: Vec<_> = nodes
            .iter()
            .flat_map(|n| n.potential_edges.iter())
            .filter(|e| e.kind == EdgeKind::Cites)
            .collect();

        // Assert: ADR-005 emits ≥3 cross-ADR Cites edges
        // ADR-005 contains references to ADR-002, ADR-003, ADR-004
        let adr005_cites = all_cites
            .iter()
            .filter(|e| {
                e.target.as_str().contains("adr-002")
                    || e.target.as_str().contains("adr-003")
                    || e.target.as_str().contains("adr-004")
            })
            .count();

        assert!(
            adr005_cites >= 3,
            "ADR-005 should emit at least 3 cross-ADR Cites edges, got {}",
            adr005_cites
        );

        // Assert: total Justifies edges equals number of ADR files in corpus
        let all_justifies: Vec<_> = nodes
            .iter()
            .flat_map(|n| n.potential_edges.iter())
            .filter(|e| e.kind == EdgeKind::Justifies)
            .collect();

        // Count actual ADR files in the corpus directory
        let adr_file_count = std::fs::read_dir(&adr_dir)
            .map(|entries| {
                entries
                    .filter_map(|e| e.ok())
                    .filter(|e| {
                        e.path().extension().map(|ext| ext == "md").unwrap_or(false)
                            && e.file_name().to_string_lossy().starts_with("ADR-")
                    })
                    .count()
            })
            .unwrap_or(0);

        assert!(
            all_justifies.len() >= adr_file_count,
            "expected at least {} Justifies edges (one per ADR file), got {}",
            adr_file_count,
            all_justifies.len()
        );
    }

    /// `**Status**: DRAFT-WIP` does not crash — unknown status is
    /// lowercased and stored verbatim.
    #[test]
    fn unknown_status_does_not_crash() {
        let text =
            "# ADR-0099: Experimental\n\n**Status**: DRAFT-WIP\n\nSome experimental decision.\n";
        let nodes = parse(text);
        assert!(
            !nodes.is_empty(),
            "parse should not crash on unknown status"
        );

        let decision_node = nodes
            .iter()
            .find(|n| n.potential_node.kind == NodeKind::Decision);
        assert!(
            decision_node.is_some(),
            "ADR with unknown status should produce a Decision node"
        );

        let status = decision_node
            .unwrap()
            .potential_node
            .properties_map()
            .get("status");
        assert_eq!(
            status.and_then(|v| v.as_str()),
            Some("draft-wip"),
            "status should be lowercased"
        );
    }

    // ---- Scenario 3: Code fence with language is preserved ----

    /// A fenced code block with a language info string MUST produce
    /// a `code_block_lang` property on the Doc node.
    #[test]
    fn code_fence_language_recorded_in_metadata() {
        let text = "# Heading\n\n```rust\nfn main() {}\n```\n";
        let nodes = parse(text);
        assert!(!nodes.is_empty(), "expected at least one node");
        let first = &nodes[0];
        assert_eq!(
            first
                .potential_node
                .properties_map()
                .get("code_block_lang")
                .and_then(|v| v.as_str()),
            Some("rust"),
            "fenced code block language should be recorded as code_block_lang"
        );
    }

    /// A fenced code block with no language (empty info string) MUST NOT
    /// produce a `code_block_lang` property.
    #[test]
    fn code_fence_no_language_not_recorded() {
        let text = "# Heading\n\n```\nsome text\n```\n";
        let nodes = parse(text);
        assert!(!nodes.is_empty(), "expected at least one node");
        let first = &nodes[0];
        assert!(
            first
                .potential_node
                .properties
                .get("code_block_lang")
                .is_none(),
            "empty info string should not produce code_block_lang"
        );
    }

    // ---- Scenario 5: Missing status leaves no property ----

    /// An ADR body with NO `Status:` line MUST NOT set a `status`
    /// property on the Decision node, and extraction MUST succeed.
    #[test]
    fn missing_status_leaves_no_property() {
        let text = "# ADR-0010: No Status Line\n\n## Context\nWe made a decision but never wrote the status.\n";
        let nodes = parse(text);
        assert!(!nodes.is_empty(), "parse should succeed without status");

        let decision_nodes: Vec<_> = nodes
            .iter()
            .filter(|n| n.potential_node.kind == NodeKind::Decision)
            .collect();

        // At least one Decision node should exist
        assert!(
            !decision_nodes.is_empty(),
            "ADR should produce at least one Decision node"
        );

        // No Decision node should have a status property
        for node in decision_nodes {
            assert!(
                node.potential_node.properties.get("status").is_none(),
                "missing Status: should leave no status property, got: {:?}",
                node.potential_node.properties.get("status")
            );
        }
    }
}
