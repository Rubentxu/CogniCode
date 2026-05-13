//! Mock LLM implementation for testing

use super::{
    SummarizationError, SummarizationPrompt, LlmSummarizationResult, LlmSummarizer,
    SummaryStyle, DiagramStatistics, ArchitectureRisk, RiskSeverity,
};

/// Mock LLM summarizer for testing
///
/// This implementation returns deterministic, structured responses
/// that can be used to verify the summarization pipeline.
pub struct MockLlmSummarizer {
    /// Provider name to return
    provider_name: &'static str,
    /// Whether to simulate errors
    simulate_error: bool,
}

impl MockLlmSummarizer {
    /// Create a new mock LLM summarizer
    pub fn new() -> Self {
        Self {
            provider_name: "mock-llm",
            simulate_error: false,
        }
    }

    /// Set a custom provider name
    pub fn with_provider_name(mut self, name: &'static str) -> Self {
        self.provider_name = name;
        self
    }

    /// Enable error simulation
    pub fn with_error(mut self) -> Self {
        self.simulate_error = true;
        self
    }
}

impl Default for MockLlmSummarizer {
    fn default() -> Self {
        Self::new()
    }
}

impl LlmSummarizer for MockLlmSummarizer {
    fn summarize(
        &self,
        prompt: SummarizationPrompt,
    ) -> impl std::future::Future<Output = Result<LlmSummarizationResult, SummarizationError>> + Send {
        let simulate_error = self.simulate_error;
        let provider_name = self.provider_name;

        async move {
            if simulate_error {
                return Err(SummarizationError::LlmError("Mock error".to_string()));
            }

            let text = generate_mock_summary(&prompt);
            let findings = generate_mock_findings(&prompt);
            let suggestions = generate_mock_suggestions(&prompt);

            // Suppress unused variable warning
            let _ = provider_name;

            Ok(LlmSummarizationResult {
                text,
                findings,
                suggestions,
            })
        }
    }

    fn provider_name(&self) -> &'static str {
        self.provider_name
    }
}

/// Generate mock summary text based on style
fn generate_mock_summary(prompt: &SummarizationPrompt) -> String {
    let stats = &prompt.statistics;
    let style = prompt.style;

    match style {
        SummaryStyle::Executive => format!(
            "The {} architecture comprises {} software system(s) with {} container(s) \
            and {} relationship(s). This is a {} system using {} technology stack(s). \
            The architecture demonstrates {} with key components including {}.",
            prompt.statistics.system_count,
            stats.system_count,
            stats.container_count,
            stats.relationship_count,
            if stats.container_count > 5 { "complex distributed" } else { "modular" },
            stats.technologies.join(", "),
            if stats.component_count > 10 { "significant component depth" } else { "straightforward design" },
            stats.container_count
        ),
        SummaryStyle::Technical => format!(
            "# Technical Analysis\n\n\
            ## Architecture Overview\n\
            - Systems: {} ({} internal)\n\
            - Containers: {} across all systems\n\
            - Components: {} total components identified\n\
            - Relationships: {} defined inter-component connections\n\n\
            ## Technology Stack\n\
            {}\n\n\
            ## System Components\n",
            stats.system_count,
            stats.system_count,
            stats.container_count,
            stats.component_count,
            stats.relationship_count,
            stats.technologies.iter().map(|t| format!("- {}", t)).collect::<Vec<_>>().join("\n")
        ),
        SummaryStyle::RiskAssessment => format!(
            "# Risk Assessment Report\n\n\
            ## Risk Profile\n\
            - Critical Risks: {}\n\
            - High Risks: {}\n\
            - Medium Risks: {}\n\
            - Low Risks: {}\n\n\
            ## Identified Risks\n\
            {}\n\n\
            ## Recommendations\n\
            The architecture should focus on addressing any high or critical risks \
            to ensure system reliability and maintainability.\n",
            prompt.risks.iter().filter(|r| super::RiskSeverity::Critical == r.severity).count(),
            prompt.risks.iter().filter(|r| super::RiskSeverity::High == r.severity).count(),
            prompt.risks.iter().filter(|r| super::RiskSeverity::Medium == r.severity).count(),
            prompt.risks.iter().filter(|r| super::RiskSeverity::Low == r.severity).count(),
            prompt.risks.iter()
                .map(|r| format!("- **[{:?}]** {}: {}", r.severity, r.id, r.description))
                .collect::<Vec<_>>()
                .join("\n")
        ),
    }
}

/// Generate mock findings
fn generate_mock_findings(prompt: &SummarizationPrompt) -> Vec<String> {
    let mut findings = Vec::new();

    if prompt.statistics.container_count > 5 {
        findings.push("Complex multi-container architecture detected".to_string());
    }

    if !prompt.statistics.technologies.is_empty() {
        findings.push(format!(
            "Uses {} distinct technologies",
            prompt.statistics.technologies.len()
        ));
    }

    if prompt.risks.len() > 3 {
        findings.push(format!(
            "High risk profile with {} identified risks",
            prompt.risks.len()
        ));
    }

    if findings.is_empty() {
        findings.push("Architecture appears well-structured".to_string());
    }

    findings
}

