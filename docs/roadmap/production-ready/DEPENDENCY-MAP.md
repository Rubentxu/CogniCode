# Mapa de dependencias y cronograma

## Grafo de ejecución

```mermaid
flowchart LR
  QW1[QW-01 C8 dossier] --> QW2[QW-02 docs reconcile]
  QW3[QW-03 tracked-file guard] --> QW4[QW-04 clean-clone preflight]
  QW1 --> CR1[CR-01 C8 recertification]
  QW2 --> CR1
  QW4 --> CR1
  CR1 --> CR2[CR-02 CP UAT constraints]

  QW2 --> CR3[CR-03 e91 profiling]
  CR3 --> CR4[CR-04 e91 optimization]
  CR4 --> CR5[CR-05 perf budget + scorecard]

  CR6[CR-06 application fitness functions] --> CR8[CR-08 adaptive PR-CI]
  CR8 --> CR9[CR-09 coverage governance]

  CR6 --> ST1[ST-01 FileOperations ports]
  ST1 --> ST2[ST-02 WorkspaceSession composition]
  ST2 --> ST3[ST-03 AnalysisService deep modules]
  ST1 --> ST4[ST-04 HandlerContext capabilities]
  ST3 --> ST4
  ST3 --> ST5[ST-05 single graph-build semantic owner]

  QW5[QW-05 pin Actions] --> QW6[QW-06 dependency updater]
  CR7[CR-07 OTel 0.28]
```

## Timeline recomendado

```mermaid
gantt
    title CogniCode Production Ready — ruta propuesta
    dateFormat  YYYY-MM-DD
    axisFormat  %d/%m

    section Quick Wins
    QW-01..04 Governance/Reproducibility :qw, 2026-09-28, 4d
    QW-05..07 Supply-chain hygiene       :2026-09-28, 3d

    section Críticos
    CR-01/02 C8-R                        :crit, after qw, 2d
    CR-03 e91 profiling                  :crit, 2026-10-01, 2d
    CR-04 e91 optimization               :crit, after CR-03 e91 profiling, 4d
    CR-05 perf gate                      :crit, after CR-04 e91 optimization, 2d
    CR-06 architecture fitness           :2026-10-02, 2d
    CR-07 OTel migration                 :2026-10-05, 2d
    CR-08 adaptive CI                    :after CR-06 architecture fitness, 3d
    CR-09 coverage policy                :after CR-08 adaptive CI, 2d

    section Estratégicos
    ST-01 FileOperations seam            :2026-10-12, 4d
    ST-02 Workspace composition          :after ST-01 FileOperations seam, 5d
    ST-03 Analysis deep modules          :after ST-02 Workspace composition, 8d
    ST-04 HandlerContext capabilities    :after ST-03 Analysis deep modules, 6d
    ST-05 graph-build owner              :after ST-03 Analysis deep modules, 6d
```

Las fechas son orientativas y sirven para visualizar dependencias. El esfuerzo válido es el rango por acción; los días exactos deben recalibrarse tras CR-03 y tras el inventario inicial de CR-06.
