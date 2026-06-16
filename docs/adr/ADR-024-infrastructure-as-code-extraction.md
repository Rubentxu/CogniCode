# ADR-024: Infrastructure-as-Code Extraction (Terraform + Ansible)

**Status:** Accepted  
**Date:** 2026-06-15  
**Source:** User request — Terraform and Ansible support

## Context

CogniCode's ingest pipeline targets code languages (Rust, Python, TS, etc.).
Infrastructure-as-Code (IaC) files — Terraform `.tf`/`.tfvars`/`.hcl` and
Ansible `.yml`/`.yaml` playbooks — are critical for understanding the full
architecture of a project. They define resources, dependencies, and
configurations that are first-class architectural concepts.

Two tree-sitter grammars are available with Rust support:

- **tree-sitter-hcl** (`tree-sitter-grammars/tree-sitter-hcl`, v1.2.0, Apache-2.0):
  Parses HCL with a built-in `dialects/terraform` directory. Handles blocks,
  attributes, expressions, and Terraform-specific syntax natively.
- **tree-sitter-yaml** (`tree-sitter-grammars/tree-sitter-yaml`, v0.7.2, MIT):
  Parses YAML 1.2. Ansible playbooks are YAML files that need a semantic
  layer on top of the YAML AST.

## Decision

Add Terraform and Ansible as **first-class infrastructure node kinds** in the
graph, with dedicated `LanguageConfig` entries and custom extraction handlers.

### Terraform extraction (via tree-sitter-hcl)

| HCL construct | GraphNode | NodeKind | Edges |
|---------------|-----------|----------|-------|
| `resource "aws_instance" "web"` | Resource node | `Symbol(Component)` | `References` to other resources |
| `data "aws_ami" "ubuntu"` | Data source node | `Symbol(Variable)` | `References` to resources |
| `variable "instance_count"` | Variable node | `Symbol(Variable)` | — |
| `module "vpc"` | Module node | `Symbol(Module)` | `Contains` to module resources |
| `provider "aws"` | Provider node | `Symbol(Module)` | — |
| `output "instance_ip"` | Output node | `Symbol(Property)` | `References` to resources |
| `locals` block | Local values | `Symbol(Variable)` | — |

**Edge extraction:**
- `aws_instance.web.ami` → `References` edge from the containing resource to
  the referenced resource/data/variable.
- `depends_on = [aws_instance.web]` → `References` edge with
  `Provenance::Extracted` (explicit dependency).
- `module.vpc.vpc_id` → `References` edge across module boundary.

```rust
pub static TERRAFORM_CONFIG: LanguageConfig = LanguageConfig {
    language: Language::Hcl,
    extensions: &[".tf", ".tfvars", ".hcl"],
    function_types: &["block"],         // resource/data/module/provider/output
    class_types: &["block"],            // same node type, different label
    import_types: &[],
    call_types: &[],
    import_handler: Some(handle_terraform_imports),
    // Custom: extract resource references from expressions
    type_ref_walker: Some(walk_hcl_references),
};
```

The `walk_hcl_references` walker traverses expression nodes (`scope_traversal`,
`get_attr`, `select_attr`) and extracts dotted references
(`aws_instance.web.ami`) as `References` edges to the referenced block's
GraphNode ID.

### Ansible extraction (via tree-sitter-yaml + semantic layer)

Ansible playbooks are YAML files, but the YAML AST alone doesn't identify
Ansible-specific constructs. A **semantic handler** interprets the YAML AST:

| YAML construct | GraphNode | NodeKind | Edges |
|----------------|-----------|----------|-------|
| Playbook file | Playbook node | `Symbol(Module)` | `Contains` plays |
| Play (`- hosts: webservers`) | Play node | `Symbol(Function)` | `Contains` tasks, `References` roles |
| Task (`- name: Install nginx`) | Task node | `Symbol(Function)` | `Calls` module |
| Module (`apt:` / `file:` / `template:`) | Module node | `Symbol(Function)` | — |
| `vars:` / `vars_files:` | Variable node | `Symbol(Variable)` | — |
| `roles:` | Role reference | — | `References` role |
| `handlers:` | Handler node | `Symbol(Function)` | `Calls` module |
| `import_playbook:` | Import edge | — | `Imports` other playbook |
| `include_tasks:` / `import_tasks:` | Import edge | — | `Imports` task file |

