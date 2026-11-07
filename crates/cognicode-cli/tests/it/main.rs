//! One integration-test binary for these suites: every file below is a module.

#[path = "../common/mod.rs"]
mod common;

// One owner for the versioned-fixture helper: the core keeps it, the CLI suites include it.
#[path = "../../../cognicode-core/tests/fixtures/mod.rs"]
mod fixtures;

mod a013_lifecycle_gate_contract;
mod a014_capabilities_json;
mod a015_licenses_gate;
mod a015_onboarding_gate;
mod a_goal_is_declared_once;
mod a_hateoas_envelope;
mod action_register_identity_contract;
mod advisory_ignore_backing_contract;
mod analysis_result_is_honest;
mod b1_w2_identity;
mod b5_w1_plan_contract;
mod b6_w3_cogh_lifecycle;
mod b6_w4_skills_autodiscovery;
mod backtick_provenance_contract;
mod cli_exit_code_propagation;
mod cogh_cli;
mod cognicode_ide_adapter;
mod cognicode_lifecycle;
mod cognicode_plugin;
mod cr07_protobuf_advisory_closed;
mod cu1_w3_presenter_parity;
mod evidence_cli_mcp_equivalence;
mod gitignore_negation_contract;
mod graph_cli_mcp_equivalence;
mod graph_impact_absent_symbol;
mod json_stdout_is_one_document;
mod one_answer_one_document;
mod one_published_line_base;
mod per_file_surfaces_publish_pending_references;
mod perf_budget_checker_contract;
mod portable_skill_bundle;
mod prf_f4_w2_binary_restart;
mod prf_f6_w3_bis_sbom_contract;
mod prf_f6_w3_bis_staging_contract;
mod qw03_bin_tracking_guard;
mod qw04_preflight_contract;
mod refactor_apply_publishes_the_revalidation;
mod t1_5_3b_index_result_serde;
mod t1_5_4_graph_result_serde;
mod t1_5_5_json_dispatch;
mod test_taxonomy_contract;
mod the_cli_mermaid_format_is_the_graphs_vocabulary;
mod the_cli_says_what_comes_next;
mod the_cli_serves_the_snapshot_the_mcp_wrote;
mod the_goals_and_the_profiles;
mod the_published_coverage_has_one_owner;
mod the_questions_say_their_coverage;
mod the_recipe_executor;
mod unresolved_references_are_facts;
