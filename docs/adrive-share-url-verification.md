# 分享 URL 末尾斜杠兼容验证

2026-10-14，xsj-dev，基线 c70f564。

## 用户反馈及修复

用户运行 `share files http://vtagt3g9n.idsapp-boe.volces.com/share/2T4U8z0lY1n/`，报 URL 校验错误。原因是接收 parser 要求 ShareID 后不能带斜杠，与 HTTP/HTTPS 无关。

`crates/adrive/src/domain/share_url.rs` 中的完整可运行 parser 现在只移除一个可选末尾斜杠，再检查非空 ShareID 和单个原始路径段。`inspect/files/download` 共享该实现；输出 canonical ShareUrl、密码处理和 checkpoint 身份相同。API origin 校验没有改变。额外路径、多个斜杠、空 ID、控制字符、原始/编码遍历、凭据/fragment 和错误 percent escape 仍拒绝。

代码理由以 Review Fix #ShareTrailingSlash1 标注；#2 修正测试驱动 dry-run JSON 层级；#3 同步根入口中文翻译；#4 让回归使用各平台的临时目录；#5 兼容独立入口的 `.exe` 文件名和默认 Windows 二进制路径。

完整源码位于 `crates/adrive/src/domain/share_url.rs`、`src/lib.rs` 和 `tests/share_url_regression.py`。命令说明、注册表、双语 Describe、MCP/Skill schema 与导出、README、内置 Skill、测试指南已同步。

## 验证证据

- 修复前，新单元测试和实际进程测试均因末尾斜杠拒绝而失败；修复后通过。
- 首次进程验证的驱动误读 `data.ShareUrl`，真实 dry-run 为 `data.selection.ShareUrl`；修正后两个入口全部通过。
- 实际中文帮助检查发现根入口旧 catalog 遗漏新的 URL 文案；修正并增加中文 receiver help 回归。
- ADrive 全量：420 单元测试 + 9 分享契约测试通过。
- 根库全量：15 项通过，包含完整中文 help catalog 审计。
- 统一入口契约：11 项通过；独立入口契约：7 项通过。
- 两个二进制重新 build 成功。
- `tests/share_url_regression.py`：两个实际入口通过 HTTP/HTTPS dry-run、接收、目录 prefix/marker/limit、下载、跨 URL 写法恢复、输出/checkpoint 脱敏和非法 URL 零网络检查；独立入口另用 `.exe` 别名实际跑 6 次 dry-run。未宣称在 Windows 操作系统上执行过本轮测试。
- 既有 `share_download_regression.py` 和 `share_oauth_regression.py`：两个入口均通过。
- 38 项实际双语 help/Describe/Skill 导出/capabilities 检查通过。
- 用户原始 URL 的 dry-run 成功，输出 canonical URL 不带末尾斜杠。此步骤不访问云端、不校验该分享当前的权限或提取码。
- 独立 Reviewer 额外检查两入口共 8 次本地请求：编码 ID 和提取码只解码一次，密码不进入 GET query，接收请求不带账户 Authorization。
- 独立 Reviewer 两入口共 152 个非法 URL/API origin 案例通过。
- fmt 和最终 diff 检查通过；三轮完整 Review 通过后与修复一并提交。

本次使用隔离本机夹具、虚拟文件/密码与临时目录，没有修改任何云端分享或用户配置。

## Review

首次完整审查发现 0 Critical、2 Major、1 Minor：测试驱动 dry-run 层级、实际中文 help catalog、测试脚本临时目录可移植性，均已修复。
第二轮完整复查为 0 Critical、0 Major、1 Minor：测试驱动未识别独立入口的 `.exe` 后缀，已修复。该问题修复前已通过实际 `.exe` 别名复现参数错误，修复后的双入口完整进程夹具通过。
第三轮最终完整复查通过：0 Critical、0 Major、0 Minor、0 Suggestion。每轮重新检查正确性、安全性、性能、可维护性、健壮性、可测试性和可观测性；本次修复范围无遗留问题。

## 建议回归用例

1. HTTP 的 `/share/<id>` 和 `/share/<id>/` 返回同一 ShareID、API origin 与 canonical URL。
2. HTTPS/自定义端口的末尾斜杠兼容，协议和端口保留。
3. `--path 'docs/' --limit 1 --marker previous` 查询当前目录一页，NextMarker 保留。
4. 加末尾斜杠下载，不带斜杠恢复；完成文件不重新签发 URL。
5. `%2F`、`%252F` ID 和双重编码提取码解码一次，输出和 checkpoint 无凭据。
6. 空 ID、双斜杠、追加子目录、编码遍历、非法编码应在网络请求前拒绝。
7. inspect/files/download 的双语 help、Describe、Skill 导出及 MCP 参数描述一致。
8. 默认匿名、显式 OAuth、提取码校验和服务端下载权限保留既有语义。

本次不更改 `share files` 与 `ls` 的表格差异；此前已确认的展示问题仍需单独处理。
