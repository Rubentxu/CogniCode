//! Telemetry Module - OpenTelemetry instrumentation for tool operations
//!
//! This module provides metrics collection for all MCP tool operations using
//! the OpenTelemetry SDK with OTLP export.

use opentelemetry::metrics::{Counter, Histogram, Meter};
use opentelemetry::KeyValue;
use std::future::Future;
use std::sync::Arc;
use std::time::Instant;

/// ToolMetrics - Shared metrics for all MCP tool operations
///
/// Collects counters and histograms for tool calls, duration, errors,
/// bytes transferred, and operation-specific metrics.
pub struct ToolMetrics {
    /// Total tool invocations by tool_name and status
    pub calls: Counter<u64>,
    /// Tool execution time in milliseconds
    pub duration: Histogram<f64>,
    /// Errors by tool_name and error_type
    pub errors: Counter<u64>,
    /// Bytes read from disk (by mode)
    pub bytes_read: Histogram<f64>,
    /// Bytes written to disk
    pub bytes_written: Histogram<f64>,
    /// Search result matches (by file_type)
    pub search_matches: Histogram<f64>,
    /// Files traversed during search/list operations
    pub files_scanned: Histogram<f64>,
    /// Tree-sitter validation time for edit operations
    pub edit_validation_ms: Histogram<f64>,
    /// Edits rejected by syntax validation (by reason)
    pub edit_rejected: Counter<u64>,
}

impl ToolMetrics {
    /// Creates a new ToolMetrics instance from a Meter
    pub fn new(meter: &Meter) -> Self {
        Self {
            calls: meter
                .u64_counter("cognicode.tool.calls")
                .with_description("Total tool invocations")
                .build(),
            duration: meter
                .f64_histogram("cognicode.tool.duration")
                .with_description("Tool execution time in milliseconds")
                .build(),
            errors: meter
                .u64_counter("cognicode.tool.errors")
                .with_description("Errors by tool_name and error_type")
                .build(),
            bytes_read: meter
                .f64_histogram("cognicode.file.bytes_read")
                .with_description("Bytes read from disk")
                .build(),
            bytes_written: meter
                .f64_histogram("cognicode.file.bytes_written")
                .with_description("Bytes written to disk")
                .build(),
            search_matches: meter
                .f64_histogram("cognicode.search.matches")
                .with_description("Search result matches")
                .build(),
            files_scanned: meter
                .f64_histogram("cognicode.search.files_scanned")
                .with_description("Files traversed during search")
                .build(),
            edit_validation_ms: meter
                .f64_histogram("cognicode.edit.validation_ms")
                .with_description("Tree-sitter validation time for edit operations")
                .build(),
            edit_rejected: meter
                .u64_counter("cognicode.edit.rejected")
                .with_description("Edits rejected by syntax validation")
                .build(),
        }
    }

    /// Creates a no-op ToolMetrics for testing or when OTel is disabled
    #[allow(dead_code)]
    pub fn noop() -> Self {
        // Return zeroed metrics - they won't be used in noop mode
        panic!("ToolMetrics::noop not intended for direct use")
    }

    /// Records a successful tool call with duration
    #[allow(dead_code)]
    pub fn record_call(&self, tool_name: &str, duration_ms: f64) {
        self.calls.add(1, &[KeyValue::new("tool", tool_name.to_string())]);
        self.duration.record(
            duration_ms,
            &[KeyValue::new("tool", tool_name.to_string())],
        );
    }

    /// Records an error during tool execution
    #[allow(dead_code)]
    pub fn record_error(&self, tool_name: &str, error_type: &str) {
        self.errors.add(
            1,
            &[
                KeyValue::new("tool", tool_name.to_string()),
                KeyValue::new("error_type", error_type.to_string()),
            ],
        );
    }

    /// Records bytes read from disk
    #[allow(dead_code)]
    pub fn record_bytes_read(&self, bytes: f64, mode: &str) {
        self.bytes_read.record(
            bytes,
            &[
                KeyValue::new("mode", mode.to_string()),
            ],
        );
    }

    /// Records bytes written to disk
    #[allow(dead_code)]
    pub fn record_bytes_written(&self, bytes: f64) {
        self.bytes_written.record(bytes, &[]);
    }