```rust
pub static ANSIBLE_CONFIG: LanguageConfig = LanguageConfig {
    language: Language::Yaml,
    extensions: &[".yml", ".yaml"],
    function_types: &["block_sequence"],     // plays, tasks
    class_types: &[],
    import_types: &["block_mapping"],         // import_playbook, include_tasks
    call_types: &[],
    import_handler: Some(handle_ansible_imports),
    // Custom: semantic handler identifies Ansible structure from YAML
    type_ref_walker: None,  // YAML has no type annotations
    // NEW: semantic handler for IaC-specific extraction
    semantic_handler: Some(interpret_ansible_playbook),
};
```

`interpret_ansible_playbook` is a post-parse pass that:
1. Detects if the YAML file is an Ansible playbook (has `hosts:` key in a
   list item, or `tasks:` key, or `roles:` key).
2. If yes, extracts plays, tasks, modules, variables as GraphNodes.
3. If no (plain YAML config file), extracts top-level keys as variable nodes.

### Node ID scheme

| IaC type | Node ID format | Example |
|----------|---------------|---------|
| Terraform resource | `tf:{file}:{type}.{name}` | `tf:main.tf:aws_instance.web` |
| Terraform variable | `tf:{file}:variable.{name}` | `tf:variables.tf:instance_count` |
| Terraform module | `tf:{file}:module.{name}` | `tf:main.tf:module.vpc` |
| Ansible playbook | `ansible:{file}:playbook` | `ansible:site.yml:playbook` |
| Ansible play | `ansible:{file}:play.{index}` | `ansible:site.yml:play.0` |
| Ansible task | `ansible:{file}:task.{play}.{index}` | `ansible:site.yml:task.0.3` |
| Ansible module | `ansible:builtin:{module_name}` | `ansible:builtin:apt` |

The `ansible:builtin:*` nodes are shared across all playbooks — `apt`, `file`,
`template`, `copy`, `service`, etc. are well-known modules. They accumulate
fan-in (every task that calls `apt:` links to `ansible:builtin:apt`).

### Cross-referencing with code

Terraform and Ansible nodes coexist with code nodes in the same graph. This
enables powerful queries:

- "Which application symbols are deployed by which Terraform resource?"
  (requires a linking mechanism — future, via tags/annotations)
- "What Ansible tasks reference the config file that this Rust struct
  deserializes?" (via filename matching)

For v1, IaC nodes live in the same `graph_nodes`/`graph_edges` tables with
`workspace_id` filtering. Cross-domain edges (code ↔ IaC) are Phase 2.

## Rationale

- **tree-sitter-hcl** is mature (1892/1892 real-world Terraform files parse at
  100% success rate). It handles all HCL constructs including Terraform's
  expression language (`for` loops, conditionals, splat expressions).
- **Ansible via tree-sitter-yaml** is the correct approach because Ansible
  playbooks ARE YAML. The semantic layer handles Ansible-specific structure
  without inventing a new parser.
- **Shared builtin module nodes** for Ansible (`ansible:builtin:apt`) create
  natural fan-in hotspots — a module used 200 times appears as a high-fan-in
  node, which is architecturally meaningful.
- **Resource references** in Terraform (`aws_instance.web.ami`) are deterministic
  and map cleanly to `References` edges — no heuristic resolution needed.

## Consequences

- Two new tree-sitter dependencies: `tree-sitter-hcl` and `tree-sitter-yaml`.
  Both are published crates with Cargo.toml.
- The `LanguageConfig` struct needs an optional `semantic_handler` field for
  post-parse semantic interpretation (Ansible). This is additive — code
  languages leave it as `None`.
- The `Language` enum gains `Hcl` and `Yaml` variants.
- Ansible builtin modules (`ansible:builtin:*`) are created as nodes on first
  reference and accumulate across playbooks. They need periodic cleanup if
  playbooks are deleted (handled by per-file DELETE in PgUpsert).
- Node IDs for IaC use a domain prefix (`tf:`, `ansible:`) to avoid collisions
  with code symbol IDs.

## Alternatives Considered

- **Separate IaC graph:** maintain a parallel graph for infrastructure nodes.
  Rejected — loses the ability to query across code and infrastructure. The
  value is in the unified graph.
- **Regex-based extraction (no tree-sitter):** rejected — HCL and YAML syntax
  is too complex for regex. Tree-sitter provides reliable AST parsing.
- **Only Terraform, skip Ansible:** Ansible is requested by the user. Its
  extraction via tree-sitter-yaml + semantic handler is straightforward.
