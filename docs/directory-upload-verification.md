# 本地目录上传与 sync 验证记录

日期：2026-10-21。开发分支：`xsj-dev`。

## 交付行为

| 用户要求 | 实现与验证 |
| --- | --- |
| 默认保留空目录 | tos、ve-tos、ve-adrive 的本地目录 `cp --recursive` 和本地到远端 `sync` 均显式上传目录；目录计 1 项、0 字节 |
| 关闭显式目录上传的开关 | `--disable-dir-object`；默认 false；cp 要求 recursive；不支持的传输方向显式报错 |
| HNS / ADrive 特化 | TOS 使用尾斜杠、空内容 PutObject，剥离文件专用 storage-class；ADrive 使用 CreateFolder 并按父子顺序创建，复用已存在的真实目录 |
| 文档与发现 | README、三个导出 Skill、八份中英文内置指南、两种语言 help / Describe / Skill、capabilities 和 MCP 参数类型及实际派发均已同步 |
| sync 同步修复 | 两种清单模式均创建目录；`--disable-dir-object` 不会使源端仍存在的空目录成为 delete-extra 对象；创建失败阻止后续清理 |

`--disable-dir-object` 仅关闭显式目录创建。HNS 和 ADrive 上传文件仍可能由服务端创建必需的父目录。

空源根目录映射到有效的目标前缀；桶根和空间根不会转换成空 key。`--include-parent` 保持原有路径映射。过滤器保留各命令既有的文件路径匹配规则；目录匹配去掉末尾斜杠，选中文件会保留必要祖先。有效目标根目录作为结构容器保留。

## 代码与原因

- `crates/tos/src/handler/directory_upload.rs`：目录扫描、显式任务、空内容上传、HNS 安全请求头及 sync 目录保护。通用文件扫描器保持原有职责，内部 mv 继续沿用原策略。
- `crates/adrive/src/handler/directory_upload.rs`：缓存有类型的父目录列表，区分目录和同名文件；409 只在再次查询确认是真实目录后视为成功；恢复执行重新确认目录是否存在。
- 两个服务的 `cli/high_level.rs`、`handler/high_level.rs`：接入 manifest 与 no-manifest、dry-run、结果统计、删除失败门禁和 checkpoint 身份。
- 三个 registry、三个 metadata handler、`src/lib.rs`：规范参数、boolean schema、实际 MCP 参数映射及中英文描述。新参数不暴露在 mv 等无关命令。
- `tests/directory_upload_regression.py`：六组实际二进制入口对本机模拟服务执行验证。

## 验证结果

以下为目录功能初次交付时的验证；参数收敛后的本次回归见末节，未重复执行全部工作区测试。

- 全工作区串行回归：`cargo test --workspace --locked --offline -- --test-threads=1`，**1632 项通过、0 失败、4 项原有忽略**。该结果包含目录功能完整修改，不包含随后独立新增的两项 ve-tos MCP 参数适配单测。
- 三个 standalone packaging crate：分别 **3 / 3 / 7 项通过，共 13 项**。
- ve-tos MCP 适配修复完成后的 TOS 核心完整串行回归：**338 项通过、0 失败**，包含新增的两项入口适配单测。
- 新增目录专项 Rust 用例：TOS 17 项、ADrive 18 项，包含在上述回归中。
- 既有分享及 OAuth Python 回归：**15 组通过**，覆盖统一入口、ADrive 独立入口及相关 PTY 场景。
- 所有四个二进制重新构建后执行：

  ```bash
  PYTHONDONTWRITEBYTECODE=1 python3 tests/directory_upload_regression.py \
    target/debug/ve-storage-uni-cli target/debug/tos-cli \
    target/debug/ve-tos-cli target/debug/ve-adrive-cli
  ```

  **六组入口 / 服务组合全部通过**。覆盖目录请求和内容、关闭策略、清单模式、重复执行、空根及 include-parent、dry-run 无服务请求、中英文发现及 MCP `execute=true` 的子进程 dry-run。sync 删除保护及目录创建失败阻止清理由 Rust 专项用例和全包回归验证。配置和凭据使用临时文件，checkpoint 显式放入临时目录。
- MCP 参数类型的反例先失败后通过：字符串、数字、null 均不能替代 boolean；目录上传规划和安全行为在实现前运行失败用例，再执行修复后验证。
- ve-tos 独立入口联测额外发现的既有 MCP 前缀缺陷已单独修复和验证，见 `docs/ve-tos-mcp-entrypoint-verification.md`。

新增目录服务测试全部使用本机模拟服务和假凭据。已有全工作区 live smoke 因环境中具备测试凭据实际执行成功，涉及其自建的唯一临时桶、对象及 multipart；随后只读列桶确认本次测试时间范围内的 smoke 桶剩余数量为 **0**。该 smoke 不等同于本次新增目录行为的真实云端验收。

