# 分享下载结果展示与权限拒绝验证

日期：2026-10-15；分支：xsj-dev。

本记录对应448d814的历史验证。默认table后续已对齐cp汇总并加入进度，当前行为及验证见 [分享下载进度记录](/Users/bytedance/workspace/tos-uni-cli/docs/adrive-share-download-progress-verification.md)。

## 结论与修改理由

用户提供的分享 `2T4U8z0lY1n` 当前为 `Permission=view`，RequireLogin/HasPassword/IsExpired/IsCancelled 均为 false。只读列举 `superpowers/plans/` 返回29个文件，包括用户指定的文件；limit=100和1000返回一致。

随后在独立临时目录实际下载该文件，服务端返回403、`Forbidden.ShareDownloadDisabled`、exit_code=5；Summary为Total=1、Completed=0、Failed=1、Pending=0，IssuanceOutcomeUnknown=false，写入文件数0。request_id为 `2ec4a6a5d6f54b2470ffced29206a670-ac1ca477-1xHBjd-SFD-ccs-ids`。没有写入用户Desktop或变更分享权限。

空目录来自下载前的目录预创建；文件下载地址申请随后被服务端拒绝。此流程保留。修改前，多文件失败把完整Files数组放入一个表格单元格，41文件夹具的最长行达3726字符；空结果成功还会隐藏汇总。修改后，默认table分为汇总和逐文件状态两个表格，空结果也保留汇总。该夹具最长行小于240字符；完整路径仍按原值显示。

已知 `Forbidden.ShareDownloadDisabled` 的错误提示改为联系分享者开启 `--permission download`，覆盖token交换和下载地址签发两个拒绝阶段。只按精确业务码与PermissionDenied分类匹配；保留原有错误码、可用状态码、请求ID、退出码与签发结果语义。显式query以及其他输出格式沿用原结构。

不传 `--path` 时选择全部分享根，单文件自动选中；分享目录需要 `--recursive`。`--path` 用于选择其中一个文件或子目录。相关说明已同步README、内置Skill、双语help、Describe、导出Skill和MCP共用元数据。

## 已执行验证

| 检查 | 结果 |
| --- | --- |
| cargo test -p ve-adrive-cli-core | 422单测 + 11分享契约通过 |
| cargo test --lib | 根入口15项通过 |
| cargo test --test functional_agent_contract_test | 11项通过 |
| 独立入口direct_invocation | 7项通过 |
| 两个入口cargo build | 通过，debug二进制已更新 |
| tests/share_download_display_regression.py | 两入口通过：空结果、无path单文件、41文件拒绝、JSON、query |
| tests/share_download_regression.py | 两入口通过：dry-run、失败汇总、SIGINT、恢复、选择器安全、403/table/CSV、系统别名、未知签发 |
| tests/share_files_display_regression.py | 两入口通过：ls列对齐、目录、空结果、分页、精确文件、JSON/query |
| tests/share_oauth_regression.py | 两入口通过：显式身份、分页/密码、下载/恢复、刷新/脱敏、无I/O拒绝 |
| tests/share_url_regression.py | 两入口通过：HTTP/HTTPS、可选尾斜杠、前缀/分页、恢复、脱敏、非法路由 |
| 编译后help/Describe/Skill导出/capabilities | 两入口、中英两种语言，共14项通过；path仍为可选参数 |
| cargo fmt --all --check / git diff --check | 通过 |

Rust验证合计466项通过。MCP通过同一分享注册/元数据契约覆盖，本次未启动外部MCP客户端。

## Review记录

按正确性、安全性、性能、可维护性、健壮性、可测试性、可观测性完成两轮独立完整审查。

| 轮次 | Critical | Major | Minor | Suggestion | 结果 |
| --- | --- | --- | --- | --- | --- |
| 第1轮发现 | 0 | 1 | 1 | 1 | 未通过 |
| 第2轮剩余 | 0 | 0 | 0 | 0 | 通过 |

第1轮Major是早期token拒绝经过既有错误映射后缺少status_code，导致新提示未生效；既有回归真实失败。已改用精确错误码和权限分类，新增早期拒绝及其他错误不改写的单测，既有回归重跑通过。Minor为夹具函数6个参数，已合并为5个；Suggestion为额外复制完整Files数组，已避免克隆包含原data的完整envelope。修复原因标注为 `Review Fix #ShareDownloadDisplay1` 至 `#ShareDownloadDisplay4`。

独立Reviewer另外实跑两份下载夹具、renderer单测、早期拒绝单测、完整11项分享契约和diff检查，均通过。没有未修复的Minor/Suggestion。

## 可复用验收用例

| 用例 | 预期 |
| --- | --- |
| 单文件分享不传path | 自动下载该文件，Completed=1 |
| 空目录分享递归下载 | 保留目录，Complete=true、Total=0可见 |
| 41文件目录在签发URL时返回已知403 | exit=5、Failed=1、Pending=40、无文件发布；汇总和逐文件表格可读 |
| token交换阶段返回禁止下载 | 无清单时仍显示业务错误与分享者权限提示，table/CSV都不隐藏错误 |
| JSON或显式query | 保持原结构或用户投影，JSON保留可用403和request_id |
| 其他权限码或非PermissionDenied类别 | 不错误替换成禁止下载提示 |
| URL签发结果不确定及SIGINT恢复 | 不自动重放未知签发；按checkpoint恢复已完成文件 |
| 路径穿越、别名及凭据隔离 | 非法子路径拒绝；目标根别名可恢复；内容请求和日志不泄露账户/分享凭据 |

完整代码见仓库中的 `crates/adrive/src/handler/common.rs`、`crates/adrive/src/handler/share_download.rs` 和对应单测/夹具。
