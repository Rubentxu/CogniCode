#!/usr/bin/env python3
"""Generador de Reporte HTML — Sandbox Smoke Tests de CogniCode.

Uso:
    python3 sandbox/scripts/generate_html_report.py [--results-dir sandbox/results] [--output /tmp/report.html]

Genera un reporte HTML autocontenido con Tailwind CSS y Mermaid.js desde los
resultados del sandbox.
"""

import json
import glob
import os
import re
import sys
import time
from collections import defaultdict
from datetime import datetime
from pathlib import Path


def load_results(results_dir: str) -> list[dict]:
    """Carga todos los result.json del directorio de resultados."""
    results = []
    pattern = os.path.join(results_dir, "*", "*", "result.json")
    for path in glob.glob(pattern):
        try:
            with open(path) as f:
                r = json.load(f)
            results.append(r)
        except Exception:
            pass
    return results


def load_stability(results_dir: str) -> dict | None:
    """Load stability.json if it exists."""
    path = os.path.join(results_dir, "stability.json")
    if os.path.exists(path):
        with open(path) as f:
            return json.load(f)
    return None


def load_summary(results_dir: str) -> dict | None:
    """Carga summary.json si existe."""
    path = os.path.join(results_dir, "summary.json")
    if os.path.exists(path):
        with open(path) as f:
            return json.load(f)
    return None


def compute_stats(results: list[dict]) -> dict:
    """Calcula estadísticas agregadas de los resultados."""
    total = len(results)
    passed = sum(1 for r in results if r.get("outcome") == "pass")
    failed = sum(1 for r in results if r.get("outcome") not in ("pass", "expected_fail", "capability_missing"))
    expected_fail = sum(1 for r in results if r.get("outcome") in ("expected_fail", "capability_missing"))
    pass_rate = (passed + expected_fail) / total * 100 if total > 0 else 0

    # Per-language
    by_lang = defaultdict(lambda: {"total": 0, "passed": 0, "failed": 0, "durations": []})
    for r in results:
        lang = r.get("language", "unknown")
        by_lang[lang]["total"] += 1
        if r.get("outcome") == "pass":
            by_lang[lang]["passed"] += 1
        elif r.get("outcome") not in ("expected_fail", "capability_missing"):
            by_lang[lang]["failed"] += 1
        dur = r.get("timing_ms", {}).get("total_ms", 0)
        if dur:
            by_lang[lang]["durations"].append(dur)

    # Per-tool
    by_tool = defaultdict(lambda: {"total": 0, "passed": 0, "failed": 0, "durations": []})
    for r in results:
        tool = r.get("tool", "unknown")
        by_tool[tool]["total"] += 1
        if r.get("outcome") == "pass":
            by_tool[tool]["passed"] += 1
        elif r.get("outcome") not in ("expected_fail", "capability_missing"):
            by_tool[tool]["failed"] += 1
        dur = r.get("timing_ms", {}).get("total_ms", 0)
        if dur:
            by_tool[tool]["durations"].append(dur)

    # All durations for percentile calculation
    all_durations = [r.get("timing_ms", {}).get("total_ms", 0) for r in results if r.get("timing_ms", {}).get("total_ms", 0)]
    all_durations.sort()

    def percentile(data, p):
        if not data:
            return 0
        idx = int(len(data) * p / 100)
        return data[min(idx, len(data) - 1)]

    # Failure distribution
    failures = defaultdict(int)
    for r in results:
        if r.get("outcome") not in ("pass", "expected_fail", "capability_missing"):
            fc = r.get("failure_class")
            if isinstance(fc, dict):
                key = list(fc.keys())[0] if fc else "unknown"
            else:
                key = str(fc) if fc else r.get("outcome", "unknown")
            failures[key] += 1

    # Dimension scores
    dims = {"correctitud": 0, "latencia": 0, "escalabilidad": 0, "consistencia": 0, "robustez": 0}
    dim_counts = defaultdict(int)
    for r in results:
        for k in dims:
            v = r.get("dimension_scores", {}).get(k)
            if v is not None and v > 0:
                dims[k] += v
                dim_counts[k] += 1
    for k in dims:
        if dim_counts[k] > 0:
            dims[k] = dims[k] / dim_counts[k]

    # MCP health score: weighted combination
    health = min(100, (pass_rate * 0.5 + dims.get("robustez", 95) * 0.3 + dims.get("latencia", 95) * 0.2))

    return {
        "total": total,
        "passed": passed,
        "failed": failed,
        "expected_fail": expected_fail,
        "pass_rate": pass_rate,
        "by_language": dict(by_lang),
        "by_tool": dict(by_tool),
        "p50": percentile(all_durations, 50),
        "p95": percentile(all_durations, 95),
        "p99": percentile(all_durations, 99),
        "dimension_scores": dims,
        "health_score": health,
        "failures": dict(failures),
        "total_duration_ms": sum(all_durations),
        "tool_count": len(by_tool),
        "language_count": len(by_lang),
    }


