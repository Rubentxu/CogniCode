# Diagramas de CogniCode

Este directorio contiene diagramas de Mermaid que documentan la arquitectura y los flujos de trabajo de CogniCode.

## Diagramas

### 1. Diagrama de Arquitectura (`architecture-diagram.mmd`)
Arquitectura de alto nivel que muestra las cuatro capas principales:
- **Capa de Interfaz**: Servidor MCP, Servidor LSP, CLI
- **Capa de Aplicación**: NavigationService, RefactorService, AnalysisService
- **Capa de Dominio**: Symbol, CallGraph, Refactor, ImpactAnalyzer, CycleDetector, ComplexityCalculator
- **Capa de Infraestructura**: TreeSitter Parser, PetGraph Store, Sistema de Archivos Virtual, Safety Gate

### 2. Flujo de Trabajo MCP (`mcp-workflow.mmd`)
Diagrama de secuencia que muestra el flujo de solicitud/respuesta de MCP:
- El Agente LLM envía solicitudes de tool_call
- La solicitud fluye a través del Servidor MCP, Servicios de Aplicación, Dominio e Infraestructura
- La respuesta retorna con el AST analizado y los símbolos

### 3. Flujo de Trabajo de Refactorización (`refactor-workflow.mmd`)
Flujo de refactorización segura con validación de Safety Gate:
- La solicitud del agente activa el análisis y la validación
- Las solicitudes inválidas retornan errores inmediatamente
- Las solicitudes válidas generan ediciones y las aplica al VFS
- La validación del AST ocurre antes de confirmar en disco
- Los ASTs inválidos activan el rechazo y la reversión

### 4. Grafo de Llamadas (`call-graph.mmd`)
Visualización de detección de ciclos:
- Representación visual de las relaciones de llamadas entre funciones
- Los nodos involucrados en ciclos se resaltan en rojo
- Demuestra cómo la detección de ciclos identifica dependencias circulares

## Uso

Estos diagramas pueden renderizarse en cualquier visor compatible con Mermaid:
- Archivos markdown de GitHub/GitLab
- VS Code con extensión Mermaid
- Mermaid Live Editor (https://mermaid.live)
- MkDocs con plugin mermaid2
