# 批量摘要与请求 ID 展示回归修复

日期：2026-10-21。基线：`4d47a72`。

## 问题与修复

`service_request_ids` 保存一次命令中多个 HTTP 响应的追踪 ID，属于诊断信息。`9016dfca`（2026-08-11）将它加入成功 Envelope 的 data，与原有“唯一数组自动作为业务列表”的规则冲突。随后 `2ccca86` 调整多响应判定，仍保留该交互问题。

tos、ve-tos 和 ve-adrive 的批量 cp/sync 等摘要可能在 table/CSV 中变成请求 ID 列表或空表，隐藏统计、manifest_path 和 report_path。清单仍生成；JSON 字段没有丢失。

两份 `handler/common.rs` 现在只将保留字段 `service_request_ids` 排除出隐式业务数组候选。显式 `--query data.service_request_ids`、JSON、真实业务数组、声明列及空列表保持原有用途。代码有 `Review Fix #TabularTrace1` 原因注释。

## 验证

- 两包各新增 9 项契约用例，先通过实际 ServiceRequestTrace 注入复现：TOS 7 失败、ADrive 6 失败；修复后 18 项通过。
- 完整 common 模块回归：TOS 70 项、ADrive 44 项，共 114 项通过。
- 本轮合并工作区（含 V1 签名修复）的完整串行回归：1653 项通过，4 项原有忽略。首次隔离环境同时移除了非敏感区域值，导致原有 advanced 测试先报缺 region；补入固定测试 region 后完整重跑通过，未修改该测试或业务校验。
- 严格 SDK V1 验签夹具在统一和 tos-cli 独立入口、endpoint / PSM 四组中验证了成功和部分失败时的 table/CSV 清单与报告路径。
- 既有分享下载表格和分享文件列表回归，在统一与 ADrive 独立入口各通过；三个服务六组目录入口回归通过。
- 独立 Reviewer 完整检查七个类别，最终 Critical / Major / Minor / Suggestion 均为 0。

回归场景包括：有请求 ID 的批量摘要、无安全 ID 但多响应的空 trace 数组、业务列表与 trace 并存、空业务列表、声明列、显式 trace query、JSON 诊断保留、成功及部分失败、三个服务入口。

本次完整工作区回归进程移除了真实服务凭据环境变量并提供固定 region，未运行真实服务 live smoke；过程测试使用回环服务及假凭据。

## 2026-10-22：前 100 个摘要 ID 与逐行报告 ID

新基线为 `73e6901`。用户确认两条独立契约：各格式命令摘要最多保留前 100 个有效请求 ID，超出数量用 `service_request_ids_omitted` 表示；执行 report 每行成功、失败只要有自身 request_id 就记录，没有则留空，明细不受摘要上限限制。

核心 `ServiceRequestTrace` 上限由 1024 改为 100，`du` 诊断复用同一上限。JSON、表格、CSV、YAML、XML、Markdown 都在格式化前使用同一份已限制的摘要；显式 query 也只能读取保留的 ID。

报告项使用独立异步作用域跟踪服务响应，结果与 ID 一起进入报告，不读取命令／进程的最后 ID。嵌套作用域保留归属，交错并发互不污染。失败后的 abort 清理继续进入命令追踪，但不覆盖文件原始失败 ID；终结响应没有 ID 或终结请求未收到响应时留空。报告 CSV 在最后追加 `request_id` 列；TOS 报告 schema 为 3，manifest/action identity 继续为 2；ADrive 旧序列化报告快照缺少新字段时，反序列化默认空。TOS 追加记录遇到旧 CSV 表头时明确拒绝混写，应使用新的报告路径。

已观察到的 RED：

- 10000 请求契约测试期望 100，旧实现实际为 1024。
- 核心作用域接线前，并发、嵌套、重试、独立保留测试均实际失败。
- 两服务报告新列测试失败；TOS 旧二进制过程测试的 CSV 末列仍为 `action_id`，缺少 `request_id`（ADrive 旧末列为 `error`）。
- ADrive multipart 失败后，原失败 ID 被 abort ID 覆盖的回归实际失败，随后增加清理隔离。

过程回归命令：

```bash
PYTHONDONTWRITEBYTECODE=1 python3 tests/report_request_id_regression.py \
  target/debug/ve-storage-uni-cli target/debug/tos-cli \
  target/debug/ve-tos-cli target/debug/ve-adrive-cli
```

测试场景：正常文件成功、服务端失败、有／无响应 ID、目录报告、并发 ID 与文件对应、超过 100 个请求但所有文件行仍可保留 ID、cp/sync 的清单及流式模式、分片上传失败后的清理、终结传输无响应、旧报告兼容、取消及嵌套作用域恢复、错误正文不能冒充真实响应头 ID。

此轮全部 HTTP 回归使用回环夹具和假凭据，不修改真实对象、配置或凭据。

最终 GREEN 与审查结果：

- 核心 request_id 专项 20 项、TOS 报告专项 20 项、ADrive 报告专项 9 项通过。
- TOS 完整 high_level 159 项、ADrive 完整库 499 项通过。
- 完整工作区串行回归 exit 0：44 组，共 1690 项通过，0 失败、4 项原有忽略。真实服务环境变量已移除，提供固定测试 region，未执行真实服务 live smoke。
- 统一入口和 tos-cli / ve-tos-cli / ve-adrive-cli 四个二进制构建通过；报告过程回归六个入口／服务组合、每组 cp/sync 两种清单模式，共 24 组全部通过。
- ByteTOS Unicode endpoint/PSM 四组、目录六组、分享下载及文件展示各两个入口回归通过；fmt、diff 检查及 Python AST 检查通过。
- 独立 Reviewer 先完成核心审查，再做整合需求／七类质量审查。错误正文冒充 ID、无作用域兼容结果丢失 ID 两项在整合期间修复并复测；ADrive 自审发现的 cleanup 覆盖原 ID 也已修复。最终无 Critical / Major / Minor。
- 无新增命令参数，help、describe、skill 的参数元数据无需新增项；既有发现能力过程回归继续通过。README、CHANGELOG、请求 ID 设计及报告契约已同步更新。

可选后续扩展：panic／多线程迁移专项，以及下载、删除、恢复、移动、失败过滤与报告分卷的真实 CLI 逐行 ID 对照。当前 HTTP 过程回归聚焦本次上传型 cp/sync；其他报告路由已做代码审查及相关模块回归，未将可选过程用例宣称为已执行。
