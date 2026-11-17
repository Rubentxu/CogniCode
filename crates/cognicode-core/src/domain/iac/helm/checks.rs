//! Security checks over concrete manifest objects.
//!
//! Every finding is a fact about an object that a real render produced with
//! fixed values; it carries the object's origin. A check never looks at
//! template text. Whether "no finding" may be read as a clean result is decided
//! by the analysis, which knows the render's coverage; this module only reports
//! what the objects show.

use serde::{Deserialize, Serialize};

use super::manifest::{
    ContainerFacts, ExposureFacts, ManifestModel, ManifestObject, ObjectFacts, ObjectId,
    ObjectOrigin, RbacFacts, WorkloadFacts,
};
use crate::domain::findings::FindingSeverity;

/// A check of the Helm manifest pack.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HelmCheck {
    /// A privileged container.
    PrivilegedContainer,
    /// Privilege escalation is allowed or not disabled.
    PrivilegeEscalation,
    /// A container that can run as root.
    RunsAsRoot,
    /// Host namespaces or host paths.
    HostAccess,
    /// An image that can change under the same reference.
    MutableImage,
    /// A wildcard RBAC rule or a cluster-admin binding.
    BroadRbac,
    /// A workload published outside the cluster.
    ExternalExposure,
}

impl HelmCheck {
    /// Every check, in a stable order.
    pub const ALL: [Self; 7] = [
        Self::PrivilegedContainer,
        Self::PrivilegeEscalation,
        Self::RunsAsRoot,
        Self::HostAccess,
        Self::MutableImage,
        Self::BroadRbac,
        Self::ExternalExposure,
    ];

    /// Stable identifier for reports.
    pub fn id(self) -> &'static str {
        match self {
            Self::PrivilegedContainer => "helm.container.privileged",
            Self::PrivilegeEscalation => "helm.container.privilege_escalation",
            Self::RunsAsRoot => "helm.container.runs_as_root",
            Self::HostAccess => "helm.pod.host_access",
            Self::MutableImage => "helm.image.mutable",
            Self::BroadRbac => "helm.rbac.broad_permissions",
            Self::ExternalExposure => "helm.service.external_exposure",
        }
    }
}

/// A fact that violates a check, on one object.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HelmFinding {
    /// The check.
    pub check: HelmCheck,
    /// Severity of this finding.
    pub severity: FindingSeverity,
    /// The object.
    pub object: ObjectId,
    /// Field path inside the object (`containers[app].securityContext`).
    pub location: String,
    /// What the fact is.
    pub detail: String,
    /// Where the object came from, with the values that fixed it.
    pub origin: ObjectOrigin,
}

impl HelmFinding {
    /// Identity of a finding across two renders: same check, same object, same
    /// place. Detail and origin may legitimately differ.
    pub fn key(&self) -> (HelmCheck, ObjectId, String) {
        (self.check, self.object.clone(), self.location.clone())
    }
}

/// Run every check over every object.
pub fn check_manifest(manifest: &ManifestModel) -> Vec<HelmFinding> {
    let mut findings: Vec<HelmFinding> = manifest.objects.iter().flat_map(check_object).collect();
    findings.sort_by_key(HelmFinding::key);
    findings
}

fn check_object(object: &ManifestObject) -> Vec<HelmFinding> {
    let report = Reporter { object };
    match &object.facts {
        ObjectFacts::Workload(workload) => check_workload(&report, workload),
        ObjectFacts::Rbac(rbac) => check_rbac(&report, rbac),
        ObjectFacts::Exposure(exposure) => check_exposure(&report, exposure),
        ObjectFacts::Unmodeled => Vec::new(),
    }
}

struct Reporter<'a> {
    object: &'a ManifestObject,
}

impl Reporter<'_> {
    fn finding(
        &self,
        check: HelmCheck,
        severity: FindingSeverity,
        location: impl Into<String>,
        detail: impl Into<String>,
    ) -> HelmFinding {
        HelmFinding {
            check,
            severity,
            object: self.object.id.clone(),
            location: location.into(),
            detail: detail.into(),
            origin: self.object.origin.clone(),
        }
    }
}

fn check_workload(report: &Reporter<'_>, workload: &WorkloadFacts) -> Vec<HelmFinding> {
    let mut findings = check_pod(report, workload);
    for container in &workload.containers {
        findings.extend(check_container(report, workload, container));
    }
    findings
}

