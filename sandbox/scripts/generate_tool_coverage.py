#!/usr/bin/env python3
"""
Generate G2 coverage matrix: cross-reference runtime tools/list against manifest tool: fields.
Exit codes: 0 = 100% coverage, 1 = gaps exist, 2 = error
"""
import argparse
import json
import sys
import yaml
from pathlib import Path
from typing import Dict, Set, List, Any


def load_tools_from_json(json_path: str) -> Set[str]:
    """Load tool names from the tools/list JSON output."""
    if json_path == "-":
        tools_data = json.load(sys.stdin)
    else:
        with open(json_path, "r") as f:
            tools_data = json.load(f)
    
    # Handle both array format and {tools: [...], total: N} format
    if isinstance(tools_data, list):
        return {t.get("name") if isinstance(t, dict) else t for t in tools_data}
    elif isinstance(tools_data, dict):
        if "tools" in tools_data:
            return {t.get("name") if isinstance(t, dict) else t for t in tools_data["tools"]}
        # Single tool name as string in dict value
        return set(tools_data.keys())
    else:
        raise ValueError(f"Unexpected tools format: {type(tools_data)}")


def load_manifest_tools(manifests_dir: str) -> Set[str]:
    """Extract unique tool: values from all manifest YAML files."""
    tools = set()
    manifests_path = Path(manifests_dir)
    
    if not manifests_path.exists():
        return tools
    
    for yaml_file in manifests_path.glob("*.yaml"):
        try:
            with open(yaml_file, "r") as f:
                data = yaml.safe_load(f)
                if data is None:
                    continue
                
                # Handle both single scenario and list of scenarios
                scenarios = data.get("scenarios", []) or data.get("scenario_defs", []) or []
                if isinstance(data, dict) and "tool" in data:
                    # Single scenario dict
                    if data.get("tool"):
                        tools.add(data["tool"])
                    scenarios = [data]
                
                for scenario in scenarios:
                    if isinstance(scenario, dict):
                        tool_name = scenario.get("tool")
                        if tool_name:
                            tools.add(tool_name)
        except Exception as e:
            print(f"WARNING: could not parse {yaml_file}: {e}", file=sys.stderr)
    
    return tools




def load_scenario_tools(scenarios_dir: str) -> Set[str]:
    """Extract tool references from scenario YAML files (tool: / tools: fields)."""
    tools = set()
    sp = Path(scenarios_dir)
    if not sp.exists():
        return tools
    for yaml_file in sp.glob("*.yaml"):
        try:
            with open(yaml_file, "r") as f:
                data = yaml.safe_load(f)
            if data is None:
                continue
            # walk nested dicts looking for tool/tools keys
            stack = [data]
            while stack:
                node = stack.pop()
                if isinstance(node, dict):
                    for k, v in node.items():
                        if k in ("tool", "tools", "tool_id") and isinstance(v, str):
                            tools.add(v)
                        elif isinstance(v, (dict, list)):
                            stack.append(v)
                elif isinstance(node, list):
                    stack.extend(x for x in node if isinstance(x, (dict, list)))
        except Exception as e:
            print(f"WARNING: could not parse scenario {yaml_file}: {e}", file=sys.stderr)
    return tools

