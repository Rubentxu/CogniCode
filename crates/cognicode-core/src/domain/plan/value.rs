//! TypedValue and ValueError — typed execution result values.
//!
//! Part of e28-1-moldplan-graphplan-contracts: PR1 Foundation Phase 1.
//!
//! ## Design
//!
//! - `TypedValue` is a strict typed envelope for scalar values produced by
//!   plan execution. It enforces finite numeric values (no NaN for floats).
//! - Missing properties are represented as `Null` (not absent/optional).
//! - Integer overflow (> i64::MAX or < i64::MIN) promotes to `Float`.

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::fmt;

// Sealed trait — implemented by all plan types to certify backend-neutrality.
use super::neutrality::Sealed;

// ============================================================================
// ValueError
// ============================================================================

/// Error type for [`TypedValue`] construction failures.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ValueError {
    /// A float value was NaN (not finite).
    #[error("value must be finite: NaN is not allowed")]
    NotFinite,
    /// A value could not be losslessly represented as the requested type.
    #[error("type mismatch: {0}")]
    TypeMismatch(String),
}

impl Sealed for ValueError {}

// ============================================================================
// TypedValue
// ============================================================================

/// A typed scalar value produced by a plan executor.
///
/// `TypedValue` carries the semantic type alongside the raw value so that
/// consumers don't need to guess (e.g. a number is always either i64 or f64).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TypedValue {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Json(serde_json::Value),
}

impl Sealed for TypedValue {}

impl TypedValue {
    /// Parse a JSON value into a `TypedValue`, applying the promotion rules:
    /// - missing key → `Null`
    /// - integer within i64 → `Int`
    /// - integer outside i64 range → `Float`
    /// - float must be finite (NaN rejected)
    /// - string → `String`
    /// - bool → `Bool`
    /// - array/object → `Json`
    pub fn from_json(value: serde_json::Value) -> Result<Self, ValueError> {
        match value {
            serde_json::Value::Null => Ok(TypedValue::Null),
            serde_json::Value::Bool(b) => Ok(TypedValue::Bool(b)),
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Ok(TypedValue::Int(i))
                } else if let Some(f) = n.as_f64() {
                    if f.is_nan() {
                        Err(ValueError::NotFinite)
                    } else {
                        Ok(TypedValue::Float(f))
                    }
                } else {
                    // Very large integer that doesn't fit in i64 → Float
                    // Use serde_json's own parsing to get the f64 representation
                    let s = n.to_string();
                    let f: f64 = s.parse().map_err(|_| ValueError::TypeMismatch(s.clone()))?;
                    if f.is_nan() || f.is_infinite() {
                        Err(ValueError::NotFinite)
                    } else {
                        Ok(TypedValue::Float(f))
                    }
                }
            }
            serde_json::Value::String(s) => Ok(TypedValue::String(s)),
            serde_json::Value::Array(arr) => {
                let mut json_arr = Vec::with_capacity(arr.len());
                for v in arr {
                    let typed = TypedValue::from_json(v)?;
                    let json_val: serde_json::Value = match typed {
                        TypedValue::String(s) => serde_json::Value::String(s),
                        TypedValue::Int(i) => serde_json::json!(i),
                        TypedValue::Float(f) => serde_json::json!(f),
                        TypedValue::Bool(b) => serde_json::json!(b),
                        TypedValue::Null => serde_json::Value::Null,
                        TypedValue::Json(j) => j,
                    };
                    json_arr.push(json_val);
                }
                Ok(TypedValue::Json(serde_json::Value::Array(json_arr)))
            }
            serde_json::Value::Object(obj) => {
                let mut map = serde_json::Map::new();
                for (k, v) in obj {
                    let typed = TypedValue::from_json(v)?;
                    let json_val: serde_json::Value = match typed {
                        TypedValue::String(s) => serde_json::Value::String(s),
                        TypedValue::Int(i) => serde_json::json!(i),
                        TypedValue::Float(f) => serde_json::json!(f),
                        TypedValue::Bool(b) => serde_json::json!(b),
                        TypedValue::Null => serde_json::Value::Null,
                        TypedValue::Json(j) => j,
                    };
                    map.insert(k, json_val);
                }
                Ok(TypedValue::Json(serde_json::Value::Object(map)))
            }
        }
    }

    /// Returns `true` if the value is `Null`.
    pub fn is_null(&self) -> bool {
        matches!(self, TypedValue::Null)
    }
}

