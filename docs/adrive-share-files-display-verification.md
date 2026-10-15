# share files 与 ls 的表格对齐验证

2026-10-14，xsj-dev，基线 7feb210。

## 原因与修复

此前末尾斜杠修复只涉及 URL parser，没有落实已确认的表格问题。share files 输出服务原始 Files/Folders，通用渲染器发现多个数组后转为 field/value 表格；ls 则提供明确的文件/目录展示行和固定列。

本次 `crates/adrive/src/handler/share_receive.rs` 直接复用 `high_level.rs` 的既有列、marker 规范化和纯行映射函数（Review Fix #ShareFilesRows1），仅默认 table/CSV 添加展示 entries。五列为 file_path、size、file_type、updated_at、is_folder；目录在前；空页保留列头；表格 Total 统计展示行数。没有新增请求、鉴权策略、列参数或递归行为。JSON、显式非空 query、服务原始数组及 marker 契约保持兼容。

共享 helper 仅提升为父模块内可见，ls 自身的映射逻辑没有改变。双语 help/Describe/Skill/MCP 共享说明、根帮助翻译、README、内置 Skill 和测试指南已同步。

完整可运行源码：`crates/adrive/src/handler/share_receive.rs`、`crates/adrive/src/handler/high_level.rs`；进程回归：`tests/share_files_display_regression.py`。

## 已执行验证

| 验证 | 结果 |
| --- | --- |
| Red | 新进程回归修复前明确失败：mixed/table 的 share files 为 field/value，ls 为五列行表 |
| 新双入口进程回归 | table/CSV 与 ls 输出逐字相等：混合、空页、仅目录、marker、负 size、单文件、分页；JSON/query 兼容通过 |
| ADrive 全量 | 420 单元测试 + 10 分享契约测试通过 |
| 根库与入口 | 根库 15 项、统一入口契约 11 项、独立入口契约 7 项通过 |
| 两入口 build | 成功 |
| 既有分享 URL/下载/OAuth 回归 | 两个入口均通过 |
| 双语帮助与发现能力 | 26 项实际 help/Describe/Skill 导出/capabilities 检查通过 |
| 用户分享只读实测 | 指定 URL files 执行成功，五列表头可见，展示 1 行，无 Files JSON 单元格 |
| fmt/diff | 通过 |

实测 URL 为用户提供的 `http://vtagt3g9n.idsapp-boe.volces.com/share/2T4U8z0lY1n`，仅获取分享概况、访问 token 和一页列表。没有申请下载地址、下载内容、修改分享或用户配置。工具输出仅展示表头和行数。

## Review 与遗留项

独立 Reviewer 第 1 轮完整七类审查通过：0 Critical、0 Major、0 Minor、1 Suggestion。新增函数满足函数长度、参数数目和嵌套限制；原鉴权、密码、分页、请求追踪与脱敏流程保持兼容。

Reviewer 独立执行分享契约 10/10、share_receive/receiver-auth 23/23 和新双入口进程夹具，全部通过。额外在两入口真实 PTY 中验证根目录无 --path、无 --output 时默认五列表格；非 TTY 保持 JSON，无 entries；缺失账户配置和凭据仍能匿名接收，秘密字段已脱敏。

唯一可选建议是将本轮独立验证的默认根目录/PTY 自动格式场景持久化到 Python 夹具。本次没有扩大脚本的终端控制范围：自动格式选择基础实现未改，核心表格逻辑已有固定 table/CSV 对比回归，默认终端场景已有独立执行证据。该项不阻塞交付，无未修复 Critical/Major/Minor。

## 建议回归用例

1. 同一混合文件/目录页，share files 与 ls 的默认 table 和 CSV 逐字相等。
2. 空页保留五列表头，table 显示 Total: 0。
3. 仅有空目录时，目录行包含 size=0、file_type=folder、is_folder=true。
4. Files 中当前目录和重复目录 marker 按 ls 规则处理，Total 与展示行数一致。
5. 负数 size 显示 0；空 FileType 显示 file，JSON 保留原始值。
6. 精确文件 --path 返回一条文件行，is_folder=false。
7. --limit/--marker 保留一页契约及 next_marker footer。
8. JSON 无展示 entries，显式 query 可选自定义字段和列，不受默认五列限制。
9. 默认真实终端选择 table，非 TTY 选择 JSON；匿名无账户配置仍能接收。
10. 现有 HTTP/HTTPS、末尾斜杠、OAuth、密码、下载与恢复功能通过回归。
