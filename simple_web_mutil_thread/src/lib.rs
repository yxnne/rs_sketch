use std::{sync::{Arc, Mutex, mpsc}, thread};


struct Worker {
    id: usize,
    thread: Option<thread::JoinHandle<()>>,
}

impl Worker {
    fn new(id: usize, receiver: Arc<Mutex<mpsc::Receiver<Job>>>) -> Worker {
        let thread = thread::spawn(move || loop {

            let message = receiver.lock().unwrap().recv();

            match message {
                Ok(job) => {
                    println!("Worker {id} got a job; executing.");
                    job();
                }
                Err(_) => {
                    println!("Worker {id} got a job; shutting down.");
                    break;
                }
            }
        });
        Worker { 
            id, 
            thread: Some(thread) 
        }
    }
}

pub struct ThreadPool {
    workers: Vec<Worker>,
    // sender: mpsc::Sender<Job>,
    sender: Option<mpsc::Sender<Job>>,
}
// struct Job;

type Job = Box<dyn FnOnce() + Send + 'static>;

impl ThreadPool {
    pub fn new(size: usize) -> ThreadPool {
        assert!(size > 0);

        let (sender, receiver) = mpsc:: channel();
        let receiver = Arc::new(Mutex::new(receiver));
        let mut workers = Vec::with_capacity(size);
        for id in 0..size {
            // threads.push(thread::spawn(f));
            workers.push(Worker::new(id, Arc::clone(&receiver)));
        }
        ThreadPool { workers, sender: Some(sender) }
    }


    pub fn execute<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let job = Box::new(f);
        // self.sender.send(job).unwrap();
        self.sender.as_ref().unwrap().send(job).unwrap();
    }
}


impl Drop for ThreadPool {
    fn drop(&mut self) {
        for worker in &mut self.workers {

            drop(self.sender.take());

            println!("Shutting down worker {}.", worker.id);

            // 既然换了 Option，就可以用 take 拿走所有权
            if let Some(thread) = worker.thread.take() {
                thread.join().unwrap();
            }
        }
    }

}


// 测试代码
#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;
    use std::time::Duration;
    use std::sync::{mpsc, Arc, atomic::{AtomicUsize, Ordering}};
    
    #[test]
    fn test_thread_pool() {
        let pool = ThreadPool::new(4);
        for i in 0..10 {
            pool.execute(move || {
                println!("Thread {} is running.", i);
            });
        }
        sleep(Duration::from_secs(1));



    }

    #[test]
    fn executes_all_jobs() {
        // 验证：所有提交的任务都能被执行并完成
        let pool = ThreadPool::new(4);
        let (tx, rx) = mpsc::channel(); // 关键：用通道收集任务完成信号
        let total = 50;
        for _ in 0..total {
            let tx = tx.clone(); // 关键：为每个任务克隆发送端
            pool.execute(move || {
                tx.send(()).unwrap(); // 关键行：任务完成后发送信号
            });
        }
        drop(tx); // 关键：关闭原始发送端，避免阻塞接收循环
        for _ in 0..total {
            rx.recv().unwrap(); // 关键行：阻塞等待所有任务完成
        }
    }

    #[test]
    fn respects_pool_size_max_concurrency() {
        // 验证：最大并行度不超过线程池大小
        let size = 4;
        let pool = ThreadPool::new(size);
        let active = Arc::new(AtomicUsize::new(0)); // 关键：当前并发计数
        let max = Arc::new(AtomicUsize::new(0)); // 关键：并发峰值
        let (tx, rx) = mpsc::channel();
        let total = 32;
        for _ in 0..total {
            let active = Arc::clone(&active);
            let max = Arc::clone(&max);
            let tx = tx.clone();
            pool.execute(move || {
                let cur = active.fetch_add(1, Ordering::SeqCst) + 1; // 关键行：进入任务递增并发数
                // 关键：用 CAS 更新并发峰值，确保并发安全
                loop {
                    let prev = max.load(Ordering::SeqCst);
                    if cur > prev {
                        if max.compare_exchange(prev, cur, Ordering::SeqCst, Ordering::SeqCst).is_ok() {
                            break;
                        }
                    } else {
                        break;
                    }
                }
                sleep(Duration::from_millis(100)); // 关键：制造任务重叠以测峰值
                active.fetch_sub(1, Ordering::SeqCst); // 关键行：退出任务递减并发数
                tx.send(()).unwrap();
            });
        }
        drop(tx);
        for _ in 0..total { rx.recv().unwrap(); }
        assert_eq!(max.load(Ordering::SeqCst), size); // 关键断言：峰值等于线程数
    }

    #[test]
    fn drop_waits_for_workers() {
        // 验证：线程池在 Drop 时会等待工作线程完成并 join
        let size = 4;
        let (tx, rx) = mpsc::channel();
        {
            let pool = ThreadPool::new(size);
            for _ in 0..size {
                let tx = tx.clone();
                pool.execute(move || {
                    sleep(Duration::from_millis(100));
                    tx.send(()).unwrap(); // 关键行：任务完成后通知
                });
            }
        } // 关键：离开作用域触发 Drop，关闭 sender 并 join 所有工作线程
        drop(tx);
        for _ in 0..size { rx.recv().unwrap(); } // 关键行：若未等待完成，此处会阻塞
    }

    #[test]
    fn handles_many_short_tasks() {
        // 验证：大量短任务在高吞吐下仍能全部完成
        let pool = ThreadPool::new(4);
        let count = Arc::new(AtomicUsize::new(0)); // 关键：原子计数器统计完成任务数
        let (tx, rx) = mpsc::channel();
        let total = 200;
        for _ in 0..total {
            let c = Arc::clone(&count);
            let tx = tx.clone();
            pool.execute(move || {
                c.fetch_add(1, Ordering::Relaxed); // 关键行：任务完成计数
                tx.send(()).unwrap();
            });
        }
        drop(tx);
        for _ in 0..total { rx.recv().unwrap(); }
        assert_eq!(count.load(Ordering::Relaxed), total); // 关键断言：完成数匹配任务数
    }



  
}
