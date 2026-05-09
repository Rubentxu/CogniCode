"""Sandbox Manager — runs isolated rule experiments in containers.

Supports Docker and Podman (auto-detected).

Usage:
    manager = SandboxManager()
    result = manager.run_experiment(rule_id="S134", git_ref="main")
    # result = {"status": "success", "rule_id": "S134", "tests_passed": 283, ...}

Features:
    - Ephemeral containers (destroyed on exit)
    - Read-only host repo mount (no contamination)
    - CPU/RAM/network limits
    - Parallel multi-agent support
    - Timeout handling + forced cleanup
    - Auto-detects Docker or Podman
"""

import subprocess
import json
import hashlib
import time
import shutil
from pathlib import Path
from typing import Dict, Optional, List
from concurrent.futures import ThreadPoolExecutor, as_completed
import logging

logger = logging.getLogger(__name__)

REPO_ROOT = Path(__file__).parent.parent.parent
SANDBOX_DIR = Path(__file__).parent
IMAGE_NAME = "cognicode-sandbox:latest"

# ═══════════════════════════════════════════════════════════════════
# Container runtime detection
# ═══════════════════════════════════════════════════════════════════

def _detect_runtime() -> str:
    """Auto-detect available container runtime: docker > podman > none."""
    if shutil.which("docker") and _check_runtime("docker"):
        return "docker"
    if shutil.which("podman") and _check_runtime("podman"):
        return "podman"
    raise RuntimeError("No container runtime found. Install Docker or Podman.")

def _check_runtime(cmd: str) -> bool:
    """Verify a container runtime is functional."""
    try:
        subprocess.run([cmd, "version"], capture_output=True, timeout=5, check=False)
        return True
    except:
        return False

CONTAINER_CMD = _detect_runtime()
IS_PODMAN = CONTAINER_CMD == "podman"

logger.info(f"Container runtime: {CONTAINER_CMD}" + (" (rootless)" if IS_PODMAN else ""))


