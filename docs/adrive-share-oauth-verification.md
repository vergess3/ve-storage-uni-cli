# 分享接收 OAuth 验证记录

2026-10-14，分支 `xsj-dev`，开发基线 `f1835f8`。用户确认的策略：显式
`--auth-mode oauth` 复用所选 `--profile`，未指定时仍匿名。参考当前 V1 Go SDK
`bf6b098` 的公开分享接口与 ShareSession 鉴权边界。

## 已实现行为

- inspect/files/download 统一采用显式 OAuth 选择；profile/环境默认 OAuth 不会开启登录接收。
- 复用 auth login/status、凭据分组选择和现有刷新机制；不自动登录、换 profile 或匿名重试。
- OAuth 使用 profile API 或 `--endpoint` 获取概况及换取分享 token；标准页面实例在每次请求的凭据快照中验证。
- 文件 API 仅携带 x-share-token；实际内容下载不携带账户凭据，保持独立无重定向传输。
- files 用 `--limit`、`--marker` 返回一页，保持 NextMarker/IsTruncated；download 自动遍历分页。
- 提取码仍由服务端检查，每个命令/分页调用重新提供。stdin > URL pwd > 普通终端隐藏提示；`--password-stdin` 读取到 EOF，应使用管道。
- 新 checkpoint 记录实际 API；OAuth 记录 profile 与已接受请求快照中的可用 user_id。续 token 时身份上下文变更会停止。旧匿名 checkpoint 仅在 URL 推导 API 下迁移。
- 双语 root/leaf help、Describe、capabilities、Skill/list/export、MCP auth_mode enum/argv、README 和测试指南已同步。

## 自动化验证

| 验证 | 结果 |
| --- | --- |
| `cargo test -p ve-adrive-cli-core --locked --offline --quiet -- --test-threads=1` | 416 单元测试 + 8 分享契约测试通过 |
| `cargo test -p ve-storage-uni-cli --test functional_agent_contract_test --locked --offline --quiet` | 11 项通过 |
| `cargo test --manifest-path packaging/cargo/ve-adrive-cli/Cargo.toml --locked --offline --quiet` | 7 项通过 |
| 统一与独立二进制 build | 均成功 |
| `tests/share_oauth_regression.py` | 两入口通过：模式/profile、分页/提取码、下载/恢复、刷新/脱敏、并发换号、实例快照、API 变更、旧 checkpoint、缺少登录引导和零 I/O |
| `tests/share_download_regression.py` | 两入口通过：既有匿名下载聚合、中断、路径/选择器安全、明确 403、根别名恢复与未知签发 |
| 实际二进制文档检查 | 46 项通过，含两入口双语 root/leaf help、Describe、Skill/list/export，以及 capabilities |
| `cargo fmt`、`git diff --check` | 通过 |

测试先复现了旧实现忽略显式 OAuth、拒绝登录分享、泄漏被刷新替换的 token
请求 ID、MCP 未接收 auth_mode，以及审查发现的恢复/快照绑定问题，再修正并回归。
文档回归曾发现新 schema 描述缺少中文翻译及新增语义改变既有数组顺序；已补齐翻译，保留既有顺序并追加新语义。

## 实机验证与清理

已有分享仅执行 OAuth inspect/files，只读成功。

完整接收验证使用本次创建的随机临时文件，内容仅为无敏感信息的合成测试文本。
新分享显式要求登录、有提取码、允许下载，20 分钟过期。实测：

1. OAuth inspect 成功，RequireLogin/HasPassword 为 true。
2. 匿名 files 被服务端 `Forbidden.ShareRequireLogin` 拒绝。
3. 错误提取码被 `Forbidden.SharePwdIncorrect` 拒绝，无备用密码/身份重试。
4. OAuth 配合正确提取码列举成功。
5. OAuth 下载成功，Complete=true、Completed=1，下载字节与合成样本一致。
6. OAuth checkpoint 恢复成功。
7. 取消本次分享、删除本次文件均成功；既有文件/分享未改动。

自动审批拒绝了为已有私有 SECURITY.md 新建分享的方案，该操作未执行；上述无敏感内容的自建临时样本作为安全替代完成实测。凭据、提取码、分享 token 和签名地址未写入本记录。

## 独立 Reviewer 记录

| 轮次 | Critical | Major | Minor | 处理 |
| --- | --- | --- | --- | --- |
| 1 | 0 | 2 | 1 | 修复实际 API 未绑定、身份取自交换后重读的凭据；同步静态 root help |
| 2 | 0 | 1 | 1 | 每次请求验证同一凭据快照的标准实例绑定；说明缺少 user_id 的边界 |
| 3 | 0 | 0 | 0 | 完整七类审查通过，独立二进制回归通过 |

审查覆盖正确性、安全性、性能、可维护性、健壮性、可测试性、可观测性。
修正通过 Review Fix 注释关联原因，包括凭据脱敏、已接受主体快照、实例快照、API 绑定、登录引导和文档同步。

## 遗留边界与建议测试

无未修复 Critical/Major/Minor。环境变量或仅 access token 凭据没有 user_id 时，
只能绑定 API/profile/模式；无法判定跨恢复替换的未知主体 token 是否属于另一账户，
不会解析 token 推断身份。实际权限始终由服务端验证。

建议场景均已有隔离回归覆盖；正常接收/下载、登录权限和提取码另有实机验证：

1. 登录分享正常列举/下载/恢复，提取码正确。
2. 默认匿名接收登录分享被拒绝；错误提取码被拒绝。
3. 单文件、目录一页与下一页、download 自动分页。
4. 缺少登录、过期凭据刷新、401 一次刷新，保留登录引导。
5. 配置/环境默认 OAuth 与坏账户配置不影响匿名；Describe/dry-run 无凭据或网络 I/O。
6. 文件 API 没有 Authorization，内容下载没有账户或分享凭据。
7. 服务在请求 ID 中回显当前/被刷新 token，输出与 trace 脱敏。
8. token 交换响应前并发换号，checkpoint 仍记录已发送请求的主体；续 token 换号停止。
9. 概况响应后实例发生变化，token 交换之前拒绝错实例凭据。
10. 恢复时切换 API/profile/已知主体拒绝；旧匿名 checkpoint 在 URL 推导 API 下迁移。
11. 明确拒绝与未知签发结果保持区分，未知签发不自动重试。
