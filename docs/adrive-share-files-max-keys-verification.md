# share files 分页对齐验证记录

日期：2026-10-15。分支：`xsj-dev`。代码修改前基线：`1aba427`。

## 已交付行为

`ls` 没有 `--limit`；按用户要求，`share files` 移除该参数，不保留别名。新增 `--max-keys`，默认 1000，限制当前目录层级的文件和目录合计条目数，内部自动获取服务分页。达到上限时返回 `NextMarker`，下次通过 `--marker` 继续；结束时 `IsTruncated=false`。管理侧 `share list --limit/--offset` 保持原有行为。

列表先按 ls 规则处理目录标记、自身目录及重复项，再计数。table、CSV、JSON 和显式 query 使用同一份聚合且规范化的数据；`Total` 与 `pagination.total_returned` 统计同一批逻辑条目。`--path 文件` 仍精确查询一个文件，`--path 目录/` 只列举该层。命令的返回量上限与服务内部单页大小分开处理；每次请求不超过剩余返回量，拒绝无法安全续取的超量响应，不裁剪后推进游标。

统一入口和独立入口的英文/中文 help、Describe、capabilities、MCP schema、导出及内置 Skill、README、测试指南均已同步。下载帮助使用独立的选择/密码说明，不出现 files 专属分页参数。files 的离线 `--dry-run` 计划显示并验证 `MaxKeys`、`Marker`。

主要完整代码：

- [CLI 参数与帮助](../crates/adrive/src/cli/share.rs)
- [有界分页及回归单测](../crates/adrive/src/handler/share_listing.rs)
- [接收流程与输出](../crates/adrive/src/handler/share_receive.rs)
- [离线参数校验与计划](../crates/adrive/src/handler/share.rs)
- [发现 schema 与语义](../crates/adrive/src/share_metadata.rs)
- [真实二进制分页夹具](../tests/share_files_pagination_regression.py)

## inspect 的 PRD 修改

用户确认：inspect 保持分享概况职责；文件信息通过 files 获取。没有为 inspect 新增文件列举或文件详情请求。

已在 [原始 PRD](https://bytedance.larkoffice.com/docx/Cw4cd4yLEoiHYFx4pZ3cfKq4n3d) 精准更新五处内容，修订号从 98 递进至 103，并重新获取文档确认全部落地：

1. 接收命令表的 inspect 职责：返回实际公开分享概况；不承诺真实 FileName/FileType，不从 ShareName 猜测。
2. 对应 V1 API：`GetShareLinkByAnonymous`、`/v1/share_link/get_by_anonymous`，不额外查询文件详情。
3. 身份与副作用：默认匿名，显式 OAuth 使用选定 profile；不读取提取码或交换分享 token，概况查询成功不代表拥有内容权限。
4. RC01 验收：概况缺少 FileName/FileType 不视为失败；文件详情由 files 在内容授权后获取。
5. 输出约定：再次明确概况/文件详情边界，不返回管理端 Password。

本次只写入已确认的 inspect 内容；分页等其他 PRD 待同步项仍保留在 [契约差异表](adrive-share-review-fixes-verification.md)。没有修改云端分享设置或下载真实分享内容。

## 验证结果

先验证失败：旧实现不认识 `--max-keys`；元数据缺少该字段；离线计划未输出 MaxKeys。实现后分别通过相同回归，覆盖参数、运行时和发现入口。

| 命令/范围 | 结果 |
| --- | --- |
| `cargo test -p ve-adrive-cli-core --locked --offline --quiet` | 459 个单元测试、15 个契约测试通过 |
| `cargo test --lib --test functional_agent_contract_test --test metadata_consistency_test --test skill_export_test --locked --offline --quiet` | 15 / 11 / 11 / 7 项通过 |
| `cargo test --manifest-path packaging/cargo/ve-adrive-cli/Cargo.toml --locked --offline --quiet` | 7 项直接调用测试通过 |
| 两入口重新构建 | 统一 `ve-storage-uni-cli` 与独立 `ve-adrive-cli` 构建通过 |
| 所有 `tests/share_*regression.py` | 13 个脚本均在两个真实二进制入口通过 |
| 格式与静态检查 | `cargo fmt --all --check`、`git diff --check` 和修改 Python 文件的语法检查通过 |

合计 525 项 Rust 测试。13 个脚本覆盖 discovery、download display/progress/aggregate、dry-run、files display/pagination、OAuth、password、PTY、Range、URL 和 view；使用 localhost 服务和合成凭据。

## 审查记录

| 轮次 | 发现、修复与回归 |
| --- | --- |
| 实现自审 | 检查完整页续取、逻辑计数、每页 token 续期、匿名/OAuth 边界及所有发现入口；通过。 |
| 独立规格审查 | 0 Critical、0 Major；发现离线计划未同步新参数的 Minor，添加 Review Fix 注释并完成红→绿回归。 |
| 独立质量复审 | 从正确性、安全性、性能、可维护性、健壮性、可测试性和可观测性七类重审；0 Critical、0 Major、0 剩余可操作 Minor。 |
| 最终集成检查 | 上述 525 项测试及全部 13 个双入口脚本通过；无新增回归。 |

本次修改无未修复审查问题。D01 域名对照仍待确认，旧记录中的全局 Describe/profile 同名 Minor 仍留待单独处理；它们不属于此次分页修改。未进行 Windows 实机运行，本次不改变平台本地 I/O 行为。

## 保留的回归用例

| 场景 | 预期 |
| --- | --- |
| 混合文件/目录，服务每页少于请求量 | 自动获取后续页，合计达到 max-keys 或真实结束 |
| 1003 条混合数据，省略 max-keys | 首次恰好 1000 条，续取剩余 3 条，无交集或遗漏 |
| 指定较小上限后续取 | 请求大小不超过剩余数量，返回游标不越过未输出条目 |
| 自身目录、重复目录/文件标记 | 规范化去重后计数，各格式结果一致 |
| 精确单文件和空目录 | 单文件一条、空目录零条，表头与分页字段稳定 |
| 旧 limit、零/负/溢出上限、控制字符游标 | 联网前退出 6；MCP 同样拒绝旧参数 |
| 重复/循环游标、截断状态矛盾 | 明确校验失败，有界退出，不挂起 |
| 服务返回超过允许条数 | 失败而非裁剪并返回可能跳过条目的游标 |
| 分页中 token 过期或接收者身份变化 | 正常续 token；身份变化拒绝继续，敏感信息不泄露 |
| help/Describe/Skill/MCP 与离线计划 | 默认 1000 和 marker 一致，files 无 limit，管理 list 保留 limit |
