# Share download ranges and preview verification

Implementation branch: `xsj-dev`; baseline: `c929a2c`.

## Behavior

Shared files at least 20MB (20 MiB) automatically use 16 MiB ranges with up to four requests in flight, without requiring a checkpoint. `--checkpoint-threshold <size>` and `--multipart-concurrency <n>` use cp's size syntax and concurrency default. A strong ETag pins each response; a provided CRC64 verifies the assembled representation. Without either identity, use one streamed GET to avoid combining different object versions. A server ignoring Range or returning a different Content-Range fails explicitly without publishing a destination file.

The common `download_ranges` module now owns lazy checked planning and bounded scheduling for both cp and share. Their transport adapters remain separate: cp uses its authenticated resource client; share's signed content requests carry neither account credentials nor share tokens. Only a failed content range retries, using the same issued URL. The pinned staged file is published atomically after final size, CRC64 and SHA-256 processing. SIGINT drops in-flight work and cleans unpublished content.

`share view <URL> [--path FILE]` is an independent command under preview permission. Default auto mode returns a short-lived document descriptor for supported documents, or raw-content headers. `--raw` requests the original stream for a pipe or redirected stdout; no success envelope or automatic browser opening. Preview uses only the share token, never automatically retries, and does not fall back to download. The document discriminator is the service's `X-Ids-Preview-Result: document`, not JSON content type.

## cp parity audit

| Area | cp | Share after this change | Assessment |
| --- | --- | --- | --- |
| Large file transport | 16 MiB concurrent ranges when checkpoint is enabled and size reaches threshold | Same planner/scheduler and defaults; automatic when snapshot identity allows | Implemented; share does not require checkpoint |
| Content retries | Configurable transport policy, body/length/CRC checks | Three attempts for a failed range, same signed URL, 200/400 ms backoff | Range retries implemented; retry configuration and Retry-After remain different |
| Integrity and publication | If-Match/CRC; range assembly to ordinary destination | Exact 206/range/ETag/length validation, final CRC/SHA, pinned staged atomic publish | Preserve share's existing guarantees |
| File progress and summary | Scan/file progress, summary | Reuses the same style and adds current-file completed ranges | Implemented; byte-granularity option remains absent |
| Checkpoint recovery | Can reuse size-matching partial range files | Verifies and reuses completed files only; an incomplete file restarts | Remaining gap: part-level recovery |
| File concurrency | Recursive default 16 | Files run sequentially; ranges within a large file run concurrently | Remaining gap |
| Folder listing | Up to four prefixes concurrently | Sequential paginated BFS | Remaining performance gap |
| Bandwidth limit | Option available, coverage differs between cp paths | No bandwidth flag | Remaining gap |
| Overwrite | Force default; no-clobber/newer skip strategies | Conflict by default, explicit atomic `--overwrite` | Existing share behavior; skip/newer options absent |
| Selection and paths | Filters, include-parent, file or directory destination | Exact path/folder, destination directory, preserved selected root | Existing semantics; include/exclude options absent |
| Failure behavior | Recursive batch records failure and continues | Fail-fast; completed files retained and remaining files reported pending | Existing difference; safe continuation needs per-file issuance state |
| Reports | Manifest and CSV report options | Aggregate envelope with per-file JSON/query details; recovery journal optional | Report/manifest options absent |
| Enumeration scale | Pre-scan or no-manifest streaming | Bounded snapshot: 20,000 entries, 10,000 pages, 8 MiB | Existing bound retained |
| Authentication | Selected account resource API | Anonymous default; explicit OAuth for info/token exchange; share-token file APIs | Required sharing contract |
| URL issuance | Account content requests can be retried by resource client | Signed URL issued once; ambiguous issuance never automatically repeated | Required sharing contract |

This audit is not a claim that all cp flags or recovery features are implemented in sharing. In particular, concurrent ranges do not imply partial-file recovery.

## Evidence and regression cases

The old binary completed one full GET for a 32 MiB + 9 byte fixture and failed the new test with `[None]` instead of three Range headers. After implementation, two requests overlap even with concurrency set to two, the final bytes match exactly, and URL issuance occurs once. A separate independent review reproduced a mixed-version file when metadata had no strong ETag/CRC64; the regression first failed with three unsafe ranges, then the implementation added a single-GET fallback (`Review Fix #ShareRange1`).

Normal/boundary/error/security cases include:

