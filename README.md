<!--
Copyright (c) 2025 Beijing Volcano Engine Technology Co., Ltd.

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
-->

# Volcengine Storage Unified CLI

Volcengine Storage Unified CLI provides three public command-line tools for
Volcengine storage workflows:

### `ve-tos-cli`

Use `ve-tos-cli` for Volcengine TOS object storage workflows, including bucket
and object operations, multipart transfers, presigned URLs, diagnostics,
capability discovery, and MCP serving.

### `tos-cli`

Use `tos-cli` for the ByteCloud TOS command surface. It shares the same storage
runtime foundations as `ve-tos-cli`, while exposing the dedicated `tos` command
behavior expected by ByteCloud users and scripts.

### `ve-adrive-cli`

Use `ve-adrive-cli` for Volcengine ADrive file workflows, including listing,
upload, download, sync, folder creation, diagnostics, capability discovery, and
MCP serving.

The internal `ve-storage-uni-cli` dispatcher is kept for local development and
cross-surface testing. Public package managers and curl installation expose the
three dedicated commands above.

## Security and privacy

This project takes security seriously.
For vulnerability reporting and supported versions, see [SECURITY.md](SECURITY.md)

## Installation

Choose one package manager. The commands below assume the package manager itself
is already installed and authenticated when required.

```bash
cargo install ve-tos-cli
cargo install tos-cli
cargo install ve-adrive-cli
```

```bash
npm install -g ve-tos-cli
npm install -g tos-cli
npm install -g ve-adrive-cli
```

```bash
pip install ve-tos-cli
pip install tos-cli
pip install ve-adrive-cli
```

```bash
brew tap volcengine/ve-storage-uni-cli https://github.com/volcengine/ve-storage-uni-cli
brew install ve-tos-cli
brew install tos-cli
brew install ve-adrive-cli
```

```powershell
winget install ve-tos-cli
winget install tos-cli
winget install ve-adrive-cli
```

Install all three CLIs with curl:

```bash
curl -fsSL https://github.com/volcengine/ve-storage-uni-cli/releases/latest/download/install.sh | sh
```

Install one CLI with curl:

```bash
curl -fsSL https://github.com/volcengine/ve-storage-uni-cli/releases/latest/download/install.sh | sh -s -- ve-tos-cli
curl -fsSL https://github.com/volcengine/ve-storage-uni-cli/releases/latest/download/install.sh | sh -s -- tos-cli
curl -fsSL https://github.com/volcengine/ve-storage-uni-cli/releases/latest/download/install.sh | sh -s -- ve-adrive-cli
```

See [packaging/README.md](packaging/README.md) for local build, packaging, and
release publishing details.

## Quick Start

Check that the commands are available:

```bash
ve-tos-cli --version
tos-cli --version
ve-adrive-cli --version
```

Configure TOS credentials with environment variables:

```bash
export TOS_ACCESS_KEY=<your-access-key-id>
export TOS_SECRET_KEY=<your-secret-access-key>
export TOS_SECURITY_TOKEN=<optional-sts-token>
```

`ve-tos-cli` supports `aksk or unified`. Unified
authentication selects the same-name profile managed by the external login
framework. It ignores local AK/SK in `config.toml` and `credentials.toml`; the
SDK supplies fresh signing credentials for each HTTP attempt. Login state is
owned externally, so use `ve login` instead of writing credentials through the
storage CLI.

```bash
ve-tos-cli --profile default --auth-mode unified ls
ve-tos-cli config set auth_mode unified
ve login
```

VeTos mode precedence is `--auth-mode` > `[profile.ve-tos].auth_mode` >
`TOS_AUTH_MODE` > the backward-compatible `aksk` default.

`tos-cli` supports `aksk` (default) and public, built-in `zti`. Select it with
`tos-cli --auth-mode zti ls tos://bucket/` or `tos-cli config set auth_mode zti`.
Its mode precedence is `--auth-mode` > `[profile.tos].auth_mode` >
`BYTETOS_AUTH_MODE` > `aksk`. ZTI uses `SEC_TOKEN_STRING`, a local Agent at
`ZTI_AGENT_SOCKET_PATH` (default `/run/zti-agent.sock`), or `SEC_TOKEN_PATH`,
in that order. It ignores AK/SK and does not fall back to them. Agent access
requires Unix; file Tokens are re-read per request, while Agent Tokens refresh
within 600 seconds or before expiration. `tos-cli doctor --check auth` reports
source availability offline; a storage request verifies remote access. ZTI does
not support `presign`, which requires AK/SK. Never place Token values in command
arguments or logs.

