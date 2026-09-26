# Proposal — Application boundary hardening

## Problem
La arquitectura escrita exige inversión de dependencias, pero `application` todavía importa implementaciones de `infrastructure` y una preocupación MCP. Las constraints ejecutables actuales no detectan ese drift.

## Value
Convertir la dirección de dependencias de application en una propiedad continuamente comprobada y crear seams de remediación sin big-bang.

## Scope
- dos nuevas fitness functions;
- inventario de violaciones;
- primera remediación vertical: FileOperationsService;
- política de excepciones temporales.

## Out of scope
- dividir todos los mega-módulos de una vez;
- reescribir Runtime;
- introducir un bus genérico de servicios.