fn check_pod(report: &Reporter<'_>, workload: &WorkloadFacts) -> Vec<HelmFinding> {
    let pod = &workload.pod;
    let mut findings = Vec::new();
    for (enabled, field) in [
        (pod.host_network, "hostNetwork"),
        (pod.host_pid, "hostPID"),
        (pod.host_ipc, "hostIPC"),
    ] {
        if enabled {
            findings.push(report.finding(
                HelmCheck::HostAccess,
                FindingSeverity::Critical,
                format!("pod.{field}"),
                format!("pod shares the host namespace through `{field}: true`"),
            ));
        }
    }
    for path in &pod.host_paths {
        findings.push(report.finding(
            HelmCheck::HostAccess,
            FindingSeverity::Warning,
            format!("pod.volumes.hostPath[{path}]"),
            format!("pod mounts the host path `{path}`"),
        ));
    }
    findings
}

fn check_container(
    report: &Reporter<'_>,
    workload: &WorkloadFacts,
    container: &ContainerFacts,
) -> Vec<HelmFinding> {
    let at = |field: &str| {
        format!(
            "{}[{}].{field}",
            if container.init {
                "initContainers"
            } else {
                "containers"
            },
            container.name
        )
    };
    let mut findings = Vec::new();
    if container.privileged == Some(true) {
        findings.push(report.finding(
            HelmCheck::PrivilegedContainer,
            FindingSeverity::Critical,
            at("securityContext.privileged"),
            "container runs privileged",
        ));
    }
    match container.allow_privilege_escalation {
        Some(false) => {}
        Some(true) => findings.push(report.finding(
            HelmCheck::PrivilegeEscalation,
            FindingSeverity::Warning,
            at("securityContext.allowPrivilegeEscalation"),
            "privilege escalation is explicitly allowed",
        )),
        None => findings.push(report.finding(
            HelmCheck::PrivilegeEscalation,
            FindingSeverity::Warning,
            at("securityContext.allowPrivilegeEscalation"),
            "privilege escalation is not disabled (the cluster default allows it)",
        )),
    }
    if let Some(detail) = runs_as_root(workload, container) {
        findings.push(report.finding(
            HelmCheck::RunsAsRoot,
            FindingSeverity::Warning,
            at("securityContext.runAsUser"),
            detail,
        ));
    }
    if let Some(image) = container.image.as_ref().filter(|i| i.is_mutable()) {
        findings.push(report.finding(
            HelmCheck::MutableImage,
            FindingSeverity::Warning,
            at("image"),
            format!("image `{}` has no digest and no pinned tag", image.raw),
        ));
    }
    findings
}

fn runs_as_root(workload: &WorkloadFacts, container: &ContainerFacts) -> Option<String> {
    let user = container.run_as_user.or(workload.pod.run_as_user);
    let non_root = container.run_as_non_root.or(workload.pod.run_as_non_root);
    match (user, non_root) {
        (Some(0), _) => Some("container runs as uid 0".to_string()),
        (Some(_), _) => None,
        (None, Some(true)) => None,
        (None, _) => Some(
            "neither `runAsNonRoot: true` nor a non-zero `runAsUser` is set, so the image user applies"
                .to_string(),
        ),
    }
}