def _build_coverage_matrix(stats: dict, results: list[dict]) -> str:
    """Build coverage matrix HTML: tool × language with pass/total cells."""
    tools = sorted(stats.get('by_tool', {}).keys())
    langs = sorted(stats.get('by_language', {}).keys())
    if not tools or not langs:
        return '<p class="text-sm text-slate-500">No data available</p>'

    # Pre-compute cells
    cells = {(t, l): {"pass": 0, "total": 0} for t in tools for l in langs}
    for r in results:
        t = r.get('tool')
        l = r.get('language')
        if t in tools and l in langs:
            cells[(t, l)]["total"] += 1
            if r.get('outcome') in ('pass', 'expected_fail', 'capability_missing'):
                cells[(t, l)]["pass"] += 1

    # Header
    html = '<table class="min-w-full text-xs"><thead><tr class="bg-slate-100"><th class="py-2 px-2 text-left font-medium text-slate-600 sticky left-0 bg-slate-100">Tool / Lang</th>'
    for lang in langs:
        html += f'<th class="py-2 px-2 text-center font-medium text-slate-600">{lang}</th>'
    html += '<th class="py-2 px-2 text-center font-medium text-slate-600 bg-slate-200">Total</th></tr></thead><tbody>'

    # Body
    for tool in tools:
        row_pass = 0
        row_total = 0
        html += f'<tr class="border-b border-slate-100"><td class="py-1 px-2 font-mono sticky left-0 bg-white">{tool}</td>'
        for lang in langs:
            c = cells.get((tool, lang), {"pass": 0, "total": 0})
            row_pass += c['pass']
            row_total += c['total']
            if c['total'] == 0:
                html += '<td class="py-1 px-2 text-center text-slate-300">·</td>'
            else:
                rate = c['pass'] / c['total'] * 100
                if rate == 100:
                    color = 'emerald'
                elif rate >= 50:
                    color = 'amber'
                else:
                    color = 'red'
                html += f'<td class="py-1 px-2 text-center text-{color}-600 font-semibold">{c["pass"]}/{c["total"]}</td>'
        # Row total
        if row_total == 0:
            row_color = 'slate'
            row_str = '·'
        else:
            row_rate = row_pass / row_total * 100
            row_color = 'emerald' if row_rate == 100 else ('amber' if row_rate >= 50 else 'red')
            row_str = f'{row_pass}/{row_total}'
        html += f'<td class="py-1 px-2 text-center text-{row_color}-700 font-semibold bg-slate-50">{row_str}</td></tr>'

    html += '</tbody></table>'
    return html


