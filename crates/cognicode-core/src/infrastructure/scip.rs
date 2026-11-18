//! Bounded, read-only decoding of SCIP indexes.
//!
//! The S3 tier answers code-intelligence questions from an index somebody
//! else produced. This module never runs an indexer: it decodes a payload
//! the caller supplies and refuses to serve it unless the caller binds it
//! to the identity envelope it expects.
//!
//! # Identity
//!
//! A SCIP index does not record the repository, the revision, the build
//! configuration or the scope it was produced for. [`ScipIndexBinding`] is
//! that missing envelope, supplied by whoever produced the index.
//! [`ScipIndex::parse_bound`] accepts an index only when the envelope
//! equals the one the caller expects, the payload digest matches the bytes,
//! and the index metadata agrees with the envelope. An index produced for
//! another revision, another build configuration, another indexer version
//! or another scope is refused, so it can never raise the precision of an
//! answer.
//!
//! # Absences
//!
//! A miss in the index is not evidence of absence: the index may not cover
//! the queried file, the symbol may be ambiguous at that position, or the
//! producer may not have emitted the relation. Every lookup therefore
//! answers `None` for a miss, and the pipeline answers from a lower tier.
//!
//! # Positions
//!
//! Only `UTF8CodeUnitOffsetFromLineStart` positions are accepted, so a
//! range is a half-open span of UTF-8 code units measured from the start of
//! its line — the same base the tree-sitter tier publishes. Lines and
//! columns are published one-indexed, the base every code-intelligence
//! surface of the product uses.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path};

use protobuf::Message;
use scip::types::{Index, Occurrence as ScipOccurrence, SymbolRole};
use sha2::{Digest, Sha256};

use crate::domain::traits::code_intelligence::{Reference, ReferenceKind};
use crate::domain::value_objects::{Location, SourceRange};

/// Maximum encoded SCIP index size accepted by the parser.
pub const DEFAULT_MAX_INDEX_BYTES: usize = 64 * 1024 * 1024;

/// Maximum number of occurrences accepted in one index.
///
/// The bound keeps the cost of a query proportional to a known ceiling:
/// every lookup walks the occurrences of one symbol, and the number of
/// occurrences is capped when the index is decoded.
pub const DEFAULT_MAX_OCCURRENCES: usize = 4_000_000;

/// Caller-verified identity envelope for an immutable SCIP payload.
///
/// SCIP itself does not encode repository, revision, build configuration or
/// indexing scope. The producer of the index supplies this envelope and
/// the consumer states the envelope it expects; the two must be equal for
/// the index to serve queries.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ScipIndexBinding {
    /// Identifier of the repository the index was produced for.
    pub repository_id: String,
    /// Revision (commit) the index was produced from.
    pub revision: String,
    /// Build configuration the index was produced with.
    pub build_configuration: String,
    /// Name of the indexer that produced the index.
    pub indexer: String,
    /// Version of the indexer that produced the index.
    pub indexer_version: String,
    /// Root the index was produced for, as the producer recorded it
    /// (`file:///abs/path`).
    pub project_root: String,
    /// Files the index covers, relative to `project_root`.
    pub scope: Vec<String>,
    /// SHA-256 of the encoded index payload, lowercase hex.
    pub payload_sha256: String,
}

impl ScipIndexBinding {
    /// Builds a binding envelope.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        repository_id: impl Into<String>,
        revision: impl Into<String>,
        build_configuration: impl Into<String>,
        indexer: impl Into<String>,
        indexer_version: impl Into<String>,
        project_root: impl Into<String>,
        scope: Vec<String>,
        payload_sha256: impl Into<String>,
    ) -> Self {
        Self {
            repository_id: repository_id.into(),
            revision: revision.into(),
            build_configuration: build_configuration.into(),
            indexer: indexer.into(),
            indexer_version: indexer_version.into(),
            project_root: project_root.into(),
            scope,
            payload_sha256: payload_sha256.into(),
        }
    }

    /// True when every field of the envelope carries a value.
    ///
    /// An envelope with a hole in it cannot be compared against anything, so
    /// it is refused rather than accepted as "partially bound".
    pub fn is_complete(&self) -> bool {
        !self.repository_id.is_empty()
            && !self.revision.is_empty()
            && !self.build_configuration.is_empty()
            && !self.indexer.is_empty()
            && !self.indexer_version.is_empty()
            && !self.project_root.is_empty()
            && !self.scope.is_empty()
            && !self.payload_sha256.is_empty()
    }
}

