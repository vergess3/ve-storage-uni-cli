# Task workflows

Read the command's installed `--help` and `--describe` before adapting these
examples. Replace example targets with the user's actual paths. Establish
profile, authentication, endpoint, and target scope using SKILL.md first.

## Upload and download

1. Identify the direction, exact destination, and whether the source is one
   file or a directory/prefix. Inspect remote state with `ls` or `stat`, and
   inspect local file existence and available disk space for a download.
2. Use explicit filenames for a single transfer to avoid ambiguous destination
   mapping. For a directory/prefix, use `cp --recursive`; inspect the mapping
   with `--dry-run` before executing.
3. Check planned writes, skipped entries, and warnings. A dry-run is a preview,
   not evidence that objects were transferred or that every runtime permission
   will succeed. Execute the same scoped command without `--dry-run` once it
   matches the user's request.
4. Verify the destination and summarize transferred, skipped, and failed items.

```bash
ve-adrive-cli cp ./file.txt adrive://instance-id/space-id/file.txt --dry-run --output json
ve-adrive-cli cp adrive://instance-id/space-id/file.txt ./downloaded-file.txt --dry-run --output json
ve-adrive-cli cp ./dir/ adrive://instance-id/space-id/backup/ --recursive --dry-run --output json
ve-adrive-cli stat adrive://instance-id/space-id/file.txt --output json
```

Use `adrive://instance-id/space-id/path` with discovered IDs. Use `--by-name`
only when intentionally resolving names; do not silently interpret an unknown
ID as a name. `--include-parent` on recursive copy includes the source directory
or prefix name. Inspect the dry-run mapping before relying on trailing-slash
behavior or replacing existing files. Resolve Space ownership and pagination
using the corresponding SKILL.md sections before selecting a destination.

Read filter and overwrite options from the contract instead of assuming shell
glob or sync semantics. Quote patterns such as `'*.log'` so the CLI receives
them unchanged. For large transfers, inspect `--checkpoint` and the checkpoint
location before selecting resumable transfer; retain state for a retry.

## Sync

Use `sync` for incremental reconciliation and `cp --recursive` for a recursive
copy. Do not add `--delete` unless removal of destination-only entries is part
of the user's request. Preview both directions and filters carefully:

```bash
ve-adrive-cli sync ./dir/ adrive://instance-id/space-id/backup/ --dry-run --output json
ve-adrive-cli sync adrive://instance-id/space-id/backup/ ./restore/ --dry-run --output json
```

`sync` has no `--recursive` flag. Check its comparison options before choosing
size-only or timestamp policies. When using `--delete`, inspect the destination
scope and planned removals and preserve the contract's required force and
confirmation flags. After execution, read the result/report and rerun a dry-run
with the same comparison options to identify remaining work. A zero-work plan
is evidence under those comparison rules, not a byte-for-byte integrity proof.

## Delete

List the exact target first. Distinguish one object/file from a prefix/folder;
recursive deletion expands the affected set. Read `rm --help` and `rm --describe`
with the target and preview the operation with `--dry-run` when supported.
Only execute within the already authorized scope, preserving required `--force`
and exact `--confirm` targets. Do not add broader flags to bypass a refusal.
For `mv`, verify the destination and source-removal result because moving
includes a destructive source operation. Re-list the affected scope afterward;
an authorization or network error is not proof of absence.

## Failure and verification

- Check exit status as well as structured stdout and diagnostics on stderr.
  A batch may have completed some items before failing: inspect failed/skipped
  counts and reports before claiming success or retrying the entire batch.
- Validation errors: correct target syntax or unsupported flag combinations
  using local help. Authentication/permission errors: check the selected mode,
  profile, endpoint, and access to the exact target; do not switch credentials
  or widen permissions as a workaround.
- Network errors: inspect current state and retry only operations whose outcome
  is known or safely repeatable. Use supported checkpoints/reports; do not
  implement an unbounded retry loop or restart destructive work blindly.
- Upload/copy: `stat` the destination and compare expected size and relevant
  metadata. Download: verify the local file exists and its expected size; use
  an independently known checksum when integrity verification is required.
  Do not assume an ETag is a full-file checksum.
- Report the exact target, executed versus preview-only status, success/skip/
  failure counts when available, and any incomplete verification. Redact
  credentials and signed query strings from diagnostic excerpts.

## Share management and receiving

