//! Python Security Rules
//!
//! This module contains Python-specific security rules for vulnerability detection.

pub mod s112_rule;       // Generic exception raised
pub mod s1148_rule;      // traceback.print_exc() instead of logging
pub mod s1163_rule;      // Catch-all except Exception: pass
pub mod s1165_rule;      // Exception swallowed without log
pub mod s1313_rule;      // Hardcoded IP addresses
pub mod s1523_rule;      // eval()/exec() usage
pub mod s2068_rule;      // Hardcoded credentials
pub mod s2077_rule;       // SQL injection (f-strings)
pub mod s2092_rule;      // Cookie without Secure
pub mod s2095_rule;      // Resource leak (file not closed)
pub mod s2221_rule;      // Catching BaseException
pub mod s2612_rule;      // Weak file permissions
pub mod s2755_rule;      // XXE vulnerability
pub mod s3330_rule;      // Cookie without HttpOnly
pub mod s3358_rule;      // Nested ternary
pub mod s3649_rule;      // SQL via string concat
pub mod s4423_rule;      // Weak TLS
pub mod s4502_rule;      // CSRF disabled
pub mod s4784_rule;      // ReDoS (regex injection)
pub mod s4829_rule;      // print() in production
pub mod s4830_rule;      // SSL verification disabled
pub mod s5042_rule;      // Zip bomb
pub mod s5247_rule;      // XSS in templates
pub mod s5332_rule;      // Clear-text HTTP
pub mod s5542_rule;      // Weak crypto (MD5, SHA1)
pub mod s5547_rule;      // Weak cipher (DES, RC4)
pub mod s5693_rule;      // File upload without size limit
pub mod s5725_rule;      // CSP missing
pub mod s5734_rule;      // HSTS missing
pub mod s5736_rule;      // X-Content-Type-Options missing

pub use s112_rule::PY_S112Rule;
pub use s1148_rule::PY_S1148Rule;
pub use s1163_rule::PY_S1163Rule;
pub use s1165_rule::PY_S1165Rule;
pub use s1313_rule::PY_S1313Rule;
pub use s1523_rule::PY_S1523Rule;
pub use s2068_rule::PY_S2068Rule;
pub use s2077_rule::PY_S2077Rule;
pub use s2092_rule::PY_S2092Rule;
pub use s2095_rule::PY_S2095Rule;
pub use s2221_rule::PY_S2221Rule;
pub use s2612_rule::PY_S2612Rule;
pub use s2755_rule::PY_S2755Rule;
pub use s3330_rule::PY_S3330Rule;
pub use s3358_rule::PY_S3358Rule;
pub use s3649_rule::PY_S3649Rule;
pub use s4423_rule::PY_S4423Rule;
pub use s4502_rule::PY_S4502Rule;
pub use s4784_rule::PY_S4784Rule;
pub use s4829_rule::PY_S4829Rule;
pub use s4830_rule::PY_S4830Rule;
pub use s5042_rule::PY_S5042Rule;
pub use s5247_rule::PY_S5247Rule;
pub use s5332_rule::PY_S5332Rule;
pub use s5542_rule::PY_S5542Rule;
pub use s5547_rule::PY_S5547Rule;
pub use s5693_rule::PY_S5693Rule;
pub use s5725_rule::PY_S5725Rule;
pub use s5734_rule::PY_S5734Rule;
pub use s5736_rule::PY_S5736Rule;
