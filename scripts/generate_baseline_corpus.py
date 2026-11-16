#!/usr/bin/env python3
"""Generate the reproducible CAA-01 baseline corpus in a directory.

The corpus is one small project that holds every change class the baseline
has to measure: a function body, a public signature, a manifest that governs
reuse, a Terraform file and a Helm chart. Generation is deterministic — no
timestamps, no randomness — so two runs produce byte-identical files and a
measurement can be attributed to the exact tree that produced it.

This is an explicit measurement instrument, not a suite test.
"""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path

LIB_RS_TEMPLATE = '''//! The corpus root: one public signature, one body, one helper each.

pub fn compute_total(values: &[u64]) -> u64 {{
    let mut acc: u64 = 0;
    for item in values {{
        acc += *item;
    }}
    acc + scale_{marker}()
}}

fn scale_{marker}() -> u64 {{
    {marker}
}}

pub fn render_total(values: &[u64]) -> String {{
    format!("total={{}}", compute_total(values))
}}
'''


def corpus_files(modules: int) -> dict[str, str]:
    """Every file of the corpus, keyed by workspace-relative path."""
    files: dict[str, str] = {}
    files["Cargo.toml"] = (
        '[package]\nname = "caa01_corpus"\nversion = "0.1.0"\nedition = "2021"\n'
    )
    files["src/lib.rs"] = LIB_RS_TEMPLATE.format(marker="0")
    for module in range(1, modules + 1):
        files[f"src/module_{module}.rs"] = LIB_RS_TEMPLATE.format(marker=str(module))
    files["infra/main.tf"] = (
        'variable "instance_count" {\n  type    = number\n  default = 2\n}\n\n'
        'resource "aws_instance" "baseline" {\n'
        '  ami           = "ami-0baseline"\n'
        '  instance_type = "t3.micro"\n'
        '  count         = var.instance_count\n}\n'
    )
    files["chart/Chart.yaml"] = (
        'apiVersion: v2\nname: caa01\nversion: 0.1.0\ndescription: baseline chart\n'
    )
    files["chart/values.yaml"] = "replicaCount: 2\nimage:\n  repository: caa01/app\n  tag: v1\n"
    # Placeholders are quoted so the template still parses as YAML: a baseline
    # corpus must keep the graph complete, because a partial graph never writes
    # a durable snapshot and the warm and incremental scenarios would silently
    # measure rebuilds.
    files["chart/templates/deployment.yaml"] = (
        "apiVersion: apps/v1\nkind: Deployment\n"
        "metadata:\n  name: caa01\nspec:\n"
        '  replicas: "{{ .Values.replicaCount }}"\n'
        "  template:\n    spec:\n      containers:\n"
        "        - name: app\n"
        '          image: "{{ .Values.image.repository }}:{{ .Values.image.tag }}"\n'
    )
    return files


# One mutation per change class, each a (path, exact old text, new text) that
# keeps the tree valid: the baseline measures the cost of the change, and an
# invalid change measures a failure instead.
CHANGE_CLASSES: dict[str, tuple[str, str, str]] = {
    "function_body": (
        "src/module_2.rs",
        "    let mut acc: u64 = 0;",
        "    let mut acc: u64 = 1;",
    ),
    "public_signature": (
        "src/lib.rs",
        "pub fn render_total(values: &[u64]) -> String {\n    format!(\"total={}\", compute_total(values))\n}",
        "pub fn render_total(values: &[u64], prefix: &str) -> String {\n"
        "    format!(\"{}total={}\", prefix, compute_total(values))\n}",
    ),
    "manifest": ("Cargo.toml", 'version = "0.1.0"', 'version = "0.1.1"'),
    "terraform": ("infra/main.tf", 'instance_type = "t3.micro"', 'instance_type = "t3.small"'),
    "helm": ("chart/values.yaml", "replicaCount: 2", "replicaCount: 3"),
}


def write_corpus(workspace: Path, modules: int) -> None:
    """Write the generated tree, unchanged: the base every sample starts from."""
    for relative, content in corpus_files(modules).items():
        target = workspace / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(content, encoding="utf-8")


def apply_change(workspace: Path, change_class: str, modules: int = 12) -> None:
    """Mutate the workspace for one change class, restoring it first.

    The restored base is the generated corpus: a sample that mutated twice
    would measure a tree nobody generated.
    """
    path, old, new = CHANGE_CLASSES[change_class]
    write_corpus(workspace, modules)
    target = workspace / path
    text = target.read_text(encoding="utf-8")
    assert old in text, f"the change class {change_class} lost its anchor: {path}"
    target.write_text(text.replace(old, new, 1), encoding="utf-8")


def corpus_digest(files: dict[str, str]) -> str:
    """The identity of the generated tree, independent of file order."""
    stream = hashlib.sha256()
    for relative in sorted(files):
        stream.update(relative.encode("utf-8"))
        stream.update(b"\0")
        stream.update(files[relative].encode("utf-8"))
        stream.update(b"\0")
    return stream.hexdigest()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", type=Path, required=True, help="workspace directory to write")
    parser.add_argument("--modules", type=int, default=12, help="Rust modules beside lib.rs")
    parser.add_argument("--json", action="store_true", help="print the manifest as JSON")
    args = parser.parse_args()
    if args.modules < 1:
        parser.error("--modules must be positive")

    files = corpus_files(args.modules)
    for relative, content in files.items():
        target = args.out / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(content, encoding="utf-8")

    if args.json:
        manifest = {
            "tool": "generate_baseline_corpus",
            "version": 1,
            "modules": args.modules,
            "digest": corpus_digest(files),
            "files": {
                relative: hashlib.sha256(content.encode("utf-8")).hexdigest()
                for relative, content in sorted(files.items())
            },
            "change_classes": sorted(CHANGE_CLASSES),
        }
        print(json.dumps(manifest, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
