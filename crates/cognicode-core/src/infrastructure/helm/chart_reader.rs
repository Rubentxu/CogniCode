//! Read a chart directory into a [`ChartModel`]. Nothing is executed and
//! nothing outside the chart root is read: symlinks are not followed.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::domain::iac::helm::{ChartDependency, ChartModel, TemplateModel, ValuesSchema};

/// Largest file the reader accepts, so a hostile chart cannot exhaust memory.
const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024;
/// Most files the reader accepts below `templates/`.
const MAX_TEMPLATES: usize = 2_000;

/// Why a chart could not be read.
#[derive(Debug, thiserror::Error)]
pub enum ChartReadError {
    /// A required or present file could not be read.
    #[error("cannot read `{path}`: {message}")]
    Io {
        /// The file.
        path: PathBuf,
        /// The OS message.
        message: String,
    },
    /// A file is not valid for its format.
    #[error("`{path}` is invalid: {message}")]
    Invalid {
        /// The file.
        path: PathBuf,
        /// What is wrong.
        message: String,
    },
    /// A file or the template set exceeds the reader's bounds.
    #[error("`{path}` exceeds the reader's limits: {message}")]
    TooLarge {
        /// The offending path.
        path: PathBuf,
        /// Which limit.
        message: String,
    },
}

/// Read the chart rooted at `root`.
///
/// # Errors
/// `Chart.yaml` missing or invalid, an unreadable or oversized file, or an
/// invalid `values.yaml` / `values.schema.json`.
pub fn read_chart(root: &Path) -> Result<ChartModel, ChartReadError> {
    let mut digest = Sha256::new();
    let chart_text = read_text(&root.join("Chart.yaml"), &mut digest, root)?;
    let chart: Value = parse_yaml(&root.join("Chart.yaml"), &chart_text)?;
    let name = text_field(&chart, "name").ok_or_else(|| ChartReadError::Invalid {
        path: root.join("Chart.yaml"),
        message: "missing `name`".into(),
    })?;
    let version = text_field(&chart, "version").unwrap_or_default();
    let default_values = match read_optional(&root.join("values.yaml"), &mut digest, root)? {
        Some(text) => non_null(parse_yaml(&root.join("values.yaml"), &text)?),
        None => Value::Object(Default::default()),
    };
    let schema = match read_optional(&root.join("values.schema.json"), &mut digest, root)? {
        Some(text) => Some(parse_schema(&root.join("values.schema.json"), &text)?),
        None => None,
    };
    let templates = read_templates(root, &mut digest)?;
    Ok(ChartModel {
        name,
        version,
        location: root.to_path_buf(),
        content_digest: format!(
            "sha256:{}",
            Sha256::finalize(digest)
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
        ),
        default_values,
        schema,
        dependencies: dependencies(&chart, root),
        templates,
    })
}

/// Read one values file (YAML) to be layered over a chart's defaults.
///
/// # Errors
/// The file is unreadable, too large or not valid YAML.
pub fn read_values_file(path: &Path) -> Result<Value, ChartReadError> {
    let mut ignored = Sha256::new();
    let text = read_text(path, &mut ignored, path.parent().unwrap_or(Path::new(".")))?;
    Ok(non_null(parse_yaml(path, &text)?))
}

fn non_null(value: Value) -> Value {
    if value.is_null() {
        Value::Object(Default::default())
    } else {
        value
    }
}

fn parse_yaml(path: &Path, text: &str) -> Result<Value, ChartReadError> {
    serde_norway::from_str(text).map_err(|error| ChartReadError::Invalid {
        path: path.to_path_buf(),
        message: error.to_string(),
    })
}

