use std::collections::VecDeque;
use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};

struct QueueFlags {
    empty: bool,
    full: bool,
}

impl QueueFlags {
    fn new() -> QueueFlags {
        QueueFlags {
            empty: true,
            full: false,
        }
    }
}

/// Blocking bounded queue
///
/// `E: Send + Sync` - the element type
/// This is a multiple producers / multiple consumers blocking bounded queue.
/// Reference: [Producer-Consumer](https://en.wikipedia.org/wiki/Producer%E2%80%93consumer_problem)
pub struct BlockingQueue<E> where E: Send + Sync {
    flags: Mutex<QueueFlags>,
    empty: Condvar,
    full: Condvar,
    elements: Mutex<VecDeque<E>>,
    capacity: usize,
}

impl<E> BlockingQueue<E> where E: Send + Sync {
    /// Create a new queue with `size` capacity
    /// ```
    /// use command_executor::blocking_queue::BlockingQueue;
    /// let q: BlockingQueue<i32> = BlockingQueue::new(4);
    /// ```
    pub fn new(capacity: usize) -> BlockingQueue<E> {
        BlockingQueue::<E> {
            flags: Mutex::new(QueueFlags::new()),
            empty: Condvar::new(),
            full: Condvar::new(),
            elements: Mutex::new(VecDeque::with_capacity(capacity)),
            capacity,
        }
    }

    /// The current length of the queue. Note that the reported length is correct at the time
    /// of checking, the actual length may change between the call and the access to the result
    /// value. Should be used for diagnostic and monitoring only.
    /// ```
    /// use command_executor::blocking_queue::BlockingQueue;
    /// let q: BlockingQueue<i32> = BlockingQueue::new(4);
    /// q.enqueue(11);
    /// assert_eq!(q.len(), 1);
    /// ```
    pub fn len(&self) -> usize {
        self.elements.lock().unwrap().len()
    }

    /// The declared capacity of the queue. May be smaller than the actual capacity of the actual
    /// storage
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Indication if the queue is empty in this point of time. Should be used for diagnostic
    /// and monitoring only.
    pub fn is_empty(&self) -> bool {
        self.elements.lock().unwrap().is_empty()
    }

    /// Indication if the queue is full in this point of time. Should be used for diagnostic
    /// and monitoring only.
    pub fn is_full(&self) -> bool {
        self.len() == self.capacity()
    }

    /// Wait until the queue is empty.
    ///
    /// Note that the empty state is temporary. This method is mostly useful when we know that no
    /// elements are to be enqueued and we want an indication of completion.
    pub fn wait_empty(&self, timeout: Duration) -> bool {
        let flags_lock = &self.flags;
        let empty = &self.empty;
        let mut flags = flags_lock.lock().unwrap();
        let mut t = timeout;
        let mut start = Instant::now();
        while !flags.empty {
            let (f, timeout_result) = empty.wait_timeout(flags, t).unwrap();
            {
                flags = f;
                if timeout_result.timed_out() {
                    break;
                } else {
                    let elapsed = start.elapsed();
                    if elapsed < t {
                        t -= elapsed;
                        start = Instant::now();
                    } else {
                        break;
                    }
                }
            }
        }
        flags.empty
    }

    /// Enqueue an element. When the queue is full will block until space available.
    pub fn enqueue(&self, element: E) {
        self.try_enqueue(element, Duration::MAX);
    }

    /// Enqueue an element with timeout. When timeout is exceeded return the element to caller.
    pub fn try_enqueue(&self, element: E, timeout: Duration) -> Option<E> {
        let flags_lock = &self.flags;
        let empty = &self.empty;
        let full = &self.full;
        let mut flags = flags_lock.lock().unwrap();
        let mut timed_out = false;
        let mut t = timeout;
        let mut start = Instant::now();
        while flags.full {
            let (f, timeout_result) = full.wait_timeout(flags, t).unwrap();
            {
                flags = f;
                if timeout_result.timed_out() {
                    timed_out = true;
                    break;
                } else {
                    let elapsed = start.elapsed();
                    if elapsed < t {
                        t -= elapsed;
                        start = Instant::now();
                    } else {
                        timed_out = true;
                        break;
                    }
                }
            }
        }

        if timed_out {
            Some(element)
        } else {
            let mut elements = self.elements.lock().unwrap();
            elements.push_back(element);
            flags.empty = false;
            empty.notify_one();
            if elements.len() == self.capacity() {
                flags.full = true;
                full.notify_all()
            }
            None
        }
    }

    /// Dequeue an element from the queue. When the queue is empty will block until an element is
    /// available
    pub fn dequeue(&self) -> Option<E> {
        self.try_dequeue(Duration::MAX)
    }

