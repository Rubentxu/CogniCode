#!/usr/bin/env python3
"""F3: Enrich rule metadata with secondary impacts and cross-language consistency."""

import json
import re
from pathlib import Path
from collections import defaultdict, Counter

def main():
    metadata_path = Path("rules_metadata_all.json")
    with open(metadata_path) as f:
        meta = json.load(f)

    stats = {"secondary_added": 0, "cc_unified": 0, "cc_conflicts": [], "no_change": 0}

    # ── Phase 1: Cross-language clean_code unification ──
    groups = defaultdict(list)
    for rid in meta:
        base = re.sub(r'^(JS_|PY_|JAVA_|GO_|TS_)?', '', rid)
        groups[base].append(rid)

    for base, rids in groups.items():
        if len(rids) < 2:
            continue
        ccs = [meta[r].get('clean_code') for r in rids if meta[r].get('clean_code')]
        if len(set(ccs)) <= 1:
            continue
        # Find majority
        counter = Counter(ccs)
        majority_cc, majority_count = counter.most_common(1)[0]
        if majority_count >= 2 and majority_count >= len(ccs) * 0.5:
            for rid in rids:
                if meta[rid].get('clean_code') != majority_cc:
                    old = meta[rid].get('clean_code')
                    meta[rid]['clean_code'] = majority_cc
                    stats["cc_unified"] += 1
                    print(f"  UNIFY {rid}: cc {old} → {majority_cc} (group {base})")
        else:
            stats["cc_conflicts"].append((base, dict(counter)))

    # ── Phase 2: Add secondary impacts ──
    # Complexity rules that should also impact Reliability
    complexity_rules = {'S134', 'S138', 'S3776', 'S1541', 'S1067', 'S1141',
                        'JS_S134', 'JS_S138', 'JS_S3776', 'JS_S1541',
                        'PY_S134', 'PY_S138', 'PY_S3776', 'PY_S1541',
                        'JAVA_S134', 'JAVA_S138', 'JAVA_S3776', 'JAVA_S1541',
                        'GO_S134', 'GO_S138', 'GO_S3776', 'GO_S1541',
                        'S1066', 'S1142', 'S1143', 'S1144', 'S1145',
                        'S1186', 'S1192', 'S1226', 'S1125', 'S1126'}

    for rid, data in meta.items():
        impacts = data.get('impacts', [])
        if not impacts:
            continue
        
        # Normalize impacts format
        normalized = []
        has_security = False
        has_reliability = False
        has_maintainability = False
        for imp in impacts:
            if isinstance(imp, list) and len(imp) >= 2:
                q = imp[0] if not isinstance(imp[0], list) else imp[0][0]
                s = imp[1] if not isinstance(imp[1], list) else imp[1][0] if len(imp) >= 2 else '?'
                normalized.append({'quality': q, 'severity': s})
                if 'Security' in str(q): has_security = True
                if 'Reliability' in str(q): has_reliability = True
                if 'Maintainability' in str(q): has_maintainability = True
        
        if not normalized:
            continue
        
        added = False
        
        # Rule 1: Security → also Reliability
        if has_security and not has_reliability:
            normalized.append({'quality': 'Reliability', 'severity': 'Medium'})
            has_reliability = True
            added = True
        
        # Rule 2: Reliability → also Maintainability
        if has_reliability and not has_maintainability:
            normalized.append({'quality': 'Maintainability', 'severity': 'Low'})
            has_maintainability = True
            added = True
        
        # Rule 3: Complexity rules → also Reliability
        if rid in complexity_rules and not has_reliability:
            normalized.append({'quality': 'Reliability', 'severity': 'Low'})
            has_reliability = True
            added = True
            # And complexity also affects Maintainability
            if not has_maintainability:
                normalized.append({'quality': 'Maintainability', 'severity': 'Medium'})
                has_maintainability = True
        
        if added:
            # Convert back to storage format
            meta[rid]['impacts'] = [[imp['quality'], imp['severity']] for imp in normalized]
            stats["secondary_added"] += 1
            if stats["secondary_added"] <= 20:
                print(f"  ENRICH {rid}: +secondary impacts → {meta[rid]['impacts']}")

    # ── Phase 3: Save enriched metadata ──
    with open(metadata_path, "w") as f:
        json.dump(meta, f, indent=2)

    # Print stats
    print()
    print(f"=== F3 Enrichment Results ===")
    print(f"Secondary impacts added: {stats['secondary_added']}")
    print(f"Clean code unified:       {stats['cc_unified']}")
    print(f"CC conflicts (tied):      {len(stats['cc_conflicts'])}")
    for base, counter in stats['cc_conflicts'][:10]:
        print(f"  {base}: {dict(counter)}")

    # New distribution
    multi = sum(1 for d in meta.values() if len(d.get('impacts', [])) > 1)
    print(f"Rules with multiple impacts: {multi} (was 1)")

if __name__ == "__main__":
    main()