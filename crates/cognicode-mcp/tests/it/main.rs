//! One integration-test binary for the crate: every file below is a module.
//! A single binary links once instead of once per file.

mod common;

#[path = "../../../cognicode-core/tests/fixtures/mod.rs"]
mod fixtures;

mod a009_agent_safe_profile;
mod a010_tool_authority_audit;
mod a012_structured_output;
mod a016_tools_runtime_consistency;
mod cr07_metrics_exposition_contract;
mod explain_finding_answers_why;
mod find_usages_compat_0_97;
mod mcp_cancel_belongs_to_its_request;
mod mcp_cancellation_belongs_to_its_request;
mod mcp_catalog_anchor;
mod mcp_failure_arrives_as_failure;
mod mcp_http_exposure_is_safe;
mod mcp_http_routes_authority;
mod mcp_reparse_tells_missing_from_unreadable;
mod prf_f4_w3_corrupt_cache_recovery;
mod prf_f5_w3_signal_cancel;
mod prf_h06_adversarial_e2e;
mod telemetry_is_opt_in;
mod the_harness_finds_the_binary_it_tests;
mod the_watch_endpoint_names_what_is_watched;
mod workspace_catalogue_cannot_define_the_product;
mod b2_w2_catalog_parity;
mod b2_w3_output_schema_contract;
mod b6_w1_mcp_robustness;
mod prf_sec_07_adversarial_campaign;
mod retired_tools_are_not_announced;
mod the_continuation_chain_is_byte_exact;
mod the_index_tool_only_offers_strategies_that_index;
mod the_mermaid_tool_only_offers_formats_it_can_publish;
mod the_published_arguments_are_the_declared_ones;
mod one_published_line_base;
mod c30_capabilities_declare_the_languages_the_registry_reads;
