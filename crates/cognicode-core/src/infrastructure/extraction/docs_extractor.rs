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
use crate::domain::aggregates::generic_graph::{GraphEdge, GraphNode, NodeId};
#[cfg(feature = "multimodal")]
use crate::domain::aggregates::SymbolId;
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
use super::docs_confidence_rules::{
    heading_confidence, score_heading, score_link, sym_short_name, ConfidenceTier,
};

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
pub fn parse_markdown(
    text: &str,
    source_path: &Path,
    file_stem: &str,
) -> Vec<ExtractedNode> {
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

    /// Flush the accumulated `body` to the most recently
    /// emitted node by appending edges AND capturing any
    /// ADR-specific metadata (`Status:` line). If no node
    /// exists yet (e.g. body before the first heading), the body
    /// is dropped — in practice, the parser guarantees the body
    /// never precedes the first heading.
    fn flush_trailing_body(
        nodes: &mut Vec<ExtractedNode>,
        body: &mut String,
        file_stem: &str,
    ) {
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
            last.potential_node = last
                .potential_node
                .clone()
                .with_property("status", status);
        }
        for line in body.lines() {
            if let Some(cites) = classify_body_line(line, file_stem) {
                last.potential_edges.push(cites.into_edge(&source_id));
            }
        }
        body.clear();
    }

    for (event_offset, event) in parser.enumerate() {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                // Open a new heading — first, flush any
                // trailing body text from the previous heading
                // onto the previously emitted node.
                flush_trailing_body(&mut nodes, &mut current_body, file_stem);
                current_heading = Some((String::new(), event_offset, level));
            }
            Event::End(TagEnd::Heading(_)) => {
                // The heading is complete; emit a node with
                // whatever body has been accumulated so far.
                // The body buffer is NOT reset here — the
                // trailing body between this heading and the
                // next is attached to THIS node by the heading
                // arm above (or the EOF flush below).
                if let Some((label, line, lvl)) = current_heading.take() {
                    let body = std::mem::take(&mut current_body);
                    let node = build_heading_node(
                        &label,
                        line,
                        lvl,
                        file_stem,
                        source_path,
                        is_adr,
                        &body,
                        &mut status_lines,
                    );
                    let edges = body
                        .lines()
                        .filter_map(|line| classify_body_line(line, file_stem))
                        .map(|cites| cites.into_edge(&node.id))
                        .collect();
                    nodes.push(ExtractedNode::with_edges(node, edges));
                }
            }
            Event::Text(text) | Event::Code(text) => {
                if let Some((ref mut label, _, _)) = current_heading {
                    if code_text_buf.is_none() {
                        label.push_str(&text);
                    }
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
            Event::Start(Tag::Link { dest_url, link_type, .. }) => {
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
            Event::Start(Tag::CodeBlock(_)) => {
                code_text_buf = Some(String::new());
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

    // Flush any trailing body to the last emitted node. This
    // covers two cases:
    //   1. A heading is still open (no closing `End(Heading)`).
    //   2. The last heading closed, then trailing paragraphs
    //      accumulated in `current_body`.
    if current_heading.take().is_some() {
        // A heading is still open at EOF — emit it with the
        // body that was accumulated so far.
        let body = std::mem::take(&mut current_body);
        if let Some((label, line, lvl)) = None::<(String, usize, HeadingLevel)> {
            let _ = (label, line, lvl);
        }
        // The `current_heading.take()` above consumed the open
        // heading without emitting a node. Re-create the
        // heading from the buffer:
        // (degenerate case: the heading's label is empty)
        if !body.is_empty() {
            // No heading to attach to — fall back to a node-less
            // body flush is impossible. Instead, emit a fallback
            // file-level node (handled by the `if nodes.is_empty()`
            // branch below).
        }
    } else {
        // Heading already closed; just flush the trailing body
        // to the most recent node.
        flush_trailing_body(&mut nodes, &mut current_body, file_stem);
    }

    // If the file had headings but no top-level heading was
    // emitted (e.g. a single H1 followed by an EOF), emit a
    // fallback file-level node so the file is never silently
    // dropped.
    if nodes.is_empty() && !text.trim().is_empty() {
        let node = build_fallback_node(text, file_stem, source_path, is_adr, &mut status_lines);
        nodes.push(ExtractedNode::new(node));
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
    pub async fn extract_file(
        &self,
        file: &Path,
    ) -> SourceExtractorResult<Vec<ExtractedNode>> {
        extract_from_file(file).await
    }
}

#[cfg(feature = "multimodal")]
#[async_trait]
impl SourceExtractor for DocsExtractor {
    fn source_kind(&self) -> &'static str {
        "markdown"
    }

    async fn extract(
        &self,
        source: SourcePath,
    ) -> SourceExtractorResult<Vec<ExtractedNode>> {
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
async fn extract_from_directory(dir: &Path, recursive: bool) -> SourceExtractorResult<Vec<ExtractedNode>> {
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
        p.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).as_deref(),
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
    let kind = if is_adr { NodeKind::Decision } else { NodeKind::Doc };
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

/// Extract the value of the canonical `Status: <value>` line in
/// an ADR's body. Matches the first `Status:` line (case
/// insensitive) and returns the trimmed remainder. Returns
/// `None` when no `Status:` line is present.
#[cfg(feature = "multimodal")]
fn extract_status(body: &str) -> Option<String> {
    for line in body.lines() {
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed
            .to_ascii_lowercase()
            .strip_prefix("status:")
            .map(|s| s.to_string())
        {
            return Some(rest.trim().to_string());
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
    let (tier, _) = score_link(&scoring_text, &[target.clone()]);
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
        3 => {
            !parts[0].is_empty()
                && !parts[1].is_empty()
                && parts[2].parse::<i32>().is_ok()
        }
        _ => false,
    }
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
            first.potential_node.properties.get("status").map(String::as_str),
            Some("accepted")
        );
        // The `heading_level` property is recorded.
        assert_eq!(
            first.potential_node.properties.get("heading_level").map(String::as_str),
            Some("h1")
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
        std::fs::write(
            &a,
            "# Top\n\nsee [helper](src/lib.rs:helper:10).\n",
        )
        .unwrap();
        std::fs::write(
            &b,
            "# ADR-0001: Add a thing\n\nStatus: proposed\n",
        )
        .unwrap();

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
        std::fs::write(
            &f,
            "# Hello\n\nsee [foo](src/foo.rs:foo:1).\n",
        )
        .unwrap();

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
        let first_ids: Vec<String> = first.iter().map(|n| n.potential_node.id.to_string()).collect();
        let second_ids: Vec<String> = second.iter().map(|n| n.potential_node.id.to_string()).collect();
        assert_eq!(first_ids, second_ids, "node ids must be deterministic across re-ingests");
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
        assert!(detect_adr("# ADR-0007: Adopt GraphQL\n\nStatus: accepted\n"));
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
            node.properties.get("fallback").map(String::as_str),
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
            .extract(SourcePath::File(PathBuf::from("/nonexistent/path/does-not-exist.md")))
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
}
