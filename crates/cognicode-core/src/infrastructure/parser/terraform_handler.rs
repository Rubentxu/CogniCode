//! Terraform semantic handler — detects HCL block structure and extracts
//! resource, data, variable, module, output nodes with `tf:` prefixed IDs
//! and `References` edges (ADR-024 / ADR-036).
//!
//! This handler post-processes the generic HCL extraction result, replacing
//! generic `Symbol(Function)` nodes with domain-specific IaC nodes.

use crate::application::ingest::types::{ExtractionEdge, ExtractionResult, TargetRef};
use crate::domain::aggregates::{GraphNode, NodeId};
use crate::domain::value_objects::{DependencyType, NodeKind, Provenance, SymbolKind};

/// Post-process an HCL extraction result to detect Terraform structure.
///
/// If the file contains HCL blocks (resource, data, variable, module, output),
/// replaces generic nodes with typed IaC nodes using `tf:` prefixed IDs.
/// If the file is not Terraform, returns the result unchanged.
pub fn interpret_terraform(
    source_path: &str,
    source_hash: &str,
    result: &ExtractionResult,
) -> ExtractionResult {
    // Quick check: does the file look like Terraform?
    if !is_terraform_file(result) {
        return result.clone();
    }

    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let file_node_id = NodeId::new(source_path);

    // Keep the file-level node from the generic extraction
    if let Some(file_node) = result.nodes.first() {
        nodes.push(file_node.clone());
    }

    // Scan labels for Terraform block patterns
    for node in &result.nodes[1..] {
        let label = &node.label;

        // Detect resource blocks: "resource aws_instance web"
        if let Some((resource_type, resource_name)) = parse_terraform_resource(label) {
            let id = format!("tf:{}:{}.{}", source_path, resource_type, resource_name);
            let iac_node = GraphNode::builder(NodeId::new(&id), NodeKind::Symbol(SymbolKind::Function))
                .label(format!("{}.{}", resource_type, resource_name))
                .source_path(std::path::PathBuf::from(source_path))
                .property("iac_type".to_string(), resource_type.clone())
                .property("iac_kind".to_string(), "resource".to_string())
                .build();
            nodes.push(iac_node);
            edges.push(ExtractionEdge {
                source: file_node_id.as_str().to_string(),
                target_ref: TargetRef::Resolved(id.clone()),
                kind: format!("dependency.{}", DependencyType::Contains),
                provenance: Provenance::Extracted,
                confidence: 1.0,
                line: None,
            });

            // Extract References edges from attribute expressions
            // Look for dotted references like "aws_security_group.allow_ssh.id"
            extract_terraform_refs(label, &id, &mut edges);
        }

        // Detect data blocks: "data aws_ami ubuntu"
        if let Some((data_type, data_name)) = parse_terraform_data(label) {
            let id = format!("tf:{}:data.{}.{}", source_path, data_type, data_name);
            let iac_node = GraphNode::builder(NodeId::new(&id), NodeKind::Symbol(SymbolKind::Variable))
                .label(format!("data.{}.{}", data_type, data_name))
                .source_path(std::path::PathBuf::from(source_path))
                .property("iac_type".to_string(), data_type.clone())
                .property("iac_kind".to_string(), "data".to_string())
                .build();
            nodes.push(iac_node);
            edges.push(ExtractionEdge {
                source: file_node_id.as_str().to_string(),
                target_ref: TargetRef::Resolved(id.clone()),
                kind: format!("dependency.{}", DependencyType::Contains),
                provenance: Provenance::Extracted,
                confidence: 1.0,
                line: None,
            });
            extract_terraform_refs(label, &id, &mut edges);
        }

        // Detect variable blocks: "variable region"
        if let Some(var_name) = parse_terraform_variable(label) {
            let id = format!("tf:{}:var.{}", source_path, var_name);
            let iac_node = GraphNode::builder(NodeId::new(&id), NodeKind::Symbol(SymbolKind::Variable))
                .label(format!("var.{}", var_name))
                .source_path(std::path::PathBuf::from(source_path))
                .property("iac_kind".to_string(), "variable".to_string())
                .build();
            nodes.push(iac_node);
            edges.push(ExtractionEdge {
                source: file_node_id.as_str().to_string(),
                target_ref: TargetRef::Resolved(id.clone()),
                kind: format!("dependency.{}", DependencyType::Contains),
                provenance: Provenance::Extracted,
                confidence: 1.0,
                line: None,
            });
        }

        // Detect module blocks: "module vpc"
        if let Some(module_name) = parse_terraform_module(label) {
            let id = format!("tf:{}:module.{}", source_path, module_name);
            let iac_node = GraphNode::builder(NodeId::new(&id), NodeKind::Symbol(SymbolKind::Module))
                .label(format!("module.{}", module_name))
                .source_path(std::path::PathBuf::from(source_path))
                .property("iac_kind".to_string(), "module".to_string())
                .build();
            nodes.push(iac_node);
            edges.push(ExtractionEdge {
                source: file_node_id.as_str().to_string(),
                target_ref: TargetRef::Resolved(id.clone()),
                kind: format!("dependency.{}", DependencyType::Contains),
                provenance: Provenance::Extracted,
                confidence: 1.0,
                line: None,
            });
        }

        // Detect output blocks: "output instance_ip"
        if let Some(output_name) = parse_terraform_output(label) {
            let id = format!("tf:{}:output.{}", source_path, output_name);
            let iac_node = GraphNode::builder(NodeId::new(&id), NodeKind::Symbol(SymbolKind::Property))
                .label(format!("output.{}", output_name))
                .source_path(std::path::PathBuf::from(source_path))
                .property("iac_kind".to_string(), "output".to_string())
                .build();
            nodes.push(iac_node);
            edges.push(ExtractionEdge {
                source: file_node_id.as_str().to_string(),
                target_ref: TargetRef::Resolved(id.clone()),
                kind: format!("dependency.{}", DependencyType::Contains),
                provenance: Provenance::Extracted,
                confidence: 1.0,
                line: None,
            });
        }
    }

    ExtractionResult {
        source_path: std::path::PathBuf::from(source_path),
        nodes,
        edges,
        content_hash: source_hash.into(),
        error: result.error.clone(),
    }
}

