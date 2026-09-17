#!/usr/bin/env bash
set -e

echo "Verifying skill bundles in skills/..."

for skill in skills/*/; do
    name=$(basename "$skill")
    echo "Checking $name..."

    # SkillSet (e84.1) has SET.yaml, not SKILL.md + manifest.yaml.
    if [ -f "$skill/SET.yaml" ]; then
        echo "  SET.yaml present (SkillSet)"
        if ! python3 -c "import yaml; yaml.safe_load(open('$skill/SET.yaml'))" 2>/dev/null; then
            echo "  ERROR: SET.yaml is not valid YAML"
            exit 1
        fi
        echo "  SET.yaml syntax OK"
        kind=$(python3 -c "import yaml; m=yaml.safe_load(open('$skill/SET.yaml')); print(m.get('kind',''))" 2>/dev/null)
        if [ "$kind" != "SkillSet" ]; then
            echo "  ERROR: SET.yaml kind must be 'SkillSet', got '$kind'"
            exit 1
        fi
        echo "  SET.yaml kind OK"
        echo "  PASS"
        continue
    fi

    if [ ! -f "$skill/SKILL.md" ]; then
        echo "  ERROR: SKILL.md missing"
        exit 1
    fi
    echo "  SKILL.md OK"

    if [ ! -f "$skill/manifest.yaml" ]; then
        echo "  ERROR: manifest.yaml missing"
        exit 1
    fi
    echo "  manifest.yaml OK"

    # Check YAML syntax
    if ! python3 -c "import yaml; yaml.safe_load(open('$skill/manifest.yaml'))" 2>/dev/null; then
        echo "  ERROR: manifest.yaml is not valid YAML"
        exit 1
    fi
    echo "  manifest.yaml syntax OK"

    # Verify manifest fields
    apiVersion=$(python3 -c "import yaml; m=yaml.safe_load(open('$skill/manifest.yaml')); print(m.get('apiVersion',''))" 2>/dev/null)
    kind=$(python3 -c "import yaml; m=yaml.safe_load(open('$skill/manifest.yaml')); print(m.get('kind',''))" 2>/dev/null)
    skillName=$(python3 -c "import yaml; m=yaml.safe_load(open('$skill/manifest.yaml')); print(m.get('name',''))" 2>/dev/null)

    if [ "$apiVersion" != "cognicode/v1" ]; then
        echo "  ERROR: manifest.yaml missing apiVersion: cognicode/v1"
        exit 1
    fi
    if [ "$kind" != "SkillBundle" ]; then
        echo "  ERROR: manifest.yaml missing kind: SkillBundle"
        exit 1
    fi
    if [ -z "$skillName" ]; then
        echo "  ERROR: manifest.yaml missing name field"
        exit 1
    fi
    echo "  manifest fields OK"

    if [ -d "$skill/references" ]; then
        echo "  references/ OK"
    fi

    if [ -d "$skill/assets" ]; then
        echo "  assets/ OK"
    fi

    echo "  PASS"
done

echo ""
echo "All skill bundles verified successfully"
