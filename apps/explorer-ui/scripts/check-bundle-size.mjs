#!/usr/bin/env node
/**
 * Bundle budget check.
 *
 * Walks `dist/assets/*` and asserts:
 *  - Total gzipped JS < 200 KB
 *  - Total gzipped CSS < 50 KB
 *  - No single JS chunk > 250 KB gzipped
 *
 * Prints a table of every asset (path, raw size, gzipped size).
 * Exits with code 1 on violation so it slots into a CI step.
 *
 * Usage:
 *   node scripts/check-bundle-size.mjs            # check after `npm run build`
 *   npm run build:check-bundle                    # convenience
 */
import { readdir, readFile, stat } from "node:fs/promises";
import { gzipSync } from "node:zlib";
import { join, extname } from "node:path";
import { fileURLToPath } from "node:url";

const DIST_DIR = fileURLToPath(new URL("../dist", import.meta.url));
const ASSETS_DIR = join(DIST_DIR, "assets");

const BUDGETS = {
  jsTotal: 200 * 1024,    // 200 KB gzipped
  jsChunk: 250 * 1024,    // 250 KB gzipped (single chunk)
  cssTotal: 50 * 1024,    // 50 KB gzipped
};

const HUMAN = (n) => {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(2)} KB`;
  return `${(n / 1024 / 1024).toFixed(2)} MB`;
};

async function listAssets() {
  try {
    return await readdir(ASSETS_DIR);
  } catch (e) {
    console.error(`Could not read ${ASSETS_DIR}. Run \`npm run build\` first.`);
    throw e;
  }
}

async function measure(path) {
  const buf = await readFile(path);
  const raw = buf.byteLength;
  const gz = gzipSync(buf).byteLength;
  return { raw, gz };
}

async function main() {
  const files = await listAssets();
  const rows = await Promise.all(
    files
      .filter((f) => [".js", ".css", ".html"].includes(extname(f)))
      .map(async (f) => {
        const p = join(ASSETS_DIR, f);
        const { raw, gz } = await measure(p);
        const st = await stat(p);
        return { file: f, raw, gz, mtime: st.mtimeMs };
      }),
  );

  rows.sort((a, b) => a.file.localeCompare(b.file));

  console.log("\n  Asset                              Raw         Gzipped");
  console.log("  ────────────────────────────────  ──────────  ──────────");
  for (const r of rows) {
    console.log(
      `  ${r.file.padEnd(34)}  ${HUMAN(r.raw).padStart(10)}  ${HUMAN(r.gz).padStart(10)}`,
    );
  }

  const js = rows.filter((r) => r.file.endsWith(".js"));
  const css = rows.filter((r) => r.file.endsWith(".css"));
  const jsTotal = js.reduce((s, r) => s + r.gz, 0);
  const cssTotal = css.reduce((s, r) => s + r.gz, 0);
  const maxChunk = js.reduce((m, r) => Math.max(m, r.gz), 0);

  console.log("\n  Totals");
  console.log("  ────────────────────────────────  ──────────  ──────────");
  console.log(
    `  JS gzipped                        ${HUMAN(js.reduce((s, r) => s + r.raw, 0)).padStart(10)}  ${HUMAN(jsTotal).padStart(10)}`,
  );
  console.log(
    `  CSS gzipped                       ${HUMAN(css.reduce((s, r) => s + r.raw, 0)).padStart(10)}  ${HUMAN(cssTotal).padStart(10)}`,
  );
  console.log(
    `  Largest JS chunk (gzipped)        ${HUMAN(js.find((r) => r.gz === maxChunk)?.raw ?? 0).padStart(10)}  ${HUMAN(maxChunk).padStart(10)}`,
  );

  console.log("\n  Budgets");
  console.log("  ────────────────────────────────  ──────────");
  console.log(`  JS total    < ${HUMAN(BUDGETS.jsTotal)}                  ${jsTotal}  ${jsTotal < BUDGETS.jsTotal ? "OK" : "FAIL"}`);
  console.log(`  JS chunk    < ${HUMAN(BUDGETS.jsChunk)}                  ${maxChunk}  ${maxChunk < BUDGETS.jsChunk ? "OK" : "FAIL"}`);
  console.log(`  CSS total   < ${HUMAN(BUDGETS.cssTotal)}                  ${cssTotal}  ${cssTotal < BUDGETS.cssTotal ? "OK" : "FAIL"}`);

  const ok =
    jsTotal < BUDGETS.jsTotal &&
    maxChunk < BUDGETS.jsChunk &&
    cssTotal < BUDGETS.cssTotal;

  if (!ok) {
    console.error("\n  Bundle budget violation. See rows above.");
    process.exit(1);
  }
  console.log("\n  All budgets OK.\n");
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
