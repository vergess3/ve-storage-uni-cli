# 分享 URL 末尾斜杠兼容修复

**Goal:** 用户提供的 `http://vtagt3g9n.idsapp-boe.volces.com/share/2T4U8z0lY1n/` 应通过本地 URL 校验。

**Architecture:** 只在接收 URL 的原始路径中移除一个可选末尾斜杠，再检查单个 ShareID 路径段。canonical ShareUrl、分享身份、密码解码和 checkpoint 绑定一致；不更改 API origin 校验，不允许子路径或多个斜杠。HTTP 和 HTTPS 都保留。

**Tech Stack:** Rust、既有 URL parser、Clap；Python 隔离本地 HTTP 夹具。

- [x] 单元回归先证明旧实现拒绝有效的末尾斜杠；同时检查额外路径、空 ID、编码与密码脱敏。
- [x] 在 `crates/adrive/src/domain/share_url.rs` 用 `strip_suffix('/')` 只规范化一个末尾斜杠，更新错误提示和接口注释。
- [x] 同步接收命令 help/Describe/Skill/MCP 使用的共享说明及中文翻译，README、内置 Skill、测试指南。
- [x] 两个实际二进制验证 inspect/files/download、HTTP/HTTPS dry-run、下钻、分页、下载恢复和非法路径不访问网络。
- [x] 执行 ADrive 全量、入口契约和现有分享进程回归；按七类清单完成独立 Review，修复 Critical/Major 后重查。
- [x] 保存验证记录和八个回归用例，与修复一并提交 xsj-dev；不推送。
