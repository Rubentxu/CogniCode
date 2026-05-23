# Batch Initialization Template

Template para crear un nuevo batch de reglas en Engram.

## Estructura de Directorios

```
engram/rules/{batch-name}/
├── state.json              # Estado operacional (requerido)
├── knowledge-research.json  # Investigación de fuentes
├── concepts.json          # Conceptos normalizados
├── legal-review.json       # Auditoría legal
├── rule-designs.json      # Diseños técnicos
├── fixture-matrix.json     # Matriz de tests
├── tasks.json             # Tareas de implementación
├── apply-progress.json    # Progreso de implementación
├── test-report.json       # Reporte de tests
├── benchmark-report.json  # Reporte de benchmark
├── review-report.json     # Reporte de review
├── commit-plan.json       # Plan de commits
├── archive-report.json     # Reporte de archivo
└── entropy-report.json    # Métricas entrópicas
```

## Template: state.json Inicial

```json
{
  "batch": "{batch-name}",
  "phase": "initialized",
  "started_at": "{ISO-8601-timestamp}",
  "updated_at": "{ISO-8601-timestamp}",
  "description": "{descripción del batch}",
  "concepts": [],
  "transitions": [
    {
      "from": "initialized",
      "to": "initialized",
      "timestamp": "{ISO-8601-timestamp}",
      "agent": "rule-orchestrator",
      "gate": "initial_state",
      "reason": "Batch initialized"
    }
  ],
  "metrics": {
    "total_concepts": 0,
    "discovered": 0,
    "normalized": 0,
    "duplicates": 0,
    "legal_blocked": 0,
    "candidates": 0,
    "designed": 0,
    "implemented": 0,
    "tested": 0,
    "benchmarked": 0,
    "approved": 0,
    "published": 0,
    "implementation_failed": 0,
    "test_failed": 0,
    "benchmark_failed": 0,
    "review_failed": 0,
    "median_file_ms": 0,
    "false_positive_rate_estimate": 0,
    "cwe_coverage_added": []
  },
  "gates": {
    "research_gate": "pending",
    "design_gate": "pending",
    "fixture_gate": "pending",
    "implementation_gate": "pending",
    "benchmark_gate": "pending",
    "review_gate": "pending",
    "commit_gate": "pending",
    "archive_gate": "pending"
  },
  "artifacts_created": [],
  "entropy_metrics": {
    "design_entropy": null,
    "implementation_entropy": null,
    "test_entropy": null,
    "overall_entropy_score": null
  }
}
```

## Template: knowledge-research.json Inicial

```json
{
  "batch": "{batch-name}",
  "phase": "research",
  "research_started_at": "{ISO-8601-timestamp}",
  "sources": [],
  "candidates_raw": [],
  "provenance": []
}
```

## Template: concepts.json Inicial

```json
{
  "batch": "{batch-name}",
  "phase": "normalization",
  "normalization_started_at": "{ISO-8601-timestamp}",
  "concepts": []
}
```

## Template: legal-review.json Inicial

```json
{
  "batch": "{batch-name}",
  "phase": "legal_review",
  "legal_review_started_at": "{ISO-8601-timestamp}",
  "candidates": []
}
```

## Template: rule-designs.json Inicial

```json
{
  "batch": "{batch-name}",
  "phase": "design",
  "design_started_at": "{ISO-8601-timestamp}",
  "designs": []
}
```

## Estados del Ciclo de Vida

```
initialized
  ↓
discovered → normalized → duplicate | legal_blocked | reference_only | candidate
                                                              ↓
                                                    designed → fixtures_ready
                                                              ↓
                                                    implementing → implemented | implementation_failed
                                                              ↓
                                                    testing → tested | test_failed
                                                              ↓
                                                    benchmarking → benchmarked | benchmark_failed
                                                              ↓
                                                    reviewing → approved | review_failed
                                                              ↓
                                                    committed → published
                                                              ↓
                                                    feedback_open | deprecated → archived
```

## Fases del Workflow

| Comando | Fases |
|---------|-------|
| `/rules-new` | research → normalize → legal → design |
| `/rules-ff` | normalize → legal → design → fixture → tasks |
| `/rules-apply` | implement → test → benchmark |
| `/rules-verify` | review → commit-plan |
| `/rules-archive` | publish/archive |

## Topic Keys en Engram

```
rules/{batch}/state              → engram/rules/{batch}/state.json
rules/{batch}/knowledge-research → engram/rules/{batch}/knowledge-research.json
rules/{batch}/concepts          → engram/rules/{batch}/concepts.json
rules/{batch}/legal-review      → engram/rules/{batch}/legal-review.json
rules/{batch}/rule-designs      → engram/rules/{batch}/rule-designs.json
rules/{batch}/fixture-matrix     → engram/rules/{batch}/fixture-matrix.json
rules/{batch}/tasks             → engram/rules/{batch}/tasks.json
rules/{batch}/apply-progress    → engram/rules/{batch}/apply-progress.json
rules/{batch}/test-report       → engram/rules/{batch}/test-report.json
rules/{batch}/benchmark-report   → engram/rules/{batch}/benchmark-report.json
rules/{batch}/review-report     → engram/rules/{batch}/review-report.json
rules/{batch}/commit-plan      → engram/rules/{batch}/commit-plan.json
rules/{batch}/archive-report    → engram/rules/{batch}/archive-report.json
rules/{batch}/entropy-report    → engram/rules/{batch}/entropy-report.json
```

---

*Template para inicializar batches de reglas*
*Última actualización: 15 de Mayo de 2026*