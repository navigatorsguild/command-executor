use std::cell::RefCell;
use std::thread;
use std::time::Duration;

use command_executor::command::Command;
use command_executor::shutdown_mode::ShutdownMode;
use command_executor::thread_pool_builder::ThreadPoolBuilder;

#[derive(Clone)]
struct Config {
    sleep_time: u64,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            sleep_time: 1,
        }
    }
}

thread_local! {
    static THREAD_LOCAL_CONFIG: RefCell<Option<Config>> = RefCell::new(None);
}

struct ThreadLocalExampleCommand {
    payload: i32,
}

impl ThreadLocalExampleCommand {
    pub fn new(payload: i32) -> ThreadLocalExampleCommand {
        ThreadLocalExampleCommand { payload }
    }
}

impl Command for ThreadLocalExampleCommand {
    fn execute(&self) -> Result<(), anyhow::Error> {
        THREAD_LOCAL_CONFIG.with(
            |config| {
                let sleep_time = config.borrow().as_ref().unwrap().sleep_time;
                thread::sleep(Duration::from_millis(sleep_time));
                println!(
                    "processing {} in {}",
                    self.payload,
                    thread::current().name().unwrap_or("unnamed")
                );
            }
        );
        Ok(())
    }
}

fn main() -> Result<(), anyhow::Error> {
    let mut thread_pool_builder = ThreadPoolBuilder::new();
    let mut tp = thread_pool_builder
        .with_name_str("example")
        .with_tasks(4)
        .with_queue_size(16)
        .with_shutdown_mode(ShutdownMode::CompletePending)
        .build()?;

    tp.set_thread_local(&THREAD_LOCAL_CONFIG, Some(Config::default()));

    for i in 0..16 {
        tp.submit(Box::new(ThreadLocalExampleCommand::new(i)));
    }

    tp.shutdown();
    tp.join()
}