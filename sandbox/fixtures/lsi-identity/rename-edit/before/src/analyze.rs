pub fn analyze_rows(rows: &[Row]) -> u32 {
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
    compact_rows(rows);
    emit_rows(rows);
    0
}

pub fn legacy_rows(rows: &[Row]) -> u32 {
    archive_rows(rows);
    0
}
