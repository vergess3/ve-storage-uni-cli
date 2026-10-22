# ChangeLog of Volcengine Storage Unified CLI

## 版本号：v1.0.3（未发布）

- tos、ve-tos、ve-adrive 各格式命令级 `service_request_ids` 统一保留前 100 个，超出数量记录在 `service_request_ids_omitted`。
- 执行结果 report CSV 尾部新增 `request_id` 列，成功、失败项有服务 ID 均逐项记录，没有则留空；文件明细不受命令级 ID 上限影响。
- 修复 ByteTOS V1 中文及特殊字符路径的签名错误；签名使用原始路径，HTTP URL 保持一次编码，PSM 转发保留完整编码路径。
- 修复 tos、ve-tos、ve-adrive 批量操作的 table/CSV 被请求 ID 列表替代，恢复摘要、计划清单路径和报告路径。
- 修复 ve-tos 独立入口 MCP/SSE 子进程重复传入 `ve-tos`，导致工具执行参数解析失败的问题；统一入口保持原有参数格式。
- 修复 tos、ve-tos、ve-adrive 本地目录递归上传遗漏空目录的问题；cp 和 sync 默认保留目录，新增 `--disable-dir-object` 关闭显式目录上传，并同步帮助与发现信息。
- 修复 ve-adrive 独立入口 MCP/SSE 子进程重复传入 `ve-adrive`，导致工具执行参数解析失败的问题。

## 版本号：v1.0.0 日期：2026-06-16

### 变更内容

- 第一次正式发布
