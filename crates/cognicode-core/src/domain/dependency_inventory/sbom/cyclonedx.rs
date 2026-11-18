//! Defensive reading of a CycloneDX JSON document.
//!
//! The document is untrusted data. It is read as data: nothing in it is
//! executed, fetched or followed (an `externalReferences` URL is never
//! opened), every dimension an attacker could inflate is bounded, and an
//! entry the reader cannot use is counted and named instead of skipped
//! silently. Components are flattened from a bounded nesting depth.

use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};

use super::model::{
    ComponentScope, ParsedSbom, SbomComponent, SbomDiagnostic, SbomDiagnosticKind, SbomError,
    SbomLimits, SbomStage, SbomSubject,
};
use crate::domain::dependency_inventory::purl::parse_purl;

/// The CycloneDX versions this reader covers, as the document writes them.
const SPEC_VERSIONS: [&str; 6] = ["1.2", "1.3", "1.4", "1.5", "1.6", "1.7"];
/// The top-level keys the reader uses or knowingly passes over.
const KNOWN_SECTIONS: [&str; 9] = [
    "$schema",
    "bomFormat",
    "specVersion",
    "serialNumber",
    "version",
    "metadata",
    "components",
    "dependencies",
    "signature",
];
/// The most licenses read from one component.
const MAX_LICENSES: usize = 32;
/// The most tools read from `metadata.tools`.
const MAX_TOOLS: usize = 32;
/// The most lifecycle phases read.
const MAX_PHASES: usize = 16;
/// The most ignored section names listed.
const MAX_IGNORED: usize = 64;

/// Collects diagnostics up to a limit and counts the rest.
struct Diagnostics {
    list: Vec<SbomDiagnostic>,
    total: usize,
    limit: usize,
}

impl Diagnostics {
    fn push(&mut self, kind: SbomDiagnosticKind, subject: &str, detail: impl Into<String>) {
        self.total += 1;
        if self.list.len() < self.limit {
            self.list.push(SbomDiagnostic {
                kind,
                subject: subject.chars().take(200).collect(),
                detail: detail.into(),
            });
        }
    }
}

struct Reader<'a> {
    limits: &'a SbomLimits,
    diagnostics: Diagnostics,
    components: Vec<SbomComponent>,
    refs: BTreeMap<String, usize>,
    rejected: usize,
}

/// A string no longer than the limit; `Err` carries its length when over.
fn bounded(value: Option<&Value>, max: usize) -> Result<Option<String>, usize> {
    match value.and_then(Value::as_str) {
        Some(text) if text.len() > max => Err(text.len()),
        Some(text) => Ok(Some(text.to_string())),
        None => Ok(None),
    }
}

fn non_empty(text: Option<String>) -> Option<String> {
    text.filter(|text| !text.trim().is_empty())
}

fn scope_of(raw: Option<&str>) -> Option<ComponentScope> {
    match raw {
        None => Some(ComponentScope::Unstated),
        Some("required") => Some(ComponentScope::Required),
        Some("optional") => Some(ComponentScope::Optional),
        Some("excluded") => Some(ComponentScope::Excluded),
        Some(_) => None,
    }
}

fn is_sha256(content: &str) -> bool {
    content.len() == 64 && content.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn tool_label(tool: &Map<String, Value>) -> Option<String> {
    let part = |key: &str| tool.get(key).and_then(Value::as_str);
    let label: Vec<&str> = ["vendor", "group", "name", "version"]
        .into_iter()
        .filter_map(part)
        .collect();
    (!label.is_empty()).then(|| label.join(" "))
}

/// `metadata.tools`: an array of tools (1.2 to 1.4) or an object with
/// `components` and `services` (1.5 and later).
fn tools_of(metadata: &Map<String, Value>) -> Vec<String> {
    let listed: Vec<&Value> = match metadata.get("tools") {
        Some(Value::Array(tools)) => tools.iter().collect(),
        Some(Value::Object(tools)) => ["components", "services"]
            .into_iter()
            .filter_map(|key| tools.get(key).and_then(Value::as_array))
            .flatten()
            .collect(),
        _ => Vec::new(),
    };
    listed
        .into_iter()
        .filter_map(Value::as_object)
        .filter_map(tool_label)
        .take(MAX_TOOLS)
        .collect()
}

fn phases_of(metadata: &Map<String, Value>) -> Vec<String> {
    metadata
        .get("lifecycles")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|lifecycle| lifecycle.get("phase").and_then(Value::as_str))
        .map(|phase| phase.chars().take(64).collect())
        .take(MAX_PHASES)
        .collect()
}

