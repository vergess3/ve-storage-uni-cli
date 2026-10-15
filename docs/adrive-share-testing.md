# ADrive 分享验证步骤

适用于 `xsj-dev` 当前源码。以下统一使用独立入口；统一入口对应为 `ve-storage-uni-cli ve-adrive`。管理命令使用已登录的 OAuth Profile，接收命令使用分享 URL。全程无需重新填写 AK/SK 或修改 Profile 配置。

## 1. 构建与本地自动化

在仓库根目录执行：

```bash
cargo build --locked --offline
cargo build --manifest-path packaging/cargo/ve-adrive-cli/Cargo.toml --locked --offline
cargo test -p ve-adrive-cli-core --locked --offline -- --test-threads=1
python3 tests/share_download_regression.py
python3 tests/share_range_regression.py target/debug/ve-storage-uni-cli packaging/cargo/ve-adrive-cli/target/debug/ve-adrive-cli
python3 tests/share_view_regression.py target/debug/ve-storage-uni-cli packaging/cargo/ve-adrive-cli/target/debug/ve-adrive-cli
SHARE_CLI="$PWD/packaging/cargo/ve-adrive-cli/target/debug/ve-adrive-cli"
```

预期：Rust 测试通过；Python 报告两个入口 PASS。Python 只访问本地 HTTP 夹具，不访问云端，覆盖 dry-run、错误聚合、SIGINT、选择器保护、403、目录别名恢复及无效下载地址。

## 2. 文档与能力发现

```bash
"$SHARE_CLI" share --help --language zh
"$SHARE_CLI" share list --help --language zh
"$SHARE_CLI" share list --describe --language zh --output json
"$SHARE_CLI" share download --help --language zh
"$SHARE_CLI" share download --describe --language zh --output json
"$SHARE_CLI" share view --help --language zh
"$SHARE_CLI" share view --describe --language zh --output json
"$SHARE_CLI" capabilities --view full --output json --query "data.capabilities[?group=='share']"
"$SHARE_CLI" skill list --output json --query "data.skills[?starts_with(name, 've_adrive_share_')]"
SHARE_DOCS="$(mktemp -d)"
"$SHARE_CLI" skill export --name ve_adrive_share_download --language zh --dir "$SHARE_DOCS"
"$SHARE_CLI" completion bash --output json
```

预期：发现 9 个命令 `create/get/list/update/cancel/inspect/files/view/download`，对应 9 个 Skill。Describe 包含参数类型、风险、API 映射、输出结构和行为说明；list 暴露可选位置参数 `target` 和条件必填的 `instance`，说明实例 URI 与 flag 两种写法。导出的下载 Skill 包含相同中文域名、根目录与未知结果说明。补全包含分享命令。MCP 工具由同一注册表生成，名称为 `ve_adrive_share_*`；工具调用默认生成计划，`execute=true` 才执行，`describe=true` 可省略实例选择器。

## 3. 云端准备

以下使用前次已实测的 BOE 实例及现有测试空间。如切换环境，修改 Instance、Space、API 地址和 OAuth Profile；先确认该 Profile 的 OAuth 会话绑定对应实例。

```bash
SHARE_INSTANCE=vtagt3g9n
SHARE_SPACE=3e9074d3a3f748ed
SHARE_PROFILE=default
share_manage() {
  "$SHARE_CLI" --profile "$SHARE_PROFILE" --auth-mode oauth \
    --output json "$@"
}
share_manage auth status
share_manage ls "adrive://$SHARE_INSTANCE/$SHARE_SPACE/"
```

预期：OAuth `ready=true`，空间列表成功。如果不成功，先处理鉴权/实例绑定问题再进行后续写操作。本例直接使用已有 API 配置，不传 `--endpoint` 或 `--share-base-url`。标准 IDS 域名映射保留协议和端口；HTTP API 推导 HTTP 分享页面，HTTPS API 推导 HTTPS 分享页面。自定义页面使用 `--share-base-url`，接收 API 可单次传 `--endpoint`。

创建本轮专用文件夹和一个小文件，保留空目录：

```bash
SHARE_LOCAL="$(mktemp -d)"
SHARE_FOLDER="share-manual-$(python3 -c 'import uuid; print(uuid.uuid4().hex[:12])')"
SHARE_REMOTE="adrive://$SHARE_INSTANCE/$SHARE_SPACE/$SHARE_FOLDER/"
printf 'share test\n' > "$SHARE_LOCAL/report.txt"
share_manage mkdir "${SHARE_REMOTE}empty/" --parents
share_manage cp "$SHARE_LOCAL/report.txt" "${SHARE_REMOTE}report.txt"
SHARE_CREATED="$(share_manage share create "$SHARE_REMOTE" --name "$SHARE_FOLDER" --permission download --expires-in 1h --require-login false)"
printf '%s\n' "$SHARE_CREATED"
```

