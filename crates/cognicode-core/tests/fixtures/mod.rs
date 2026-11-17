//! The repository's real fixtures, in a scratch directory.
//!
//! A contract that answers a question about a project should ask it about
//! the project the repository really holds, not about a toy written inside
//! the test. The versioned fixtures are that project, and copying one into
//! a temporary directory is how a test gets to read it without writing
//! into the tree.
//!
//! Doing it here once keeps four contracts from growing their own copy of
//! the same twenty lines — and keeps them from drifting apart, which is
//! how two of them end up testing different fixtures under the same name.
//!
//! Not every contract needs every helper here, so the unused ones are not
//! a finding.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

use cognicode_core::interface::mcp::handlers::{
    BuildGraphInput, HandlerContext, handle_build_graph,
};

/// The directory the versioned fixtures live in.
pub fn fixtures_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../sandbox/fixtures")
        .canonicalize()
        .expect("the versioned fixtures are where the repository says they are")
}

/// The fixture named `name`, copied into `dir`.
///
/// The copy holds the versioned content: a contract that runs against it
/// runs against what the repository really holds, and the repository is
/// left as it was.
pub fn copy_fixture(dir: &Path, name: &str) -> PathBuf {
    let target = dir.join(name);
    copy_tree(&fixtures_root().join(name), &target);
    target
}

/// Copies a directory tree, creating the directories it needs.
pub fn copy_tree(source: &Path, target: &Path) {
    std::fs::create_dir_all(target).expect("mkdir");
    for entry in std::fs::read_dir(source).expect("read fixture") {
        let entry = entry.expect("entry");
        let path = entry.path();
        let destination = target.join(entry.file_name());
        if path.is_dir() {
            copy_tree(&path, &destination);
        } else {
            std::fs::copy(&path, &destination).expect("copy fixture");
        }
    }
}

/// The graph of a scratch directory that holds the given fixture.
///
/// The graph is built the way the product builds it, over the fixture the
/// repository holds, so a contract that reads this graph reads what a
/// consumer would read.
pub async fn graph_of(dir: &Path, fixture: &str) -> HandlerContext {
    copy_fixture(dir, fixture);
    graph_of_dir(dir).await
}

/// The graph of a scratch directory whose files are already in it.
pub async fn graph_of_dir(dir: &Path) -> HandlerContext {
    let ctx = HandlerContext::builder().with_working_dir(dir).build();
    handle_build_graph(&ctx, BuildGraphInput { directory: None })
        .await
        .expect("the graph builds");
    ctx
}

/// A workspace with one Terraform root and one Helm chart.
///
/// The Terraform file leaves `acl` to a variable nothing binds, so one of its
/// verdicts is `unknown` by construction; the chart is a plain deployment whose
/// template needs the engine to be read at all.
pub fn security_pack_workspace(dir: &Path) -> PathBuf {
    let root = dir.join("workspace");
    std::fs::create_dir_all(root.join("chart/templates")).expect("chart directory");
    std::fs::write(
        root.join("main.tf"),
        "variable \"acl\" {}\nresource \"aws_s3_bucket\" \"b\" {\n  acl = var.acl\n}\n",
    )
    .expect("terraform file");
    std::fs::write(
        root.join("chart/Chart.yaml"),
        "apiVersion: v2\nname: demo\nversion: 0.3.0\n",
    )
    .expect("chart file");
    std::fs::write(root.join("chart/values.yaml"), "image: demo:latest\n").expect("values");
    std::fs::write(
        root.join("chart/templates/deployment.yaml"),
        "apiVersion: apps/v1\nkind: Deployment\nmetadata:\n  name: {{ .Release.Name }}\n",
    )
    .expect("template");
    root
}

/// A directory holding a `helm` that prints a fixed `helm template` stream.
///
/// This is a labelled test double for the engine, not the engine: it lets a
/// subprocess test exercise the rendered path without the real binary. The
/// real-engine lane is separate (`COGNICODE_REQUIRE_HELM`). The script uses
/// only shell builtins because the product runs `helm` with a cleared
/// environment, so there is no `PATH` for it to find anything else on.
#[cfg(unix)]
pub fn fake_helm_directory(dir: &Path) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let bin = dir.join("fake-helm-bin");
    std::fs::create_dir_all(&bin).expect("fake helm directory");
    let script = bin.join("helm");
    std::fs::write(
        &script,
        "#!/bin/sh\nprintf '%s\\n' '---' '# Source: demo/templates/deployment.yaml' 'apiVersion: apps/v1' 'kind: Deployment' 'metadata:' '  name: release' 'spec:' '  template:' '    spec:' '      containers:' '        - name: app' '          image: demo:latest' '          securityContext:' '            privileged: true'\n",
    )
    .expect("fake helm script");
    let mut permissions = std::fs::metadata(&script).expect("metadata").permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&script, permissions).expect("executable");
    bin
}