fn stage_of(phases: &[String]) -> SbomStage {
    let source = phases
        .iter()
        .any(|phase| matches!(phase.as_str(), "design" | "pre-build" | "discovery"));
    let artifact = phases
        .iter()
        .any(|phase| matches!(phase.as_str(), "build" | "post-build" | "operations"));
    match (source, artifact) {
        (true, true) => SbomStage::Ambiguous,
        (true, false) => SbomStage::Source,
        (false, true) => SbomStage::Artifact,
        (false, false) => SbomStage::Unstated,
    }
}

impl Reader<'_> {
    fn note_ref(&mut self, bom_ref: &str, place: &str) {
        let count = self.refs.entry(bom_ref.to_string()).or_insert(0);
        *count += 1;
        if *count == 2 {
            self.diagnostics.push(
                SbomDiagnosticKind::DuplicateBomRef,
                bom_ref,
                format!("{place} repeats a bom-ref another entry already uses"),
            );
        }
    }

    /// The `bom-ref`s of components nested under the subject. They are not
    /// part of what was shipped, but dependency entries may point at them.
    fn note_nested_refs(&mut self, value: &Value, depth: usize) {
        if depth > self.limits.max_depth {
            return;
        }
        for child in value
            .get("components")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if let Some(bom_ref) = child.get("bom-ref").and_then(Value::as_str) {
                self.refs.entry(bom_ref.to_string()).or_insert(0);
            }
            self.note_nested_refs(child, depth + 1);
        }
    }

    fn hashes(&mut self, entry: &Map<String, Value>, subject: &str) -> (Vec<String>, Vec<String>) {
        let mut sha256 = BTreeSet::new();
        let mut others = BTreeSet::new();
        for hash in entry
            .get("hashes")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let algorithm = hash.get("alg").and_then(Value::as_str).unwrap_or_default();
            let content = hash
                .get("content")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if algorithm.eq_ignore_ascii_case("SHA-256") {
                if is_sha256(content) {
                    sha256.insert(content.to_ascii_lowercase());
                } else {
                    self.diagnostics.push(
                        SbomDiagnosticKind::InvalidHash,
                        subject,
                        "a SHA-256 hash that is not 64 hex digits",
                    );
                }
            } else if !algorithm.is_empty() {
                others.insert(algorithm.chars().take(32).collect::<String>());
            }
        }
        (sha256.into_iter().collect(), others.into_iter().collect())
    }

    fn licenses(entry: &Map<String, Value>, max_text: usize) -> Vec<String> {
        let mut licenses = BTreeSet::new();
        for item in entry
            .get("licenses")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let named = item.get("license").and_then(|license| {
                license
                    .get("id")
                    .or_else(|| license.get("name"))
                    .and_then(Value::as_str)
            });
            let text = named.or_else(|| item.get("expression").and_then(Value::as_str));
            if let Some(text) = text.filter(|text| !text.is_empty() && text.len() <= max_text) {
                licenses.insert(text.to_string());
            }
            if licenses.len() >= MAX_LICENSES {
                break;
            }
        }
        licenses.into_iter().collect()
    }

    fn reject(&mut self, place: &str, detail: impl Into<String>) {
        self.rejected += 1;
        self.diagnostics
            .push(SbomDiagnosticKind::RejectedComponent, place, detail);
    }

    fn component(&mut self, value: &Value, depth: usize, place: &str) {
        let Some(entry) = value.as_object() else {
            return self.reject(place, "the component is not an object");
        };
        let max = self.limits.max_text;
        let read = |key: &str| bounded(entry.get(key), max);
        let (Ok(name), Ok(version), Ok(purl), Ok(bom_ref), Ok(group), Ok(kind)) = (
            read("name"),
            read("version"),
            read("purl"),
            read("bom-ref"),
            read("group"),
            read("type"),
        ) else {
            return self.reject(place, format!("a field is longer than {max} bytes"));
        };
        let Some(name) = non_empty(name) else {
            return self.reject(place, "the component has no name");
        };
        let version = non_empty(version);
        if version.is_none() {
            self.diagnostics.push(
                SbomDiagnosticKind::MissingVersion,
                &name,
                "no version: the component cannot be matched to a resolved package",
            );
        }
        let purl = non_empty(purl).and_then(|raw| match parse_purl(&raw) {
            Ok(_) => Some(raw),
            Err(error) => {
                self.diagnostics.push(
                    SbomDiagnosticKind::InvalidPurl,
                    &name,
                    format!("`{raw}` is not a valid Package URL ({error:?})"),
                );
                None
            }
        });
        let raw_scope = entry.get("scope").and_then(Value::as_str);
        let scope = scope_of(raw_scope).unwrap_or_else(|| {
            self.diagnostics.push(
                SbomDiagnosticKind::UnknownScope,
                &name,
                format!(
                    "scope `{}` is not a CycloneDX scope",
                    raw_scope.unwrap_or("")
                ),
            );
            ComponentScope::Unstated
        });
        let bom_ref = non_empty(bom_ref);
        if let Some(bom_ref) = &bom_ref {
            self.note_ref(bom_ref, "a component");
        }
        let (sha256, other_hash_algorithms) = self.hashes(entry, &name);
        self.components.push(SbomComponent {
            bom_ref,
            component_type: non_empty(kind).unwrap_or_else(|| "unstated".to_string()),
            group: non_empty(group),
            name,
            version,
            purl,
            scope,
            sha256,
            other_hash_algorithms,
            licenses: Self::licenses(entry, max),
            depth,
        });
    }

    /// Flattens `components`, depth first, in document order.
    fn components_of(&mut self, list: &Value, depth: usize, place: &str) -> Result<(), SbomError> {
        let Some(items) = list.as_array() else {
            return Ok(());
        };
        if depth >= self.limits.max_depth {
            return Err(SbomError::TooDeep {
                limit: self.limits.max_depth,
            });
        }
        for (index, item) in items.iter().enumerate() {
            if self.components.len() + self.rejected >= self.limits.max_components {
                return Err(SbomError::TooManyComponents {
                    limit: self.limits.max_components,
                });
            }
            let here = format!("{place}[{index}]");
            self.component(item, depth, &here);
            if let Some(children) = item.get("components") {
                self.components_of(children, depth + 1, &format!("{here}.components"))?;
            }
        }
        Ok(())
    }

    fn subject(&mut self, metadata: &Map<String, Value>) -> Option<SbomSubject> {
        let Some(value) = metadata.get("component").filter(|value| value.is_object()) else {
            self.diagnostics.push(
                SbomDiagnosticKind::MissingSubject,
                "metadata.component",
                "the document does not say what it describes",
            );
            return None;
        };
        let entry = value.as_object()?;
        let max = self.limits.max_text;
        let read = |key: &str| bounded(entry.get(key), max).ok().flatten();
        let name = non_empty(read("name"))?;
        let bom_ref = non_empty(read("bom-ref"));
        if let Some(bom_ref) = &bom_ref {
            self.note_ref(bom_ref, "the subject");
        }
        self.note_nested_refs(value, 0);
        let (sha256, _) = self.hashes(entry, &name);
        Some(SbomSubject {
            component_type: non_empty(read("type")).unwrap_or_else(|| "unstated".to_string()),
            name,
            version: non_empty(read("version")),
            purl: non_empty(read("purl")),
            bom_ref,
            sha256,
        })
    }

    /// Checks the dependency entries against the known `bom-ref`s and
    /// counts them.
    fn dependencies(&mut self, list: &Value) -> Result<(usize, usize), SbomError> {
        let mut entries = 0;
        let mut edges = 0;
        let known: BTreeSet<String> = self.refs.keys().cloned().collect();
        let mut dangling: BTreeSet<String> = BTreeSet::new();
        for entry in list.as_array().into_iter().flatten() {
            entries += 1;
            let from = entry.get("ref").and_then(Value::as_str).unwrap_or_default();
            if !known.contains(from) {
                dangling.insert(from.to_string());
            }
            for target in entry
                .get("dependsOn")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                edges += 1;
                if edges > self.limits.max_edges {
                    return Err(SbomError::TooManyEdges {
                        limit: self.limits.max_edges,
                    });
                }
                let target = target.as_str().unwrap_or_default();
                if !known.contains(target) {
                    dangling.insert(target.to_string());
                }
            }
        }
        for reference in dangling {
            self.diagnostics.push(
                SbomDiagnosticKind::DanglingDependencyRef,
                &reference,
                "a dependency entry names a bom-ref that no component or subject has",
            );
        }
        Ok((entries, edges))
    }
}

