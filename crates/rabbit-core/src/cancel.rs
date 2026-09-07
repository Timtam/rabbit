//! Cooperative cancellation for the install pipeline.
//!
//! The pipeline runs on a worker thread while the wizard keeps painting, so
//! "stop what you are doing" has to travel the other way: from the UI thread
//! into a run that is already several packages deep. Killing the thread (or
//! the process, which is what closing the window used to do) is not an
//! option — a half-copied `UserPlugins` DLL, a vendor installer mid-write, a
//! lock file nobody released and a temp folder nobody removed are exactly
//! the mess this type exists to avoid.
//!
//! So cancellation is cooperative and coarse. The pipeline reads the token
//! at package boundaries, between configuration steps, and inside the
//! download loop's chunk/retry checkpoints. Whatever step is running when
//! the flag goes up runs to completion — an elevated vendor installer
//! cannot be unwound halfway, and a file copy that is already under way is
//! safer finished than abandoned. Everything after it is recorded as
//! cancelled and skipped, the operation returns its report through the
//! normal path, and every `Drop` along the way (the install lock, the
//! extraction temp dirs) runs as it would on a clean finish.
//!
//! The token is `Send + Sync` and cheap to clone: the UI holds one, hands a
//! clone to the worker, and flips it from the close handler.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// A shared "please stop" flag. Clones observe the same flag, so cancelling
/// any clone cancels them all.
///
/// A token created with [`CancelToken::new`] and never cancelled is the
/// no-op case, which is what the non-interactive entry points (the CLI,
/// tests) pass so they don't have to care.
#[derive(Clone, Default)]
pub struct CancelToken {
    flag: Arc<AtomicBool>,
    /// Set for tokens made by [`CancelToken::child`]: the child is
    /// cancelled when its parent is, but not the other way around.
    parent: Option<Arc<CancelToken>>,
}

impl CancelToken {
    pub fn new() -> Self {
        Self::default()
    }

    /// Ask everything watching this token to stop at its next checkpoint.
    pub fn cancel(&self) {
        self.flag.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.flag.load(Ordering::Relaxed)
            || self
                .parent
                .as_ref()
                .is_some_and(|parent| parent.is_cancelled())
    }

    /// A token that stops when `self` stops, plus whenever it is cancelled
    /// on its own. Cancelling the child leaves the parent alone.
    ///
    /// The download pool uses this: it cancels its own workers when the
    /// batch bails on a failed artifact, without telling the caller's token
    /// that the whole operation was cancelled.
    pub fn child(&self) -> Self {
        Self {
            flag: Arc::new(AtomicBool::new(false)),
            parent: Some(Arc::new(self.clone())),
        }
    }
}

impl std::fmt::Debug for CancelToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CancelToken")
            .field("cancelled", &self.is_cancelled())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_token_is_not_cancelled() {
        assert!(!CancelToken::new().is_cancelled());
    }

    #[test]
    fn clones_share_one_flag() {
        let token = CancelToken::new();
        let clone = token.clone();

        token.cancel();

        assert!(clone.is_cancelled());
    }

    #[test]
    fn a_child_follows_its_parent() {
        let parent = CancelToken::new();
        let child = parent.child();

        parent.cancel();

        assert!(child.is_cancelled());
    }

    #[test]
    fn a_cancelled_child_leaves_its_parent_alone() {
        let parent = CancelToken::new();
        let child = parent.child();

        child.cancel();

        assert!(child.is_cancelled());
        assert!(!parent.is_cancelled());
    }
}
