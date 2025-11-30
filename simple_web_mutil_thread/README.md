
**简介**
- 该线程池以 `mpsc` 通道为任务队列，工作线程从通道中阻塞接收任务并执行。
- 并发度由固定数量的工作线程决定，提交的任务会排队等待空闲线程。
- 关闭时，通过关闭发送端让所有工作线程感知到“无更多任务”，随后逐一 `join` 保证有序退出。

**关键组件**
- `Worker`：保存线程 `JoinHandle` 与标识，创建后进入循环消费任务队列（`src/lib.rs:4–7`, `src/lib.rs:10–30`）。
- `ThreadPool`：持有 `workers` 与任务发送端 `sender`（使用 `Option` 以便在 `Drop` 中 `take` 所有权）（`src/lib.rs:33–37`）。
- `Job`：任务类型别名，`Box<dyn FnOnce() + Send + 'static>`，满足跨线程安全与一次性执行（`src/lib.rs:40`）。

**工作流程**
- 初始化：`ThreadPool::new` 创建 `mpsc::channel`，将接收端用 `Arc<Mutex<_>>` 包装并克隆给每个 `Worker`（`src/lib.rs:42–54`）。
- 工作线程：`Worker::new` 启动线程，循环 `recv`：
  - `Ok(job)`：执行 `job()`（`src/lib.rs:15–19`）。
  - `Err(_)`：说明发送端已关闭，退出循环（`src/lib.rs:20–23`）。
- 提交任务：`ThreadPool::execute` 将闭包封装为 `Job` 并通过 `sender.send(job)` 入队（`src/lib.rs:57–64`）。

**并发与队列**
- 并发度等于线程池大小：同一时刻最多有 `size` 个任务在执行。
- 任务按入队顺序被工作线程拉取；`mpsc` 的发送是非阻塞，接收是阻塞式拉取，避免忙等。

**关闭与清理**
- `Drop` 实现：
  - 关闭任务发送端：`drop(self.sender.take())`，使所有 `recv` 返回 `Err`（`src/lib.rs:72`）。
  - 打印并逐一 `join` 工作线程，确保资源不泄漏（`src/lib.rs:74–80`）。
- 使用 `Option<mpsc::Sender<Job>>` 的原因：在 `Drop` 中通过 `take()` 转移并释放发送端所有权，防止重复关闭与借用问题（`src/lib.rs:36`, `src/lib.rs:72`）。

**示例用法**
- Web 服务器中为每个连接分发处理任务到线程池（`src/main.rs:14–23`），任务内部执行 IO 与逻辑（`src/main.rs:26–51`）。
- 通过 `GET /sleep` 模拟慢请求，验证线程池在并发请求下的吞吐与隔离（`src/main.rs:36–43`）。

**关键实现片段**
- 创建与并发工作循环：`Worker::new`（`src/lib.rs:10–30`）。
- 任务提交接口：`ThreadPool::execute`（`src/lib.rs:57–64`）。
- 资源有序回收：`impl Drop for ThreadPool`（`src/lib.rs:68–83`）。

**测试要点**
- 执行完整性：50 个任务全部执行完成（`executes_all_jobs`，`src/lib.rs:110–125`）。
- 并发度上限：并发峰值等于线程数（`respects_pool_size_max_concurrency`，`src/lib.rs:128–161`）。
- 释放行为：`Drop` 等待工作线程完成并 `join`（`drop_waits_for_workers`，`src/lib.rs:164–180`）。
- 吞吐稳定性：200 个短任务全部完成（`handles_many_short_tasks`，`src/lib.rs:183–200`）。

**设计取舍与改进**
- 互斥锁包裹接收端：简单可靠，但在高负载下存在锁竞争；可改为每个 `Worker` 拥有独立 `Receiver` 通过 `clone` 创建多消费者通道或用 `crossbeam`。
- 关闭策略：当前在遍历 `workers` 时关闭 `sender`，首次迭代即可生效；可将关闭动作提前并分离，提升清晰度。
- 背压与队列控制：目前无限队列；可加入队列容量与提交端阻塞/拒绝策略，避免内存膨胀。
- 错误处理：`send`/`join` 的 `unwrap` 简化了逻辑，但在生产场景建议改为可恢复错误路径或日志上报。