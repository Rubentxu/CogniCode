#!/usr/bin/env python3
"""
KB → declare_rule! Generator

Reads security rules from the Knowledge Base (JSON) and generates
Rust `declare_rule!` blocks for cognicode-axiom.

Usage:
    python -m kb_generator.generate \
        --kb /tmp/kb-output/kb-final.json \
        --output crates/cognicode-axiom/src/rules/rules/security \
        --language javascript

Pattern conversion:
    regex     → ctx.source.contains(pattern)
    tree_sitter → ctx.query_nodes(tree_sitter_query)
    ast_grep  → converted to tree_sitter via heuristic
    dataflow  → ctx.source.contains() with sink keywords
"""

import json
import re
import sys
import argparse
from pathlib import Path
from typing import Optional

# ─────────────────────────────────────────────────────────────────────────────
# PATTERN CONVERTERS
# ─────────────────────────────────────────────────────────────────────────────

def ast_grep_to_tree_sitter(pattern: str, language: str) -> str:
    """Convert ast-grep pattern to tree-sitter query.

    ast-grep → tree-sitter mappings:
        eval($$$ARGS)        → (call_expression (identifier) @call (#eq? @call "eval"))
        finalize($ARGS) { }  → (method_declaration (identifier) @m (#eq? @m "finalize"))
        f"...{$EXPR}..."     → (string literal with interpolation) — heuristic
        Some(...)            → (expression)

    This handles the most common cases from the KB.
    """
    pattern = pattern.strip()

    # eval($$$ARGS) → find call_expression where callee is identifier "eval"
    if re.match(r'^(\w+)\(\$\$\$\w+\)$', pattern):
        fn_name = re.match(r'^(\w+)\(\$\$\$\w+\)$', pattern).group(1)
        return f'(call_expression (identifier) @call (#eq? @call "{fn_name}"))'

    # function_name($ARGS) → method call
    if re.match(r'^(\w+)\(\$\w+\)$', pattern):
        fn_name = re.match(r'^(\w+)\(\$\w+\)$', pattern).group(1)
        return f'(call_expression (identifier) @call (#eq? @call "{fn_name}"))'

    # Some(...) → expression
    if re.match(r'^(\w+)\(.+\)$', pattern) and not pattern.startswith('"'):
        fn_name = re.match(r'^(\w+)\(', pattern).group(1)
        return f'(call_expression (identifier) @call (#eq? @call "{fn_name}"))'

    # Pattern with inside: constraint (e.g., f"...{ $EXPR }..." inside execute)
    # Return generic string interpolation query
    if 'f"' in pattern or "f'" in pattern:
        return '(string_literal) @str'

    # Generic fallback — just look for the function name
    if '(' in pattern:
        fn_name = pattern.split('(')[0].strip()
        return f'(call_expression (identifier) @call (#eq? @call "{fn_name}"))'

    # Plain identifier
    return f'(identifier) @id (#eq? @id "{pattern}")'