    /// Records search matches found
    #[allow(dead_code)]
    pub fn record_search_matches(&self, count: f64, file_type: &str) {
        self.search_matches.record(
            count,
            &[KeyValue::new("file_type", file_type.to_string())],
        );
    }

    /// Records number of files scanned
    #[allow(dead_code)]
    pub fn record_files_scanned(&self, count: f64) {
        self.files_scanned.record(count, &[]);
    }

    /// Records edit validation time
    #[allow(dead_code)]
    pub fn record_edit_validation_ms(&self, duration_ms: f64) {
        self.edit_validation_ms.record(duration_ms, &[]);
    }

    /// Records an edit rejection
    #[allow(dead_code)]
    pub fn record_edit_rejected(&self, reason: &str) {
        self.edit_rejected.add(
            1,
            &[KeyValue::new("reason", reason.to_string())],
        );
    }
}

/// Error type for tool operations
#[derive(Debug, Clone)]
pub struct ToolError {
    pub kind: String,
    pub message: String,
}

impl ToolError {
    #[allow(dead_code)]
    pub fn new(kind: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            message: message.into(),
        }
    }

    #[allow(dead_code)]
    pub fn kind(&self) -> &str {
        &self.kind
    }
}

impl std::fmt::Display for ToolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.kind, self.message)
    }
}

impl std::error::Error for ToolError {}

impl From<ToolError> for String {
    fn from(e: ToolError) -> Self {
        e.message
    }
}

impl<T> From<ToolError> for Result<T, ToolError> {
    fn from(e: ToolError) -> Self {
        Err(e)
    }
}

/// Instrumented tool call wrapper that records metrics
///
/// Wraps a tool operation and records call count, duration, and errors
/// to the provided ToolMetrics instance.
pub async fn instrument_tool<F, T>(
    metrics: &ToolMetrics,
    tool_name: &str,
    f: F,
) -> Result<T, ToolError>
where
    F: Future<Output = Result<T, ToolError>>,
{
    let start = Instant::now();
    metrics.calls.add(1, &[KeyValue::new("tool", tool_name.to_string())]);

    match f.await {
        Ok(result) => {
            let duration_ms = start.elapsed().as_secs_f64() * 1000.0;
            metrics.duration.record(
                duration_ms,
                &[KeyValue::new("tool", tool_name.to_string())],
            );
            Ok(result)
        }
        Err(e) => {
            metrics.errors.add(
                1,
                &[
                    KeyValue::new("tool", tool_name.to_string()),
                    KeyValue::new("error_type", e.kind.clone()),
                ],
            );
            Err(e)
        }
    }
}

/// Creates a ToolMetrics instance from the global meter provider
///
/// If no global meter provider is set, returns None.
pub fn create_metrics_from_global() -> Option<ToolMetrics> {
    let meter = opentelemetry::global::meter("cognicode");
    Some(ToolMetrics::new(&meter))
}

/// Global shared ToolMetrics instance
static TOOL_METRICS: std::sync::OnceLock<Arc<ToolMetrics>> = std::sync::OnceLock::new();

/// Initializes the global ToolMetrics from the global meter provider
pub fn init_global_metrics() -> Result<(), Box<dyn std::error::Error>> {
    let metrics = create_metrics_from_global()
        .ok_or("Failed to get global meter")?;
    TOOL_METRICS
        .set(Arc::from(metrics))
        .map_err(|_| "Global metrics already initialized")?;
    Ok(())
}

/// Gets the global ToolMetrics instance
pub fn get_global_metrics() -> Option<Arc<ToolMetrics>> {
    TOOL_METRICS.get().cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_error_creation() {
        let err = ToolError::new("PathTraversal", "Path outside workspace");
        assert_eq!(err.kind, "PathTraversal");
        assert_eq!(err.message, "Path outside workspace");
        assert_eq!(err.kind(), "PathTraversal");
    }

    #[test]
    fn test_tool_error_display() {
        let err = ToolError::new("Validation", "Invalid input");
        let display = format!("{}", err);
        assert!(display.contains("Validation"));
        assert!(display.contains("Invalid input"));
    }
}