/// A parsed index with validated relative paths and range encodings.
#[derive(Debug)]
pub struct ScipIndex {
    project_root: String,
    tool_name: String,
    tool_version: String,
    binding: Option<ScipIndexBinding>,
    documents: BTreeMap<String, ScipDocument>,
    /// Occurrences of every symbol, in document order.
    occurrences_by_symbol: BTreeMap<String, Vec<SymbolOccurrence>>,
    /// The kind every symbol was declared with, when the producer declared it.
    symbol_kinds: BTreeMap<String, i32>,
    /// The symbols each symbol implements, when the producer declared the
    /// relationship.
    implementations: BTreeMap<String, Vec<String>>,
}

#[derive(Debug)]
struct ScipDocument {
    occurrences: Vec<ScipOccurrence>,
}

/// Where an occurrence of a symbol lives.
#[derive(Debug, Clone)]
struct SymbolOccurrence {
    path: String,
    index: usize,
}

/// A SCIP occurrence mapped into the product's published, one-based locations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexedOccurrence {
    /// The SCIP symbol the occurrence belongs to.
    pub symbol: String,
    /// The occurrence span, in published coordinates.
    pub range: SourceRange,
    /// True when the occurrence declares the definition of the symbol.
    pub is_definition: bool,
}

/// Why a supplied SCIP payload cannot be consumed.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ScipIndexError {
    #[error("SCIP index exceeds the configured byte limit")]
    TooLarge,
    #[error("SCIP index exceeds the configured occurrence limit")]
    TooManyOccurrences,
    #[error("SCIP index is not a valid protobuf payload")]
    InvalidProtobuf,
    #[error("SCIP index is missing required metadata")]
    MissingMetadata,
    #[error("SCIP index metadata is incomplete or unsupported")]
    InvalidMetadata,
    #[error("SCIP index contains an unsafe or duplicate document path")]
    InvalidDocumentPath,
    #[error("SCIP document does not declare UTF-8 byte positions")]
    UnsupportedPositionEncoding,
    #[error("SCIP occurrence has an invalid range")]
    InvalidRange,
    #[error("SCIP identity binding does not match the active workspace or payload")]
    IdentityMismatch,
}

impl ScipIndex {
    /// Parses an existing SCIP protobuf payload without running external
    /// tools.
    ///
    /// The result is unbound: it may be inspected, but it does not serve
    /// queries, because nothing vouches for the context it was produced
    /// for. Consumers that want to serve queries use [`Self::parse_bound`].
    pub fn parse(bytes: &[u8], max_bytes: usize) -> Result<Self, ScipIndexError> {
        Self::parse_with_limits(bytes, max_bytes, DEFAULT_MAX_OCCURRENCES)
    }

