# ADrive V1 Share Implementation Plan

> **For agentic workers:** Use subagent-driven-development or executing-plans to implement the tasks with test-first changes and separate specification/code-quality reviews. Work in the existing `xsj-dev` checkout as explicitly requested by the user. Preserve unrelated untracked files.

**Goal:** Add eight usable V1 share commands to both ADrive CLI entrypoints, with complete help, discovery, Skill, MCP, and download recovery contracts.

**Architecture:** Keep management calls on the existing authenticated IDS client. Add an anonymous share client and token-scoped session for receivers, plus a separate credential-free client for signed file transfers. Keep CLI arguments, wire models, orchestration, and local download safety in focused share modules.

**Tech Stack:** Rust, clap, reqwest, tokio, serde, existing Envelope and registry-backed metadata.

## Accepted product decisions

- Use the maintained V1 APIs described by Go SDK `ids/share.go` and `ids/types.go`; the supplied API document describes V2 and is product context rather than the V1 wire contract.
- Live follow-up on 2026-10-13: the current SDK explicitly requires OAuth Bearer / token-source authentication for share management. Both the original environment AK/SK and the replacement supplied by the user passed ordinary resource operations but failed share creation. AK/SK-only share management therefore needs a confirmed server contract/deployment change; it cannot be inferred from other V1 APIs. The user requested AK/SK-only verification, so OAuth login was cancelled and no successful live share lifecycle is claimed.
- Commands: `share create`, `get`, `list`, `update`, `cancel`, `inspect`, `files`, `download`.
- Create shares one remote file or directory through a one-element `FilePathList`. Expose `view` and `download`, duration converted to `ExpirationSec`, stdin password, name/description, and preview/download limits.
- Preserve V1 fields and enums: `ShareID`, integer `UpdatedAt`, `revoked`, offset-based management search, marker-based file browsing, and `x-share-token`.
- Construct `ShareUrl` as the instance share-page origin plus `/share/<encoded ShareID>`. Allow an explicit share-page base URL when it differs from the API origin. File and directory links use the same rule.
- `files` accepts single-file shares and exact child file selection; exact selection uses `GetShareFile`, not a filename prefix approximation.
- Server owns quota accounting. Count timing and per-folder accounting do not block implementation. Unknown download-address issuance is not blindly replayed; content retries use the already-issued address.
- Directory hierarchy, empty directories, pagination, environment routing, and actual auth compatibility are verified after implementation with real resources.
- Download destination always names a directory. Recursive selection preserves the selected directory name once. Default refuses existing files; overwrite applies only to ordinary files. File-level checkpoint resume reauthorizes and checks both sides.
- Creation/get may explicitly return a password for delivery. Diagnostics, dry-run, MCP argv, checkpoints and errors redact passwords, tokens and signed URLs.

## Task 1: V1 wire models, transport, URL identity

Files: create `crates/adrive/src/domain/share_types.rs`, `share_url.rs`, `share_access.rs`, `client/share.rs`; update `domain/mod.rs` and `domain/client.rs`.

- [x] Add failing tests for URL construction/encoding, malformed URLs, single-object create serialization, partial update zero values, legacy encoded `FilePathList`, and exact V1 methods/paths.
- [x] Run `cargo test -p ve-adrive-cli-core --lib share` and confirm the expected missing behavior.
- [x] Implement typed management methods: POST create, PUT update, GET get/search, POST cancel. Preserve optional response fields and unknown additions.
- [x] Implement anonymous GET information, POST access-token exchange, token-header GET list/get/download-url. Root listing uses `ListRoot=true`; directory browsing uses `Prefix` and `/` delimiter.
- [x] Add explicit non-replayable operation policy, bounded metadata retries, no receiver management credentials, disabled API redirects, response-size bounds, and exact secret redaction.
- [x] Verify with local HTTP fixtures and perform full self-review, followed by independent specification and quality reviews.

## Task 2: CLI, metadata, help, configuration, MCP

Files: create `cli/share.rs`, `share_metadata.rs`, `share_registry.rs`; update `cli/mod.rs`, `registry.rs`, `handler/meta.rs`, `src/lib.rs`, `crates/tos-core/src/infra/config.rs`, bundled ADrive Skill files.

