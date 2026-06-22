/**
 * Language detection from file paths.
 *
 * Maps file extensions → Prism language identifiers for syntax highlighting.
 * Pure function — no React imports, no side effects.
 */

/**
 * Subset of Prism grammars supported by this implementation.
 * Each string must match the name used in `prismjs/components/prism-{lang}`.
 */
export type PrismLanguage =
  | "rust"
  | "typescript"
  | "tsx"
  | "javascript"
  | "jsx"
  | "python"
  | "go"
  | "java"
  | "c"
  | "cpp"
  | "hcl"
  | "yaml"
  | "json"
  | "toml"
  | "bash"
  | "ruby";

/**
 * Extension → language map. Keys are lowercase extensions (with dot prefix).
 * Extensions that share a grammar (e.g. .h / .hpp) map to the same language.
 */
export const EXT_TO_LANG: Readonly<Record<string, PrismLanguage>> = {
  ".rs": "rust",
  ".ts": "typescript",
  ".tsx": "tsx",
  ".js": "javascript",
  ".jsx": "javascript",
  ".py": "python",
  ".go": "go",
  ".java": "java",
  ".c": "c",
  ".h": "c",
  ".cpp": "cpp",
  ".hpp": "cpp",
  ".tf": "hcl",
  ".hcl": "hcl",
  ".yml": "yaml",
  ".yaml": "yaml",
  ".json": "json",
  ".toml": "toml",
  ".sh": "bash",
  ".bash": "bash",
  ".rb": "ruby",
} as const;

/**
 * Detect language from a file path string.
 * Returns the matching PrismLanguage or undefined if the extension is unknown.
 *
 * @param filePath - absolute or relative file path (e.g. "src/lib.rs" or "/home/user/src/main.ts")
 */
export function detectLanguage(filePath: string): PrismLanguage | undefined {
  if (!filePath) return undefined;

  const lastSegment = filePath.replace(/^.*[/\\]/, "").toLowerCase();

  // Find the last dot to extract extension
  const lastDotIndex = lastSegment.lastIndexOf(".");
  if (lastDotIndex === -1 || lastDotIndex === lastSegment.length - 1) {
    return undefined;
  }

  const ext = lastSegment.slice(lastDotIndex);
  return EXT_TO_LANG[ext];
}
