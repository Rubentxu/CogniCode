/// Vulnerable function using EXECUTE with string concat - triggers S2091
pub fn execute_query(query: String) -> String {
    // EXECUTE with user input concatenation
    let sql = format!("EXECUTE sp_get_data {}", query);
    sql
}

/// Vulnerable function using EXEC with concat
pub fn exec_command(cmd: String) -> String {
    let sql = format!("EXEC usp_RunCommand '{}'", cmd);
    sql
}

/// Direct EXECUTE with variable
pub fn run_stored_proc(proc_name: String, param: String) -> String {
    format!("EXECUTE {} @param = '{}'", proc_name, param)
}
