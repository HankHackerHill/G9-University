//! Cooperative cancellation (ARCHITECTURE.md §10 "cancellation releases
//! resources; a late response for a cancelled invocation is discarded";
//! handoff §13 "Timeouts and cancellations must cleanly release resources").
//!
//! A [`CancelToken`] is a shared flag. The orchestrator keeps one clone and
//! hands another to the runtime; `cancel()` on any clone is visible to all.
//!
//! **Late responses.** A runtime may only observe the flag at its next check
//! point, so a response can still arrive after `cancel()` was called. Such a
//! response **must be discarded by the caller**: pass every response through
//! [`Guard::accept`], which returns `None` once the token is cancelled, and
//! never persist, validate or display a response that was not accepted.
//! [`crate::run_inference`] applies this rule automatically.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Shared cancellation flag. Cloning yields another handle to the same flag.
#[derive(Debug, Clone, Default)]
pub struct CancelToken {
    flag: Arc<AtomicBool>,
}

impl CancelToken {
    /// A fresh, not-yet-cancelled token.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the flag. Idempotent; cannot be undone.
    pub fn cancel(&self) {
        self.flag.store(true, Ordering::SeqCst);
    }

    /// Whether `cancel()` was called on any handle of this token.
    pub fn is_cancelled(&self) -> bool {
        self.flag.load(Ordering::SeqCst)
    }

    /// Whether two handles refer to the same underlying flag.
    pub fn same_token(&self, other: &CancelToken) -> bool {
        Arc::ptr_eq(&self.flag, &other.flag)
    }
}

/// Late-response guard. Stateless; the token carries the state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Guard;

impl Guard {
    /// Accept `response` only if `token` is still live. When the token was
    /// cancelled the response is dropped here and `None` is returned, so a late
    /// response can never reach validation, the UI or storage.
    pub fn accept<T>(response: T, token: &CancelToken) -> Option<T> {
        if token.is_cancelled() {
            drop(response);
            None
        } else {
            Some(response)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clones_share_the_flag() {
        let a = CancelToken::new();
        let b = a.clone();
        assert!(!a.is_cancelled());
        assert!(a.same_token(&b));
        b.cancel();
        assert!(a.is_cancelled());
        assert!(b.is_cancelled());
        // Idempotent.
        a.cancel();
        assert!(a.is_cancelled());
        assert!(!a.same_token(&CancelToken::new()));
    }

    #[test]
    fn guard_accepts_only_while_live() {
        let t = CancelToken::new();
        assert_eq!(Guard::accept("late?", &t), Some("late?"));
        t.cancel();
        assert_eq!(Guard::accept("late", &t), None);
    }
}
