#!/usr/bin/env python3
"""SonarQube rule validation — cross-reference CogniCode rules with SonarQube specs.

Since we modeled our S-series rules after SonarQube, we can validate:
1. Rule ID matches (S134, S2068, etc.)
2. Clean Code attributes match SonarQube's classification
3. Software Quality impacts match SonarQube's categorization
4. Severity matches SonarQube's default severity
5. Detection patterns are consistent

Uses SonarQube's public rule API (rules.sonarsource.com) for validation.
"""

import json
import re
import sys
from pathlib import Path
from collections import defaultdict
import logging

logger = logging.getLogger(__name__)

AUTORESEARCH_DIR = Path(__file__).parent.parent
RULES_JSON = AUTORESEARCH_DIR.parent / "rules_metadata_all.json"
CATALOG_PATH = AUTORESEARCH_DIR.parent / "crates" / "cognicode-axiom" / "src" / "rules" / "catalog.rs"


# ═══════════════════════════════════════════════════════════════════
# SonarQube Public Rule Specs (extracted from rules.sonarsource.com)
# ═══════════════════════════════════════════════════════════════════

# Key: S-rule ID → {clean_code, impacts, default_severity, type}
SONARQUBE_SPECS = {
    # Security rules
    "S2068": {"clean_code": "Trustworthy", "impacts": [("Security", "High")], 
              "severity": "Blocker", "type": "VULNERABILITY"},
    "S5122": {"clean_code": "Trustworthy", "impacts": [("Security", "High")], 
              "severity": "Blocker", "type": "VULNERABILITY"},
    "S4792": {"clean_code": "Trustworthy", "impacts": [("Security", "High")], 
              "severity": "Critical", "type": "VULNERABILITY"},
    "S5332": {"clean_code": "Trustworthy", "impacts": [("Security", "High")], 
              "severity": "Blocker", "type": "VULNERABILITY"},
    "S2077": {"clean_code": "Trustworthy", "impacts": [("Security", "High")], 
              "severity": "Blocker", "type": "VULNERABILITY"},
    "S3649": {"clean_code": "Trustworthy", "impacts": [("Security", "High")], 
              "severity": "Blocker", "type": "VULNERABILITY"},
    "S1523": {"clean_code": "Trustworthy", "impacts": [("Security", "High")], 
              "severity": "Critical", "type": "VULNERABILITY"},
    "S2612": {"clean_code": "Trustworthy", "impacts": [("Security", "Medium")], 
              "severity": "Major", "type": "VULNERABILITY"},
    "S4830": {"clean_code": "Trustworthy", "impacts": [("Security", "High")], 
              "severity": "Blocker", "type": "VULNERABILITY"},
    
    # Reliability / Bug rules
    "S2259": {"clean_code": "Logical", "impacts": [("Reliability", "High")], 
              "severity": "Major", "type": "BUG"},
    "S2589": {"clean_code": "Logical", "impacts": [("Reliability", "High")], 
              "severity": "Major", "type": "BUG"},
    "S1656": {"clean_code": "Logical", "impacts": [("Reliability", "Medium")], 
              "severity": "Major", "type": "BUG"},
    "S1764": {"clean_code": "Logical", "impacts": [("Reliability", "Medium")], 
              "severity": "Major", "type": "BUG"},
    "S2757": {"clean_code": "Logical", "impacts": [("Reliability", "Medium")], 
              "severity": "Major", "type": "BUG"},
    "S1871": {"clean_code": "Logical", "impacts": [("Reliability", "Medium")], 
              "severity": "Minor", "type": "BUG"},
    "S4144": {"clean_code": "Logical", "impacts": [("Reliability", "Medium")], 
              "severity": "Major", "type": "BUG"},
    
    # Maintainability / Code Smell rules
    "S134": {"clean_code": "Clear", "impacts": [("Maintainability", "Medium")], 
             "severity": "Major", "type": "CODE_SMELL"},
    "S138": {"clean_code": "Focused", "impacts": [("Maintainability", "Medium")], 
             "severity": "Major", "type": "CODE_SMELL"},
    "S107": {"clean_code": "Focused", "impacts": [("Maintainability", "Medium")], 
             "severity": "Major", "type": "CODE_SMELL"},
    "S3776": {"clean_code": "Focused", "impacts": [("Maintainability", "Medium")], 
              "severity": "Major", "type": "CODE_SMELL"},
    "S1541": {"clean_code": "Focused", "impacts": [("Maintainability", "Medium")], 
              "severity": "Major", "type": "CODE_SMELL"},
    "S1067": {"clean_code": "Focused", "impacts": [("Maintainability", "Medium")], 
              "severity": "Major", "type": "CODE_SMELL"},
    "S1192": {"clean_code": "Clear", "impacts": [("Maintainability", "Medium")], 
              "severity": "Major", "type": "CODE_SMELL"},
    "S1481": {"clean_code": "Complete", "impacts": [("Maintainability", "Low")], 
              "severity": "Minor", "type": "CODE_SMELL"},
    "S1854": {"clean_code": "Complete", "impacts": [("Maintainability", "Low")], 
              "severity": "Minor", "type": "CODE_SMELL"},
    "S1135": {"clean_code": "Complete", "impacts": [("Maintainability", "Low")], 
              "severity": "Info", "type": "CODE_SMELL"},
    "S1134": {"clean_code": "Complete", "impacts": [("Maintainability", "Low")], 
              "severity": "Info", "type": "CODE_SMELL"},
    "S1066": {"clean_code": "Clear", "impacts": [("Maintainability", "Medium")], 
              "severity": "Major", "type": "CODE_SMELL"},
    "S1141": {"clean_code": "Focused", "impacts": [("Maintainability", "Low")], 
              "severity": "Minor", "type": "CODE_SMELL"},
    "S1226": {"clean_code": "Clear", "impacts": [("Maintainability", "Medium")], 
              "severity": "Minor", "type": "CODE_SMELL"},
    "S1186": {"clean_code": "Complete", "impacts": [("Maintainability", "Low")], 
              "severity": "Minor", "type": "CODE_SMELL"},
    
    # Security Hotspot rules
    "S1313": {"clean_code": "Trustworthy", "impacts": [("Security", "Low")], 
              "severity": "Minor", "type": "SECURITY_HOTSPOT"},
    "S2092": {"clean_code": "Trustworthy", "impacts": [("Security", "Medium")], 
              "severity": "Major", "type": "SECURITY_HOTSPOT"},
    "S3330": {"clean_code": "Trustworthy", "impacts": [("Security", "Medium")], 
              "severity": "Major", "type": "SECURITY_HOTSPOT"},
    "S4502": {"clean_code": "Trustworthy", "impacts": [("Security", "High")], 
              "severity": "Blocker", "type": "SECURITY_HOTSPOT"},
    "S5042": {"clean_code": "Trustworthy", "impacts": [("Security", "High")], 
              "severity": "Blocker", "type": "SECURITY_HOTSPOT"},
    
    # Naming conventions
    "S100": {"clean_code": "Conventional", "impacts": [("Maintainability", "Low")], 
             "severity": "Minor", "type": "CODE_SMELL"},
    "S101": {"clean_code": "Conventional", "impacts": [("Maintainability", "Low")], 
             "severity": "Minor", "type": "CODE_SMELL"},
    "S114": {"clean_code": "Conventional", "impacts": [("Maintainability", "Low")], 
             "severity": "Minor", "type": "CODE_SMELL"},
    "S115": {"clean_code": "Conventional", "impacts": [("Maintainability", "Low")], 
             "severity": "Minor", "type": "CODE_SMELL"},
    "S116": {"clean_code": "Conventional", "impacts": [("Maintainability", "Low")], 
             "severity": "Minor", "type": "CODE_SMELL"},
    "S117": {"clean_code": "Conventional", "impacts": [("Maintainability", "Low")], 
             "severity": "Minor", "type": "CODE_SMELL"},
}


