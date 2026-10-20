//! Ansible semantic handler: a playbook is a structure, and the structure
//! comes from the tree.
//!
//! ## Why this handler parses the file itself
//!
//! A generic extraction pass reduces a YAML file to the node of the file
//! itself, so a handler that reads labels never sees a play: the word
//! `hosts:` is not a label, it is a key inside a tree. The previous version
//! of this handler compared labels against the text of a playbook, which
//! meant it never ran, and it compared a task against a list of the modules
//! somebody had remembered —so `postgresql_query`, absent from that list,
//! was invisible in a task the handler had found.
//!
//! Reading the file here gives the domain layer the authority over its own
//! language: it decides what a play is, what a task calls, and what it
//! pulls in. The generic pass stays what it is —the path taken by the
//! languages that have no domain layer of their own.
//!
//! ## What the file states, and what it does not
//!
//! A playbook states its own structure: the plays it has, the tasks of each
//! play, the module each task calls, the roles it pulls in. What it does not
//! state is where those roles live, and that is why a role comes back
//! unresolved: the relation is real, the destination is in another file, and
//! no node is invented for it. The coverage of a playbook is *per file*.
//!
//! ## What proves a file is a playbook
//!
//! A play is proved by `hosts:` —the keyword that says who it runs on— and a
//! task is proved by naming itself and calling something, or by grouping
//! other tasks: a task without a name is one `ansible-lint` asks to be
//! fixed. A YAML list of records that does none of that is not a list of
//! tasks, and the file is left exactly as the generic extraction left it.

use crate::application::ingest::types::{ExtractionEdge, ExtractionResult, TargetRef};
use crate::domain::aggregates::{GraphNode, NodeId};
use crate::domain::value_objects::{DependencyType, NodeKind, Provenance, SymbolKind};
use std::collections::BTreeMap;
use tree_sitter::Node;

/// The keywords of Ansible, closed by Ansible itself.
///
/// The list of *modules* is open —a collection adds one every day— but the
/// list of keywords is not: Ansible defines it, and it is what tells a
/// module apart from a modifier. The module of a task is the key that is
/// left once the keywords are removed, so the module that nobody
/// remembered is still found.
const KEYWORDS: &[&str] = &[
    "action",
    "always",
    "any_errors_fatal",
    "args",
    "async",
    "become",
    "become_exe",
    "become_flags",
    "become_method",
    "become_user",
    "block",
    "changed_when",
    "check_mode",
    "collections",
    "connection",
    "debugger",
    "delay",
    "delegate_facts",
    "delegate_to",
    "diff",
    "environment",
    "failed_when",
    "force_handlers",
    "gather_facts",
    "handlers",
    "hosts",
    "ignore_errors",
    "include",
    "include_role",
    "include_tasks",
    "import_playbook",
    "import_role",
    "import_tasks",
    "listen",
    "local_action",
    "loop",
    "loop_control",
    "max_fail_percentage",
    "module_defaults",
    "name",
    "no_log",
    "notify",
    "order",
    "poll",
    "port",
    "post_tasks",
    "pre_tasks",
    "register",
    "remote_user",
    "rescue",
    "retries",
    "role",
    "roles",
    "run_once",
    "serial",
    "strategy",
    "tags",
    "tasks",
    "throttle",
    "timeout",
    "until",
    "vars",
    "vars_files",
    "vars_prompt",
    "when",
];

/// The keys that pull in another file instead of calling a module.
///
/// Ansible calls all three an import —`import_playbook`, `import_tasks`,
/// `import_role`— and the two legacy `include_*` spellings are the same
/// idea written the old way.
const IMPORT_KEYS: &[&str] = &[
    "include",
    "include_role",
    "include_tasks",
    "import_playbook",
    "import_role",
    "import_tasks",
];

/// The keys of a play whose value is a list of tasks.
const TASK_LIST_KEYS: &[&str] = &["post_tasks", "pre_tasks", "tasks"];

/// The keys of a task whose value is a list of tasks.
///
/// A block is a task that groups other tasks, and Ansible gives it three
/// names: the block itself, the rescue and the always. The tasks they hold
/// belong to the block, not to the play that holds the block.
const BLOCK_KEYS: &[&str] = &["always", "block", "rescue"];

