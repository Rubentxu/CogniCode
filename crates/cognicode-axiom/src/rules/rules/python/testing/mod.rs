//! Python Testing Rules
//!
//! This module contains Python-specific rules for testing code quality and best practices.

pub mod t1_rule;       // Test without assertion
pub mod t2_rule;       // Test with time.sleep()
pub mod t3_rule;       // assertEqual vs assertTrue misuse
pub mod t4_rule;       // setUp/tearDown vs setUpClass/tearDownClass misuse
pub mod t5_rule;       // unittest.skip without reason
pub mod t6_rule;       // Test method not starting with test_
pub mod t7_rule;       // Test fixture too complex (>20 lines setup)
pub mod t8_rule;       // Multiple asserts in one test
pub mod t9_rule;       // Test using random (non-deterministic)
pub mod t10_rule;      // Duplicated test method
pub mod t11_rule;      // broad except in tests
pub mod t12_rule;      // print() in tests
pub mod t13_rule;      // hardcoded temp file path
pub mod t14_rule;      // missing cleanup for temp/monkeypatch resources
pub mod t15_rule;      // network call in unit test

pub use t1_rule::PY_T1Rule;
pub use t2_rule::PY_T2Rule;
pub use t3_rule::PY_T3Rule;
pub use t4_rule::PY_T4Rule;
pub use t5_rule::PY_T5Rule;
pub use t6_rule::PY_T6Rule;
pub use t7_rule::PY_T7Rule;
pub use t8_rule::PY_T8Rule;
pub use t9_rule::PY_T9Rule;
pub use t10_rule::PY_T10Rule;
pub use t11_rule::PY_T11Rule;
pub use t12_rule::PY_T12Rule;
pub use t13_rule::PY_T13Rule;
pub use t14_rule::PY_T14Rule;
pub use t15_rule::PY_T15Rule;
