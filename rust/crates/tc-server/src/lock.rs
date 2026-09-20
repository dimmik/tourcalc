//! Taking a lock that a panic has passed through.
//!
//! `std`'s `RwLock` and `Mutex` mark themselves **poisoned** when a thread panics while
//! holding them, and every later `read()`/`write()` answers with an error. That is a
//! sensible default when a panic ends the process: nobody sees the error anyway.
//!
//! This server no longer ends on a panic. It is built to unwind and wraps its handlers in
//! `CatchPanicLayer`, so a panicking request becomes a 500 and the next request is served -
//! except that the poison would outlive it, and every later request touching the same lock
//! would panic in its turn. One bad request would leave a server that is up, answers 500 to
//! everything, and never restarts because nothing has died.
//!
//! So the poison is ignored, deliberately. What these locks hold is a map of tours and a map
//! of subscriptions: an insert that did not finish leaves the map without that entry, never
//! half an entry, because the value is built before the lock is taken. The worst case is a
//! lost write, which is what the failed request already was.

use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

pub fn read<T>(lock: &RwLock<T>) -> RwLockReadGuard<'_, T> {
    lock.read().unwrap_or_else(|poisoned| poisoned.into_inner())
}

pub fn write<T>(lock: &RwLock<T>) -> RwLockWriteGuard<'_, T> {
    lock.write().unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A panic under the lock costs that one write and nothing else.
    #[test]
    fn a_poisoned_lock_still_opens() {
        let numbers: RwLock<Vec<i32>> = RwLock::new(vec![1]);

        let poisoned = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut held = write(&numbers);
            held.push(2);
            panic!("as a handler might");
        }));
        assert!(poisoned.is_err());
        assert!(numbers.read().is_err(), "std now refuses it");

        // The server does not.
        assert_eq!(*read(&numbers), vec![1, 2]);
        write(&numbers).push(3);
        assert_eq!(*read(&numbers), vec![1, 2, 3]);
    }
}
