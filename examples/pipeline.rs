//! An example of how to build a three stage pipeline

use std::cell::RefCell;
use std::collections::HashSet;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::RwLock;

use anyhow::{anyhow, Context, Error};

use command_executor::command::Command;
use command_executor::shutdown_mode::ShutdownMode;
use command_executor::thread_pool::ThreadPool;
use command_executor::thread_pool_builder::ThreadPoolBuilder;

thread_local! {
    pub static NEXT_THREAD_POOL: RefCell<Option<Arc<RwLock<ThreadPool>>>> = RefCell::new(None);
    pub static RESULT_FILE: RefCell<Option<File>> = RefCell::new(None);
}

static RESULT_FILE_PATH: &str = "./target/pipeline-example-result";

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
        NEXT_THREAD_POOL.with(
            |tp| {
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
        NEXT_THREAD_POOL.with(
            |tp| {
                tp.borrow().as_ref().unwrap().read().unwrap().submit(
                    Box::new(ThirdStageCommand::new(self.payload))
                );
                Ok(())
            }
        )
    }
}

struct ThirdStageCommand {
    payload: i32,
}

impl ThirdStageCommand {
    fn new(payload: i32) -> ThirdStageCommand {
        ThirdStageCommand {
            payload,
        }
    }
}

impl Command for ThirdStageCommand {
    fn execute(&self) -> Result<(), Error> {
        RESULT_FILE.with(
            |cell| {
                let mut file_opt = cell.borrow_mut();
                match file_opt.as_mut() {
                    None => {
                        let mut f = File::create(PathBuf::from(RESULT_FILE_PATH))
                            .with_context(|| anyhow!("path: {}", RESULT_FILE_PATH))?;
                        writeln!(f, "{}", self.payload)
                            .with_context(|| anyhow!("path: {}", RESULT_FILE_PATH))?;
                        file_opt.replace(f);
                        Ok(())
                    }
                    Some(f) => {
                        writeln!(f, "{}", self.payload)
                            .with_context(|| anyhow!("path: {}", RESULT_FILE_PATH))
                    }
                }
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

pub fn main() -> Result<(), anyhow::Error> {
    let first_stage = create_thread_pool("first", 2)?;
    let second_stage = create_thread_pool("second", 2)?;
    let third_stage = create_thread_pool("third", 1)?;

    set_next(first_stage.clone(), second_stage.clone())?;
    set_next(second_stage.clone(), third_stage.clone())?;

    let mut source_set = HashSet::new();
    for i in 0..256 {
        source_set.insert(i);
        let tp = first_stage
            .write()
            .map_err(|e| anyhow!("failed to lock tread pool: {e}"))?;
        tp.submit(Box::new(FirstStageCommand::new(i)))
    }

    shutdown(first_stage.clone())?;
    // First stage thread pool finished processing the last command
    shutdown(second_stage.clone())?;
    // Second stage thread pool finished processing the last command
    shutdown(third_stage.clone())?;
    // Third stage thread pool finished processing the last command

    let mut result_set = HashSet::new();
    let reader = BufReader::new(File::open(PathBuf::from(RESULT_FILE_PATH))?);
    for line_result in reader.lines() {
        let line = line_result?;
        let i = i32::from_str(line.as_str())?;
        result_set.insert(i);
    }
    assert_eq!(source_set, result_set);
    std::fs::remove_file(PathBuf::from(RESULT_FILE_PATH))?;
    Ok(())
}