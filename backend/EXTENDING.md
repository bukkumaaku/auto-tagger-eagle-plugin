# 扩展 ai-worker

框架负责 JSONL 输入输出、协议版本检查、请求路由、错误响应和进程生命周期。
具体功能只放在 `crates/ai-worker/src/handlers/` 下。

## 新增一个命令

你只需要改三类位置：协议、功能实现、功能注册。

### 1. 定义协议

在 `crates/protocol/src/lib.rs` 中：

1. 新增请求和结果结构体。
2. 给 `Command` 增加一个请求 variant。
3. 给 `ResultPayload` 增加对应的结果 variant。

协议层只定义数据，不读取文件、不运行模型、不处理 stdin/stdout。

### 2. 实现功能

新增 `crates/ai-worker/src/handlers/<feature>.rs`：

```rust
use protocol::{FeatureRequest, FeatureResult};

use super::{HandlerError, HandlerResult};

pub fn handle(request: FeatureRequest) -> HandlerResult<FeatureResult> {
    // 在这里实现功能。
    Err(HandlerError::new("NOT_IMPLEMENTED", "feature is not implemented"))
}
```

成功时返回：

```rust
Ok(FeatureResult { /* fields */ })
```

可预期的功能错误使用：

```rust
Err(HandlerError::new("ERROR_CODE", "message for the user"))
```

### 3. 注册功能

在 `handlers/mod.rs` 导出模块：

```rust
pub mod feature;
```

然后在 `BuiltinHandlers::handle` 增加一个分支：

```rust
Command::Feature(request) => feature::handle(request).map(ResultPayload::Feature),
```

框架会自动保留 `requestId`，把成功结果包装为 `result`，把
`HandlerError` 包装为 `error`。

## 使用 Session 缓存

`BuiltinHandlers` 持有进程内的 `SessionManager`。需要模型 Session 的命令在注册
分支中把它传给 handler：

```rust
Command::WdTag(request) => {
    wd_tag::handle(self.sessions(), request).map(ResultPayload::WdTag)
}
```

handler 通过 `get_or_try_init` 复用 Session：

```rust
use crate::{
    handlers::{HandlerError, HandlerResult},
    sessions::SessionManager,
};

pub fn handle(
    sessions: &SessionManager,
    request: WdTagRequest,
) -> HandlerResult<WdTagResult> {
    let session = sessions
        .get_or_try_init("wd-tagger", || load_session(&request.model_path))
        .map_err(|error| HandlerError::new("SESSION_INIT_FAILED", error.to_string()))?;

    let mut session = session
        .lock()
        .map_err(|error| HandlerError::new("SESSION_LOCK_FAILED", error.to_string()))?;

    run_inference(&mut session, request)
}
```

Session 管理规则：

- 相同 key 和类型返回同一个 Session。
- 注册表使用读写锁，多个查找可以并行。
- 每个 Session 使用独立互斥锁，同一个 Session 同时只允许一个调用。
- 不同 Session 可以同时运行。
- `remove(key)` 从缓存注销一个 Session。
- runner 退出时自动调用 `shutdown()` 清空全部 Session。
- Session 只在当前进程中存在，不写入 JSON，也不会跨进程恢复。

## 验证

给协议解析和 handler 各写一个测试，然后运行：

```powershell
cargo fmt --all
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

还可以直接通过 JSONL 测试完整链路：

```powershell
'{"protocolVersion":1,"requestId":"r1","type":"echo","payload":{"message":"hello"}}' | cargo run -q -p ai-worker
```

## 边界

- handler 接收类型化的请求，返回 `HandlerResult<T>`。
- handler 不读取 stdin，也不写 stdout。
- 正常新增命令时不要修改 `runner.rs`、`transport.rs` 和 `main.rs`。
- stdout 只允许输出 JSONL 协议消息，诊断日志写 stderr。
- 新功能需要进度事件、并发或取消时，再扩展框架，不在 handler 里绕过协议。
