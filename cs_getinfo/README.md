# getinfo 学习笔记

一个用 Rust + Tokio 实现的极简 TCP 客户端/服务端示例。客户端发送指令 `gettime`，服务端调用系统 `date` 命令并把结果返回给客户端。

---

## 项目概览

- 二进制：`server`（服务端）、`client`（客户端）
- 默认地址：`127.0.0.1:8888`（可通过命令行参数覆盖）
- 技术栈：`tokio` 异步运行时、`TcpListener`/`TcpStream`、`AsyncReadExt`/`AsyncWriteExt`、`tokio::process::Command`

目录结构：

```
getinfo/
├── Cargo.toml
└── src/
    ├── server.rs
    └── client.rs
```

---

## 环境依赖

- Rust（建议使用最新 stable）
- 操作系统：macOS 或 Linux（依赖系统命令 `date`）
- 主要依赖：
  - `tokio = { version = "1.48.0", features = ["full"] }`
  - `tokio-util = { version = "0.7.17", features = ["full"] }`
  - `bytes`, `futures`

> 注意：`Cargo.toml` 中 `edition = "2024"`，需要较新的 Rust toolchain。

---

## 快速开始

- 启动服务端：
  - 默认端口：
    - `cargo run --bin server`
  - 指定地址（例如监听所有网卡）：
    - `cargo run --bin server -- 0.0.0.0:8888`

- 运行客户端：
  - 默认连接到 `127.0.0.1:8888`：
    - `cargo run --bin client`
  - 指定服务端地址：
    - `cargo run --bin client -- 127.0.0.1:8888`

---

## 运行演示

服务端输出（示例）：

```
Listening on: 127.0.0.1:8888
offset: 0, n: 7
gettime
Tue Oct 31 14:56:27 CST 2023
```

客户端输出（示例）：

```
Tue Oct 31 14:56:27 CST 2023
```

---

## 核心代码解读

### 服务端（`src/server.rs`）

- 入口：`#[tokio::main] async fn main()`，启动 Tokio 运行时。
- 监听：`TcpListener::bind(addr).await?`；进入无限循环，持续接受连接。
- 并发：每个连接通过 `tokio::spawn(async move { ... })` 派生新任务处理。
- 读取：
  - 使用固定缓冲区 `buf = [0; 1024]` 与 `offset` 控制读入位置。
  - `socket.read(&mut buf[offset..]).await?` 返回本次读入字节数 `n`。
  - `n == 0` 表示遇到 EOF（对端关闭写端），直接返回结束任务。
- 解析与响应：
  - 尝试 `std::str::from_utf8(&buf[..end])` 把已读到的字节解析为 UTF-8 字符串。
  - 成功时，将其作为指令 `directive`，交给 `process(directive).await`。
  - `process("gettime")` 使用 `tokio::process::Command::new("date").output().await`，把 `stdout` 原样返回。
  - 失败时（UTF-8 不完整），将 `offset = end`，继续读直到可解析。

### 客户端（`src/client.rs`）

- 入口：`#[tokio::main] async fn main()`。
- 连接：`TcpStream::connect(addr).await?`。
- 写入指令：`stream.write_all(b"gettime").await?`。
- 读取响应：
  - 使用小块缓冲 `resp = [0u8; 2048]` 循环读取，累积到动态数组 `buf`。
  - 当 `buf.len() >= 28`（足够形成典型 `date` 输出）时停止；否则继续读。
  - 遇到 `n == 0`（EOF）直接 `panic!("Unexpected EOF")`。
  - `String::from_utf8(buf)?` 转成字符串后打印。

---

## 协议与数据处理

- 协议极简：客户端直接发送纯文本指令 `gettime`，服务端返回纯文本结果。
- 边界与定界：当前示例没有“消息边界”，服务端通过“能否解析为 UTF-8”来判定是否读完整；客户端通过返回内容长度“猜测”是否足够。
- 生产场景建议：
  - 增加定界规则（如 `\n` 结尾）或长度前缀（如 `u32` length + payload）。
  - 更稳定地判断 EOF 与异常；避免在客户端遇到 EOF 就 `panic!`。

---

## 错误处理与健壮性

- 示例中使用了 `unwrap()`/`expect()`，适合 demo；生产中应返回错误或记录日志。
- 服务端执行外部命令：
  - 目前只允许 `gettime`，安全性较好；不要把任意输入拼接到命令中。
  - 可以改为白名单指令集或直接用 Rust 标准库获取时间而非外部命令。
- 客户端读取策略：
  - 通过固定阈值 `28` 停止读取是“经验法”；更推荐使用定界或长度前缀协议。

---

## 并发模型

- 每连接一个任务：`tokio::spawn`。
- 任务间共享最少：每个任务独立持有 `socket` 与缓冲区。
- 注意：如果流量大，建议：
  - 增加连接超时与最大并发限制。
  - 使用 framed codec（见下文）减少手写解析逻辑。

---

## 进阶改造建议

- 使用 `tokio-util` 的 `Framed` + `LinesCodec`：
  - 服务端：按行（以 `\n` 结尾）收发字符串，避免自己维护 `offset`。
  - 客户端：以行协议收取响应，更清晰稳定。
- 指令扩展：
  - `gethostname`、`getuptime`、`getpid` 等，或返回 JSON 结构。
- 替换外部命令：
  - 用 `chrono`/`time` 获取时间，避免依赖系统命令与本地化差异。
- 错误处理：
  - 引入 `thiserror`/`anyhow`，统一错误类型与日志。
- 测试与示例：
  - 加入集成测试（`tokio::test`）启动临时 server，验证 client 行为。

---

## 常见问题与排查

- 客户端 `Unexpected EOF`：可能服务端提前关闭或网络中断；建议处理为错误返回而非 panic。
- 非 UTF-8 内容：若协议不是纯文本，需改为二进制协议并避免 `from_utf8`。
- Windows 兼容：`date` 命令不一致；建议使用 Rust 库生成时间字符串。
- 端口占用：`bind` 失败时检查是否已有进程占用端口或防火墙设置。

---

## 参考点摘抄（便于复习）

- `TcpListener::bind(addr).await?` 监听地址，`listener.accept().await?` 获取 `(socket, peer_addr)`。
- `tokio::spawn(async move { ... })` 派生异步任务，适合每连接处理。
- `AsyncReadExt::read(&mut buf).await?` 返回本次读取字节数；`n == 0` 表示 EOF。
- `AsyncWriteExt::write_all(bytes).await?` 保证把所有字节写入。
- `tokio::process::Command::new("date").output().await?` 异步执行外部命令。
- `String::from_utf8(vec)?` 与 `std::str::from_utf8(bytes)` 用于字节转字符串。

---

## License

本仓库为学习示例，未附加具体 License。若要开源，请自行补充许可条款。