/// Check if an extraction result looks like a Terraform file.
fn is_terraform_file(result: &ExtractionResult) -> bool {
    let text = result.nodes.iter()
        .map(|n| n.label.clone())
        .collect::<Vec<_>>()
        .join("\n");
    text.contains("resource ") || text.contains("data ") || text.contains("variable ") ||
    text.contains("module ") || text.contains("output ") || text.contains("provider ") ||
    text.contains("terraform ")
}

/// Parse "resource aws_instance web" → Some(("aws_instance", "web"))
fn parse_terraform_resource(label: &str) -> Option<(String, String)> {
    let parts: Vec<&str> = label.split_whitespace().collect();
    // Pattern: "resource" type name
    if parts.len() >= 3 && parts[0] == "resource" {
        return Some((parts[1].to_string(), parts[2].to_string()));
    }
    None
}

/// Parse "data aws_ami ubuntu" → Some(("aws_ami", "ubuntu"))
fn parse_terraform_data(label: &str) -> Option<(String, String)> {
    let parts: Vec<&str> = label.split_whitespace().collect();
    if parts.len() >= 3 && parts[0] == "data" {
        return Some((parts[1].to_string(), parts[2].to_string()));
    }
    None
}

/// Parse "variable region" → Some("region")
fn parse_terraform_variable(label: &str) -> Option<String> {
    let parts: Vec<&str> = label.split_whitespace().collect();
    if parts.len() >= 2 && parts[0] == "variable" {
        return Some(parts[1].to_string());
    }
    None
}

/// Parse "module vpc" → Some("vpc")
fn parse_terraform_module(label: &str) -> Option<String> {
    let parts: Vec<&str> = label.split_whitespace().collect();
    if parts.len() >= 2 && parts[0] == "module" {
        return Some(parts[1].to_string());
    }
    None
}

