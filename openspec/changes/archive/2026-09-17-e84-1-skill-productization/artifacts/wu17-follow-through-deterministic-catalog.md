# WU17 follow-through — Deterministic runtime-catalog lookup

## What was wrong

`scripts/validate_skills.py::_find_catalog()` (after the archive
fix in `ba60e806`) walked `openspec/changes/archive/*/artifacts/`
and returned `candidates[0]`:

```python
def _find_catalog() -> Path | None:
    candidates = []
    arch = REPO_ROOT / "openspec/changes/archive"
    if arch.exists():
        for sub in arch.iterdir():
            if not sub.is_dir():
                continue
            cand = sub / "artifacts" / "runtime-tools-list.json"
            if cand.exists():
                candidates.append(cand)
    return candidates[0] if candidates else None
```

`Path.iterdir()` order is **not guaranteed** by the filesystem.
With one catalog today this is harmless; with two or more
(future cycles will append), the validator might silently pick
the wrong one — leading to **stale tool-ref validation** that
would pass on old data but fail on fresh data.

This is exactly the gotcha the user flagged: "future cycles
should keep this pattern (don't hardcode cycle-specific paths in
cross-cutting tools)".

## Fix

Sort candidates by `YYYY-MM-DD-` prefix descending; pick the
newest. Tie-break by full path so behavior is deterministic.

```python
_DATE_PREFIX = re.compile(r"^(\d{4}-\d{2}-\d{2})-")


def _catalog_sort_key(path: Path) -> str:
    m = _DATE_PREFIX.match(path.parent.parent.name)
    return m.group(1) if m else ""


def _find_catalog() -> Path | None:
    candidates: list[Path] = []
    arch = REPO_ROOT / "openspec/changes/archive"
    if arch.exists():
        for sub in arch.iterdir():
            if not sub.is_dir():
                continue
            cand = sub / "artifacts" / "runtime-tools-list.json"
            if cand.exists():
                candidates.append(cand)
    if not candidates:
        return None
    candidates.sort(key=lambda p: (_catalog_sort_key(p), p.as_posix()), reverse=True)
    return candidates[0]
```

## Verification

- Single-catalog case (today): unchanged behavior. PASS.
- Mocked multi-catalog case: `2026-09-17-e84` beats
  `2026-08-06-e86` beats no-date. Correct.
- All 4 skills still PASS validation. Tool-ref count: 52 (up
  from 51 — see skill update in WU18 follow-through).

## Why this is better than alternatives

- Hardcoded path: same bug as before, just shifted.
- env var: would require setting it in every dev/CI invocation.
- Hash the catalog file and compare: solves staleness but not
  ordering.
- Date-prefix sort: zero-config, deterministic, matches the
  existing archive naming convention.
