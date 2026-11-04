//! One integration-test binary for the crate: every file below is a module.
//! A single binary links once instead of once per file.

mod common;

mod a009_agent_safe_profile;
mod a010_tool_authority_audit;
mod a012_structured_output;
mod a016_tools_runtime_consistency;
mod continuation_e2e;
mod cr07_metrics_exposition_contract;
mod find_usages_compat_0_97;
mod mcp_cancel_belongs_to_its_request;
mod mcp_catalog_anchor;
mod mcp_failure_arrives_as_failure;
mod mcp_http_exposure_is_safe;
mod mcp_http_routes_authority;
mod mcp_reparse_tells_missing_from_unreadable;
mod prf_f4_w3_corrupt_cache_recovery;
mod prf_f5_w3_signal_cancel;
mod prf_h06_adversarial_e2e;
mod the_harness_finds_the_binary_it_tests;
mod the_watch_endpoint_names_what_is_watched;
mod workspace_catalogue_cannot_define_the_product;