For ByteCloud TOS PSM discovery, `tos-cli --psm <service>` normally checks
bucket routing through BNS first. Set `TOS_FORCE_PSM=true` to skip BNS and
resolve the selected CLI/configured PSM directly through Consul. This setting
only affects `tos-cli` PSM mode; an explicit `--endpoint` takes precedence.
`doctor --check network` reports configuration and does not validate a PSM;
use a bucket request to test service discovery.

Configure ADrive credentials with environment variables:

```bash
export ADRIVE_ACCESS_KEY=<your-adrive-access-key-id>
export ADRIVE_SECRET_KEY=<your-adrive-secret-access-key>
export ADRIVE_SECURITY_TOKEN=<optional-sts-token>
```

ADrive supports `aksk, oauth, or unified`. Existing installations keep using
`aksk` by default.

```bash
ve-adrive-cli config set auth_mode aksk
ve-adrive-cli config set auth_mode oauth
ve-adrive-cli --profile default --auth-mode unified ls
ve-adrive-cli config set auth_mode unified
ve login
ve-adrive-cli config set auth_endpoint https://idsauth.volces.com
ve-adrive-cli auth login --instance inst-1
ve-adrive-cli auth status
ve-adrive-cli auth logout
ve-adrive-cli --dry-run auth logout
```

Mode precedence is `--auth-mode` > `[profile.adrive].auth_mode` >
`ADRIVE_AUTH_MODE` > the backward-compatible `aksk` default. Once a
mode is selected, credentials from the other mode are not used as fallback.
For one-off automation, pass `--auth-mode` without changing the Profile.
Unified selects the same-name externally managed profile and ignores local
AK/SK and OAuth tokens. `ve-adrive-cli auth login/logout` does not modify
Unified state; use `ve login` / `ve logout` through the external framework.
`auth logout --dry-run` reports whether the selected Profile would be cleared
without rewriting `credentials.toml` or creating local encryption key material.

`auth login` runs the Device Authorization flow in the foreground: it prints
the verification URL and then polls until the user finishes or the grant
expires. It requests the fixed `all` scope and saves
the returned Access/Refresh Token pair under the current Profile in encrypted
`credentials.toml`, along with any returned `user_id` identity metadata. OAuth
user-owned Space creation uses that metadata when `--owner-id` is omitted;
OAuth group-owned Spaces always require an explicit `--owner-id`. The Auth
endpoint priority is `--auth-endpoint` > `[profile.adrive].auth_endpoint` >
`ADRIVE_AUTH_ENDPOINT`; one of these sources is required for every new login.

OAuth resource requests use Bearer authentication. Before each command the CLI
reuses a valid Access Token or refreshes file-backed credentials when at most 60
seconds remain. A first resource `401` forces one coordinated refresh and one
replay; `403` never refreshes. The CLI is not a resident process and does not
refresh in the background. If Refresh is no longer possible, scripts receive a
`login_required` error; the CLI never starts an interactive login implicitly.

`ADRIVE_ACCESS_TOKEN` is supported for read-only process-scoped automation when
no OAuth Token exists in `credentials.toml`. Environment credential groups are
never mixed with file credentials and cannot be auto-refreshed because rotated
Refresh Tokens cannot be written back to the parent process environment.

Sensitive credentials are stored separately from normal configuration:

```text
$HOME/.tos/config.toml       # region, endpoint, auth_mode, and other settings
$HOME/.tos/credentials.toml  # encrypted AK/SK, security tokens, and OAuth tokens
```

New AK/SK writes from `tos-cli`, `ve-tos-cli`, and `ve-adrive-cli` go only to
`credentials.toml`. Existing AK/SK values in `config.toml` remain readable for
compatibility and are not migrated or duplicated automatically. Credential
precedence is `credentials.toml` > legacy `config.toml` > environment variables.
Bare credential keys are isolated by command surface:

```bash
tos-cli config set access_key_id <byte-tos-access-key>
tos-cli config set secret_access_key <byte-tos-secret-key>
ve-tos-cli config set access_key_id <volcengine-tos-access-key>
ve-tos-cli config set secret_access_key <volcengine-tos-secret-key>
ve-adrive-cli config set access_key_id <adrive-access-key>
ve-adrive-cli config set secret_access_key <adrive-secret-key>
```

These commands write `[profile.tos]`, `[profile.ve-tos]`, and
`[profile.adrive]`, respectively. Use an explicit key such as
`default.access_key_id` only when shared credentials for `tos-cli` and
`ve-tos-cli` are intended.

