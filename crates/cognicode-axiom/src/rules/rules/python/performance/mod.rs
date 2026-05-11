//! Python Performance Rules
//!
//! This module contains Python-specific performance rules for detecting
//! inefficient patterns that impact execution speed and resource usage.

pub mod p1_range_len_rule;      // range(len(x)) instead of enumerate
pub mod p2_keys_iteration_rule;  // keys() iteration instead of direct dict iteration
pub mod p3_map_filter_lambda_rule; // map/filter with lambda instead of comprehension
pub mod p4_list_append_rule;     // list.append in loop instead of comprehension
pub mod p5_string_concat_rule;   // + string concat in loop instead of join
pub mod p6_time_sleep_rule;      // time.sleep() in test
pub mod p7_global_abuse_rule;    // global keyword abuse
pub mod p8_del_list_element_rule; // del on list element (O(n))
pub mod p9_in_list_rule;         // in on list instead of set
pub mod p10_class_attr_rule;     // Class-level attribute instead of instance
pub mod p11_repeated_list_conv_rule; // Repeated list(set(x)) conversion
pub mod p12_unnecessary_deepcopy_rule; // Unnecessary deepcopy
pub mod p13_repeated_regex_rule; // Repeated regex compile in loop
pub mod p14_len_zero_check_rule; // len() == 0 instead of not x
pub mod p15_repeated_format_rule; // Repeated string format calls

pub use p1_range_len_rule::PY_P1Rule;
pub use p2_keys_iteration_rule::PY_P2Rule;
pub use p3_map_filter_lambda_rule::PY_P3Rule;
pub use p4_list_append_rule::PY_P4Rule;
pub use p5_string_concat_rule::PY_P5Rule;
pub use p6_time_sleep_rule::PY_P6Rule;
pub use p7_global_abuse_rule::PY_P7Rule;
pub use p8_del_list_element_rule::PY_P8Rule;
pub use p9_in_list_rule::PY_P9Rule;
pub use p10_class_attr_rule::PY_P10Rule;
pub use p11_repeated_list_conv_rule::PY_P11Rule;
pub use p12_unnecessary_deepcopy_rule::PY_P12Rule;
pub use p13_repeated_regex_rule::PY_P13Rule;
pub use p14_len_zero_check_rule::PY_P14Rule;
pub use p15_repeated_format_rule::PY_P15Rule;