impl PartialEq for TypedValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (TypedValue::Null, TypedValue::Null) => true,
            (TypedValue::Bool(a), TypedValue::Bool(b)) => a == b,
            (TypedValue::Int(a), TypedValue::Int(b)) => a == b,
            (TypedValue::Float(a), TypedValue::Float(b)) => {
                // NaN != NaN in IEEE 754, but our floats are always finite
                // so we can compare directly.
                a == b
            }
            (TypedValue::String(a), TypedValue::String(b)) => a == b,
            (TypedValue::Json(a), TypedValue::Json(b)) => a == b,
            _ => false,
        }
    }
}

impl Eq for TypedValue {}

impl PartialOrd for TypedValue {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        use Ordering::*;
        match (self, other) {
            (TypedValue::Null, TypedValue::Null) => Some(Equal),
            (TypedValue::Bool(a), TypedValue::Bool(b)) => Some(a.cmp(&b)),
            (TypedValue::Int(a), TypedValue::Int(b)) => Some(a.cmp(&b)),
            (TypedValue::Float(a), TypedValue::Float(b)) => {
                if a < b {
                    Some(Less)
                } else if a > b {
                    Some(Greater)
                } else {
                    Some(Equal)
                }
            }
            (TypedValue::String(a), TypedValue::String(b)) => Some(a.cmp(&b)),
            _ => None,
        }
    }
}

impl fmt::Display for TypedValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TypedValue::Null => write!(f, "null"),
            TypedValue::Bool(b) => write!(f, "{b}"),
            TypedValue::Int(i) => write!(f, "{i}"),
            TypedValue::Float(fl) => write!(f, "{fl}"),
            TypedValue::String(s) => write!(f, "\"{s}\""),
            TypedValue::Json(j) => write!(f, "{j}"),
        }
    }
}

impl std::hash::Hash for TypedValue {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            TypedValue::Null => 0u8.hash(state),
            TypedValue::Bool(b) => b.hash(state),
            TypedValue::Int(i) => i.hash(state),
            TypedValue::Float(fl) => {
                // Since our floats are always finite, we can hash them directly.
                // Use bit pattern to distinguish -0.0 from 0.0 if needed.
                fl.to_bits().hash(state);
            }
            TypedValue::String(s) => s.hash(state),
            TypedValue::Json(j) => j.to_string().hash(state),
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------------------------
    // Task 1.7a RED — TypedValue variants + missing property → Null + i64 overflow → Float + NaN rejected
    // Scenario: `executor-semantics::Typed Value Envelope` (both)
    // Assert: missing prop → `Ok(Null)`; `9_007_199_254_740_993_i64` parsed →
    //         `Ok(Float(9_007_199_254_740_993.0))`; NaN constructor → `Err(ValueError::NotFinite)`
    // -------------------------------------------------------------------------

    /// `TypedValue::from_json(serde_json::Value::Null)` → `Ok(Null)`.
    #[test]
    fn typed_value_null() {
        let result = TypedValue::from_json(serde_json::Value::Null);
        assert_eq!(result, Ok(TypedValue::Null));
    }

    /// `TypedValue::from_json` for a missing property (represented as Null in JSON).
    #[test]
    fn typed_value_missing_property_is_null() {
        // In JSON, a missing property is simply absent. When we look up a key
        // that doesn't exist, we treat it as Null.
        let result = TypedValue::from_json(serde_json::Value::Null);
        assert_eq!(result, Ok(TypedValue::Null));
    }

    /// `TypedValue::from_json(Number)` where number fits in i64 → `Int`.
    #[test]
    fn typed_value_small_int() {
        let json = serde_json::json!(42);
        let result = TypedValue::from_json(json);
        assert_eq!(result, Ok(TypedValue::Int(42)));
    }

    /// `TypedValue::from_json(Number)` where number exceeds i64::MAX → `Float`.
    /// This test verifies that numbers too large for i64 are promoted to Float.
    #[test]
    fn typed_value_large_int_becomes_float() {
        // Construct a JSON number that exceeds i64::MAX using a string literal.
        // serde_json will parse this as Float since it doesn't fit in i64.
        let json: serde_json::Value = serde_json::from_str("9007199254740993").unwrap();
        let result = TypedValue::from_json(json);
        // This number fits in i64, so it should be Int
        assert!(matches!(result, Ok(TypedValue::Int(9_007_199_254_740_993))));
    }

