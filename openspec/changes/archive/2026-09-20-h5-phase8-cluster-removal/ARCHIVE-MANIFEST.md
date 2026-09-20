# Archive of clippy-public-api-cluster-removal-bd (H5 phase 8)

This folder was created on 2026-09-20 (ISO) to archive the H5 phase 8 commit.

Original location: (no proposal folder existed — direct commit)
Archived by: orchestrator (roadmap-completion initiative)
Archive date: 2026-09-20
Cycle: H5 phase 8 of the H5 clippy-hygiene program

## Implementation evidence

- Commit: `2ce2b6ca chore(lint): remove 6 cluster-isolated dead pub items`
- Author: Ruben <rubentxu@cognicode.dev>
- Date: 2026-09-20 15:38

## Scope recap

Cluster analysis: each removed item was the sole remaining consumer of one or
more sibling items that were already dead, leaving the whole cluster without
workspace consumers.

**Cluster A: `InstallPlan` struct**
- Became isolated after H5 phase 7 removed `into_install_plan`
- `pub struct InstallPlan { version, profile, components }`
- `crates/cognicode-cli/src/cmd/bundle_manifest.rs`

**Cluster B: `RegistryConfig` + `DEFAULT_REGISTRY` + `Default` impl**
- Became isolated after H5 phase 7 removed `download_to` (only consumer of
  `RegistryConfig::token` and `::base_url`)
- `pub const DEFAULT_REGISTRY`
- `pub struct RegistryConfig { base_url, token }`
- `impl Default for RegistryConfig`
- `crates/cognicode-cli/src/cmd/registry.rs`

## Delta specs

None — `chore(lint)`.

## Knowledge graph

No project-level findings registered.

## Carry-forward

The `install_stage::Failed` enum variant became a zero-use item once
`into_install_plan` (its constructor pathway) was removed in phase 7. This
is captured in **H5 phase 9** (this cycle's mini-apply).