# OAuth Endpoint Fallback Implementation Plan

**Goal:** Apply the user-approved same-origin support and fallback to the selected business endpoint, allowing explicit HTTP and HTTPS without upgrading protocols.

**Architecture:** Resolve business endpoint with its existing CLI > profile > environment priority, then append it after all explicit auth endpoint sources. Remove login/startup origin-inequality guards. Keep refresh pinned to the endpoint saved at login, redirects disabled, URL/userinfo validation and token redaction. Retain HTTPS as the default only for bare hosts.

**Scope:** Login resolution and doctor readiness, OAuth URL validation, OAuth client construction, help/Describe/Skill/MCP, README and active contract documentation. Existing share mapping and authentication selection stay unchanged. Isolated localhost tests use fake tokens and no real account login.

- [x] Add a real-entrypoint regression for omitted auth endpoint, all source priorities, same-origin login/resource/refresh, explicit HTTP, missing configuration, doctor and discovery. Observe the old implementation fail before production edits.
- [x] Convert the old same-origin rejection test to acceptance and add public HTTP normalization coverage. Remove obsolete inequality functions and the unused login-settings resource field; resolve fallback before constructing OAuthClient.
- [x] Include fallback in login availability checks; update diagnostic/help/registry text and bilingual catalogs. Document explicit-auth precedence, HTTP(S), and refresh pinning. Amend the local frozen contract with a dated user override.
- [x] Build both entrypoints; run focused red/green, full ADrive and root discovery/config regressions, existing OAuth/share scripts, formatting and syntax checks. Independently review all seven categories and correct Critical/Major issues.
- [x] Record full verification and remaining limitations. Integrate on xsj-dev with a commit, rebase onto freshly fetched origin/master and a normal push after checks; Git command results provide the integration record.

Validation: locked/offline Rust suites; `python3 tests/oauth_endpoint_regression.py <unified> <dedicated>`; existing `tests/share_*regression.py` on both rebuilt binaries. No cloud calls or modifications to user profiles are necessary.

Results: [verification and review record](../../adrive-oauth-endpoint-fallback-verification.md). 641 Rust tests and 15 regression groups passed across both entrypoints. Independent review round 1 found one Minor test-coverage issue; it was corrected and round 2 passed with no remaining findings.
