# Share Files Pagination Parity Implementation Plan

> **For agentic workers:** Use subagent-driven-development with independent specification and quality review; execute continuously within the user-authorized scope.

**Goal:** Align share files with ls: --max-keys=1000 and --marker, aggregate current-level files/folders across service pages, remove files --limit, and keep inspect as share overview with file information in files.

**Architecture:** Keep the V1 transport's internal page-size field. Add a bounded-listing handler that reuses ShareReceiver session renewal and ls normalization; request at most the remaining result allowance so continuation never skips unreturned entries. Stop at the cap or true end, reject inconsistent/repeated cursors and unsafe overproduction instead of silently losing results. Preserve anonymous/OAuth boundaries and table/CSV/query output conventions. No change to management share list --limit or download enumeration.

**Tech Stack:** Rust/clap/serde, existing ShareReceiver/ListFilesOutput/ls helpers, Python actual-entrypoint localhost and MCP fixtures.

## Tasks

- [x] Write and run a red actual CLI regression for --max-keys, removal of --limit, mixed files/folders, short server pages and continuation with no omissions. Add default=1000, exact file, empty, invalid cap, stalled/repeated cursor and bounds coverage.
- [x] Update ShareFilesArgs and a focused listing handler, validate positive cap before network, reuse session renewal per page, expose Selection.MaxKeys and final cursor. Update Rust literal constructors/tests without changing unrelated APIs. Annotate Review Fix reasons; new functions <=50 lines, <=5 parameters, nesting <=3.
- [x] Update only files registry parameters and typed schemas; preserve management list limit. Split files-specific guidance out of download help. Synchronize EN/ZH help, Describe, Skill/MCP, README, bundled Skill and test guide. Keep historical verification reports truthful.
- [x] Adapt existing URL/OAuth/files-display fixtures from --limit to --max-keys and realistic terminal pages; add default aggregation and both-entrypoint MCP coverage. Run focused red/green before integration.
- [x] Reflect user confirmation: inspect remains public share overview, file details through files. Update the PRD's relevant requirements within this authorized scope and keep an exact change record; no new file-detail network calls in inspect.
- [x] Build unified and dedicated binaries; run ADrive unit/contracts, root help/discovery tests, dedicated invocation tests and relevant sharing localhost fixtures. Perform independent spec review then full seven-category quality review, fix Critical/Major, record results and commit locally on xsj-dev without pushing.

Results: 525 Rust tests and all 13 sharing regression scripts passed on both entrypoints. Independent specification and quality review passed after correcting offline-plan parameter visibility. PRD inspect-only changes were read back at revision 103. See [verification record](../../adrive-share-files-max-keys-verification.md).

## Validation commands

`python3 tests/share_files_pagination_regression.py target/debug/ve-storage-uni-cli packaging/cargo/ve-adrive-cli/target/debug/ve-adrive-cli`

`cargo test -p ve-adrive-cli-core --locked --offline --quiet`

`cargo test --lib --test functional_agent_contract_test --test metadata_consistency_test --test skill_export_test --locked --offline --quiet`

`cargo test --manifest-path packaging/cargo/ve-adrive-cli/Cargo.toml --locked --offline --quiet`

Local HTTP binding requires sandbox escalation in this host. Test credentials are synthetic; no live share mutations/downloads are needed. D01 adriveapp mapping remains pending; do not invent domains or page-size hard limits from unrelated APIs.