def generate_coverage_matrix(
    runtime_tools: Set[str],
    manifest_tools: Set[str]
) -> Dict[str, Any]:
    """Generate coverage matrix showing which runtime tools have manifest coverage."""
    covered = runtime_tools & manifest_tools
    uncovered = runtime_tools - manifest_tools
    
    # Tool families for organization
    families = {
        "workspace": ["explorer_open_workspace"],
        "brain": ["brain_open", "brain_close", "brain_status", "brain_focus", "brain_attach", "brain_ask", "brain_add_space", "brain_remove_space", "brain_spaces"],
        "search": ["explorer_spotter_search", "explorer_query_moldql"],
        "inspection": ["explorer_inspect_object", "explorer_get_views", "explorer_get_view", "explorer_get_lenses", "explorer_apply_lens"],
        "graph_traversal": ["graph_subgraph", "graph_cluster", "graph_explain"],
        "impact": ["impact_radius", "impact_forward_radius", "impact_has_path", "impact_shortest_path", "impact_detect_cycles", "impact_component"],
        "analytics": ["graph_pagerank", "graph_god_nodes", "graph_communities", "graph_community_god_nodes", "graph_surprising_connections", "graph_transitive_reduction", "graph_feedback_arc_set", "graph_all_simple_paths"],
        "architecture": ["detect_architecture_drift"],
        "multimodal": ["docs_ingest", "issues_ingest", "graph_search"],
        "views": ["view_save", "view_load", "view_list", "view_delete"],
    }
    
    matrix = {
        "summary": {
            "total_tools": len(runtime_tools),
            "covered": len(covered),
            "uncovered_count": len(uncovered),
            "coverage_percent": round(len(covered) / len(runtime_tools) * 100, 1) if runtime_tools else 0,
        },
        "covered_tools": sorted(covered),
        "uncovered_tools": sorted(uncovered),
        "tool_scenarios": {},
    }
    
    # Add covered/uncovered per tool
    for tool in sorted(runtime_tools):
        matrix["tool_scenarios"][tool] = {
            "covered": tool in covered,
            "scenarios": []  # Would be populated from manifests in full implementation
        }
    
    return matrix


def print_markdown_table(matrix: Dict[str, Any]) -> None:
    """Print human-readable markdown coverage table."""
    total = matrix["summary"]["total_tools"]
    covered = matrix["summary"]["covered"]
    uncovered = matrix["summary"]["uncovered_count"]
    pct = matrix["summary"]["coverage_percent"]
    
    print(f"\n## G2 Tool Coverage: {covered}/{total} ({pct}%)")
    print()
    print(f"| Status | Covered | Total | Gaps |")
    print(f"|--------|---------|-------|------|")
    status = "✓ GREEN" if uncovered == 0 else "✗ RED"
    print(f"| {status} | {covered} | {total} | {uncovered} |")
    print()
    
    if matrix["summary"]["uncovered_count"] > 0:
        print("### Uncovered Tools")
        for tool in matrix["uncovered_tools"]:
            print(f"- `{tool}`")
        print()


def main() -> int:
    parser = argparse.ArgumentParser(description="Generate G2 coverage matrix")
    parser.add_argument("--tools", required=True, help="Path to tools/list JSON (or - for stdin)")
    parser.add_argument("--manifests-dir", required=True, help="Directory containing manifest YAML files")
    parser.add_argument("--scenarios-dir", required=True, help="Directory containing scenario YAML files")
    parser.add_argument("--output", help="Output file for YAML coverage matrix")
    args = parser.parse_args()
    
    try:
        # Load runtime tools
        runtime_tools = load_tools_from_json(args.tools)
        if not runtime_tools:
            print("ERROR: no tools found in input", file=sys.stderr)
            return 2
        
        # Load manifest tools
        manifest_tools = load_manifest_tools(args.manifests_dir)
        scenario_tools = load_scenario_tools(args.scenarios_dir)
        manifest_tools |= scenario_tools
        
        # Generate coverage matrix
        matrix = generate_coverage_matrix(runtime_tools, manifest_tools)
        
        # Output YAML if requested
        if args.output:
            output_path = Path(args.output)
            output_path.parent.mkdir(parents=True, exist_ok=True)
            with open(output_path, "w") as f:
                yaml.dump(matrix, f, default_flow_style=False, sort_keys=False)
        
        # Print markdown summary
        print_markdown_table(matrix)
        
        # Exit code: 0 = 100%, 1 = gaps, 2 = error
        if matrix["summary"]["uncovered_count"] == 0:
            return 0
        else:
            print(f"\nExit: 1 (gaps exist)", file=sys.stderr)
            return 1
            
    except Exception as e:
        print(f"ERROR: {e}", file=sys.stderr)
        import traceback
        traceback.print_exc()
        return 2


if __name__ == "__main__":
    sys.exit(main())