/// The keys whose value is a list of handlers.
const HANDLER_LIST_KEYS: &[&str] = &["handlers"];

/// Interprets a YAML file as Ansible.
///
/// When the file declares no play and no task, the result comes back
/// untouched: a YAML file of another dialect is not a playbook.
pub fn interpret_ansible(
    source_path: &str,
    source_hash: &str,
    source: &str,
    result: &ExtractionResult,
) -> ExtractionResult {
    let Some(tree) = parse(source) else {
        return result.clone();
    };
    let items = top_level_items(tree.root_node());
    if items.is_empty() {
        return result.clone();
    }

    let bytes = source.as_bytes();
    let file_id = NodeId::new(source_path);
    let mut world = World {
        source_path,
        source: bytes,
        names: Names::default(),
        declared: 0,
        plays: 0,
        tasks: 0,
        handlers: 0,
        nodes: Vec::new(),
        edges: Vec::new(),
    };

    // The node of the file comes from the generic path and is not
    // reinterpreted: a file is not a declaration.
    if let Some(file_node) = result.nodes.first() {
        world.nodes.push(file_node.clone());
    }

    for item in items {
        let Some(mapping) = Mapping::of(item, bytes) else {
            continue;
        };
        if mapping.has("hosts") {
            world.play(&mapping, file_id.as_str());
        } else if mapping.is_task() {
            world.task(&mapping, file_id.as_str());
        } else if let Some(import) = mapping.import() {
            world.import_edge(&mapping, file_id.as_str(), &import);
        }
    }

    if world.declared == 0 {
        // Nothing was declared: the file is not a playbook, and it comes
        // back exactly as the generic extraction left it.
        return result.clone();
    }

    ExtractionResult {
        source_path: std::path::PathBuf::from(source_path),
        nodes: world.nodes,
        edges: world.edges,
        content_hash: source_hash.into(),
        statements_by_function: std::collections::BTreeMap::new(),
        error: result.error.clone(),
    }
}

/// The state the interpretation needs while it walks the file.
struct World<'a> {
    source_path: &'a str,
    source: &'a [u8],
    /// Keeps the things the file declares apart from each other.
    names: Names,
    /// How many things the file declares. Zero means the file is not a
    /// playbook, and the result comes back untouched.
    declared: usize,
    /// How many plays, tasks and handlers have been declared so far.
    plays: usize,
    tasks: usize,
    handlers: usize,
    nodes: Vec<GraphNode>,
    edges: Vec<ExtractionEdge>,
}

impl<'a> World<'a> {
    /// Declares a play: the unit that says who it runs on.
    fn play(&mut self, mapping: &Mapping<'a>, container: &str) {
        self.declared += 1;
        self.plays += 1;
        let ordinal = self.plays;
        let name = mapping.text("name");
        let identity = self.names.of(name.as_deref(), ordinal, "play");
        let id = self.id("play", &identity.slug);
        self.node(&id, &identity.label, SymbolKind::Module, mapping.line());
        self.edge(
            container,
            DependencyType::Contains,
            TargetRef::Resolved(id.clone()),
            mapping.line(),
        );

        for role in mapping.roles() {
            // The role lives in another file, so the relation is real and
            // its destination is not: it comes back unresolved, and no node
            // is invented for it.
            self.edge(
                &id,
                DependencyType::Imports,
                TargetRef::Unresolved(format!("ansible:role:{role}")),
                mapping.line(),
            );
        }

        for key in TASK_LIST_KEYS {
            for item in mapping.items_of(key) {
                let Some(child) = Mapping::of(item, self.source) else {
                    continue;
                };
                if child.is_task() {
                    self.task(&child, &id);
                }
            }
        }

        for key in HANDLER_LIST_KEYS {
            for item in mapping.items_of(key) {
                let Some(child) = Mapping::of(item, self.source) else {
                    continue;
                };
                if child.has("name") {
                    self.handler(&child, &id);
                }
            }
        }
    }

