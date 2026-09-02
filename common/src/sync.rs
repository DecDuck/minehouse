use std::{
    fmt::Debug,
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
use tokio::sync::{Mutex as AsyncMutex, MutexGuard as AsyncMutexGuard, Notify};

pub struct DropLockAndNotify<T> {
    inner: Mutex<Option<T>>,
    hold: AsyncMutex<()>,
    finished: AtomicBool,
    finished_notify: Notify,
}

impl<T> Debug for DropLockAndNotify<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DropLockAndNotify")
            .field("finished", &self.finished)
            .finish()
    }
}

impl<T> DropLockAndNotify<T> {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(None),
            hold: AsyncMutex::new(()),
            finished: AtomicBool::new(false),
            finished_notify: Notify::new(),
        }
    }

    pub async fn lock<'a>(&'a self, value: T) -> Option<DropLockAndNotifyGuard<'a, T>> {
        if self.finished() {
            return None;
        }

        let guard = self.hold.try_lock().ok()?;
        if self.finished() {
            return None;
        }

        *self.inner.lock().unwrap() = Some(value);

        Some(DropLockAndNotifyGuard {
            guard: Some(guard),
            drop_lock_and_notify: self,
        })
    }

    pub fn read(&self) -> Option<T>
    where
        T: Clone,
    {
        self.inner.lock().unwrap().clone()
    }

    pub fn finished(&self) -> bool {
        self.finished.load(Ordering::Acquire)
    }

    pub fn mark_finished(&self) {
        if !self.finished.swap(true, Ordering::Release) {
            self.finished_notify.notify_waiters();
        }
    }

    pub async fn wait_finished(&self) {
        loop {
            let mut notified = Box::pin(self.finished_notify.notified());
            notified.as_mut().enable();

            if self.finished() {
                return;
            }

            notified.await;
        }
    }

    fn guard_release(&self) {
        let mut inner = self.inner.lock().unwrap();
        *inner = None;
    }
}

pub struct DropLockAndNotifyGuard<'a, T> {
    guard: Option<AsyncMutexGuard<'a, ()>>,
    drop_lock_and_notify: &'a DropLockAndNotify<T>,
}

impl<'a, T> DropLockAndNotifyGuard<'a, T> {
    pub async fn wait_finished(&self) {
        self.drop_lock_and_notify.wait_finished().await;
    }
}

impl<'a, T> Drop for DropLockAndNotifyGuard<'a, T> {
    fn drop(&mut self) {
        self.drop_lock_and_notify.guard_release();
        self.guard.take();
    }
}

#[cfg(test)]
mod tests {
    use std::{sync::Arc, time::Duration};

    use super::DropLockAndNotify;

    #[tokio::test]
    async fn waits_while_exclusively_holding_readable_value() {
        let lock = Arc::new(DropLockAndNotify::new());
        let guard = lock.lock(42).await.unwrap();

        assert_eq!(lock.read(), Some(42));
        assert!(lock.lock(7).await.is_none());

        let finishing_lock = Arc::clone(&lock);
        tokio::spawn(async move {
            tokio::task::yield_now().await;
            finishing_lock.mark_finished();
        });

        tokio::time::timeout(Duration::from_secs(1), guard.wait_finished())
            .await
            .unwrap();
        assert_eq!(lock.read(), Some(42));

        drop(guard);
        assert_eq!(lock.read(), None);
    }

    #[tokio::test]
    async fn returns_immediately_when_already_finished() {
        let lock = DropLockAndNotify::new();
        let guard = lock.lock(42).await.unwrap();
        lock.mark_finished();

        tokio::time::timeout(Duration::from_secs(1), guard.wait_finished())
            .await
            .unwrap();
    }
}
