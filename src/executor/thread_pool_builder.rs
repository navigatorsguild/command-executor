use crate::errors::GenericError;
use crate::executor::shutdown_mode::ShutdownMode;
use crate::executor::thread_pool::ThreadPool;

pub struct ThreadPoolBuilder {
    name: String,
    tasks: usize,
    queue_size: usize,
    join_error_handler: fn(String, String),
    shutdown_mode: ShutdownMode,
}



impl ThreadPoolBuilder {
    pub fn new() -> ThreadPoolBuilder {
        let join_error_handler = |name: String, message: String| {
            panic!("Thread {name} ended with and error {message}")
        };

        ThreadPoolBuilder {
            name: "unnamed".to_string(),
            tasks: 1,
            queue_size: 1024,
            join_error_handler,
            shutdown_mode: ShutdownMode::Immediate,
        }
    }

    pub fn name(&mut self, name: String) -> &mut ThreadPoolBuilder {
        self.name = name.clone();
        self
    }

    pub fn tasks(&mut self, tasks: usize) -> &mut ThreadPoolBuilder {
        self.tasks = tasks;
        self
    }


    pub fn shutdown_mode(&mut self, shutdown_mode: ShutdownMode) -> &mut ThreadPoolBuilder {
        self.shutdown_mode = shutdown_mode;
        self
    }

    pub fn queue_size(&mut self, queue_size: usize) -> &mut ThreadPoolBuilder {
        self.queue_size = queue_size;
        self
    }

    pub fn join_error_handler(&mut self, join_error_handler: fn(String, String)) -> &mut ThreadPoolBuilder{
        self.join_error_handler = join_error_handler;
        self
    }

    pub fn build(&self) -> Result<ThreadPool, GenericError> {
        ThreadPool::new(
            self.name.clone(),
            self.tasks,
            self.queue_size,
            self.join_error_handler,
            self.shutdown_mode.clone(),
        )
    }
}