# ADrive share management URI compatibility

**Goal:** Support the user-approved `adrive://instance/share-id` target alongside existing `share-id --instance instance` for get/update/cancel on `xsj-dev`.

**Architecture:** Parse management targets independently of file/space URIs. Normalize once into a typed InstanceID/ShareID identity before name resolution or API calls; preserve opaque IDs through update guards, cancellation and interruption diagnostics. Both forms share the existing authenticated V1 API paths, page derivation and confirmation behavior. URI components decode once; duplicate instances must match. The work adds target syntax without adding delivery output fields or changing HTTP(S) protocol rules.

- [x] Add failing no-instance URI/dry-run-equivalence, instance-conflict and optional-instance metadata regressions.
- [x] Add the pure typed target parser and route get/update/cancel, update guard, dry-run and interruption diagnostics through the selected identity.
- [x] Update CLI/registry conditional instance requirements, localized help/Describe/discovery/export semantics and typed MCP planning validation.
- [x] Add opaque/encoded identity, malformed-input, API request and MCP regressions; update README and bundled Skill/workflows.
- [x] Rebuild both entrypoints; run complete ADrive and entrypoint contracts plus actual terminal/JSON/URI/legacy local fixtures.
- [x] Read-only verify both get forms against the user's existing share without modifying it; test update/cancel only on local fixtures.
- [x] Complete independent seven-category review, resolve findings and record results; retain the work in a local commit without pushing.

Release cases: URI and legacy identity equivalence; missing bare-instance and conflicting URI-instance rejection before I/O; encoded opaque IDs decoded once; get/update/cancel wire IDs and update guards; by-name Instance selection and cancellation confirmation; bilingual metadata/MCP/Skill plus table and JSON outputs on both binaries. Creation still uses the file/space URI grammar.

Validation: 387 ADrive unit tests + 7 contracts passed; after a test-only opaque mutation/interruption coverage addition, all six focused URI management tests passed again. Unified functional contracts: 11 passed; standalone ADrive: 7 passed. Both rebuilt entrypoints passed local URI/legacy TTY/table/CSV/JSON and mutation identity/zero-I/O negative probes; actual existing-share get forms matched under the default OAuth profile with no cloud writes. Twelve bilingual command-Skill exports and help/Describe passed. Independent second complete review has 0/0/0/0 remaining findings. Detailed evidence and platform limits remain in the sharing review ledger.

## Instance list selector follow-up — 2026-10-14

The user identified the omitted analogous `share list adrive://instance` syntax. Audit all eight commands by their resource scope: create uses a file/folder URI or explicit selectors; list uses an instance URI or `--instance`; get/update/cancel use a share URI or ShareID plus `--instance`; inspect/files/download use the public HTTP(S) URL. Preserve existing filters, authentication, protocol and receiver behavior.

- [x] Reproduce the CLI and metadata omissions with failing regressions.
- [x] Add an instance-only selector accepting an optional root slash, matching duplicate flags, decoding once and rejecting malformed/conflicting input before I/O; use it in validation, plans, name resolution and typed MCP.
- [x] Update bilingual help, registry, Describe/discovery/Skill/MCP, README, bundled Skill/workflows and test instructions with the complete input matrix.
- [x] Run the ADrive suite and both entrypoint contracts; rebuild both debug binaries and verify local processes plus read-only real OAuth URI/legacy list equivalence.
- [x] Complete independent seven-category review, record at least five regression cases and fresh evidence, then commit on `xsj-dev`.

Validation: 397 ADrive unit tests + 8 contracts, 11 unified contracts and 7 standalone contracts; both rebuilt binaries passed local URI/root-slash/legacy/duplicate list output checks and read-only real OAuth list equivalence. All eight leaves passed bilingual help/Describe/Skill checks, including 32 exports and capability metadata. Independent second complete review has no remaining findings; its 24 real stdio MCP checks include selector-free Describe. See the sharing review ledger for corrections, six release regression groups and detailed evidence.