def pattern_to_code(pattern_type: str, pattern: str, language: str,
                    constraints: list) -> str:
    """Convert KB pattern to Rust check() body code.

    Returns the body of the `check` closure.
    """
    if not pattern:
        return "vec![]"

    # ── regex ──────────────────────────────────────────────────────────────
    if pattern_type == "regex":
        # Escape the pattern for use in string literal
        escaped = pattern.replace('\\', '\\\\').replace('"', '\\"')
        escaped = escaped.replace('\n', '\\n').replace('\t', '\\t')
        return f"""let mut issues = Vec::new();
        let re = regex::Regex::new(r"{pattern}").ok();
        if let Some(re) = re {{
            for m in re.find_iter(ctx.source) {{
                let line_number = ctx.source[..m.start()].lines().count() + 1;
                issues.push(Issue::new(
                    self.id(),
                    self.message().to_string(),
                    self.severity(),
                    self.category(),
                    ctx.file_path,
                    line_number,
                ));
            }}
        }}
        issues"""

    # ── tree_sitter ───────────────────────────────────────────────────────
    if pattern_type == "tree_sitter":
        nodes_code = f'ctx.query_nodes("{pattern}")'

        # Apply constraints
        constraint_handlers = []
        for c in constraints:
            if ':' in c:
                key, val = c.split(':', 1)
                if key == 'contains':
                    constraint_handlers.append(f'ctx.source.contains("{val}")')
                elif key == 'not_contains':
                    constraint_handlers.append(f'!ctx.source.contains("{val}")')

        constraint_code = ""
        if constraint_handlers:
            constraint_code = "if " + " && ".join(constraint_handlers) + " {\n"

        code = f"""let mut issues = Vec::new();
        for node in {nodes_code} {{
{constraint_code}            let start = node.start_position();
            issues.push(Issue::new(
                self.id(),
                self.message().to_string(),
                self.severity(),
                self.category(),
                ctx.file_path,
                start.row + 1,
            ));
{'            }' if constraint_code else ''}}}
        issues"""

        return code

    # ── ast_grep ──────────────────────────────────────────────────────────
    if pattern_type == "ast_grep":
        ts_query = ast_grep_to_tree_sitter(pattern, language)
        nodes_code = f'ctx.query_nodes("{ts_query}")'

        # Apply inside: constraint as additional node filtering
        inside_handlers = []
        for c in constraints:
            if c.startswith('inside:'):
                inside_fn = c.split(':', 1)[1]
                inside_handlers.append(f'ctx.source.contains("{inside_fn}")')

        inside_code = ""
        if inside_handlers:
            inside_code = "if " + " && ".join(inside_handlers) + " {\n"

        code = f"""let mut issues = Vec::new();
        for node in {nodes_code} {{
{inside_code}            let start = node.start_position();
            issues.push(Issue::new(
                self.id(),
                self.message().to_string(),
                self.severity(),
                self.category(),
                ctx.file_path,
                start.row + 1,
            ));
{'            }' if inside_code else ''}}}
        issues"""

        return code

    # ── dataflow ──────────────────────────────────────────────────────────
    if pattern_type == "dataflow":
        # Extract keywords from pattern: "UserInput -> ShellCommand"
        keywords = []
        if 'UserInput' in pattern or 'user' in pattern.lower():
            keywords.append('input')
        if 'ShellCommand' in pattern or 'exec' in pattern or 'system(' in pattern:
            keywords.append('exec')
            keywords.append('system')
        if 'SQL' in pattern or 'query' in pattern.lower():
            keywords.append('query')
            keywords.append('execute')
        if 'FilePath' in pattern or 'file' in pattern.lower():
            keywords.append('open')
            keywords.append('read')
        if 'HTML' in pattern or 'innerHTML' in pattern:
            keywords.append('innerHTML')
            keywords.append('document.write')
        if 'XSS' in pattern:
            keywords.append('xss')
        if 'eval(' in pattern:
            keywords.append('eval')
        if 'password' in pattern or 'secret' in pattern or 'credential' in pattern:
            keywords.append('password')
            keywords.append('secret')

        # Deduplicate
        seen = set()
        unique_kws = []
        for kw in keywords:
            k = kw.lower()
            if k not in seen:
                seen.add(k)
                unique_kws.append(kw)

        kw_checks = ' || '.join(f'ctx.source.contains("{kw}")' for kw in unique_kws[:5])

        return f"""let mut issues = Vec::new();
        if {kw_checks} {{
            let lines: Vec<&str> = ctx.source.lines().collect();
            for (i, line) in lines.iter().enumerate() {{
                issues.push(Issue::new(
                    self.id(),
                    self.message().to_string(),
                    self.severity(),
                    self.category(),
                    ctx.file_path,
                    i + 1,
                ));
                break;  // Dataflow: flag the file once
            }}
        }}
        issues"""

    # ── keyword (default) ─────────────────────────────────────────────────
    return f"""let mut issues = Vec::new();
        if ctx.source.contains("{pattern}") {{
            let line_number = ctx.source.lines().count();
            issues.push(Issue::new(
                self.id(),
                self.message().to_string(),
                self.severity(),
                self.category(),
                ctx.file_path,
                line_number,
            ));
        }}
        issues"""