/// Parse "output instance_ip" → Some("instance_ip")
fn parse_terraform_output(label: &str) -> Option<String> {
    let parts: Vec<&str> = label.split_whitespace().collect();
    if parts.len() >= 2 && parts[0] == "output" {
        return Some(parts[1].to_string());
    }
    None
}

/// Extract References edges from a label containing dotted expressions.
///
/// E.g., if label contains "aws_security_group.allow_ssh.id", creates a
/// References edge from the source node to "aws_security_group.allow_ssh".
fn extract_terraform_refs(label: &str, source_id: &str, edges: &mut Vec<ExtractionEdge>) {
    // Look for patterns like "type.name.attr" (3+ segments)
    for word in label.split_whitespace() {
        let segments: Vec<&str> = word.split('.').collect();
        if segments.len() >= 3 {
            // Could be a reference like "aws_security_group.allow_ssh.id"
            // The reference target is all segments except the last (attribute)
            let ref_target = segments[..segments.len()-1].join(".");
            edges.push(ExtractionEdge {
                source: source_id.to_string(),
                target_ref: TargetRef::Unresolved(ref_target),
                kind: format!("dependency.{}", DependencyType::References),
                provenance: Provenance::Inferred,
                confidence: 0.8,
                line: None,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_terraform_resource() {
        assert_eq!(
            parse_terraform_resource("resource aws_instance web"),
            Some(("aws_instance".into(), "web".into()))
        );
        assert_eq!(parse_terraform_resource("data aws_ami ubuntu"), None);
        assert_eq!(parse_terraform_resource("variable region"), None);
    }

    #[test]
    fn test_parse_terraform_data() {
        assert_eq!(
            parse_terraform_data("data aws_ami ubuntu"),
            Some(("aws_ami".into(), "ubuntu".into()))
        );
        assert_eq!(parse_terraform_data("resource aws_instance web"), None);
    }

    #[test]
    fn test_parse_terraform_variable() {
        assert_eq!(parse_terraform_variable("variable region"), Some("region".into()));
        assert_eq!(parse_terraform_variable("resource aws_instance web"), None);
    }

    #[test]
    fn test_is_terraform_file() {
        let result = ExtractionResult::ok(
            std::path::PathBuf::from("main.tf"),
            "hash".to_string(),
            vec![
                GraphNode::builder(NodeId::new("file"), NodeKind::Symbol(SymbolKind::File))
                    .label("main.tf".to_string())
                    .build(),
                GraphNode::builder(NodeId::new("r1"), NodeKind::Symbol(SymbolKind::Function))
                    .label("resource aws_instance web".to_string())
                    .build(),
            ],
            vec![],
        );
        assert!(is_terraform_file(&result));

        let plain = ExtractionResult::ok(
            std::path::PathBuf::from("lib.rs"),
            "hash".to_string(),
            vec![
                GraphNode::builder(NodeId::new("file"), NodeKind::Symbol(SymbolKind::File))
                    .label("lib.rs".to_string())
                    .build(),
                GraphNode::builder(NodeId::new("fn1"), NodeKind::Symbol(SymbolKind::Function))
                    .label("main".to_string())
                    .build(),
            ],
            vec![],
        );
        assert!(!is_terraform_file(&plain));
    }

    #[test]
    fn test_extract_terraform_refs() {
        let mut edges = Vec::new();
        extract_terraform_refs(
            "resource aws_instance web { ami = data.aws_ami.ubuntu.id }",
            "tf:main.tf:aws_instance.web",
            &mut edges,
        );
        // Should find "data.aws_ami.ubuntu.id" → reference to "data.aws_ami.ubuntu"
        assert!(!edges.is_empty());
        assert!(edges.iter().any(|e| matches!(&e.target_ref, TargetRef::Unresolved(r) if r == "data.aws_ami.ubuntu")));
    }
}