Initialize or inspect local configuration:

```bash
ve-tos-cli config init
# config init writes region=cn-beijing and endpoint=tos-cn-beijing.volces.com
ve-tos-cli config show --output json

tos-cli config set region cn-beijing
tos-cli config set endpoint https://your-bytetos-endpoint.example.com
tos-cli config show --output json

ve-adrive-cli config set region cn-beijing
ve-adrive-cli config set endpoint https://ids-cn-beijing.volces.com
ve-adrive-cli config show --output json
```

Run common TOS workflows:

```bash
ve-tos-cli capabilities --view groups
ve-tos-cli ls tos://my-bucket/prefix/ --output table
ve-tos-cli cp ./local.txt tos://my-bucket/local.txt --dry-run
ve-tos-cli cp ./local.txt tos://my-bucket/local.txt
ve-tos-cli stat tos://my-bucket/local.txt --output json
```

Run the ByteCloud TOS command surface:

```bash
tos-cli capabilities --view groups
tos-cli ls tos://my-bucket/prefix/ --output table
tos-cli cp ./local.txt tos://my-bucket/local.txt --dry-run
tos-cli sync ./dir tos://my-bucket/backup/ --recursive --dry-run
```

ADrive V1 share links expose nine commands: `share create`, `get`, `list`,
`update`, `cancel`, `inspect`, `files`, `view`, and `download`. The current Go SDK contract
requires OAuth Bearer / token-source authentication for share management. AK/SK
or Unified access to other resource APIs does not establish share-management
support. Receivers default to anonymous sessions and load no account credentials.
Explicit `--auth-mode oauth` uses the recipient's selected `--profile` for
`inspect`, `files`, `view`, and `download`.
Use `share <action> --help` or `--describe` for the complete typed contract.

| Commands | Target | Compatible explicit form |
| --- | --- | --- |
| `create` | `adrive://instance/space/path` (file or folder) | `--instance --space --folder/--file` |
| `list` | `adrive://instance` (optional root slash) | `--instance instance` |
| `get`, `update`, `cancel` | `adrive://instance/share-id` | `share-id --instance instance` |
| `inspect`, `files`, `view`, `download` | Public HTTP(S) `/share/<ShareID>` URL | Same URL; `files`, `view`, and `download` accept `--path` |

```bash
ve-adrive-cli share create adrive://instance/space/docs/ --permission download --expires-in 7d
ve-adrive-cli share get share-id --instance instance
ve-adrive-cli share get adrive://instance/share-id
ve-adrive-cli share list adrive://instance
ve-adrive-cli share list --instance instance --creator user-id --limit 100
ve-adrive-cli share update share-id --instance instance --require-login false --expires-in 0
ve-adrive-cli share cancel share-id --instance instance --yes
ve-adrive-cli share inspect https://instance.example.test/share/share-id
ve-adrive-cli share files https://instance.example.test/share/share-id --path docs/
ve-adrive-cli share download https://instance.example.test/share/share-id ./downloads --recursive --checkpoint ./share-checkpoint.json
ve-adrive-cli config set share_base_url https://pages.example.test
```

Create accepts one file/folder URI or explicit selectors, default permission
`view` (preview only), and default expiration `0` (never). Allow downloads explicitly
with `--permission download`, including directory receipt. Update retains explicit zero,
false and empty strings and fetches `UpdatedAt` when no guard is supplied. List
requires repeated `--creator` except when OAuth supplies its bound `user_id`;
search returns one page with original pagination fields. Cancel requires global
`--yes` outside ordinary TTY execution. Unknown mutation outcomes return
`share_result_unknown`: reconcile with get/list before retrying.

