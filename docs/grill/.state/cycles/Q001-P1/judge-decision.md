# Q001-P1 Judge Decision (User-Overridden)

**Judge initial verdict**: REJECTED (wanted Leptos extension)
**User override**: "no vamos a usar leptos. el dashboard es otra aplicacion independiente"

**Final verdict**: ACCEPTED (ProxyAnswer) with user clarification

**Final answer**: React 19 + TypeScript strict mode, `.tsx`/`.ts`, clean rewrite from prototype. Dashboard is independent — no code sharing, no Leptos.

**Rationale**: User explicitly rejected extending the Leptos dashboard. Explorer frontend is a standalone React application as intended by ADR 0009. Dashboard and Explorer are separate products.