    /// Parses an existing SCIP protobuf payload, bounding the number of
    /// occurrences it may carry.
    pub fn parse_with_limits(
        bytes: &[u8],
        max_bytes: usize,
        max_occurrences: usize,
    ) -> Result<Self, ScipIndexError> {
        if bytes.len() > max_bytes {
            return Err(ScipIndexError::TooLarge);
        }
        let index = Index::parse_from_bytes(bytes).map_err(|_| ScipIndexError::InvalidProtobuf)?;
        let metadata = index
            .metadata
            .as_ref()
            .ok_or(ScipIndexError::MissingMetadata)?;
        let tool = metadata
            .tool_info
            .as_ref()
            .ok_or(ScipIndexError::MissingMetadata)?;
        if metadata.project_root.is_empty()
            || tool.name.is_empty()
            || tool.version.is_empty()
            || metadata.text_document_encoding.value() != scip::types::TextEncoding::UTF8 as i32
        {
            return Err(ScipIndexError::InvalidMetadata);
        }

        let mut documents = BTreeMap::new();
        let mut occurrences_by_symbol: BTreeMap<String, Vec<SymbolOccurrence>> = BTreeMap::new();
        let mut symbol_kinds: BTreeMap<String, i32> = BTreeMap::new();
        let mut implementations: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut total_occurrences = 0usize;

        for document in index.documents {
            let relative_path = validate_relative_path(&document.relative_path)?;
            if document.position_encoding.value()
                != scip::types::PositionEncoding::UTF8CodeUnitOffsetFromLineStart as i32
            {
                return Err(ScipIndexError::UnsupportedPositionEncoding);
            }
            total_occurrences += document.occurrences.len();
            if total_occurrences > max_occurrences {
                return Err(ScipIndexError::TooManyOccurrences);
            }
            for occurrence in &document.occurrences {
                decode_range(&relative_path, &occurrence.range)?;
            }
            for symbol in &document.symbols {
                if symbol.symbol.is_empty() {
                    continue;
                }
                symbol_kinds
                    .entry(symbol.symbol.clone())
                    .or_insert_with(|| symbol.kind.value());
                for relationship in &symbol.relationships {
                    if relationship.is_implementation && !relationship.symbol.is_empty() {
                        implementations
                            .entry(symbol.symbol.clone())
                            .or_default()
                            .push(relationship.symbol.clone());
                    }
                }
            }
            for (index, occurrence) in document.occurrences.iter().enumerate() {
                if occurrence.symbol.is_empty() {
                    continue;
                }
                occurrences_by_symbol
                    .entry(occurrence.symbol.clone())
                    .or_default()
                    .push(SymbolOccurrence {
                        path: relative_path.clone(),
                        index,
                    });
            }
            if documents
                .insert(
                    relative_path,
                    ScipDocument {
                        occurrences: document.occurrences,
                    },
                )
                .is_some()
            {
                return Err(ScipIndexError::InvalidDocumentPath);
            }
        }

        Ok(Self {
            project_root: metadata.project_root.clone(),
            tool_name: tool.name.clone(),
            tool_version: tool.version.clone(),
            binding: None,
            documents,
            occurrences_by_symbol,
            symbol_kinds,
            implementations,
        })
    }

    /// Parses an existing index only when its external identity envelope
    /// matches the caller's current workspace context and its payload digest.
    ///
    /// A refused index is not a degraded index: it serves nothing, so the
    /// pipeline answers from a lower tier instead of answering from an index
    /// that belongs to another context.
    pub fn parse_bound(
        bytes: &[u8],
        max_bytes: usize,
        binding: &ScipIndexBinding,
        expected: &ScipIndexBinding,
    ) -> Result<Self, ScipIndexError> {
        if binding != expected
            || !binding.is_complete()
            || binding.payload_sha256 != hex::encode(Sha256::digest(bytes))
        {
            return Err(ScipIndexError::IdentityMismatch);
        }
        let mut index = Self::parse(bytes, max_bytes)?;
        if index.project_root != binding.project_root
            || index.tool_name != binding.indexer
            || index.tool_version != binding.indexer_version
        {
            return Err(ScipIndexError::IdentityMismatch);
        }
        let bound_scope = binding
            .scope
            .iter()
            .map(|path| validate_relative_path(path))
            .collect::<Result<BTreeSet<_>, _>>()
            .map_err(|_| ScipIndexError::IdentityMismatch)?;
        let indexed_scope = index.documents.keys().cloned().collect::<BTreeSet<_>>();
        if bound_scope.len() != binding.scope.len() || bound_scope != indexed_scope {
            return Err(ScipIndexError::IdentityMismatch);
        }
        index.binding = Some(binding.clone());
        Ok(index)
    }

    /// The root the index was produced for, as the producer recorded it.
    pub fn project_root(&self) -> &str {
        &self.project_root
    }

    /// The name of the indexer that produced the index.
    pub fn tool_name(&self) -> &str {
        &self.tool_name
    }

