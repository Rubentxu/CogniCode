//! Template utilities for summarization
//!
//! This module provides template-based summarization as an alternative to LLM-based summarization.

use super::{DiagramStatistics, SummaryStyle};

/// Generate a section from a template
pub fn fill_template(template: &str, variables: &[(&str, &str)]) -> String {
    let mut result = template.to_string();
    for (key, value) in variables {
        result = result.replace(&format!("{{{}}}", key), value);
    }
    result
}

/// Generate executive summary template
pub fn executive_template() -> &'static str {
    r#"# Executive Summary: {system_name}

{intro_paragraph}

## System Overview
{system_overview}

## Key Containers
{container_summary}

## Critical Relationships
{relationships_summary}

## Statistics
- Systems: {system_count}
- Containers: {container_count}
- Components: {component_count}
- Relationships: {relationship_count}
"#
}

/// Generate technical summary template
pub fn technical_template() -> &'static str {
    r#"# Technical Architecture Summary: {system_name}

## Overview
{description}

## Component Statistics
| Type | Count |
|------|-------|
| Systems | {system_count} |
| Containers | {container_count} |
| Components | {component_count} |
| Relationships | {relationship_count} |

## Technologies Used
{technologies}

## System Details
{system_details}

## Relationship Map
{relationships}
"#
}

/// Generate risk assessment template
pub fn risk_template() -> &'static str {
    r#"# Risk Assessment: {system_name}

## Risk Overview
| Severity | Count |
|----------|-------|
| Critical | {critical_count} |
| High | {high_count} |
| Medium | {medium_count} |
| Low | {low_count} |

## Risk Details
{risk_details}

## Overall Assessment
{overall_assessment}

## Recommendations
{recommendations}
"#
}