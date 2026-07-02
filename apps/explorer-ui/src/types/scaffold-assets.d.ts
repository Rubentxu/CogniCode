/**
 * Type declarations for `@scaffold-assets` module alias.
 *
 * The alias resolves to `crates/cognicode-explorer/assets` where
 * `moldql-scaffolds.yaml` lives. Vite handles the import via
 * `@rollup/plugin-yaml`; these declarations make TypeScript aware
 * of the module shape.
 */
declare module "@scaffold-assets/moldql-scaffolds.yaml" {
  const data: unknown;
  export default data;
}
