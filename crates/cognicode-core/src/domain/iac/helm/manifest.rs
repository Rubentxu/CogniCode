//! Concrete Kubernetes objects produced by a real render.
//!
//! A [`ManifestObject`] exists only because an engine rendered the chart with
//! fixed values; it records where it came from (chart, template, document
//! position, values digest) and the security-relevant facts of its kind. Kinds
//! without a model are counted, never guessed at.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Where an object came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObjectOrigin {
    /// Chart name.
    pub chart: String,
    /// Chart version.
    pub chart_version: String,
    /// The template the engine reported for the document, when it did.
    pub template: Option<String>,
    /// Position of the document in the rendered stream.
    pub document_index: usize,
    /// Digest of the effective values the render used.
    pub values_digest: String,
}

/// Identity of an object across two renders.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ManifestObjectId {
    /// Kubernetes `kind`.
    pub kind: String,
    /// `metadata.namespace`, when set.
    pub namespace: Option<String>,
    /// `metadata.name`.
    pub name: String,
}

impl std::fmt::Display for ManifestObjectId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.namespace {
            Some(ns) => write!(f, "{}/{ns}/{}", self.kind, self.name),
            None => write!(f, "{}/{}", self.kind, self.name),
        }
    }
}

/// An image reference split into the parts that decide mutability.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageRef {
    /// The reference as written.
    pub raw: String,
    /// Tag, when present.
    pub tag: Option<String>,
    /// Digest (`sha256:..`), when present.
    pub digest: Option<String>,
}

impl ImageRef {
    /// Split `raw` into tag and digest. A registry port (`host:5000/img`) is
    /// not a tag.
    pub fn parse(raw: &str) -> Self {
        let (name, digest) = match raw.split_once('@') {
            Some((name, digest)) => (name, Some(digest.to_string())),
            None => (raw, None),
        };
        let last = name.rsplit('/').next().unwrap_or(name);
        let tag = last.split_once(':').map(|(_, tag)| tag.to_string());
        Self {
            raw: raw.to_string(),
            tag,
            digest,
        }
    }

    /// Whether the reference can resolve to different content over time: no
    /// digest, and a missing or `latest` tag.
    pub fn is_mutable(&self) -> bool {
        self.digest.is_none() && self.tag.as_deref().is_none_or(|tag| tag == "latest")
    }
}

/// Security-relevant facts of one container.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContainerFacts {
    /// Container name.
    pub name: String,
    /// Whether it is an init container.
    pub init: bool,
    /// Image, when the container declares one.
    pub image: Option<ImageRef>,
    /// `securityContext.privileged`.
    pub privileged: Option<bool>,
    /// `securityContext.allowPrivilegeEscalation`.
    pub allow_privilege_escalation: Option<bool>,
    /// `securityContext.runAsNonRoot`.
    pub run_as_non_root: Option<bool>,
    /// `securityContext.runAsUser`.
    pub run_as_user: Option<i64>,
    /// `securityContext.readOnlyRootFilesystem`.
    pub read_only_root_filesystem: Option<bool>,
    /// `securityContext.capabilities.add`.
    pub capabilities_add: Vec<String>,
}

/// Security-relevant facts of a pod template.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct PodFacts {
    /// `hostNetwork`.
    pub host_network: bool,
    /// `hostPID`.
    pub host_pid: bool,
    /// `hostIPC`.
    pub host_ipc: bool,
    /// Pod-level `securityContext.runAsNonRoot`.
    pub run_as_non_root: Option<bool>,
    /// Pod-level `securityContext.runAsUser`.
    pub run_as_user: Option<i64>,
    /// `serviceAccountName`.
    pub service_account: Option<String>,
    /// Paths of `hostPath` volumes.
    pub host_paths: Vec<String>,
}

/// A workload: an object that runs containers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkloadFacts {
    /// `spec.replicas`, when set.
    pub replicas: Option<i64>,
    /// Pod-level facts.
    pub pod: PodFacts,
    /// Containers and init containers.
    pub containers: Vec<ContainerFacts>,
}