- [x] Add failing entrypoint tests for all eight command paths, help, required selectors, flags and global describe recovery.
- [x] Implement argument structs with explicit duration/permission/status/selector validation. Group-only invocation prints help.
- [x] Add share-page base URL configuration and explicit override. Configuration validation accepts safe HTTP(S) origins, preserving the configured protocol and port without upgrading HTTP.
- [x] Register each leaf's parameters, defaults, side effects, examples, and API mappings. Implement bilingual descriptions and input/output schemas.
- [x] Connect completion, capabilities, Skill export, and README examples together with command availability.
- [x] Extend MCP positional/boolean/typed arguments, pass passwords through piped stdin, redact planned and executed argv including URL `pwd`, and terminate timed-out child commands while preserving download recovery.
- [x] Run CLI metadata/functional contract tests and both review stages.

## Task 3: Authenticated management orchestration

Files: create `handler/share.rs`, `share_password.rs`; update `handler/mod.rs` and common name-resolution visibility only where needed.

- [x] Add failing HTTP/entrypoint tests for create URL output, selectors, partial updates, explicit version bypass, no-version Get then Update, cancellation and dry-run.
- [x] Reuse runtime profile/auth selection, URI and by-name resolution, error envelopes, output query and tracing.
- [x] Validate input before mutation; resolve share-page URL before create. Search requires explicit creators unless the selected OAuth identity supplies its own ID.
- [x] Implement hidden receiver password input and bounded explicit stdin input without trimming meaningful characters. No help/describe invocation reads stdin or authenticates.
- [x] Implement update version guard, cancellation's single TTY confirmation or explicit `--yes`, redacted plans, and result-unknown/incomplete-response errors preserving known identifiers.
- [x] Verify tests and perform both review stages.

## Task 4: Anonymous browsing and safe download/recovery

Files: create `handler/share_receive.rs`, `share_download.rs`, `share_checkpoint.rs`, `share_local.rs` and relevant tests.

- [x] Add failing fixtures for anonymous access, pwd precedence, single files, root/child directory pagination, repeated markers, conflicts and interruption.
- [x] Implement inspect without token acquisition, files root/directory listing and exact file metadata, token renewal bounded by known expiry.
- [x] Enumerate a complete bounded task plan before URL issuance. Preserve selection boundaries and empty directories; reject unsafe names, path traversal, symlinks and mapping collisions.
- [x] Download sequentially into same-directory temporary files; validate size and available checksum/ETag, then atomically publish without clobber races. No management headers or share token reach the file service.
- [x] Write versioned, locked, atomic checkpoints without secrets. Verify identity, selection, destination, overwrite policy, remote metadata and local checksum on resume; do not expand old tasks to newly added files.
- [x] Return one aggregate Envelope with Complete/Summary/failed/pending state, consistent exit codes, and preserved successful files. Dry-run issues no download URL and writes no destination/checkpoint.
- [x] Verify fixtures and perform specification and full security/correctness/robustness reviews.

## Task 5: Integrated verification and rollout

- [x] Run `cargo fmt --all -- --check`, `cargo test -p ve-adrive-cli-core`, relevant root CLI/metadata/MCP tests, and workspace checks justified by shared configuration changes.
- [x] Add/update live E2E cases and execute against an available dedicated test environment; report unavailable live coverage honestly without claiming it passed. The dedicated environment is absent, so the lifecycle test was collected and explicitly skipped; its cleanup correction is tracked by the specification review.
- [x] Verify both entrypoints, Chinese/English help, describe, discovery, completion, Skill export, MCP schema and examples against the executable contract.
- [x] Complete independent review of the full diff. Fix all Critical/Major issues with `// [Review Fix #N]` rationale and rerun affected tests; retain a review ledger.
- [x] Report complete code through file links, tests actually executed, review rounds and remaining limitations. Keep work on `xsj-dev`; do not publish, merge or push without an instruction to do so.

## Tracking

The user-approved work is continuous: complete the tasks without additional business-design approvals. A newly discovered environmental blocker can be reported while unaffected implementation proceeds.

Implementation and local verification completed on 2026-10-13. All specification and code-quality review gates passed; the final remaining issue counts are zero. Dedicated live-service verification and Windows runtime verification remain environment-dependent follow-up work, not claimed as completed tests. See the accompanying review ledger for executed checks and coverage limits.
