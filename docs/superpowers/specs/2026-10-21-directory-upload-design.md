# Preserve local directories in recursive uploads

Date: 2026-10-21. User-approved scope: recursive cp and local-to-remote sync on tos, ve-tos and ve-adrive, default directory preservation, an optional explicit-directory opt-out, service-specific handling, documentation and discovery coverage. Existing xsj-dev checkout is clean at dafcd8c and is reused as requested.

## Contract

- Local-directory uploads preserve selected directories, including empty directories, by default. Existing include-parent semantics remain: no parent basename unless explicitly requested. An empty source root creates its effective destination prefix when representable; bucket/space roots never become empty-key objects.
- Add boolean `--disable-dir-object`, default false to cp and sync on all three surfaces. Cp requires recursive. The switch is valid for local-directory-to-remote uploads and disables explicit directory tasks, while file uploads can still create their required parents server-side. Other directions reject the switch instead of silently ignoring it.
- Include/exclude retain each command's existing file-path convention: cp and ADrive include the retained parent segment, while TOS sync matches source-relative paths before adding its target parent. Directories follow that convention without a terminal separator. Matching files imply required ancestors, including FNS markers. The effective destination root is an unfiltered structural container, including for an empty source with active filters; descendants still obey filters. Directory rejection does not prune unrelated file traversal. Symlink descendants are not followed; this matches existing manifest traversal and prevents cycles in streaming upload.
- FNS/ByteTOS directory representation is a zero-byte object with a terminal slash. HNS uses directory PutObject with empty content and no file-only storage class; existing HNS directories are idempotent and files cannot be overwritten by directory creation. Do not infer success from an arbitrary 403/409.
- ADrive uses the existing CreateFolder API. Create parents before files, detect existing folders via typed directory listings, preserve file-versus-folder conflicts, and revalidate directory presence on resume. Do not use file-only HEAD/mtime checks as proof of folder identity.
- Manifest and no-manifest execution both process and report directory tasks. Directory byte size is zero. Creation failures are failures and suppress sync delete-extra. File transfer concurrency, checksums and multipart behavior remain unchanged.
- Sync creates missing directories and preserves existing ones. Its desired set includes matching source directories and ancestors of selected files. Disabling explicit creation does not make source-present folders extraneous for deletion. Actual extras remain subject to existing filters, confirmation and child-first deletion rules.
- Do not globally change file-only scanners used by unrelated mv/local-copy/delete logic. Internal mv-to-cp conversion retains its previous directory policy.
- Help in both languages, describe, capabilities, MCP boolean typing/dispatch, exported and bundled Skill, README and relevant test guides state the default and switch, including implicit-parent behavior and dry-run semantics. Dry-run describes the directory policy without claiming actual uploads or creating local/cloud artifacts.

## Reference evidence

Local reference `/Users/bytedance/workspace/tosutil/cmd/subcmd/transfer.go` declares ddo=false; transfer_upload.go uploads directories with an empty-body PUT. It skips directory upload on its non-public surface; this task intentionally implements the user's requested behavior on tos as well. No evidence found of a separate HNS branch in its directory upload path. Our implementation follows service contracts, rather than copying a private tool limitation.

Official HNS PutObject contract: https://docs.volcengine.com/docs/TorchObjectStorage/APIListHierarchicalBucket?lang=zh . Missing parents are created automatically; directory keys end in slash, use empty bodies, have STANDARD storage, and existing directory creation returns success without overwriting it.

## Validation and review

Red/green tests precede production changes. Cover mixed and entirely empty trees, zero-byte files, include-parent/root boundaries, the canonical option and rejected aliases, repeated uploads, FNS/HNS/ADrive responses, filters, symlinks, both manifest modes, checkpoint/resume, directory failure, sync delete protection, help/describe/MCP and direct/unified binaries. All service tests use isolated localhost fixtures with fake credentials; real-service testing is reported separately if performed. Perform independent specification review then all seven user-mandated review categories; fix all Critical/Major and record review reasons. Keep the directory-upload work in its own commit(s), rebase and push xsj-dev after verification.
