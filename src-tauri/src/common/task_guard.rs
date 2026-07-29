//! Ownership guard for background tasks.

use std::sync::Arc;
use tokio::task::JoinHandle;

/// Aborts a task when the last holder of this guard is dropped.
///
/// Both backends run a reader task tied to a connection, and both keep that connection inside
/// actor state that the framework requires to be `Clone`. So the connection is shared rather
/// than owned, and aborting on the first drop would kill a reader another clone still uses.
/// Wrapping the handle in an `Arc<AbortOnDrop>` ties the task's life to the last clone instead.
///
/// Without this the task would outlive the connection and hold the port or pipe open, blocking
/// the next connection attempt.
#[derive(Debug)]
pub struct AbortOnDrop(JoinHandle<()>);

impl AbortOnDrop {
    pub fn new(handle: JoinHandle<()>) -> Arc<Self> {
        Arc::new(Self(handle))
    }
}

impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use tokio::time::{Duration, sleep};

    #[tokio::test]
    async fn the_task_survives_while_any_clone_is_alive() {
        let finished = Arc::new(AtomicBool::new(false));
        let flag = finished.clone();

        let guard = AbortOnDrop::new(tokio::spawn(async move {
            sleep(Duration::from_millis(50)).await;
            flag.store(true, Ordering::SeqCst);
        }));

        let second = guard.clone();
        drop(guard);

        sleep(Duration::from_millis(150)).await;
        assert!(
            finished.load(Ordering::SeqCst),
            "dropping one clone must not abort the task"
        );
        drop(second);
    }

    #[tokio::test]
    async fn the_task_is_aborted_once_every_clone_is_gone() {
        let finished = Arc::new(AtomicBool::new(false));
        let flag = finished.clone();

        let guard = AbortOnDrop::new(tokio::spawn(async move {
            sleep(Duration::from_secs(30)).await;
            flag.store(true, Ordering::SeqCst);
        }));

        drop(guard);
        sleep(Duration::from_millis(50)).await;

        assert!(!finished.load(Ordering::SeqCst), "the task should be gone");
    }
}