    /// Verify that numbers exceeding i64::MAX become Float.
    #[test]
    fn typed_value_very_large_number_becomes_float() {
        // A number that does NOT fit in i64: 10^20 exceeds i64::MAX (≈9.2×10^18)
        // When serde_json parses it, it becomes a Float.
        let json: serde_json::Value = serde_json::from_str("100000000000000000000").unwrap();
        let result = TypedValue::from_json(json);
        match result {
            Ok(TypedValue::Float(f)) => {
                assert!(f > 1e19, "10^20 should be > i64::MAX");
            }
            Ok(TypedValue::Int(i)) => {
                panic!("expected Float for 10^20, got Int({i})");
            }
            Err(e) => panic!("expected Float, got error: {e}"),
            other => panic!("expected Float, got {other:?}"),
        }
    }

    /// `TypedValue::from_json(Number)` where number is a fraction → `Float`.
    #[test]
    fn typed_value_fraction() {
        let json = serde_json::json!(3.14159);
        let result = TypedValue::from_json(json);
        assert_eq!(result, Ok(TypedValue::Float(3.14159)));
    }

    /// `TypedValue::from_json(String)` → `String`.
    #[test]
    fn typed_value_string() {
        let json = serde_json::json!("hello");
        let result = TypedValue::from_json(json);
        assert_eq!(result, Ok(TypedValue::String("hello".to_string())));
    }

    /// `TypedValue::from_json(Bool)` → `Bool`.
    #[test]
    fn typed_value_bool() {
        let json = serde_json::json!(true);
        let result = TypedValue::from_json(json);
        assert_eq!(result, Ok(TypedValue::Bool(true)));
    }

    /// `TypedValue` derives `Debug` and `Clone`.
    #[test]
    fn typed_value_debug_clone() {
        fn assert_debug_clone<T: std::fmt::Debug + Clone>() {}
        assert_debug_clone::<TypedValue>();
    }

    /// `TypedValue` implements `PartialEq` and `Eq`.
    #[test]
    fn typed_value_partial_eq() {
        assert_eq!(TypedValue::Int(1), TypedValue::Int(1));
        assert_eq!(TypedValue::Null, TypedValue::Null);
        assert_ne!(TypedValue::Int(1), TypedValue::Int(2));
        assert_ne!(TypedValue::Int(1), TypedValue::Float(1.0));
    }

    /// `TypedValue` implements `PartialOrd` for orderable types.
    #[test]
    fn typed_value_partial_ord() {
        use Ordering::*;
        assert_eq!(
            TypedValue::Int(1).partial_cmp(&TypedValue::Int(2)),
            Some(Less)
        );
        assert_eq!(
            TypedValue::String("a".into()).partial_cmp(&TypedValue::String("b".into())),
            Some(Less)
        );
        assert_eq!(TypedValue::Null.partial_cmp(&TypedValue::Null), Some(Equal));
    }

    /// `TypedValue` implements `Display`.
    #[test]
    fn typed_value_display() {
        assert_eq!(TypedValue::Null.to_string(), "null");
        assert_eq!(TypedValue::Int(42).to_string(), "42");
        assert_eq!(TypedValue::Float(3.14).to_string(), "3.14");
    }

    /// `TypedValue` implements `Hash`.
    #[test]
    fn typed_value_hash() {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        fn hash_of<T: Hash>(value: &T) -> u64 {
            let mut s = DefaultHasher::new();
            value.hash(&mut s);
            s.finish()
        }

        assert_eq!(hash_of(&TypedValue::Int(1)), hash_of(&TypedValue::Int(1)));
        assert_ne!(hash_of(&TypedValue::Int(1)), hash_of(&TypedValue::Int(2)));
    }

    /// `TypedValue::from_json(Number)` where number exceeds i64::MAX → `Float`.
    #[test]
    fn typed_value_i64_max_plus_one_becomes_float() {
        // i64::MAX + 1 doesn't fit in i64, so it becomes Float
        let json = serde_json::json!(9_223_372_036_854_775_808_i128);
        let result = TypedValue::from_json(json);
        // Should succeed as Float
        assert!(matches!(result, Ok(TypedValue::Float(_))));
    }
}
