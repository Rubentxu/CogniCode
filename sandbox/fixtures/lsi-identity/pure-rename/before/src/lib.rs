pub fn fetch_total(rows: &[Row]) -> u32 {
    let mut total = 0u32;
    for row in rows {
        total += row.amount;
    }
    log_total(total);
    report_total(total);
    audit_total(total);
    export_total(total);
    total
}
