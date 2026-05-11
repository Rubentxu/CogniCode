//! S5042 — Auto-segregated by Karpathy workflow (SOLID/SRP)
use crate::{Severity,Category,Issue,Remediation,Rule,RuleContext,RuleEntry};
use crate::rules::{CleanCodeAttribute,SoftwareQuality,SoftwareQualityImpact,ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;

declare_rule! {
    id: "S5042"
    name: "Archive extraction should check size before decompression"
    severity: Major
    category: Vulnerability
    language: "rust"
    params: {}

    explanation: "Extracting archive files without size limits can enable zip bomb attacks where small files decompress to enormous sizes, exhausting system resources.",
    clean_code: Trustworthy,
    impacts: [Security: Medium, Reliability: Medium, Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        for (line_num, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with("//") || trimmed.starts_with("///")
            || trimmed.starts_with("//!") || trimmed.starts_with("/*") || trimmed.starts_with("*")
            || trimmed.starts_with("#") { continue; }
            let has_archive = trimmed.contains(".zip(") || trimmed.contains("ZipArchive") || trimmed.contains("zip::") || trimmed.contains("tar::") || trimmed.contains("Archive::") || trimmed.contains("unzip") || trimmed.contains("flate2") || trimmed.contains("extract_all") || trimmed.contains("unpack") || trimmed.contains("sevenz") || trimmed.contains("bzip2::") || trimmed.contains("xz::") || trimmed.contains("zstd::") || trimmed.contains("snap::") || trimmed.contains("ar::");
            if has_archive && !trimmed.contains("limit") && !trimmed.contains("max_") && !trimmed.contains("size_limit") {
                let context_start = line_num.saturating_sub(10);
                let context_end = std::cmp::min(ctx.source.lines().count(), line_num + 8);
                let context: String = ctx.source.lines().skip(context_start).take(context_end - context_start).collect::<Vec<_>>().join("\n");
                if !context.contains("size") && !context.contains("limit") && !context.contains("max_size") && !context.contains("uncompressed_size") && !context.contains("decompressed_size") && !context.contains("capacity") && !context.contains("max_bytes") && !context.contains("max_len") && !context.contains("budget") && !context.contains("quota") && !context.contains("set_size_limit") && !context.contains("len") && !context.contains("byte") && !context.contains("memory_limit") {
                    issues.push(Issue::new(
                        "S5042",
                        "Archive extraction without size check - potential zip bomb",
                        Severity::Major,
                        Category::Vulnerability,
                        ctx.file_path,
                        line_num + 1,
                    ));
                }
            }
        }
        issues
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_s5042_registered() {
        let rule=S5042Rule::new();
        assert_eq!(rule.id(),+rule_id+);
        assert!(rule.name().len()>0);
    }
}
