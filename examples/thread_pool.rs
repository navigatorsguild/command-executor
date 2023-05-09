use std::thread;
use std::time::Duration;

use command_executor::command::Command;
use command_executor::shutdown_mode::ShutdownMode;
use command_executor::thread_pool_builder::ThreadPoolBuilder;

struct ExampleCommand {
    payload: i32,
}

impl ExampleCommand {
    pub fn new(payload: i32) -> ExampleCommand {
        ExampleCommand {
            payload,
        }
    }
}

impl Command for ExampleCommand {
    fn execute(&self) -> Result<(), anyhow::Error> {
        println!("processing {} in {}", self.payload, thread::current().name().unwrap_or("unnamed"));
        thread::sleep(Duration::from_millis(10));
        Ok(())
    }
}

pub fn main() -> Result<(), anyhow::Error> {
    let mut thread_pool_builder = ThreadPoolBuilder::new();
    let mut tp = thread_pool_builder
        .with_name_str("example")
        .with_tasks(4)
        .with_queue_size(16)
        .with_shutdown_mode(ShutdownMode::CompletePending)
        .build()?;

    for i in 0..16 {
        tp.submit(Box::new(ExampleCommand::new(i)));
    }

    tp.shutdown();
    tp.join()
}
