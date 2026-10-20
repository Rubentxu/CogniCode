//! UAT-U10 compatibility corpus. Versioned, fixed content.
pub fn alpha() -> i32 { 1 }
pub fn beta(x: i32) -> i32 { x + alpha() }
pub struct Widget { pub id: u32 }
impl Widget { pub fn new(id: u32) -> Self { Widget { id } } }
