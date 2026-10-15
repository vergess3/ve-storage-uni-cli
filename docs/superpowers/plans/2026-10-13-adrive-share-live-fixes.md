# ADrive sharing live-test corrections

**Goal:** Close the three defects reproduced with OAuth on BOE, following the user's approved domain and local-root rules.

**Architecture:** Map the exact public/BOE IDS host families between page and API origins, preserving scheme, port and Instance label; explicit overrides win and custom hosts remain unchanged. Resolve user-selected local roots with the OS before pinning no-follow handles, use the resolved destination/checkpoint identity for collision checks and resume, and keep remote descendants no-follow. Preserve explicit download-URL rejection diagnostics; only ambiguous issuance remains unknown and non-replayable.

**Scope:** Existing `xsj-dev` checkout; no new worktree, push, merge or configuration change. Continue the user-approved implementation in this session using debugging, TDD, verification and independent code review.

- [x] Add failing URL/receiver tests for public and BOE mapping, Instance labels, explicit API/page overrides, custom/lookalike hosts and secret-free receiver construction. Implement in `domain/share_url.rs` and `handler/share_receive.rs`.
- [x] Add failing local-root/download tests for generic symlink roots, macOS `/tmp` and `/var`, alias-to-canonical resume, checkpoint/lock alias collisions and remote descendant escapes. Resolve only local roots in `handler/share_local.rs`, `share_local_windows.rs`, `share_checkpoint.rs` and `share_download.rs`.
- [x] Add failing HTTP tests for definite 403/429 rejection, request ID/status/category, malformed successful responses and 5xx ambiguity with exactly one issuance. Correct `share_download.rs` aggregate semantics and strengthen the actual-entrypoint Python regression.
- [x] Update English/Chinese help, Describe/discovery semantics, README and bundled Skill/workflows for the three changes. Record actual OAuth hierarchy and denial evidence in the review ledger.
- [x] Run targeted and complete serial workspace tests, standalone ADrive contracts, process regressions, formatting and diff checks. Request a fresh seven-category review and fix all Critical/Major findings with regression coverage.
- [x] Rebuild both entrypoints and rerun OAuth lifecycle plus naked page URL/root-alias/403 probes. Cancel every test share and delete only the exact test roots; independently verify cleanup.
- [x] Record review/verification results and create a local commit with all related source/tests/docs; report remaining platform/environment limits accurately.

Acceptance cases: page URL points to `idsapp`/`idsapp-boe`; receiver APIs automatically use corresponding `idsapi`/`idsapi-boe` without credentials or `--endpoint`; custom endpoint flag wins. User-root aliases download successfully, interior links cannot escape, aliases cannot bypass journal/lock protection or resume identity. A known 403 exits 5 with permission-denied kind, service code/status and unknown=false; 5xx/interruption/invalid success remain unknown with no automatic replay. Dry-run remains zero-I/O.

Final outcome: all three live defects and all three subsequent review findings are closed. Fresh full workspace: 1,488 passed, 4 existing ignored; standalone: 13; packaging: 77. Both compiled entrypoints passed process regressions and real OAuth lifecycle/precise denials/default-domain routing under `/tmp`, with all cloud resources cleaned. Complete second-pass review has 0/0/0/0 remaining findings. Windows backend type-check passed; full Windows application check is blocked by missing MSVC headers on macOS, and Windows runtime remains unverified. See the existing sharing review ledger for detailed evidence.

## User-directed protocol correction — 2026-10-13

The user rejected the unrequested HTTPS-only policy. Sharing must support HTTP and HTTPS for configured page origins, returned URLs, anonymous receiver API origins and service-issued download addresses. Standard IDS host mapping retains the input scheme and port; it must not promote HTTP to HTTPS. Existing malformed-input rejection, secret redaction, account/receiver/content credential separation, no-redirect transport and download containment remain in scope.

- [x] Reproduce the policy defect with failing public HTTP page/password round-trip, content-address and persisted page-configuration tests.
- [x] Remove the HTTP loopback-only guards from page/receiver, configuration and content validation; delete automatic HTTPS promotion and adjust default-profile/port expectations.
- [x] Synchronize bilingual help, Describe/export semantics, README, bundled Skill and the test procedure; keep unknown-issuance tests focused on truly invalid addresses.
- [x] Rebuild both binaries, run ADrive/TOS-core/entrypoint contracts and actual receiver process regressions, and perform default HTTP real-service lifecycle verification with exact cleanup.
- [x] Complete independent seven-category review, fix required findings and record evidence; retain the correction in a local `xsj-dev` commit without pushing.

Acceptance cases: ordinary non-loopback HTTP page overrides persist; generated page/API URLs preserve HTTP(S) and custom ports; URL passwords remain excluded from safe output and Debug; credentials/fragments/traversal/non-HTTP schemes remain rejected; both entrypoints create and receive default-profile single-file and directory shares with verified bytes and cleanup. The earlier HTTPS-promotion commit's test results are historical evidence, superseded by this user-directed contract.

Validation: final ADrive 374 unit tests + 6 contracts; TOS-core 263 unit/integration tests, one existing ignored documentation example; unified sharing 7 contracts and standalone ADrive 7 contracts. Both rebuilt entrypoints passed receiver process regression, default HTTP directory lifecycle, and final bare single-file create/download. Actual bilingual help/Describe, 32 command-Skill exports, HTTP page configuration and six receiver dry-runs passed. Third complete independent review passed with 0/0/0/0 remaining findings. Detailed defects, red/green evidence, cleanup and six release regression cases are in the sharing review ledger. Windows runtime remains unverified; no new full-workspace or Windows-runtime claim is made.
