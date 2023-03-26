pub(crate) mod blocking_queue;
pub(crate) mod thread_pool;
pub(crate) mod thread_pool_builder;
pub(crate) mod signal;
pub(crate) mod shutdown_mode;

pub trait Command {
    fn execute(&self) -> Result<(), anyhow::Error>;
}

pub type BlockingQueue<E, S> = blocking_queue::BlockingQueue<E, S>;
pub type ThreadPool = thread_pool::ThreadPool;
pub type ThreadPoolBuilder = thread_pool_builder::ThreadPoolBuilder;
pub type Signal = signal::Signal;
pub type ShutdownMode = shutdown_mode::ShutdownMode;