1. Default automatic three-part download; final short range and exact part boundary.
2. Explicit concurrency one/two, default threshold, higher threshold, invalid zero concurrency.
3. A single 503 or interrupted body retries only its range; no repeated URL issuance.
4. Ignored Range, wrong Content-Range, wrong ETag, short body and bad CRC64 never publish a target.
5. SIGINT during active ranges retains the file-level checkpoint and removes staged content.
6. Forced/suppressed range progress stays on stderr; stdout remains a valid structured result.
7. Missing snapshot identity selects one streamed GET; content requests never receive account/share tokens.
8. Document preview discriminates by response header; an ordinary JSON file remains raw.
9. Raw preview streams large content, handles interruption/closed pipes, and does not mix stdout envelopes.
10. Preview service denials preserve code/status/request ID and never reissue the counted operation.
11. Exact file selection, single-file automatic selection, directory/multiple-root rejection and password/OAuth behavior.
12. EN/ZH help, Describe, exported skills, capabilities and structured MCP contracts agree for both binaries.

## Validation results

All commands below completed successfully for this change:

| Check | Result |
| --- | --- |
| `cargo test -p ve-adrive-cli-core --quiet` | 432 unit + 13 share contract tests |
| `cargo test --lib --quiet` | 15 root tests |
| `cargo test --test functional_agent_contract_test --quiet` | 11 functional contracts |
| Standalone `cargo test --test direct_invocation --quiet` | 7 tests |
| `share_range_regression.py` against both final debug binaries | Passed: overlapping ranges, retries, integrity, controls, cancellation, missing identity fallback |
| `share_view_regression.py` against both final debug binaries | Passed: document/raw/large stream, failures, malformed error bodies, selection, raw-output safety and request IDs |
| Six existing receiver scripts against both entrypoints | Passed: download, download display, download progress, files display, OAuth and URL regressions |
| Compiled EN/ZH help, Describe, exported skills, capabilities | 26 checks passed across both entrypoints |
| Formatting and diff whitespace | `cargo fmt --all -- --check`, `git diff --check` passed |

Both `target/debug/ve-storage-uni-cli` and `packaging/cargo/ve-adrive-cli/target/debug/ve-adrive-cli` were rebuilt. Rust total: 478 tests. Tests use synthetic tokens and isolated temporary files; local HTTP fixtures do not depend on cloud credentials.

Independent specification review passed separately for ranges and preview. Full code review checked correctness, security, performance, maintainability, robustness, testability and observability. Download review found and repaired one Major (mixed-version ranges without identity) and the cancellation-fixture isolation/style issue. Preview review repaired HTTP rejection context, structured MCP/raw compatibility, unsupported descriptor restrictions, and terminal request-ID propagation. Review annotations `ShareRange1/2` and `ShareView1` through `ShareView5` explain each change. Final re-review: 0 Critical, 0 Major, 0 Minor remaining. No more than three repair iterations were needed in either review stream.

Full runnable implementation is in `handler/download_ranges.rs`, `handler/share_content_ranges.rs`, `handler/share_content.rs`, `handler/share_download.rs`, `handler/share_download_progress.rs`, `domain/share_preview.rs`, and `handler/share_view.rs` under `crates/adrive/src`, with the existing cp adapter in `handler/high_level.rs`. The parity table records the remaining feature gaps; no other review suggestions remain open.

## Live environment limitation

The Go SDK main commit `607844e07b62a5175e1743145898ec1d39c63b00` (remote head rechecked) exposes V1 ShareSession.PreviewFile/PreviewFileAuto. The user confirmed BOE deployment, so the remaining issue is being investigated as a concrete routing discrepancy, not assumed to be an undeployed service.

Fresh read-only checks against `http://vtagt3g9n.idsapi-boe.volces.com` established:

- Share info, token exchange and the selected file metadata return 200 with `Server: IdsServer` and IDS request IDs.
- `/v1/share_file/preview` with the same valid session/path returns HTTP 404, `text/plain; charset=utf-8`, body exactly `404 page not found\n`, without IDS request ID or `Server: IdsServer`.
- Raw and auto requests behave identically; independently matching the Go SDK's JSON Content-Type does not change the result. The standard page host's preview path also returns the same plain 404.
- The Rust request path/query/token agree with the current SDK. No redirect was followed; no file contents, account tokens or signed preview URLs were logged.

These observations narrow the problem to the reachable host/path/backend routing, but do not prove which deployment or gateway setting is responsible. A verified server request URL/curl or deployment lane/header is still needed for live resolution. Local contract success is not represented as successful BOE preview. The existing view share's download denial remains unchanged.
