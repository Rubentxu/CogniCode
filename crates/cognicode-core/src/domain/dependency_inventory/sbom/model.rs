//! What a CycloneDX document claims, as plain values.
//!
//! Everything here is a claim by the producer of the document. Nothing in it
//! has been checked against the artifact it describes.

use serde::{Deserialize, Serialize};

/// Everything the reader bounds. The document is untrusted data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SbomLimits {
    /// The most bytes a document may have.
    pub max_bytes: usize,
    /// The most components a document may list, nested ones included.
    pub max_components: usize,
    /// The deepest `components` nesting read.
    pub max_depth: usize,
    /// The most dependency edges (`dependsOn` entries) read.
    pub max_edges: usize,
    /// The longest identifier, name, version or PURL, in bytes.
    pub max_text: usize,
    /// The most diagnostics listed individually; the total is always given.
    pub max_diagnostics: usize,
}

impl Default for SbomLimits {
    fn default() -> Self {
        Self {
            max_bytes: 16 * 1024 * 1024,
            max_components: 100_000,
            max_depth: 16,
            max_edges: 1_000_000,
            max_text: 4_096,
            max_diagnostics: 100,
        }
    }
}

/// Why a document was not read at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SbomError {
    /// The document is above the size limit.
    TooLarge {
        /// The size of the document.
        bytes: usize,
        /// The limit.
        limit: usize,
    },
    /// The document is not valid JSON, or nests deeper than the JSON reader
    /// allows.
    Malformed {
        /// The line of the first problem.
        line: usize,
        /// The column of the first problem.
        column: usize,
    },
    /// The JSON is not an object with a `bomFormat`.
    NotAnSbom,
    /// A well-known SBOM format this reader does not read (SPDX).
    UnsupportedFormat {
        /// The format, as far as it could be recognised.
        found: String,
    },
    /// A CycloneDX `specVersion` outside the versions this reader covers.
    UnsupportedSpecVersion {
        /// The `specVersion` the document states.
        found: String,
    },
    /// More components than the limit.
    TooManyComponents {
        /// The limit.
        limit: usize,
    },
    /// More dependency edges than the limit.
    TooManyEdges {
        /// The limit.
        limit: usize,
    },
    /// Components nested deeper than the limit.
    TooDeep {
        /// The limit.
        limit: usize,
    },
}

impl std::fmt::Display for SbomError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooLarge { bytes, limit } => {
                write!(f, "{bytes} bytes exceed the {limit} byte limit")
            }
            Self::Malformed { line, column } => {
                write!(f, "not valid JSON at line {line}, column {column}")
            }
            Self::NotAnSbom => write!(f, "not a CycloneDX document: no `bomFormat`"),
            Self::UnsupportedFormat { found } => write!(
                f,
                "the format `{found}` is not read yet; only CycloneDX JSON is"
            ),
            Self::UnsupportedSpecVersion { found } => write!(
                f,
                "CycloneDX specVersion `{found}` is not read; 1.2 to 1.7 are"
            ),
            Self::TooManyComponents { limit } => {
                write!(f, "more than {limit} components")
            }
            Self::TooManyEdges { limit } => {
                write!(f, "more than {limit} dependency edges")
            }
            Self::TooDeep { limit } => {
                write!(f, "components nested deeper than {limit} levels")
            }
        }
    }
}

/// The lifecycle stage a document says it describes.
///
/// A source-manifest SBOM and an artifact SBOM are different claims: the
/// first lists what sources ask for, the second what was built. They are
/// never mixed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SbomStage {
    /// `design`, `pre-build` or `discovery`: sources and manifests.
    Source,
    /// `build`, `post-build` or `operations`: a built artifact.
    Artifact,
    /// The document states no lifecycle: the producer did not say.
    Unstated,
    /// The document states both kinds of stage.
    Ambiguous,
}

/// The `scope` a component states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComponentScope {
    /// The component is needed by the subject.
    Required,
    /// The component is optional.
    Optional,
    /// The component is not part of what the subject ships.
    Excluded,
    /// No scope, or one outside the specification.
    Unstated,
}

/// One component of the document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SbomComponent {
    /// `bom-ref`: the document-local identity other entries point to.
    pub bom_ref: Option<String>,
    /// The CycloneDX component type (`library`, `application`, ...).
    pub component_type: String,
    /// The `group`: a namespace as the producer writes it.
    pub group: Option<String>,
    /// The name.
    pub name: String,
    /// The version.
    pub version: Option<String>,
    /// The Package URL, verbatim.
    pub purl: Option<String>,
    /// The stated scope.
    pub scope: ComponentScope,
    /// The SHA-256 hashes, as lowercase hex.
    pub sha256: Vec<String>,
    /// The other hash algorithms the component carries (not comparable with
    /// a lockfile checksum).
    pub other_hash_algorithms: Vec<String>,
    /// The licenses: SPDX ids, names or expressions, as written.
    pub licenses: Vec<String>,
    /// The nesting level: 0 for a top-level component.
    pub depth: usize,
}

/// What the document says it describes: `metadata.component`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SbomSubject {
    /// The component type.
    pub component_type: String,
    /// The name.
    pub name: String,
    /// The version.
    pub version: Option<String>,
    /// The Package URL, verbatim.
    pub purl: Option<String>,
    /// The `bom-ref`.
    pub bom_ref: Option<String>,
    /// The SHA-256 hashes the document states for the subject itself: the
    /// only thing in a document that can bind it to an artifact digest.
    pub sha256: Vec<String>,
}

