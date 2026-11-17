//! Helm values: layering, lookup and a canonical digest.
//!
//! Values are plain JSON documents. Layering follows Helm's coalescing: maps
//! merge key by key, scalars and lists from the upper layer replace the lower
//! ones, and an explicit `null` in the upper layer deletes the key. The digest
//! names the exact effective values a manifest was produced from.

use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

/// Merge `upper` over `base` the way Helm coalesces values.
pub fn merge_values(base: &Value, upper: &Value) -> Value {
    match (base, upper) {
        (Value::Object(base), Value::Object(upper)) => {
            let mut merged: Map<String, Value> = base.clone();
            for (key, value) in upper {
                if value.is_null() {
                    merged.remove(key);
                    continue;
                }
                let next = match merged.get(key) {
                    Some(existing) => merge_values(existing, value),
                    None => strip_nulls(value),
                };
                merged.insert(key.clone(), next);
            }
            Value::Object(merged)
        }
        (_, upper) => strip_nulls(upper),
    }
}

fn strip_nulls(value: &Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.iter()
                .filter(|(_, v)| !v.is_null())
                .map(|(k, v)| (k.clone(), strip_nulls(v)))
                .collect(),
        ),
        other => other.clone(),
    }
}

/// Fold `layers` over `defaults`, lowest priority first.
pub fn effective_values(defaults: &Value, layers: &[Value]) -> Value {
    layers.iter().fold(strip_nulls(defaults), |acc, layer| {
        merge_values(&acc, layer)
    })
}

/// Look up a dotted path (`image.tag`). `None` means the path is not set.
pub fn lookup<'a>(values: &'a Value, path: &str) -> Option<&'a Value> {
    path.split('.')
        .try_fold(values, |node, key| node.as_object()?.get(key))
}

/// A stable digest of a values document, independent of key order.
pub fn values_digest(values: &Value) -> String {
    let mut canonical = String::new();
    write_canonical(values, &mut canonical);
    let digest = Sha256::digest(canonical.as_bytes());
    let hex: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    format!("sha256:{hex}")
}

fn write_canonical(value: &Value, out: &mut String) {
    match value {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            out.push('{');
            for key in keys {
                out.push_str(&Value::String(key.clone()).to_string());
                out.push(':');
                write_canonical(&map[key], out);
                out.push(',');
            }
            out.push('}');
        }
        Value::Array(items) => {
            out.push('[');
            for item in items {
                write_canonical(item, out);
                out.push(',');
            }
            out.push(']');
        }
        scalar => out.push_str(&scalar.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn maps_merge_and_scalars_and_lists_replace() {
        let base = json!({"image": {"repo": "a", "tag": "1"}, "ports": [1, 2], "x": 1});
        let upper = json!({"image": {"tag": "2"}, "ports": [3]});
        assert_eq!(
            merge_values(&base, &upper),
            json!({"image": {"repo": "a", "tag": "2"}, "ports": [3], "x": 1})
        );
    }

    #[test]
    fn explicit_null_deletes_the_key_it_overrides() {
        let base = json!({"a": {"b": 1, "c": 2}});
        let upper = json!({"a": {"b": null}});
        assert_eq!(merge_values(&base, &upper), json!({"a": {"c": 2}}));
    }

    #[test]
    fn effective_values_apply_layers_in_order() {
        let defaults = json!({"replicas": 1});
        let layers = [json!({"replicas": 2}), json!({"replicas": 3})];
        assert_eq!(effective_values(&defaults, &layers), json!({"replicas": 3}));
    }

    #[test]
    fn lookup_distinguishes_unset_from_falsy() {
        let values = json!({"a": {"enabled": false}});
        assert_eq!(lookup(&values, "a.enabled"), Some(&json!(false)));
        assert_eq!(lookup(&values, "a.missing"), None);
        assert_eq!(lookup(&values, "a.enabled.deeper"), None);
    }

    #[test]
    fn digest_ignores_key_order_and_changes_with_content() {
        let a = json!({"x": 1, "y": {"p": 1, "q": 2}});
        let b = json!({"y": {"q": 2, "p": 1}, "x": 1});
        assert_eq!(values_digest(&a), values_digest(&b));
        assert_ne!(values_digest(&a), values_digest(&json!({"x": 2})));
    }
}