    /// The version of the indexer that produced the index.
    pub fn tool_version(&self) -> &str {
        &self.tool_version
    }

    /// The identity envelope this index was bound to, when it was bound.
    pub fn binding(&self) -> Option<&ScipIndexBinding> {
        self.binding.as_ref()
    }

    /// True when the index may serve queries.
    ///
    /// An unbound index is inspectable but not servable: nothing vouches for
    /// the context it was produced for.
    pub fn is_bound(&self) -> bool {
        self.binding.is_some()
    }

    /// The files the index covers, relative to its project root.
    pub fn document_paths(&self) -> impl Iterator<Item = &str> {
        self.documents.keys().map(String::as_str)
    }

    /// The symbols `symbol` implements, when the producer declared the
    /// relationship.
    ///
    /// Not every indexer emits implementation relationships: an index that
    /// declares none answers with an empty list, and the caller treats the
    /// absence as "not declared", not as "implements nothing".
    pub fn implementations_of(&self, symbol: &str) -> Vec<&str> {
        self.implementations
            .get(symbol)
            .map(|symbols| symbols.iter().map(String::as_str).collect())
            .unwrap_or_default()
    }

    /// Every occurrence of `symbol`, in document order.
    ///
    /// `None` when the index carries no occurrence of the symbol: an absent
    /// occurrence is not evidence that the relation does not exist.
    pub fn occurrences(&self, symbol: &str) -> Option<Vec<IndexedOccurrence>> {
        let mut found = Vec::new();
        for occurrence in self.occurrences_by_symbol.get(symbol)? {
            let document = self.documents.get(&occurrence.path)?;
            let scip_occurrence = document.occurrences.get(occurrence.index)?;
            found.push(IndexedOccurrence {
                symbol: scip_occurrence.symbol.clone(),
                range: decode_range(&occurrence.path, &scip_occurrence.range).ok()?,
                is_definition: is_definition(scip_occurrence),
            });
        }
        Some(found)
    }

    /// Resolves the references to the symbol at `location`.
    ///
    /// The answer requires a bound index and an unambiguous symbol at the
    /// queried position. A miss is `None`, so a lower tier can answer
    /// instead of the absence of SCIP evidence turning into a negative
    /// result.
    pub fn find_references_at(
        &self,
        location: &Location,
        include_declaration: bool,
    ) -> Option<Vec<Reference>> {
        self.binding.as_ref()?;
        let symbol = self.symbol_at(location)?;
        let references = self
            .occurrences_by_symbol
            .get(&symbol)?
            .iter()
            .filter_map(|occurrence| {
                let document = self.documents.get(&occurrence.path)?;
                let scip_occurrence = document.occurrences.get(occurrence.index)?;
                if !include_declaration && is_definition(scip_occurrence) {
                    return None;
                }
                let range = decode_range(&occurrence.path, &scip_occurrence.range).ok()?;
                Some(Reference {
                    location: self.absolute_location(range.start()),
                    reference_kind: self.reference_kind_for(scip_occurrence),
                    container: None,
                })
            })
            .collect::<Vec<_>>();
        (!references.is_empty()).then_some(references)
    }

    /// Resolves the definition of the symbol at `location`.
    ///
    /// `None` when the index does not cover the position, when the position
    /// is ambiguous, or when the index carries no definition for the symbol.
    pub fn find_definition_at(&self, location: &Location) -> Option<Location> {
        self.binding.as_ref()?;
        let symbol = self.symbol_at(location)?;
        self.occurrences_by_symbol
            .get(&symbol)?
            .iter()
            .find_map(|occurrence| {
                let document = self.documents.get(&occurrence.path)?;
                let scip_occurrence = document.occurrences.get(occurrence.index)?;
                if !is_definition(scip_occurrence) {
                    return None;
                }
                let range = decode_range(&occurrence.path, &scip_occurrence.range).ok()?;
                Some(self.absolute_location(range.start()))
            })
    }