/// One RBAC policy rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RbacRule {
    /// `apiGroups`.
    pub api_groups: Vec<String>,
    /// `resources`.
    pub resources: Vec<String>,
    /// `verbs`.
    pub verbs: Vec<String>,
}

impl RbacRule {
    /// Whether any of verbs, resources or API groups is the wildcard.
    pub fn has_wildcard(&self) -> bool {
        [&self.api_groups, &self.resources, &self.verbs]
            .iter()
            .any(|list| list.iter().any(|item| item == "*"))
    }
}

/// A subject of a binding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RbacSubject {
    /// `User`, `Group` or `ServiceAccount`.
    pub kind: String,
    /// Subject name.
    pub name: String,
}

/// RBAC facts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RbacFacts {
    /// A `Role` or `ClusterRole`.
    Role {
        /// Its rules.
        rules: Vec<RbacRule>,
    },
    /// A `RoleBinding` or `ClusterRoleBinding`.
    Binding {
        /// The referenced role kind.
        role_kind: String,
        /// The referenced role name.
        role_name: String,
        /// Who is granted the role.
        subjects: Vec<RbacSubject>,
    },
}

/// A port a service or ingress publishes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublishedPort {
    /// Port number or name as written.
    pub port: String,
    /// `nodePort`, when set.
    pub node_port: Option<i64>,
}

/// How an object is reachable from outside its pods.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExposureFacts {
    /// A `Service`.
    Service {
        /// `spec.type` (`ClusterIP` when unset).
        service_type: String,
        /// Ports.
        ports: Vec<PublishedPort>,
    },
    /// An `Ingress`.
    Ingress {
        /// Hosts, `*` for a rule without one.
        hosts: Vec<String>,
        /// Whether `spec.tls` is present.
        tls: bool,
    },
}

/// The modeled facts of an object, or the statement that its kind is not
/// modeled.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObjectFacts {
    /// Deployment, StatefulSet, DaemonSet, ReplicaSet, Job, CronJob, Pod.
    Workload(WorkloadFacts),
    /// Role, ClusterRole, RoleBinding, ClusterRoleBinding.
    Rbac(RbacFacts),
    /// Service, Ingress.
    Exposure(ExposureFacts),
    /// Any other kind: present, but with no security model.
    Unmodeled,
}

/// A concrete object with its origin and facts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestObject {
    /// Identity.
    pub id: ManifestObjectId,
    /// `apiVersion`.
    pub api_version: String,
    /// Provenance.
    pub origin: ObjectOrigin,
    /// Modeled facts.
    pub facts: ObjectFacts,
}

/// A rendered document that could not become an object.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkippedDocument {
    /// Position in the rendered stream.
    pub document_index: usize,
    /// Why it was skipped.
    pub reason: String,
}

/// The objects of one render.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ManifestModel {
    /// Objects in render order.
    pub objects: Vec<ManifestObject>,
    /// Documents that were not objects.
    pub skipped: Vec<SkippedDocument>,
}

/// A parsed document of a rendered stream.
#[derive(Debug, Clone, PartialEq)]
pub struct RenderedDocument {
    /// The template named by the engine's `# Source:` comment.
    pub template: Option<String>,
    /// Position in the stream (counting empty documents).
    pub index: usize,
    /// The document.
    pub value: Value,
}

impl ManifestModel {
    /// Build the model from rendered documents. `origin` supplies chart and
    /// values identity; template and index come from each document.
    pub fn from_documents(
        documents: &[RenderedDocument],
        chart: &str,
        chart_version: &str,
        values_digest: &str,
    ) -> Self {
        let mut model = Self::default();
        for doc in documents {
            let origin = ObjectOrigin {
                chart: chart.to_string(),
                chart_version: chart_version.to_string(),
                template: doc.template.clone(),
                document_index: doc.index,
                values_digest: values_digest.to_string(),
            };
            model.absorb(&doc.value, origin, doc.index);
        }
        model
    }

