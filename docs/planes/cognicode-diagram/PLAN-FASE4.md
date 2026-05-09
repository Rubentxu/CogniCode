# Plan Fase 4 — Layout Engine + SVG Nativo

## Objetivo

Implementar un motor de layout propio basado en Sugiyama con soporte para puertos y nodos compuestos, capaz de renderizar diagramas directamente a SVG sin depender de Mermaid como intermediario.

## Duracion Estimada: 2 semanas

## Pre-requisitos

- Fase 1 completada (model types)
- `rust-sugiyama` crate integrado
- Tipos de layout definidos en `layout/types.rs`

## Tasks

### T4.1: Layout Types (1 dia)

**Descripcion**: Definir los tipos de datos para el layout.

**Pasos**:
- [ ] `layout/types.rs`:
  - [ ] `LayoutedNode` — position, size, ports, label, style
  - [ ] `LayoutedEdge` — source_port, target_port, bend_points, label
  - [ ] `Point` — x, y (f64)
  - [ ] `Port` — side (N/S/E/W), offset, connected_edge
  - [ ] `PortSide` — North, South, East, West
  - [ ] `LayoutConfig` — direction, spacing, margin, node_separation, rank_separation
  - [ ] `LayoutedDiagram` — nodes, edges, bounds

**Criterio de aceptacion**: Tipos compilan y se pueden usar para representar un diagrama de 3 nodos con 2 edges.

---

### T4.2: Sugiyama Wrapper (3 dias)

**Descripcion**: Wrapper sobre `rust-sugiyama` que convierte C4Model → petgraph → layout coordinates.

**Pasos**:
- [ ] `layout/sugiyama.rs` — `compute_layout(C4Workspace, LayoutConfig) -> LayoutedDiagram`
- [ ] Convertir C4 elements → petgraph nodes
- [ ] Convertir C4 relationships → petgraph edges
- [ ] Llamar `rust-sugiyama::layout` para obtener coordenadas
- [ ] Mapear coordenadas de vuelta a `LayoutedNode` con sizes
- [ ] Calcular sizes de nodos basado en contenido (numero de metodos, nombre length)
- [ ] Soporte para direction: TB (top-bottom), LR (left-right)
- [ ] Calcular bounds totales del diagrama

**Criterio de aceptacion**: Para un grafo de 20 nodos, produce coordenadas sin solapamientos.

---

### T4.3: Port Assignment (3 dias)

**Descripcion**: Asignar puertos a los nodos basandose en el tipo de relacion y la direccion del layout.

**Pasos**:
- [ ] `layout/port_assigner.rs` — `assign_ports(LayoutedDiagram) -> LayoutedDiagram`
- [ ] Reglas de asignacion:
  - [ ] Layout TB: edges hacia abajo → Puerto Sur en source, Puerto Norte en target
  - [ ] Layout LR: edges hacia derecha → Puerto Este en source, Puerto Oeste en target
  - [ ] Si un nodo tiene >1 edge en el mismo lado, distribuir offsets equitativamente
  - [ ] Cruzamientos: si source/target estan en capas no adyacentes, usar puertos laterales
- [ ] Minimizar cruces de puertos (heuristica de baricentro)
- [ ] Asignar bend_points entre puertos para enrutado ortogonal

**Criterio de aceptacion**: Para un diagrama de 10 nodos con 15 edges, cada edge tiene puertos asignados sin solapamientos de puertos en un mismo lado.

---

### T4.4: Compound Nodes (2 dias)

**Descripcion**: Soporte para nodos compuestos (parent-children) para diagramas de containers y componentes.

**Pasos**:
- [ ] `layout/compound.rs` — `layout_compound(LayoutedDiagram) -> LayoutedDiagram`
- [ ] Modelar jerarquia: SoftwareSystem contiene Containers, Container contiene Components
- [ ] Layout recursivo: primero layout hijos, luego expandir size del padre
- [ ] Padding y margin entre padre e hijos
- [ ] Edges que cruzan boundaries de padres: enrutado a traves del borde del padre
- [ ] Integracion con port_assigner para edges cross-boundary

**Criterio de aceptacion**: Diagrama de containers de CogniCode muestra 13 crates dentro del boundary del sistema "CogniCode" sin solapamientos.

---

### T4.5: SVG Renderer (3 dias)

**Descripcion**: Renderizar `LayoutedDiagram` directamente a SVG sin intermediarios.

**Pasos**:
- [ ] `render/svg.rs` — `render_svg(LayoutedDiagram, Theme) -> String`
- [ ] Elementos SVG:
  - [ ] Rectangulos redondeados para services/libraries
  - [ ] Cilindros para data stores (ellipse + rect + ellipse)
  - [ ] Icono persona para actors
  - [ ] Lineas con flechas para relationships (orthogonal routing)
  - [ ] Labels en nodes y edges
  - [ ] Subgraphs/boundaries para compound nodes (rect punteado)
- [ ] Temas: reutilizar los 14 temas existentes de `cognicode-core/mermaid/mod.rs`
- [ ] Responsive: viewBox calculado desde bounds
- [ ] CSS inline para estilos (sin dependencias externas)
- [ ] Export a archivo con `std::fs::write`

**Criterio de aceptacion**: SVG generado para diagrama de containers de CogniCode es visualizable en navegador sin artefactos, con al menos 10 nodos legibles.

---

### T4.6: Layout Cache (1 dia)

**Descripcion**: Cache de layouts para regeneracion incremental.

**Pasos**:
- [ ] Hash del C4Workspace (blake3 de JSON serializado)
- [ ] Si hash coincide, reutilizar layout cacheado
- [ ] Invalidar si cambia FileManifest (reutilizar logica de core)
- [ ] Almacenar en `.cognicode/layout.cache`

**Criterio de aceptacion**: Segunda generacion del mismo diagrama es >10x mas rapida que la primera.

---

### T4.7: Tests (1 dia)

- [ ] Test: layout de 5 nodos sin solapamientos
- [ ] Test: port assignment para 10 edges sin conflictos
- [ ] Test: compound node con 3 hijos, size correcto
- [ ] Test: SVG output es valido (parseable como XML)
- [ ] Test: cache funciona (segunda llamada es instantanea)
- [ ] Test de rendimiento: layout + SVG de 100 nodos <1s

## Milestone M7

**Criterio**: Layout con puertos produce SVG legible para diagrama de containers de CogniCode (13 crates + 2 data stores + relationships) sin cruces visibles.
