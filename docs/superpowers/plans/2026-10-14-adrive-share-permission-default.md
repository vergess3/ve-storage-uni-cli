# Align sharing permission with the PRD

The user questioned the documented default. Read-only verification of PRD revision 94 and API document revision 224 confirms default preview; the latest V1 SDK (`bf6b098`) exposes `view`/`download` and does not supply a create default. The CLI's explicit `download` default is an implementation discrepancy.

- [x] Reproduce the mismatch in dry-run, create wire body and create/update metadata tests.
- [x] Set create-only permission fallback to V1 `view`; preserve explicit download and omitted update permission.
- [x] Explain the default in bilingual help, Describe/Skill/MCP, README and workflows; document directory receipt, password fields and creator/receiver login roles accurately.
- [x] Run focused and complete ADrive tests, both entrypoint contracts and compiled-process documentation/plan checks; complete independent seven-category review.
- [x] Record review/validation evidence and commit on `xsj-dev`.

Final evidence: 399 ADrive unit tests + 8 sharing contracts, 11 unified contracts and 7 standalone contracts passed. Both rebuilt debug entrypoints passed local HTTP process checks for create default view/explicit download, update omission, zero-I/O plans and SharePwd/RequireLogin wire fields. Both languages passed help/Describe/Skill defaults and eight named-command exports. Review round 1 found one stale test expectation (Major); round 2 has no remaining findings. Password-clearing semantics remain an explicitly unconfirmed service contract, not a claimed behavior correction. Detailed evidence and six release cases are in the sharing review ledger.
