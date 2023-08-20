use std::sync::Arc;
use std::time::{Duration, Instant};

use crossbeam::queue::ArrayQueue;

pub struct CrossbeamBlockingQueue<E> where E: Send + Sync {
    elements: Arc<ArrayQueue<E>>,
}

impl<E> CrossbeamBlockingQueue<E> where E: Send + Sync {
    pub fn new(size: usize) -> CrossbeamBlockingQueue<E> {
        CrossbeamBlockingQueue::<E> {
            elements: Arc::new(ArrayQueue::new(size)),
        }
    }

    pub fn len(&self) -> usize {
        self.elements.len()
    }

    pub fn capacity(&self) -> usize {
        self.elements.capacity()
    }

    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }

    pub fn is_full(&self) -> bool {
        self.elements.is_full()
    }

    pub fn wait_empty(&self, timeout: Duration) -> bool {
        let backoff = crossbeam::utils::Backoff::new();
        let mut t = timeout;
        let mut start = Instant::now();
        while !self.elements.is_empty() {
            let elapsed = start.elapsed();
            if elapsed < t {
                t -= elapsed;
                start = Instant::now();
            } else {
                break;
            }
            backoff.spin();
        }
        self.elements.is_empty()
    }

    pub fn enqueue(&self, element: E) {
        self.try_enqueue(element, Duration::MAX);
    }

    pub fn try_enqueue(&self, element: E, timeout: Duration) -> Option<E> {
        let backoff = crossbeam::utils::Backoff::new();
        let mut t = timeout;
        let mut start = Instant::now();
        let mut e = element;
        loop {
            let result = self.elements.push(e);
            match result {
                Ok(_) => {
                    break None;
                }
                Err(element) => {
                    e = element;
                    let elapsed = start.elapsed();
                    if elapsed < t {
                        t -= elapsed;
                        start = Instant::now();
                    } else {
                        break Some(e);
                    }
                    backoff.spin();
                }
            }
        }
    }

    pub fn dequeue(&self) -> Option<E> {
        self.try_dequeue(Duration::MAX)
    }

    pub fn try_dequeue(&self, timeout: Duration) -> Option<E> {
        let backoff = crossbeam::utils::Backoff::new();
        let mut t = timeout;
        let mut start = Instant::now();
        loop {
            let element = self.elements.pop();
            if element.is_none() {
                let elapsed = start.elapsed();
                if elapsed < t {
                    t -= elapsed;
                    start = Instant::now();
                } else {
                    break None;
                }
            } else {
                break element;
            }
            backoff.spin();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::thread::Builder;

    use super::*;

    #[test]
    fn test_try_dequeue() {
        let q = CrossbeamBlockingQueue::<i32>::new(128);

        let r = q.try_dequeue(Duration::from_millis(0));
        assert_eq!(r, None);
        let r = q.try_dequeue(Duration::from_millis(10));
        assert_eq!(r, None);
    }

    #[test]
    fn test_try_enqueue() {
        let q = CrossbeamBlockingQueue::<i32>::new(128);
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
        let q = CrossbeamBlockingQueue::<i32>::new(128);
        for i in 0..128 {
            q.enqueue(i);
        }

        for i in 0..128 {
            assert_eq!(q.dequeue().unwrap(), i);
        }
    }

    #[test]
    fn test_mpsc() {
        let q = Arc::new(CrossbeamBlockingQueue::<(i32, i32)>::new(16));
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
        let q = Arc::new(CrossbeamBlockingQueue::<(i32, i32)>::new(16));
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
