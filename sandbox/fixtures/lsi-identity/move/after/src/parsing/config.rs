pub fn parse_config(text: &str) -> u32 {
    read_config(text);
    split_config(text);
    merge_config(text);
    validate_config(text);
    0
}