    /// The symbol at `location`, when one symbol is the innermost one there.
    ///
    /// SCIP nests occurrences: the crate occurrence covers a whole file and
    /// the declaration of a symbol covers a few characters inside it. The
    /// innermost occurrence is the one the position names, so the smallest
    /// span covering the position wins. Two symbols sharing that smallest
    /// span is an ambiguity the index does not resolve, and the position
    /// answers nothing.
    fn symbol_at(&self, location: &Location) -> Option<String> {
        let relative_path = self.relative_path_for(location.file())?;
        let document = self.documents.get(&relative_path)?;
        let target = (location.line(), location.column());
        let candidates = document
            .occurrences
            .iter()
            .filter_map(|occurrence| {
                let range = decode_range(&relative_path, &occurrence.range).ok()?;
                if !range_contains(&range, target) || occurrence.symbol.is_empty() {
                    return None;
                }
                Some((span_size(&range), occurrence.symbol.as_str()))
            })
            .collect::<Vec<_>>();
        let smallest = candidates.iter().map(|(span, _)| *span).min()?;
        let mut symbols = candidates
            .iter()
            .filter(|(span, _)| *span == smallest)
            .map(|(_, symbol)| *symbol)
            .collect::<BTreeSet<_>>();
        let symbol = symbols.pop_first()?;
        symbols.is_empty().then(|| symbol.to_string())
    }

    /// The kind of reference an occurrence declares.
    ///
    /// The role bits come first, because they are what the producer
    /// declared about the occurrence itself; the symbol kind only decides
    /// the kind of a plain reference.
    fn reference_kind_for(&self, occurrence: &ScipOccurrence) -> ReferenceKind {
        let roles = occurrence.symbol_roles;
        if roles & (SymbolRole::Import as i32) != 0 {
            return ReferenceKind::Import;
        }
        if roles & (SymbolRole::WriteAccess as i32) != 0 {
            return ReferenceKind::Write;
        }
        if roles & (SymbolRole::ReadAccess as i32) != 0 {
            return ReferenceKind::Read;
        }
        if is_definition(occurrence) {
            return ReferenceKind::Write;
        }
        match self.symbol_kinds.get(&occurrence.symbol).copied() {
            Some(kind) if is_callable_kind(kind) => ReferenceKind::Call,
            Some(kind) if is_type_kind(kind) => ReferenceKind::Type,
            _ => ReferenceKind::Read,
        }
    }

    /// The path of a document inside the index, relative to its root.
    fn relative_path_for(&self, absolute_path: &str) -> Option<String> {
        let root = self.absolute_root();
        let relative = Path::new(absolute_path)
            .strip_prefix(Path::new(&root))
            .ok()?;
        validate_relative_path(&relative.to_string_lossy()).ok()
    }

    /// The absolute root of the index, without the `file://` scheme.
    fn absolute_root(&self) -> String {
        self.project_root
            .strip_prefix("file://")
            .unwrap_or(&self.project_root)
            .trim_end_matches('/')
            .to_string()
    }

    /// The published location of a position inside an indexed document.
    ///
    /// The index records document paths relative to its root; the product
    /// publishes absolute paths, the base every other code-intelligence
    /// surface uses.
    fn absolute_location(&self, position: &Location) -> Location {
        let relative_path = position.file();
        let relative_path = relative_path
            .strip_prefix(&format!("{}/", self.absolute_root()))
            .unwrap_or(relative_path);
        Location::new(
            format!("{}/{}", self.absolute_root(), relative_path),
            position.line(),
            position.column(),
        )
    }
}

fn is_definition(occurrence: &ScipOccurrence) -> bool {
    occurrence.symbol_roles & (SymbolRole::Definition as i32) != 0
}

