use std::{
    fmt::Debug,
    sync::{
        Mutex, MutexGuard,
        atomic::{AtomicBool, Ordering},
    },
};

pub struct DropLockAndNotify<T> {
    inner: Mutex<Option<T>>,
    hold: Mutex<()>,
    finished: AtomicBool,
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
            hold: Mutex::new(()),
            finished: AtomicBool::new(false),
        }
    }

    pub async fn lock<'a>(&'a self, value: T) -> Option<DropLockAndNotifyGuard<'a, T>> {
        if self.finished() {
            return None;
        }

        let mut inner = self.inner.lock().unwrap();
        if inner.is_some() {
            return None;
        }

        let guard = self.hold.lock().unwrap();
        *inner = Some(value);

        Some(DropLockAndNotifyGuard {
            guard,
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
        self.finished.load(Ordering::Relaxed)
    }

    pub fn mark_finished(&self) {
        self.finished.store(true, Ordering::Relaxed);
    }

    fn guard_release(&self) {
        let mut inner = self.inner.lock().unwrap();
        *inner = None;
    }
}

pub struct DropLockAndNotifyGuard<'a, T> {
    #[allow(dead_code)]
    guard: MutexGuard<'a, ()>,
    drop_lock_and_notify: &'a DropLockAndNotify<T>,
}

impl<'a, T> Drop for DropLockAndNotifyGuard<'a, T> {
    fn drop(&mut self) {
        self.drop_lock_and_notify.guard_release();
    }
}
