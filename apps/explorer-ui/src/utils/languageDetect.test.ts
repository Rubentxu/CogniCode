/**
 * Unit tests for languageDetect utility.
 */
import { describe, it, expect } from "vitest";
import { detectLanguage } from "./languageDetect";

describe("detectLanguage", () => {
  // --- Happy path — one representative case per language family ---
  it('detects "rust" from .rs', () => {
    expect(detectLanguage("src/lib.rs")).toBe("rust");
  });

  it('detects "typescript" from .ts', () => {
    expect(detectLanguage("src/main.ts")).toBe("typescript");
  });

  it('detects "tsx" from .tsx', () => {
    expect(detectLanguage("src/components/App.tsx")).toBe("tsx");
  });

  it('detects "javascript" from .js', () => {
    expect(detectLanguage("src/index.js")).toBe("javascript");
  });

  it('detects "javascript" from .jsx', () => {
    expect(detectLanguage("src/App.jsx")).toBe("javascript");
  });

  it('detects "python" from .py', () => {
    expect(detectLanguage("scripts/deploy.py")).toBe("python");
  });

  it('detects "go" from .go', () => {
    expect(detectLanguage("cmd/server/main.go")).toBe("go");
  });

  it('detects "java" from .java', () => {
    expect(detectLanguage("src/main/java/App.java")).toBe("java");
  });

  it('detects "c" from .c', () => {
    expect(detectLanguage("src/main.c")).toBe("c");
  });

  it('detects "c" from .h', () => {
    expect(detectLanguage("src/header.h")).toBe("c");
  });

  it('detects "cpp" from .cpp', () => {
    expect(detectLanguage("src/main.cpp")).toBe("cpp");
  });

  it('detects "cpp" from .hpp', () => {
    expect(detectLanguage("src/math.hpp")).toBe("cpp");
  });

  it('detects "hcl" from .tf', () => {
    expect(detectLanguage("infra/main.tf")).toBe("hcl");
  });

  it('detects "hcl" from .hcl', () => {
    expect(detectLanguage("config/prod.hcl")).toBe("hcl");
  });

  it('detects "yaml" from .yml', () => {
    expect(detectLanguage(".github/workflows/ci.yml")).toBe("yaml");
  });

  it('detects "yaml" from .yaml', () => {
    expect(detectLanguage("config.yaml")).toBe("yaml");
  });

  it('detects "json" from .json', () => {
    expect(detectLanguage("package.json")).toBe("json");
  });

  it('detects "toml" from .toml', () => {
    expect(detectLanguage("Cargo.toml")).toBe("toml");
  });

  it('detects "bash" from .sh', () => {
    expect(detectLanguage("scripts/setup.sh")).toBe("bash");
  });

  it('detects "bash" from .bash', () => {
    expect(detectLanguage("script.bash")).toBe("bash");
  });

  it('detects "ruby" from .rb', () => {
    expect(detectLanguage("script.rb")).toBe("ruby");
  });

  // --- Case insensitivity ---
  it("handles uppercase extension", () => {
    expect(detectLanguage("SRC/lib.RS")).toBe("rust");
  });

  it("handles mixed case path", () => {
    expect(detectLanguage("src/lib.TypeScript.ts")).toBe("typescript");
  });

  // --- Last-segment match (handles dots in directory names) ---
  it("matches extension of the last path segment only", () => {
    expect(detectLanguage("path/with.dots/file.ts")).toBe("typescript");
  });

  it("matches extension of last segment with Windows backslash", () => {
    expect(detectLanguage("path\\with.dots\\file.ts")).toBe("typescript");
  });

  // --- Unknown / edge cases ---
  it("returns undefined for unknown extension", () => {
    expect(detectLanguage("file.unknown")).toBe(undefined);
  });

  it("returns undefined for extension with no dot", () => {
    expect(detectLanguage("Makefile")).toBe(undefined);
  });

  it("returns undefined for empty string", () => {
    expect(detectLanguage("")).toBe(undefined);
  });

  it("returns undefined for extension-only dot file", () => {
    expect(detectLanguage(".env")).toBe(undefined);
  });

  it("returns undefined when extension is just a trailing dot", () => {
    expect(detectLanguage("file.")).toBe(undefined);
  });
});

// ============================================================================
// resolveSignatureLanguage — content heuristic (T3.2)
// ============================================================================

import { resolveSignatureLanguage } from "./languageDetect";

describe("resolveSignatureLanguage", () => {
  it('detects "rust" from fn signature', () => {
    expect(resolveSignatureLanguage("fn build_overview() {")).toBe("rust");
  });

  it('detects "rust" from pub fn signature', () => {
    expect(resolveSignatureLanguage("pub fn main() {")).toBe("rust");
  });

  it('detects "rust" from impl block', () => {
    expect(resolveSignatureLanguage("impl Display for Error {")).toBe("rust");
  });

  it('detects "rust" from struct definition', () => {
    expect(resolveSignatureLanguage("struct MyStruct {")).toBe("rust");
  });

  it('detects "python" from def signature', () => {
    expect(resolveSignatureLanguage("def hello(name):")).toBe("python");
  });

  it('detects "python" from def signature', () => {
    expect(resolveSignatureLanguage("def hello(name):")).toBe("python");
  });

  it('detects "python" from import statement', () => {
    expect(resolveSignatureLanguage("import os")).toBe("python");
    expect(resolveSignatureLanguage("from typing import List")).toBe("python");
  });

  it('detects "go" from func signature', () => {
    expect(resolveSignatureLanguage("func main() {")).toBe("go");
  });

  it('detects "go" from type declaration', () => {
    expect(resolveSignatureLanguage("type Result struct {")).toBe("go");
  });

  it('detects "typescript" from function keyword', () => {
    expect(resolveSignatureLanguage("function handleRequest() {")).toBe("typescript");
  });

  it('detects "typescript" from const/let/var', () => {
    expect(resolveSignatureLanguage("const x: number = 1;")).toBe("typescript");
    expect(resolveSignatureLanguage("let name: string = \"\";")).toBe("typescript");
  });

  it("returns undefined for unrecognized signature", () => {
    expect(resolveSignatureLanguage("some random text")).toBeUndefined();
  });

  it("returns undefined for empty string", () => {
    expect(resolveSignatureLanguage("")).toBeUndefined();
  });
});
