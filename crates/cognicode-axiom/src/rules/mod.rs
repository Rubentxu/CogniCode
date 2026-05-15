//! Rules module - individual rule implementations

pub mod catalog;
pub mod test_smell;
pub mod error_handling;
pub mod bug_concurrency;
pub mod code_smells;

pub use catalog::*;
pub use test_smell::*;
pub use error_handling::*;
pub use bug_concurrency::*;
pub use code_smells::*;

// Re-export all test smell rules for convenient access
pub use test_smell::{
    TestWithoutAssertionRule,       // CC_TEST_001
    TestUsingSleepRule,              // CC_TEST_002
    TestSkippedWithoutReasonRule,   // CC_TEST_003
    TestNamingConventionRule,       // CC_TEST_004
    ComplexFixtureSetupRule,        // CC_TEST_005
    MultipleAssertionsRule,          // CC_TEST_006
    DuplicatedTestRule,              // CC_TEST_007
    TestUsingRandomRule,            // CC_TEST_008
    TestWithoutDescribeRule,        // CC_TEST_009
    AssertionsCountMismatchRule,    // CC_TEST_010
    NestedTestHooksRule,            // CC_TEST_011
    MockImplementationConfusionRule, // CC_TEST_012
    SpyOnNotRestoredRule,           // CC_TEST_013
    ActWrapperMissingRule,           // CC_TEST_014
    WeakAssertionStyleRule,         // CC_TEST_015
};

// Re-export all error handling rules
pub use error_handling::{
    UnwrapOnOptionRule,             // CC_ERR_001
    ExpectOnOptionRule,              // CC_ERR_002
    UnwrapOnResultRule,             // CC_ERR_003
    ExpectOnResultRule,             // CC_ERR_004
    ErrorChainRule,                // CC_ERR_005
    CustomErrorTraitRule,           // CC_ERR_006
    OptionInsteadOfResultRule,      // CC_ERR_007
    PanicForValidationRule,         // CC_ERR_008
    UnwrapOrDefaultRule,            // CC_ERR_009
    IncompleteMatchRule,           // CC_ERR_010
    ErrorLoggingRule,               // CC_ERR_011
    ToStringErrorRule,              // CC_ERR_012
};

// Re-export all bug-concurrency rules
pub use bug_concurrency::{
    RaceConditionRule,              // CC_CONC001
    MutexGuardLeakRule,             // CC_CONC002
    DeadlockRiskRule,               // CC_CONC003
    ChannelClosedRule,              // CC_CONC004
    RefCellBorrowAcrossAwaitRule,    // CC_CONC005
    UnboundedChannelRule,           // CC_CONC006
    ArcCloneInHotPathRule,          // CC_CONC007
    ConcurrentMapAccessRule,        // CC_CONC008
};

// Re-export all code smell rules
pub use code_smells::{
    TodoFixmeCommentRule,            // CC_CS_001
    EmptyNestedBlockRule,           // CC_CS_002
    DuplicateBranchesRule,           // CC_CS_003
    EmptyStatementRule,             // CC_CS_004
    RedundantSemicolonRule,         // CC_CS_005
    WildcardBeforeSpecificRule,     // CC_CS_006
    FunctionNamingConventionRule,    // CC_CS_007
    StubFunctionRule,               // CC_CS_008
    RedundantParenthesesRule,       // CC_CS_009
};
