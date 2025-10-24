//! An example of how to fetch from a thread pool data aggregated in thread local storage of each
//! thread

use std::{mem, thread};
use std::cell::RefCell;
use std::collections::HashSet;
use std::ops::DerefMut;
use std::sync::{Arc, Mutex};
use std::sync::RwLock;
use std::time::Duration;

use anyhow::{anyhow, Error};

use command_executor::command::Command;
use command_executor::shutdown_mode::ShutdownMode;
use command_executor::thread_pool::ThreadPool;
use command_executor::thread_pool_builder::ThreadPoolBuilder;

thread_local! {
    pub static NEXT_THREAD_POOL: RefCell<Option<Arc<RwLock<ThreadPool>>>> = const { RefCell::new(None) };
    pub static INTERMEDIATE_RESULT: RefCell<HashSet<i32>> = RefCell::new(HashSet::new());
}

struct FirstStageCommand {
    payload: i32,
}

impl FirstStageCommand {
    fn new(payload: i32) -> FirstStageCommand {
        FirstStageCommand {
            payload,
        }
    }
}

impl Command for FirstStageCommand {
    fn execute(&self) -> Result<(), Error> {
        thread::sleep(Duration::from_millis(1));
        NEXT_THREAD_POOL.with(
            |tp| {
                // Note that submit needs read access only
                tp.borrow().as_ref().unwrap().read().unwrap().submit(
                    Box::new(SecondStageCommand::new(self.payload))
                );
                Ok(())
            }
        )
    }
}

struct SecondStageCommand {
    payload: i32,
}

impl SecondStageCommand {
    fn new(payload: i32) -> SecondStageCommand {
        SecondStageCommand {
            payload,
        }
    }
}

impl Command for SecondStageCommand {
    fn execute(&self) -> Result<(), Error> {
        thread::sleep(Duration::from_millis(1));
        INTERMEDIATE_RESULT.with(
            |intermediate| {
                intermediate.borrow_mut().insert(self.payload);
                Ok(())
            }
        )
    }
}

fn create_thread_pool(name: &str, tasks: usize) -> Result<Arc<RwLock<ThreadPool>>, anyhow::Error> {
    Ok(
        Arc::new(
            RwLock::new(
                ThreadPoolBuilder::new()
                    .with_name_str(name)
                    .with_tasks(tasks)
                    .with_queue_size(4)
                    .with_shutdown_mode(ShutdownMode::CompletePending)
                    .build()?
            )
        )
    )
}

fn set_next(thread_pool: Arc<RwLock<ThreadPool>>, next: Arc<RwLock<ThreadPool>>) -> Result<(), anyhow::Error> {
    let tp = thread_pool
        .write()
        .map_err(|e| anyhow!("failed to lock tread pool: {e}"))?;
    tp.set_thread_local(&NEXT_THREAD_POOL, Some(next.clone()));
    Ok(())
}

fn shutdown(thread_pool: Arc<RwLock<ThreadPool>>) -> Result<(), anyhow::Error> {
    let mut tp = thread_pool
        .write()
        .map_err(|e| anyhow!("failed to lock tread pool: {e}"))?;
    tp.shutdown();
    tp.join()
}

fn fetch(thread_pool: Arc<RwLock<ThreadPool>>) -> Result<HashSet<i32>, anyhow::Error> {
    let result_set = Arc::new(Mutex::new(HashSet::<i32>::new()));
    let tp = thread_pool
        .write()
        .map_err(|e| anyhow!("failed to lock tread pool: {e}"))?;
    let result_set_clone = result_set.clone();
    tp.in_all_threads_mut(
        Arc::new(
            Mutex::new(
                move || {
                    INTERMEDIATE_RESULT.with(
                        |intermediate| {
                            let payload = intermediate.take();
                            let mut result_set = result_set_clone.lock().unwrap();
                            result_set.extend(payload);
                        }
                    )
                })
        )
    );
    let mut result_set = result_set.lock().unwrap();
    Ok(mem::take(result_set.deref_mut()))
}

pub fn main() -> Result<(), anyhow::Error> {
    let first_stage = create_thread_pool("first", 2)?;
    let second_stage = create_thread_pool("second", 2)?;

    set_next(first_stage.clone(), second_stage.clone())?;

    let mut source_set = HashSet::new();
    for i in 0..8192 {
        source_set.insert(i);
        let tp = first_stage
            .write()
            .map_err(|e| anyhow!("failed to lock tread pool: {e}"))?;
        tp.submit(Box::new(FirstStageCommand::new(i)))
    }

    shutdown(first_stage.clone())?;
    // First stage thread pool finished processing the last command
    let result_set = fetch(second_stage.clone())?;
    shutdown(second_stage.clone())?;

    assert_eq!(source_set, result_set);
    Ok(())
}

