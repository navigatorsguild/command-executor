use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::{thread};
use std::time::{Duration, SystemTime};
use std::cell::{RefCell};
use std::fs::{File, remove_file};
use std::io::{BufRead, BufReader, Write};
use std::ops::AddAssign;
use std::path::PathBuf;
use command_executor::command::Command;
use command_executor::shutdown_mode::ShutdownMode;
use command_executor::thread_pool_builder::ThreadPoolBuilder;

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
    fn execute(&self) -> Result<(), anyhow::Error> {
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
    run(1, 4, 10, 256);
    let e1 = t1.elapsed().unwrap().as_millis() as f64;

    let t2 = SystemTime::now();
    run(2, 4, 10, 256);
    let e2 = t2.elapsed().unwrap().as_millis() as f64;

    let t4 = SystemTime::now();
    run(4, 4, 10, 256);
    let e4 = t4.elapsed().unwrap().as_millis() as f64;

    assert!(e1 / e2 > 1.8_f64);
    assert!(e2 / e4 > 1.8_f64);
}

thread_local! {
    pub static THREAD_LOCAL_FILE: RefCell<Option<File>> = RefCell::new(None);
}

struct Store {
    i: usize,
}

impl Store {
    pub fn new(i: usize) -> Store {
        Store {
            i,
        }
    }
}

impl Command for Store {
    fn execute(&self) -> Result<(), anyhow::Error> {
        THREAD_LOCAL_FILE.with(
            |tlf| {
                let mut f = tlf.replace(None).unwrap();
                f.write(format!("{}\n", self.i).as_bytes()).expect("Failed writing a number to test file");
                tlf.replace(Some(f))
            }
        );
        Ok(())
    }
}

#[test]
fn test_in_all_threads_mut() {
    let mut thread_pool_builder = ThreadPoolBuilder::new();
    let mut tp = thread_pool_builder
        .name("thread-local-file".to_string())
        .tasks(4)
        .queue_size(2048)
        .shutdown_mode(ShutdownMode::CompletePending)
        .build()
        .unwrap();

    for i in 0..tp.tasks() {
        let name = format!("thread-local-file-{i}");
        let path = PathBuf::from(format!("./target/{name}"));
        if path.exists() {
            remove_file(path).expect("Filed to remove test file path");
        }
    }

    tp.in_all_threads_mut(
        Arc::new(
            Mutex::new(
                move || {
                    THREAD_LOCAL_FILE.with(
                        |tlf| {
                            let name = thread::current().name().unwrap().to_string();
                            let path = PathBuf::from(format!("./target/{name}"));
                            tlf.replace(
                                Some(
                                    File::create(path).unwrap()
                                )
                            );
                        }
                    );
                }
            )
        )
    );

    for i in 0..1024 {
        tp.submit(Box::new(Store::new(i)));
    }

    tp.shutdown();
    let mut total: usize = 0;
    for i in 0..tp.tasks() {
        let name = format!("thread-local-file-{i}");
        let path = PathBuf::from(format!("./target/{name}"));
        let f = File::open(path).unwrap();
        total.add_assign(BufReader::new(f).lines().count());
    }
    assert_eq!((), tp.join().unwrap());
    assert_eq!(total, 1024);
}

#[test]
fn test_in_all_threads() {
    let mut thread_pool_builder = ThreadPoolBuilder::new();
    let mut tp = thread_pool_builder
        .name("thread-local-file".to_string())
        .tasks(4)
        .queue_size(2048)
        .shutdown_mode(ShutdownMode::CompletePending)
        .build()
        .unwrap();

    tp.in_all_threads(
        Arc::new(
            move || {}
        )
    );
    tp.shutdown();
}
