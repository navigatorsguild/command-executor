![Maintenance](https://img.shields.io/badge/maintenance-activly--developed-brightgreen.svg)

# command-executor

Thread pool implementation in Rust

Command Executor is a multiple producer/multiple consumer thread pool implementation in Rust
that provides an interface for concurrent command execution with backpressure. Main design
goals of this implementation are:
* control of memory consumption - achieved through use of a bounded blocking queue
* indication of workload completion - achieved through a shutdown "after all work is complete"
* maintain a predictable models of the execution - commands are submitted for execution in FIFO order with
backpressure until all work is done. Note that the actual order of execution is depending on
the system scheduler, however, you can assume that the `n + 1` command will be dequeued by an
executing thread after 'n' command was dequeued.

## Use cases
The use case for this approach to parallelism is when handling very large data sets that cannot fit
into memory, or when the memory consumption must be within specified bounds. When data set can
fit into memory or the memory consumption is not an issue almost always better results would be
achieved using [Rayon](https://crates.io/crates/rayon) or other data parallelism libraries that
use non blocking queues.
Example for such an use case was to convert a large and very dense Protobuf file - about
67 GB of sorted data - into about 1 TB of pg_dump files ready to load into Postgresql
database. Using this library it was possible to create a pipeline that parallelized the
Protobuf decoding and pg_dump encoding and finally wrote the merged result while maintaining
the initial order and keeping the memory within bounds.

## Examples

Submit commands for execution and wait for completion
```rust
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
       .name("example".to_string())
       .tasks(4)
       .queue_size(16)
       .shutdown_mode(ShutdownMode::CompletePending)
       .build()
       .unwrap();

   for i in 0..16 {
       tp.submit(Box::new(ExampleCommand::new(i)));
   }

   tp.shutdown();
   tp.join()
}
```

Install a thread local value in all threads of the thread pool
```rust
use std::thread;
use std::time::Duration;
use std::cell::RefCell;
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
       .name("example".to_string())
       .tasks(4)
       .queue_size(16)
       .shutdown_mode(ShutdownMode::CompletePending)
       .build()
       .unwrap();

   tp.set_thread_local(&THREAD_LOCAL_CONFIG, Some(Config::default()));

   for i in 0..16 {
       tp.submit(Box::new(ThreadLocalExampleCommand::new(i)));
   }

   tp.shutdown();
   tp.join()
}
```


License: MIT OR Apache-2.0