The page origin follows explicit `--share-base-url`, selected
`[profile.adrive].share_base_url`, `ADRIVE_SHARE_BASE_URL`, then endpoint/Instance
derivation. Standard `idsapi.volces.com` / `idsapp.volces.com` and
`idsapi-boe.volces.com` / `idsapp-boe.volces.com` hosts map automatically in both
directions, retaining the Instance label, scheme and port. Share pages, receiver
APIs and download addresses support HTTP and HTTPS; the CLI does not upgrade
protocols. Anonymous receivers derive the API origin
from the page URL without loading account configuration; use explicit `--endpoint`
for a custom receiver API. Explicit OAuth uses the selected profile's API endpoint
or `--endpoint`, so account credentials are not sent to a copied page origin.
Standard page Instance labels must match the OAuth binding. Other hosts are not rewritten. File and folder links both
use `/share/<ShareID>`, with no filename appended. Receivers also accept
`/share/<ShareID>/` and normalize its single trailing slash; it identifies the same
share and download checkpoint. Receiver `--path` is exact for
files and ends in `/` for directory listing; no path lists roots, including
single-file shares. `files` returns one marker page; `--limit` sets the page size
and the returned `NextMarker` goes into the next call's `--marker`, until
`IsTruncated=false`. The service documents a default/maximum of 100; the CLI
forwards any positive limit. Exact file selections return one file without directory
pagination. Default table/CSV uses the same `file_path`, `size`, `file_type`,
`updated_at`, `is_folder` columns and folder-first rows as `ls`; empty pages retain
these headers. The table Total counts displayed rows after folder marker normalization.
JSON and explicit `--query` retain the original service arrays and fields.
Download automatically enumerates pages. Download destinations are
directories; `--resume` requires a checkpoint file and resumes completed files.
Omitting `--path` selects all share roots; single-file shares select their file
automatically, while folders require `--recursive`. Preflight may create directory
scaffolding before a subsequent transfer denial, so empty local folders do not
indicate downloaded file contents. The default download table shows a cp-style
summary (total/succeeded/failed/skipped/pending), including empty results.
JSON and explicit queries retain file details. JSON Summary.Completed includes
Summary.Skipped, the validated completed files reused during resume; the default
table's succeeded count excludes those skipped files. Scanning and file-count
progress use stderr and are enabled on TTY by default. Use --progress to force
progress, --no-progress to disable it, or --quiet to suppress it. The two progress
flags are mutually exclusive. Forced progress on a pipe uses plain lines.
Files at least 20MB with a strong ETag or CRC64 automatically use 16MiB ranges with four concurrent
requests, even without a checkpoint. Use `--checkpoint-threshold 64MB` or
`--multipart-concurrency 8` to override those controls. Only failed ranges are
retried using the same issued URL; responses must honor Range exactly. The current
checkpoint resumes completed files; an interrupted file restarts its ranges.
Large-file progress also shows completed ranges for the current file.
Files without a strong ETag or CRC64 use a single streamed GET to avoid mixing versions.
`Forbidden.ShareDownloadDisabled` requires the share owner to allow downloads
with `--permission download`.
`share view <url> [--path exact/file]` previews a file using the share's view
permission. A single shared file is selected automatically; folders or multiple
roots require `--path` to a file. Default auto mode returns a short-lived PDF
`PreviewURL`, `ExpiresAt` and `PreviewType` for PDF/Office documents, or raw
content metadata for other files. It does not open a browser. Keep capability
URLs private. Each successful preview request consumes `PreviewLimit`, and
preview requests never automatically retry. Deployment support is required:
the previously tested BOE environment returned preview HTTP 404.

Use `share view <url> --path notes.txt --raw | less` for original bytes.
`--raw` requires a pipe or redirected stdout and conflicts with explicit
`--output`/`--query`; failures after streaming starts go to stderr. Default
structured mode supports normal JSON/table/query output and MCP tools.
A typical single-file
download makes five requests: share info, share token, root listing, download URL,
and content. An explicit --path replaces root listing with file metadata lookup.
New checkpoints bind the actual receiver API origin; OAuth also binds the selected
profile, OAuth mode and applied recipient subject when available. Environment/access-only
credentials without `user_id` bind API/profile/mode; the CLI cannot establish whether
an unknown-subject token replaced across resume belongs to another account and does
not decode tokens to infer identity. Legacy anonymous checkpoints can migrate only
when receiving through the URL-derived API. Changing a custom API requires a fresh task.

Passwords use bounded `--password-stdin` input read to EOF (use a pipe), preserving spaces and stripping
one terminal newline. If manually entering into `--password-stdin` on macOS/Linux,
press Enter then Ctrl-D within 30 seconds; Enter alone does not send EOF. A pipe
closes stdin automatically. For files/view/download in an ordinary interactive terminal,
omit the flag to use the hidden prompt and finish with Enter.
Receiver precedence is stdin, URL `pwd`, then a hidden TTY
prompt. JSON/noninteractive calls need stdin or URL password; `inspect` never reads
one. MCP's virtual write-only `password` overrides URL `pwd`, is piped to stdin,
and is removed from planned/executed argv. Each independent files/view/download call,
including subsequent pages, needs the code again; it is never persisted.
Login-required shares deny anonymous file access; explicit OAuth supplies the
recipient identity. Dry-run and describe never read passwords or create tokens.
Actual authenticated `create` and `get` results may include the service-returned
`SharePwd` for authorized delivery. The page URL has no password query. Keep that
delivery password out of logs and saved plans; list, update, and receiver output
omit it.

