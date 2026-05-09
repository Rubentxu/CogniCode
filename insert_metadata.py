#!/usr/bin/env python3
"""Insert metadata (explanation, clean_code, impacts) into catalog.rs declare_rule! blocks."""

import json
import re
from pathlib import Path

def main():
    # Load metadata
    metadata_path = Path("rules_metadata_all.json")
    catalog_path = Path("crates/cognicode-axiom/src/rules/catalog.rs")

    with open(metadata_path) as f:
        metadata = json.load(f)

    with open(catalog_path) as f:
        content = f.read()

    # Valid enum values (must match Rust enums)
    valid_clean_code = {
        "Formatted", "Conventional", "Identifiable", "Clear", "Logical",
        "Complete", "Efficient", "Focused", "Distinct", "Modular",
        "Lawful", "Trustworthy", "Respectful"
    }
    valid_qualities = {"Security", "Reliability", "Maintainability"}
    valid_severities = {"Blocker", "High", "Medium", "Low"}

    # Track stats
    updated = 0
    missing = 0
    skipped = 0

    def sanitize_string(s):
        """Escape double quotes and backslashes in string."""
        if s is None:
            return None
        # Remove control characters but keep newlines as \n
        s = s.replace('\\', '\\\\').replace('"', '\\"').replace('\r', '')
        return s

    def format_explanation(exp):
        if exp is None:
            return None
        sanitized = sanitize_string(exp)
        return f'explanation: "{sanitized}"'

    def format_clean_code(cc):
        if cc is None:
            return None
        cc = cc.strip()
        if cc not in valid_clean_code:
            return None
        return f"clean_code: {cc}"

    def format_impacts(impacts):
        if not impacts:
            return None
        formatted = []
        for imp in impacts:
            # Handle both ["Quality", "Severity"] and [["Quality", "Severity"]] formats
            if isinstance(imp, list) and len(imp) == 2:
                quality, severity = imp
                # Unwrap nested array if needed
                if isinstance(quality, list):
                    quality, severity = quality[0], quality[1]
            elif isinstance(imp, str):
                # Handle "Quality/Severity" format
                parts = imp.split("/")
                if len(parts) == 2:
                    quality, severity = parts
                else:
                    continue
            else:
                continue

            quality = quality.strip()
            severity = severity.strip()
            if quality in valid_qualities and severity in valid_severities:
                formatted.append(f"{quality}: {severity}")

        if not formatted:
            return None
        return f"impacts: [{', '.join(formatted)}]"

    # Pattern to find declare_rule! blocks
    # Matches: declare_rule! { ... id: "S134" ... check: => { ... } }
    pattern = re.compile(
        r'(declare_rule!\s*\{)(.*?)(\n\})',
        re.DOTALL
    )

    def replace_rule_block(match):
        nonlocal updated, missing, skipped
        prefix = match.group(1)
        body = match.group(2)
        suffix = match.group(3)

        # Extract rule ID
        id_match = re.search(r'id:\s*"([^"]+)"', body)
        if not id_match:
            skipped += 1
            return match.group(0)

        rule_id = id_match.group(1)

        # Get metadata for this rule
        if rule_id not in metadata:
            missing += 1
            return match.group(0)

        meta = metadata[rule_id]

        # Build new fields
        new_fields = []
        exp_field = format_explanation(meta.get("explanation"))
        if exp_field:
            new_fields.append(exp_field)

        cc_field = format_clean_code(meta.get("clean_code"))
        if cc_field:
            new_fields.append(cc_field)

        imp_field = format_impacts(meta.get("impacts"))
        if imp_field:
            new_fields.append(imp_field)

        if not new_fields:
            skipped += 1
            return match.group(0)

        # Build the new body by inserting fields before 'check:'
        # Find where 'check:' starts
        check_pos = body.find("check:")
        if check_pos == -1:
            skipped += 1
            return match.group(0)

        # Find the line start before check
        before_check = body[:check_pos].rstrip()
        # Only add comma if it doesn't already end with one and doesn't end with }
        # (params: { ... } has no trailing comma before check:)
        if not before_check.endswith(",") and not before_check.endswith("}"):
            before_check += ","

        new_fields_str = "\n    " + ",\n    ".join(new_fields) + ",\n    "
        new_body = before_check + "\n" + new_fields_str + body[check_pos:]

        updated += 1
        return prefix + new_body + suffix

    new_content = pattern.sub(replace_rule_block, content)

    # Write updated catalog
    with open(catalog_path, "w") as f:
        f.write(new_content)

    print(f"Done! Updated: {updated}, Missing in JSON: {missing}, Skipped: {skipped}")
    print(f"Total rules in JSON: {len(metadata)}")

if __name__ == "__main__":
    main()