# share files 表格对齐修复

**Goal:** 默认 table/CSV 与 ls 使用相同五列、同一文件/目录行规则，不再展示 Files/Folders JSON 单元格。

**Architecture:** 复用已有 ls 的固定列、目录 marker 规范化、文件/目录行映射。仅普通 table/CSV 添加展示 entries；保留 JSON、显式 query、SDK 原始数组、接收鉴权及分页接口契约。

**Tech Stack:** Rust、现有公共输出管线；两个实际二进制及 Python 本机 HTTP 夹具。

- [x] 新增实际进程回归，修复前因 mixed/table 与 ls 不一致失败。
- [x] 最小实现：让 share_receive 使用同一套 ls 行映射及列定义。
- [x] 验证混合文件目录、空页、仅目录、当前目录 marker、重复 marker、负数 size、单文件、分页提示、JSON/显式 query。
- [x] 同步双语 help/Describe/Skill/MCP、README、内置 Skill 和测试指南。
- [x] 执行 ADrive 全量、两入口构建、现有分享 URL/下载/OAuth 回归及完整 Review。
- [x] 记录十个回归用例，与修复一并提交 xsj-dev；不推送。