Share passwords are the service's extraction codes: create/update send `SharePwd`,
and files/view/download send the supplied code to `GetShareAccessToken` before browsing
or downloading. `inspect` reads public metadata without a password. Password clearing
is not established by the PRD; an empty value must not be described as confirmed
server-side clearing behavior.

`--require-login` maps to the V1 service's `RequireLogin` field and controls the
recipient's login requirement. Sharing creation and management still require the
owner's authenticated account. `false` permits anonymous receipt subject to password
and share permissions; `true` requires the recipient to log in. Default
files/view/download reject such shares, preserving service denial codes; when public
metadata is available, the CLI returns `share_login_required`. Inspect can still
read public metadata when the service permits it. To receive as a logged-in user:

```bash
ve-adrive-cli --profile receiver auth login --instance instance
ve-adrive-cli --profile receiver auth status
ve-adrive-cli --profile receiver share inspect "$SHARE_URL" --auth-mode oauth
ve-adrive-cli --profile receiver share files "$SHARE_URL" --auth-mode oauth --path docs/ --limit 100
ve-adrive-cli --profile receiver share download "$SHARE_URL" ./downloads --auth-mode oauth --recursive
```

Configure the recipient profile's API endpoint before login/receipt. A cached
token is not proof of service authorization: the API validates identity, instance,
scope and share rules. Existing stored-token refresh is reused; no login or account
fallback occurs automatically. A profile/environment OAuth default alone does not
activate authenticated receipt. Public info and token exchange use OAuth; file APIs
use only `x-share-token`, and signed content uses a credential-free client.
For a protected share, read once without echo and pipe to each command:

```bash
read -r -s -p 'Extraction code: ' SHARE_PASSWORD; printf '\n'
printf '%s\n' "$SHARE_PASSWORD" | ve-adrive-cli share files "$SHARE_URL" --password-stdin --limit 100
printf '%s\n' "$SHARE_PASSWORD" | ve-adrive-cli share files "$SHARE_URL" --password-stdin --limit 100 --marker "$NEXT_MARKER"
printf '%s\n' "$SHARE_PASSWORD" | ve-adrive-cli share download "$SHARE_URL" ./downloads --password-stdin --recursive
unset SHARE_PASSWORD
```

Add `--auth-mode oauth --profile receiver` to each receipt command when login is
required. Extraction codes and OAuth identity are independent checks.

`get`, `update`, and `cancel` accept either `adrive://instance/share-id` or
`share-id --instance instance`. A duplicate `--instance` must match the URI.
The URI contains exactly one ShareID segment and decodes components once;
`--by-name` resolves its Instance name. File/space URIs are interpreted separately
by their command context; sharing creation still selects `instance/space/path`.
`list` accepts `adrive://instance` or `--instance instance`, with the same filters,
pagination and output. The URI may end in one root slash and has no share/space/file
path. Components decode once; a duplicate instance flag must agree. `--by-name`
resolves the selected Instance name in either form.

With no output override, terminals display a table and pipes receive JSON.
Share detail tables include `ShareID`, `ShareUrl`, any authorized delivery
`SharePwd`, and file paths. Use `--output json` for structured results; retrieve
an existing share with `share get` or `share list` rather than creating it again
to change its display format.

Share downloads enumerate a bounded snapshot before requesting any signed file URL
(20,000 entries, 10,000 pages, 64 path components, 8 MiB manifest/checkpoint).
Folders require `--recursive`; the selected folder basename is retained once,
including empty folders. The OS resolves user-selected destination/checkpoint
directory aliases, including macOS `/tmp` and `/var`, before the CLI pins their
resolved roots; permission, disk and type errors remain native OS errors.
Downloaded subpaths with traversal, symlinks/reparse points, portable
case/Unicode collisions, or file/directory conflicts fail before URL issuance.
Existing files require `--overwrite`; replacement publishes only a fully verified
file. HTTP(S) content requests carry no account or share-token credentials and do
not follow redirects. Size, available CRC64 and strong ETag are checked, with
SHA-256 recorded locally. Content retries reuse the same signed URL.

