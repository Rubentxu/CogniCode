#!/usr/bin/env python3
"""KB → declare_rule! Generator.

Usage:
    python -m kb_generator --kb /tmp/kb.json --output rules/security
    python -m kb_generator --kb /tmp/kb.json --output rules/security --language javascript python
"""

import json
import re
import argparse
from pathlib import Path
from typing import Optional

# ─────────────────────────────────────────────────────────────────────────────
# PATTERN CONVERTERS
# ─────────────────────────────────────────────────────────────────────────────

def ast_grep_to_tree_sitter(pattern: str, language: str) -> str:
    """Convert ast-grep pattern to tree-sitter query.

    ast-grep → tree-sitter mappings:
        eval($$$ARGS)    → (call_expression (identifier) @call (#eq? @call "eval"))
        finalize($ARGS)  → (method_declaration (identifier) @m (#eq? @m "finalize"))
        f"...{$EXPR}..." → (string_literal) — heuristic
    """
    pattern = pattern.strip()

    # eval($$$ARGS) or function_name($$$ARGS)
    m = re.match(r'^(\w+)\(\$\$\$\w+\)$', pattern)
    if m:
        fn_name = m.group(1)
        return f'(call_expression (identifier) @call (#eq? @call "{fn_name}"))'

    # function_name($ARGS)
    m = re.match(r'^(\w+)\(\$\w+\)$', pattern)
    if m:
        fn_name = m.group(1)
        return f'(call_expression (identifier) @call (#eq? @call "{fn_name}"))'

    # Some(...) where Some is an identifier
    m = re.match(r'^(\w+)\(.+\)$', pattern)
    if m and not pattern.startswith('"'):
        fn_name = m.group(1)
        return f'(call_expression (identifier) @call (#eq? @call "{fn_name}"))'

    # f"..." string interpolation
    if 'f"' in pattern or "f'" in pattern:
        return '(string_literal) @str'

    # Generic: function_name(...)
    if '(' in pattern:
        fn_name = pattern.split('(')[0].strip()
        return f'(call_expression (identifier) @call (#eq? @call "{fn_name}"))'

    return f'(identifier) @id (#eq? @id "{pattern}")'


