# Share Endpoint Rename Implementation Plan

**Goal:** Use the approved `--share-endpoint` name consistently for share-page origins, preserving the existing production/BOE mapping and previously saved configuration.

**Architecture:** Rename the canonical CLI field, TOML key, environment variable and discovery metadata. Keep the old CLI spelling as a hidden alias, accept the old TOML key through serde and config set, and use the old environment variable only when the new one is absent. New serialization and documentation use the canonical name. Keep CLI > profile > environment > derivation precedence and existing sanitized validation.

**Tech Stack:** Rust/clap/serde, existing config and share contracts, Python actual-entrypoint localhost fixtures.

- [x] Add contract tests accepting the new CLI spelling on create/get/list/update and exposing it in Describe, while hiding the old alias. Run them red before implementation.
- [x] Add config tests writing `share_endpoint`, reading old `share_base_url`, and serializing only the new key. Run the new-key test red before implementation.
- [x] Rename production fields and metadata in ADrive, shared config and root help; add compatibility aliases and fallback. Preserve custom-page literal origins and HTTP(S)/port handling.
- [x] Update active README, bundled Skill/reference, test guide and E2E harness to canonical names. Keep old verification reports and plans as historical records.
- [x] Verify full production examples from `http(s)://idsapi.volces.com` to `http(s)://<Instance>.idsapp.volces.com/share/<ShareID>`, including already-prefixed Instance and a custom page override. Cover canonical/legacy environment precedence and secret-safe invalid-origin handling.
- [x] Build both entrypoints and run relevant config, ADrive, help/discovery and localhost regressions; perform seven-category self-review and independent review. Record evidence and commit locally on xsj-dev without pushing.

Result: 793 Rust tests passed (1 existing ignored test), all 14 sharing scripts passed on both entrypoints, and independent review round 2 found no remaining Critical/Major/Minor. See [verification record](../../adrive-share-endpoint-verification.md).

Validation commands: `cargo test -p tos-core --test test_config --locked --offline`; `cargo test -p ve-adrive-cli-core --locked --offline`; root help/discovery/Skill tests; dedicated direct-invocation tests; all sharing regression scripts against both rebuilt entrypoints. Live sharing mutation is unnecessary.
