---
name: ve-adrive-cli
description: Use when managing Volcengine A-Drive resources with ve-adrive-cli, including AK/SK, OAuth, or Unified authentication, login and status, owner-aware Space creation and listing, and file or folder operations.
---

# ve-adrive-cli

Use `ve-adrive-cli` for Volcengine ADrive. Resolve one executable before planning commands and use that exact path for
the whole task:

1. Use an explicit executable path supplied by the user.
2. Otherwise use `command -v ve-adrive-cli` and verify it with `--version`.
3. In a `ve-storage-uni-cli` source checkout, fall back to
   `packaging/cargo/ve-adrive-cli/target/release/ve-adrive-cli` when executable.

Do not run storage operations if the binary is missing or cannot be verified.

## CLI installation

Installing this skill does not install the CLI executable or configure credentials.
First run `ve-adrive-cli --version`; if the selected executable works, skip installation.
Install only when it is in scope. Choose one channel for the current OS and
available tools. These channels are alternatives; do not run all commands.

| Channel | Suitable environment and behavior | Installation commands |
|---|---|---|
| Homebrew | macOS with Homebrew; installs a prebuilt binary | `brew tap volcengine/ve-storage-uni-cli https://github.com/volcengine/ve-storage-uni-cli`<br>`brew install ve-adrive-cli` |
| npm | Node.js and npm; installs a wrapper and downloads a platform binary | `npm install -g ve-adrive-cli` |
| pip | Python with pip; installs a wrapper and a matching OS/architecture wheel | `pip install ve-adrive-cli` |
| Cargo | Rust toolchain and build tools; compiles the dedicated executable | `cargo install ve-adrive-cli` |