/// The kinds of structural problem a document can have.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SbomDiagnosticKind {
    /// Two entries share a `bom-ref`.
    DuplicateBomRef,
    /// A dependency entry names a `bom-ref` no component or subject has.
    DanglingDependencyRef,
    /// A component could not be read and was left out.
    RejectedComponent,
    /// A PURL that is not a valid Package URL.
    InvalidPurl,
    /// A hash that is not valid for its algorithm.
    InvalidHash,
    /// A component with no version: it cannot be matched to a resolved
    /// package.
    MissingVersion,
    /// A `scope` outside `required`, `optional` and `excluded`.
    UnknownScope,
    /// The document has no `metadata.component`: it does not say what it
    /// describes.
    MissingSubject,
}

/// One structural problem, with the entry it is about.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SbomDiagnostic {
    /// What kind of problem.
    pub kind: SbomDiagnosticKind,
    /// The `bom-ref`, name or position of the entry.
    pub subject: String,
    /// What exactly is wrong.
    pub detail: String,
}

/// A CycloneDX document, read defensively.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedSbom {
    /// The `specVersion`.
    pub spec_version: String,
    /// The `serialNumber`.
    pub serial_number: Option<String>,
    /// The document `version` (a revision counter the producer keeps).
    pub version: Option<u64>,
    /// The `metadata.timestamp`, verbatim.
    pub timestamp: Option<String>,
    /// The producing tools, as `vendor name version`.
    pub tools: Vec<String>,
    /// The raw lifecycle phases.
    pub lifecycle_phases: Vec<String>,
    /// The stage those phases imply.
    pub stage: SbomStage,
    /// What the document says it describes.
    pub subject: Option<SbomSubject>,
    /// Every component, nested ones flattened.
    pub components: Vec<SbomComponent>,
    /// How many `dependencies` entries the document has.
    pub dependency_entries: usize,
    /// How many `dependsOn` edges those entries have.
    pub dependency_edges: usize,
    /// Whether the document carries a `signature`. It is never verified.
    pub signature_present: bool,
    /// The top-level sections the reader does not use, by name.
    pub ignored_sections: Vec<String>,
    /// How many components could not be read.
    pub rejected_components: usize,
    /// The first structural problems.
    pub diagnostics: Vec<SbomDiagnostic>,
    /// How many structural problems there are in all.
    pub diagnostics_total: usize,
}

/// How many components carry each fact, for the coverage of a document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ComponentFacts {
    /// Components read.
    pub total: usize,
    /// With a PURL.
    pub with_purl: usize,
    /// With a SHA-256 hash.
    pub with_sha256: usize,
    /// With a stated scope.
    pub with_scope: usize,
    /// With a license.
    pub with_license: usize,
    /// With a `bom-ref`.
    pub with_bom_ref: usize,
}

impl ParsedSbom {
    /// The coverage of the facts a comparison reads.
    pub fn facts(&self) -> ComponentFacts {
        let count =
            |keep: fn(&SbomComponent) -> bool| self.components.iter().filter(|c| keep(c)).count();
        ComponentFacts {
            total: self.components.len(),
            with_purl: count(|c| c.purl.is_some()),
            with_sha256: count(|c| !c.sha256.is_empty()),
            with_scope: count(|c| c.scope != ComponentScope::Unstated),
            with_license: count(|c| !c.licenses.is_empty()),
            with_bom_ref: count(|c| c.bom_ref.is_some()),
        }
    }
}

/// What a document claims about itself and how completely it could be read.
/// The producer's statements, not verified facts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SbomClaim {
    /// The format read.
    pub format: String,
    /// The `specVersion`.
    pub spec_version: String,
    /// The `serialNumber`.
    pub serial_number: Option<String>,
    /// The document `version`.
    pub version: Option<u64>,
    /// The `metadata.timestamp`.
    pub timestamp: Option<String>,
    /// The producing tools.
    pub tools: Vec<String>,
    /// The lifecycle phases the document states.
    pub lifecycle_phases: Vec<String>,
    /// The stage those phases imply.
    pub stage: SbomStage,
    /// What the document says it describes.
    pub subject: Option<SbomSubject>,
    /// How many components carry each fact.
    pub components: ComponentFacts,
    /// How many dependency entries and edges the document has.
    pub dependency_entries: usize,
    /// The edges of those entries.
    pub dependency_edges: usize,
    /// Whether a `signature` is present. It is never verified here.
    pub signature_present: bool,
    /// The top-level sections the reader does not use.
    pub ignored_sections: Vec<String>,
    /// How many components could not be read.
    pub rejected_components: usize,
    /// The first structural problems.
    pub diagnostics: Vec<SbomDiagnostic>,
    /// How many structural problems there are in all.
    pub diagnostics_total: usize,
}

impl ParsedSbom {
    /// The claim part of the document, for a report.
    pub fn claim(&self) -> SbomClaim {
        SbomClaim {
            format: "cyclonedx".to_string(),
            spec_version: self.spec_version.clone(),
            serial_number: self.serial_number.clone(),
            version: self.version,
            timestamp: self.timestamp.clone(),
            tools: self.tools.clone(),
            lifecycle_phases: self.lifecycle_phases.clone(),
            stage: self.stage,
            subject: self.subject.clone(),
            components: self.facts(),
            dependency_entries: self.dependency_entries,
            dependency_edges: self.dependency_edges,
            signature_present: self.signature_present,
            ignored_sections: self.ignored_sections.clone(),
            rejected_components: self.rejected_components,
            diagnostics: self.diagnostics.clone(),
            diagnostics_total: self.diagnostics_total,
        }
    }
}
