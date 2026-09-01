use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

/// A multi-producer queue that can be drained by a consumer and snapshotted for
/// inspection (e.g. rendering in a UI) without removing items.
///
/// Cloning yields another handle to the same underlying queue, so it can be freely
/// shared across tokio tasks. Every operation takes a short lock and never awaits.
pub struct InspectableQueue<T> {
    inner: Arc<Mutex<VecDeque<T>>>,
}

impl<T> InspectableQueue<T> {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    /// Pushes an item onto the back of the queue. Callable from any task.
    #[allow(dead_code)]
    pub fn push(&self, item: T) {
        self.inner.lock().unwrap().push_back(item);
    }

    /// Removes and returns the item at the front of the queue, if any.
    pub fn pop(&self) -> Option<T> {
        self.inner.lock().unwrap().pop_front()
    }
}

impl<T: Clone> InspectableQueue<T> {
    /// Returns a snapshot of the queued items, front to back, without removing them.
    #[allow(dead_code)]
    pub fn snapshot(&self) -> Vec<T> {
        self.inner.lock().unwrap().iter().cloned().collect()
    }
}

impl<T> Clone for InspectableQueue<T> {
    // Manual impl clones the shared handle without requiring `T: Clone`.
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<T> Default for InspectableQueue<T> {
    fn default() -> Self {
        Self::new()
    }
}
