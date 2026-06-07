# Q001-P1 Proxy Answer

**Question**: TypeScript or JavaScript for the Explorer frontend?

**Answer**: TypeScript with strict mode, `.tsx` for components, `.ts` for utilities. Domain types generated from Rust DTOs via `ts-rs` into shared `@cognicode/types` package. `strict: true` in tsconfig. Prototype rewritten, not migrated.

**Confidence**: high
**Needs research**: no
**Needs user validation**: false
