# Share download progress and cp-style summary

**Goal:** 按已确认的cp体验补齐分享下载：stderr扫描/文件计数进度、结束汇总，以及成功/失败/恢复/中断契约。

**Architecture:** 复用cp的扫描和批处理进度样式，添加分享下载的生命周期适配；默认TTY启用，--progress强制启用，--no-progress/--quiet禁用。匿名接收保持不读取账户配置。默认table展示cp风格summary，不展开完整Files；JSON保留原字段，并增加Summary.Skipped区分恢复时已完成文件。Completed继续表示全部已完成文件。服务拒绝及未知签发行为保持原契约。

**Tech Stack:** Rust/tokio/indicatif；Python localhost及PTY进程夹具。

- [x] 新增双入口进度夹具，观察缺少TTY进度的真实失败；更新默认table和元数据契约断言。
- [x] 新建share_download_progress模块；最小暴露cp进度helper；串接扫描、清单、文件处理和中断清理。
- [x] 添加互斥progress/no-progress参数、Skipped计数和cp风格table；显式query及JSON的已有字段保留。
- [x] 同步中英help、Describe、Skill、MCP共用元数据、README和测试文档。
- [x] 跑ADrive全量/两入口/既有夹具；完成两轮七类独立Review，恢复计数Major已修复并复测。
- [x] 记录验证和至少五个用例；与修复一并提交xsj-dev，不推送。

请求链路同时验证：无path单文件为info/token/root-list/download-url/content五次请求；显式path以get-file替代root-list。CLI当前没有分享正文预览命令，本次不增加新预览入口。