fn text_field(value: &Value, key: &str) -> Option<String> {
    match value.get(key)? {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

fn io_error(path: &Path, error: &std::io::Error) -> ChartReadError {
    ChartReadError::Io {
        path: path.to_path_buf(),
        message: error.to_string(),
    }
}

fn read_text(path: &Path, digest: &mut Sha256, root: &Path) -> Result<String, ChartReadError> {
    let meta = fs::symlink_metadata(path).map_err(|e| io_error(path, &e))?;
    if !meta.is_file() {
        return Err(ChartReadError::Io {
            path: path.to_path_buf(),
            message: "not a regular file (symlinks are not followed)".into(),
        });
    }
    if meta.len() > MAX_FILE_BYTES {
        return Err(ChartReadError::TooLarge {
            path: path.to_path_buf(),
            message: format!("{} bytes, limit {MAX_FILE_BYTES}", meta.len()),
        });
    }
    let text = fs::read_to_string(path).map_err(|e| io_error(path, &e))?;
    let relative = path.strip_prefix(root).unwrap_or(path);
    digest.update(relative.to_string_lossy().as_bytes());
    digest.update([0]);
    digest.update(text.as_bytes());
    digest.update([0]);
    Ok(text)
}

fn read_optional(
    path: &Path,
    digest: &mut Sha256,
    root: &Path,
) -> Result<Option<String>, ChartReadError> {
    match fs::symlink_metadata(path) {
        Ok(_) => read_text(path, digest, root).map(Some),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(io_error(path, &error)),
    }
}

fn parse_schema(path: &Path, text: &str) -> Result<ValuesSchema, ChartReadError> {
    let schema: Value = serde_json::from_str(text).map_err(|error| ChartReadError::Invalid {
        path: path.to_path_buf(),
        message: error.to_string(),
    })?;
    let mut required_paths = Vec::new();
    collect_required(&schema, "", &mut required_paths);
    Ok(ValuesSchema { required_paths })
}

fn collect_required(node: &Value, prefix: &str, out: &mut Vec<String>) {
    let join = |key: &str| {
        if prefix.is_empty() {
            key.to_string()
        } else {
            format!("{prefix}.{key}")
        }
    };
    for key in node
        .get("required")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
    {
        out.push(join(key));
    }
    for (key, child) in node
        .get("properties")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
    {
        collect_required(child, &join(key), out);
    }
}

fn dependencies(chart: &Value, root: &Path) -> Vec<ChartDependency> {
    chart
        .get("dependencies")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|dep| {
            let name = text_field(dep, "name")?;
            Some(ChartDependency {
                vendored: is_vendored(root, &name),
                version: text_field(dep, "version").unwrap_or_default(),
                repository: text_field(dep, "repository"),
                alias: text_field(dep, "alias"),
                name,
            })
        })
        .collect()
}

fn is_vendored(root: &Path, name: &str) -> bool {
    let Ok(entries) = fs::read_dir(root.join("charts")) else {
        return false;
    };
    entries.flatten().any(|entry| {
        let file = entry.file_name().to_string_lossy().to_string();
        file == name || (file.starts_with(&format!("{name}-")) && file.ends_with(".tgz"))
    })
}

fn read_templates(root: &Path, digest: &mut Sha256) -> Result<Vec<TemplateModel>, ChartReadError> {
    let mut files = Vec::new();
    collect_files(&root.join("templates"), &mut files)?;
    if files.len() > MAX_TEMPLATES {
        return Err(ChartReadError::TooLarge {
            path: root.join("templates"),
            message: format!("{} files, limit {MAX_TEMPLATES}", files.len()),
        });
    }
    files.sort();
    files
        .iter()
        .map(|file| {
            let text = read_text(file, digest, root)?;
            let relative = file
                .strip_prefix(root)
                .unwrap_or(file)
                .to_string_lossy()
                .replace('\\', "/");
            Ok(TemplateModel::analyze(&relative, &text))
        })
        .collect()
}

fn collect_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), ChartReadError> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(io_error(dir, &error)),
    };
    for entry in entries {
        let path = entry.map_err(|e| io_error(dir, &e))?.path();
        let meta = fs::symlink_metadata(&path).map_err(|e| io_error(&path, &e))?;
        if meta.is_dir() {
            collect_files(&path, out)?;
        } else if meta.is_file() {
            out.push(path);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(root: &Path, relative: &str, text: &str) {
        let path = root.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    fn sample() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(
            root,
            "Chart.yaml",
            "name: demo\nversion: 0.2.0\ndependencies:\n  - name: redis\n    version: 1.x\n    repository: https://example.invalid\n  - name: pg\n    version: 2.x\n    alias: db\n",
        );
        write(root, "values.yaml", "image: app:1\nreplicas: 2\n");
        write(
            root,
            "values.schema.json",
            r#"{"required":["image"],"properties":{"auth":{"required":["user"]}}}"#,
        );
        write(root, "templates/dep.yaml", "r: {{ .Values.replicas }}");
        write(
            root,
            "templates/_helpers.tpl",
            r#"{{- define "x" -}}y{{- end -}}"#,
        );
        write(
            root,
            "charts/redis/Chart.yaml",
            "name: redis\nversion: 1.0.0\n",
        );
        dir
    }

    #[test]
    fn a_chart_directory_becomes_a_static_model() {
        let dir = sample();
        let chart = read_chart(dir.path()).unwrap();
        assert_eq!(chart.name, "demo");
        assert_eq!(chart.version, "0.2.0");
        assert_eq!(chart.default_values["replicas"], 2);
        assert_eq!(
            chart.schema.unwrap().required_paths,
            vec!["image".to_string(), "auth.user".to_string()]
        );
        assert_eq!(chart.templates.len(), 2);
        assert!(chart.templates.iter().any(|t| t.is_helper));
        let vendored: Vec<(&str, bool)> = chart
            .dependencies
            .iter()
            .map(|d| (d.values_key(), d.vendored))
            .collect();
        assert_eq!(vendored, vec![("redis", true), ("db", false)]);
    }

    #[test]
    fn the_content_digest_follows_the_content() {
        let dir = sample();
        let before = read_chart(dir.path()).unwrap().content_digest;
        assert_eq!(before, read_chart(dir.path()).unwrap().content_digest);
        write(dir.path(), "templates/dep.yaml", "r: 3");
        assert_ne!(before, read_chart(dir.path()).unwrap().content_digest);
    }

    #[test]
    fn a_directory_without_chart_yaml_is_an_error_not_an_empty_chart() {
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(
            read_chart(dir.path()),
            Err(ChartReadError::Io { .. })
        ));
    }

    #[test]
    fn invalid_values_and_oversized_files_are_refused() {
        let dir = sample();
        write(dir.path(), "values.yaml", "a: [unclosed");
        assert!(matches!(
            read_chart(dir.path()),
            Err(ChartReadError::Invalid { .. })
        ));
        write(dir.path(), "values.yaml", "a: 1");
        let big = "x".repeat(MAX_FILE_BYTES as usize + 1);
        write(dir.path(), "templates/big.yaml", &big);
        assert!(matches!(
            read_chart(dir.path()),
            Err(ChartReadError::TooLarge { .. })
        ));
    }

    #[cfg(unix)]
    #[test]
    fn a_symlinked_template_is_not_followed_out_of_the_chart() {
        let outside = tempfile::tempdir().unwrap();
        write(outside.path(), "secret.yaml", "kind: Secret");
        let dir = sample();
        std::os::unix::fs::symlink(
            outside.path().join("secret.yaml"),
            dir.path().join("templates/link.yaml"),
        )
        .unwrap();
        let chart = read_chart(dir.path()).unwrap();
        assert!(
            !chart
                .templates
                .iter()
                .any(|t| t.path.ends_with("link.yaml"))
        );
    }
}