`--checkpoint <file> --resume` reauthorizes and uses only the saved snapshot,
verifying each current remote fingerprint and each completed local SHA-256.
Changed files or a remote identity lacking both strong ETag and CRC64 fail safely.
New remote files are not added; partial files restart from zero. The checkpoint
and its `.lock` sidecar must not overlap download destinations, including aliases
to the same resolved path. Resume compares the resolved destination identity. A pre-existing
checkpoint requires `--resume`. Cancellation/failure produces one aggregate
response with `Complete`, `PlanComplete`, `CheckpointAvailable`, `Summary`, and
per-file `completed`/`failed`/`pending` states; failures exit nonzero. Definite
service rejection retains its error code, HTTP status, category and request ID
when supplied (for example, download-disabled 403 exits 5 with
`IssuanceOutcomeUnknown=false`). An unknown
URL-issuance outcome is never automatically replayed and may have consumed quota.
A later explicit resume can issue a new URL for a pending file. Counters remain
server-owned. Offline dry-run does not enumerate or write files/checkpoints.

分享下载先完整枚举快照，再申请文件下载地址；目录必须显式 `--recursive`，
仅保留一次选中目录名，并保留空目录。用户指定的根目录及 checkpoint 目录别名
由操作系统正常解析，固定实际根目录后再处理下载子路径；支持 macOS `/tmp`、
`/var`，权限及磁盘等错误由操作系统返回。下载子路径拒绝路径穿越、符号链接/Windows reparse
点、大小写及 Unicode 规范化冲突。默认不覆盖现有文件，`--overwrite` 也只接受
普通文件；大小、可用 CRC64 和强 ETag 校验通过后才原子发布。
`--checkpoint <文件> --resume` 只恢复原清单，重新验证远端指纹及已完成本地文件
SHA-256，不加入新文件，也不进行字节范围续传。检查点不保存密码、token 或签名
URL。失败或中断保留已完成文件，并以单个聚合响应和非零退出码报告状态。
标准 IDS 公网及 BOE 页面/API 域名自动对应；自定义页面用 `--share-base-url`，
接收 API 用 `--endpoint` 覆盖。明确 403 拒绝保留服务端错误码，退出码为 5，
`IssuanceOutcomeUnknown=false`；只有无法确定签发结果时才标记未知。

For reproducible OAuth validation, documentation/discovery checks and local receiver
regressions, follow [ADrive sharing test steps](docs/adrive-share-testing.md).

Dedicated live share tests: set `ADRIVE_E2E_INSTANCE` and `ADRIVE_E2E_SPACE`
for existing disposable test resources, optionally `ADRIVE_E2E_CREATOR` for
explicit creator filtering, `ADRIVE_E2E_SHARE_BASE_URL` for page origin, and
`ADRIVE_E2E_RECEIVER_ENDPOINT` when the receiver API differs from the page origin.
`ADRIVE_E2E_BINARY` selects a built unified or standalone CLI. Run
`python3 -m pytest scripts/e2e/adrive/test_share.py`; missing dedicated resources
skip the live test. It creates and cleans only its unique test folder and shares.
The current live harness injects AK/SK credentials. On the BOE service verified
on 2026-10-13, ordinary file operations succeeded but share creation returned
`AccessDenied`; a share search returned `unable to determine user identity`.
An authorized OAuth-backed runner is needed for deployments following the
current SDK share-management contract. This failure is not a passing live-share
validation and must not be converted into a skip.


Run common ADrive workflows:

```bash
ve-adrive-cli capabilities --view groups
ve-adrive-cli ls adrive://instance/space/path/ --output table
ve-adrive-cli cp ./local.txt adrive://instance/space/local.txt --dry-run
ve-adrive-cli mkdir adrive://instance/space/new-folder
ve-adrive-cli sync ./dir adrive://instance/space/backup/ --recursive --dry-run
```

Inspect command contracts before executing:

```bash
ve-tos-cli cp --describe --output json
tos-cli cp --describe --output json
ve-adrive-cli sync --describe --output json
```

Start MCP servers for agent clients:

```bash
ve-tos-cli serve --mcp
tos-cli serve --mcp
ve-adrive-cli serve --mcp
```

`stdio` is the default transport and is intended for an Agent that starts the
CLI as a child process. To use the local SSE transport, the Agent must run on
the same machine (and in the same network namespace) as the CLI:

```bash
ve-tos-cli serve --mcp --transport sse --port 9090
```

After the loopback listener binds, the command writes a fresh credential once
to stderr:

```text
MCP SSE listening on http://127.0.0.1:9090/sse
Authorization: Bearer <TOKEN>
```

