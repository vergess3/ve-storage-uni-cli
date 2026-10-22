# Directory Upload Implementation Plan

**Goal:** Preserve directories in local recursive cp and sync uploads on tos, ve-tos and ve-adrive, with --disable-dir-object opt-out.

**Architecture:** Extend only upload planners/executors with explicit zero-byte directory tasks and preserve existing file transfer paths. Use service-specific folder creation, deterministic directory ordering, accurate reporting and desired-directory protection for sync. Maintain separate runtime ownership per crate while integrating shared discovery and docs centrally.

**Tech Stack:** Rust/clap/serde, existing TOS/IDS APIs and batch reports, Python localhost process/MCP fixtures.

## Tasks

- [x] TOS runtime and CLI: add CpArgs/SyncArgs bool with the canonical long flag; write and observe failing planning tests; extend manifest and streaming local uploads and dedicated sync; keep root keys valid, HNS directory headers valid, failure gates and delete protection intact. Files: crates/tos/src/cli/high_level.rs, crates/tos/src/handler/high_level.rs and focused helper modules/tests. Run `cargo test -p ve-tos-cli-core --locked --offline`.
- [x] ADrive runtime and CLI: write failing folder-planning/idempotency/delete tests, add flag and propagation, folder task execution on both paths and source directory desired sets; preserve resume checks, actual error categories and mv behavior. Files: crates/adrive/src/cli/high_level.rs, crates/adrive/src/handler/high_level.rs and focused helper modules/tests. Run `cargo test -p ve-adrive-cli-core --locked --offline`.
- [x] Discovery: add the canonical parameter to cp/sync only on all registries and boolean schemas/dispatch; synchronize translations, Describe APIs and dry-run policy. Files: three registries, ADrive metadata and src/lib.rs. Add actual-entrypoint help/Describe/Skill/MCP assertions for canonical name, rejected aliases and boolean type.
- [x] Documentation: document examples for default and opt-out, empty root/include-parent, HNS/ADrive implicit parents, filters, sync-delete and mode equivalence in README, three bundled Skills, shared command guides and high-level plan. Update release notes with this dedicated behavior fix.
- [x] Process regression: use localhost TOS FNS/HNS and IDS fixtures to capture object keys, directory requests, headers, bytes and repeated uploads; test both unified/direct entrypoints and both manifest modes. Complement these process checks with Rust deletion/failure tests. Run with all rebuilt binaries; no user configuration or cloud mutations.
- [x] Review: inspect agent changes as specification Reviewer, then commission a fresh seven-category code review; correct findings with Review Fix annotations and rerun affected tests. Record full results, remaining limits and at least five release cases in docs/directory-upload-verification.md.
- [x] Integration preparation: format/static checks, relevant Rust/core/config/discovery/entrypoint suites and existing sharing regressions. Verification records: docs/directory-upload-verification.md and docs/ve-tos-mcp-entrypoint-verification.md.

Spec: [approved scope and contract](../specs/2026-10-21-directory-upload-design.md). User's explicit instruction to fix with the five listed requirements authorizes implementation without another approval gate.

Post-verification integration: commit the independently discovered ve-tos MCP entrypoint fix separately from directory uploads; fetch/rebase origin/master, push xsj-dev and verify exact remote hash. Actual commit and remote state are authoritative for these post-commit steps.

## Parameter simplification (2026-10-21)

The user approved keeping only `--disable-dir-object`. Removed the alias declarations and synchronized runtime errors, English/Chinese discovery metadata, bundled/exported Skills, README, release notes and this specification/plan. Existing tests now verify the canonical flag and rejection of removed spellings. The follow-up verification in `docs/directory-upload-verification.md` records 35 directory tests, 57 discovery/export tests, six real-entrypoint fixture combinations and the independent review.