/// True for the symbol kinds that denote something callable.
///
/// The producer declares the kind of the symbol, not the kind of the
/// occurrence; a reference to a function is a call site, and a reference to
/// a type is a type reference.
fn is_callable_kind(kind: i32) -> bool {
    use scip::types::symbol_information::Kind as K;
    const ABSTRACT_METHOD: i32 = K::AbstractMethod as i32;
    const ACCESSOR: i32 = K::Accessor as i32;
    const CONSTRUCTOR: i32 = K::Constructor as i32;
    const DELEGATE: i32 = K::Delegate as i32;
    const FUNCTION: i32 = K::Function as i32;
    const GETTER: i32 = K::Getter as i32;
    const MACRO: i32 = K::Macro as i32;
    const METHOD: i32 = K::Method as i32;
    const METHOD_ALIAS: i32 = K::MethodAlias as i32;
    const METHOD_SPECIFICATION: i32 = K::MethodSpecification as i32;
    const PURE_VIRTUAL_METHOD: i32 = K::PureVirtualMethod as i32;
    const SETTER: i32 = K::Setter as i32;
    const SINGLETON_METHOD: i32 = K::SingletonMethod as i32;
    const STATIC_METHOD: i32 = K::StaticMethod as i32;
    const SUBSCRIPT: i32 = K::Subscript as i32;
    const TRAIT_METHOD: i32 = K::TraitMethod as i32;
    matches!(
        kind,
        ABSTRACT_METHOD
            | ACCESSOR
            | CONSTRUCTOR
            | DELEGATE
            | FUNCTION
            | GETTER
            | MACRO
            | METHOD
            | METHOD_ALIAS
            | METHOD_SPECIFICATION
            | PURE_VIRTUAL_METHOD
            | SETTER
            | SINGLETON_METHOD
            | STATIC_METHOD
            | SUBSCRIPT
            | TRAIT_METHOD
    )
}

/// True for the symbol kinds that denote a type.
fn is_type_kind(kind: i32) -> bool {
    use scip::types::symbol_information::Kind as K;
    const CLASS: i32 = K::Class as i32;
    const ENUM: i32 = K::Enum as i32;
    const INTERFACE: i32 = K::Interface as i32;
    const MODULE: i32 = K::Module as i32;
    const NAMESPACE: i32 = K::Namespace as i32;
    const OBJECT: i32 = K::Object as i32;
    const PACKAGE: i32 = K::Package as i32;
    const PROTOCOL: i32 = K::Protocol as i32;
    const SINGLETON_CLASS: i32 = K::SingletonClass as i32;
    const STRUCT: i32 = K::Struct as i32;
    const TRAIT: i32 = K::Trait as i32;
    const TYPE: i32 = K::Type as i32;
    const TYPE_ALIAS: i32 = K::TypeAlias as i32;
    const UNION: i32 = K::Union as i32;
    matches!(
        kind,
        CLASS
            | ENUM
            | INTERFACE
            | MODULE
            | NAMESPACE
            | OBJECT
            | PACKAGE
            | PROTOCOL
            | SINGLETON_CLASS
            | STRUCT
            | TRAIT
            | TYPE
            | TYPE_ALIAS
            | UNION
    )
}

fn range_contains(range: &SourceRange, (line, column): (u32, u32)) -> bool {
    let start = (range.start().line(), range.start().column());
    let end = (range.end().line(), range.end().column());
    (line, column) >= start && (line, column) < end
}

/// How wide a range is, as (line span, column span).
///
/// The line span comes first because it dominates: a range inside one line
/// is narrower than a range that crosses lines, whatever its columns.
fn span_size(range: &SourceRange) -> (u32, u32) {
    let start = (range.start().line(), range.start().column());
    let end = (range.end().line(), range.end().column());
    if start.0 == end.0 {
        (0, end.1.saturating_sub(start.1))
    } else {
        (end.0.saturating_sub(start.0), end.1)
    }
}

fn validate_relative_path(path: &str) -> Result<String, ScipIndexError> {
    let candidate = Path::new(path);
    if path.is_empty()
        || path.contains('\\')
        || path.contains('\0')
        || candidate.is_absolute()
        || candidate
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(ScipIndexError::InvalidDocumentPath);
    }
    Ok(candidate
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/"))
}

