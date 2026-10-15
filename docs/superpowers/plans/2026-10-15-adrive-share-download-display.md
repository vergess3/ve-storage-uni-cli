# 分享下载汇总及拒绝提示修复

**Goal:** 回答空目录、403 和单文件选择问题；修复默认 table 隐藏汇总及多文件拒绝时的超宽 JSON 单元格。

**Architecture:** 仅默认 table 的 share download envelope 展示汇总及单独文件状态行；显式 query 和其他格式沿用旧结构。按真实服务错误码 Forbidden.ShareDownloadDisabled 提供分享者权限提示，保留状态码、请求 ID、退出码和签发语义。权限及目录预创建流程保持原有服务/CLI契约，不自动更改权限，不新增请求/重试。

- [x] 只读确认用户分享仍为 view；plans/ 能列举29个文件。
- [x] 本机夹具修复前失败：空结果隐藏汇总；41文件拒绝的表格最大行3726字符、目录2个、文件0个。
- [x] 验证空目录、无path单文件、41文件403、JSON/error契约和query。
- [x] 更新双语help/Describe/Skill及MCP共用元数据和用户文档；两入口14项实际帮助/发现检查通过。
- [x] 执行ADrive全量、根入口/独立入口及既有下载、列举、URL、OAuth回归，完成两轮七类独立Review。
- [x] 保存验证记录及至少五个用例，准备与修复一并提交xsj-dev，不推送。

实际单文件下载复现：服务端返回403 / Forbidden.ShareDownloadDisabled，Completed=0、Failed=1，临时目标目录未写入文件。未变更分享权限。