# ─────────────────────────────────────────────────────────────────────────────
# KB RULE → declare_rule! CONVERTER
# ─────────────────────────────────────────────────────────────────────────────

def kb_to_declare_rule(kb_rule: dict, target_language: str = None) -> Optional[str]:
    """Convert a KB rule dict to a declare_rule! block string.

    Returns None if the rule can't be converted (e.g., wrong language).
    """
    rule_id = kb_rule.get("id", "")
    name = kb_rule.get("name", "")
    message = kb_rule.get("message", "") or name
    severity_raw = kb_rule.get("severity", "major")

    # Filter by language if specified
    languages = kb_rule.get("languages", [])
    if target_language and target_language not in languages:
        return None

    # Map severity
    severity_map = {
        "blocker": "Blocker",
        "critical": "Critical",
        "major": "Major",
        "minor": "Minor",
        "info": "Info",
    }
    severity = severity_map.get(severity_raw.lower(), "Major")

    # Map category (declare_rule! uses Vulnerability for security)
    category = "Vulnerability"

    # Extract detection info
    detection = kb_rule.get("detection", {})
    primary = detection.get("primary_pattern", {})
    pattern_type = primary.get("pattern_type", "regex")
    pattern = primary.get("pattern", "")
    constraints = primary.get("constraints", [])

    # CWE and OWASP
    security = kb_rule.get("security", {})
    cwe_list = security.get("cwe", [])
    cwe_str = cwe_list[0] if cwe_list else ""
    owasp_list = security.get("owasp", [])
    owasp_str = owasp_list[0] if owasp_list else ""

    # Fix suggestion
    fix = kb_rule.get("fix")
    fix_suggestion = ""
    if fix:
        fix_suggestion = fix.get("suggestion", "") or ""

    # Examples
    examples = kb_rule.get("examples", {})
    vuln_examples = examples.get("vulnerable", [])
    secure_examples = examples.get("secure", [])

    # Generate check() body
    check_body = pattern_to_code(pattern_type, pattern, languages[0] if languages else "generic", constraints)

    # Map CWE to impacts
    impact_map = {
        "CWE-89": "[Security: Critical]",
        "CWE-79": "[Security: High]",
        "CWE-78": "[Security: Critical]",
        "CWE-95": "[Security: High]",
        "CWE-22": "[Security: High]",
        "CWE-798": "[Security: Critical]",
        "CWE-502": "[Security: High]",
        "CWE-352": "[Security: High]",
        "CWE-200": "[Security: Medium]",
    }
    impact = impact_map.get(cwe_str, "[Security: Medium]")

    # Get language string for Rust
    lang = languages[0] if languages else "generic"

    # Build the declare_rule! block
    rule_block = f'''declare_rule! {{
    id: "{rule_id}"
    name: "{name}"
    severity: {severity}
    category: {category}
    language: "{lang}"

    explanation: "{fix_suggestion or f"Detects potential security issue in {lang} code."}"

    impacts: {impact}

    check: => {{
        {check_body}
    }}
}}
'''
    return rule_block


# ─────────────────────────────────────────────────────────────────────────────
# FILE GENERATOR
# ─────────────────────────────────────────────────────────────────────────────