Configure the MCP client to send that value in the `Authorization` header on
both the `GET /sse` stream and every `POST /message` request. Do not put the
credential in a URL, query parameter, or MCP payload. The accepted Host values
are exactly `127.0.0.1:9090` and `localhost:9090`. Native clients may omit
`Origin`; if present, it must be an HTTP loopback origin on the same port.
`--dry-run` and `--describe` report this contract without creating a token or
opening a listener.

## Common Options

Most commands share these options:

| Option                      | Env           | Description                                               |
|-----------------------------|---------------|-----------------------------------------------------------|
| `-P, --profile <PROFILE>`   | `TOS_PROFILE` | Configuration profile, default `default`.                 |
| `--config-path <PATH>`      | `TOS_CONFIG_PATH` | Config TOML path, default `$HOME/.tos/config.toml`.   |
| `--credentials-path <PATH>` | `TOS_CREDENTIALS_PATH` | Credentials TOML path; defaults beside the effective config file. |
| `-r, --region <REGION>`     |               | Region override.                                          |
| `-e, --endpoint <URL>`      |               | Data-plane endpoint override.                             |
| `--control-endpoint <URL>`  |               | Control-plane endpoint override for TOS.                  |
| `--account-id <ACCOUNT_ID>` |               | Account ID for control-plane operations.                  |
| `-o, --output <FORMAT>`     | `TOS_OUTPUT`  | `json`, `yaml`, `table`, `csv`, or `markdown`.            |
| `--query <JMESPATH>`        |               | Filter structured output.                                 |
| `--dry-run`                 |               | Preview without executing supported write operations.     |
| `--describe`                |               | Print a structured command contract.                      |
| `-y, --yes`                 |               | Auto-confirm supported destructive prompts.               |
| `--confirm <RESOURCE>`      |               | Confirm critical delete operations with the exact target. |
| `--no-color [<BOOL>]`       | `NO_COLOR`    | Disable colored output.                                   |
| `-v, --verbose`             |               | Verbose logs to stderr.                                   |
| `-q, --quiet`               |               | Suppress non-error output.                                |
| `--trace-dir <DIR>`         |               | Write trace diagnostics.                                  |
| `--trace-redact <LEVEL>`    |               | `strict`, `relaxed`, or `off`; default `strict`.          |

Credential variables are resolved by the config layer:

| Variable                | Description                                                                    |
|-------------------------|--------------------------------------------------------------------------------|
| `TOS_ACCESS_KEY`        | TOS access key ID.                                                             |
| `TOS_SECRET_KEY`        | TOS secret access key.                                                         |
| `TOS_SECURITY_TOKEN`    | Optional TOS STS security token.                                               |
| `TOS_AUTH_MODE`         | VeTos authentication mode: `aksk` or `unified`; ignored by `tos-cli`.          |
| `BYTETOS_AUTH_MODE`     | ByteCloud TOS authentication mode: `aksk` or `zti`.                            |
| `SEC_TOKEN_STRING`      | Highest-priority ZTI Token source; keep its value out of logs.                |
| `ZTI_AGENT_SOCKET_PATH` | Optional Unix Agent socket path; default `/run/zti-agent.sock`.              |
| `SEC_TOKEN_PATH`        | ZTI Token file, used if no environment Token or Agent is available.          |
| `ADRIVE_ACCESS_KEY`     | ADrive access key ID.                                                          |
| `ADRIVE_SECRET_KEY`     | ADrive secret access key.                                                      |
| `ADRIVE_SECURITY_TOKEN` | Optional ADrive STS security token.                                            |
| `ADRIVE_REGION`         | ADrive signing region; required when it cannot be parsed from the configured endpoint. |
| `ADRIVE_ENDPOINT`       | Required ADrive IDS resource endpoint.                                        |
| `ADRIVE_SHARE_BASE_URL` | Safe HTTP(S) share-page origin override (after CLI/profile config).            |
| `ADRIVE_AUTH_MODE`      | Process-scoped ADrive authentication mode: `aksk`, `oauth`, or `unified`.        |
| `ADRIVE_AUTH_ENDPOINT`  | OAuth Authorization Server used by the next explicit login.                      |
| `ADRIVE_DEFAULT_INSTANCE` | Default IDS Instance used by OAuth login.                                      |
| `ADRIVE_DEVICE_NAME`    | Device name displayed during OAuth authorization.                                |
| `ADRIVE_ACCESS_TOKEN`   | Read-only process-scoped OAuth Access Token.                                      |
| `ADRIVE_REFRESH_TOKEN`  | Process-scoped OAuth group field; it is informational unless an Access Token is also supplied. |

