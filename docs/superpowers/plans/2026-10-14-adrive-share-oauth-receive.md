# ADrive explicit OAuth share receipt

User-approved mode: explicit --auth-mode oauth reuses selected profile; omitted mode remains anonymous. Spec: ../specs/2026-10-14-adrive-share-oauth-receive-design.md. Work on xsj-dev.

- [x] Add failing handler/transport regressions proving explicit OAuth receives a RequireLogin share while default receipt remains isolated.
- [x] Reuse Client's OAuth share transport only for public metadata/token exchange; route authenticated share sessions to configured API and preserve credential-free file/token boundaries.
- [x] Wire inspect/files/download through explicit mode selection; validate stored standard-instance binding and preserve authenticated API/mode/profile in download identity.
- [x] Add profile/missing/refresh/password/pagination/denial/redaction/zero-I/O and header-separation tests, typed MCP mode/schema support and compiled-process cases.
- [x] Sync bilingual help/Describe/discovery/Skill, README/workflows/test instructions, including --limit/--marker and password input examples.
- [x] Run complete ADrive/entrypoint suites, both process fixtures and read-only OAuth receipt; complete independent seven-category review, record >=5 cases and commit.

Verification and three independent review rounds: ../../adrive-share-oauth-verification.md. Read-only existing-share receipt and a safe owned synthetic-file RequireLogin/password/download/resume lifecycle passed. The owned share and file were cleaned up.
