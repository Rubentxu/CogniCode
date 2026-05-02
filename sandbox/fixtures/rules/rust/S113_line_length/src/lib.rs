/// Function with a very long line (over 140 characters) - triggers S113
pub fn long_line_function() -> String {
    let very_long_string_that_exceeds_the_maximum_line_length_and_causes_a_violation = "This is a very long string that exceeds the maximum line length allowed by the rule and should trigger S113".to_string();
    very_long_string_that_exceeds_the_maximum_line_length_and_causes_a_violation
}