fn recognise(root: &Map<String, Value>) -> Result<String, SbomError> {
    if root.contains_key("spdxVersion") || root.contains_key("SPDXID") {
        return Err(SbomError::UnsupportedFormat {
            found: "spdx".to_string(),
        });
    }
    match root.get("bomFormat").and_then(Value::as_str) {
        Some("CycloneDX") => {}
        Some(other) => {
            return Err(SbomError::UnsupportedFormat {
                found: other.chars().take(64).collect(),
            });
        }
        None => return Err(SbomError::NotAnSbom),
    }
    let version = root
        .get("specVersion")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if SPEC_VERSIONS.contains(&version) {
        Ok(version.to_string())
    } else {
        Err(SbomError::UnsupportedSpecVersion {
            found: version.chars().take(32).collect(),
        })
    }
}

fn ignored_sections(root: &Map<String, Value>) -> Vec<String> {
    let mut ignored: Vec<String> = root
        .keys()
        .filter(|key| !KNOWN_SECTIONS.contains(&key.as_str()))
        .map(|key| key.chars().take(64).collect())
        .collect();
    ignored.sort();
    ignored.truncate(MAX_IGNORED);
    ignored
}

/// Reads a CycloneDX JSON document under `limits`.
pub fn parse_cyclonedx(bytes: &[u8], limits: &SbomLimits) -> Result<ParsedSbom, SbomError> {
    if bytes.len() > limits.max_bytes {
        return Err(SbomError::TooLarge {
            bytes: bytes.len(),
            limit: limits.max_bytes,
        });
    }
    let document: Value = serde_json::from_slice(bytes).map_err(|error| SbomError::Malformed {
        line: error.line(),
        column: error.column(),
    })?;
    let root = document.as_object().ok_or(SbomError::NotAnSbom)?;
    let spec_version = recognise(root)?;
    let mut reader = Reader {
        limits,
        diagnostics: Diagnostics {
            list: Vec::new(),
            total: 0,
            limit: limits.max_diagnostics,
        },
        components: Vec::new(),
        refs: BTreeMap::new(),
        rejected: 0,
    };
    let empty = Map::new();
    let metadata = root
        .get("metadata")
        .and_then(Value::as_object)
        .unwrap_or(&empty);
    let subject = reader.subject(metadata);
    if let Some(list) = root.get("components") {
        reader.components_of(list, 0, "components")?;
    }
    let (dependency_entries, dependency_edges) = match root.get("dependencies") {
        Some(list) => reader.dependencies(list)?,
        None => (0, 0),
    };
    let lifecycle_phases = phases_of(metadata);
    let text = |key: &str| {
        bounded(root.get(key), limits.max_text)
            .ok()
            .flatten()
            .filter(|text| !text.is_empty())
    };
    Ok(ParsedSbom {
        spec_version,
        serial_number: text("serialNumber"),
        version: root.get("version").and_then(Value::as_u64),
        timestamp: bounded(metadata.get("timestamp"), 64).ok().flatten(),
        tools: tools_of(metadata),
        stage: stage_of(&lifecycle_phases),
        lifecycle_phases,
        subject,
        components: reader.components,
        dependency_entries,
        dependency_edges,
        signature_present: root.contains_key("signature"),
        ignored_sections: ignored_sections(root),
        rejected_components: reader.rejected,
        diagnostics: reader.diagnostics.list,
        diagnostics_total: reader.diagnostics.total,
    })
}
