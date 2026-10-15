# 分享下载进度及cp风格汇总验证

日期：2026-10-15；分支：xsj-dev；基线：448d814。

## 完整实现与行为

进度适配见 [share_download_progress.rs](/Users/bytedance/workspace/tos-uni-cli/crates/adrive/src/handler/share_download_progress.rs)，下载编排及恢复计数见 [share_download.rs](/Users/bytedance/workspace/tos-uni-cli/crates/adrive/src/handler/share_download.rs)，默认table映射见 [common.rs](/Users/bytedance/workspace/tos-uni-cli/crates/adrive/src/handler/common.rs:1621)。

- 复用cp的扫描和批处理进度样式。TTY默认在stderr显示扫描和按文件数计量的传输进度；--progress强制开启，非TTY使用普通文本行；--no-progress及--quiet关闭。两个progress参数互斥。
- 密码交互完成后才启动扫描，进度标签不含分享URL、提取码、分享token或签名下载地址。结果输出前以及取消/Drop时清理进度；关闭强制进度的stderr管道不应中断文件传输。
- 默认table与cp类似展示total/succeeded/failed/skipped，额外保留fail-fast下载的pending及checkpoint/未知签发状态。默认不展开Files清单；JSON和显式query保留文件明细。JSON新增Summary.Skipped，表示恢复复用的已验证完成文件数，包含在Completed中；table的succeeded为Completed-Skipped。
- 恢复校验及预检查失败时，未经验证的旧完成记录不计成功。单独维护内存验证状态，不改动供preflight判断覆盖策略及持久化使用的原journal；成功验证前不写原checkpoint。验证失败的当前项显示failed，其余未验证项pending。
- README、内置Skill、中英help、Describe、导出Skill、capabilities和MCP共用元数据同步了参数、计数及进度规则。

当前CLI没有分享正文预览命令。inspect查分享信息、files查文件元数据，view分享需在浏览器预览。未增加分享预览接口或改变服务端下载权限。

进程夹具实录普通匿名单文件下载的五次请求为：get_by_anonymous → get_access_token → share_file/list(list_root=true) → get_download_url → content。显式--path将第三步替换为share_file/get，不减少请求次数。第三步提供大小、ETag等校验/本地映射所需元数据。分页、刷新和内容重试可能增加请求。

## 已执行验证

| 检查 | 结果 |
| --- | --- |
| cargo test -p ve-adrive-cli-core | 422单测及12分享契约通过 |
| cargo test --lib | 根入口15项通过 |
| cargo test --test functional_agent_contract_test | 11项通过，包含真实终端密码/SIGINT行为 |
| 独立入口direct_invocation | 7项通过 |
| 两入口cargo build | 通过，debug二进制更新 |
| share_download_progress_regression.py | 两入口通过；真实PTY/管道、递归成功、恢复/损坏/符号链接、403、中断、五次请求 |
| share_download_regression.py | 两入口通过；dry-run、失败汇总、未知签发、SIGINT、选择器安全、403/table/CSV、根别名恢复 |
| share_download_display_regression.py | 两入口通过；空结果/单文件/41文件403汇总、JSON/query |
| share_files_display_regression.py | 两入口通过；ls对齐、空列表、目录、分页及查询兼容 |
| share_oauth_regression.py | 两入口通过；身份、分页/密码、下载/恢复、刷新/脱敏及无I/O拒绝 |
| share_url_regression.py | 两入口通过；HTTP/HTTPS、尾斜杠、前缀/分页、恢复、脱敏、非法路由 |
| 编译后双语help/Describe/Skill导出/capabilities | 两入口共14项通过，含布尔progress参数和Skipped schema |
| fmt / diff及新增Python函数约束检查 | 通过 |

Rust合计467项通过。CLI/MCP的共享元数据契约已测试；没有启动外部MCP客户端。本轮全部下载使用本机合成服务和临时目录，未修改云端分享权限或用户Desktop。

## 审查与回归记录

新PTY夹具修复前真实失败：missing or unexpected stderr progress。补齐后两入口通过。自审及独立首轮Review另复现了恢复校验失败的计数问题：旧Completed=1被映射成succeeded=1。新增本地/远程变化断言先失败，再加入内存验证状态后通过；磁盘checkpoint字节不变。

完整独立Review覆盖正确性、安全性、性能、可维护性、健壮性、可测试性和可观测性。首轮Critical=0、Major=1、Minor=0、Suggestion=0；Major为上述恢复误报，已修复。第二轮Critical/Major/Minor/Suggestion均为0，审查通过，无遗留问题。

Reviewer独立运行12项分享契约、3项恢复/取消定向单测及首轮双入口PTY/展示夹具；修后二进制又分别验证本地损坏、预检symlink、部分验证后失败、验证中取消、关闭stderr、--quiet --progress六个场景，两入口均通过，相关失败场景的磁盘checkpoint字节不变。最新版扩充整套PTY夹具由主agent执行通过，Reviewer没有重复该整套运行。

修改原因由Review Fix #ShareProgress1至#ShareProgress5关联：匿名配置隔离、强制非TTY进度、cp风格默认汇总、进度管道错误处理、恢复验证后的结果计数。

## 可复用验收用例

| 用例 | 预期 |
| --- | --- |
| TTY中递归下载两个文件 | 可见扫描及0/2、1/2等传输计数；两文件内容正确，succeeded=2 |
| --no-progress及--quiet | 无进度字符；结果和退出码仍遵循原输出规则 |
| stdout为JSON、stderr为TTY | JSON可独立解析且不含ANSI，进度仅在stderr |
| 非TTY默认及显式--progress | 默认不显示进度；强制模式输出普通行 |
| 空目录或单文件不传path | 空结果保留0汇总；单文件自动选择并成功下载 |
| 正常checkpoint恢复 | skipped=1，succeeded=0，不重取已完成文件下载地址 |
| 本地/远程文件变化、预检符号链接 | 不误报成功；未验证项pending或当前项failed，不签发新地址，原checkpoint字节不变 |
| view分享明确403 | exit=5，错误码/请求ID保留，failed=1、其余pending；不发布文件 |
| Ctrl-C及未知URL签发 | 清理终端/临时文件，保留可恢复状态，不自动重放未知签发 |
| JSON/query及凭据隔离 | 文件明细可查；进度不泄露URL提取码/账户和分享凭据 |