<!-- [Review Fix #Install2] Preserve the npm postinstall prerequisite when inlining. -->
npm also needs access to GitHub Releases: its postinstall script downloads and
verifies the binary. Do not use `--ignore-scripts`. For pip, select the intended
Python environment and activate it when needed; `python -m pip install ve-adrive-cli`
uses the pip for that interpreter. Ensure the package manager's bin/scripts
directory is on PATH. Each channel requires an available build for the OS and CPU.

**Release install script (Linux/macOS):** choose this for a prebuilt binary without
a package manager. It needs a POSIX shell, curl, archive and checksum tools;
it detects the platform, downloads a GitHub Release archive and verifies its checksum:

```bash
curl -fsSL https://github.com/volcengine/ve-storage-uni-cli/releases/latest/download/install.sh | sh -s -- ve-adrive-cli
```

The script installs into `$HOME/.local/bin` by default; add that directory to PATH
or use the executable's full path. `VE_STORAGE_UNI_CLI_INSTALL_DIR` selects another
directory and `VE_STORAGE_UNI_CLI_VERSION` pins a release.

**From a source checkout:** developers can build and verify the dedicated artifact:

```bash
cargo build --release --manifest-path packaging/cargo/ve-adrive-cli/Cargo.toml
./packaging/cargo/ve-adrive-cli/target/release/ve-adrive-cli --version
```

Only copy a verified artifact onto PATH when installation is in scope. Do not
substitute another ADrive, TOS, or general cloud CLI.

**Verify installation:** run `ve-adrive-cli --version` using the selected executable.
If it is missing or reports an unexpected version, check PATH and the active
environment before reinstalling. Keep the verified path for the rest of the task.
A version check verifies executable availability, not authentication; continue
with profile/endpoint setup below and a harmless resource request.

Read `references/safety.md` before commands that write, delete, move, overwrite, sync, change config, or expose
identifiers/URLs.

## Task routing and command contracts

Use this skill to choose and combine commands. Read [Task workflows](references/workflows.md)
when uploading, downloading, synchronizing, deleting, or diagnosing a failed operation.
Read [Safety](references/safety.md) before making changes or sharing URLs.

| User intent                 | Start here                                  | Completion evidence                                             |
|-----------------------------|---------------------------------------------|-----------------------------------------------------------------|
| Inspect or locate resources | `ls`, `stat`, then `find`/`du` if supported | Exact target, metadata, and listing completeness                |
| Upload, download, or copy   | `cp --help`, then a concrete dry-run        | Destination metadata or local file verification                 |
| Reconcile a directory       | `sync --help` and a dry-run                 | Planned versus completed transfers; deletion count if requested |
| Remove or move resources    | Exact source/target and command contract    | Remaining source/target state, including failed items           |
| Diagnose authentication     | Selected profile and mode, then `doctor`    | Successful harmless resource request                            |

Use the installed version's help and structured contract as the authority. If a flag or command is missing, inspect its
help and report the version mismatch; do not guess equivalent flags or automatically change authentication modes. Keep
the same executable, profile, endpoint, and authentication mode throughout the task unless the user requests a change.

```bash
ve-adrive-cli cp --help
ve-adrive-cli cp --describe ./file.txt adrive://instance-id/space-id/file.txt --output json
ve-adrive-cli skill list --output json
ve-adrive-cli skill export --help
```

`--help` explains CLI syntax; `--describe` supplies the selected command's structured contract. `capabilities` discovers
supported command groups when the right command is unknown. `skill list` discovers exportable command guides; choose an
identifier from that output and use `skill export` when persistent command documentation is needed. Export writes
documentation; it does not run storage operations or configure an Agent to load it. For a unified CLI session, retain
that session's domain-qualified entry point when requesting an export.

## Discovery

Start read-only. Resolve and verify the executable first, then inspect the selected Profile. Keep full capability
discovery conditional because its compact JSON output is large; use it only when command support is uncertain:

```bash
ve-adrive-cli --version
ve-adrive-cli config show --output json
ve-adrive-cli --profile default --auth-mode oauth auth status --output json
ve-adrive-cli --profile default --auth-mode oauth doctor --check auth --output json
ve-adrive-cli capabilities --view groups --output json  # only when needed
```

Do not interpret an unset default `auth_mode` as proof that OAuth credentials are missing. Before the Profile selects a
mode, a generic `doctor` falls back to AK/SK and can produce an irrelevant missing-key warning. Use explicit OAuth
status and Auth Doctor during setup; run the generic Doctor after configuration.

## Connection configuration

ADrive has independent Resource and OAuth Authorization Server endpoints. Resource commands require an explicit
endpoint; a recognizable IDS endpoint may supply region, while a custom endpoint also needs an explicit region. New
OAuth logins additionally require an explicit Auth endpoint. The Resource and Auth endpoints must use different origins.

Endpoint values without a scheme default to `https://`; dotless service names with numeric ports, such as
`resource:9000`, are treated as hosts. Resource endpoints with an explicit non-HTTP (S) scheme are rejected before any
request.

Configure AK/SK only when the user selects that mode. Never print the supplied credentials or read them back as
plaintext:

```bash
ve-adrive-cli config set auth_mode aksk
ve-adrive-cli config set access_key_id <adrive-access-key>
ve-adrive-cli config set secret_access_key <adrive-secret-key>
ve-adrive-cli config set region cn-beijing
ve-adrive-cli config set endpoint https://ids-cn-beijing.volces.com
```

Bare AK/SK keys are stored under the active profile's `[profile.adrive]`
credentials section, independently from both TOS command surfaces.

## Authentication modes

<!-- [Review Fix #2] Keep this skill aligned with the ADrive registry and README. -->
ADrive supports `aksk, oauth, or unified`. Mode precedence is
`--auth-mode` > `[profile.adrive].auth_mode` > `ADRIVE_AUTH_MODE` > `aksk`.

Use AK/SK when the user explicitly requests it:

```bash
ve-adrive-cli ls adrive://<instance-id> --output json
```

Unified selects the same-name externally managed profile and ignores local AK/SK and OAuth credentials. The storage CLI
does not modify Unified login state; use `ve login` and `ve logout`:

```bash
ve-adrive-cli config set auth_mode unified
ve login
ve-adrive-cli --profile default --auth-mode unified ls
ve-adrive-cli doctor --check auth --output json
ve-adrive-cli ls adrive://<instance-id> --output json
```

## OAuth login

Default to OAuth for a user's cloud drive. When the user asks to set up or log in, configure the selected Profile once
so later Resource commands stay short. For normal public ADrive, require only the Instance ID and use:

- Auth base URL: `https://<instance-id>.idsauth.volces.com`
- Resource base URL: `https://idsapi.volces.com`
- Resource signing region: `cn-beijing`

The Auth host is Instance-scoped. The Resource host is shared; the CLI appends
`/v1/instances/<instance-id>/...` to Resource requests.

The signing region is a CLI connection input, not a discovered statement about where the Instance is physically
deployed.

Persist the non-secret connection settings in the selected Profile before the first login. `config set` writes one key
at a time, so set every field once and then re-read the Profile to catch a partial setup. If the user explicitly
requests a non-persistent invocation, pass the same values as command or environment overrides instead:

```bash
ve-adrive-cli config set auth_mode oauth
ve-adrive-cli config set auth_endpoint https://<instance-id>.idsauth.volces.com
ve-adrive-cli config set endpoint https://idsapi.volces.com
ve-adrive-cli config set region cn-beijing
ve-adrive-cli config set default_instance <instance-id>
ve-adrive-cli config show --output json
```

Pass only the base URL to `--auth-endpoint`; the CLI appends paths such as
`/v1/oauth/device_authorization` and `/v1/oauth/token`. Keep the foreground process running while the user completes
Device Authorization:

```bash
ve-adrive-cli auth login --output json
```

Login is a mixed-stream foreground operation: authorization instructions arrive as plain text on stderr, while the final
result uses the selected stdout format. Read stderr incrementally, preserve the original process/session, and wait for
that same process after authorization; do not start a second login.

Forward the complete verification URL exactly as the CLI returns it. The CLI accepts HTTPS URLs, plus HTTP only for
loopback development URLs, and rejects userinfo or unsafe schemes before printing. The URL is supplied by the
Authorization Server and its UI origin can differ from the Auth API origin. The CLI may not print a separate User Code;
only display one separately when it is explicitly present in the returned URL. Never print Device Codes, Access Tokens,
Refresh Tokens, authorization headers, or stored secret material.

After authorization, require the original process to succeed, then verify explicit OAuth status and Auth Doctor. Status
plus Doctor validate local credential readiness; finish a real end-to-end check with a harmless Resource command such as
`ls`. A generic Doctor is primarily diagnostic, and even its live-network check only establishes endpoint reachability
rather than business API success.

Treat an OAuth status as reusable only when `ready` is true and its
`instance_id` matches the requested Instance. Otherwise start a new explicit login for the requested Instance.

OAuth login stores returned identity metadata such as `user_id` with the selected profile. It does not decode identity
from a Token.

If a login or refresh response does not provide `user_id`, run
`ve-adrive-cli --auth-mode oauth auth login --instance instance-id` again with the OAuth-authorized Instance ID, or pass
`--owner-id` explicitly for the owner-aware operation.

If Auth returns 404, verify the Instance ID and documented Auth base URL. If a Resource request rejects a Token, inspect
explicit OAuth status, requested Instance, and Resource endpoint before deciding whether to log in again. Do not use the
Auth endpoint as the Resource endpoint.

## Space listing and ownership

After Profile setup, reuse its selected mode, Resource Server, and region. Do not repeat connection flags on normal
Resource commands:

```bash
# Spaces owned by the logged-in user.
ve-adrive-cli ls adrive://<instance-id> --owner-type user --output json

# Spaces owned by groups available to the logged-in user.
ve-adrive-cli ls adrive://<instance-id> --owner-type group --output json
```

For a deliberate non-persistent mode override, the equivalent commands are
`ve-adrive-cli --auth-mode oauth ls adrive://instance-id` and
`ve-adrive-cli --auth-mode oauth ls adrive://instance-id --owner-type group`. Normal Profile-based use should keep the
shorter commands above.

For each owner collection, read `data.is_truncated` and `data.next_marker`. When truncated, pass the opaque marker to
the next request and repeat until
`is_truncated` is false; never construct or modify a marker:

```bash
ve-adrive-cli ls adrive://<instance-id> --owner-type user \
  --max-keys 1000 --marker '<previous-next-marker>' --output json
```

Run the same loop for `--owner-type group`. Count any separately returned
`data.root_space` once across all pages. Report user-owned and group-owned counts separately before the total.

After discovering Spaces, list each Space root with the same pagination loop:

```bash
ve-adrive-cli ls adrive://<instance-id>/<space-id>/ \
  --max-keys 1000 --output json
```

For creation, OAuth user-owned Spaces default `--owner-id` to the stored
`user_id`; group-owned Spaces always require an explicit owner ID:

```bash
ve-adrive-cli crt adrive://<instance-id>/personal --owner-type user

ve-adrive-cli crt adrive://<instance-id>/team \
  --owner-type group --owner-id <group-id>
```

Instance creation retains the authentication-mode-specific service defaults:

```bash
# AK/SK defaults to arkclaw.
ve-adrive-cli crt adrive://<instance-name>

# OAuth defaults to paas after the Profile selects OAuth.
ve-adrive-cli crt adrive://<instance-name> --service-type paas
```

## File and folder commands

<!-- [Review Fix #2] SyncArgs has no --recursive option; sync examples omit it. -->

Use the mode, Resource endpoint, and region already stored in the selected Profile. Add connection flags only for a
deliberate one-off override:

```bash
ve-adrive-cli ls adrive://<instance-id>/<space-id>/path/ --output json

ve-adrive-cli stat \
  adrive://<instance-id>/<space-id>/path/file --output json

ve-adrive-cli cp ./file.txt \
  adrive://<instance-id>/<space-id>/file.txt --dry-run

ve-adrive-cli sync ./dir \
  adrive://<instance-id>/<space-id>/prefix/ --dry-run

ve-adrive-cli mkdir \
  adrive://<instance-id>/<space-id>/new-folder
```

Use `ve-adrive-cli <command> --help` when non-connection flags are uncertain.

## Share links (V1)

Use `share create`, `share get`, `share list`, `share update`, and `share cancel`
for authenticated management. Use `share inspect`, `share files`, `share view`, and
`share download` for public receiver links. Receipt defaults to anonymous without
loading account configuration, even if a profile configures OAuth. Explicit
`--auth-mode oauth` uses the selected recipient `--profile` for all four commands.
Anonymous files/view/download reject login-required shares, preserving service denial
codes; when public metadata is available, the CLI returns `share_login_required`.

| Commands | Target | Compatible explicit form |
| --- | --- | --- |
| `create` | `adrive://instance/space/path` | `--instance --space --folder/--file` |
| `list` | `adrive://instance` or `adrive://instance/` | `--instance instance` |
| `get`, `update`, `cancel` | `adrive://instance/share-id` | `share-id --instance instance` |
| `inspect`, `files`, `view`, `download` | Public HTTP(S) `/share/<ShareID>` URL | Same URL, with optional `--path` for files/view/download |

The current Go SDK requires OAuth Bearer / token-source authentication for share
management. AK/SK or Unified success on ordinary file APIs does not establish
share-management support. If the user explicitly selects AK/SK, keep that choice
and verify the service contract when sharing returns `AccessDenied` or
`unable to determine user identity`; do not switch to OAuth, invent user-identity
headers, change permissions, or mark the live test passed without evidence.

Create one file or folder share with an `adrive://` target or explicit
`--instance --space --folder/--file` selectors. Do not mix URI and selectors.
`--by-name` resolves Instance and Space names. Permission defaults to `view`;
`--permission view` permits viewing. `--expires-in 0` (default) never expires;
other durations use seconds or `s`, `m`, `h`, `d`, `w` suffixes. Optional
`--require-login true|false`, `--preview-limit`, and `--download-limit` are explicit
settings, and zero limits mean unlimited.
Downloads require explicit `--permission download`. This is the PRD's preview-only
default mapped to V1; the SDK does not inject a create permission.

`SharePwd` and `RequireLogin` are service fields. The owner authenticates for share
management; RequireLogin controls the recipient instead. False permits anonymous
receipt subject to the share/password rules. True requires recipient login: opt
into `--auth-mode oauth` after `auth login --instance <instance>` and `auth status`
on the recipient profile. Cached-token status does not establish service access;
the API validates login, instance binding, scope and share rules. Existing refresh
is reused, but login and fallback are never automatic. OAuth authenticates public
info/token exchange, then file APIs use only `x-share-token`; signed content carries
no account credentials. Share passwords are extraction codes exchanged for a
share access token, not account credentials.

Management `get/update/cancel` accept both `adrive://instance/share-id` and
`share-id --instance instance`. URI input supplies the Instance; a duplicate
`--instance` must agree. The ShareID is one encoded segment and components decode
once. `--by-name` resolves only the Instance name. A file/space URI belongs to
its file-operation context; share creation still uses `instance/space/path`.
`list` accepts an instance URI or `--instance`, with identical filtering and
pagination. A root slash is optional; no share/space/file path is accepted. URI
components decode once, duplicate flags must agree, and `--by-name` resolves
the Instance name for either form.

```bash
ve-adrive-cli share get adrive://instance/share-id
ve-adrive-cli share get share-id --instance instance
ve-adrive-cli share list adrive://instance
ve-adrive-cli share list --instance instance
ve-adrive-cli share update adrive://instance/share-id --name New
ve-adrive-cli share cancel adrive://instance/share-id --yes
```

The returned page URL is `/share/<ShareID>` for both files and folders. Receivers
also accept `/share/<ShareID>/`, normalizing one trailing slash to the same share
and checkpoint identity. Directory prefixes use `--path`, not extra URL segments. Its origin
uses `--share-base-url` > `[profile.adrive].share_base_url` >
`ADRIVE_SHARE_BASE_URL` > the API endpoint with the validated Instance host label.
Public `idsapi.volces.com` / `idsapp.volces.com` and BOE
`idsapi-boe.volces.com` / `idsapp-boe.volces.com` map automatically between API
and page origins, retaining the Instance label, scheme and port. Share pages,
receiver APIs and download addresses support HTTP and HTTPS without protocol
upgrades. Other hosts are unchanged.
Anonymous receivers derive API origins from the page URL without reading account config;
explicit OAuth uses the selected profile API or `--endpoint` and validates a standard
page's Instance against the stored OAuth binding. This keeps credentials away from
arbitrary copied page origins. Use explicit `--endpoint` for custom receiver APIs. Page overrides remain
literal and do not select the receiver API.
Use the configured HTTP(S) origin. Never append a filename.

List uses repeated `--creator` filters. Only OAuth's bound `user_id` may supply an
omitted creator; otherwise the CLI requires an explicit creator. This argument
rule does not establish that the service accepts another auth mode. Optional status
is `active|expired|revoked`, creation bounds are RFC3339, offset is at most 10000,
and limit is positive. Each invocation returns one page, preserving
`Items`, `Total`, `Offset`, and `Limit`.

Update changes only supplied settings, forwarding zero, false and empty strings.
It fetches `UpdatedAt` unless `--updated-at` is explicit. The shared target cannot
change. Cancel uses global `--yes` in JSON, quiet or noninteractive execution;
ordinary TTY execution asks once. Dry-run never confirms or reads password stdin.
Cancellation uses API selectors and does not resolve a share-page origin. Ctrl-C
restores terminal echo; an interruption after mutation submission requires
reconciliation with get/list before retrying.

Use `--password-stdin` with a pipe; it reads to EOF and keeps a password out of argv.
For manual macOS/Linux stdin, press Enter then Ctrl-D within 30 seconds. Enter alone
does not close stdin. Files/download can instead use the ordinary hidden terminal
prompt without this flag and finish with Enter. One terminal LF/CRLF is
removed and significant spaces are preserved. The PRD does not confirm password
clearing; do not treat empty input as a documented clearing contract.
Passwords are bounded to 4096 UTF-8 bytes and reject controls
or multiple lines. MCP's virtual `password` is write-only and piped to child stdin;
explicit password takes precedence over a URL `pwd` query. Do not place passwords
in commands, saved plans, or logs.
Actual authenticated create/get results may include the service-returned
`SharePwd` for authorized delivery. Share the safe page URL and delivery password
through the authorized channel. List/update and receiver results omit passwords.

With no output override, terminals use a table and pipes use JSON. Share detail
tables preserve `ShareID`, `ShareUrl`, authorized delivery passwords and file
paths. Use `--output json` for structured data. To recover a previous creation's
ID/URL, use read-only `share list`/`share get`; do not repeat `share create` merely
to select another output format.

`share inspect` reads public info without share-token/password I/O; explicit OAuth
can load/refresh the selected account token. `share files`
uses exact metadata for a file `--path` and directory listing for a trailing `/`;
no path lists share roots, including a single-file share. Preserve returned paths
exactly. Listing returns one page with `NextMarker` and `IsTruncated`; pass
`--marker` for another page and `--limit` for its size (service default/max 100;
CLI accepts positive limits). Default table/CSV uses the same `file_path`, `size`,
`file_type`, `updated_at`, `is_folder` columns and folder-first rows as `ls`, with
stable headers even on empty pages. Table Total counts normalized display rows;
JSON and explicit `--query` preserve the raw service arrays. Download enumerates
pages automatically. Each
independent call/page needs the extraction code again; passwords are never saved.
OAuth does not bypass password checks. Receiver password precedence is stdin > URL `pwd` >
a hidden ordinary-TTY prompt; JSON/noninteractive execution requires stdin or URL.
No alternate password is retried after an explicit failure.

`share download <url> <destination-dir>` uses a directory destination, with
no `--path` selecting all share roots. A single-file share automatically selects
its file; folders require `--recursive`. Preflight may create directory scaffolding
before a later transfer denial, leaving empty folders. Default table output shows
the same summary style as cp (total/succeeded/failed/skipped/pending), including
empty results. JSON and explicit queries retain file details. Summary.Completed
includes Summary.Skipped; the table's succeeded count excludes resumed files.
Scanning/file-count progress uses stderr, defaults to TTY, can be forced by
--progress, and is disabled by --no-progress or --quiet. The progress flags are
mutually exclusive. Forced non-TTY progress uses plain lines.
`Forbidden.ShareDownloadDisabled` is a share policy error: the share
owner must enable `--permission download`.
Supported options are `--path`, `--recursive`, `--overwrite`, `--progress`,
`--no-progress`, `--checkpoint-threshold <size>`, `--multipart-concurrency <n>`,
`--checkpoint <file>`, and
`--resume` (requires checkpoint). Checkpoints track completed files, not partial
byte ranges. Inspect the live `--describe` contract before transfer execution.
`share view <url> [--path exact/file]` previews under the share's view permission.
No path selects the unique shared file; folder/multiple-root shares require an
exact file path. Default auto returns `ResultType=document` with short-lived
PDF `PreviewURL`/`ExpiresAt`/`PreviewType` for supported PDF/Office files, or
`ResultType=raw` with content metadata for other files. No browser opens.
`share view <url> --path notes.txt --raw | less` streams original bytes; terminal
stdout and explicit `--output`/`--query` are rejected. MCP uses structured mode.
Successful preview requests consume `PreviewLimit` and never automatically
retry. Keep descriptor URLs private. Unsupported deployments can return 404;
the previously tested BOE preview endpoint returned 404.
A normal single-file download uses info/token/root-list/download-url/content;
an explicit --path uses get-file in place of root-list.
New checkpoints record the actual API origin for both receipt modes. OAuth records
the applied profile/OAuth mode and recipient subject when available, and rejects a subject change during share-token
renewal. Legacy anonymous journals migrate only with the URL-derived API; an API
override needs a fresh task if the old journal did not record it.
Environment/access-only credentials without `user_id` bind API/profile/mode only;
the CLI cannot determine whether an unknown-subject token replaced across resume
belongs to a different account. Never decode tokens to invent a subject.

Read `references/workflows.md` and `references/safety.md` for share workflows.

Share download safety/recovery contract: enumerate at most 20,000 entries / 10,000
pages / 64 path components / 8 MiB before issuing any signed URL. Preserve the
selected directory basename once and empty directories. The OS resolves user-selected
destination/checkpoint directory aliases before the CLI pins their actual roots;
macOS `/tmp` and `/var` work without special-case rules. Native permission/type/disk
errors are retained. Downloaded subpaths reject traversal,
symlinks/reparse points, case/Unicode and ancestor collisions; `--overwrite`
accepts only ordinary files. Transfer with an independent credential-free HTTP(S)
client, no redirects, size/CRC64/strong ETag checks and local SHA-256. A signed URL
is issued once per attempted file; bounded content retries reuse that address.
`--resume` requires the same share origin/ID, selection, resolved destination and overwrite/
recursive policy, rechecks remote fingerprints and completed local hashes, and
never adds new files or resumes partial byte ranges. A checkpoint with no strong
remote identity is refused. The checkpoint and lock cannot overlap mapped destinations,
even through local directory aliases. Explicit service rejection retains its service
code/status/category/request ID when available; download-disabled 403 exits 5 with
permission-denied kind and `IssuanceOutcomeUnknown=false`. Do not blindly retry unknown URL issuance: quota may
already have been consumed. The aggregate has `Complete`, `PlanComplete`,
`CheckpointAvailable`, `IssuanceOutcomeUnknown`, `Summary` and per-file states.

Shared files at least 20MB with a strong ETag or CRC64 automatically use concurrent
16MiB Range requests; files without either use one streamed GET to avoid mixing versions.
`share download --checkpoint-threshold <size> --multipart-concurrency <n>` adjusts
the threshold and default 4 concurrent ranges without requiring a checkpoint.
Failed content ranges reuse the issued URL; checkpoint recovery remains file-level.
Range completion is shown on stderr alongside the aggregate file count.
