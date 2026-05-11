# 08 — Backlog Integration

> Cómo el usuario humano alimenta ideas, bugs, y propuestas de mejora al
> circuito autónomo de AutoResearch. El backlog es el puente entre la
> intención humana y la ejecución autónoma del agente.

---

## 1. El Archivo `backlog.md`

El usuario escribe (o el agente genera durante Planning) un archivo Markdown
que contiene items priorizados de mejora:

```markdown
# CogniCode Improvement Backlog

> Last updated: 2026-05-10
> Total items: 12 | P0: 3 | P1: 5 | P2: 4

## P0 — Critical (do these first)

### [P0] Fix SonarQube metadata mismatches
  Phase: Development | Component: cognicode-axiom
  Description: 26 rules have incorrect severity or type metadata.
  Current SonarQube accuracy: 46%. Target: 90%+
  Expected health impact: SONARQUBE +0.15
  Risk: Low (metadata changes don't affect detection logic)
  Effort: Small (~5 min per rule × 26 = ~2h)

### [P0] S107 excludes self parameter from count
  Phase: Development | Component: cognicode-axiom/src/rules/s107.rs
  Description: Method receivers (self, &self, &mut self) are counted
  as parameters, inflating the count and causing false positives.
  Expected health impact: SMELLS +0.02
  Risk: Low

### [P0] Fix tree-sitter queries with invalid capture groups
  Phase: Development | Component: cognicode-axiom/src/rules/
  Description: Several rules reference capture groups that don't exist
  in the actual tree-sitter query, causing "no such group" errors.
  Affected: S4792, S2068, S1134 at minimum.
  Expected health impact: Tests gate passes (currently flaky)
  Risk: Medium

## P1 — High (do these after P0)

### [P1] Reduce build time by parallelizing call graph construction
  Phase: Development | Component: cognicode-core
  Expected health impact: PERFORMANCE +0.05

### [P1] Add Python rule coverage for S100-S120
  Phase: Development | Component: cognicode-axiom
  Expected health impact: MULTI-LANG +0.10

## P2 — Medium (nice to have)
  ...
```

---

## 2. Formato Estructurado (JSON Schema)

Para integración programática, el backlog también se puede expresar en JSON:

```json
{
  "version": "1.0",
  "project": "CogniCode",
  "items": [
    {
      "id": "BL-001",
      "priority": "P0",
      "title": "Fix SonarQube metadata mismatches",
      "phase": "Development",
      "component": "cognicode-axiom",
      "description": "26 rules have incorrect severity or type metadata.",
      "expected_health_impact": {
        "sonarqube": 0.15
      },
      "risk": "Low",
      "effort_minutes": 120,
      "dependencies": [],
      "status": "pending",
      "created_by": "human",
      "created_at": "2026-05-10T10:00:00Z"
    }
  ]
}
```

---

## 3. Integración con el Agente

### Cómo el agente consulta el backlog

```
En cada iteración, el agente:

1. Evalúa el Health Score actual
2. Consulta el backlog: autoresearch_backlog(action="List", filter="pending")
3. PRIORIZA:
   - Si hay items P0 → tomarlos primero
   - Si hay items P1 en la fase SDLC actual → considerarlos
   - Si el backlog está vacío → exploración libre guiada por Health Score
4. Cuando completa un item → autoresearch_backlog(action="Complete", id="BL-001")
```

### Algoritmo de Priorización

```rust
pub fn select_next_task(
    health_score: &HealthScore,
    backlog: &[BacklogItem],
    current_phase: SdlcPhase,
) -> Option<BacklogItem> {
    // 1. Filtrar por fase actual
    let mut candidates: Vec<_> = backlog.iter()
        .filter(|item| item.phase == current_phase || item.phase.is_any())
        .filter(|item| item.status == BacklogStatus::Pending)
        .collect();

    if candidates.is_empty() {
        return None; // Exploración libre
    }

    // 2. Ordenar por prioridad (P0 > P1 > P2)
    candidates.sort_by_key(|item| item.priority);

    // 3. Entre items de misma prioridad, ordenar por ratio impacto/esfuerzo
    candidates.sort_by(|a, b| {
        let ratio_a = a.total_health_impact() / a.effort_minutes as f64;
        let ratio_b = b.total_health_impact() / b.effort_minutes as f64;
        ratio_b.partial_cmp(&ratio_a).unwrap()
    });

    Some(candidates[0].clone())
}
```

---

## 4. Ciclo de Vida de un Backlog Item

