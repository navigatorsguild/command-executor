//! An example of how to build a three stage pipeline

use std::cell::RefCell;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::io::Read;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::RwLock;

use anyhow::{anyhow, Context, Error};
use sha1::{Digest, Sha1};

use command_executor::command::Command;
use command_executor::shutdown_mode::ShutdownMode;
use command_executor::thread_pool::ThreadPool;
use command_executor::thread_pool_builder::ThreadPoolBuilder;

thread_local! {
    pub static NEXT_THREAD_POOL: RefCell<Option<Arc<RwLock<ThreadPool>>>> = const { RefCell::new(None) };
    pub static RESULT_FILE: RefCell<Option<File>> = const { RefCell::new(None) };
}

static RESULT_FILE_PATH: &str = "./target/read-process-write-example-result";
static SOURCE_FILE_PATH: &str = "./target/10-million-password-list-top-1000000.txt";

struct ProcessingCommand {
    payload: String,
}

impl ProcessingCommand {
    fn new(payload: String) -> ProcessingCommand {
        ProcessingCommand {
            payload,
        }
    }
}

impl Command for ProcessingCommand {
    fn execute(&self) -> Result<(), Error> {
        NEXT_THREAD_POOL.with(
            |tp| {
                let mut hasher = Sha1::new();
                hasher.update(&self.payload);
                let result = hasher.finalize();
                let hash = hex::encode(result);
                tp.borrow().as_ref().unwrap().read().unwrap().submit(
                    Box::new(WritingCommand::new(self.payload.clone(), hash)));
                Ok(())
            }
        )
    }
}

struct WritingCommand {
    payload: String,
    hash: String,
}

impl WritingCommand {
    fn new(payload: String, hash: String) -> WritingCommand {
        WritingCommand {
            payload,
            hash,
        }
    }
}

impl Command for WritingCommand {
    fn execute(&self) -> Result<(), Error> {
        RESULT_FILE.with(
            |cell| {
                let mut file_opt = cell.borrow_mut();
                match file_opt.as_mut() {
                    None => {
                        let mut f = File::create(PathBuf::from(RESULT_FILE_PATH))
                            .with_context(|| anyhow!("path: {}", RESULT_FILE_PATH))?;
                        writeln!(f, "{}\t{}", self.hash, self.payload)
                            .with_context(|| anyhow!("path: {}", RESULT_FILE_PATH))?;
                        file_opt.replace(f);
                        Ok(())
                    }
                    Some(f) => {
                        writeln!(f, "{}\t{}", self.hash, self.payload)
                            .with_context(|| anyhow!("path: {}", RESULT_FILE_PATH))
                    }
                }
            }
        )
    }
}

fn create_thread_pool(name: &str, tasks: usize, queue_size: usize) -> Result<Arc<RwLock<ThreadPool>>, anyhow::Error> {
    Ok(
        Arc::new(
            RwLock::new(
                ThreadPoolBuilder::new()
                    .with_name_str(name)
                    .with_tasks(tasks)
                    .with_queue_size(queue_size)
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

fn fetch_file(url: &str, output: PathBuf) -> Result<(), Error> {
    if !output.exists() {
        println!("Downloading file: {} -> {:?}", url, output);
        let mut response = reqwest::blocking::get(url)
            .with_context(|| anyhow!("Failed to download the file from {:?}", url)
            )?;
        let mut body = Vec::new();
        response.read_to_end(&mut body)
            .with_context(|| anyhow!("Failed to read the file from {:?}", url)
            )?;
        let mut file = File::create(&output)
            .with_context(|| anyhow!("Failed to create the file: {:?}", output)
            )?;

        file.write(body.as_slice())
            .with_context(|| anyhow!("failed to write content to {:?}", output)
            )?;
    } else {
        println!("File exists at {:?}, skipping download", output);
    }
    Ok(())
}

fn read(processing_stage: Arc<RwLock<ThreadPool>>) -> Result<(), anyhow::Error> {
    fetch_file(
        "https://github.com/danielmiessler/SecLists/raw/master/Passwords/Common-Credentials/10-million-password-list-top-1000000.txt",
        PathBuf::from(SOURCE_FILE_PATH),
    )?;
    let f = File::open(PathBuf::from(SOURCE_FILE_PATH))
        .with_context(|| anyhow!("{}", SOURCE_FILE_PATH))?;
    let reader = BufReader::new(f);
    let tp = processing_stage
        .read()
        .map_err(|e| anyhow!("failed to lock tread pool: {e}"))?;
    for line_result in reader.lines() {
        let line = line_result?;
        tp.submit(Box::new(ProcessingCommand::new(line)))
    }
    Ok(())
}

pub fn main() -> Result<(), anyhow::Error> {
    let processing_stage = create_thread_pool("processing", 8, 1000)?;
    let writing_stage = create_thread_pool("writing", 1, 1000)?;

    set_next(processing_stage.clone(), writing_stage.clone())?;

    read(processing_stage.clone())?;

    shutdown(processing_stage.clone())?;
    shutdown(writing_stage.clone())?;

    Ok(())
}