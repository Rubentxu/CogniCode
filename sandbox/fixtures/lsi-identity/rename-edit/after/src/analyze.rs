pub fn process_rows(rows: &[Row]) -> u32 {
    normalize_rows(rows);
    dedupe_rows(rows);
    sort_rows(rows);
    filter_rows(rows);
    join_rows(rows);
    group_rows(rows);
    aggregate_rows(rows);
    score_rows(rows);
    rank_rows(rows);
    merge_rows(rows);
    stamp_rows(rows);
    emit_rows(rows);
    0
}

pub fn audit_rows(rows: &[Row]) -> u32 {
    inspect_rows(rows);
    0
}