## Skill Installation

Choose the artifact according to the task:

| Entry | Purpose | How it is used |
|---|---|---|
| `skills/*/SKILL.md` in this repository | Agent workflows: setup, command selection, execution, recovery and verification | Install the matching CLI skill with its `references/` directory |
| `<cli> skill list --output json` | Live registry metadata and MCP input schemas | Discover tool IDs and contracts for the installed CLI version |
| `<cli> skill export` | A command reference pack with an index and individual command skills | Export locally, then install or read the selected Markdown artifact |
| `<cli> serve --mcp` | MCP server backed by the live command registry | Configure an MCP client to launch the CLI; it does not read exported Markdown |

Installing an agent skill does not install the CLI executable. Repository skills
teach multi-step workflows; exported command skills document individual command
syntax, parameters, examples and operational behavior. Use current `--help` and
`--describe` contracts if a skill and the installed binary differ.

The repository provides one installable AI-agent skill per public CLI:

```text
skills/ve-tos-cli
skills/tos-cli
skills/ve-adrive-cli
```

Each skill is a standard skill directory with `SKILL.md` plus optional
resources. Use any agent skill installer that can install from a GitHub folder
URL:

```text
https://github.com/volcengine/ve-storage-uni-cli/tree/main/skills/ve-tos-cli
https://github.com/volcengine/ve-storage-uni-cli/tree/main/skills/tos-cli
https://github.com/volcengine/ve-storage-uni-cli/tree/main/skills/ve-adrive-cli
```

For Codex skill installers, use this repo/path input:

```text
repo: volcengine/ve-storage-uni-cli
path: skills/ve-tos-cli
path: skills/tos-cli
path: skills/ve-adrive-cli
```

Install one skill by passing only the matching path or GitHub folder URL.
Restart Codex after installing skills so agents pick up the new instructions.

### Export command references

Exported commands follow the entrypoint used to generate them. Registry IDs and
MCP tool names do not change:

| Export invocation | Example inside the export |
|---|---|
| `tos-cli skill export --name tos_cp` | `tos-cli cp ...` |
| `ve-storage-uni-cli tos skill export --name tos_cp` | `ve-storage-uni-cli tos cp ...` |
| `ve-tos-cli skill export --name ve_tos_cp` | `ve-tos-cli cp ...` |
| `ve-storage-uni-cli ve-tos skill export --name ve_tos_cp` | `ve-storage-uni-cli ve-tos cp ...` |
| `ve-adrive-cli skill export --name ve_adrive_cp` | `ve-adrive-cli cp ...` |
| `ve-storage-uni-cli ve-adrive skill export --name ve_adrive_cp` | `ve-storage-uni-cli ve-adrive cp ...` |

For example, discover the exact tool ID, preview paths, then export:

```bash
tos-cli skill list --output json
tos-cli skill export --name tos_cp --language zh --dir ./tos-cp-skills --dry-run --output json
tos-cli skill export --name tos_cp --language zh --dir ./tos-cp-skills --output json
```

`--language` accepts `en` (default) or `zh`. Omit `--name` to export all command
skills. Use an exact tool ID from `skill list` for portable selection:

| CLI | Additional `--name` matching |
|---|---|
| `tos-cli` | Exact canonical command, such as `"tos cp"` |
| `ve-tos-cli` | Canonical command or suffix, such as `"ve-tos cp"` or `cp`; a suffix can match multiple commands |
| `ve-adrive-cli` | Canonical command, short command, domain, or supported legacy name |

Export creates `DIR/SKILL.md` as the index and
`DIR/{domain}/{skill_name}/SKILL.md` as command references. Both have skill
metadata. Read the root index with its children available, or install a single
command directory independently. Export itself does not install or register
anything with an agent. Keep the installed directory name consistent with the
skill's `name` frontmatter when your installer requires it.

The command refuses to overwrite existing target files, including the root
index. To update a pack, export into a fresh directory, review the result and
replace the installed copy. `--dry-run` previews paths without writing files.
The parameter table describes CLI syntax; the JSON schema describes MCP
arguments, so fields such as `execute` are not CLI flags.

## More Documentation

- [packaging/README.md](packaging/README.md): local builds, release archives,
  GitHub Release assets, npm, PyPI, Homebrew, WinGet, curl installer, and
  release publishing.
- [docs/api_implementation_principles.md](docs/api_implementation_principles.md):
  command-surface design and implementation principles.
- [scripts/e2e/README.md](scripts/e2e/README.md): live end-to-end test setup.

## License

Apache-2.0