fn check_rbac(report: &Reporter<'_>, rbac: &RbacFacts) -> Vec<HelmFinding> {
    match rbac {
        RbacFacts::Role { rules } => rules
            .iter()
            .enumerate()
            .filter(|(_, rule)| rule.has_wildcard())
            .map(|(i, rule)| {
                let everything =
                    rule.verbs.iter().any(|v| v == "*") && rule.resources.iter().any(|r| r == "*");
                report.finding(
                    HelmCheck::BroadRbac,
                    if everything {
                        FindingSeverity::Critical
                    } else {
                        FindingSeverity::Warning
                    },
                    format!("rules[{i}]"),
                    format!(
                        "rule grants verbs [{}] on resources [{}] in groups [{}]",
                        rule.verbs.join(","),
                        rule.resources.join(","),
                        rule.api_groups.join(",")
                    ),
                )
            })
            .collect(),
        RbacFacts::Binding {
            role_kind,
            role_name,
            subjects,
        } if role_kind == "ClusterRole" && role_name == "cluster-admin" => vec![report.finding(
            HelmCheck::BroadRbac,
            FindingSeverity::Critical,
            "roleRef",
            format!(
                "binds cluster-admin to {}",
                subjects
                    .iter()
                    .map(|s| format!("{}/{}", s.kind, s.name))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        )],
        RbacFacts::Binding { .. } => Vec::new(),
    }
}

fn check_exposure(report: &Reporter<'_>, exposure: &ExposureFacts) -> Vec<HelmFinding> {
    match exposure {
        ExposureFacts::Service {
            service_type,
            ports,
        } if service_type == "LoadBalancer" || service_type == "NodePort" => {
            let ports: Vec<&str> = ports.iter().map(|p| p.port.as_str()).collect();
            vec![report.finding(
                HelmCheck::ExternalExposure,
                FindingSeverity::Warning,
                "service.type",
                format!(
                    "service of type {service_type} publishes ports [{}] outside the cluster",
                    ports.join(",")
                ),
            )]
        }
        ExposureFacts::Service { .. } => Vec::new(),
        ExposureFacts::Ingress { hosts, tls } => vec![report.finding(
            HelmCheck::ExternalExposure,
            if *tls {
                FindingSeverity::Info
            } else {
                FindingSeverity::Warning
            },
            "ingress.hosts",
            format!(
                "ingress publishes hosts [{}]{}",
                hosts.join(","),
                if *tls { "" } else { " without TLS" }
            ),
        )],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::iac::helm::manifest::RenderedDocument;
    use serde_json::{Value, json};

    fn findings(value: Value) -> Vec<HelmFinding> {
        let doc = RenderedDocument {
            template: Some("c/templates/t.yaml".into()),
            index: 0,
            value,
        };
        check_manifest(&ManifestModel::from_documents(&[doc], "c", "1", "sha256:v"))
    }

    fn deployment(container: Value, pod_extra: Value) -> Value {
        let mut pod = json!({"containers": [container]});
        pod.as_object_mut()
            .unwrap()
            .extend(pod_extra.as_object().unwrap().clone());
        json!({"kind": "Deployment", "metadata": {"name": "web"},
            "spec": {"template": {"spec": pod}}})
    }

    fn checks(findings: &[HelmFinding]) -> Vec<HelmCheck> {
        findings.iter().map(|f| f.check).collect()
    }

    #[test]
    fn a_hardened_container_with_a_pinned_image_has_no_findings() {
        let result = findings(deployment(
            json!({"name": "a", "image": "app:1.2.3", "securityContext": {
                "allowPrivilegeEscalation": false, "runAsNonRoot": true}}),
            json!({}),
        ));
        assert!(result.is_empty(), "{result:?}");
    }

    #[test]
    fn privileged_root_mutable_and_escalation_are_each_reported_with_origin() {
        let result = findings(deployment(
            json!({"name": "a", "image": "app:latest",
                "securityContext": {"privileged": true, "runAsUser": 0}}),
            json!({"hostNetwork": true}),
        ));
        let found = checks(&result);
        for expected in [
            HelmCheck::PrivilegedContainer,
            HelmCheck::PrivilegeEscalation,
            HelmCheck::RunsAsRoot,
            HelmCheck::MutableImage,
            HelmCheck::HostAccess,
        ] {
            assert!(found.contains(&expected), "{expected:?} in {found:?}");
        }
        assert!(result.iter().all(|f| f.origin.values_digest == "sha256:v"));
        assert!(
            result
                .iter()
                .all(|f| f.origin.template.as_deref() == Some("c/templates/t.yaml"))
        );
    }

    #[test]
    fn a_pod_level_non_root_setting_covers_the_containers() {
        let result = findings(deployment(
            json!({"name": "a", "image": "app:1", "securityContext": {"allowPrivilegeEscalation": false}}),
            json!({"securityContext": {"runAsNonRoot": true}}),
        ));
        assert!(!checks(&result).contains(&HelmCheck::RunsAsRoot));
    }

    #[test]
    fn rbac_wildcards_and_cluster_admin_are_reported() {
        let role = findings(json!({"kind": "ClusterRole", "metadata": {"name": "r"},
            "rules": [{"apiGroups": ["*"], "resources": ["*"], "verbs": ["*"]},
                      {"apiGroups": [""], "resources": ["pods"], "verbs": ["get"]}]}));
        assert_eq!(role.len(), 1);
        assert_eq!(role[0].severity, FindingSeverity::Critical);
        assert_eq!(role[0].location, "rules[0]");
        let binding = findings(
            json!({"kind": "ClusterRoleBinding", "metadata": {"name": "b"},
            "roleRef": {"kind": "ClusterRole", "name": "cluster-admin"},
            "subjects": [{"kind": "ServiceAccount", "name": "app"}]}),
        );
        assert_eq!(checks(&binding), vec![HelmCheck::BroadRbac]);
    }

    #[test]
    fn exposure_is_reported_for_node_ports_load_balancers_and_ingress_only() {
        let lb = findings(json!({"kind": "Service", "metadata": {"name": "s"},
            "spec": {"type": "LoadBalancer", "ports": [{"port": 443}]}}));
        assert_eq!(checks(&lb), vec![HelmCheck::ExternalExposure]);
        let internal = findings(json!({"kind": "Service", "metadata": {"name": "s"},
            "spec": {"ports": [{"port": 80}]}}));
        assert!(internal.is_empty());
        let ingress = findings(json!({"kind": "Ingress", "metadata": {"name": "i"},
            "spec": {"rules": [{"host": "a.example.com"}]}}));
        assert_eq!(ingress[0].severity, FindingSeverity::Warning);
    }
}
