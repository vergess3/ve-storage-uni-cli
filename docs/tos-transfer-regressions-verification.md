# ByteTOS 中文路径及目录显示排查记录

日期：2026-10-21。基线：`4d47a72`。

## 中文路径签名

`566fe33`（2026-07-30）正确添加了对象 URL 的一次百分号编码，同时错误地将 ByteTOS V1 签名路径改为编码路径，形成中文和特殊字符鉴权回归。旧测试 `bytetos_v1_signs_the_encoded_wire_path` 固化了错误预期；没有找到该提交中文路径通过真实 ByteTOS 鉴权验收的记录。`fe0749f` 处理的是进度文字的 UTF-8 安全截断，属于另一问题。

primary 参考是 tosutil 依赖的 `code.byted.org/gopkg/tos@v1.5.4`：`tos.go:601` 使用 Go `req.URL.Path`，`sig.go:201` 直接将原始路径写入 canonical bytes。HTTP RequestURI 使用 escaped path，二者不能混为一条编码后的字符串。

`crates/tos-core/src/infra/client.rs` 已恢复 ByteTOS V1 使用原始逻辑 key 签名，同时保留 object_endpoint 的一次编码。PSM 只替换目标 origin，传输路径取自编码后的 URL，避免 raw path 中的 `?`、`#`、`%2F` 被当成 query、fragment 或分隔符。TOS4 保持独立签名规则。

## 验证证据

- 修改契约测试后，原始签名路径和 PSM 编码路径两项先失败；修复后 V1 路径相关 10 项全部通过。
- 新增 `tests/tos_unicode_transfer_regression.py` 独立复刻 Go canonicalRequest 的 HMAC 校验，旧二进制实际 HTTP 请求在中文路径返回 403，ASCII 成功；修复后二进制通过。
- 固定向量：PUT、key `【田野巴士】The Fast Yellow Bus 黄色快速公交车/book.pdf`、bucket `bucket`、测试 secret `sk`、expiration `1700003600`，原始路径签名为 `YJj75zGLcScmNbJIkFADkh89q36l2X`；错误的编码路径得到不同签名。
- 统一及 tos-cli 独立入口，在 endpoint 与 PSM 下共四组过程回归全部通过。覆盖中文源根、中文和空格/加号/百分号/转义外观/问号/井号文件名、字节内容、空目录尾斜杠与零内容、两种清单模式及关闭显式目录上传。
- 夹具比较全部文件 key 和内容，验证每个目录 key 对应零字节，未出现额外同名无斜杠文件；并验证 table/CSV 的成功与部分失败摘要。
- 全工作区串行回归 1653 项通过、4 项原有忽略；完整环境隔离说明见 `docs/tabular-request-id-verification.md`。
- 六组目录入口回归及两组分享展示回归通过；格式与独立七类 Review 通过，无遗留问题。

## BOE 实机验收

使用当前配置的 ByteTOS PSM、`--addr_family v6` 和给定测试桶，在随机独立前缀 `__cli_utf8_check_cce180493b3f400290d7e7fcec201a85/` 上传自建的几字节测试文件，未重传用户截图。源根为中文目录，文件名包含中文、空格、`+`、`%25`、`%2F`、`?` 和 `#`，同时包含空 `bin/`。

- 上传全部成功；服务请求 ID：`6621b5d8df8036c86ad8df80-2605g340gcd50g2000g7a60g7c94g91e1gaa0f`。
- 按原始完整 key 下载并逐字节比较通过，确认特殊字符未改名或变为 query / fragment。
- 空 `bin/` HEAD 返回 Content-Length 0。
- finally 精确清理该随机前缀，随后列表为空且根目录对象 HEAD 返回不存在。
- 配置及凭据文件的前后 SHA256 一致。

该实机验收与上述已禁用 live smoke 的工作区回归分开执行，不涉及用户原有对象的删除。

复现与验证命令：

```bash
cargo test -p tos-core bytetos_v1_ --locked --offline -- --test-threads=1
PYTHONDONTWRITEBYTECODE=1 python3 tests/tos_unicode_transfer_regression.py \
  target/debug/ve-storage-uni-cli target/debug/tos-cli
```

## 空目录与网页控制台

用户确认 `bin/bin` 出现在 TOS 网页控制台。约定的上传结果是 key 为 `bin/`、内容为 0 字节的目录对象，不能用额外 `bin` 或 `bin/bin` 替代。

对原测试前缀只读核对：`xsj/123/` HEAD 200，Content-Length 0、Content-Type `application/x-directory`；`xsj/123`、`xsj/123/123`、`xsj/123/123/` 均 404。官方 Go SDK 原始 ListPrefix 查询 `xsj/123/` 返回 `objects=[{key:"xsj/123/",size:0}]`，即目录对象自身；当前 `tos-cli ls .../xsj/123/` 返回空子列表。

因此，原例没有创建第二层同名 key，控制台需要过滤当前目录自身对象，避免将其显示为子文件。此次不改变已确认的 `bin/` 零字节对象契约，也不自动删除用户原有目录对象。该控制台显示问题仍需控制台侧处理，不能宣称由本次 CLI 修复解决。

本机夹具仍不能替代网页控制台验收。原测试前缀的服务核对均为只读；官方 SDK 的临时探针使用既有 BYTE_TOS_ENDPOINT 获取了上述原始列表。
