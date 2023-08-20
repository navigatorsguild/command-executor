use crate::queue_type::QueueType;
use crate::shutdown_mode::ShutdownMode;
use crate::thread_pool::ThreadPool;

/// Build a [ThreadPool]
///
/// Modify default thread pool parameters and build the thread pool
pub struct ThreadPoolBuilder {
    name: String,
    tasks: usize,
    queue_type: QueueType,
    queue_size: usize,
    join_error_handler: fn(String, String),
    shutdown_mode: ShutdownMode,
}

impl ThreadPoolBuilder {
    /// Create a new builder
    ///
    /// Default values:
    /// * `name` - "unnamed"
    /// * `tasks` - 1
    /// * `queue_size` - 16
    /// * `join_error_handler` - a simple panicking error handler
    /// * `shutdown_mode` - [ShutdownMode::Immediate]
    ///
    ///  # Example
    /// ```
    ///
    /// use command_executor::shutdown_mode::ShutdownMode;
    /// use command_executor::thread_pool::ThreadPool;
    /// use command_executor::thread_pool_builder::ThreadPoolBuilder;
    ///
    /// fn create_thread_pool() -> Result<ThreadPool, anyhow::Error> {
    ///     ThreadPoolBuilder::new()
    ///         .with_name_str("example")
    ///         .with_tasks(4)
    ///         .with_queue_size(16)
    ///         .with_shutdown_mode(ShutdownMode::CompletePending)
    ///         .build()
    /// }
    /// ```
    pub fn new() -> ThreadPoolBuilder {
        let join_error_handler = |name: String, message: String| {
            panic!("Thread {name} ended with and error {message}")
        };

        ThreadPoolBuilder {
            name: "unnamed".to_string(),
            tasks: 1,
            queue_type: QueueType::CrossbeamBlockingQueue,
            queue_size: 16,
            join_error_handler,
            shutdown_mode: ShutdownMode::Immediate,
        }
    }

    /// Set the base name for threads in the thread pool
    pub fn with_name(&mut self, name: String) -> &mut ThreadPoolBuilder {
        self.name = name.clone();
        self
    }

    /// Set the base name for threads in the thread pool. A convenience method that accepts &str
    pub fn with_name_str(&mut self, name: &str) -> &mut ThreadPoolBuilder {
        self.name = name.to_string();
        self
    }

    /// Set the number of threads in the thread pool
    pub fn with_tasks(&mut self, tasks: usize) -> &mut ThreadPoolBuilder {
        self.tasks = tasks;
        self
    }

    /// Specify the [ShutdownMode]
    pub fn with_shutdown_mode(&mut self, shutdown_mode: ShutdownMode) -> &mut ThreadPoolBuilder {
        self.shutdown_mode = shutdown_mode;
        self
    }

    /// Specify the [QueueType]
    pub fn with_queue_type(&mut self, queue_type: QueueType) -> &mut ThreadPoolBuilder {
        self.queue_type = queue_type;
        self
    }

    /// Specify the queue size for the thread pool
    pub fn with_queue_size(&mut self, queue_size: usize) -> &mut ThreadPoolBuilder {
        self.queue_size = queue_size;
        self
    }

    /// Set the error handler that is called for each thread that exited with error during join
    pub fn with_join_error_handler(&mut self, join_error_handler: fn(String, String)) -> &mut ThreadPoolBuilder {
        self.join_error_handler = join_error_handler;
        self
    }

    /// Build the thread pool
    pub fn build(&self) -> Result<ThreadPool, anyhow::Error> {
        ThreadPool::new(
            self.name.clone(),
            self.tasks,
            self.queue_type,
            self.queue_size,
            self.join_error_handler,
            self.shutdown_mode.clone(),
        )
    }
}

impl Default for ThreadPoolBuilder {
    fn default() -> Self {
        Self::new()
    }
}
