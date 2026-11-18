pub fn greet(name: &str) -> String {
    format!("hello, {name}")
}

pub fn call_greet() -> String {
    greet("SCIP")
}