/// Generate mock suggestions
fn generate_mock_suggestions(prompt: &SummarizationPrompt) -> Vec<String> {
    let mut suggestions = Vec::new();

    // Check for missing technologies
    if prompt.statistics.technologies.is_empty() {
        suggestions.push("Consider documenting technology stack for each container".to_string());
    }

    // Check for relationship density
    let rel_density = if prompt.statistics.container_count > 0 {
        prompt.statistics.relationship_count as f64 / prompt.statistics.container_count as f64
    } else {
        0.0
    };

    if rel_density > 5.0 {
        suggestions.push("High relationship density detected - consider refactoring to reduce coupling".to_string());
    }

    // Check for risk severity
    let has_critical = prompt.risks.iter().any(|r| r.severity == super::RiskSeverity::Critical);
    if has_critical {
        suggestions.push("Critical risks identified - prioritize remediation".to_string());
    }

    if suggestions.is_empty() {
        suggestions.push("Architecture follows best practices".to_string());
    }

    suggestions
}

#[cfg(test)]
mod tests {
    use super::{SummarizationPrompt, LlmSummarizer, MockLlmSummarizer, SummaryStyle, DiagramStatistics};
    use crate::summarization::{ArchitectureRisk, RiskSeverity};

    fn create_test_prompt(style: SummaryStyle) -> SummarizationPrompt {
        SummarizationPrompt {
            style,
            statistics: DiagramStatistics {
                system_count: 2,
                container_count: 4,
                component_count: 10,
                relationship_count: 8,
                person_count: 1,
                technologies: vec!["Rust".to_string(), "PostgreSQL".to_string()],
            },
            workspace_json: r#"{"name":"Test"}"#.to_string(),
            risks: vec![
                ArchitectureRisk {
                    id: "RISK-001".to_string(),
                    severity: RiskSeverity::Medium,
                    description: "Missing technology documentation".to_string(),
                    affected_elements: vec!["container-1".to_string()],
                    recommendation: "Add technology labels".to_string(),
                },
            ],
        }
    }

    #[tokio::test]
    async fn test_mock_summarizer_executive() {
        let mock = MockLlmSummarizer::new();
        let prompt = create_test_prompt(SummaryStyle::Executive);

        let result = mock.summarize(prompt).await.unwrap();

        assert!(!result.text.is_empty());
        assert!(result.text.contains("architecture"));
        assert!(!result.findings.is_empty());
    }

    #[tokio::test]
    async fn test_mock_summarizer_technical() {
        let mock = MockLlmSummarizer::new();
        let prompt = create_test_prompt(SummaryStyle::Technical);

        let result = mock.summarize(prompt).await.unwrap();

        assert!(result.text.contains("Technical Analysis"));
        assert!(result.text.contains("Rust"));
    }

    #[tokio::test]
    async fn test_mock_summarizer_risk() {
        let mock = MockLlmSummarizer::new();
        let prompt = create_test_prompt(SummaryStyle::RiskAssessment);

        let result = mock.summarize(prompt).await.unwrap();

        assert!(result.text.contains("Risk Assessment"));
        assert!(result.text.contains("Medium Risks: 1"));
    }

    #[tokio::test]
    async fn test_mock_summarizer_error() {
        let mock = MockLlmSummarizer::new().with_error();
        let prompt = create_test_prompt(SummaryStyle::Executive);

        let result = mock.summarize(prompt).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_mock_summarizer_provider_name() {
        let mock = MockLlmSummarizer::new();
        assert_eq!(mock.provider_name(), "mock-llm");

        let custom = MockLlmSummarizer::new().with_provider_name("custom-llm");
        assert_eq!(custom.provider_name(), "custom-llm");
    }

    #[tokio::test]
    async fn test_findings_generation() {
        let mock = MockLlmSummarizer::new();
        let prompt = create_test_prompt(SummaryStyle::Executive);

        let result = mock.summarize(prompt).await.unwrap();

        // With container_count=4 and technologies=["Rust", "PostgreSQL"],
        // should generate "Uses 2 distinct technologies" finding
        assert!(!result.findings.is_empty(), "Expected non-empty findings");
        // Check that at least one finding mentions technologies
        assert!(
            result.findings.iter().any(|f| f.contains("distinct technologies")),
            "Expected finding about distinct technologies, got: {:?}",
            result.findings
        );
    }

    #[tokio::test]
    async fn test_suggestions_generation() {
        let mock = MockLlmSummarizer::new();
        let prompt = create_test_prompt(SummaryStyle::RiskAssessment);

        let result = mock.summarize(prompt).await.unwrap();

        // Should have suggestions
        assert!(!result.suggestions.is_empty());
    }
}