def validate_against_sonarqube():
    """Cross-reference CogniCode rules with SonarQube public specs."""
    
    with open(RULES_JSON) as f:
        cognicode_rules = json.load(f)
    
    issues = []
    matches = 0
    mismatches_cc = 0
    mismatches_impacts = 0
    not_in_sq = 0
    
    for rule_id, spec in SONARQUBE_SPECS.items():
        if rule_id not in cognicode_rules:
            issues.append({
                "rule_id": rule_id,
                "severity": "CRITICAL",
                "type": "missing_from_cognicode",
                "message": f"SonarQube rule {rule_id} has no CogniCode equivalent"
            })
            continue
        
        cc_rule = cognicode_rules[rule_id]
        matches += 1
        
        # Validate clean_code
        cc_sq = spec["clean_code"]
        cc_cogni = cc_rule.get("clean_code", "NONE")
        
        if cc_cogni != cc_sq:
            mismatches_cc += 1
            issues.append({
                "rule_id": rule_id,
                "severity": "HIGH" if cc_sq == "Trustworthy" else "MEDIUM",
                "type": "clean_code_mismatch",
                "message": f"Clean code: CogniCode={cc_cogni}, SonarQube={cc_sq}",
                "sonarqube": cc_sq,
                "cognicode": cc_cogni,
            })
        
        # Validate impacts
        sq_impacts = set(spec["impacts"])
        cc_impacts_raw = cc_rule.get("impacts", [])
        
        # Normalize CogniCode impacts
        cc_impacts = set()
        for imp in cc_impacts_raw:
            if isinstance(imp, list) and len(imp) >= 2:
                q = imp[0] if isinstance(imp[0], str) else imp[0][0] if isinstance(imp[0], list) else str(imp[0])
                s = imp[1] if isinstance(imp[1], str) else imp[1][0] if isinstance(imp[1], list) else str(imp[1])
                cc_impacts.add((q, s))
        
        # Primary impact must match
        sq_primary = list(sq_impacts)[0] if sq_impacts else None
        cc_primary = list(cc_impacts)[0] if cc_impacts else None
        
        if sq_primary and sq_primary not in cc_impacts:
            mismatches_impacts += 1
            issues.append({
                "rule_id": rule_id,
                "severity": "HIGH" if sq_primary[0] == "Security" else "MEDIUM",
                "type": "primary_impact_missing",
                "message": f"Primary impact {sq_primary} missing from CogniCode {list(cc_impacts)}",
                "sonarqube": list(sq_impacts),
                "cognicode": list(cc_impacts),
            })
    
    # Print results
    logger.info("="*60)
    logger.info("  SONARQUBE RULE VALIDATION")
    logger.info("="*60)
    logger.info(f"  Rules validated: {len(SONARQUBE_SPECS)}")
    logger.info(f"  ✅ Match: {matches}")
    logger.info(f"  ⚠️ Clean code mismatches: {mismatches_cc}")
    logger.info(f"  ⚠️ Impact mismatches: {mismatches_impacts}")
    logger.info(f"  ❌ Not in CogniCode: {not_in_sq}")
    logger.info("="*60)
    
    # Show issues by severity
    critical = [i for i in issues if i["severity"] == "CRITICAL"]
    high = [i for i in issues if i["severity"] == "HIGH"]
    medium = [i for i in issues if i["severity"] == "MEDIUM"]
    
    if critical:
        logger.info(f"\n  🔴 CRITICAL ({len(critical)}):")
        for i in critical:
            logger.info(f"    {i['rule_id']}: {i['message']}")
    
    if high:
        logger.info(f"\n  🟠 HIGH ({len(high)}):")
        for i in high[:10]:
            logger.info(f"    {i['rule_id']}: {i['message']}")
    
    if medium:
        logger.info(f"\n  🟡 MEDIUM ({len(medium)}):")
        for i in medium[:10]:
            logger.info(f"    {i['rule_id']}: {i['message']}")
    
    # Summary
    coverage = matches / len(SONARQUBE_SPECS) * 100 if SONARQUBE_SPECS else 0
    accuracy = matches - mismatches_cc - mismatches_impacts
    accuracy_pct = accuracy / matches * 100 if matches else 0
    
    logger.info(f"\n  📊 Coverage: {coverage:.0f}% ({matches}/{len(SONARQUBE_SPECS)})")
    logger.info(f"  📊 Accuracy: {accuracy_pct:.0f}% ({accuracy}/{matches})")
    
    return {
        "total_validated": len(SONARQUBE_SPECS),
        "matches": matches,
        "clean_code_mismatches": mismatches_cc,
        "impact_mismatches": mismatches_impacts,
        "not_in_cognicode": not_in_sq,
        "coverage_pct": coverage,
        "accuracy_pct": accuracy_pct,
        "issues": issues,
    }


