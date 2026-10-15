# 分享下载拒绝输出与密码输入验证

2026-10-14，分支 xsj-dev，基线 5bf885e。

用户反馈：view 分享执行 download 只显示多个 `++`；使用 --password-stdin 输入后，单独回车不能结束。

## 原因及处理

- 用本地拒绝夹具复现：退出码 5，JSON 有 403/Forbidden.ShareDownloadDisabled；table 只有 `++\n++\n++\n`。
- 原因是表格/CSV 只取 envelope.data：空 Files 数组变成空表格，error.code/message 和失败状态被丢掉。
- 失败的表格/CSV 改为展示完整 envelope，保留错误、HTTP 状态、文件结果和汇总；正常结果与显式 query 继续使用原有选择方式。JSON 等结构化输出不变。
- 密码读取行为保持原有 EOF 契约：管道结束后执行；macOS/Linux 手动输入须回车后在空行按 Ctrl-D，30 秒内完成。files/download 普通终端的隐藏提示不传该标志，按回车即可。
- flag help、双语目录、Describe/Skill/MCP 的参数描述、README 和测试指南补充了 EOF/Ctrl-D 用法。

## 已执行验证

| 检查 | 结果 |
| --- | --- |
| ADrive 全量 | 418 单元测试 + 8 契约测试通过 |
| 统一入口契约 | 11 项通过 |
| 独立入口契约 | 7 项通过 |
| 两入口 build | 成功 |
| 分享下载二进制夹具 | table/CSV，计划形成前/后 403，以及已有下载、中断、恢复、未知签发用例通过 |
| OAuth 二进制夹具 | 两入口全部通过 |
| 实际伪终端 | 两入口验证：回车后仍等待、Ctrl-D 后继续；默认终端能显示拒绝原因 |
| 双语 help/Skill 导出 | 32 项通过 |
| fmt/diff 检查 | 通过 |

新增实际进程回归先失败于“tabular output hid the failure”，修复后通过。
第一轮全量测试中，既有 100ms 中断夹具出现一次 socket read timeout；对应源码未改，单项重跑及最后完整重跑均通过。
本轮仅使用隔离本地夹具、虚拟密码及临时目录，无云端修改。

独立 Reviewer 一轮完整七类审查通过，Critical/Major/Minor 均为 0。
独立验证了 32 项 common 单测、两入口进程夹具、双语 help/中文 Describe、JSON 与显式 query，以及输出脱敏；无新增遗留问题。

## 回归场景

1. token 交换阶段拒绝：空文件列表仍显示错误码、原因和任务状态。
2. 下载地址签发阶段拒绝：文件结果和失败汇总保留。
3. 显式 CSV 输出：错误与汇总为 field/value 行，不返回空内容。
4. 默认 TTY table 输出：拒绝原因可见，不再只显示空表格边框。
5. 成功输出与显式 query：保留原选择行为。
6. 既有 JSON 的退出码、403、IssuanceOutcomeUnknown=false 保持一致。
7. --password-stdin：Enter 不发送 EOF；macOS/Linux 空行 Ctrl-D 后继续。
8. OAuth、隐藏输入、中断及未知签发语义保持不变。
