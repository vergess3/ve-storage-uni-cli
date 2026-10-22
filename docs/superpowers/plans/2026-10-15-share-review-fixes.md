# Share review fixes implementation plan

> **For agentic workers:** Use executing-plans or subagent-driven-development task by task, with independent specification and code-quality review.

**Goal:** Implement the user-authorized D02–D06 fixes and give recommendations for the report's remaining contract differences. Explain D01 without guessing an unprovided domain mapping.

**Architecture:** Download dry-run becomes a separate read-only planning path. It reuses receiver authentication, bounded manifest enumeration and checkpoint validation, and inspects local paths without creating directories, files or locks. Resume mismatches use cp's existing Conflict semantics; intentional local path rejection uses a stable local conflict category; empty explicit stdin passwords fail; download denials avoid asserting permission versus quota without evidence.

**Tech stack:** Rust, existing CliError/Envelope/SafeDir, Python actual-entrypoint localhost fixtures.

## Scope and decisions

The user explicitly approved implementation of D02–D06 after discussion. Remain on xsj-dev; commit locally after tests and independent review; do not push. No server deployment, real-share mutation, V2 migration, new public aliases, or guessed adriveapp-domain rewriting is included. Existing explicit HTTP(S), anonymous default and OAuth opt-in remain in force.

- D02: metadata queries are allowed; URL issuance, content transfer, destination/checkpoint/lock writes are forbidden. Return selected paths, mapping, file/directory counts, total bytes and conflicts. A complete plan with conflicts exits Conflict, retaining the plan in data. Describe remains offline. Resume planning reads the saved journal and validates identities/content without locking or saving it.
- D03: use Conflict for local completed-file size/hash mismatches, consistent with cp checkpoint mismatch handling. Preserve existing local data and journal.
- D04: intentional symlink/reparse/nonordinary rejection yields a local conflict and local remediation. Preserve original OS errors for unrelated I/O failures and preserve no-follow checks.
- D05: omitted password flag remains optional; explicit EOF/LF/CRLF empty fails before create/update or token exchange. Dry-run validates explicit password input without emitting it. Preserve significant spaces and help/describe's no-input behavior.
- D06: preserve service code/status/request ID; use policy-or-quota wording when the service code is ambiguous. Never decrement/check quotas as an authorization substitute.

## Tasks

- [x] Add red regression assertions for detailed dry-run, traversal/pre-existing target, read-only checkpoint/resume, empty explicit passwords, local content conflicts and policy/quota guidance. Run before implementation.
- [x] Implement diagnostic/password fixes with focused tests; compare actual cp Conflict behavior and annotate review fixes.
- [x] Implement read-only destination/checkpoint inspection and plan rendering; reuse enumeration and resume verification without calling the writing execution path. Cover existing aliases, symlink leaves/parents, missing roots, directory collisions and checkpoint overlap.
- [x] Update EN/ZH help, Describe/Skill/MCP side effects and dry-run schema, README, bundled skill and sharing test guide. Preserve execution output fields and V1 management contracts.
- [x] Build both binaries, run core/root/standalone tests and meaningful sharing localhost regressions. Independently review specification then all seven quality categories, repair Critical/Major findings and re-review. Record results and remaining contract recommendations before local commit.

## Contract recommendations to deliver

Retain the previously approved V1 SDK and wire field names; retain --permission/--expires-in rather than silently introducing V2 aliases. Inspect returns available public metadata without manufacturing filename/type; selection may use files/meta. Do not invent a hard 100-item maximum from a single sample. Preserve transfer result schema and add required planner fields explicitly. Describe must state local writes, checkpoint writes/overwrites and possible server download-count consumption; destructive=false does not imply no side effects. Retain V1 revoked and management Offset/Limit/Total separately from receiver Marker pagination. D05 is a confirmed fix, no longer an unresolved product question.
