/**
 * Production entrypoint for the bench:renderer script.
 *
 * Wires the CLI to a real node filesystem and exits with the
 * runCli's exit_code. Invoked from package.json via:
 *
 *   bench:renderer -- node --experimental-strip-types src/bench/entry.ts
 */

import { mkdirSync, writeFileSync } from "node:fs";
import { dirname } from "node:path";

import { runCli } from "./cli";
import type { ReportFilesystem } from "./report";

const fs: ReportFilesystem = {
  writeFile(path, content) {
    mkdirSync(dirname(path), { recursive: true });
    writeFileSync(path, content, "utf8");
  },
};

const argv = process.argv.slice(2);
const env = process.env;

runCli(argv, env, fs)
  .then((result) => {
    process.exit(result.exit_code);
  })
  .catch((err: unknown) => {
    console.error("[bench] fatal:", err);
    process.exit(1);
  });