    /// Declares a task: the step of a play, or a step of a role.
    fn task(&mut self, mapping: &Mapping<'a>, container: &str) {
        self.declared += 1;
        self.tasks += 1;
        let ordinal = self.tasks;
        let name = mapping.text("name");
        let identity = self.names.of(name.as_deref(), ordinal, "task");
        let id = self.id("task", &identity.slug);
        self.node(&id, &identity.label, SymbolKind::Function, mapping.line());
        self.edge(
            container,
            DependencyType::Contains,
            TargetRef::Resolved(id.clone()),
            mapping.line(),
        );

        if let Some(module) = mapping.module() {
            self.edge(
                &id,
                DependencyType::Calls,
                TargetRef::Unresolved(module_target(&module)),
                mapping.line(),
            );
        }

        if let Some(import) = mapping.import() {
            self.import_edge(mapping, &id, &import);
        }

        for key in BLOCK_KEYS {
            for item in mapping.items_of(key) {
                let Some(child) = Mapping::of(item, self.source) else {
                    continue;
                };
                if child.is_task() {
                    self.task(&child, &id);
                }
            }
        }
    }

    /// Declares a handler: the step a notification triggers.
    fn handler(&mut self, mapping: &Mapping<'a>, container: &str) {
        self.declared += 1;
        self.handlers += 1;
        let ordinal = self.handlers;
        let name = mapping.text("name");
        let identity = self.names.of(name.as_deref(), ordinal, "handler");
        let id = self.id("handler", &identity.slug);
        self.node(&id, &identity.label, SymbolKind::Function, mapping.line());
        self.edge(
            container,
            DependencyType::Contains,
            TargetRef::Resolved(id.clone()),
            mapping.line(),
        );

        if let Some(module) = mapping.module() {
            self.edge(
                &id,
                DependencyType::Calls,
                TargetRef::Unresolved(module_target(&module)),
                mapping.line(),
            );
        }
    }

    /// The relation of a file that pulls in another one.
    fn import_edge(&mut self, mapping: &Mapping<'a>, container: &str, import: &str) {
        let Some(target) = mapping.import_target(import) else {
            return;
        };
        self.declared += 1;
        self.edge(
            container,
            DependencyType::Imports,
            TargetRef::Unresolved(target),
            mapping.line(),
        );
    }

    /// The id of a declared thing in the graph.
    ///
    /// A single definition: the emitted node and the far end of the edge
    /// that points at it have to come from here, or a change of format
    /// separates them and the relation is lost in silence.
    fn id(&self, kind: &str, slug: &str) -> String {
        format!("ansible:{}:{}:{}", self.source_path, kind, slug)
    }

    fn node(&mut self, id: &str, label: &str, kind: SymbolKind, line: u32) {
        self.nodes.push(
            GraphNode::builder(NodeId::new(id), NodeKind::Symbol(kind))
                .label(label.to_string())
                .source_path(std::path::PathBuf::from(self.source_path))
                .property("line".to_string(), line.to_string())
                .build(),
        );
    }

    fn edge(&mut self, source: &str, kind: DependencyType, target_ref: TargetRef, line: u32) {
        self.edges.push(ExtractionEdge {
            source: source.to_string(),
            target_ref,
            kind: format!("dependency.{kind}"),
            provenance: Provenance::Extracted,
            confidence: 1.0,
            line: Some(line),
        });
    }
}

/// Where a module lives.
///
/// A module is a builtin of Ansible unless the file says otherwise with a
/// fully qualified name, and it lives in Ansible itself: the relation is
/// real and its destination is not in this file.
fn module_target(module: &str) -> String {
    if module.contains('.') {
        module.to_string()
    } else {
        format!("ansible:builtin:{module}")
    }
}

/// How a declared thing is called inside its file.
struct Identity {
    /// The name the reader sees.
    label: String,
    /// The name inside the id: no spaces, and no character that would blur
    /// the boundary between the parts of the id.
    slug: String,
}

/// Keeps the things a file declares apart from each other.
///
/// Ansible allows the same name twice, and the graph identifies a symbol by
/// its name inside its file, so the second one gets a number. The file is
/// the only thing that can tell them apart.
#[derive(Default)]
struct Names {
    seen: BTreeMap<String, usize>,
}

