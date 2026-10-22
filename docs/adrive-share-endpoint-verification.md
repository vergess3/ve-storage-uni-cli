# 分享页面 endpoint 改名验证

日期：2026-10-20。分支：`xsj-dev`。修改前基线：`4051b2f442cf60df43e36d9ec4093421bd6819e6`。

## 行为与修改理由

按用户要求，分享页面参数统一为 `--share-endpoint`，配置键为 `[profile.adrive].share_endpoint`，环境变量为 `ADRIVE_SHARE_ENDPOINT`。API 地址仍使用 `--endpoint`；页面覆盖值不决定接收 API 地址。

原 `--share-base-url` 保留为隐藏 CLI 别名，旧 TOML 键可读取/设置，旧环境变量保留回退。帮助、Describe、MCP schema、配置输出和保存的 TOML 使用新名称。优先级仍是 CLI > profile > 环境变量 > 自动推导；环境变量内部新名称优先。使用旧配置名的实际写入、dry-run plan 和 confirm_command 也规范化为新名称，不改变其他服务的配置键。

统一与独立入口的中英文帮助、注册表、Describe、导出 Skill/MCP、内置 Skill/reference、README、测试指南和 E2E 测试入口同步。历史验证报告和计划不改写。E2E 的页面覆盖变量采用 `ADRIVE_E2E_SHARE_ENDPOINT`，旧变量仍可回退。

## 生产域名示例

| 输入 | Instance | ShareID | 交付 URL |
| --- | --- | --- | --- |
| `http://idsapi.volces.com` | `vtagt3g9n` | `2T4U8z0lY1n` | `http://vtagt3g9n.idsapp.volces.com/share/2T4U8z0lY1n` |
| `https://idsapi.volces.com` | 同上 | 同上 | `https://vtagt3g9n.idsapp.volces.com/share/2T4U8z0lY1n` |
| `http://vtagt3g9n.idsapi.volces.com:8443` | 同上 | 同上 | `http://vtagt3g9n.idsapp.volces.com:8443/share/2T4U8z0lY1n` |
| 显式 `--share-endpoint http://pages.example.test` | 同上 | 同上 | `http://pages.example.test/share/2T4U8z0lY1n` |

标准域名保留协议和端口，Instance 只添加一次；显式页面覆盖直接使用，不自动添加 Instance。API endpoint 如果只填裸域名，既有规范化逻辑补 HTTPS。BOE 映射及 HTTP(S) 校验行为保持。

## 完整代码位置

- [CLI 参数及隐藏别名](../crates/adrive/src/cli/share.rs)
- [配置解析、序列化及校验](../crates/tos-core/src/infra/config.rs)
- [页面来源优先级](../crates/adrive/src/handler/share.rs)
- [配置输出规范化及发现入口](../crates/adrive/src/handler/meta.rs)
- [域名映射与生产示例单测](../crates/adrive/src/domain/share_url.rs)
- [两个真实入口的参数/环境/配置/MCP 回归](../tests/share_endpoint_regression.py)

修正处包含 `Review Fix #ShareEndpoint1`～`#ShareEndpoint4`，测试入口修正使用 `#ShareEndpointPty`，说明每处兼容或回显修正的理由。

## 验证结果

先写测试并验证红态：旧 CLI 不识别新参数，旧配置处理不识别新 key；独立审查发现的旧 key 输出问题同样先在真实二进制回归中复现，再修正为绿态。

| 范围／命令 | 最终结果 |
| --- | --- |
| `cargo test -p tos-core --locked --offline --quiet` | 207 个单元测试、57 个集成测试通过；1 个已有忽略项 |
| `cargo test -p ve-adrive-cli-core --locked --offline --quiet` | 461 个单元测试、17 个契约测试通过 |
| 根库及 functional_agent_contract_test / metadata_consistency_test / skill_export_test | 15 / 11 / 11 / 7 项通过 |
| 独立入口 direct invocation | 7 项通过 |
| 统一与独立入口重新构建 | 通过，debug 二进制已更新 |
| 全部 `tests/share_*regression.py` | 14 个脚本全部通过；每组覆盖两个入口，PTY 分别单独运行两次 |
| 格式／静态检查 | `cargo fmt --all --check`、`git diff --check`、修改 Python 文件语法检查通过 |

合计 793 项 Rust 测试通过。全部 HTTP 接口验证使用 localhost、合成凭据和隔离临时目录；未访问真实分享、下载云端内容或修改云端文档。未运行 Windows 实机测试；本次不改变平台文件 I/O 逻辑。

## 审查与修正

| 轮次 | Critical | Major | Minor | 处理 |
| --- | --- | --- | --- | --- |
| Coder 自审 | 0 | 0 | 0 | 核对名称覆盖、兼容读取、域名规则及脱敏 |
| 独立 Review 1 | 0 | 0 | 1 | 旧 config set 名称仍出现在回显/计划，添加规范化和红→绿回归 |
| 独立 Review 2 | 0 | 0 | 0 | 七类完整复审，确认上轮问题已修复且无新增回归 |

集成测试发现 PTY 夹具原来写死统一入口的 `ve-adrive` 前缀，传入独立二进制时无法启动。已复现并修正为按入口构造 argv，保留全部密码隐藏、SIGINT 和取消确认断言；最终两个入口各自通过。

无本次修改遗留的 Minor/Suggestion。

## 保留的测试场景

| 场景 | 预期 |
| --- | --- |
| create/get/list/update 的新参数 | 均能解析，帮助与 Describe/MCP 只展示新名称 |
| 旧 CLI 参数和旧 TOML | 正常读取；保存及输出使用新名称 |
| 新旧环境变量同时存在 | 新变量优先，profile 和显式参数仍优先于环境 |
| 旧配置 key 的 bare/qualified set 和 dry-run | 输出、计划、确认命令均使用新 key；dry-run 不改文件 |
| 生产 API、预带 Instance、端口、HTTP/HTTPS | 页面域名对应正确，协议/端口保留，无重复 Instance |
| 显式自定义页面 origin | 直接拼接 `/share/<id>`，不添加 Instance，不修改 API 路由 |
| 非法 origin 含敏感 userinfo | 校验退出 6，输出不泄露敏感值 |
| PTY 密码输入与中断 | 两个入口均隐藏密码，SIGINT 后恢复回显并正确退出 |
