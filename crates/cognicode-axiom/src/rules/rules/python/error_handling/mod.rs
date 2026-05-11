//! Python Error Handling Rules
//!
//! This module contains Python-specific error handling rules for detecting
//! problematic patterns in exception handling code.

pub mod s108_rule;       // Empty except block
pub mod s1121_rule;     // Raise generic Exception
pub mod s1130_rule;     // Raise in finally
pub mod s1141_rule;     // Nested try-except (>2)
pub mod s1160_rule;     // Public function raises generic exception
pub mod s1162_rule;     // Exception class naming
pub mod s1164_rule;     // Catch-all except
pub mod s2737_rule;     // except with pass
pub mod s2225_rule;     // Exception message not informative
pub mod s2226_rule;     // Logging exception without traceback
pub mod s2227_rule;     // Raising Exception without message
pub mod s2228_rule;     // Raising string (Python 2 style)
pub mod s2701_rule;     // assert with literal
pub mod s3415_rule;     // assert arg order
pub mod s1122_rule;     // Fallthrough in except

pub use s108_rule::PY_S108Rule;
pub use s1121_rule::PY_S1121Rule;
pub use s1130_rule::PY_S1130Rule;
pub use s1141_rule::PY_S1141Rule;
pub use s1160_rule::PY_S1160Rule;
pub use s1162_rule::PY_S1162Rule;
pub use s1164_rule::PY_S1164Rule;
pub use s2737_rule::PY_S2737Rule;
pub use s2225_rule::PY_S2225Rule;
pub use s2226_rule::PY_S2226Rule;
pub use s2227_rule::PY_S2227Rule;
pub use s2228_rule::PY_S2228Rule;
pub use s2701_rule::PY_S2701Rule;
pub use s3415_rule::PY_S3415Rule;
pub use s1122_rule::PY_S1122Rule;