/// Decodes a SCIP range into published coordinates.
///
/// A three-element range is single-line (`[line, start, end]`); a
/// four-element range is multi-line (`[start_line, start_column, end_line,
/// end_column]`). Columns are UTF-8 code units from the start of the line,
/// and both bases are published one-indexed.
fn decode_range(path: &str, encoded: &[i32]) -> Result<SourceRange, ScipIndexError> {
    let (start_line, start_column, end_line, end_column) = match encoded {
        [line, start, end] => (*line, *start, *line, *end),
        [start_line, start_column, end_line, end_column] => {
            (*start_line, *start_column, *end_line, *end_column)
        }
        _ => return Err(ScipIndexError::InvalidRange),
    };
    if [start_line, start_column, end_line, end_column]
        .iter()
        .any(|value| *value < 0)
        || (end_line, end_column) < (start_line, start_column)
    {
        return Err(ScipIndexError::InvalidRange);
    }
    let start = Location::new(
        path,
        u32::try_from(start_line)
            .map_err(|_| ScipIndexError::InvalidRange)?
            .saturating_add(1),
        u32::try_from(start_column)
            .map_err(|_| ScipIndexError::InvalidRange)?
            .saturating_add(1),
    );
    let end = Location::new(
        path,
        u32::try_from(end_line)
            .map_err(|_| ScipIndexError::InvalidRange)?
            .saturating_add(1),
        u32::try_from(end_column)
            .map_err(|_| ScipIndexError::InvalidRange)?
            .saturating_add(1),
    );
    Ok(SourceRange::new(start, end))
}

#[cfg(test)]
mod tests {
    use super::*;

    const REAL_INDEX: &[u8] = include_bytes!("../../tests/data/scip/rust-analyzer-minimal.scip");

    fn real_index_binding() -> ScipIndexBinding {
        ScipIndexBinding {
            repository_id: "fixture-repo".to_string(),
            revision: "0123456789abcdef".to_string(),
            build_configuration: "cargo-default".to_string(),
            indexer: "rust-analyzer".to_string(),
            indexer_version: "1.96.0 (ac68faa 2026-05-25)".to_string(),
            project_root: "file:///tmp/cgh096-scip-fixture".to_string(),
            scope: vec!["src/lib.rs".to_string()],
            payload_sha256: hex::encode(Sha256::digest(REAL_INDEX)),
        }
    }

    fn bound_real_index() -> ScipIndex {
        let binding = real_index_binding();
        ScipIndex::parse_bound(REAL_INDEX, DEFAULT_MAX_INDEX_BYTES, &binding, &binding).unwrap()
    }

    #[test]
    fn bound_index_requires_matching_workspace_identity_and_digest() {
        let binding = real_index_binding();
        let parsed =
            ScipIndex::parse_bound(REAL_INDEX, DEFAULT_MAX_INDEX_BYTES, &binding, &binding)
                .unwrap();
        assert_eq!(parsed.binding(), Some(&binding));

        let mut stale = binding.clone();
        stale.revision = "fedcba9876543210".to_string();
        assert_eq!(
            ScipIndex::parse_bound(REAL_INDEX, DEFAULT_MAX_INDEX_BYTES, &binding, &stale)
                .unwrap_err(),
            ScipIndexError::IdentityMismatch
        );

        let mut incomplete_scope = binding.clone();
        incomplete_scope.scope.push("src/other.rs".to_string());
        assert_eq!(
            ScipIndex::parse_bound(
                REAL_INDEX,
                DEFAULT_MAX_INDEX_BYTES,
                &incomplete_scope,
                &incomplete_scope,
            )
            .unwrap_err(),
            ScipIndexError::IdentityMismatch
        );

        let mut other_digest = binding.clone();
        other_digest.payload_sha256 = "0".repeat(64);
        assert_eq!(
            ScipIndex::parse_bound(
                REAL_INDEX,
                DEFAULT_MAX_INDEX_BYTES,
                &other_digest,
                &other_digest,
            )
            .unwrap_err(),
            ScipIndexError::IdentityMismatch
        );

        let mut incomplete = binding.clone();
        incomplete.build_configuration = String::new();
        assert_eq!(
            ScipIndex::parse_bound(
                REAL_INDEX,
                DEFAULT_MAX_INDEX_BYTES,
                &incomplete,
                &incomplete,
            )
            .unwrap_err(),
            ScipIndexError::IdentityMismatch
        );
    }