早期过程夹具暴露了原有批任务默认 checkpoint 路径的行为，曾产生六个本机模拟任务 journal；已按假 URI 和临时源路径逐个确认并清理。最终夹具显式指定临时 checkpoint 目录，避免依赖用户目录。

## Review 记录

| 轮次 | 发现与修正 | 结果 |
| --- | --- | --- |
| 实现者自查与规格审查 | 检查三个服务、两种执行模式、过滤器兼容、目录失败和 sync 删除保护；修正必要父目录及空根结构容器处理 | 重新运行受影响用例 |
| 独立完整 Review 与修正 | 修正 bool 非法值静默忽略、MCP clap 参数映射、中文目录和默认值描述、CreateFolder API 发现信息、原子 no-clobber / 并发 412、夹具隔离 | 均有明确原因及 Review Fix 注释；相关用例通过 |
| 最终独立完整 Review | 重新检查正确性、安全性、性能、可维护性、健壮性、可测试性及可观测性 | Critical 0、Major 0、Minor 0、Suggestion 0 |

## 发布验收场景

1. 混合树、嵌套空目录和零字节文件：目录存在，文件内容不变，统计字节正确。
2. 完全空的源根目录：普通前缀与 include-parent 创建预期目录，桶 / 空间根不创建空 key。
3. 规范长参数、已移除别名拒绝及两种清单模式：停止显式目录任务，继续上传文件；方向和类型错误正确拒绝。
4. HNS 及 ADrive 隐式父目录、重复执行、同名文件冲突：幂等目录成功，冲突及无权限不掩盖。
5. include / exclude 只选中文件：保留必需祖先，不额外创建不匹配的空子目录。
6. 子目录符号链接及循环：不跟随；根路径别名按既有入口行为解析。
7. sync delete-extra 与关闭显式目录上传：源端目录受保护，真实额外目录按子先父后清理，目录创建失败时不清理。
8. checkpoint 恢复：远端目录被删除后重新创建，不凭旧成功记录跳过目录校验。
9. no-clobber：已有对象不覆盖，目录 PUT 带原子条件，并发创建的 412 视为跳过，403 / 非幂等冲突保留失败。
10. dry-run 与发现：无上传、删除及凭据修改；三个服务的 help / Describe / Skill / capabilities / MCP 与实际参数一致。

## 仍需区分的边界

- 本次新增行为的云端目录验收尚未执行；HNS 合约基于官方文档、本地 tosutil 源码核对及模拟服务测试。
- 运行环境为 macOS，未执行 Windows 平台运行验证。
- ADrive 远端目录下载保留空目录属于另一传输方向，本次本地上传修复不扩展该行为。
- 原有文件 checkpoint 跳过策略未在本次重做；新增目录任务每次重新校验。

无遗留 Critical / Major / Minor / Suggestion。

## 参数收敛回归（2026-10-21）

用户确认仅保留 `--disable-dir-object`。已移除别名声明，两个旧拼写均按未知参数拒绝；目录上传、默认值、递归约束、HNS / ADrive 和 sync 处理保持原有行为。README、CHANGELOG、八份中英文内置指南、三个导出 Skill、设计与计划、运行时错误和中英文发现描述均已同步。

- RED：先修改原有参数契约用例，两个 crate 均因旧别名仍被接受而失败，再删除生产声明。
- GREEN：`cargo test -p ve-tos-cli-core -p ve-adrive-cli-core directory_upload --locked --offline -- --test-threads=1`：**35 项通过、0 失败**。
- 帮助、Describe、能力契约及 Skill 导出：`cargo test --locked --offline --test metadata_consistency_test --test skill_export_test --test functional_agent_contract_test --test core_help_test -- --test-threads=1`：**57 项通过、0 失败**。
- 统一入口及三个独立二进制重新构建后运行上述目录过程脚本：**六组入口 / 服务组合全部通过**。实际验证长参数生效、旧拼写被拒绝、中英文 help / Describe / Skill 及英文 capabilities 均没有别名说明；默认及关闭目录上传在两种清单模式下均通过本机模拟服务验证。MCP 仍使用规范参数派发子进程 dry-run。
- 独立 Reviewer 完整检查七个类别：Critical 0、Major 0、Minor 0、Suggestion 0。

本次验收包括：三个服务的 cp 长参数、三个服务的 sync 长参数、旧拼写拒绝、中英文 help / Describe / Skill 及英文 capabilities 描述与 boolean 类型、Skill 导出、两种清单模式的实际目录上传。本次没有调用真实云端目录接口。