    fn absorb(&mut self, value: &Value, origin: ObjectOrigin, index: usize) {
        if value.is_null() {
            return;
        }
        let kind = value.get("kind").and_then(Value::as_str);
        if kind == Some("List") {
            let items = value.get("items").and_then(Value::as_array);
            for item in items.into_iter().flatten() {
                self.absorb(item, origin.clone(), index);
            }
            return;
        }
        match object_from(value, origin) {
            Ok(object) => self.objects.push(object),
            Err(reason) => self.skipped.push(SkippedDocument {
                document_index: index,
                reason,
            }),
        }
    }

    /// Kinds present without a security model, sorted and deduplicated.
    pub fn unmodeled_kinds(&self) -> Vec<String> {
        let mut kinds: Vec<String> = self
            .objects
            .iter()
            .filter(|o| o.facts == ObjectFacts::Unmodeled)
            .map(|o| o.id.kind.clone())
            .collect();
        kinds.sort();
        kinds.dedup();
        kinds
    }
}

fn object_from(value: &Value, origin: ObjectOrigin) -> Result<ManifestObject, String> {
    let kind = value
        .get("kind")
        .and_then(Value::as_str)
        .ok_or("document has no `kind`")?;
    let metadata = value.get("metadata");
    let name = metadata
        .and_then(|m| m.get("name"))
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{kind} has no `metadata.name`"))?;
    let namespace = metadata
        .and_then(|m| m.get("namespace"))
        .and_then(Value::as_str)
        .map(str::to_string);
    Ok(ManifestObject {
        id: ManifestObjectId {
            kind: kind.to_string(),
            namespace,
            name: name.to_string(),
        },
        api_version: value
            .get("apiVersion")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        origin,
        facts: facts_of(kind, value),
    })
}

fn facts_of(kind: &str, value: &Value) -> ObjectFacts {
    match kind {
        "Pod" => ObjectFacts::Workload(workload(value.get("spec"), None)),
        "Deployment" | "StatefulSet" | "DaemonSet" | "ReplicaSet" | "Job" => {
            let pod = value.pointer("/spec/template/spec");
            ObjectFacts::Workload(workload(pod, int_at(value, "/spec/replicas")))
        }
        "CronJob" => {
            let pod = value.pointer("/spec/jobTemplate/spec/template/spec");
            ObjectFacts::Workload(workload(pod, None))
        }
        "Role" | "ClusterRole" => ObjectFacts::Rbac(RbacFacts::Role {
            rules: array_at(value, "/rules").iter().map(rbac_rule).collect(),
        }),
        "RoleBinding" | "ClusterRoleBinding" => ObjectFacts::Rbac(RbacFacts::Binding {
            role_kind: str_at(value, "/roleRef/kind").unwrap_or_default(),
            role_name: str_at(value, "/roleRef/name").unwrap_or_default(),
            subjects: array_at(value, "/subjects")
                .iter()
                .map(|s| RbacSubject {
                    kind: str_at(s, "/kind").unwrap_or_default(),
                    name: str_at(s, "/name").unwrap_or_default(),
                })
                .collect(),
        }),
        "Service" => ObjectFacts::Exposure(ExposureFacts::Service {
            service_type: str_at(value, "/spec/type").unwrap_or_else(|| "ClusterIP".to_string()),
            ports: array_at(value, "/spec/ports")
                .iter()
                .map(|p| PublishedPort {
                    port: scalar_text(p.get("port")).unwrap_or_default(),
                    node_port: int_at(p, "/nodePort"),
                })
                .collect(),
        }),
        "Ingress" => ObjectFacts::Exposure(ExposureFacts::Ingress {
            hosts: array_at(value, "/spec/rules")
                .iter()
                .map(|r| str_at(r, "/host").unwrap_or_else(|| "*".to_string()))
                .collect(),
            tls: !array_at(value, "/spec/tls").is_empty(),
        }),
        _ => ObjectFacts::Unmodeled,
    }
}

