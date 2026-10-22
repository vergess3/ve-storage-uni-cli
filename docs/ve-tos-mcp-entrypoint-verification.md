# ve-tos 独立入口 MCP 参数派发修复验证

日期：2026-10-21。该修复与目录上传功能分别提交。

## 问题与修改

目录上传的真实 MCP 入口联测发现：`ve-tos-cli serve --mcp` 派发子进程时仍使用统一入口的 `ve-tos <command>` 命令路径，独立入口因此报 `unrecognized subcommand 've-tos'`。这是既有入口适配缺陷，与目录上传策略无关。

- `crates/tos/src/handler/mcp_entrypoint.rs`：只在独立入口删除完整命令路径中的服务前缀；不会误删值为 `ve-tos` 的 profile。缺失或不合法的命令路径在启动子进程前返回错误。
- `crates/tos/src/handler/meta.rs`：依据当前 invocation surface 选择参数格式，兼容重命名后的独立二进制；统一入口原样保留。
- `tests/tos_mcp_entrypoint_regression.py`：通过实际 stdio MCP 调用执行只读 `ls --describe`。临时配置使用无效占位 endpoint，不调用云服务。

## 验证证据

1. 修复前二进制运行过程回归失败，错误为 `unrecognized subcommand 've-tos'`，退出码 6。
2. `cargo test -p ve-tos-cli-core handler::meta::mcp_entrypoint --locked --offline --quiet -- --test-threads=1`：2 项通过。
3. 重新构建统一入口及独立入口后执行：

   ```bash
   PYTHONDONTWRITEBYTECODE=1 python3 tests/tos_mcp_entrypoint_regression.py \
     target/debug/ve-storage-uni-cli target/debug/ve-tos-cli
   ```

   两个入口均通过，子进程退出码 0；返回的 Describe 对应 ls；临时凭据文件内容保持不变。
4. 目录上传联测的 MCP cp `execute=true` 子进程 dry-run 也通过了统一入口和 ve-tos 独立入口；该步骤不实际上传。
5. 修复后执行完整 `cargo test -p ve-tos-cli-core --locked --offline -- --test-threads=1`：338 项通过、0 失败。

## 审查与回归场景

独立 Reviewer 完整审查正确性、安全性、性能、可维护性、健壮性、可测试性及可观测性，最终 Critical / Major / Minor / Suggestion 均为 0。

已覆盖的场景：统一入口保留前缀、独立入口删除前缀、profile 与服务同名、空命令路径、不完整命令路径、服务不匹配、命令路径在 argv 中缺失、实际 MCP 子进程执行。SSE 与 stdio 共用同一派发函数；本次新增过程回归使用 stdio。