```bash
ve-adrive-cli share create adrive://instance/space/docs/ --name Docs --expires-in 7d --permission download
ve-adrive-cli share get share-id --instance instance
ve-adrive-cli share get adrive://instance/share-id
ve-adrive-cli share list adrive://instance
ve-adrive-cli share list --instance instance --creator user-id --status active --limit 100
ve-adrive-cli share update share-id --instance instance --require-login false --expires-in 0
ve-adrive-cli share cancel share-id --instance instance --yes
ve-adrive-cli share inspect https://instance.example.test/share/share-id
ve-adrive-cli share files https://instance.example.test/share/share-id --path docs/ --limit 100
ve-adrive-cli share view https://instance.example.test/share/share-id --path docs/report.docx
ve-adrive-cli share view https://instance.example.test/share/share-id --path readme.txt --raw | less
ve-adrive-cli share download https://instance.example.test/share/share-id ./downloads --recursive --checkpoint ./share-checkpoint.json
```

1. Confirm the exact file or folder scope before create; use `--dry-run` to inspect
   selectors without acquiring or displaying secrets. Supply a safe page origin
   with `config set share_base_url https://pages.example.test` when derivation is
   inappropriate. `config show` reports its source.
   Standard public/BOE `idsapi` and `idsapp` hosts under `volces.com` map
   automatically, preserving Instance, scheme and port. Custom receiver APIs use explicit
   `--endpoint`. Anonymous receipt never loads account configuration; explicit
   `--auth-mode oauth` uses the selected recipient profile API or `--endpoint`.
2. Inspect returned `ShareID` and `ShareUrl`. Missing `ShareID` after create means
   `share_result_unknown`; reconcile with get/list before another create.
   Manage an existing share through `adrive://instance/share-id` or the compatible
   `share-id --instance instance` form; get/update/cancel all support both.
   Duplicate instances must agree, and `--by-name` resolves the Instance only.
   List shares in an Instance with `adrive://instance` (optional root slash), or
   `--instance instance`; both support the same filters and one-page pagination.
   Omit `--creator` only when OAuth supplies a bound `user_id`.
   Actual create/get results may include the service-returned `SharePwd` for
   authorized delivery. Deliver it separately from the safe page URL and exclude
   it from logs and plans.
3. For update, omit `--updated-at` to fetch the current concurrency guard, or use
   an explicitly known guard. A conflict requires a fresh read. Preserve explicit
   zero/false/empty settings. Password clearing is not confirmed by the PRD;
   do not promise that an empty input clears a service password.
4. Anonymous inspect does not require account setup. For files, choose exact paths
   or trailing-slash folders and consume one page at a time with `--limit` and
   `--marker` from `NextMarker`, ending at `IsTruncated=false`. Protected shares
   need the extraction code for every independent call/page; stdin is read to EOF
   and is best supplied by a pipe. Never echo or save it. Login-required receipt
   uses an explicitly selected recipient profile: run `auth login --instance <id>`
   and `auth status`, then pass `--auth-mode oauth --profile receiver` on
   inspect/files/view/download. OAuth does not bypass passwords. Never switch identity
   or retry anonymously automatically after rejection.
5. `share view` selects one exact file. Omit `--path` only for a single-file share.
   Default auto returns a temporary PDF descriptor for supported PDF/Office files,
   or raw metadata for other files. Keep `PreviewURL` private and respect expiry;
   no browser opens. Use `--raw` through a pipe/redirection, without `--output`
   or `--query`. Preview consumes `PreviewLimit` and never automatically retries.
   A 404 may mean the deployment does not provide preview.
6. Downloads resolve the user-selected local root with the OS (including `/tmp`,
   `/var` or configured aliases), then pin it and protect downloaded subpaths.
   Select recursion/overwrite
   explicitly and retain the checkpoint for file-level resume. Verify completed,
   failed and skipped items before reporting a transfer complete.

Resume the exact same share download with the same destination and policy:

```bash
ve-adrive-cli share download https://instance.example.test/share/share-id ./downloads --recursive --checkpoint ./share-checkpoint.json --resume
```

The original manifest is reused; newly added remote files are excluded. Changed
remote/local files fail validation. Completed files require strong remote ETag or
CRC64 plus the recorded local SHA-256. Pending files restart in fresh temporary
files. Inspect `Complete`, `PlanComplete`, `CheckpointAvailable`, `Summary`, and
`Files[].Status`; nonzero exit means interruption or failure. If a signed URL's
issuance outcome is unknown, check server state/quota before explicit resume.
An explicit download-disabled 403 instead retains `Forbidden.ShareDownloadDisabled`,
exits 5, and sets `IssuanceOutcomeUnknown=false`; changing local paths or blindly
resuming does not grant download permission. Checkpoint/lock collision checks and
resume identity include resolved local aliases.
Dry-run reports an unknown manifest and file count, with no network or file writes.
