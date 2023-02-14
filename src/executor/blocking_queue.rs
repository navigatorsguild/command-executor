use std::collections::VecDeque;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, SystemTime};

struct QueueFlags<S> where S: Send + Sync + Clone {
    empty: bool,
    full: bool,
    signal: Option<S>,
}

impl<S> QueueFlags<S> where S: Send + Sync + Clone {
    fn new() -> QueueFlags<S> {
        QueueFlags {
            empty: true,
            full: false,
            signal: None,
        }
    }
}

pub struct BlockingQueue<E, S> where E: Send + Sync, S: Send + Sync + Clone {
    flags: Arc<Mutex<QueueFlags<S>>>,
    empty: Arc<Condvar>,
    full: Arc<Condvar>,
    elements: Arc<Mutex<VecDeque<E>>>,
    size: usize,
}

impl<E, S> BlockingQueue<E, S> where E: Send + Sync, S: Send + Sync + Clone {
    pub fn new(size: usize) -> BlockingQueue<E, S> {
        let flags = Arc::new(Mutex::new(QueueFlags::new()));
        BlockingQueue::<E, S> {
            flags,
            empty: Arc::new(Condvar::new()),
            full: Arc::new(Condvar::new()),
            elements: Arc::new(Mutex::new(VecDeque::new())),
            size,
        }
    }

    pub fn len(&self) -> usize {
        self.elements.lock().unwrap().len()
    }

    pub fn wait_empty(&self, timeout: Duration) -> bool {
        let flags_lock = &*self.flags;
        let empty = &*self.empty;
        let mut flags = flags_lock.lock().unwrap();
        let mut t = timeout;
        let mut start = SystemTime::now();
        while !(*flags).empty {
            match empty.wait_timeout(flags, t).unwrap() {
                (f, timeout_result) => {
                    flags = f;
                    if timeout_result.timed_out() {
                        break;
                    } else {
                        let elapsed = start.elapsed().unwrap();
                        if elapsed < t {
                            t = t - elapsed;
                            start = SystemTime::now();
                        }
                    }
                }
            }
        }

        (*flags).empty
    }

    pub fn enqueue(&self, element: E) {
        self.try_enqueue(element, Duration::MAX);
    }

    pub fn try_enqueue(&self, element: E, timeout: Duration) -> Option<E> {
        let flags_lock = &*self.flags;
        let empty = &*self.empty;
        let full = &*self.full;
        let mut flags = flags_lock.lock().unwrap();
        let mut timed_out = false;
        let mut t = timeout;
        let mut start = SystemTime::now();
        while (*flags).full {
            match full.wait_timeout(flags, t).unwrap() {
                (f, timeout_result) => {
                    flags = f;
                    if timeout_result.timed_out() {
                        timed_out = true;
                        break;
                    } else {
                        let elapsed = start.elapsed().unwrap();
                        if elapsed < t {
                            t = t - elapsed;
                            start = SystemTime::now();
                        }
                    }
                }
            }
        }

        if timed_out {
            Some(element)
        } else {
            let mut elements = self.elements.lock().unwrap();
            elements.push_back(element);
            if elements.len() == self.size {
                (*flags).full = true;
                full.notify_all()
            } else {
                (*flags).empty = false;
                empty.notify_one();
            }
            None
        }
    }

    pub fn dequeue(&self) -> (Option<E>, Option<S>) {
        self.try_dequeue(Duration::MAX)
    }

    pub fn try_dequeue(&self, timeout: Duration) -> (Option<E>, Option<S>) {
        let flags_lock = &*self.flags;
        let empty = &*self.empty;
        let full = &*self.full;
        let mut flags = flags_lock.lock().unwrap();
        let mut timed_out = false;
        let mut t = timeout;
        let mut start = SystemTime::now();
        while (*flags).empty && flags.signal.is_none() {
            match empty.wait_timeout(flags, t).unwrap() {
                (f, timeout_result) => {
                    flags = f;
                    if timeout_result.timed_out() {
                        timed_out = true;
                        break;
                    } else {
                        let elapsed = start.elapsed().unwrap();
                        if elapsed < t {
                            t = t - elapsed;
                            start = SystemTime::now();
                        }
                    }
                }
            }
        }

        if timed_out {
            (None, None)
        } else {
            let mut elements = self.elements.lock().unwrap();
            let element = elements.pop_front();

            if elements.len() == 0 {
                (*flags).empty = true;
                empty.notify_all();
            } else {
                (*flags).full = false;
                full.notify_one();
            }
            (element, flags.signal.clone())
        }
    }

    pub fn signal(&self, signal: S) {
        let empty = &*self.empty;
        let mut flags = self.flags.lock().unwrap();
        (*flags).signal = Some(signal);
        empty.notify_all();
    }
}

#[cfg(test)]
mod tests {
    use std::thread::Builder;
    use super::*;

    #[test]
    fn test_try_dequeue() {
        let q = BlockingQueue::<i32, ()>::new(128);

        let r = q.try_dequeue(Duration::from_millis(0));
        assert_eq!(r, (None, None));
        let r = q.try_dequeue(Duration::from_millis(10));
        assert_eq!(r, (None, None));
    }

    #[test]
    fn test_try_enqueue() {
        let q = BlockingQueue::<i32, ()>::new(128);
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
        let q = BlockingQueue::<i32, ()>::new(128);
        for i in 0..128 {
            q.enqueue(i);
        }

        for i in 0..128 {
            assert_eq!(q.dequeue().0.unwrap(), i);
        }
    }

    #[test]
    fn test_mpsc() {
        let q = Arc::new(BlockingQueue::<(i32, i32), ()>::new(16));
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
                        let (element, _signal) = qc1.dequeue();
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
        let q = Arc::new(BlockingQueue::<(i32, i32), ()>::new(16));
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
                        let (element, signal) = qc1.dequeue();
                        match element {
                            None => {}
                            Some(e) => {
                                collector.push(e);
                            }
                        }
                        match signal {
                            None => {}
                            Some(_) => {
                                break collector;
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
                        let (element, signal) = qc2.dequeue();
                        match element {
                            None => {}
                            Some(e) => {
                                collector.push(e);
                            }
                        }
                        match signal {
                            None => {}
                            Some(_) => {
                                break collector;
                            }
                        }
                    }
                }
            );
        p1.unwrap().join().expect("failed to join producer");
        p2.unwrap().join().expect("failed to join producer");
        q.signal(());

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
