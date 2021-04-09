// -*- coding: utf-8 -*-
//
// Copyright 2021 Michael Büsch <m@bues.ch>
//
// Licensed under the Apache License version 2.0
// or the MIT license, at your option.
// SPDX-License-Identifier: Apache-2.0 OR MIT
//

use std::{
    cell::UnsafeCell,
    collections::HashSet,
    ops::{
        Deref,
        DerefMut,
        Range,
    },
    sync::{
        LockResult,
        Mutex,
        PoisonError,
        TryLockError,
        TryLockResult,
    }
};

#[derive(Debug)]
pub struct RangeLock<T> {
    ranges: Mutex<HashSet<Range<usize>>>,
    data:   UnsafeCell<Vec<T>>,
}

// SAFETY:
// It is safe to access RangeLock and the contained data (via RangeLockGuard)
// from multiple threads simultaneously.
// The lock ensures that access to the data is strictly serialized.
unsafe impl<T> Sync for RangeLock<T> {}

fn overlaps_any(ranges: &HashSet<Range<usize>>,
                range:  &Range<usize>) -> bool {
    for r in ranges {
        if range.end > r.start && range.start < r.end {
            return true;
        }
    }
    false
}

impl<'a, T> RangeLock<T> {
    /// Construct a new RangeLock.
    pub fn new(data: Vec<T>) -> RangeLock<T> {
        RangeLock {
            ranges: Mutex::new(HashSet::new()),
            data:   UnsafeCell::new(data),
        }
    }

    /// Unwrap the RangeLock into the contained data.
    pub fn into_inner(self) -> Vec<T> {
        debug_assert!(self.ranges.lock().unwrap().is_empty());
        self.data.into_inner()
    }

    /// Try to lock the given data range.
    pub fn try_lock(&'a self, range: Range<usize>) -> TryLockResult<RangeLockGuard<'a, T>> {
        if let LockResult::Ok(mut ranges) = self.ranges.lock() {
            if overlaps_any(&*ranges, &range) {
                TryLockResult::Err(TryLockError::WouldBlock)
            } else {
                ranges.insert(range.clone());
                TryLockResult::Ok(RangeLockGuard::new(self, range))
            }
        } else {
            TryLockResult::Err(TryLockError::Poisoned(
                PoisonError::new(RangeLockGuard::new(self, range))))
        }
    }

    fn unlock(&self, range: &Range<usize>) {
        let mut ranges = self.ranges.lock()
            .expect("RangeLock: Failed to take ranges mutex.");
        ranges.remove(range);
    }

    // SAFETY: See get_mut_slice().
    #[inline]
    unsafe fn get_slice(&self, range: &Range<usize>) -> &[T] {
        &(*self.data.get())[range.clone()]
    }

    // SAFETY:
    // The caller must ensure that:
    // * No overlapping slices must coexist on multiple threads.
    // * Immutable slices to overlapping ranges may only coexist on a single thread.
    // * Immutable and mutable slices must not coexist.
    #[inline]
    unsafe fn get_mut_slice(&self, range: &Range<usize>) -> &mut [T] {
        let cptr = self.get_slice(range) as *const [T];
        let mut_slice = (cptr as *mut [T]).as_mut();
        mut_slice.unwrap() // The pointer is never null.
    }
}

#[derive(Debug)]
pub struct RangeLockGuard<'a, T> {
    lock:   &'a RangeLock<T>,
    range:  Range<usize>,
}

impl<'a, T> RangeLockGuard<'a, T> {
    fn new(lock:    &'a RangeLock<T>,
           range:   Range<usize>) -> RangeLockGuard<'a, T> {
        RangeLockGuard {
            lock,
            range,
        }
    }
}

impl<'a, T> Drop for RangeLockGuard<'a, T> {
    #[inline]
    fn drop(&mut self) {
        self.lock.unlock(&self.range);
    }
}

impl<'a, T> Deref for RangeLockGuard<'a, T> {
    type Target = [T];

    #[inline]
    fn deref(&self) -> &Self::Target {
        // SAFETY: See deref_mut().
        unsafe { self.lock.get_slice(&self.range) }
    }
}