确认创建成功后提取响应字段（只在本地变量中保存）：

```bash
SHARE_ID="$(printf '%s' "$SHARE_CREATED" | python3 -c 'import json,sys; print(json.load(sys.stdin)["data"]["ShareID"])')"
SHARE_URL="$(printf '%s' "$SHARE_CREATED" | python3 -c 'import json,sys; print(json.load(sys.stdin)["data"]["ShareUrl"])')"
```

预期：当前默认 HTTP API 配置返回 `http://vtagt3g9n.idsapp-boe.volces.com/share/<ShareID>`；如果使用 HTTPS API 或页面覆盖，则返回对应 HTTPS URL。不附加文件名或目录路径，CLI 接收命令不需要再传 API endpoint；网页是否重定向由服务端决定。

默认权限按 PRD 为仅预览，对应 V1 的 `view`。本例会下载目录，因此创建时显式传 `--permission download`。更新时未传 permission 保留原权限。

密码是服务端提取码：create/update 的 `--password-stdin` 把输入映射为 V1 `SharePwd`；分享者将链接和提取码交给接收者。接收者在 files/view/download 中使用同一参数输入该提取码，CLI 调用 `GetShareAccessToken`，由服务端校验后返回分享访问 token。inspect 只读取公开概况，不需要密码。PRD 尚未确认清除提取码语义，空输入不能作为已确认的清除操作。

`--require-login` 映射为 V1 `RequireLogin`，控制接收者是否必须登录，不改变分享者创建和管理时的身份要求。false 允许按密码、权限等规则匿名接收；true 要求接收者登录。接收默认匿名，即使 profile 已配置 OAuth；默认 files/view/download 会拒绝 RequireLogin=true 的分享，保留服务端拒绝码（实测 Forbidden.ShareRequireLogin）；如果服务端允许读取概况，CLI 则返回 `share_login_required`。显式 `--auth-mode oauth` 在 inspect/files/view/download 中复用所选 `--profile`，沿用登录与刷新机制，不自动登录或切换身份。服务端验证登录、实例绑定、scope 和分享权限；auth status 有缓存 token 并不等于服务端必然允许访问。

接收者先配置 profile 的 API endpoint，执行登录与状态检查，再显式启用 OAuth：

```bash
"$CLI" --profile receiver auth login --instance "$SHARE_INSTANCE"
"$CLI" --profile receiver auth status
"$CLI" --profile receiver share inspect "$SHARE_URL" --auth-mode oauth
"$CLI" --profile receiver share files "$SHARE_URL" --auth-mode oauth --path "$SHARE_FOLDER/" --limit 1
# 将上一页 data.NextMarker 赋给 NEXT_MARKER，再取下一页
"$CLI" --profile receiver share files "$SHARE_URL" --auth-mode oauth --path "$SHARE_FOLDER/" --limit 1 --marker "$NEXT_MARKER"
"$CLI" --profile receiver share download "$SHARE_URL" ./share-oauth-download --auth-mode oauth --recursive
```

显式 OAuth 的 API 来自所选 profile 或 `--endpoint`，不向复制来的任意页面域名发送账户凭据；标准 IDS 页面实例须匹配 OAuth 绑定。OAuth 用于概况和换取分享 token，文件 API 只携带 x-share-token，内容下载不携带账户凭据。设置提取码时 OAuth 仍须提供。普通终端可隐藏提示输入；`--password-stdin` 读取到 EOF，建议通过管道输入。提取码不会保存，每个 files 页和 download 独立调用都需要提供：

```bash
read -r -s -p '提取码: ' SHARE_PASSWORD; printf '\n'
printf '%s\n' "$SHARE_PASSWORD" | "$CLI" --profile receiver share files "$SHARE_URL" --auth-mode oauth --password-stdin --limit 1
printf '%s\n' "$SHARE_PASSWORD" | "$CLI" --profile receiver share files "$SHARE_URL" --auth-mode oauth --password-stdin --limit 1 --marker "$NEXT_MARKER"
printf '%s\n' "$SHARE_PASSWORD" | "$CLI" --profile receiver share download "$SHARE_URL" ./share-oauth-download --auth-mode oauth --password-stdin --recursive
unset SHARE_PASSWORD
```