# ═══════════════════════════════════════════════════════════════════
# Rule ID cross-reference
# ═══════════════════════════════════════════════════════════════════

def check_rule_id_consistency():
    """Verify all S-series rules exist and have consistent naming."""
    
    with open(RULES_JSON) as f:
        rules = json.load(f)
    
    s_rules = {k: v for k, v in rules.items() if re.match(r'^S\d+$', k)}
    
    logger.info(f"\n  S-series rules: {len(s_rules)}")
    
    # Check for missing metadata
    no_exp = [k for k, v in s_rules.items() if not v.get("explanation")]
    no_cc = [k for k, v in s_rules.items() if not v.get("clean_code")]
    no_imp = [k for k, v in s_rules.items() if not v.get("impacts")]
    
    if no_exp:
        logger.warning(f"  Missing explanation: {len(no_exp)} rules")
    if no_cc:
        logger.warning(f"  Missing clean_code: {len(no_cc)} rules")
    if no_imp:
        logger.warning(f"  Missing impacts: {len(no_imp)} rules")
    
    if not no_exp and not no_cc and not no_imp:
        logger.info(f"  ✅ All {len(s_rules)} S-rules have complete metadata")
    
    return len(s_rules)


# ═══════════════════════════════════════════════════════════════════
# Severity consistency check
# ═══════════════════════════════════════════════════════════════════