def pattern_to_code(pattern_type: str, pattern: str, language: str,
                     constraints: list) -> str:
    """Convert KB pattern to Rust check() body code."""
    if not pattern:
        return "vec![]"

    # ── regex ──────────────────────────────────────────────────────────────
    if pattern_type == "regex":
        return f'''let mut issues = Vec::new();
        if let Some(re) = regex::Regex::new(r##"{pattern}"##).ok() {{
            for m in re.find_iter(ctx.source) {{
                let line_number = ctx.source[..m.start()].lines().count() + 1;
                issues.push(Issue::new(
                    self.id(),
                    "Security issue detected",
                    self.severity(),
                    self.category(),
                    ctx.file_path,
                    line_number,
                ));
            }}
        }}
        issues'''

    # ── tree_sitter ───────────────────────────────────────────────────────
    if pattern_type == "tree_sitter":
        # Escape double quotes in pattern for Rust string safety
        escaped_pattern = pattern.replace('"', '\\"')
        nodes_code = f'ctx.query_nodes("{escaped_pattern}")'
        constraint_checks = []
        for c in constraints:
            if ':' in c:
                key, val = c.split(':', 1)
                if key == 'contains':
                    # Check the node's text, not the whole source
                    # Use OR for multiple contains checks (string may have password OR secret)
                    constraint_checks.append(f'node_text.contains("{val}")')
                elif key == 'not_contains':
                    constraint_checks.append(f'!node_text.contains("{val}")')

        # Use OR for multiple contains constraints (more practical for security)
        # e.g., "password" OR "secret" OR "api_key" in the same string
        if len(constraint_checks) > 1:
            cond = " || ".join(constraint_checks)
        elif constraint_checks:
            cond = constraint_checks[0]
        else:
            cond = "true"

        return f'''let mut issues = Vec::new();
        for node in {nodes_code} {{
            let node_text = node.utf8_text(ctx.source.as_bytes()).unwrap_or("");
            if {cond} {{
                let start = node.start_position();
                issues.push(Issue::new(
                    self.id(),
                    "Security issue detected",
                    self.severity(),
                    self.category(),
                    ctx.file_path,
                    start.row + 1,
                ));
            }}
        }}
        issues'''

    # ── ast_grep ──────────────────────────────────────────────────────────
    if pattern_type == "ast_grep":
        ts_query = ast_grep_to_tree_sitter(pattern, language)
        # Escape double quotes in ts_query for Rust string safety
        escaped_ts_query = ts_query.replace('"', '\\"')
        nodes_code = f'ctx.query_nodes("{escaped_ts_query}")'

        inside_checks = []
        for c in constraints:
            if c.startswith('inside:'):
                fn = c.split(':', 1)[1]
                inside_checks.append(f'ctx.source.contains("{fn}")')

        if inside_checks:
            cond = " && ".join(inside_checks)
            return f'''let mut issues = Vec::new();
        for node in {nodes_code} {{
            if {cond} {{
                let start = node.start_position();
                issues.push(Issue::new(
                    self.id(),
                    "Security issue detected",
                    self.severity(),
                    self.category(),
                    ctx.file_path,
                    start.row + 1,
                ));
            }}
        }}
        issues'''
        else:
            return f'''let mut issues = Vec::new();
        for node in {nodes_code} {{
            let start = node.start_position();
            issues.push(Issue::new(
                self.id(),
                "Security issue detected",
                self.severity(),
                self.category(),
                ctx.file_path,
                start.row + 1,
            ));
        }}
        issues'''

    # ── dataflow ──────────────────────────────────────────────────────────
    if pattern_type == "dataflow":
        keywords = []
        p_lower = pattern.lower()
        if 'userinput' in p_lower or 'user' in p_lower:
            keywords.extend(['input', 'user', 'req', 'request'])
        if 'shell' in p_lower or 'exec' in p_lower or 'system(' in pattern:
            keywords.extend(['exec', 'system', 'spawn', 'Command'])
        if 'sql' in p_lower or 'query' in p_lower:
            keywords.extend(['query', 'execute', 'sql', 'cursor'])
        if 'filepath' in p_lower or 'file' in p_lower or 'path' in p_lower:
            keywords.extend(['open', 'read', 'path', 'file'])
        if 'html' in p_lower or 'innerhtml' in p_lower:
            keywords.extend(['innerHTML', 'document.write', 'inner_html'])
        if 'xss' in p_lower:
            keywords.append('xss')
        if 'eval' in pattern:
            keywords.append('eval')
        if 'password' in p_lower or 'secret' in p_lower or 'credential' in p_lower:
            keywords.extend(['password', 'secret', 'api_key', 'token', 'credential'])
        if 'command' in p_lower:
            keywords.extend(['command', 'exec', 'run', 'bash'])

        seen, unique = set(), []
        for kw in keywords:
            k = kw.lower()
            if k not in seen:
                seen.add(k)
                unique.append(kw)

        kw_checks = ' || '.join(f'ctx.source.contains("{kw}")' for kw in unique[:6])
        if not kw_checks:
            kw_checks = 'true'

        return f'''let mut issues = Vec::new();
        if {kw_checks} {{
            let lines: Vec<&str> = ctx.source.lines().collect();
            for (i, _) in lines.iter().enumerate() {{
                issues.push(Issue::new(
                    self.id(),
                    "Security issue detected",
                    self.severity(),
                    self.category(),
                    ctx.file_path,
                    i + 1,
                ));
                break;  // Flag once per file for dataflow detection
            }}
        }}
        issues'''

    # ── keyword (fallback) ─────────────────────────────────────────────────
    escaped_pat = pattern.replace('\\', '\\\\').replace('"', '\\"')
    return f'''let mut issues = Vec::new();
        if ctx.source.contains("{escaped_pat}") {{
            let line_number = ctx.source.lines().count();
            issues.push(Issue::new(
                self.id(),
                "Security issue detected",
                self.severity(),
                self.category(),
                ctx.file_path,
                line_number,
            ));
        }}
        issues'''


# ─────────────────────────────────────────────────────────────────────────────
# KB RULE → declare_rule! CONVERTER
# ─────────────────────────────────────────────────────────────────────────────