class SandboxManager:
    """Creates and manages ephemeral Docker containers for rule experiments.
    
    Each container:
    - Clones a fresh copy of the repo (from mounted host-repo)
    - Optionally applies a change script
    - Runs: cargo check → cargo test → sandbox evaluation
    - Outputs JSON result to stdout
    - Is destroyed on exit (--rm flag)
    """
    
    def __init__(self, image: str = IMAGE_NAME):
        self.image = image
        self._ensure_image()
    
    def _ensure_image(self):
        """Build sandbox image if not exists."""
        result = subprocess.run(
            [CONTAINER_CMD, "images", "-q", self.image],
            capture_output=True, text=True
        )
        if not result.stdout.strip():
            logger.info(f"Building {CONTAINER_CMD} image: {self.image}...")
            subprocess.run(
                [CONTAINER_CMD, "build", "-t", self.image, str(SANDBOX_DIR)],
                check=True, cwd=str(SANDBOX_DIR)
            )
            logger.info(f"Image built: {self.image}")
    
    def run_experiment(
        self,
        rule_id: str,
        git_ref: str = "main",
        change_script: Optional[str] = None,
        timeout: int = 600,
        cpus: int = 4,
        memory: str = "4g",
    ) -> Dict:
        """Run a single rule experiment in an isolated container.
        
        Args:
            rule_id: Rule to evaluate (e.g., "S134", "S2068")
            git_ref: Git branch/commit to clone
            change_script: Optional path to Python script that edits code
            timeout: Max seconds before killing container
            cpus: CPU limit
            memory: RAM limit
            
        Returns:
            Dict with status, metrics, and any errors
        """
        container_name = f"cognicode-eval-{rule_id}-{_short_hash()}"
        
        logger.info(f"Starting sandbox: {container_name} (rule={rule_id}, ref={git_ref})")
        
        # Build container run command
        cmd = [
            CONTAINER_CMD, "run",
            "--rm",                              # Auto-remove on exit
            "--name", container_name,
            "--cpus", str(cpus),                  # CPU limit
            "--memory", memory,                   # RAM limit
            "-v", f"{REPO_ROOT}:/host-repo:ro",   # Read-only host repo
            "-v", f"{REPO_ROOT}/autoresearch/results:/results",  # Write results
            "-v", "cognicode-cargo-cache:/usr/local/cargo/registry",  # Persistent cargo cache
            "-v", "cognicode-target-cache:/workspace/CogniCode/target",  # Persistent target dir
        ]
        
        # Podman rootless: skip --network=none (needs root)
        if not IS_PODMAN:
            cmd.extend(["--network", "none"])      # No network (Docker only)
        
        cmd.append(self.image)
        cmd.extend([rule_id, git_ref])
        
        # Mount change script if provided
        if change_script and Path(change_script).exists():
            cmd.insert(-4, "-v")
            cmd.insert(-4, f"{change_script}:/change.py:ro")
            cmd.append("/change.py")
        else:
            cmd.append("")  # Empty change script
        
        try:
            result = subprocess.run(
                cmd,
                capture_output=True,
                text=True,
                timeout=timeout,
                cwd=str(REPO_ROOT),
            )
            
            output = result.stdout
            
            # Extract JSON from output (find the last JSON line)
            json_result = None
            for line in reversed(output.strip().split("\n")):
                line = line.strip()
                if line.startswith("{") and "status" in line:
                    try:
                        json_result = json.loads(line)
                        break
                    except json.JSONDecodeError:
                        continue
            
            if json_result:
                logger.info(f"  {rule_id}: {json_result['status']} " +
                           f"(tests: {json_result.get('tests_passed', '?')})")
                return json_result
            
            # No JSON found — return error with output
            return {
                "status": "failed",
                "reason": "no_json_output",
                "stdout": output[-1000:] if output else "",
                "stderr": result.stderr[-500:] if result.stderr else "",
            }
            
        except subprocess.TimeoutExpired:
            logger.warning(f"  {rule_id}: TIMEOUT ({timeout}s)")
            self._force_kill(container_name)
            return {"status": "timeout", "reason": f"exceeded_{timeout}s"}
            
        except Exception as e:
            logger.error(f"  {rule_id}: ERROR — {e}")
            self._force_kill(container_name)
            return {"status": "failed", "reason": str(e)}
    
    def run_parallel(
        self,
        experiments: List[Dict],
        max_workers: int = 3,
    ) -> Dict[str, Dict]:
        """Run multiple experiments in parallel sandboxes.
        
        Args:
            experiments: List of {"rule_id": str, "git_ref": str, "change_script": str}
            max_workers: Max parallel containers
            
        Returns:
            Dict mapping rule_id → result dict
        """
        results = {}
        
        logger.info(f"Running {len(experiments)} experiments in parallel (max {max_workers})")
        
        with ThreadPoolExecutor(max_workers=max_workers) as executor:
            futures = {}
            for exp in experiments:
                future = executor.submit(
                    self.run_experiment,
                    rule_id=exp["rule_id"],
                    git_ref=exp.get("git_ref", "main"),
                    change_script=exp.get("change_script"),
                )
                futures[future] = exp["rule_id"]
            
            for future in as_completed(futures):
                rule_id = futures[future]
                try:
                    results[rule_id] = future.result()
                    status = results[rule_id].get("status", "?")
                    logger.info(f"  ✓ {rule_id}: {status}")
                except Exception as e:
                    results[rule_id] = {"status": "failed", "reason": str(e)}
                    logger.error(f"  ✗ {rule_id}: {e}")
        
        return results
    
    def baseline_all(self, rule_ids: List[str], git_ref: str = "main") -> Dict[str, Dict]:
        """Run baseline evaluation for multiple rules (no changes).
        
        This establishes the starting metrics for all rules against the corpus.
        """
        experiments = [
            {"rule_id": rid, "git_ref": git_ref}
            for rid in rule_ids
        ]
        return self.run_parallel(experiments, max_workers=3)
    
    def _force_kill(self, container_name: str):
        """Force-kill and remove a container."""
        try:
            subprocess.run(
                [CONTAINER_CMD, "kill", container_name],
                capture_output=True, timeout=5
            )
            subprocess.run(
                [CONTAINER_CMD, "rm", "-f", container_name],
                capture_output=True, timeout=5
            )
        except Exception:
            pass  # Container may already be dead
    
    def cleanup(self):
        """Remove all cognicode-eval-* containers (safety cleanup)."""
        result = subprocess.run(
            [CONTAINER_CMD, "ps", "-a", "--filter", "name=cognicode-eval-", 
             "--format", "{{.Names}}"],
            capture_output=True, text=True
        )
        for name in result.stdout.strip().split("\n"):
            if name:
                logger.info(f"Cleaning up: {name}")
                self._force_kill(name)


def _short_hash(length: int = 8) -> str:
    """Generate a short unique hash for container names."""
    return hashlib.md5(str(time.time()).encode()).hexdigest()[:length]


# ═══════════════════════════════════════════════════════════════════
# Quick test (run directly)
# ═══════════════════════════════════════════════════════════════════

if __name__ == "__main__":
    import sys
    logging.basicConfig(level=logging.INFO)
    
    manager = SandboxManager()
    
    if len(sys.argv) > 1:
        rule_id = sys.argv[1]
        result = manager.run_experiment(rule_id=rule_id)
        print(json.dumps(result, indent=2))
    else:
        # Dry run: baseline S134
        print("Running baseline for S134...")
        result = manager.run_experiment(rule_id="S134")
        print(json.dumps(result, indent=2))
