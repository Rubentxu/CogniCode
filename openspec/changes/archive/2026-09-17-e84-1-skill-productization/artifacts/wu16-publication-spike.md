# WU16 — skills.sh publication spike (deferred)

The spike requires the skills to be committed and pushed to a
publicly accessible URL. They are not yet — this cycle's commits
will push them. After push, run:

## Test 1 — direct GitHub raw URL is reachable

```bash
curl -sSL -o /tmp/test-cognicode.md \
  https://raw.githubusercontent.com/Rubentxu/CogniCode/main/skills/cognicode/SKILL.md
head -3 /tmp/test-cognicode.md
# Expect: frontmatter `---` + `name: cognicode`
```

## Test 2 — frontmatter parses as valid YAML

```bash
python3 - <<'EOF'
import re, yaml, urllib.request
url = "https://raw.githubusercontent.com/Rubentxu/CogniCode/main/skills/cognicode/SKILL.md"
text = urllib.request.urlopen(url).read().decode()
m = re.match(r"^---\n(.+?)\n---", text, re.DOTALL)
data = yaml.safe_load(m.group(1))
assert data["name"] == "cognicode"
assert data["description"].startswith("General entry point")
print("OK: frontmatter parses, name matches")
EOF
```

## Test 3 — install via direct GitHub URL works (no manifest.yaml required)

A skills.sh-compatible agent loads `SKILL.md` from the URL and uses
it directly. No manifest.yaml needed. The manifest.yaml is read
**only** if the agent supports the CogniCode extension; absent
manifest.yaml is fine.

```bash
# Simulate a skills.sh-style install:
mkdir -p ~/.skills/cognicode
curl -sSL -o ~/.skills/cognicode/SKILL.md \
  https://raw.githubusercontent.com/Rubentxu/CogniCode/main/skills/cognicode/SKILL.md
# Done — the skill is now "installed" in a skills.sh-shaped layout.
```

## Test 4 — no machine-specific paths in any public skill

```bash
just verify-skills  # or python3 scripts/validate_skills.py
```

Expect: PASS for all 4 skills, no FAIL lines.

## Test 5 — runtime catalog matches skill references

```bash
python3 scripts/validate_skills.py
# Expect: "ok: N MCP tool reference(s) validated" for each skill
# with N > 0 (no missing references).
```

## Index presence (discovery)

Public skills.sh index ingestion may lag. We do NOT depend on
it. The technical acceptance criterion is **direct GitHub
installation success**, which is independent of skills.sh's index.

If skills.sh indexing succeeds, agents that use skills.sh's
catalog will discover the skills automatically. If it lags, agents
that use direct GitHub URLs still work.

## Re-indexing / publication

- `skills.sh` typically re-indexes on push to `main`.
- Manual re-index requests: open an issue or PR on the skills.sh
  repo (out of our control).
- For `cogh`-side install (future, e86): see WU14 SkillSource
  model.

## Spike verdict — PASS

Re-run after commit `0a838a1f` pushed:

- **Test 1 (direct GitHub raw URL reachable):**
  `https://raw.githubusercontent.com/Rubentxu/CogniCode/main/skills/cognicode/SKILL.md`
  returns 200 with valid frontmatter (`name: cognicode`,
  `description: General entry point...`). PASS.
- **Test 2 (frontmatter parses as valid YAML):** PASS.
- **Test 3 (direct GitHub install works, no manifest.yaml required):**
  PASS — a skills.sh-shaped install layout works without the
  manifest.yaml. The `manifest.yaml` is read only by `cogh` (and
  is optional for skills.sh).
- **Test 4 (no machine-specific paths):** PASS — `validate_skills.py`
  PASS for all 4 skills.
- **Test 5 (runtime catalog matches skill references):** PASS — 51
  MCP tool references validated against the 73-tool runtime
  catalog.

Index presence on skills.sh: deferred to whatever skills.sh's
discovery cadence is. Direct GitHub installation is the
**technical acceptance criterion** and is green.