```
                    ┌──────────┐
                    │ PENDING  │ ← El humano (o Planning) lo crea
                    └────┬─────┘
                         │
                    ┌────▼─────┐
                    │ IN_PROGRESS│ ← El agente lo toma
                    └────┬─────┘
                         │
              ┌──────────┼──────────┐
              │          │          │
         ┌────▼───┐ ┌───▼────┐ ┌───▼────┐
         │DONE    │ │BLOCKED │ │FAILED  │
         │(health │ │(dep on │ │(no se  │
         │mejoró) │ │other)  │ │pudo)   │
         └────────┘ └────────┘ └────────┘
```

---

## 5. Auto-Generación del Backlog

Durante la fase de Planning, el agente genera automáticamente items de backlog:

```rust
pub fn generate_backlog(ctx: &ProjectContext) -> Vec<BacklogItem> {
    let mut items = Vec::new();

    // 1. De métricas con peor puntuación
    let health = evaluate_health(ctx)?;
    for (dim, score) in health.breakdown() {
        if score < 0.50 {
            items.push(BacklogItem {
                priority: Priority::P1,
                title: format!("Improve {} score (currently {:.2})", dim.name(), score),
                phase: SdlcPhase::Development,
                auto_generated: true,
                ..
            });
        }
    }

    // 2. De smells detectados
    let smells = detect_smells(ctx)?;
    for smell in smells.iter().take(10) {
        items.push(BacklogItem {
            priority: if smell.severity == Severity::Critical { Priority::P0 }
                      else { Priority::P2 },
            title: smell.description.clone(),
            phase: SdlcPhase::Development,
            auto_generated: true,
            ..
        });
    }

    // 3. De bugs conocidos (git log de fixes recientes)
    let recent_bugs = analyze_git_log_for_bugs(ctx)?;
    for bug in recent_bugs {
        items.push(BacklogItem {
            priority: Priority::P0,
            title: format!("Fix bug: {}", bug.description),
            phase: SdlcPhase::Development,
            auto_generated: true,
            ..
        });
    }

    items
}
```

---

## 6. MCP Tools para el Backlog

```rust
#[tool(name = "autoresearch_backlog")]
/// CRUD del backlog de mejoras.
async fn backlog(
    action: String,     // "list" | "add" | "prioritize" | "complete" | "fail"
    item: Option<BacklogItemInput>,
    filter: Option<String>, // "pending" | "in_progress" | "all" | "P0"
) -> Result<Vec<BacklogItem>> {
    match action.as_str() {
        "list" => {
            let mut items = load_backlog()?;
            if let Some(filter) = filter {
                items = apply_filter(items, &filter);
            }
            Ok(items)
        }
        "add" => {
            let item = item.ok_or(anyhow!("item required for add"))?;
            let backlog_item = BacklogItem::new(item);
            append_to_backlog(&backlog_item)?;
            Ok(vec![backlog_item])
        }
        "prioritize" => {
            let mut items = load_backlog()?;
            // Reordenar por impacto/esfuerzo estimado
            items.sort_by_key(|i| i.estimated_impact_effort_ratio());
            save_backlog(&items)?;
            Ok(items)
        }
        "complete" => {
            let id = item.ok_or(anyhow!("item id required"))?.id;
            update_status(&id, BacklogStatus::Done)?;
            Ok(vec![])
        }
        "fail" => {
            let id = item.ok_or(anyhow!("item id required"))?.id;
            update_status(&id, BacklogStatus::Failed)?;
            Ok(vec![])
        }
        _ => Err(anyhow!("Unknown action: {}", action)),
    }
}
```

---

## 7. Sincronización Bidireccional

El backlog puede sincronizarse con sistemas externos:

| Sistema | Dirección | Qué se sincroniza |
|---------|-----------|-------------------|
| GitHub Issues | Bidireccional | Issues ↔ Backlog items |
| Jira | Bidireccional | Tickets ↔ Backlog items |
| Linear | Bidireccional | Issues ↔ Backlog items |
| Notion | Export | Documento de planning |
| Markdown | Nativo | `backlog.md` en el repo |

---

## 8. Métricas del Backlog

El meta-agente (Nivel 3) analiza la eficiencia del backlog:

```
Backlog Efficiency Metrics:
  - Items completed per day: 3.2
  - Avg health gain per item: +0.004
  - Human items vs auto-generated: 40/60
  - Most effective priority: P0 (avg gain +0.012)
  - Bottleneck: items blocked by dependencies (3 items)
  - Time-to-complete: P0 items = 2.3h avg, P1 = 8.7h avg
```

---

## Siguiente: [09 — program.md Reference](09-program-md-reference.md)