files 默认一页，`--limit` 对应 page size，`--marker` 对应上一页 NextMarker；IsTruncated=false 时结束。服务文档默认/最大 100，CLI 校验正数后透传；精确文件查询不分页。download 自动枚举分页。新的匿名/OAuth checkpoint 均记录实际 API；OAuth 还绑定选定 profile、模式和实际请求中可用的 user_id，续 token 时该身份上下文改变会拒绝继续。环境变量或仅 access token 凭据未提供 user_id 时只能绑定 API/profile/模式，CLI 不能确定跨恢复替换的未知主体 token 是否来自另一个账户，也不会解析 token 推断身份。旧匿名 checkpoint 只在 URL 推导 API 下迁移，原来自定义 API 未被记录时须新建任务。checkpoint 仅保留非敏感身份上下文，不含账户 token、分享 token 或密码。

`share files` 默认 table/CSV 与 `ls` 使用相同五列：file_path、size、file_type、updated_at、is_folder，目录在前，空页也保留列头。当前目录和重复目录 marker 按 ls 规则规范化，表格 Total 统计展示行数。JSON 和显式 `--query` 保留既有服务原始数组、字段及分页契约。

## 4. 核心验收

下载不传 `--path` 会选择全部分享根；单文件分享自动选中该文件，目录分享仍需 `--recursive`。`--path` 用于选择某个文件或子目录。预检查会先创建目录结构，后续申请下载地址被服务端拒绝时，可能只留下空目录。默认 table 与 cp 一样展示 total/succeeded/failed/skipped 汇总，并补充 pending；空结果也保留汇总。逐文件明细用 JSON 或显式 query 查看。JSON 的 Summary.Skipped 为恢复时复用的已验证完成文件数，包含在 Summary.Completed 中；表格 succeeded 为本次新增完成数。TTY 默认在 stderr 显示扫描和按文件计数的进度，--progress 强制开启（非TTY为普通文本行），--no-progress/--quiet 关闭；两个progress参数互斥。`Forbidden.ShareDownloadDisabled` 是分享策略禁止下载，应由分享者开启 `--permission download`。

有强 ETag 或 CRC64 且大于等于 20MB 的文件自动使用 16MiB Range、默认 4 路并发，无需 checkpoint。用 `--checkpoint-threshold 64MB`、`--multipart-concurrency 8` 调整；当前文件的分片完成进度也写到 stderr。分片失败只重试该分片，复用同一下载 URL。无强 ETag/CRC64 时使用单次流式 GET，避免混合版本。checkpoint 仍是文件级恢复，不复用未完成文件的分片。

`share view` 使用预览权限，默认返回文档预览 URL 或普通文件内容信息；`--raw` 将原始字节输出到管道/重定向（例如接 `less`），不会混入成功 Envelope。MCP 只提供结构化模式，不支持 raw。单文件分享可省略 --path，目录分享需指定完整文件路径。沿用提取码和显式 OAuth 规则；预览请求不会自动重试。当前给定 BOE 域名的预览路径实测为纯文本 404，需先确认可达的部署路由，不能把本地夹具通过当作 BOE 成功。普通单文件下载有 info/token/root-list/download-url/content 五次请求，显式 --path 以 get-file 替代 root-list；分页、刷新和内容重试可能增加请求。

```bash
share_manage share get "$SHARE_ID" --instance "$SHARE_INSTANCE"
share_manage share get "adrive://$SHARE_INSTANCE/$SHARE_ID"
share_manage share list "adrive://$SHARE_INSTANCE" --name "$SHARE_FOLDER"
share_manage share list --instance "$SHARE_INSTANCE" --name "$SHARE_FOLDER"
share_manage share update "$SHARE_ID" --instance "$SHARE_INSTANCE" --description 'manual check' --download-limit 0
"$SHARE_CLI" share inspect "$SHARE_URL" --output json
"$SHARE_CLI" share files "$SHARE_URL" --output json
"$SHARE_CLI" share files "$SHARE_URL" --path "$SHARE_FOLDER/report.txt" --output json
# 预览路由确认后执行；view 权限即可预览，无需允许下载。
"$SHARE_CLI" share view "$SHARE_URL" --path "$SHARE_FOLDER/report.txt" --output json
"$SHARE_CLI" share view "$SHARE_URL" --path "$SHARE_FOLDER/report.txt" --raw | less
"$SHARE_CLI" share download "$SHARE_URL" "$SHARE_LOCAL/downloads" --recursive --checkpoint "$SHARE_LOCAL/checkpoint.json" --output json
cmp "$SHARE_LOCAL/report.txt" "$SHARE_LOCAL/downloads/$SHARE_FOLDER/report.txt"
test -d "$SHARE_LOCAL/downloads/$SHARE_FOLDER/empty"
"$SHARE_CLI" share download "$SHARE_URL" "$SHARE_LOCAL/downloads" --recursive --checkpoint "$SHARE_LOCAL/checkpoint.json" --resume --output json
```

