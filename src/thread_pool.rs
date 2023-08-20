use std::cell::RefCell;
use std::sync::{Arc, Barrier, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{Builder, JoinHandle, LocalKey};
use std::time::Duration;

use anyhow::anyhow;

use crate::blocking_queue_adapter::BlockingQueueAdapter;
use crate::command::Command;
use crate::queue_type::QueueType;
use crate::shutdown_mode::ShutdownMode;

struct EmptyCommand {}

impl EmptyCommand {
    pub fn new() -> EmptyCommand {
        EmptyCommand {}
    }
}

impl Command for EmptyCommand {
    fn execute(&self) -> Result<(), anyhow::Error> {
        Ok(())
    }
}

struct RunInAllThreadsCommand {
    f: Arc<dyn Fn() + Send + Sync>,
    b: Arc<Barrier>,
}

impl RunInAllThreadsCommand {
    pub fn new(f: Arc<dyn Fn() + Send + Sync>, b: Arc<Barrier>) -> RunInAllThreadsCommand {
        RunInAllThreadsCommand {
            f,
            b,
        }
    }
}

impl Command for RunInAllThreadsCommand {
    fn execute(&self) -> Result<(), anyhow::Error> {
        {
            (self.f)();
        }
        self.b.wait();
        Ok(())
    }
}

struct RunMutInAllThreadsCommand {
    f: Arc<Mutex<dyn FnMut() + Send + Sync>>,
    b: Arc<Barrier>,
}

impl RunMutInAllThreadsCommand {
    pub fn new(f: Arc<Mutex<dyn FnMut() + Send + Sync>>, b: Arc<Barrier>) -> RunMutInAllThreadsCommand {
        RunMutInAllThreadsCommand {
            f,
            b,
        }
    }
}

impl Command for RunMutInAllThreadsCommand {
    fn execute(&self) -> Result<(), anyhow::Error> {
        {
            let mut f = self.f.lock().unwrap();
            f();
        }
        self.b.wait();
        Ok(())
    }
}

/// Execute tasks concurrently while maintaining bounds on memory consumption
///
/// To demonstrate the use case this implementation solves let's consider a program that reads
/// lines from a file and writes those lines to another file after some processing. The processing
/// itself is stateless and can be done in parallel on each line, but the reading and writing must
/// be sequential. Using this implementation we will read the input in the main thread, submit it
/// for concurrent processing to a processing thread pool and collect it for writing in a writing thread pool
/// with a single thread. See ./examples/read_process_write_pipeline.rs. The submission to a thread
/// pool is done through a blocking bounded queue, so if the processing thread pool or the writing
/// thread pool cannot keep up, their blocking queues will fill up and create a backpressure that
/// will pause the reading. So the resulting pipeline will stabilize on a throughput commanded by the
/// slowest stage with the memory consumption determined by sizes of queues and number of
/// threads in each thread pool.
///
/// For reference see [Command Pattern](https://en.wikipedia.org/wiki/Command_pattern) and
/// [Producer-Consumer](https://en.wikipedia.org/wiki/Producer%E2%80%93consumer_problem)
///
pub struct ThreadPool {
    name: String,
    tasks: usize,
    queue: Arc<BlockingQueueAdapter<Box<dyn Command + Send + Sync>>>,
    threads: Vec<JoinHandle<Result<(), anyhow::Error>>>,
    join_error_handler: fn(String, String),
    shutdown_mode: ShutdownMode,
    stopped: Arc<AtomicBool>,
    expired: bool,
}

impl ThreadPool {
    pub(crate) fn new(
        name: String,
        tasks: usize,
        queue_type: QueueType,
        queue_size: usize,
        join_error_handler: fn(String, String),
        shutdown_mode: ShutdownMode,
    ) -> Result<ThreadPool, anyhow::Error> {
        let start_barrier = Arc::new(Barrier::new(tasks + 1));
        let stopped = Arc::new(AtomicBool::new(false));
        let mut threads = Vec::<JoinHandle<Result<(), anyhow::Error>>>::new();
        let queue = Arc::new(BlockingQueueAdapter::new(queue_type, queue_size));
        for i in 0..tasks {
            let barrier = start_barrier.clone();
            let t = Self::create_thread(
                &name,
                i,
                barrier,
                queue.clone(),
                stopped.clone(),
            );
            threads.push(t.unwrap());
        }

        start_barrier.wait();

        Ok(
            ThreadPool {
                name,
                tasks,
                queue: queue.clone(),
                threads,
                join_error_handler,
                shutdown_mode,
                stopped: stopped.clone(),
                expired: false,
            }
        )
    }

    /// Get the number of concurrent threads in the thread pool
    pub fn tasks(&self) -> usize {
        self.tasks
    }

    fn create_thread(
        name: &String,
        index: usize,
        barrier: Arc<Barrier>,
        queue: Arc<BlockingQueueAdapter<Box<dyn Command + Send + Sync>>>,
        stopped: Arc<AtomicBool>,
    ) -> Result<JoinHandle<Result<(), anyhow::Error>>, anyhow::Error> {
        let builder = Builder::new();
        Ok(builder
            .name(format!("{name}-{index}"))
            .spawn(move || {
                barrier.wait();
                let mut r: Result<(), anyhow::Error> = Ok(());
                while !stopped.load(Ordering::SeqCst) {
                    let command = queue.dequeue();
                    if let Some(c) = command {
                        match c.execute() {
                            Ok(_) => {}
                            Err(e) => {
                                r = Err(e);
                            }
                        }
                    }
                }
                r
            }
            )?
        )
    }

    /// Execute f in all threads.
    ///
    /// This function returns only after f had completed in all threads. Can be used to collect
    /// data produced by the threads. See ./examples/fetch_thread_local.rs.
    ///
    /// Caveat: this is a [barrier](https://en.wikipedia.org/wiki/Barrier_%28computer_science%29)
    /// function. So if one of the threads is busy with a long running task or is deadlocked, this
    /// will halt all the threads until f can be executed.
    pub fn in_all_threads_mut(&self, f: Arc<Mutex<dyn FnMut() + Send + Sync>>) {
        let b = Arc::new(Barrier::new(self.tasks + 1));
        for _i in 0..self.tasks {
            self.submit(Box::new(RunMutInAllThreadsCommand::new(f.clone(), b.clone())));
        }
        b.wait();
    }

    /// Execute f in all threads.
    ///
    /// This function returns only after f had completed in all threads. Can be used to flush
    /// data produced by the threads or simply execute work concurrently. See ./examples/flush_thread_local.rs.
    ///
    /// Caveat: this is a [barrier](https://en.wikipedia.org/wiki/Barrier_%28computer_science%29)
    /// function. So if one of the threads is busy with a long running task or is deadlocked, this
    /// will halt all the threads until f can be executed.
    pub fn in_all_threads(&self, f: Arc<dyn Fn() + Send + Sync>) {
        let b = Arc::new(Barrier::new(self.tasks + 1));
        for _i in 0..self.tasks {
            self.submit(Box::new(RunInAllThreadsCommand::new(f.clone(), b.clone())));
        }
        b.wait();
    }

    /// Initializes the `local_key` to contain `val`.
    ///
    /// See ./examples/thread_local.rs
    pub fn set_thread_local<T>(&self, local_key: &'static LocalKey<RefCell<T>>, val: T)
        where T: Sync + Send + Clone {
        self.in_all_threads(
            Arc::new(
                move || {
                    local_key.with(
                        |value| {
                            value.replace(val.clone())
                        }
                    );
                }
            )
        );
    }

    /// Shut down the thread pool.
    ///
    /// This will shut down the thread pool according to configuration. When configured with
    /// * [ShutdownMode::Immediate] - terminate each tread after completing the current task
    /// * [ShutdownMode::CompletePending] - terminate after completing all pending tasks
    pub fn shutdown(&mut self) {
        self.expired = true;
        match self.shutdown_mode {
            ShutdownMode::Immediate => {
                self.stopped.store(true, Ordering::SeqCst);
            }
            ShutdownMode::CompletePending => {
                self.queue.wait_empty(Duration::MAX);
                self.stopped.store(true, Ordering::SeqCst);
            }
        }
        for _i in 0..self.tasks {
            self.unchecked_submit(Box::new(EmptyCommand::new()));
        }
    }

    /// Wait until all thread pool threads completed.
    pub fn join(&mut self) -> Result<(), anyhow::Error> {
        let mut join_errors = Vec::<String>::new();
        while let Some(t) = self.threads.pop() {
            let name = t.thread().name().unwrap_or("unnamed").to_string();
            match t.join() {
                Ok(r) => {
                    match r {
                        Ok(_) => {}
                        Err(e) => {
                            let message = format!("{e:?}");
                            join_errors.push(message.clone());
                            (self.join_error_handler)(name, message);
                        }
                    }
                }
                Err(e) => {
                    let mut message = "Unknown error".to_string();
                    if let Some(error) = e.downcast_ref::<&'static str>() {
                        message = error.to_string();
                    }
                    join_errors.push(message.clone());
                    (self.join_error_handler)(name, message);
                }
            }
        }
        if join_errors.is_empty() {
            Ok(())
        } else {
            Err(anyhow!("Errors occurred while joining threads in the {} pool: {}", self.name, join_errors.join(", "))
            )
        }
    }

    /// Submit command for execution
    pub fn submit(&self, command: Box<dyn Command + Send + Sync>) {
        self.try_submit(command, Duration::MAX);
    }

    pub fn unchecked_submit(&self, command: Box<dyn Command + Send + Sync>) {
        self.queue.enqueue(command);
    }

    /// Submit command for execution with timeout
    ///
    /// Returns the command on failure and None on success
    pub fn try_submit(&self, command: Box<dyn Command + Send + Sync>, timeout: Duration) -> Option<Box<dyn Command + Send + Sync>> {
        assert!(!self.expired);
        self.queue.try_enqueue(command, timeout)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use crate::shutdown_mode::ShutdownMode;
    use crate::shutdown_mode::ShutdownMode::CompletePending;
    use crate::thread_pool_builder::ThreadPoolBuilder;

    use super::*;

    struct TestCommand {
        _payload: i32,
        execution_counter: Arc<AtomicUsize>,
    }

    impl TestCommand {
        pub fn new(payload: i32, execution_counter: Arc<AtomicUsize>) -> TestCommand {
            TestCommand {
                _payload: payload,
                execution_counter,
            }
        }
    }

    impl Command for TestCommand {
        fn execute(&self) -> Result<(), anyhow::Error> {
            self.execution_counter.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    #[test]
    fn test_create() {
        let mut thread_pool_builder = ThreadPoolBuilder::new();
        let tp_result = thread_pool_builder
            .with_name("t".to_string())
            .with_tasks(4)
            .with_queue_size(8)
            .build();

        match tp_result {
            Ok(mut tp) => {
                assert!(true);
                tp.shutdown();
                assert_eq!((), tp.join().unwrap());
            }
            Err(_) => {
                assert!(false);
            }
        }
    }

    #[test]
    fn test_submit() {
        let mut thread_pool_builder = ThreadPoolBuilder::new();
        let mut tp = thread_pool_builder
            .with_name("t".to_string())
            .with_tasks(4)
            .with_queue_size(2048)
            .build()
            .unwrap();

        let execution_counter = Arc::new(AtomicUsize::from(0));
        for _i in 0..1024 {
            let ec = execution_counter.clone();
            tp.submit(Box::new(TestCommand::new(4, ec)));
        }

        tp.shutdown();
        tp.join().expect("Failed to join thread pool");
        assert_eq!((), tp.join().unwrap());
        // accidental but usually works
        // if fails safe to comment out the next two lines
        // assert!(execution_counter.fetch_or(0, Ordering::SeqCst) > 0);
        // assert!(execution_counter.fetch_or(0, Ordering::SeqCst) < 1024);
    }

    #[test]
    fn test_shutdown_complete_pending() {
        let mut thread_pool_builder = ThreadPoolBuilder::new();
        let mut tp = thread_pool_builder
            .with_name("t".to_string())
            .with_tasks(4)
            .with_queue_size(2048)
            .with_shutdown_mode(ShutdownMode::CompletePending)
            .build()
            .unwrap();

        let execution_counter = Arc::new(AtomicUsize::from(0));
        for _i in 0..1024 {
            let ec = execution_counter.clone();
            tp.submit(Box::new(TestCommand::new(4, ec)));
        }

        tp.shutdown();
        tp.join().expect("Failed to join thread pool");
        assert_eq!((), tp.join().unwrap());
        assert_eq!(execution_counter.fetch_or(0, Ordering::SeqCst), 1024);
    }

    struct PanicTestCommand {}

    impl PanicTestCommand {
        pub fn new() -> PanicTestCommand {
            PanicTestCommand {}
        }
    }

    impl Command for PanicTestCommand {
        fn execute(&self) -> Result<(), anyhow::Error> {
            Err(anyhow!("simulating error during command execution"))
        }
    }

    #[test]
    fn test_join_error_handler() {
        let mut thread_pool_builder = ThreadPoolBuilder::new();
        let mut tp = thread_pool_builder
            .with_name("t".to_string())
            .with_tasks(4)
            .with_shutdown_mode(CompletePending)
            .with_queue_size(8)
            .with_join_error_handler(
                |name, message| {
                    println!("Thread {name} ended with and error {message}")
                }
            )
            .build()
            .unwrap();

        for _i in 0..2 {
            tp.submit(Box::new(PanicTestCommand::new()));
        }

        tp.shutdown();
        let r = tp.join();
        assert!(r.is_err());
    }

    #[test]
    #[should_panic]
    fn test_use_after_join() {
        let mut thread_pool_builder = ThreadPoolBuilder::new();
        let mut tp = thread_pool_builder
            .with_name("t".to_string())
            .with_tasks(4)
            .with_queue_size(2048)
            .with_shutdown_mode(ShutdownMode::CompletePending)
            .build()
            .unwrap();

        let execution_counter = Arc::new(AtomicUsize::from(0));
        for _i in 0..1024 {
            let ec = execution_counter.clone();
            tp.submit(Box::new(TestCommand::new(4, ec)));
        }

        tp.shutdown();
        tp.join().expect("Failed to join thread pool");
        let execution_counter = Arc::new(AtomicUsize::from(0));
        for _i in 0..1024 {
            let ec = execution_counter.clone();
            tp.submit(Box::new(TestCommand::new(4, ec)));
        }
    }
}