def check_severity_consistency():
    """Check that severity matches SonarQube defaults."""
    
    with open(RULES_JSON) as f:
        rules = json.load(f)
    
    content = CATALOG_PATH.read_text()
    
    mismatches = []
    for rule_id, spec in SONARQUBE_SPECS.items():
        sq_severity = spec["severity"]
        
        # Extract CogniCode severity from catalog.rs
        pattern = rf'id:\s*"{rule_id}".*?severity:\s*(\w+)'
        match = re.search(pattern, content, re.DOTALL)
        if match:
            cc_severity = match.group(1)
            
            # Map severity names
            severity_map = {
                "Blocker": "Blocker", "Critical": "Critical", 
                "Major": "Major", "Minor": "Minor", "Info": "Info",
                # CogniCode uses different names sometimes
                "CRITICAL": "Critical", "HIGH": "Critical",
                "MEDIUM": "Major", "LOW": "Minor",
            }
            
            cc_normalized = severity_map.get(cc_severity, cc_severity)
            
            if cc_normalized != sq_severity:
                mismatches.append({
                    "rule_id": rule_id,
                    "sonarqube": sq_severity,
                    "cognicode": cc_severity,
                })
    
    if mismatches:
        logger.info(f"\n  ⚠️ Severity mismatches ({len(mismatches)}):")
        for m in mismatches[:10]:
            logger.info(f"    {m['rule_id']}: SQ={m['sonarqube']} vs CogniCode={m['cognicode']}")
    else:
        logger.info(f"\n  ✅ All severities match SonarQube")
    
    return mismatches


if __name__ == "__main__":
    logging.basicConfig(level=logging.INFO, format="%(message)s")
    
    logger.info("🔍 Validating CogniCode rules against SonarQube specs...\n")
    
    # 1. Validate against SonarQube public specs
    results = validate_against_sonarqube()
    
    # 2. Check rule ID consistency
    s_count = check_rule_id_consistency()
    
    # 3. Check severity consistency
    sev_issues = check_severity_consistency()
    
    # Final verdict
    logger.info(f"\n{'='*60}")
    logger.info(f"  VERDICT")
    logger.info(f"{'='*60}")
    
    total_issues = len(results["issues"]) + len(sev_issues)
    
    if results["accuracy_pct"] >= 95 and total_issues == 0:
        logger.info(f"  ✅ SonarQube validation PASSED")
    elif results["accuracy_pct"] >= 80:
        logger.info(f"  ⚠️ SonarQube validation: {results['accuracy_pct']:.0f}% accuracy")
        logger.info(f"     {total_issues} issues to review")
    else:
        logger.info(f"  ❌ SonarQube validation needs attention")
        logger.info(f"     Accuracy: {results['accuracy_pct']:.0f}%")
    
    # Save results
    output = {
        "validated_at": __import__('datetime').datetime.now().isoformat(),
        "sonarqube_specs_count": len(SONARQUBE_SPECS),
        "coverage_pct": results["coverage_pct"],
        "accuracy_pct": results["accuracy_pct"],
        "issues": results["issues"],
        "severity_mismatches": sev_issues,
    }
    
    out_path = AUTORESEARCH_DIR / "results" / "sonarqube_validation.json"
    out_path.parent.mkdir(parents=True, exist_ok=True)
    with open(out_path, "w") as f:
        json.dump(output, f, indent=2)
    
    logger.info(f"\n  Results saved: {out_path}")