预期：两种 get 写法返回相同 ShareID 和 URL；两种 list 写法在相同过滤条件下返回相同分享条目和分页字段。list 的 URI 可带一个根目录斜杠，不能带 ShareID/空间/文件路径；update/cancel 也支持分享 URI，重复指定的 `--instance` 必须一致，`--by-name` 解析实例名称。下载和恢复都为 `Complete=true`、`Summary.Completed=1`、`Summary.Failed=0`；`cmp` 和 `test` 返回 0。空目录保留，精确文件查询只返回一个文件。`share files` 默认返回一页当前层；使用 `--limit 1` 检查 `NextMarker`，再传 `--marker` 取下一页，直到 `IsTruncated=false`。

整个命令组的输入按资源范围保持一致：create 用文件/文件夹 URI 或显式选择器；list 用实例 URI 或 `--instance`；get/update/cancel 用分享 URI 或 ShareID 加 `--instance`；inspect/files/view/download 使用公开 HTTP(S) 分享 URL。接收支持 `/share/<ShareID>` 和 `/share/<ShareID>/`，只规范化末尾一个斜杠；两者是同一分享和恢复任务。目录下钻使用 `--path '完整目录前缀/'`，不追加到分享 URL 后面。对应信息同步在 `--help`、`--describe`、capabilities、Skill 和 MCP 中。

建议按以下用例继续验收：

| 用例 | 操作 | 预期 |
| --- | --- | --- |
| 单文件分享 | 对 `${SHARE_REMOTE}report.txt` 再 create；调用 files/view/download | 一条文件，下载落为 `report.txt`；记录新 ShareID 用于清理 |
| 密码 | 创建时加 `--password-stdin`，接收时通过 stdin 提供正确/错误密码 | 正确密码成功，错误密码明确拒绝；inspect 不读取密码 |
| 明确禁止下载 | 将当前分享 update 为 `--permission view`，再下载到新目录 | 退出码 5、403、`Forbidden.ShareDownloadDisabled`、`IssuanceOutcomeUnknown=false` |
| 根目录与恢复 | 使用普通目录或自己配置的目录别名作为 destination/checkpoint | 正常下载；等价实际目录可恢复 |
| 并发更新 | 保存 get 的 `UpdatedAt`，更新后用旧值 `--updated-at` 再 update | 冲突，退出码 9；旧请求不覆盖新设置 |
| 撤销 | cancel 后再次 files | 明确拒绝，不继续签发下载地址 |

密码通过 `--password-stdin` 传入，避免放进 argv 或日志。该参数等待 EOF：macOS/Linux 手动输入时，30 秒内输入密码、回车，再在空行按 Ctrl-D；单独回车不会结束读取。管道关闭时自动执行。普通交互终端接收 files/view/download 可不传该参数，按隐藏提示输入并回车。未知签发结果、路径穿越、内部链接、checkpoint/lock 冲突与签发期间中断由本地自动化夹具验证，无需破坏云端环境来制造异常。

## 5. 清理本轮云端数据

先取消本轮每个创建成功的 ShareID，再删除这个随机文件夹。额外单文件/密码分享也需要逐个取消。
已在撤销用例中取消的 ShareID 无需再次取消；跳过对应 cancel 命令即可。

```bash
share_manage share cancel "$SHARE_ID" --instance "$SHARE_INSTANCE" --yes
share_manage rm "$SHARE_REMOTE" --recursive --force --confirm "$SHARE_REMOTE" --yes
share_manage stat "$SHARE_REMOTE"
```

预期：取消、删除成功；最后 stat 返回 404 / 退出码 4。清理仅使用完整的随机测试目录 URI，保留现有 Instance、Space 和其他内容。本地样本与导出文档在 `$SHARE_LOCAL`、`$SHARE_DOCS`，检查后自行删除。

仓库中的 `scripts/e2e/adrive/test_share.py` 默认 fixture 使用 AK/SK 环境变量；现有 BOE 分享管理要求 OAuth，不能把该 fixture 的 AK/SK 失败当成上述 OAuth 功能失败。上一轮 OAuth 实测通过适配器复用了其中的生命周期断言；本文给出可复现的 OAuth 管理及匿名/显式 OAuth 接收命令。

完整 cp 差异审计、回归记录与审查修正见 [Range 与预览验证报告](adrive-share-ranges-and-view-verification.md)。
