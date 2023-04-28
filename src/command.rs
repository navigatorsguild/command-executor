/// Trait that specifies the interface for concurrent task execution
pub trait Command {
    /// The execute method will executed in the context of one of the threads of the thread pool.
    ///
    /// The execute method should avoid panic or returning errors, however, if errors will be
    /// returned, the last one will be passed to join handler set by
    /// [crate::thread_pool_builder::ThreadPoolBuilder::join_error_handler]
    fn execute(&self) -> Result<(), anyhow::Error>;
}
