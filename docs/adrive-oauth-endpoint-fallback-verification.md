# OAuth endpoint 回退与同源支持验证

日期：2026-10-21。分支：`xsj-dev`。修改基线：`26699f0`。

## 最终行为

按用户确认，本次移除 OAuth Auth 与 Resource 必须使用不同 origin 的约束。未配置认证地址时，登录直接使用有效的业务 endpoint；显式 `http://`、`https://` 保留原协议，裸域名仍按既有规则使用 HTTPS。

认证地址优先级：`auth login --auth-endpoint` > 选定 profile 的 `auth_endpoint` > `ADRIVE_AUTH_ENDPOINT` > 有效业务 endpoint。有效业务 endpoint 沿用 `--endpoint` > 选定 profile 的 `endpoint` > `ADRIVE_ENDPOINT`；空白配置视为未设置。全部缺失时仍报 `oauth_auth_endpoint_required`。

登录保存最终选定的认证地址。后续刷新仍使用凭据内保存的认证地址；仅修改业务 endpoint 不会将已有 refresh token 转发到新地址。保留 URL 输入验证、禁止自动重定向和凭据输出脱敏。

## 完整代码与修改理由

| 文件 | 修改理由 |
| --- | --- |
| [登录处理](../crates/adrive/src/handler/auth.rs) | 认证地址回退到有效业务 endpoint，Doctor 可用性判断同步；移除登录同源拦截，允许 HTTP 验证页面。 |
| [OAuth 客户端](../crates/adrive/src/domain/oauth.rs) | 接受显式 HTTP/HTTPS 并保留协议，继续拒绝无 host、userinfo、query、fragment 和其他协议。 |
| [业务客户端](../crates/adrive/src/domain/client.rs) | 使用已登录凭据构建业务客户端时允许认证与业务同源。 |
| [Token 管理](../crates/adrive/src/domain/token_manager.rs) | 删除不再适用的同源拒绝函数；已有 refresh 的认证地址绑定规则保持不变。 |
| [命令帮助](../crates/adrive/src/cli/auth.rs)、[注册信息](../crates/adrive/src/registry.rs)、[ADrive 翻译](../crates/adrive/src/handler/meta.rs)、[统一入口翻译](../src/lib.rs) | 帮助、describe 等发现信息及中文文本说明 endpoint 回退。 |
| [README](../README.md)、[导出 Skill](../skills/ve-adrive-cli/SKILL.md) | 更新地址配置、优先级、协议与刷新行为。 |
| [OAuth 契约](superpowers/specs/2026-07-31-ve-adrive-cli-oauth-contract-design.md)、[Endpoint 契约](superpowers/specs/2026-08-05-explicit-service-endpoint-design.md) | 以 2026-10-21 用户确认覆盖旧的同源拒绝和认证地址必须单独配置约定。 |
| [实际入口回归](../tests/oauth_endpoint_regression.py) | 用临时配置、假凭据和 localhost 服务验证两个发布入口，不修改用户配置。 |

行为修正的原因标记为 `Review Fix #OAuthEndpoint1`～`#OAuthEndpoint4`；审查后补强 Doctor 回归的原因标记为 `#OAuthEndpoint5`。

## 红绿验证

修改生产代码前，以下测试分别在旧实现失败：

1. 同源业务客户端构建：旧实现返回 `different origins`。
2. 显式公网 HTTP 认证地址：旧实现要求 HTTPS。
3. 实际入口仅配置业务 endpoint 登录：旧实现返回 `oauth_auth_endpoint_required`。
4. HTTP 验证页面 URL：旧实现拒绝非 loopback HTTP。

修改后上述场景全部通过。

## 执行结果

两个入口均重新构建成功：

```bash
cargo build --bin ve-storage-uni-cli --locked --offline --quiet
cargo build --manifest-path packaging/cargo/ve-adrive-cli/Cargo.toml --locked --offline --quiet
```

Rust 共 **641 个测试通过**，不重复计算后续重跑的子集：

| 命令/测试组 | 通过数 |
| --- | ---: |
| `cargo test -p ve-adrive-cli-core --locked --offline --quiet` | 462 单元 + 17 契约 |
| 根包 `--lib` | 15 |
| `config_test` | 111 |
| `functional_agent_contract_test` | 11 |
| `metadata_consistency_test` | 11 |
| `skill_export_test` | 7 |
| 独立入口 `packaging/cargo/ve-adrive-cli/Cargo.toml` | 7 |

根包执行参数为 `--locked --offline --quiet -- --test-threads=1`；独立入口执行参数为 `--locked --offline --quiet`。

新增 OAuth endpoint 回归和既有 14 组 `tests/share_*regression.py`，共 **15 组实际入口回归**，均在统一入口和独立入口通过。PTY 测试每个入口分别执行。新增脚本在补强 Doctor 场景后再次完整通过：

```bash
python3 tests/oauth_endpoint_regression.py \
  target/debug/ve-storage-uni-cli \
  packaging/cargo/ve-adrive-cli/target/debug/ve-adrive-cli
```

`cargo fmt --all --check`、`git diff --check` 和新增 Python 脚本语法检查通过。

## 审查总结

以独立 Reviewer 执行正确性、安全性、性能、可维护性、健壮性、可测试性、可观测性七类完整审查。

| 轮次 | Critical | Major | Minor | Suggestion | 修复与回归 |
| --- | ---: | ---: | ---: | ---: | --- |
| 第一轮 | 0 | 0 | 1 | 0 | Doctor 测试存在已登录 token，可能掩盖 fallback 判断失效；改为无凭据并明确断言详情、状态和修复命令，重跑通过。 |
| 第二轮 | 0 | 0 | 0 | 0 | 重新检查完整 diff；首轮问题修复，无新问题。 |

审查通过，无遗留 Minor/Suggestion。

## 覆盖场景与后续验证建议

| 场景 | 已执行断言 |
| --- | --- |
| 正常：profile 仅配置 endpoint | 登录成功、保存该认证地址，普通 ls 和显式 OAuth 分享查询成功。 |
| 正常：多个认证地址来源 | CLI > profile > 认证环境变量 > 有效业务 endpoint。 |
| 边界：显式认证来源为空白 | 继续回退业务 endpoint。 |
| 边界：没有任何 endpoint | 明确返回 `oauth_auth_endpoint_required`。 |
| 协议：公网形式 HTTP 与 HTTPS | HTTP 不升级协议；裸域名默认 HTTPS；非 HTTP(S) 和非法 URL 被拒绝。 |
| 状态：Doctor 没有已登录凭据 | 识别业务 endpoint 可用于登录，并提示执行 `ve-adrive auth login`。 |
| 只读：login dry-run | 不发请求、不修改凭据，输出最终有效认证地址。 |
| 安全：修改业务地址后刷新 | refresh 仍请求保存的认证服务，资源请求使用新的业务 endpoint。 |
| 安全：凭据脱敏 | access/refresh/device token 均不出现在 stdout/stderr。 |
| 发现能力 | 两入口中英文 help、元数据一致性、Skill 导出及既有分享回归通过。 |

本轮使用 localhost 契约服务，没有重新执行 BOE 真实账号登录，也没有运行 Windows 实机。未修改线上文档或用户凭据。
