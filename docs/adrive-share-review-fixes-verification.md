# 分享能力 D01～D06 核对与修复记录

日期：2026-10-15。分支：`xsj-dev`。基线：`e1b7edb830cde400f366178de6d25fbcceff8275`。

依据：[问题报告](https://bytedance.larkoffice.com/docx/WyQCdoIM5oO85rxzMxKcdn5Anub)、[PRD](https://bytedance.larkoffice.com/docx/Cw4cd4yLEoiHYFx4pZ3cfKq4n3d)、现有 V1 SDK 类型和当前代码。初次修复阶段仅核对云端文档；随后用户确认 inspect 保持分享概况、文件信息通过 files 获取，已精准更新 PRD 的 inspect 要求至第 103 版并回读验证。其他待议 PRD 条目尚未写入云端。

## 本次行为

| 项目 | 结果与理由 |
| --- | --- |
| D01 域名映射 | 保持讨论，不猜测新映射。`adriveapp` 是报告对分享网页域名的描述，不是 CLI 参数或环境变量；报告没有给出完整页面域名，尚不能确认其正式支持范围及 API 对应规则。当前 CLI 自动识别 `idsapp↔idsapi` 和 BOE 对应关系，保留实例、协议和端口；其他安全 HTTP(S) 网页域名可作为自定义链接输入，但不会自动改写为 API 域名。报告中显式 API endpoint 能成功，支持继续核对路由；不能直接断言分享 API 不存在或某套新域名必然受支持。确认完整页面域名与对应 API origin 后，再决定是否补映射。自定义域名继续显式 `--endpoint`。 |
| D02 下载预演 | `download --dry-run` 复用有界分页与目录遍历，输出远端文件、本地映射、文件/目录数量、总字节数和冲突。只请求 info/token/list/get 元数据；不签发下载 URL、不取内容、不创建目录/临时文件、不写 checkpoint/锁。完成的计划有冲突时退出 9，并保留完整计划。默认表格限制清单展开，JSON/显式 query 保留详情。 |
| D03 本地变化 | 与 cp checkpoint 冲突分类对齐：已完成本地文件缺失、大小/摘要变化或远端身份变化返回 Conflict 9，不建议自动重试；不修改本地内容或保存失败校验后的 checkpoint。实际下载与恢复预演复用验证。 |
| D04 符号链接错误 | 保留下载子路径的 no-follow/reparse 检查，将主动拒绝的本地类型问题表达为本地路径冲突，给出目标目录修复建议。其他原生 I/O 错误保持原分类。用户指定的根目录仍按操作系统语义解析别名。 |
| D05 显式空密码 | stdin 去掉末尾一个 LF/CRLF 后为空，返回校验错误 6；覆盖 create/update 正常执行和 dry-run，以及接收 token 交换。显著空格保留，未传参数的 update 不修改密码。空取消确认仍按原有逻辑处理。 |
| D06 额度耗尽 | `Forbidden.ShareDownloadDisabled` 同时提示检查下载权限和下载次数额度，不仅凭该码断言原因。签发前的 token 失败和签发失败均保留 HTTP 状态、服务码、request ID；缺失 ID 时不回填旧环境值。CLI 不替服务端扣次或授权。 |

完整实现：`share_download_plan.rs`、`share_plan_local.rs`、`share_checkpoint.rs`、`share_download.rs`、`share_content.rs`、`share_local.rs`、`share_local_windows.rs`、`share_password.rs`、`share.rs`、`common.rs`、`domain/share_metadata_decode.rs`、`domain/token_manager.rs`。修正处包含 `Review Fix` 注释。

## 预演的明确边界

`data.selection` 包含 `Manifest`、`PlanComplete`、`FileCount`、`DirectoryCount`、`TotalBytes`、`SkippedCount`、`Conflicts`。缺少或负数文件大小时校验失败，不能把未知大小算成零；显式零表示空文件。

`DownloadsIssued=0`、`LocalWrites=false`。`MetadataResponses` 统计收到的元数据响应；未计量所有网络尝试，因此 `NetworkRequests=null`。`SnapshotGuaranteed=false`：检查后本地和远端都可能变化，执行必须重新检查。

恢复预演只读保存的任务清单，严格检查任务身份和已完成文件；不获取 checkpoint 锁，所以它不能证明任务没有被另一进程使用。执行阶段才执行正式加锁与保存。

显式 OAuth 预演使用现有可用 token，禁用自动刷新，避免改写凭据、创建认证锁。沿用现有 60 秒刷新窗口；过期、进入窗口或被服务端 401 拒绝时，先执行正常的 `share inspect <url> --auth-mode oauth` 允许刷新，再重试预演。匿名默认和选定 profile 规则保持。

Help、中文 help、Describe 的独立 `dry_run_output_schema`、导出 Skill、内置 Skill、README 与测试指南同步。MCP 默认只规划 argv；显式 `execute=true,dry_run=true` 执行元数据预演。下载工具实际可以写文件，所以 `readOnlyHint=false`；`destructiveHint=false` 保持原有非删除语义。新增真实调用还发现并修复了独立入口启动 MCP 子命令时重复插入 `ve-adrive` 的旧缺陷；执行参数按实际入口转换，保留同名 profile 等参数。真实下载的 `Complete/PlanComplete/Summary/Files/CheckpointAvailable` schema 不变。

## “需要产品/研发确认的契约差异”的建议

本表仅保留待处理或待确认事项；仅涉及 CLI 且已完成的副作用说明、显式空密码等修复不再列入。

| 项目 | 建议 | 修改归属 | 当前状态 |
| --- | --- | --- | --- |
| D01：adriveapp 域名 | 先确认报告中的完整分享页域名、是否属于支持范围，以及对应 API 地址；确认后再决定补 CLI 映射还是明确 PRD 支持范围。 | 待确认后决定 PRD/CLI 归属 | 待确认；当前不改域名映射 |
| CLI 参数和默认值 | 保留已确认的 `--permission view\|download`、`--expires-in 7d` 写法及创建默认 view；修正 PRD 中另一套命名的示例。若需要别名，另行列出兼容需求，不自动改名。 | 改 PRD；CLI 实现保持 | PRD 待同步 |
| V1/V2 与服务字段 | 按已确认维护的 V1 交付，保留 `Permission/ExpirationSec/ShareID` 等 V1 线协议，文档按 V1 对齐；不因新版示例迁移 V2。 | 改 PRD；CLI 实现保持 V1 | PRD 待同步 |
| share files 与 ls 的分页能力对齐 | 新增 `--max-keys`，默认 1000，限制当前层文件和目录合计返回条数；内部自动翻页直至达到上限或列举结束，保留 `--marker` 及后续游标。按用户要求移除 files 的 `--limit`，不保留别名。同步 PRD、help、Describe、Skill 和 MCP。 | 改 CLI 实现＋改 PRD | CLI 与帮助/发现入口已完成并验证；PRD 分页要求待同步 |
| 下载结果契约 | PRD 固化当前真实传输结果，Skipped 是 Completed 的子集；预演单独给 schema，不能让“预演完成”暗示“下载完成”；未知大小报错，不用零值冒充。 | 改 PRD 固化结果 | PRD 待同步 |
| 状态和两类分页 | 管理状态保留 V1 `revoked`，不把 IsCancelled 布尔字段直接改成另一个状态枚举；管理列表保留 Items/Offset/Limit/Total，接收列表保留 Files/Folders/NextMarker/IsTruncated。 | 改 PRD；CLI 实现保持 | PRD 待同步 |

分页对齐：`ls --max-keys` 默认 1000，限制本次返回总量，内部自动翻页；share files 按相同语义实现。该值统计文件与目录，指定 `--path` 时仍只列举选中目录当前层。达到上限时返回可继续使用的 marker；列举结束则无后续游标。ls 没有 `--limit`，因此 files 也移除该参数，不保留别名；管理 share list 的 `--limit` 不受影响。1000 是命令返回量默认值，不据此推断分享服务的单页硬上限。回归覆盖多页、文件/目录混合计数、空结果、精确单文件、续页无遗漏、异常游标、旧参数拒绝及全部帮助/发现入口。

归属基于已确认采用 V1 和命令形式。2026-10-15 复核报告第 14 版；inspect 的职责已按用户选择完成 PRD 修改（第 98→103 版），从待处理表移出。其余表内“改 PRD”仍是待同步建议。

当前仍需确认 D01 的真实域名对照与支持范围。inspect 不再列为待确认。share files 的新增代码、525 项 Rust 测试及 13 个双入口回归脚本结果见 [分页对齐验证记录](adrive-share-files-max-keys-verification.md)，下面的 505 项 Rust/12 个脚本属于此前 D02～D06 修复的历史验证。

## 验证结果

- ADrive Rust：441 个单元测试、13 个契约测试通过。
- 统一入口库：15 项通过；functional agent / metadata consistency / skill export 三组分别 11 / 11 / 7 项通过。
- 独立入口：7 个直接调用测试通过。
- 两入口 localhost 分享回归：download aggregate、dry-run、files display、download display、download progress、OAuth、password、PTY、Range、URL、view 通过。新增 MCP discovery 回归核对真实工具注解、默认 argv 规划和执行只读预演；该脚本亦在两个入口通过。合计 505 个 Rust 测试、12 个两入口 Python 回归脚本通过。
- `cargo fmt --all --check`、`git diff --check`、修改 Python 文件语法检查通过。

所有分享接口测试仅使用隔离本地夹具；本轮没有重新访问真实分享或修改云端分享设置。Windows 后端经过源码审查及独立目标类型检查，未在 Windows 上运行。完整 Windows crate 构建受到本机 `ring` 所需 Windows SDK `assert.h` 缺失限制，不能视为已通过完整平台构建。

## 审查记录

| 范围 | 轮次 | 发现与处理 |
| --- | --- | --- |
| D03～D06 | 自审 1/2 | 修复本地 I/O 冲突标记识别；第二轮无 Critical/Major。 |
| D03～D06 | 独立 1 | 发现 token 失败 HTTP 状态丢失 Major；增加正常 create/update 密码覆盖、修正过时的否定断言。均修复。 |
| D03～D06 | 独立 2 | 七类重新审查与针对性验证通过；无剩余 Critical/Major。 |
| D02 与帮助 | 独立 1 | 发现未知大小被零值默认 Major，修复解码与校验；本地路径建议 Minor 已改。 |
| D02 与帮助 | 独立 2 | 七类通过；发现既有 MCP readOnlyHint 与本地写入不符，已作准确注解并新增真实 RPC 覆盖。 |
| D02 与帮助 | 独立 3 | 七类通过；提取工具条目构造函数，修正新增代码造成的函数长度 Minor。 |
| MCP 实际执行 | 独立 4 | 两入口真实 RPC 发现并修复独立入口重复命令前缀 Major；新增参数适配单测和完整 RPC 夹具通过。七类复审无剩余 Critical/Major。 |

最终无已知 Critical/Major。遗留事项是上述服务域名确认、其余 PRD 同步及平台实机验证；分页实现已在后续工作中完成。未自动扩展其他命令的 MCP 注解规则。

另记一项既有 Minor：统一入口 MCP 的 `execute=true,describe=true` 若同时选 `profile=ve-adrive`，根层命令定位可能把 profile 值误当工具名，返回根概况而非下载叶命令 Describe；独立入口的同一查询正确。本次参数适配保留了该 profile 值，但没有改写全局命令定位逻辑。此边界涉及所有根命令的 Describe 分派，留待单独修复及覆盖，不将参数单测表述为端到端发现已覆盖该特殊名称。普通 profile 的两入口真实 MCP 预演与 Describe 已通过。

## 至少应保留的回归用例

| 场景 | 预期 |
| --- | --- |
| 多分页目录，含子目录和空目录 | 完整计数/总字节/映射；没有 URL 或内容请求，没有目标/checkpoint/锁写入。 |
| 本地已有文件，分别不覆盖/显式覆盖 | 不覆盖返回含计划的 Conflict 9；覆盖只显示 overwrite，预演仍不改变内容。 |
| 下载后恢复预演 | 已完成文件验证后 skip，checkpoint 字节不变，不创建锁。 |
| 已完成文件增大、缩小、同长度内容变化或缺失 | Conflict 9；真实恢复及预演都不签发新 URL。 |
| 下载子路径为符号链接/特殊条目 | 明确的本地冲突，外部哨兵不变；根目录的正常 OS 别名仍可用。 |
| checkpoint 与目标重叠、路径遍历、坏清单 | 校验失败，不在目标外创建任何内容。 |
| 密码 EOF/LF/CRLF 空输入及正常空格 | 空输入退出 6，正常空格原样提交；省略 update 密码参数时不提交 SharePwd。 |
| token 阶段和签发阶段拒绝下载 | 保留码/HTTP 状态/request ID，提示权限或额度；无 ID 不带出旧 ID。 |
| 缺少文件大小与显式空文件 | 未知大小失败；Size=0 可以规划，不能混淆。 |
| OAuth 过期及正常 token | 正常只读规划成功；需刷新时先失败，无认证锁或凭据改写。 |
| MCP tools/list 和 tools/call | 下载 readOnlyHint=false；默认 argv 无 I/O，显式 dry_run 返回与 CLI 相同计划。 |