    /// Dequeue and element from the queue with timeout.
    pub fn try_dequeue(&self, timeout: Duration) -> Option<E> {
        let flags_lock = &self.flags;
        let empty = &self.empty;
        let full = &self.full;
        let mut flags = flags_lock.lock().unwrap();
        let mut timed_out = false;
        let mut t = timeout;
        let mut start = Instant::now();
        while flags.empty {
            let (f, timeout_result) = empty.wait_timeout(flags, t).unwrap();
            {
                flags = f;
                if timeout_result.timed_out() {
                    timed_out = true;
                    break;
                } else {
                    let elapsed = start.elapsed();
                    if elapsed < t {
                        t -= elapsed;
                        start = Instant::now();
                    } else {
                        timed_out = true;
                        break;
                    }
                }
            }
        }

        if timed_out {
            None
        } else {
            let mut elements = self.elements.lock().unwrap();
            let element = elements.pop_front();
            flags.full = false;
            full.notify_one();
            if elements.len() == 0 {
                flags.empty = true;
                empty.notify_all();
            }
            element
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::thread::Builder;

    use super::*;

    #[test]
    fn test_try_dequeue() {
        let q = BlockingQueue::<i32>::new(128);

        let r = q.try_dequeue(Duration::from_millis(0));
        assert_eq!(r, None);
        let r = q.try_dequeue(Duration::from_millis(10));
        assert_eq!(r, None);
    }

    #[test]
    fn test_try_enqueue() {
        let q = BlockingQueue::<i32>::new(128);
        for i in 0..128 {
            q.enqueue(i);
        }

        let r = q.try_enqueue(128, Duration::from_millis(0));
        assert_eq!(r, Some(128));
        let r = q.try_enqueue(128, Duration::from_millis(10));
        assert_eq!(r, Some(128));
    }

    #[test]
    fn test_fifo() {
        let q = BlockingQueue::<i32>::new(128);
        for i in 0..128 {
            q.enqueue(i);
        }

        for i in 0..128 {
            assert_eq!(q.dequeue().unwrap(), i);
        }
    }

    #[test]
    fn test_mpsc() {
        let q = Arc::new(BlockingQueue::<(i32, i32)>::new(16));
        let qp1 = q.clone();
        let qp2 = q.clone();
        let qc1 = q.clone();

        let p1 = Builder::new()
            .spawn(
                move || {
                    for i in 0..2048 {
                        qp1.enqueue((1, i));
                    }
                }
            );

        let p2 = Builder::new()
            .spawn(
                move || {
                    for i in 0..2048 {
                        qp2.enqueue((2, i));
                    }
                }
            );

        let c1 = Builder::new()
            .spawn(
                move || {
                    let mut collector = Vec::<(i32, i32)>::new();
                    loop {
                        let element = qc1.dequeue();
                        collector.push(element.unwrap());
                        if collector.len() == 4096 {
                            break collector;
                        }
                    }
                }
            );
        p1.unwrap().join().expect("failed to join producer");
        p2.unwrap().join().expect("failed to join producer");

        let mut collector = c1.unwrap().join().expect("failed to join consumer");
        for i in 0..2048 {
            let i1 = collector.iter().position(|e| *e == (1, i)).unwrap();
            collector.remove(i1);
            let i2 = collector.iter().position(|e| *e == (2, i)).unwrap();
            collector.remove(i2);
        }
        assert!(collector.is_empty());
    }

    #[test]
    fn test_mpmc() {
        let q = Arc::new(BlockingQueue::<(i32, i32)>::new(16));
        let qp1 = q.clone();
        let qp2 = q.clone();
        let qc1 = q.clone();
        let qc2 = q.clone();

        let p1 = Builder::new()
            .spawn(
                move || {
                    for i in 0..2048 {
                        qp1.enqueue((1, i));
                    }
                }
            );

        let p2 = Builder::new()
            .spawn(
                move || {
                    for i in 0..2048 {
                        qp2.enqueue((2, i));
                    }
                }
            );

        let c1 = Builder::new()
            .spawn(
                move || {
                    let mut collector = Vec::<(i32, i32)>::new();
                    loop {
                        let element = qc1.dequeue();
                        match element {
                            None => {}
                            Some((-1, -1)) => {
                                break collector;
                            }
                            Some(e) => {
                                collector.push(e);
                            }
                        }
                    }
                }
            );

        let c2 = Builder::new()
            .spawn(
                move || {
                    let mut collector = Vec::<(i32, i32)>::new();
                    loop {
                        let element = qc2.dequeue();
                        match element {
                            None => {}
                            Some((-1, -1)) => {
                                break collector;
                            }
                            Some(e) => {
                                collector.push(e);
                            }
                        }
                    }
                }
            );

        p1.unwrap().join().expect("failed to join producer");
        p2.unwrap().join().expect("failed to join producer");

        q.enqueue((-1, -1));
        q.enqueue((-1, -1));

        let mut collector1 = c1.unwrap().join().expect("failed to join consumer");
        let mut collector2 = c2.unwrap().join().expect("failed to join consumer");

        let mut collector = Vec::<(i32, i32)>::new();
        collector.append(&mut collector1);
        collector.append(&mut collector2);

        for i in 0..2048 {
            let i1 = collector.iter().position(|e| *e == (1, i)).unwrap();
            collector.remove(i1);
            let i2 = collector.iter().position(|e| *e == (2, i)).unwrap();
            collector.remove(i2);
        }
        assert!(collector.is_empty());
    }
}