impl Names {
    /// The identity of the thing at `ordinal` that the file calls `name`.
    ///
    /// A thing the file does not name gets its number as its label: the
    /// number says where it is in the file, which is the only thing that
    /// distinguishes it.
    fn of(&mut self, name: Option<&str>, ordinal: usize, kind: &str) -> Identity {
        let label = match name {
            Some(name) => name.to_string(),
            None => format!("{kind} {ordinal}"),
        };
        let slug = match name {
            Some(name) => slugify(name),
            None => ordinal.to_string(),
        };
        // The counter is over the slug and not over the label, so a thing
        // the file does not name and a thing it names the same way end up
        // with different identities instead of the same one twice.
        let count = self.seen.entry(slug.clone()).or_insert(0);
        *count += 1;
        if *count == 1 {
            Identity { label, slug }
        } else {
            Identity {
                label: format!("{label} ({count})"),
                slug: format!("{slug}_{count}"),
            }
        }
    }
}

/// The name of a thing as it goes inside an id.
///
/// A space is a word separator and a colon separates the parts of an id, so
/// both become an underscore. Everything else that is not a letter, a digit
/// or a dash does the same, and two words do not become two underscores.
fn slugify(name: &str) -> String {
    let mut slug = String::with_capacity(name.len());
    let mut last_was_separator = false;
    for character in name.chars() {
        let keep = character.is_alphanumeric() || character == '-' || character == '_';
        if keep {
            slug.push(character);
            last_was_separator = false;
        } else if !last_was_separator {
            slug.push('_');
            last_was_separator = true;
        }
    }
    slug.trim_matches('_').to_string()
}

/// A YAML mapping, read once.
struct Mapping<'a> {
    node: Node<'a>,
    pairs: Vec<(String, Node<'a>)>,
    source: &'a [u8],
}

impl<'a> Mapping<'a> {
    /// Reads the mapping a sequence item holds.
    fn of(item: Node<'a>, source: &'a [u8]) -> Option<Mapping<'a>> {
        let node = mapping_of(item)?;
        let mut cursor = node.walk();
        let pairs = node
            .named_children(&mut cursor)
            .filter(|child| child.kind() == "block_mapping_pair")
            .filter_map(|pair| {
                let mut inner = pair.walk();
                let mut children = pair.named_children(&mut inner);
                let key = children.next()?;
                let value = children.next()?;
                Some((scalar(key, source), value))
            })
            .collect();
        Some(Mapping {
            node,
            pairs,
            source,
        })
    }

    fn has(&self, key: &str) -> bool {
        self.pairs.iter().any(|(name, _)| name == key)
    }

    /// The text of a key, when the key holds a scalar.
    fn text(&self, key: &str) -> Option<String> {
        let value = self.value(key)?;
        (value.kind() == "flow_node").then(|| scalar(value, self.source))
    }

    fn value(&self, key: &str) -> Option<Node<'a>> {
        self.pairs
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| *value)
    }

    /// Whether the mapping is a task: it names itself, and it either calls
    /// something or it groups other tasks.
    fn is_task(&self) -> bool {
        self.has("name") && (self.module().is_some() || self.has_block())
    }

    /// Whether the mapping groups other tasks.
    fn has_block(&self) -> bool {
        BLOCK_KEYS.iter().any(|key| self.has(key))
    }

    /// The module a task calls: the key that is not one of the keywords.
    fn module(&self) -> Option<String> {
        self.pairs
            .iter()
            .map(|(name, _)| name.as_str())
            .find(|name| !KEYWORDS.contains(name))
            .map(|name| name.to_string())
    }

    /// The key that pulls in another file, when the mapping is one.
    fn import(&self) -> Option<String> {
        self.pairs
            .iter()
            .map(|(name, _)| name.as_str())
            .find(|name| IMPORT_KEYS.contains(name))
            .map(|name| name.to_string())
    }

    /// What an import points at.
    ///
    /// `include_tasks: more.yml` names a file, and `import_role: name=nginx`
    /// names a role among other parameters. The name is what the reader
    /// needs, so the parameters are dropped.
    fn import_target(&self, import: &str) -> Option<String> {
        let value = self.value(import)?;
        if value.kind() != "flow_node" {
            return None;
        }
        let text = scalar(value, self.source);
        if !text.contains('=') {
            return (!text.is_empty()).then_some(text);
        }
        text.split_whitespace()
            .find_map(|parameter| parameter.strip_prefix("name="))
            .map(|name| name.to_string())
            .or_else(|| (!text.is_empty()).then_some(text))
    }

    /// The items of the list a key holds.
    fn items_of(&self, key: &str) -> Vec<Node<'a>> {
        let Some(value) = self.value(key) else {
            return Vec::new();
        };
        if value.kind() != "block_node" {
            return Vec::new();
        }
        let mut cursor = value.walk();
        let Some(sequence) = value
            .named_children(&mut cursor)
            .find(|child| child.kind() == "block_sequence")
        else {
            return Vec::new();
        };
        let mut cursor = sequence.walk();
        sequence
            .named_children(&mut cursor)
            .filter(|child| child.kind() == "block_sequence_item")
            .collect()
    }

    /// The roles a play uses.
    ///
    /// A role is written as a mapping —`- role: nginx`— or as a bare
    /// name, and both are read the same way.
    fn roles(&self) -> Vec<String> {
        let mut roles = Vec::new();
        for item in self.items_of("roles") {
            if let Some(mapping) = Mapping::of(item, self.source) {
                if let Some(role) = mapping.text("role") {
                    roles.push(role);
                }
                continue;
            }
            if let Some(text) = item_text(item, self.source) {
                roles.push(text);
            }
        }
        roles
    }

    fn line(&self) -> u32 {
        (self.node.start_position().row + 1) as u32
    }
}

