# Q003-P1 Judge Decision

**Verdict**: MODIFIED

**Final answer**: cmdk for Spotter (cmd+K overlay). Everything else — MillerColumn, ViewTabs, ListRow, CardGrid, Playground, ColumnHeader, Breadcrumb — uses direct ARIA attributes + Tailwind CSS 4. No direct Radix dependencies. cmdk handles its own Radix Dialog internally. ViewTabs ~60 lines following WAI-ARIA Tabs Pattern (APG).

**Why**: Only Spotter has genuine complexity (portal, focus trap, overlay, filtering). Everything else is simple enough for direct ARIA. Miller Columns are custom regardless — no library helps.