impl<'a, T> DerefMut for RangeLockGuard<'a, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        // SAFETY:
        // The lifetime of the slice is bounded by the lifetime of the guard.
        // The lifetime of the guard is bounded by the lifetime of the range lock.
        // The underlying data is owned by the range lock.
        // Therefore the slice cannot outlive the data.
        // The range lock ensures that no overlapping/conflicting guards
        // can be constructed.
        // The compiler ensures that the DerefMut result cannot be used,
        // if there's also an immutable Deref result.
        unsafe { self.lock.get_mut_slice(&self.range) }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::sync::{Arc, Barrier};
    use std::thread;
    use super::*;

    #[test]
    fn test_base() {
        let a = RangeLock::new(vec![1_i32, 2, 3, 4, 5, 6]);
        let mut g = a.try_lock(2..4).unwrap();
        assert_eq!(g[0..2], [3, 4]);
        g[1] = 10;
        assert_eq!(g[0..2], [3, 10]);
    }

    #[test]
    #[should_panic(expected="index out of bounds")]
    fn test_base_oob_read() {
        let a = RangeLock::new(vec![1_i32, 2, 3, 4, 5, 6]);
        let g = a.try_lock(2..4).unwrap();
        let _ = g[2];
    }

    #[test]
    #[should_panic(expected="index out of bounds")]
    fn test_base_oob_write() {
        let a = RangeLock::new(vec![1_i32, 2, 3, 4, 5, 6]);
        let mut g = a.try_lock(2..4).unwrap();
        g[2] = 10;
    }

    #[test]
    #[should_panic(expected="guard 1 panicked")]
    fn test_overlap0() {
        let a = RangeLock::new(vec![1_i32, 2, 3, 4, 5, 6]);
        let _g0 = a.try_lock(2..4).expect("guard 0 panicked");
        let _g1 = a.try_lock(3..5).expect("guard 1 panicked");
    }

    #[test]
    #[should_panic(expected="guard 0 panicked")]
    fn test_overlap1() {
        let a = RangeLock::new(vec![1_i32, 2, 3, 4, 5, 6]);
        let _g1 = a.try_lock(3..5).expect("guard 1 panicked");
        let _g0 = a.try_lock(2..4).expect("guard 0 panicked");
    }

    #[test]
    fn test_thread_no_overlap() {
        let a = Arc::new(RangeLock::new(vec![1_i32, 2, 3, 4, 5, 6]));
        let b = Arc::clone(&a);
        let c = Arc::clone(&a);
        let ba0 = Arc::new(Barrier::new(2));
        let ba1 = Arc::clone(&ba0);
        let j0 = thread::spawn(move || {
            {
                let mut g = b.try_lock(2..4).unwrap();
                assert_eq!(g[0..2], [3, 4]);
                g[1] = 10;
                assert_eq!(g[0..2], [3, 10]);
            }
            ba0.wait();
        });
        let j1 = thread::spawn(move || {
            {
                let g = c.try_lock(4..6).unwrap();
                assert_eq!(g[0..2], [5, 6]);
            }
            ba1.wait();
            let g = c.try_lock(3..5).unwrap();
            assert_eq!(g[0..2], [10, 5]);
        });
        j1.join().expect("Thread 1 panicked.");
        j0.join().expect("Thread 0 panicked.");
    }

    struct NoSyncStruct(RefCell<u32>); // No Sync auto-trait.

    #[test]
    fn test_nosync() {
        let a = Arc::new(RangeLock::new(vec![
            NoSyncStruct(RefCell::new(1)),
            NoSyncStruct(RefCell::new(2)),
            NoSyncStruct(RefCell::new(3)),
            NoSyncStruct(RefCell::new(4)),
        ]));
        let b = Arc::clone(&a);
        let c = Arc::clone(&a);
        let ba0 = Arc::new(Barrier::new(2));
        let ba1 = Arc::clone(&ba0);
        let j0 = thread::spawn(move || {
            let _g = b.try_lock(0..1).unwrap();
            ba0.wait();
        });
        let j1 = thread::spawn(move || {
            let _g = c.try_lock(1..2).unwrap();
            ba1.wait();
        });
        j1.join().expect("Thread 1 panicked.");
        j0.join().expect("Thread 0 panicked.");
    }

    #[test]
    fn test_overlaps_any() {
        let mut a = HashSet::new();
        a.insert(0..1);
        a.insert(4..6);
        assert!(overlaps_any(&a, &(0..1)));
        assert!(!overlaps_any(&a, &(1..2)));
        assert!(!overlaps_any(&a, &(1..3)));
        assert!(!overlaps_any(&a, &(2..4)));
        assert!(overlaps_any(&a, &(3..5)));
        assert!(overlaps_any(&a, &(4..6)));
        assert!(overlaps_any(&a, &(5..7)));
        assert!(!overlaps_any(&a, &(6..8)));
        assert!(!overlaps_any(&a, &(7..9)));
    }
}

// vim: ts=4 sw=4 expandtab