/// The mapping a sequence item holds.
///
/// A sequence item is `- name: x`: the item itself is the dash, and the
/// content is a mapping one level in.
fn mapping_of(item: Node) -> Option<Node> {
    let mut cursor = item.walk();
    let block = item
        .named_children(&mut cursor)
        .find(|child| child.kind() == "block_node")?;
    let mut cursor = block.walk();
    block
        .named_children(&mut cursor)
        .find(|child| child.kind() == "block_mapping")
}

/// The text of a sequence item that is a bare word.
fn item_text(item: Node, source: &[u8]) -> Option<String> {
    let mut cursor = item.walk();
    let content = item
        .named_children(&mut cursor)
        .find(|child| child.kind() != "block_node")?;
    let text = scalar(content, source);
    (!text.is_empty()).then_some(text)
}

/// The items of the top-level sequence of a YAML document.
///
/// A playbook is a list: `- name: ...` repeated. A YAML file that is a
/// mapping —`version: 3` and a `services:` block— has no sequence at all,
/// and that is what tells the two apart before anything is read.
fn top_level_items(root: Node) -> Vec<Node> {
    let mut node = root;
    loop {
        if node.kind() == "block_sequence" {
            break;
        }
        let Some(child) = first_named_child(node) else {
            return Vec::new();
        };
        node = child;
    }
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .filter(|child| child.kind() == "block_sequence_item")
        .collect()
}

/// The first child that is not a comment.
///
/// A comment is a named node, and a document that opens with a comment
/// would make the descent stop on it.
fn first_named_child(node: Node) -> Option<Node> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .find(|child| child.kind() != "comment")
}

/// The text of a node, without the quotes a scalar may carry.
fn scalar(node: Node, source: &[u8]) -> String {
    let raw = String::from_utf8_lossy(&source[node.start_byte()..node.end_byte()]);
    let trimmed = raw.trim();
    let bytes = trimmed.as_bytes();
    if bytes.len() >= 2 {
        let first = bytes[0];
        let last = bytes[bytes.len() - 1];
        if (first == b'"' && last == b'"') || (first == b'\'' && last == b'\'') {
            return trimmed[1..trimmed.len() - 1].to_string();
        }
    }
    trimmed.to_string()
}

fn parse(source: &str) -> Option<tree_sitter::Tree> {
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&tree_sitter_yaml::LANGUAGE.into())
        .ok()?;
    parser.parse(source, None)
}