SEVERITY_MAP = {
    "blocker": "Blocker",
    "critical": "Critical",
    "major": "Major",
    "minor": "Minor",
    "info": "Info",
}

IMPACT_MAP = {
    "CWE-89": "[Security: High]",
    "CWE-79": "[Security: High]",
    "CWE-78": "[Security: High]",
    "CWE-95": "[Security: High]",
    "CWE-22": "[Security: High]",
    "CWE-798": "[Security: High]",
    "CWE-502": "[Security: High]",
    "CWE-352": "[Security: High]",
    "CWE-200": "[Security: Medium]",
    "CWE-327": "[Security: High]",
    "CWE-476": "[Reliability: High]",
    "CWE-562": "[Security: Medium]",
    "CWE-020": "[Security: Medium]",
    "CWE-755": "[Security: High]",
}


def kb_to_declare_rule(kb_rule: dict, target_language: str = None) -> Optional[str]:
    """Convert a KB rule dict to a declare_rule! block string."""
    rule_id = kb_rule.get("id", "")
    if not rule_id:
        return None

    name = kb_rule.get("name", "")
    message = kb_rule.get("message", "") or name or rule_id
    severity_raw = kb_rule.get("severity", "major")
    severity = SEVERITY_MAP.get(severity_raw.lower(), "Major")

    # Sanitize rule_id for use as Rust Ident (struct name base)
    # Keep original for rule.id() but create safe version for struct
    safe_id = re.sub(r'[^a-zA-Z0-9]', '_', rule_id)
    safe_id = re.sub(r'_+', '_', safe_id).strip('_')

    languages = kb_rule.get("languages", [])
    if target_language and target_language not in languages:
        return None

    lang = languages[0] if languages else "generic"

    # Extract pattern
    detection = kb_rule.get("detection", {})
    primary = detection.get("primary_pattern", {})
    pattern_type = primary.get("pattern_type", "regex")
    pattern = primary.get("pattern", "")
    constraints = primary.get("constraints", [])

    # CWE/OWASP
    security = kb_rule.get("security", {})
    cwe_list = security.get("cwe", [])
    cwe_str = cwe_list[0] if cwe_list else ""
    owasp_list = security.get("owasp", [])
    owasp_str = owasp_list[0] if owasp_list else ""

    # Fix suggestion
    fix = kb_rule.get("fix")
    fix_suggestion = ""
    if fix:
        fix_suggestion = (fix.get("suggestion", "") or "").strip()

    # Generate check body
    check_body = pattern_to_code(pattern_type, pattern, lang, constraints)

    # Impact from CWE
    impact = IMPACT_MAP.get(cwe_str, "[Security: Medium]")

    # Sanitize strings for Rust
    def rs_str(s):
        return s.replace('\\', '\\\\').replace('"', '\\"').replace('\n', ' ').replace('\r', '')[:200]

    rule_block = f'''declare_rule! {{
    id: "{rule_id}"
    name: "{rs_str(name)}"
    severity: {severity}
    category: Vulnerability
    language: "{lang}"
    params: {{}}

    explanation: "{rs_str(fix_suggestion or f"Detects potential security issue in {lang} code.")}"

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
                            languages_filter: list = None,
                            dry_run: bool = False):
    """Generate Rust rule files from KB JSON."""

    with open(kb_path) as f:
        data = json.load(f)
    rules = data if isinstance(data, list) else data.get("rules", [])

    security_rules = [
        r for r in rules
        if r.get("category") in ("Security", "security")
        and r.get("detection", {}).get("primary_pattern", {}).get("pattern")
    ]

    output_path = Path(output_dir)
    if not dry_run:
        output_path.mkdir(parents=True, exist_ok=True)

    generated = []
    skipped = []

    for rule in security_rules:
        rule_id = rule.get("id", "")
        languages = rule.get("languages", [])

        if languages_filter:
            matching = [l for l in languages if l in languages_filter]
            if not matching:
                skipped.append(f"{rule_id} (language: {languages})")
                continue

        target_lang = languages[0] if languages else "generic"
        rule_code = kb_to_declare_rule(rule, target_lang)
        if not rule_code:
            skipped.append(f"{rule_id} (generation failed)")
            continue

        # File name from rule_id
        safe_name = re.sub(r'[^a-zA-Z0-9_]', '_', rule_id)
        safe_name = re.sub(r'_+', '_', safe_name).strip('_')
        fname = f"{safe_name}.rs"
        fpath = output_path / fname

        # Struct name must match what the macro generates:
        # macro sanitizes: sec/sql-injection-python -> sec_sql_injection_python -> sec_sql_injection_pythonRule
        safe_id = re.sub(r'[^a-zA-Z0-9]', '_', rule_id)
        safe_id = re.sub(r'_+', '_', safe_id).strip('_')
        struct_name = safe_id + "Rule"

        # Build file content
        header = f"//! Auto-generated from KB: {rule_id}\n"
        header += f"//! Languages: {', '.join(languages)}\n"
        header += "use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};\n"
        header += "use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};\n"
        header += "use cognicode_macros::declare_rule;\n"
        header += "use inventory::submit;\n"
        header += "use regex;\n\n"

        tests = f"""
#[cfg(test)]
mod tests {{
    use super::*;

    #[test]
    fn test_{safe_name}_registered() {{
        let rule = {struct_name}::new();
        assert_eq!(rule.id(), "{safe_id}");
        assert!(!rule.name().is_empty());
    }}
}}
"""

        full_content = header + rule_code + "\n" + tests

        if dry_run:
            print(f"\n{'='*60}")
            print(f"FILE: {fname}")
            print(f"{'='*60}")
            print(full_content[:500] + "..." if len(full_content) > 500 else full_content)
        else:
            fpath.write_text(full_content)

        generated.append((rule_id, fname, target_lang, struct_name))

    # Generate mod.rs
    if not dry_run:
        mod_rs = "// Auto-generated security rules module\n"
        mod_rs += "// Do not edit manually — regenerate with kb_generator\n\n"
        for rule_id, fname, _, _ in sorted(generated, key=lambda x: x[0]):
            mod_name = fname.replace('.rs', '')
            mod_rs += f"pub mod {mod_name};\n"

        (output_path / "mod.rs").write_text(mod_rs)

    return generated, skipped


# ─────────────────────────────────────────────────────────────────────────────
# CLI
# ─────────────────────────────────────────────────────────────────────────────

def main():
    parser = argparse.ArgumentParser(
        description="KB → declare_rule! Generator for CogniCode Security Rules"
    )
    parser.add_argument("--kb", required=True, help="Path to KB JSON file")
    parser.add_argument("--output", default="rules/security",
                        help="Output directory for generated rules")
    parser.add_argument("--language", nargs="+",
                        choices=["javascript", "typescript", "python", "java", "rust", "go", "generic"],
                        help="Filter by language(s)")
    parser.add_argument("--dry-run", action="store_true",
                        help="Print generated code without writing files")

    args = parser.parse_args()

    print(f"📖 KB: {args.kb}")
    with open(args.kb) as f:
        kb_data = json.load(f)
    rules = kb_data if isinstance(kb_data, list) else kb_data.get("rules", [])
    sec_rules = [r for r in rules if r.get("category") in ("Security", "security")]
    print(f"   {len(sec_rules)} security rules in KB")

    print(f"\n🔧 Generating security rules...")
    generated, skipped = generate_security_rules(
        args.kb,
        args.output,
        languages_filter=args.language,
        dry_run=args.dry_run,
    )

    print(f"\n✅ Generated {len(generated)} rule file(s):")
    for rule_id, fname, lang, struct in sorted(generated, key=lambda x: x[0]):
        print(f"   [{lang}] {rule_id} → {fname} ({struct})")

    if skipped:
        print(f"\n⏭ Skipped {len(skipped)}:")
        for s in skipped[:5]:
            print(f"   {s}")
        if len(skipped) > 5:
            print(f"   ... +{len(skipped)-5} more")

    if not args.dry_run:
        print(f"\n📁 Output: {args.output}/")
        print(f"   Add `pub mod rules.security;` to your catalog.rs")
        print(f"   And add `mod security;` to rules/mod.rs")


if __name__ == "__main__":
    main()
