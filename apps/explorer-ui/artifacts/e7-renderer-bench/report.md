# E7 Renderer Benchmark Report

Generated at: 2026-06-21T18:34:27.585Z
Record count: 24

## Renderer: `cytoscape-canvas`

### Fixture: `call-graph-small`

| run | mode | load (ms) | select (ms) | pan (ms) | zoom (ms) | relayout (ms) | valid | regressions |
|-----|------|-----------|-------------|-----------|-----------|----------------|-------|-------------|
| 0 | cold | 52.50 | 0.20 | 0.10 | 1.10 | 4.00 | yes | 0 |
| 1 | warm | 13.50 | 0.00 | 0.00 | 0.70 | 2.20 | yes | 0 |

### Fixture: `call-graph-medium`

| run | mode | load (ms) | select (ms) | pan (ms) | zoom (ms) | relayout (ms) | valid | regressions |
|-----|------|-----------|-------------|-----------|-----------|----------------|-------|-------------|
| 0 | cold | 522.60 | 0.10 | 0.00 | 77.80 | 130.00 | yes | 0 |
| 1 | warm | 413.00 | 0.10 | 0.00 | 70.90 | 115.90 | yes | 0 |

### Fixture: `call-graph-large`

| run | mode | load (ms) | select (ms) | pan (ms) | zoom (ms) | relayout (ms) | valid | regressions |
|-----|------|-----------|-------------|-----------|-----------|----------------|-------|-------------|
| 0 | cold | 2197.40 | 0.20 | 0.00 | 384.70 | 619.00 | yes | 0 |
| 1 | warm | 2240.30 | 0.10 | 0.00 | 374.20 | 623.70 | yes | 0 |

### Fixture: `dependency-graph-small`

| run | mode | load (ms) | select (ms) | pan (ms) | zoom (ms) | relayout (ms) | valid | regressions |
|-----|------|-----------|-------------|-----------|-----------|----------------|-------|-------------|
| 0 | cold | 16.20 | 0.00 | 0.00 | 0.60 | 1.70 | yes | 0 |
| 1 | warm | 14.10 | 0.00 | 0.00 | 0.70 | 1.40 | yes | 0 |

### Fixture: `architecture-c4-medium`

| run | mode | load (ms) | select (ms) | pan (ms) | zoom (ms) | relayout (ms) | valid | regressions |
|-----|------|-----------|-------------|-----------|-----------|----------------|-------|-------------|
| 0 | cold | 13.80 | 0.00 | 0.00 | 0.40 | 1.10 | yes | 0 |
| 1 | warm | 12.90 | 0.10 | 0.00 | 0.40 | 0.80 | yes | 0 |

### Fixture: `landing-overview-medium`

| run | mode | load (ms) | select (ms) | pan (ms) | zoom (ms) | relayout (ms) | valid | regressions |
|-----|------|-----------|-------------|-----------|-----------|----------------|-------|-------------|
| 0 | cold | 16.00 | 0.10 | 0.00 | 0.50 | 1.60 | yes | 0 |
| 1 | warm | 14.40 | 0.00 | 0.00 | 0.50 | 1.60 | yes | 0 |

## Renderer: `cytoscape-webgl`

### Fixture: `call-graph-small`

| run | mode | load (ms) | select (ms) | pan (ms) | zoom (ms) | relayout (ms) | valid | regressions |
|-----|------|-----------|-------------|-----------|-----------|----------------|-------|-------------|
| 0 | cold | 41.30 | 0.00 | 0.00 | 0.90 | 1.90 | yes | 0 |
| 1 | warm | 25.40 | 0.10 | 0.10 | 0.40 | 1.60 | yes | 0 |

### Fixture: `call-graph-medium`

| run | mode | load (ms) | select (ms) | pan (ms) | zoom (ms) | relayout (ms) | valid | regressions |
|-----|------|-----------|-------------|-----------|-----------|----------------|-------|-------------|
| 0 | cold | 417.10 | 0.00 | 0.00 | 86.00 | 138.60 | yes | 0 |
| 1 | warm | 415.10 | 0.10 | 0.00 | 70.90 | 117.50 | yes | 0 |

### Fixture: `call-graph-large`

| run | mode | load (ms) | select (ms) | pan (ms) | zoom (ms) | relayout (ms) | valid | regressions |
|-----|------|-----------|-------------|-----------|-----------|----------------|-------|-------------|
| 0 | cold | 2080.90 | 0.20 | 0.00 | 388.70 | 641.50 | yes | 0 |
| 1 | warm | 2131.60 | 0.00 | 0.00 | 391.20 | 649.90 | yes | 0 |

### Fixture: `dependency-graph-small`

| run | mode | load (ms) | select (ms) | pan (ms) | zoom (ms) | relayout (ms) | valid | regressions |
|-----|------|-----------|-------------|-----------|-----------|----------------|-------|-------------|
| 0 | cold | 19.40 | 0.00 | 0.00 | 0.70 | 1.90 | yes | 0 |
| 1 | warm | 19.80 | 0.00 | 0.00 | 0.60 | 1.40 | yes | 0 |

### Fixture: `architecture-c4-medium`

| run | mode | load (ms) | select (ms) | pan (ms) | zoom (ms) | relayout (ms) | valid | regressions |
|-----|------|-----------|-------------|-----------|-----------|----------------|-------|-------------|
| 0 | cold | 19.20 | 0.00 | 0.00 | 0.40 | 1.20 | yes | 0 |
| 1 | warm | 19.50 | 0.00 | 0.00 | 0.30 | 0.90 | yes | 0 |

### Fixture: `landing-overview-medium`

| run | mode | load (ms) | select (ms) | pan (ms) | zoom (ms) | relayout (ms) | valid | regressions |
|-----|------|-----------|-------------|-----------|-----------|----------------|-------|-------------|
| 0 | cold | 19.90 | 0.00 | 0.00 | 0.60 | 1.50 | yes | 0 |
| 1 | warm | 19.40 | 0.00 | 0.00 | 0.40 | 1.30 | yes | 0 |

## Regressions

No regressions detected.
