# Share Range Download and Preview Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** Automatically download large shared files using concurrent byte ranges, share transfer primitives with cp, audit remaining parity gaps, and add the approved independent `share view` command.

**Architecture:** Extract a transport-independent 16 MiB range planner and bounded async scheduler used by cp and sharing. Keep the share content client credential-free and retain pinned local handles, atomic publication, final SHA-256/CRC64 verification, and one signed-URL issuance per file. Preview uses the SDK V1 share-session preview route and the explicit document discriminator; it never changes a share's permissions or substitutes preview for download.

**Tech Stack:** Rust, reqwest, futures, indicatif, Python local HTTP/PTY fixtures.

## Accepted behavior and limits

- Remain on the user-selected xsj-dev checkout. Commit verified changes locally; do not push.
- Large shared files use ranges automatically at the cp default threshold (20MB); each range is 16 MiB, concurrency defaults to 4. Expose cp's `--checkpoint-threshold` and `--multipart-concurrency` controls. A checkpoint is optional and retains its established file-level recovery contract.
- Validate HTTP 206, exact Content-Range, ETag, body length, final size and CRC64. Retry only the failed content range using the same issued URL. A service ignoring Range must fail explicitly rather than corrupt a published file.
- Progress retains cp-style file totals and additionally reports current large-file range completion; --quiet/--no-progress and stdout isolation apply throughout.
- Download audit records file/list concurrency, overwrite/filter/report options, progress granularity, resumability, retries, cancellation, and intentional auth/signed-URL differences. Do not claim complete cp parity.
- User approved a separate `share view <URL> [--path FILE]`: default auto returns a document preview descriptor or raw-content metadata; explicit `--raw` streams content to stdout for a pager. No implicit browser launch. Preserve anonymous default and explicit OAuth/password behavior.
- The SDK declares /v1/share_file/preview; the previously tested BOE deployment returned 404. Local contract validation and live deployment status must be reported separately.

## Task 1: Shared range primitives and automatic share range transport

Files: handler/download_ranges.rs (new), handler/share_content_ranges.rs (new), handler/share_content.rs, handler/high_level.rs, handler/share_download.rs, handler/share_download_progress.rs, cli/share.rs, handler/mod.rs; tests/share_range_regression.py (new).

- [x] Write failing real-CLI fixture checks: automatic ranges, overlapping requests, exact output bytes, credential isolation, single URL issuance, failed-range-only retry, ignored/wrong Range response, truncated body, checksum mismatch, cancellation and no partial publication. Run against the current binary to establish missing Range behavior.
- [x] Add a lazy checked RangePlan (number, offset, size) with 16 MiB parts, and a generic bounded range runner. Unit cases: zero length, exact part boundary, short final part, overflow/count validation, concurrency bound and error cancellation.
- [x] Replace cp's range construction and scheduling with the shared primitives without changing its authenticated transport/checkpoint semantics. Run cp transfer unit tests.
- [x] Implement share ranged writes into an exclusive staged file with bounded buffers and no credential headers. Check every response before writing; retry a failed range in place. Hash/check final staged contents before atomic publication. Keep cancellation cleanup owned by StagedFile.
- [x] Add validated threshold/concurrency options before receiver requests; connect range progress without changing stdout envelopes. Keep existing tests' default small-file behavior.
- [x] Synchronize help, Chinese translations, registry, describe/skill parameter metadata, README and repository skill documentation. Run both CLI entrypoints against the new fixture.

## Task 2: V1 share preview command

Files: cli/share.rs, handler/share.rs, handler/share_view.rs (new), domain/share_preview.rs (new), handler/mod.rs, domain/mod.rs, share_registry.rs, share_metadata.rs, handler/meta.rs, src/lib.rs, docs/skills/README and share contract tests.

- [x] Write failing command/fixture tests for a raw text file, ordinary JSON file, document discriminator and descriptor, explicit path and single-file automatic selection, multi-file directory rejection, password/OAuth reuse, 403/404, stream interruption and credential redaction.
- [x] Add a non-replayed streaming share-session preview request. Default `preview_mode=auto`; `--raw` requests raw bytes. Only X-Ids-Preview-Result=document permits descriptor decoding. Bound JSON descriptor/error responses and close unused raw responses.
- [x] Implement single-file selection through existing receiver metadata/session APIs. Default emits a normal envelope describing document URL/expiry/type or raw content type/size/status. Raw mode streams to stdout without a mixed success envelope; prevent control/binary output directly to an interactive terminal and report write/HTTP errors.
- [x] Register view in CLI, validation/dry-run routing, help/translation catalogs, Describe, Skill export, capabilities and tests. Ensure no downloaded raw preview is represented as a download operation.
- [x] Test both binaries and the existing share receiver regressions.

## Task 3: Audit, review and delivery

Files: docs/adrive-share-ranges-and-view-verification.md (new).

- [x] Record the cp parity matrix with concrete behavior and remaining differences, plus at least five normal/boundary/error/security test cases.
- [x] Run core tests, root/standalone functional contracts, old share fixture regressions and new range/preview regressions; build both debug binaries.
- [x] Complete independent specification review, then full seven-category code review. Repair every Critical/Major and re-review; label review fixes with reasons. No more than three repair rounds.
- [x] Verify diff/format/status, commit local changes with the report, and provide reproducible user commands plus the unresolved BOE routing evidence. Do not push.

Execution note: local implementation, verification and independent reviews are complete. The user confirmed BOE deployment; live preview routing remains under investigation pending a known-good request URL or lane/header. No successful BOE preview is claimed.
