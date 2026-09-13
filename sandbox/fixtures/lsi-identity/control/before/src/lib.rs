pub fn run_pipeline(rows: &[Row]) -> u32 {
    prepare_rows(rows);
    validate_rows(rows);
    summarize_rows(rows);
    publish_rows(rows);
    0
}

pub fn audit_pipeline(rows: &[Row]) -> u32 {
    inspect_rows(rows);
    1
}
