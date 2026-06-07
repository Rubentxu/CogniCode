# Q003-P1 Skeptic Challenge

**Key concerns**:
1. Radix UI maintenance slowed post-WorkOS acquisition (2022). shadcn/ui already supports Base UI as alternative layer.
2. Miller Columns — the hardest component — has NO Radix primitive. Manual keyboard nav in cascading columns is the real complexity.
3. ~8 components total — "code judo" with aria attributes + Tailwind may be simpler than Radix dependency.
4. Tailwind 4 @theme works but CSS variables from prototype need mapping to `--color-*`, `--font-*` namespaces.
5. Spike needed for Miller Columns focus management before committing.

**Suggested correction**: Evaluate direct aria + Tailwind. Spike Miller Columns keyboard navigation first. If Radix, use unified `radix-ui` package (tree-shakeable).