fn workload(pod: Option<&Value>, replicas: Option<i64>) -> WorkloadFacts {
    let empty = Value::Null;
    let pod = pod.unwrap_or(&empty);
    let mut containers: Vec<ContainerFacts> = array_at(pod, "/containers")
        .iter()
        .map(|c| container(c, false))
        .collect();
    containers.extend(
        array_at(pod, "/initContainers")
            .iter()
            .map(|c| container(c, true)),
    );
    WorkloadFacts {
        replicas,
        pod: PodFacts {
            host_network: bool_at(pod, "/hostNetwork").unwrap_or(false),
            host_pid: bool_at(pod, "/hostPID").unwrap_or(false),
            host_ipc: bool_at(pod, "/hostIPC").unwrap_or(false),
            run_as_non_root: bool_at(pod, "/securityContext/runAsNonRoot"),
            run_as_user: int_at(pod, "/securityContext/runAsUser"),
            service_account: str_at(pod, "/serviceAccountName"),
            host_paths: array_at(pod, "/volumes")
                .iter()
                .filter_map(|v| str_at(v, "/hostPath/path"))
                .collect(),
        },
        containers,
    }
}

fn container(value: &Value, init: bool) -> ContainerFacts {
    ContainerFacts {
        name: str_at(value, "/name").unwrap_or_default(),
        init,
        image: str_at(value, "/image").map(|raw| ImageRef::parse(&raw)),
        privileged: bool_at(value, "/securityContext/privileged"),
        allow_privilege_escalation: bool_at(value, "/securityContext/allowPrivilegeEscalation"),
        run_as_non_root: bool_at(value, "/securityContext/runAsNonRoot"),
        run_as_user: int_at(value, "/securityContext/runAsUser"),
        read_only_root_filesystem: bool_at(value, "/securityContext/readOnlyRootFilesystem"),
        capabilities_add: strings_at(value, "/securityContext/capabilities/add"),
    }
}

fn rbac_rule(value: &Value) -> RbacRule {
    RbacRule {
        api_groups: strings_at(value, "/apiGroups"),
        resources: strings_at(value, "/resources"),
        verbs: strings_at(value, "/verbs"),
    }
}

fn array_at<'a>(value: &'a Value, pointer: &str) -> &'a [Value] {
    value
        .pointer(pointer)
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

