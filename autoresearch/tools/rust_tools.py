"""Rust CLI tool wrappers for the self-evolving rule system.

All communication with the Rust codebase goes through subprocess calls:
- cargo check → verify compilation
- cargo test → run test suite
- git → commit/discard changes
- sandbox-orchestrator → evaluate rules on corpus
"""

import subprocess
import shlex
from pathlib import Path
from typing import Tuple, Optional
import logging

logger = logging.getLogger(__name__)

REPO_ROOT = Path(__file__).parent.parent.parent
CATALOG_PATH = REPO_ROOT / "crates" / "cognicode-axiom" / "src" / "rules" / "catalog.rs"


class CargoTool:
    """Wrapper for cargo CLI operations."""
    
    def check(self, package: str = "cognicode-axiom", timeout: int = 120) -> Tuple[bool, str]:
        """Run cargo check. Returns (success, stderr)."""
        cmd = ["cargo", "check", "-p", package]
        logger.info(f"Running: {' '.join(cmd)}")
        
        result = subprocess.run(
            cmd, capture_output=True, text=True, timeout=timeout, cwd=str(REPO_ROOT)
        )
        return result.returncode == 0, result.stderr
    
    def test(self, workspace: bool = True, timeout: int = 600) -> Tuple[bool, str]:
        """Run cargo test. Returns (all_passed, combined_output)."""
        args = ["cargo", "test"]
        if workspace:
            args.append("--workspace")
        
        logger.info(f"Running: {' '.join(args)}")
        result = subprocess.run(
            args, capture_output=True, text=True, timeout=timeout, cwd=str(REPO_ROOT)
        )
        return result.returncode == 0, result.stdout + "\n" + result.stderr
    
    def build_release(self, timeout: int = 300) -> Tuple[bool, str]:
        """Build release binaries."""
        cmd = ["cargo", "build", "--release"]
        logger.info(f"Running: {' '.join(cmd)}")
        result = subprocess.run(
            cmd, capture_output=True, text=True, timeout=timeout, cwd=str(REPO_ROOT)
        )
        return result.returncode == 0, result.stderr


class GitTool:
    """Wrapper for git operations."""
    
    def diff(self, file_path: Optional[str] = None) -> str:
        """Show pending changes."""
        args = ["git", "diff"]
        if file_path:
            args.extend(["--", file_path])
        result = subprocess.run(
            args, capture_output=True, text=True, cwd=str(REPO_ROOT)
        )
        return result.stdout
    
    def checkout(self, file_path: str):
        """Revert changes to a file."""
        logger.info(f"Reverting: {file_path}")
        subprocess.run(
            ["git", "checkout", "--", file_path],
            check=True, cwd=str(REPO_ROOT)
        )
    
    def commit(self, message: str) -> bool:
        """Stage all changes and commit. Returns success."""
        try:
            subprocess.run(
                ["git", "add", "-f", str(CATALOG_PATH)],
                check=True, cwd=str(REPO_ROOT)
            )
            subprocess.run(
                ["git", "commit", "-m", message],
                check=True, cwd=str(REPO_ROOT)
            )
            logger.info(f"Committed: {message}")
            return True
        except subprocess.CalledProcessError as e:
            logger.error(f"Commit failed: {e}")
            return False
    
    def current_commit(self) -> str:
        """Get current HEAD commit hash (short)."""
        result = subprocess.run(
            ["git", "rev-parse", "--short", "HEAD"],
            capture_output=True, text=True, cwd=str(REPO_ROOT)
        )
        return result.stdout.strip()
    
    def status(self) -> str:
        """Get git status."""
        result = subprocess.run(
            ["git", "status", "--short"],
            capture_output=True, text=True, cwd=str(REPO_ROOT)
        )
        return result.stdout


class SandboxTool:
    """Wrapper for sandbox-orchestrator CLI."""
    
    def __init__(self, binary_path: str = "./target/release/sandbox-orchestrator"):
        self.binary = REPO_ROOT / binary_path
    
    def eval_rule(self, rule_id: str, corpus: str = "all", 
                  timeout: int = 300) -> Tuple[bool, str, str]:
        """Run sandbox evaluation for a specific rule.
        
        Returns: (success, stdout, stderr)
        """
        manifest = REPO_ROOT / "sandbox" / "manifests" / f"rust_fixture.yaml"
        
        args = [
            str(self.binary), "run",
            str(manifest),
            "--results-dir", str(REPO_ROOT / "autoresearch" / "results"),
            "--jsonl",
        ]
        
        if rule_id:
            args.extend(["--filter-rule", rule_id])
        
        logger.info(f"Running: {' '.join(args)}")
        result = subprocess.run(
            args, capture_output=True, text=True, timeout=timeout, cwd=str(REPO_ROOT)
        )
        return result.returncode == 0, result.stdout, result.stderr
    
    def run_full_corpus(self, timeout: int = 600) -> Tuple[bool, str, str]:
        """Run sandbox on full corpus (all rules, all languages)."""
        return self.eval_rule(rule_id="", corpus="all", timeout=timeout)
    
    def ensure_binary(self):
        """Ensure sandbox-orchestrator binary exists (build if needed)."""
        if not self.binary.exists():
            logger.info("Building sandbox-orchestrator...")
            cargo = CargoTool()
            ok, err = cargo.build_release()
            if not ok:
                raise RuntimeError(f"Failed to build sandbox-orchestrator: {err}")


class ExternalToolRunner:
    """Run external static analysis tools and parse their output."""
    
    def run_eslint(self, file_path: str, timeout: int = 60) -> dict:
        """Run ESLint on a file, return parsed findings."""
        args = ["npx", "eslint", "--format=json", file_path]
        result = subprocess.run(
            args, capture_output=True, text=True, timeout=timeout
        )
        if result.returncode not in (0, 1):  # 1 = lint errors found (expected)
            return {"error": result.stderr, "findings": []}
        
        import json
        try:
            return {"findings": json.loads(result.stdout) if result.stdout.strip() else []}
        except json.JSONDecodeError:
            return {"error": "Failed to parse ESLint output", "findings": []}
    
    def run_ruff(self, file_path: str, timeout: int = 60) -> dict:
        """Run Ruff on a file, return parsed findings."""
        args = ["ruff", "check", "--output-format=json", file_path]
        result = subprocess.run(
            args, capture_output=True, text=True, timeout=timeout
        )
        import json
        try:
            return {"findings": json.loads(result.stdout) if result.stdout.strip() else []}
        except json.JSONDecodeError:
            return {"error": "Failed to parse Ruff output", "findings": []}
    
    def run_clippy(self, file_path: str, timeout: int = 120) -> dict:
        """Run Clippy on a file, return parsed findings."""
        args = ["cargo", "clippy", "--message-format=json"]
        result = subprocess.run(
            args, capture_output=True, text=True, timeout=timeout, cwd=str(REPO_ROOT)
        )
        import json
        findings = []
        for line in result.stdout.strip().split("\n"):
            if not line:
                continue
            try:
                msg = json.loads(line)
                if msg.get("reason") == "compiler-message":
                    for span in msg.get("message", {}).get("spans", []):
                        if span.get("file_name", "").endswith(file_path):
                            findings.append({
                                "file": span["file_name"],
                                "line": span["line_start"],
                                "rule": msg["message"].get("code", {}).get("code", "unknown"),
                                "message": msg["message"]["message"],
                                "severity": msg["message"]["level"]
                            })
            except json.JSONDecodeError:
                continue
        
        return {"findings": findings}