    #[test]
    fn consumes_real_rust_analyzer_index_and_finds_definition_and_reference() {
        let index = ScipIndex::parse(REAL_INDEX, DEFAULT_MAX_INDEX_BYTES).unwrap();
        assert_eq!(index.tool_name(), "rust-analyzer");
        assert!(index.tool_version().starts_with("1.96.0"));
        assert_eq!(index.project_root(), "file:///tmp/cgh096-scip-fixture");
        assert_eq!(index.document_paths().collect::<Vec<_>>(), ["src/lib.rs"]);

        let occurrences = index
            .occurrences("rust-analyzer cargo scip_fixture 0.1.0 greet().")
            .unwrap();
        assert_eq!(occurrences.len(), 2);
        assert!(
            occurrences
                .iter()
                .any(|occurrence| occurrence.is_definition)
        );
        assert!(
            occurrences
                .iter()
                .any(|occurrence| !occurrence.is_definition)
        );
        assert!(occurrences.iter().all(|occurrence| {
            occurrence.range.start().file() == "src/lib.rs"
                && occurrence.range.start().line() > 0
                && occurrence.range.start().column() > 0
        }));
    }

    #[test]
    fn an_unbound_index_serves_nothing() {
        let index = ScipIndex::parse(REAL_INDEX, DEFAULT_MAX_INDEX_BYTES).unwrap();
        assert!(!index.is_bound());
        let location = Location::new("/tmp/cgh096-scip-fixture/src/lib.rs", 1, 8);
        assert!(index.find_references_at(&location, true).is_none());
        assert!(index.find_definition_at(&location).is_none());
    }

    #[test]
    fn resolves_references_and_definition_of_a_bound_index() {
        let index = bound_real_index();
        let definition = Location::new("/tmp/cgh096-scip-fixture/src/lib.rs", 1, 8);

        let references = index.find_references_at(&definition, true).unwrap();
        assert_eq!(references.len(), 2);
        assert!(
            references
                .iter()
                .all(|reference| reference.location.file().ends_with("/src/lib.rs"))
        );

        let without_declaration = index.find_references_at(&definition, false).unwrap();
        assert_eq!(without_declaration.len(), 1);
        assert_eq!(without_declaration[0].location.line(), 6);
        assert_eq!(without_declaration[0].reference_kind, ReferenceKind::Call);

        let resolved = index.find_definition_at(&definition).unwrap();
        assert_eq!(resolved.line(), 1);
        assert_eq!(resolved.column(), 8);
    }

    #[test]
    fn a_position_outside_the_index_answers_nothing() {
        let index = bound_real_index();
        let outside = Location::new("/tmp/other-project/src/lib.rs", 1, 8);
        assert!(index.find_references_at(&outside, true).is_none());
        assert!(index.find_definition_at(&outside).is_none());
    }

    #[test]
    fn rejects_invalid_and_oversized_payloads() {
        assert_eq!(
            ScipIndex::parse(b"not protobuf", DEFAULT_MAX_INDEX_BYTES).unwrap_err(),
            ScipIndexError::InvalidProtobuf
        );
        assert_eq!(
            ScipIndex::parse(REAL_INDEX, REAL_INDEX.len() - 1).unwrap_err(),
            ScipIndexError::TooLarge
        );
    }

    #[test]
    fn rejects_an_index_with_more_occurrences_than_the_limit() {
        assert_eq!(
            ScipIndex::parse_with_limits(REAL_INDEX, DEFAULT_MAX_INDEX_BYTES, 1).unwrap_err(),
            ScipIndexError::TooManyOccurrences
        );
    }

    #[test]
    fn rejects_traversal_paths() {
        assert_eq!(
            validate_relative_path("../outside.rs").unwrap_err(),
            ScipIndexError::InvalidDocumentPath
        );
        assert_eq!(
            validate_relative_path("/absolute.rs").unwrap_err(),
            ScipIndexError::InvalidDocumentPath
        );
    }
}