fn str_at(value: &Value, pointer: &str) -> Option<String> {
    value
        .pointer(pointer)
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn bool_at(value: &Value, pointer: &str) -> Option<bool> {
    value.pointer(pointer).and_then(Value::as_bool)
}

fn int_at(value: &Value, pointer: &str) -> Option<i64> {
    value.pointer(pointer).and_then(Value::as_i64)
}

fn strings_at(value: &Value, pointer: &str) -> Vec<String> {
    array_at(value, pointer)
        .iter()
        .filter_map(|v| v.as_str().map(str::to_string))
        .collect()
}

fn scalar_text(value: Option<&Value>) -> Option<String> {
    match value? {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

impl ManifestObject {
    /// The object's facts as `path -> text`, the single projection that deltas
    /// compare, so what a delta can report is exactly what the model holds.
    pub fn fact_lines(&self) -> BTreeMap<String, String> {
        let mut lines = BTreeMap::new();
        lines.insert("apiVersion".to_string(), self.api_version.clone());
        match &self.facts {
            ObjectFacts::Workload(w) => workload_lines(w, &mut lines),
            ObjectFacts::Rbac(r) => rbac_lines(r, &mut lines),
            ObjectFacts::Exposure(e) => exposure_lines(e, &mut lines),
            ObjectFacts::Unmodeled => {}
        }
        lines
    }
}

fn opt<T: ToString>(value: &Option<T>) -> String {
    value
        .as_ref()
        .map_or_else(|| "<unset>".to_string(), ToString::to_string)
}

fn workload_lines(w: &WorkloadFacts, lines: &mut BTreeMap<String, String>) {
    lines.insert("replicas".into(), opt(&w.replicas));
    lines.insert("pod.hostNetwork".into(), w.pod.host_network.to_string());
    lines.insert("pod.hostPID".into(), w.pod.host_pid.to_string());
    lines.insert("pod.hostIPC".into(), w.pod.host_ipc.to_string());
    lines.insert("pod.runAsNonRoot".into(), opt(&w.pod.run_as_non_root));
    lines.insert("pod.runAsUser".into(), opt(&w.pod.run_as_user));
    lines.insert("pod.serviceAccount".into(), opt(&w.pod.service_account));
    lines.insert("pod.hostPaths".into(), w.pod.host_paths.join(","));
    for c in &w.containers {
        let prefix = format!(
            "{}[{}]",
            if c.init {
                "initContainers"
            } else {
                "containers"
            },
            c.name
        );
        lines.insert(
            format!("{prefix}.image"),
            c.image
                .as_ref()
                .map_or_else(|| "<unset>".to_string(), |i| i.raw.clone()),
        );
        lines.insert(format!("{prefix}.privileged"), opt(&c.privileged));
        lines.insert(
            format!("{prefix}.allowPrivilegeEscalation"),
            opt(&c.allow_privilege_escalation),
        );
        lines.insert(format!("{prefix}.runAsNonRoot"), opt(&c.run_as_non_root));
        lines.insert(format!("{prefix}.runAsUser"), opt(&c.run_as_user));
        lines.insert(
            format!("{prefix}.readOnlyRootFilesystem"),
            opt(&c.read_only_root_filesystem),
        );
        lines.insert(
            format!("{prefix}.capabilities.add"),
            c.capabilities_add.join(","),
        );
    }
}

fn rbac_lines(r: &RbacFacts, lines: &mut BTreeMap<String, String>) {
    match r {
        RbacFacts::Role { rules } => {
            for (i, rule) in rules.iter().enumerate() {
                lines.insert(
                    format!("rules[{i}]"),
                    format!(
                        "apiGroups={} resources={} verbs={}",
                        rule.api_groups.join(","),
                        rule.resources.join(","),
                        rule.verbs.join(",")
                    ),
                );
            }
        }
        RbacFacts::Binding {
            role_kind,
            role_name,
            subjects,
        } => {
            lines.insert("roleRef".into(), format!("{role_kind}/{role_name}"));
            for (i, s) in subjects.iter().enumerate() {
                lines.insert(format!("subjects[{i}]"), format!("{}/{}", s.kind, s.name));
            }
        }
    }
}

fn exposure_lines(e: &ExposureFacts, lines: &mut BTreeMap<String, String>) {
    match e {
        ExposureFacts::Service {
            service_type,
            ports,
        } => {
            lines.insert("service.type".into(), service_type.clone());
            for (i, p) in ports.iter().enumerate() {
                lines.insert(
                    format!("service.ports[{i}]"),
                    format!("{} nodePort={}", p.port, opt(&p.node_port)),
                );
            }
        }
        ExposureFacts::Ingress { hosts, tls } => {
            lines.insert("ingress.hosts".into(), hosts.join(","));
            lines.insert("ingress.tls".into(), tls.to_string());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn doc(value: Value) -> RenderedDocument {
        RenderedDocument {
            template: Some("chart/templates/x.yaml".into()),
            index: 0,
            value,
        }
    }

    fn model(value: Value) -> ManifestModel {
        ManifestModel::from_documents(&[doc(value)], "chart", "1.0.0", "sha256:v")
    }

    #[test]
    fn image_references_split_tag_and_digest_without_mistaking_a_registry_port() {
        assert_eq!(ImageRef::parse("nginx").tag, None);
        assert!(ImageRef::parse("nginx").is_mutable());
        assert!(ImageRef::parse("nginx:latest").is_mutable());
        assert!(!ImageRef::parse("nginx:1.25").is_mutable());
        assert_eq!(ImageRef::parse("reg:5000/app").tag, None);
        assert_eq!(ImageRef::parse("reg:5000/app:2").tag.as_deref(), Some("2"));
        assert!(!ImageRef::parse("app:latest@sha256:abc").is_mutable());
    }

    #[test]
    fn a_deployment_yields_workload_facts_with_origin() {
        let m = model(json!({
            "apiVersion": "apps/v1", "kind": "Deployment",
            "metadata": {"name": "web", "namespace": "prod"},
            "spec": {"replicas": 2, "template": {"spec": {
                "hostNetwork": true,
                "containers": [{"name": "app", "image": "nginx:1.25",
                    "securityContext": {"privileged": true, "capabilities": {"add": ["NET_ADMIN"]}}}],
                "initContainers": [{"name": "init", "image": "busybox"}],
                "volumes": [{"name": "h", "hostPath": {"path": "/var/run"}}]
            }}}
        }));
        let object = &m.objects[0];
        assert_eq!(object.id.to_string(), "Deployment/prod/web");
        assert_eq!(
            object.origin.template.as_deref(),
            Some("chart/templates/x.yaml")
        );
        assert_eq!(object.origin.values_digest, "sha256:v");
        let ObjectFacts::Workload(w) = &object.facts else {
            panic!("expected a workload")
        };
        assert_eq!(w.replicas, Some(2));
        assert!(w.pod.host_network);
        assert_eq!(w.pod.host_paths, vec!["/var/run".to_string()]);
        assert_eq!(w.containers.len(), 2);
        assert_eq!(w.containers[0].privileged, Some(true));
        assert_eq!(
            w.containers[0].capabilities_add,
            vec!["NET_ADMIN".to_string()]
        );
        assert!(w.containers[1].init);
    }

    #[test]
    fn cronjob_and_pod_paths_reach_their_pod_spec() {
        let cron = model(json!({"kind": "CronJob", "metadata": {"name": "c"},
            "spec": {"jobTemplate": {"spec": {"template": {"spec": {"containers": [{"name": "a", "image": "x:1"}]}}}}}}));
        let pod = model(json!({"kind": "Pod", "metadata": {"name": "p"},
            "spec": {"containers": [{"name": "a", "image": "x:1"}]}}));
        for m in [cron, pod] {
            let ObjectFacts::Workload(w) = &m.objects[0].facts else {
                panic!("expected a workload")
            };
            assert_eq!(w.containers.len(), 1);
        }
    }

    #[test]
    fn rbac_and_exposure_are_modeled() {
        let role = model(json!({"kind": "ClusterRole", "metadata": {"name": "r"},
            "rules": [{"apiGroups": [""], "resources": ["*"], "verbs": ["get"]}]}));
        let ObjectFacts::Rbac(RbacFacts::Role { rules }) = &role.objects[0].facts else {
            panic!("expected a role")
        };
        assert!(rules[0].has_wildcard());
        let svc = model(json!({"kind": "Service", "metadata": {"name": "s"},
            "spec": {"type": "NodePort", "ports": [{"port": 80, "nodePort": 30080}]}}));
        let ObjectFacts::Exposure(ExposureFacts::Service {
            service_type,
            ports,
        }) = &svc.objects[0].facts
        else {
            panic!("expected a service")
        };
        assert_eq!(service_type, "NodePort");
        assert_eq!(ports[0].node_port, Some(30080));
    }

    #[test]
    fn unmodeled_kinds_and_broken_documents_are_reported_not_hidden() {
        let m = ManifestModel::from_documents(
            &[
                doc(json!({"kind": "ConfigMap", "metadata": {"name": "c"}})),
                RenderedDocument {
                    index: 1,
                    ..doc(json!({"data": 1}))
                },
                RenderedDocument {
                    index: 2,
                    ..doc(Value::Null)
                },
            ],
            "c",
            "1",
            "d",
        );
        assert_eq!(m.unmodeled_kinds(), vec!["ConfigMap".to_string()]);
        assert_eq!(m.skipped.len(), 1);
        assert_eq!(m.skipped[0].document_index, 1);
    }

    #[test]
    fn a_list_kind_is_expanded_into_its_items() {
        let m = model(json!({"kind": "List", "items": [
            {"kind": "Service", "metadata": {"name": "a"}},
            {"kind": "Service", "metadata": {"name": "b"}}]}));
        assert_eq!(m.objects.len(), 2);
    }
}
