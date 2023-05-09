use std::time::Duration;

use crate::blocking_queue::BlockingQueue;
use crate::crossbeam_blocking_queue::CrossbeamBlockingQueue;
use crate::queue_type::QueueType;

/// Wrapper over available queue types.
pub enum BlockingQueueAdapter<E> where E: Send + Sync {
    BlockingQueue {
        blocking_queue: BlockingQueue<E>,
    },
    CrossbeamBlockingQueue {
        crossbeam_blocking_queue: CrossbeamBlockingQueue<E>
    },
}

impl<E> BlockingQueueAdapter<E> where E: Send + Sync {
    pub fn new(queue_type: QueueType, size: usize) -> BlockingQueueAdapter::<E> {
        match queue_type {
            QueueType::BlockingQueue => {
                BlockingQueueAdapter::BlockingQueue {
                    blocking_queue: BlockingQueue::new(size)
                }
            }
            QueueType::CrossbeamBlockingQueue => {
                BlockingQueueAdapter::CrossbeamBlockingQueue {
                    crossbeam_blocking_queue: CrossbeamBlockingQueue::new(size)
                }
            }
        }
    }

    pub fn len(&self) -> usize {
        match self {
            BlockingQueueAdapter::BlockingQueue { blocking_queue } => {
                blocking_queue.len()
            }
            BlockingQueueAdapter::CrossbeamBlockingQueue { crossbeam_blocking_queue } => {
                crossbeam_blocking_queue.len()
            }
        }
    }

    pub fn capacity(&self) -> usize {
        match self {
            BlockingQueueAdapter::BlockingQueue { blocking_queue } => {
                blocking_queue.capacity()
            }
            BlockingQueueAdapter::CrossbeamBlockingQueue { crossbeam_blocking_queue } => {
                crossbeam_blocking_queue.capacity()
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        match self {
            BlockingQueueAdapter::BlockingQueue { blocking_queue } => {
                blocking_queue.is_empty()
            }
            BlockingQueueAdapter::CrossbeamBlockingQueue { crossbeam_blocking_queue } => {
                crossbeam_blocking_queue.is_empty()
            }
        }
    }

    pub fn is_full(&self) -> bool {
        match self {
            BlockingQueueAdapter::BlockingQueue { blocking_queue } => {
                blocking_queue.is_full()
            }
            BlockingQueueAdapter::CrossbeamBlockingQueue { crossbeam_blocking_queue } => {
                crossbeam_blocking_queue.is_full()
            }
        }
    }

    pub fn wait_empty(&self, timeout: Duration) -> bool {
        match self {
            BlockingQueueAdapter::BlockingQueue { blocking_queue } => {
                blocking_queue.wait_empty(timeout)
            }
            BlockingQueueAdapter::CrossbeamBlockingQueue { crossbeam_blocking_queue } => {
                crossbeam_blocking_queue.wait_empty(timeout)
            }
        }
    }

    pub fn enqueue(&self, element: E) {
        match self {
            BlockingQueueAdapter::BlockingQueue { blocking_queue } => {
                blocking_queue.enqueue(element)
            }
            BlockingQueueAdapter::CrossbeamBlockingQueue { crossbeam_blocking_queue } => {
                crossbeam_blocking_queue.enqueue(element)
            }
        }
    }

    pub fn try_enqueue(&self, element: E, timeout: Duration) -> Option<E> {
        match self {
            BlockingQueueAdapter::BlockingQueue { blocking_queue } => {
                blocking_queue.try_enqueue(element, timeout)
            }
            BlockingQueueAdapter::CrossbeamBlockingQueue { crossbeam_blocking_queue } => {
                crossbeam_blocking_queue.try_enqueue(element, timeout)
            }
        }
    }

    pub fn dequeue(&self) -> Option<E> {
        match self {
            BlockingQueueAdapter::BlockingQueue { blocking_queue } => {
                blocking_queue.dequeue()
            }
            BlockingQueueAdapter::CrossbeamBlockingQueue { crossbeam_blocking_queue } => {
                crossbeam_blocking_queue.dequeue()
            }
        }
    }

    pub fn try_dequeue(&self, timeout: Duration) -> Option<E> {
        match self {
            BlockingQueueAdapter::BlockingQueue { blocking_queue } => {
                blocking_queue.try_dequeue(timeout)
            }
            BlockingQueueAdapter::CrossbeamBlockingQueue { crossbeam_blocking_queue } => {
                crossbeam_blocking_queue.try_dequeue(timeout)
            }
        }
    }
}
