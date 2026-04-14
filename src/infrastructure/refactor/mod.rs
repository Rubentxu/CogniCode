//! Refactor infrastructure module

pub mod change_signature_strategy;
pub mod extract_strategy;
pub mod inline_strategy;
pub mod move_strategy;
pub mod rename_strategy;

pub use change_signature_strategy::ChangeSignatureStrategy;
pub use extract_strategy::ExtractStrategy;
pub use inline_strategy::InlineStrategy;
pub use move_strategy::MoveStrategy;
pub use rename_strategy::RenameStrategy;
