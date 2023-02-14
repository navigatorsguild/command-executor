use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, SystemTime};
use command_executor::errors::GenericError;
use command_executor::executor::command::Command;
use command_executor::executor::shutdown_mode::ShutdownMode;
use command_executor::executor::thread_pool_builder::ThreadPoolBuilder;

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
fn test_thread_pool() {
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

struct SleepyCommand {
    sleep_time: u64,
    execution_counter: Arc<AtomicUsize>,
}

impl SleepyCommand {
    pub fn new(sleep_time: u64, execution_counter: Arc<AtomicUsize>) -> SleepyCommand {
        SleepyCommand {
            sleep_time,
            execution_counter,
        }
    }
}

impl Command for SleepyCommand {
    fn execute(&self) -> Result<(), GenericError> {
        thread::sleep(Duration::from_millis(self.sleep_time));
        self.execution_counter.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
}

fn run(tasks: usize, queue_size: usize, sleep_time: u64, command_count: usize) {
    let mut thread_pool_builder = ThreadPoolBuilder::new();
    let mut tp = thread_pool_builder
        .name("t".to_string())
        .tasks(tasks)
        .queue_size(queue_size)
        .shutdown_mode(ShutdownMode::CompletePending)
        .build()
        .unwrap();

    let execution_counter = Arc::new(AtomicUsize::from(0));
    for _i in 0..command_count {
        let ec = execution_counter.clone();
        tp.submit(Box::new(SleepyCommand::new(sleep_time, ec)));
    }

    tp.shutdown();
    assert_eq!((), tp.join().unwrap());
    assert_eq!(execution_counter.fetch_or(0, Ordering::Relaxed), command_count);
}


#[test]
fn test_concurrency() {
    let t1 = SystemTime::now();
    run(1, 4, 1, 256);
    let e1 = t1.elapsed().unwrap().as_millis() as f64;

    let t2 = SystemTime::now();
    run(2, 4, 1, 256);
    let e2 = t2.elapsed().unwrap().as_millis() as f64;

    let t4 = SystemTime::now();
    run(4, 4, 1, 256);
    let e4 = t4.elapsed().unwrap().as_millis() as f64;

    assert!(e1 / e2 > 1.9_f64);
    assert!(e2 / e4 > 1.9_f64);
}