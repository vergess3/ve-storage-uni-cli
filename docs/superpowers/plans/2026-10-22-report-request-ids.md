# Report Request IDs Implementation Plan

**Goal:** 各格式命令摘要统一保留前 100 个服务请求 ID；tos、ve-tos、ve-adrive 的 report 每行有服务 request_id 就记录，没有则留空。

**Architecture:** 每个被执行的报告项有独立的异步请求跟踪作用域，HTTP 响应继续进入命令级摘要，同时更新所属报告项。作用域支持并发 future 与嵌套，报告不能依赖进程或命令的最后一个 ID。CSV 在既有列尾部追加 request_id，文件明细不受摘要上限影响。

**Tech Stack:** Rust, Tokio task-local, serde, existing CSV report writers.

- [ ] 核心回归：10000 个响应保留前 100 个、遗漏数 9900；最后成功 ID 保持最新。先确认旧实现测试失败，再调整常量。
- [ ] 核心作用域：新增 capture_service_request_id 与 current_service_request_id。验证交错并发、重试后成功、无 ID 响应、传输失败、嵌套与作用域释放。
- [ ] TOS 报告：为 BatchItemResult 和 CSV 尾列添加 request_id；覆盖单文件、递归、流式、同步与其他报告写入路径，先 RED 后 GREEN。
- [ ] ADrive 报告：按同样规则传递并写入文件级 ID，兼容旧报告反序列化，先 RED 后 GREEN。
- [ ] 验证：核心与两服务完整相关模块回归，实际二进制过程测试及至少 100 个文件以上的成功/失败报告对照。
- [ ] Review：先核对需求，再独立检查正确性、安全、性能、可维护性、健壮性、可测试性和可观测性；修复全部 Critical / Major。
- [ ] 更新 README、CHANGELOG 与验收记录，构建四个二进制，提交、同步 master 并推送 xsj-dev。

文件计划：核心 `crates/tos-core/src/agent/request_id.rs`；报告 `crates/tos/src/handler/high_level.rs` 与 `crates/adrive/src/handler/high_level.rs` 及其测试；过程测试 `tests/tos_unicode_transfer_regression.py` 与现有目录夹具；文档 `README.md`、`CHANGELOG.md`、`docs/tabular-request-id-verification.md`。

核心命令：

```bash
cargo test -p tos-core agent::request_id::tests --locked --offline -- --test-threads=1
cargo test -p ve-tos-cli-core handler::high_level:: --locked --offline -- --test-threads=1
cargo test -p ve-adrive-cli-core handler::high_level:: --locked --offline -- --test-threads=1
```

成功/失败均填写本项服务 ID。响应未提供 ID、本地失败或终结传输无响应时留空。目录显示排查仍等待控制台对象详情，不改变已有对象。
