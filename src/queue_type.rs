use std::fmt::{Display, Formatter};

#[derive(Copy, Clone)]
pub enum QueueType {
    BlockingQueue,
    CrossbeamBlockingQueue,
}

impl Display for QueueType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            QueueType::BlockingQueue => {
                write!(f, "BlockingQueue")
            }
            QueueType::CrossbeamBlockingQueue => {
                write!(f, "CrossbeamBlockingQueue")
            }
        }
    }
}