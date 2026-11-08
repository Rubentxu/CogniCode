//! Shared JSON Schema normalization for interface contracts.
//!
//! This module is neutral: it derives schemas from interface-owned Rust types
//! without depending on a transport adapter such as MCP.

use serde_json::{Map, Value};

/// Derive a transport-ready JSON Schema for `T`.
///
/// The wire contract omits the dialect and Rust type title, inlines
/// subschemas, and removes integer `format` annotations that are not part of
/// the JSON Schema vocabulary consumed by clients.
pub fn schema_of<T: schemars::JsonSchema>() -> Map<String, Value> {
    let settings = schemars::generate::SchemaSettings::draft2020_12().with(|settings| {
        settings.inline_subschemas = true;
    });
    let schema = settings.into_generator().into_root_schema_for::<T>();
    let value = serde_json::to_value(schema).expect("schemars output is always serialisable");
    let mut object = value
        .as_object()
        .cloned()
        .expect("schemars produces a schema object");
    object.remove("$schema");
    object.remove("title");
    strip_format(&mut object);
    if !object.contains_key("properties") {
        object.insert("properties".to_string(), Value::Object(Map::new()));
    }
    object
}

fn strip_format(schema: &mut Map<String, Value>) {
    schema.remove("format");
    if let Some(properties) = schema.get_mut("properties").and_then(|p| p.as_object_mut()) {
        for property in properties.values_mut() {
            if let Some(property) = property.as_object_mut() {
                strip_format(property);
            }
        }
    }
    if let Some(items) = schema.get_mut("items").and_then(|i| i.as_object_mut()) {
        strip_format(items);
    }
}
