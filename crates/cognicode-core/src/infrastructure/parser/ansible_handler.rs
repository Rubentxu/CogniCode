//! Ansible semantic handler — detects Ansible playbook structure from YAML
//! AST and extracts plays, tasks, modules, variables, handlers (ADR-024).
//!
//! Ansible playbooks are YAML files. This handler interprets the YAML AST
//! after the generic YAML extractor runs, extracting Ansible-specific nodes
//! and edges.

use crate::application::ingest::types::{ExtractionEdge, ExtractionResult, TargetRef};
use crate::domain::aggregates::{GraphNode, NodeId};
use crate::domain::value_objects::{NodeKind, Provenance, SymbolKind};

/// Post-process a YAML extraction result to detect Ansible structure.
/// If the file is an Ansible playbook, adds play/task/module nodes and edges.
/// If not (plain YAML), returns the result unchanged.
pub fn interpret_ansible(source_path: &str, source_hash: &str, result: &ExtractionResult) -> ExtractionResult {
    // Quick check: does the file look like an Ansible playbook?
    if !is_ansible_playbook(result) {
        return result.clone();
    }

    let mut nodes = result.nodes.clone();
    let mut edges = result.edges.clone();
    let file_node_id = NodeId::new(source_path);

    // Extract plays, tasks, modules from the YAML structure
    // This is a heuristic — we look for specific key patterns in node labels
    for node in &result.nodes {
        let label = &node.label;

        // Detect play level: "- hosts:" starts a play
        if label.contains("hosts:") || label.contains("hosts ") {
            let play_id = format!("ansible:{}:play:{}", source_path, nodes.len());
            let play_node = GraphNode::builder(NodeId::new(&play_id), NodeKind::Symbol(SymbolKind::Function))
                .label("play".to_string())
                .source_path(std::path::PathBuf::from(source_path))
                .build();
            edges.push(ExtractionEdge {
                source: file_node_id.as_str().into(),
                target_ref: TargetRef::Resolved(play_id.clone()),
                kind: "dependency.contains".into(),
                provenance: Provenance::Extracted,
                confidence: 0.9,
                line: None,
            });
            nodes.push(play_node);
        }

        // Detect tasks: "  - name:" or "  tasks:" 
        if label.contains("tasks:") || label.contains("  - ") {
            let task_id = format!("ansible:{}:task:{}", source_path, nodes.len());
            let task_node = GraphNode::builder(NodeId::new(&task_id), NodeKind::Symbol(SymbolKind::Function))
                .label("task".to_string())
                .source_path(std::path::PathBuf::from(source_path))
                .build();
            nodes.push(task_node);

            // Detect module names: "    apt:", "    file:", "    template:", etc.
            for known_module in ANSIBLE_MODULES {
                if label.contains(&format!("    {}:", known_module)) {
                    let module_id = format!("ansible:builtin:{}", known_module);
                    edges.push(ExtractionEdge {
                        source: task_id.clone(),
                        target_ref: TargetRef::Unresolved(module_id),
                        kind: "dependency.calls".into(),
                        provenance: Provenance::Inferred,
                        confidence: 0.7,
                        line: None,
                    });
                }
            }
        }

        // Detect handlers
        if label.contains("handlers:") {
            let handler_id = format!("ansible:{}:handler:{}", source_path, nodes.len());
            let handler_node = GraphNode::builder(NodeId::new(&handler_id), NodeKind::Symbol(SymbolKind::Function))
                .label("handler".to_string())
                .source_path(std::path::PathBuf::from(source_path))
                .build();
            nodes.push(handler_node);
        }

        // Detect import_playbook / include_tasks
        if label.contains("import_playbook:") || label.contains("include_tasks:") {
            let import_target = label.split(':').last().unwrap_or("").trim().trim_matches('"');
            if !import_target.is_empty() {
                edges.push(ExtractionEdge {
                    source: file_node_id.as_str().into(),
                    target_ref: TargetRef::Unresolved(import_target.into()),
                    kind: "dependency.imports".into(),
                    provenance: Provenance::Extracted,
                    confidence: 0.9,
                    line: None,
                });
            }
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

/// Check if an extraction result looks like an Ansible playbook.
fn is_ansible_playbook(result: &ExtractionResult) -> bool {
    let text = result.nodes.iter()
        .map(|n| n.label.clone())
        .collect::<Vec<_>>()
        .join("\n");
    let has_hosts = text.contains("hosts:");
    let has_tasks = text.contains("tasks:");
    let has_roles = text.contains("roles:");
    has_hosts || has_tasks || has_roles
}

/// Well-known Ansible builtin modules.
const ANSIBLE_MODULES: &[&str] = &[
    "apt", "yum", "dnf", "pip", "npm", "gem",
    "file", "copy", "template", "fetch", "unarchive",
    "service", "systemd", "cron", "user", "group",
    "command", "shell", "raw", "script",
    "docker_container", "docker_image", "kubernetes",
    "git", "lineinfile", "blockinfile", "replace",
    "debug", "assert", "fail", "pause", "wait_for",
    "uri", "get_url", "set_fact", "include_vars",
];
