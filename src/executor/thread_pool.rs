use std::thread::{Builder, JoinHandle};
use std::sync::{Arc, Barrier};
use std::error::Error;
use std::time::Duration;
use crate::errors::GenericError;
use crate::executor::blocking_queue::BlockingQueue;
use crate::executor::command::Command;
use crate::executor::shutdown_mode::ShutdownMode;
use crate::executor::signal::Signal;
use crate::executor::signal::Signal::Shutdown;
use crate::executor::signal::Signal::Reload;

pub struct ThreadPool {
    name: String,
    queue: Arc<BlockingQueue<Box<dyn Command + Send + Sync>, Signal>>,
    threads: Vec<JoinHandle<Result<(), GenericError>>>,
    join_error_handler: fn(String, String),
    shutdown_mode: ShutdownMode,
}

impl ThreadPool {
    pub(crate) fn new(name: String, tasks: usize, queue_size: usize, join_error_handler: fn(String, String), shutdown_mode: ShutdownMode) -> Result<ThreadPool, GenericError> {
        let start_barrier = Arc::new(Barrier::new(tasks));
        let mut threads = Vec::<JoinHandle<Result<(), GenericError>>>::new();
        let queue = Arc::new(BlockingQueue::<Box<dyn Command + Send + Sync>, Signal>::new(queue_size));
        for i in 0..tasks {
            let barrier = start_barrier.clone();
            let builder = std::thread::Builder::new();
            let t = Self::create_thread(
                name.clone(),
                i,
                barrier,
                queue.clone(),
                builder);
            threads.push(t.unwrap());
        }

        Ok(
            ThreadPool {
                name,
                queue: queue.clone(),
                threads,
                join_error_handler,
                shutdown_mode,
            }
        )
    }

    fn create_thread(
        name: String,
        index: usize,
        barrier: Arc<Barrier>,
        queue: Arc<BlockingQueue<Box<dyn Command + Send + Sync>, Signal>>,
        builder: Builder,
    ) -> Result<JoinHandle<Result<(), Box<dyn Error + Send + Sync>>>, GenericError> {
        Ok(
            builder
                .name(format!("{name}-{index}"))
                .spawn(move || {
                    barrier.wait();
                    let mut r: Result<(), Box<dyn Error + Send + Sync>> = Ok(());
                    loop {
                        let (command, signal) = queue.dequeue();
                        if let Some(c) = command {
                            match c.execute() {
                                Ok(_) => {}
                                Err(e) => {
                                    r = Err(e);
                                }
                            }
                        }
                        if let Some(s) = signal {
                            match s {
                                Shutdown => {
                                    break r;
                                }
                                Reload => {}
                            }
                        }
                    }
                }
                )?
        )
    }

    pub fn shutdown(&self) {
        match self.shutdown_mode {
            ShutdownMode::Immediate => {
                self.queue.signal(Shutdown);
            }
            ShutdownMode::CompletePending => {
                self.queue.wait_empty(Duration::MAX);
                self.queue.signal(Shutdown);
            }
        }
    }

    pub fn join(&mut self) -> Result<(), GenericError> {
        let mut join_errors = Vec::<String>::new();
        while self.threads.len() > 0 {
            let t = self.threads.pop().unwrap();
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
            Err(GenericError::from(
                format!("Errors occurred while joining threads in the {} pool: {}", self.name, join_errors.join(", ")))
            )
        }
    }

    pub fn submit(&self, command: Box<dyn Command + Send + Sync>) {
        self.try_submit(command, Duration::MAX);
    }

    pub fn try_submit(&self, command: Box<dyn Command + Send + Sync>, timeout: Duration) -> Option<Box<dyn Command + Send + Sync>> {
        self.queue.try_enqueue(command, timeout)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use crate::executor::shutdown_mode::ShutdownMode;
    use crate::executor::shutdown_mode::ShutdownMode::CompletePending;
    use crate::executor::thread_pool_builder::ThreadPoolBuilder;
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
        fn execute(&self) -> Result<(), GenericError> {
            self.execution_counter.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }
    }


    #[test]
    fn test_create() {
        let mut thread_pool_builder = ThreadPoolBuilder::new();
        let tp_result = thread_pool_builder
            .name("t".to_string())
            .tasks(4)
            .queue_size(8)
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
            .name("t".to_string())
            .tasks(4)
            .queue_size(2048)
            .build()
            .unwrap();

        let execution_counter = Arc::new(AtomicUsize::from(0));
        for _i in 0..1024 {
            let ec = execution_counter.clone();
            tp.submit(Box::new(TestCommand::new(4, ec)));
        }

        tp.shutdown();
        assert_eq!((), tp.join().unwrap());
        // accidental but usually works
        // if fails safe to comment out the test_shutdown_when_empty tests superset of this test
        // assert!(execution_counter.fetch_or(0, Ordering::Relaxed) > 0);
        // assert!(execution_counter.fetch_or(0, Ordering::Relaxed) < 1024);
    }

    #[test]
    fn test_shutdown_complete_pending() {
        let mut thread_pool_builder = ThreadPoolBuilder::new();
        let mut tp = thread_pool_builder
            .name("t".to_string())
            .tasks(4)
            .queue_size(2048)
            .shutdown_mode(ShutdownMode::CompletePending)
            .build()
            .unwrap();

        let execution_counter = Arc::new(AtomicUsize::from(0));
        for _i in 0..1024 {
            let ec = execution_counter.clone();
            tp.submit(Box::new(TestCommand::new(4, ec)));
        }

        tp.shutdown();
        assert_eq!((), tp.join().unwrap());
        assert_eq!(execution_counter.fetch_or(0, Ordering::Relaxed), 1024);
    }

    struct PanicTestCommand {}

    impl PanicTestCommand {
        pub fn new() -> PanicTestCommand {
            PanicTestCommand {}
        }
    }

    impl Command for PanicTestCommand {
        fn execute(&self) -> Result<(), GenericError> {
            Err(GenericError::from("simulating error during command execution"))
        }
    }

    #[test]
    fn test_join_error_handler() {
        let mut thread_pool_builder = ThreadPoolBuilder::new();
        let mut tp = thread_pool_builder
            .name("t".to_string())
            .tasks(4)
            .shutdown_mode(CompletePending)
            .queue_size(8)
            .join_error_handler(
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
}