def generate_security_rules(kb_path: str, output_dir: str,
                             languages_filter: list = None):
    """Generate Rust rule files from KB JSON for all security rules."""

    # Load KB
    with open(kb_path) as f:
        data = json.load(f)

    rules = data if isinstance(data, list) else data.get("rules", [])

    # Filter security rules
    security_rules = [
        r for r in rules
        if r.get("category") in ("Security", "security")
        and r.get("detection", {}).get("primary_pattern", {}).get("pattern")
    ]

    output_path = Path(output_dir)
    output_path.mkdir(parents=True, exist_ok=True)

    generated = []
    skipped = []

    for rule in security_rules:
        rule_id = rule.get("id", "")
        languages = rule.get("languages", [])

        # Filter by language if specified
        if languages_filter:
            matching_langs = [l for l in languages if l in languages_filter]
            if not matching_langs:
                skipped.append(f"{rule_id} (language mismatch: {languages})")
                continue

        # Pick the primary language for this rule
        target_lang = languages[0] if languages else "generic"

        # Generate declare_rule! block
        rule_code = kb_to_declare_rule(rule, target_lang)
        if not rule_code:
            skipped.append(f"{rule_id} (generation failed)")
            continue

        # Write individual rule file
        safe_name = rule_id.replace("/", "_").replace("-", "_")
        fname = f"{safe_name}.rs"
        fpath = output_path / fname

        header = f"//! Generated from Knowledge Base — {rule_id}\n"
        header += f"//! Languages: {', '.join(languages)}\n"
        header += "use crate::{{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry}};\n"
        header += "use crate::rules::{{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity}};\n"
        header += "use cognicode_macros::declare_rule;\n"
        header += "use inventory::submit;\n\n"

        full_content = header + rule_code + "\n#[cfg(test)]\nmod tests {\n    use super::*;\n\n    #[test]\n    fn test_" + safe_name + """_registered() {
        let rule = """ + rule_id.replace("/", "").replace("-", "_").title().replace("_", "") + "Rule::new();\n"
        full_content += f"        assert_eq!(rule.id(), \"{rule_id}\");\n"
        full_content += "    }\n}\n"

        fpath.write_text(full_content)
        generated.append((rule_id, fname, target_lang))

    # Generate mod.rs
    mod_rs = "// Auto-generated security rules\n"
    mod_rs += "// Run `python -m kb_generator.generate` to regenerate\n\n"
    for rule_id, fname, lang in sorted(generated, key=lambda x: x[0]):
        mod_name = fname.replace(".rs", "")
        mod_rs += f"pub mod {mod_name};\n"

    (output_path / "mod.rs").write_text(mod_rs)

    return generated, skipped


# ─────────────────────────────────────────────────────────────────────────────
# CLI
# ─────────────────────────────────────────────────────────────────────────────

def main():
    parser = argparse.ArgumentParser(description="KB → declare_rule! Generator")
    parser.add_argument("--kb", required=True, help="Path to KB JSON file")
    parser.add_argument("--output", default="rules/security", help="Output directory")
    parser.add_argument("--language", nargs="+",
                        choices=["javascript", "typescript", "python", "java", "rust", "go"],
                        help="Filter by language(s)")
    parser.add_argument("--dry-run", action="store_true", help="Print without writing")

    args = parser.parse_args()

    print(f"📖 Reading KB: {args.kb}")
    with open(args.kb) as f:
        kb_data = json.load(f)
    rules = kb_data if isinstance(kb_data, list) else kb_data.get("rules", [])
    sec_rules = [r for r in rules if r.get("category") in ("Security", "security")]
    print(f"   Found {len(sec_rules)} security rules in KB")

    print(f"\n🔧 Generating rules...")
    generated, skipped = generate_security_rules(
        args.kb,
        args.output,
        languages_filter=args.language,
    )

    print(f"\n✅ Generated {len(generated)} rule(s):")
    for rule_id, fname, lang in generated:
        print(f"   {rule_id} ({lang}) → {fname}")

    if skipped:
        print(f"\n⏭ Skipped {len(skipped)} rule(s):")
        for s in skipped[:10]:
            print(f"   {s}")
        if len(skipped) > 10:
            print(f"   ... and {len(skipped) - 10} more")

    print(f"\n📁 Output directory: {args.output}")
    print(f"   Add `pub mod rules.security;` to your module tree")


if __name__ == "__main__":
    main()