def render_html(stats: dict, results: list[dict], summary: dict | None, results_dir: str) -> str:
    """Renderiza el reporte HTML completo."""
    now = datetime.now().strftime("%Y-%m-%d %H:%M:%S")

    # ── Helper functions ──

    def pct_bar(value, total, color="emerald"):
        if total == 0:
            return ""
        pct = min(100, value / total * 100)
        return f'<div class="w-full bg-slate-200 rounded-full h-2.5"><div class="bg-{color}-500 h-2.5 rounded-full" style="width:{pct:.0f}%"></div></div>'

    def badge(text, color="slate"):
        colors = {
            "emerald": "bg-emerald-100 text-emerald-800",
            "amber": "bg-amber-100 text-amber-800",
            "red": "bg-red-100 text-red-800",
            "slate": "bg-slate-100 text-slate-600",
            "blue": "bg-blue-100 text-blue-800",
            "violet": "bg-violet-100 text-violet-800",
        }
        return f'<span class="px-2 py-1 text-xs font-medium rounded-full {colors.get(color, colors["slate"])}">{text}</span>'

    def kpi_card(title, value, subtitle, color="slate", icon=""):
        return f'''
        <div class="bg-white rounded-xl border border-slate-200 p-5 shadow-sm">
            <div class="flex items-center gap-2 mb-1">
                <span class="text-2xl">{icon}</span>
                <span class="text-xs font-medium text-slate-500 uppercase tracking-wider">{title}</span>
            </div>
            <div class="text-4xl font-bold text-{color}-600 mb-1">{value}</div>
            <div class="text-xs text-slate-400">{subtitle}</div>
        </div>'''

    # ── Language rows ──
    lang_rows = ""
    for lang, data in sorted(stats["by_language"].items(), key=lambda x: x[1]["total"], reverse=True):
        dur = data["durations"]
        p50 = sorted(dur)[len(dur) // 2] if dur else 0
        p95 = sorted(dur)[min(len(dur) - 1, int(len(dur) * 0.95))] if dur else 0
        rate = (data["passed"] / data["total"] * 100) if data["total"] > 0 else 0
        color = "emerald" if rate >= 90 else ("amber" if rate >= 70 else "red")
        lang_rows += f'''
        <tr class="border-b border-slate-100 hover:bg-slate-50">
            <td class="py-3 px-4 font-medium">{lang}</td>
            <td class="py-3 px-4">{data["total"]}</td>
            <td class="py-3 px-4 text-emerald-600">{data["passed"]}</td>
            <td class="py-3 px-4 text-red-600">{data["failed"]}</td>
            <td class="py-3 px-4">
                <div class="flex items-center gap-2">
                    <span class="text-sm font-semibold text-{color}-600">{rate:.0f}%</span>
                    <div class="w-24 bg-slate-200 rounded-full h-2"><div class="bg-{color}-500 h-2 rounded-full" style="width:{rate:.0f}%"></div></div>
                </div>
            </td>
            <td class="py-3 px-4 text-sm text-slate-500">{p50}ms</td>
            <td class="py-3 px-4 text-sm text-slate-500">{p95}ms</td>
        </tr>'''

    # ── Tool rows ──
    tool_rows = ""
    for tool, data in sorted(stats["by_tool"].items(), key=lambda x: x[1]["total"], reverse=True):
        dur = data["durations"]
        p50 = sorted(dur)[len(dur) // 2] if dur else 0
        p95 = sorted(dur)[min(len(dur) - 1, int(len(dur) * 0.95))] if dur else 0
        rate = (data["passed"] / data["total"] * 100) if data["total"] > 0 else 0
        color = "emerald" if rate >= 90 else ("amber" if rate >= 70 else "red")
        tool_rows += f'''
        <tr class="border-b border-slate-100 hover:bg-slate-50">
            <td class="py-2 px-4 font-mono text-sm">{tool}</td>
            <td class="py-2 px-4">{data["total"]}</td>
            <td class="py-2 px-4 text-emerald-600">{data["passed"]}</td>
            <td class="py-2 px-4 text-red-600">{data["failed"]}</td>
            <td class="py-2 px-4">
                <div class="flex items-center gap-2">
                    <span class="text-xs font-semibold text-{color}-600">{rate:.0f}%</span>
                    <div class="w-16 bg-slate-200 rounded-full h-1.5"><div class="bg-{color}-500 h-1.5 rounded-full" style="width:{rate:.0f}%"></div></div>
                </div>
            </td>
            <td class="py-2 px-4 text-xs text-slate-500">{p50}ms</td>
            <td class="py-2 px-4 text-xs text-slate-500">{p95}ms</td>
        </tr>'''

    # ── Coverage Matrix (tool × language) ──
    coverage_matrix_html = _build_coverage_matrix(stats, results)
        # ── Failure rows ──
    failure_rows = ""
    for name, count in sorted(stats["failures"].items(), key=lambda x: x[1], reverse=True):
        failure_rows += f'''
        <tr class="border-b border-slate-100">
            <td class="py-2 px-4 font-medium text-red-700">{name}</td>
            <td class="py-2 px-4">{count}</td>
            <td class="py-2 px-4 text-sm text-slate-500">—</td>
        </tr>'''

    # ── Stability section ──
    # For repeat-run directories (run-N), look in the parent for stability.json
    _parent_for_stability = results_dir
    if re.search(r"/run-\d+$", results_dir):
        _parent_for_stability = str(Path(results_dir).parent)
    stability = load_stability(_parent_for_stability)
    stability_html = ""
    if stability and stability.get("repeat_count", 0) >= 2:
        flk = stability.get("flaky_scenarios", [])
        top_flaky = stability.get("top_flaky_scenarios", [])
        top_cv = stability.get("top_high_variance_scenarios", [])
        by_lang = stability.get("by_language", {})

        # Flaky rows
        flaky_rows = ""
        for s in top_flaky:
            rate = s.get("pass_rate", 0)
            runs = s.get("runs", 0)
            lang = s.get("language", "")
            color = "red" if rate < 0.5 else ("amber" if rate < 0.9 else "emerald")
            flaky_rows += f'''
            <tr class="border-b border-slate-100">
                <td class="py-2 px-4 font-mono text-xs">{s.get("scenario_id", "unknown")}</td>
                <td class="py-2 px-4 text-sm">{lang}</td>
                <td class="py-2 px-4">{runs}</td>
                <td class="py-2 px-4 text-{color}-600 font-semibold">{rate*100:.0f}%</td>
                <td class="py-2 px-4 text-sm text-slate-500">
                    {", ".join(f"{k}: {v}" for k, v in (s.get("outcome_distribution") or {}).items())}
                </td>
            </tr>'''

        # CV rows
        cv_rows = ""
        for item in top_cv:
            cv_val = item.get("cv", 0)
            sid = item.get("scenario_id", "unknown")
            # Find full stats
            full = next((s for s in stability.get("scenario_stats", []) if s["scenario_id"] == sid), {})
            timing = full.get("timing", {}) if full else {}
            mean = timing.get("mean", 0)
            std = timing.get("std_dev", 0)
            cv_rows += f'''
            <tr class="border-b border-slate-100">
                <td class="py-2 px-4 font-mono text-xs">{sid}</td>
                <td class="py-2 px-4 text-sm">{full.get("language", "")}</td>
                <td class="py-2 px-4 text-red-600 font-semibold">{cv_val*100:.1f}%</td>
                <td class="py-2 px-4 text-sm text-slate-500">{mean:.0f}ms</td>
                <td class="py-2 px-4 text-sm text-slate-500">±{std:.0f}ms</td>
            </tr>'''

        # Language stability rows
        lang_rows = ""
        for lang, data in sorted(by_lang.items(), key=lambda x: x[1]["flaky"], reverse=True):
            flk_c = data["flaky"]
            total = data["total"]
            rate = data.get("flaky_rate", 0)
            color = "red" if rate > 20 else ("amber" if rate > 5 else "emerald")
            lang_rows += f'''
            <tr class="border-b border-slate-100 hover:bg-slate-50">
                <td class="py-2 px-4 font-medium">{lang}</td>
                <td class="py-2 px-4">{total}</td>
                <td class="py-2 px-4 text-red-600">{flk_c}</td>
                <td class="py-2 px-4 text-{color}-600 font-semibold">{rate:.1f}%</td>
                <td class="py-2 px-4 text-sm text-slate-500">{total - flk_c} stable</td>
            </tr>'''

        stability_html = f'''
        <!-- Stability Analysis -->
        <section>
            <h2 class="text-lg font-semibold text-slate-700 mb-4">🔁 Stability Analysis <span class="text-xs font-normal text-slate-400">({stability["repeat_count"]} repeats)</span></h2>
            <div class="grid grid-cols-2 md:grid-cols-4 gap-4 mb-6">
                {kpi_card("Total Escenarios", str(stability["total_scenarios"]), f"{stability['total_runs']} total runs", "slate", "📋")}
                {kpi_card("Flaky", f"{stability["flaky_count"]}", f"{stability["flaky_percentage"]:.1f}% of scenarios", "red" if stability["flaky_percentage"] > 10 else "amber", "🔀")}
                {kpi_card("Stable", f"{stability["stable_count"]}", f"{100 - stability["flaky_percentage"]:.1f}% of scenarios", "emerald", "✅")}
                {kpi_card("Pass Rate", f"{stability["pass_rate"]:.1f}%", "Across all repeats", "violet", "🎯")}
            </div>

            <!-- Top Flakiest Scenarios -->
            <div class="bg-white rounded-xl border border-slate-200 overflow-hidden shadow-sm mb-6">
                <div class="px-4 py-3 bg-red-50 border-b border-slate-200">
                    <h3 class="text-sm font-semibold text-red-700">⚠️ Top Flakiest Scenarios</h3>
                </div>
                <table class="w-full text-sm">
                    <thead class="bg-slate-50">
                        <tr>
                            <th class="py-2 px-4 text-left font-medium text-slate-600">Scenario</th>
                            <th class="py-2 px-4 text-left font-medium text-slate-600">Lang</th>
                            <th class="py-2 px-4 text-left font-medium text-slate-600">Runs</th>
                            <th class="py-2 px-4 text-left font-medium text-slate-600">Pass Rate</th>
                            <th class="py-2 px-4 text-left font-medium text-slate-600">Outcomes</th>
                        </tr>
                    </thead>
                    <tbody>{flaky_rows if flaky_rows else "<tr><td colspan='5' class='py-3 px-4 text-sm text-slate-400'>No flaky scenarios detected</td></tr>"}</tbody>
                </table>
            </div>

            <!-- Top High-Variance Scenarios -->
            <div class="bg-white rounded-xl border border-slate-200 overflow-hidden shadow-sm mb-6">
                <div class="px-4 py-3 bg-amber-50 border-b border-slate-200">
                    <h3 class="text-sm font-semibold text-amber-700">📊 Top High-Variance Scenarios (by CV)</h3>
                </div>
                <table class="w-full text-sm">
                    <thead class="bg-slate-50">
                        <tr>
                            <th class="py-2 px-4 text-left font-medium text-slate-600">Scenario</th>
                            <th class="py-2 px-4 text-left font-medium text-slate-600">Lang</th>
                            <th class="py-2 px-4 text-left font-medium text-slate-600">CV</th>
                            <th class="py-2 px-4 text-left font-medium text-slate-600">Mean</th>
                            <th class="py-2 px-4 text-left font-medium text-slate-600">Std Dev</th>
                        </tr>
                    </thead>
                    <tbody>{cv_rows if cv_rows else "<tr><td colspan='5' class='py-3 px-4 text-sm text-slate-400'>No high-variance scenarios</td></tr>"}</tbody>
                </table>
            </div>

            <!-- Per-Language Stability -->
            <div class="bg-white rounded-xl border border-slate-200 overflow-hidden shadow-sm">
                <div class="px-4 py-3 bg-slate-100 border-b border-slate-200">
                    <h3 class="text-sm font-semibold text-slate-700">🌍 Per-Language Stability Summary</h3>
                </div>
                <table class="w-full text-sm">
                    <thead class="bg-slate-50">
                        <tr>
                            <th class="py-2 px-4 text-left font-medium text-slate-600">Language</th>
                            <th class="py-2 px-4 text-left font-medium text-slate-600">Total</th>
                            <th class="py-2 px-4 text-left font-medium text-slate-600">Flaky</th>
                            <th class="py-2 px-4 text-left font-medium text-slate-600">Flaky Rate</th>
                            <th class="py-2 px-4 text-left font-medium text-slate-600">Stable</th>
                        </tr>
                    </thead>
                    <tbody>{lang_rows if lang_rows else "<tr><td colspan='5' class='py-3 px-4 text-sm text-slate-400'>No language data</td></tr>"}</tbody>
                </table>
            </div>
        </section>'''

    # ── Dimension gauges ──
    dim_colors = {"correctitud": "violet", "latencia": "emerald", "escalabilidad": "blue", "consistencia": "amber", "robustez": "red"}
    dim_gauges = ""
    for name, label in [("correctitud", "CORR"), ("latencia", "LAT"), ("escalabilidad", "ESC"), ("consistencia", "CON"), ("robustez", "ROB")]:
        v = stats["dimension_scores"].get(name, 0)
        c = dim_colors.get(name, "slate")
        dim_gauges += f'''
        <div class="text-center">
            <div class="relative w-16 h-16 mx-auto">
                <svg class="w-16 h-16 transform -rotate-90">
                    <circle cx="32" cy="32" r="28" fill="none" stroke="#e2e8f0" stroke-width="6"/>
                    <circle cx="32" cy="32" r="28" fill="none" stroke="currentColor" stroke-width="6"
                            stroke-dasharray="{v*1.76:.0f} 176" class="text-{c}-500"/>
                </svg>
                <span class="absolute inset-0 flex items-center justify-center text-sm font-bold text-{c}-600">{v:.0f}</span>
            </div>
            <span class="text-xs text-slate-500 mt-1 block">{label}</span>
        </div>'''

    # ── Mermaid latency chart ──
    mermaid_chart = f'''flowchart LR
    subgraph "Distribución de Latencia"
        p50["p50<br/>{stats['p50']}ms"] --> p95["p95<br/>{stats['p95']}ms"] --> p99["p99<br/>{stats['p99']}ms"]
    end
    classDef fast fill:#059669,color:#fff,stroke:#047857
    classDef mid fill:#d97706,color:#fff,stroke:#b45309
    classDef slow fill:#dc2626,color:#fff,stroke:#b91c1c
    class p50 fast
    class p95 mid
    class p99 slow'''

    # ── Full HTML ──
    return f'''<!doctype html>
<html lang="es">
<head>
    <meta charset="utf-8"/>
    <title>CogniCode — Reporte de Smoke Tests</title>
    <script src="https://cdn.tailwindcss.com"></script>
    <script type="module">
        import mermaid from "https://cdn.jsdelivr.net/npm/mermaid@11/dist/mermaid.esm.min.mjs";
        mermaid.initialize({{ startOnLoad: true, theme: "neutral", securityLevel: "loose" }});
    </script>
</head>
<body class="bg-stone-50 text-slate-900 font-sans">
    <main class="max-w-6xl mx-auto px-6 py-8 space-y-8">

        <!-- Header -->
        <header class="space-y-2">
            <div class="flex items-center justify-between">
                <div>
                    <h1 class="text-3xl font-bold tracking-tight text-slate-900">🧠 CogniCode</h1>
                    <p class="text-lg text-slate-500">Reporte de Smoke Tests — Sandbox Validation</p>
                </div>
                <div class="text-right text-sm text-slate-400">
                    <div>{now}</div>
                    <div>v{summary.get("orchestrator_version", "0.5.0") if summary else "0.5.0"}</div>
                </div>
            </div>
            <div class="h-1 bg-gradient-to-r from-emerald-400 via-violet-400 to-amber-400 rounded-full"></div>
        </header>

        <!-- KPI Cards -->
        <section>
            <h2 class="text-lg font-semibold text-slate-700 mb-4">📊 Indicadores Clave</h2>
            <div class="grid grid-cols-2 md:grid-cols-4 lg:grid-cols-6 gap-4">
                {kpi_card("Pass Rate", f"{stats['pass_rate']:.1f}%", f"{stats['passed']}/{stats['total']} escenarios", "emerald", "✅")}
                {kpi_card("Health", f"{stats['health_score']:.0f}/100", "MCP Health Score", "violet", "💚")}
                {kpi_card("p50 Latencia", f"{stats['p50']}ms", "Mediana", "blue", "⚡")}
                {kpi_card("p95 Latencia", f"{stats['p95']}ms", "Percentil 95", "amber", "⏱")}
                {kpi_card("Tools", str(stats['tool_count']), "Herramientas MCP", "slate", "🔧")}
                {kpi_card("Lenguajes", str(stats['language_count']), "Soportados", "slate", "🌐")}
            </div>
        </section>

        <!-- Dimension Scores -->
        <section>
            <h2 class="text-lg font-semibold text-slate-700 mb-4">🎯 Dimensiones de Calidad</h2>
            <div class="bg-white rounded-xl border border-slate-200 p-6 shadow-sm">
                <div class="flex justify-around">{dim_gauges}</div>
                <div class="mt-4 text-xs text-center text-slate-400">
                    CORR=Correctitud · LAT=Latencia · ESC=Escalabilidad · CON=Consistencia · ROB=Robustez
                </div>
            </div>
        </section>

        <!-- Language Breakdown -->
        <section>
            <h2 class="text-lg font-semibold text-slate-700 mb-4">🌍 Desglose por Lenguaje</h2>
            <div class="bg-white rounded-xl border border-slate-200 overflow-hidden shadow-sm">
                <table class="w-full text-sm">
                    <thead class="bg-slate-100">
                        <tr>
                            <th class="py-3 px-4 text-left font-medium text-slate-600">Lenguaje</th>
                            <th class="py-3 px-4 text-left font-medium text-slate-600">Total</th>
                            <th class="py-3 px-4 text-left font-medium text-slate-600">Pass</th>
                            <th class="py-3 px-4 text-left font-medium text-slate-600">Fail</th>
                            <th class="py-3 px-4 text-left font-medium text-slate-600">Pass Rate</th>
                            <th class="py-3 px-4 text-left font-medium text-slate-600">p50</th>
                            <th class="py-3 px-4 text-left font-medium text-slate-600">p95</th>
                        </tr>
                    </thead>
                    <tbody>{lang_rows}</tbody>
                </table>
            </div>
        </section>

        <!-- Tool Breakdown -->
        <section>
            <h2 class="text-lg font-semibold text-slate-700 mb-4">🔧 Desglose por Herramienta</h2>
            <div class="bg-white rounded-xl border border-slate-200 overflow-hidden shadow-sm max-h-96 overflow-y-auto">
                <table class="w-full text-sm">
                    <thead class="bg-slate-100 sticky top-0">
                        <tr>
                            <th class="py-2 px-4 text-left font-medium text-slate-600">Tool</th>
                            <th class="py-2 px-4 text-left font-medium text-slate-600">Total</th>
                            <th class="py-2 px-4 text-left font-medium text-slate-600">Pass</th>
                            <th class="py-2 px-4 text-left font-medium text-slate-600">Fail</th>
                            <th class="py-2 px-4 text-left font-medium text-slate-600">Rate</th>
                            <th class="py-2 px-4 text-left font-medium text-slate-600">p50</th>
                            <th class="py-2 px-4 text-left font-medium text-slate-600">p95</th>
                        </tr>
                    </thead>
                    <tbody>{tool_rows}</tbody>
                </table>
            </div>
        </section>

        <!-- Latency Distribution Chart -->
        <section>
            <h2 class="text-lg font-semibold text-slate-700 mb-4">📈 Distribución de Latencia</h2>
            <div class="bg-white rounded-xl border border-slate-200 p-6 shadow-sm">
                <pre class="mermaid">{mermaid_chart}</pre>
            </div>
        <!-- Coverage Matrix -->
        <section>
            <h2 class="text-lg font-semibold text-slate-700 mb-4">🧮 Coverage Matrix (tool × language)</h2>
            <div class="bg-white rounded-xl border border-slate-200 p-6 shadow-sm overflow-x-auto">
                <p class="text-xs text-slate-500 mb-3">Pass/total per (tool × language) — includes expected_fail and capability_missing.</p>
                {coverage_matrix_html}
            </div>
        </section>

        <!-- Failure Analysis -->
        <section>
            <h2 class="text-lg font-semibold text-slate-700 mb-4">⚠️ Análisis de Fallos</h2>
            <div class="bg-white rounded-xl border border-slate-200 overflow-hidden shadow-sm">
                <table class="w-full text-sm">
                    <thead class="bg-slate-100">
                        <tr>
                            <th class="py-2 px-4 text-left font-medium text-slate-600">Clase de Fallo</th>
                            <th class="py-2 px-4 text-left font-medium text-slate-600">Cantidad</th>
                            <th class="py-2 px-4 text-left font-medium text-slate-600">Root Cause</th>
                        </tr>
                    </thead>
                    <tbody>{failure_rows}</tbody>
                </table>
            </div>
            {f'<div class="mt-2 text-sm text-slate-500">Total fallos: {stats["failed"]} de {stats["total"]} escenarios</div>' if stats["failed"] > 0 else '<div class="mt-2 text-sm text-emerald-600 font-medium">✨ Ningún fallo real — todos los escenarios pasan o son expected_fail</div>'}
        </section>

        {stability_html}

        <!-- Legend & Footer -->
        <footer class="text-center text-xs text-slate-400 py-4 border-t border-slate-200">
            <p>CogniCode Sandbox Orchestrator · Reporte generado automáticamente · {now}</p>
            <p class="mt-1">expected_fail = escenario marcado como fallo esperado (feature no implementada) · capability_missing = tool no registrada</p>
        </footer>

    </main>
</body>
</html>'''


def main():
    results_dir = "sandbox/results"
    output_path = None

    args = sys.argv[1:]
    i = 0
    while i < len(args):
        if args[i] == "--results-dir" and i + 1 < len(args):
            results_dir = args[i + 1]
            i += 2
        elif args[i] == "--output" and i + 1 < len(args):
            output_path = args[i + 1]
            i += 2
        else:
            i += 1

    print(f"📂 Cargando resultados desde: {results_dir}")
    results = load_results(results_dir)
    print(f"   {len(results)} escenarios encontrados")

    if not results:
        print("   ❌ No hay resultados. Ejecuta el sandbox primero.")
        sys.exit(1)

    summary = load_summary(results_dir)
    stats = compute_stats(results)

    print(f"   ✅ Pass: {stats['passed']} | ❌ Fail: {stats['failed']} | ⏭ Expected: {stats['expected_fail']}")
    print(f"   📊 Pass Rate: {stats['pass_rate']:.1f}% | 🏥 Health: {stats['health_score']:.0f}/100")
    print(f"   ⚡ Latencia: p50={stats['p50']}ms p95={stats['p95']}ms p99={stats['p99']}ms")

    html = render_html(stats, results, summary, results_dir)

    if not output_path:
        ts = datetime.now().strftime("%Y%m%d-%H%M%S")
        output_path = f"/tmp/cognicode-smoke-report-{ts}.html"

    with open(output_path, "w") as f:
        f.write(html)

    print(f"\n📄 Reporte generado: {output_path}")
    print(f"   Abrir con: xdg-open {output_path}")


if __name__ == "__main__":
    main()
