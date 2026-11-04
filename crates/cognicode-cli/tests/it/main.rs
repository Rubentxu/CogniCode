//! One integration-test binary for these suites: every file below is a module.

#[path = "../common/mod.rs"]
mod common;

mod a013_lifecycle_gate_contract;
mod a014_capabilities_json;
mod a015_licenses_gate;
mod a015_onboarding_gate;
mod action_register_identity_contract;
mod advisory_ignore_backing_contract;
mod analysis_result_is_honest;
mod b1_w2_identity;
mod b6_w3_cogh_lifecycle;
mod b6_w4_skills_autodiscovery;
mod backtick_provenance_contract;
mod cli_exit_code_propagation;
mod cogh_cli;
mod cognicode_ide_adapter;
mod cognicode_lifecycle;
mod cognicode_plugin;
mod cr07_protobuf_advisory_closed;
mod evidence_cli_mcp_equivalence;
mod gitignore_negation_contract;
mod graph_cli_mcp_equivalence;
mod graph_impact_absent_symbol;
mod perf_budget_checker_contract;
mod portable_skill_bundle;
mod prf_f4_w2_binary_restart;
mod prf_f6_w3_bis_sbom_contract;
mod prf_f6_w3_bis_staging_contract;
mod qw03_bin_tracking_guard;
mod qw04_preflight_contract;
mod refactor_apply_publishes_the_revalidation;
mod t1_5_5_json_dispatch;
mod test_taxonomy_contract;
