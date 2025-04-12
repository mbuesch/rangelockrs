// -*- coding: utf-8 -*-
//
// Copyright 2021-2025 Michael Büsch <m@bues.ch>
//
// Licensed under the Apache License version 2.0
// or the MIT license, at your option.
// SPDX-License-Identifier: Apache-2.0 OR MIT
//

use crate::{lockedranges::LockedRanges, util::get_bounds};
use std::{
    cell::UnsafeCell,
    marker::PhantomData,
    ops::{Deref, DerefMut, Range, RangeBounds},
    sync::{LockResult, Mutex, PoisonError, TryLockError, TryLockResult},
};

/// General purpose multi-thread range lock for [std::vec::Vec].
///
/// # Example
///
/// ```
/// use range_lock::VecRangeLock;
/// use std::{sync::{Arc, Barrier}, thread};
///
/// let data = vec![10, 11, 12, 13];
///
/// let data_lock0 = Arc::new(VecRangeLock::new(data));
/// let data_lock1 = Arc::clone(&data_lock0);
/// let data_lock2 = Arc::clone(&data_lock0);
///
/// // Thread barrier, only for demonstration purposes.
/// let barrier0 = Arc::new(Barrier::new(2));
/// let barrier1 = Arc::clone(&barrier0);
///
/// thread::scope(|s| {
///     s.spawn(move || {
///         {
///             let mut guard = data_lock0.try_lock(0..2).expect("T0: Failed to lock 0..2");
///             guard[0] = 100; // Write to data[0]
///         }
///         barrier0.wait(); // Synchronize with second thread.
///         {
///             let guard = data_lock0.try_lock(2..4).expect("T0: Failed to lock 2..4");
///             assert_eq!(guard[0], 200); // Read from data[2]
///         }
///     });
///
///     s.spawn(move || {
///         {
///             let mut guard = data_lock1.try_lock(2..4).expect("T1: Failed to lock 2..4");
///             guard[0] = 200; // Write to data[2]
///         }
///         barrier1.wait(); // Synchronize with first thread.
///         {
///             let guard = data_lock1.try_lock(0..2).expect("T1: Failed to lock 0..2");
///             assert_eq!(guard[0], 100); // Read from data[0]
///         }
///     });
/// });
///
/// let data = Arc::try_unwrap(data_lock2).expect("Arc unwrap failed").into_inner();
///
/// assert_eq!(data, vec![100, 11, 200, 13]);
/// ```
#[derive(Debug)]
pub struct VecRangeLock<T> {
    /// Set of the currently locked ranges.
    ranges: Mutex<LockedRanges>,
    /// The underlying data.
    data: UnsafeCell<Vec<T>>,
    /// Length of the underlying Vec.
    len: usize,
}

// SAFETY:
// It is safe to access VecRangeLock and the contained data (via VecRangeLockGuard)
// from multiple threads simultaneously.
// The lock ensures that access to the data is strictly serialized.
// T must be Send-able to other threads.
unsafe impl<T> Sync for VecRangeLock<T> where T: Send {}

impl<'a, T> VecRangeLock<T> {
    /// Construct a new [VecRangeLock].
    ///
    /// * `data`: The data [Vec] to protect.
    pub fn new(data: Vec<T>) -> VecRangeLock<T> {
        let len = data.len();
        VecRangeLock {
            ranges: Mutex::new(LockedRanges::new()),
            data: UnsafeCell::new(data),
            len,
        }
    }

    /// Get the length (in number of elements) of the embedded [Vec].
    #[inline]
    pub fn data_len(&self) -> usize {
        self.len
    }

    /// Unwrap this [VecRangeLock] into the contained data.
    /// This method consumes self.
    #[inline]
    pub fn into_inner(self) -> Vec<T> {
        debug_assert!(self.ranges.lock().unwrap().is_empty());
        self.data.into_inner()
    }

    /// Try to lock the given data `range`.
    ///
    /// * On success: Returns a [VecRangeLockGuard] that can be used to access the locked region.
    ///               Dereferencing [VecRangeLockGuard] yields a slice of the `data`.
    /// * On failure: Returns [TryLockError::WouldBlock], if the range is contended.
    ///               The locking attempt may be retried by the caller upon contention.
    ///               Returns [TryLockError::Poisoned], if the lock is poisoned.
    pub fn try_lock(
        &'a self,
        range: impl RangeBounds<usize>,
    ) -> TryLockResult<VecRangeLockGuard<'a, T>> {
        let data_len = self.data_len();
        let (range_start, range_end) = get_bounds(&range, data_len);
        if range_start >= data_len || range_end > data_len {
            panic!("Range is out of bounds.");
        }
        if range_start > range_end {
            panic!("Invalid range. Start is bigger than end.");
        }
        let range = range_start..range_end;

        if range.is_empty() {
            TryLockResult::Ok(VecRangeLockGuard::new(self, range))
        } else if let LockResult::Ok(mut ranges) = self.ranges.lock() {
            if ranges.insert(&range) {
                TryLockResult::Ok(VecRangeLockGuard::new(self, range))
            } else {
                TryLockResult::Err(TryLockError::WouldBlock)
            }
        } else {
            TryLockResult::Err(TryLockError::Poisoned(PoisonError::new(
                VecRangeLockGuard::new(self, range),
            )))
        }
    }

    /// Unlock a range.
    fn unlock(&self, range: &Range<usize>) {
        if !range.is_empty() {
            let mut ranges = self
                .ranges
                .lock()
                .expect("VecRangeLock: Failed to take ranges mutex.");
            ranges.remove(range);
        }
    }

    /// Get an immutable slice to the specified range.
    ///
    /// # SAFETY
    ///
    /// See get_mut_slice().
    #[inline]
    unsafe fn get_slice(&self, range: &Range<usize>) -> &[T] {
        let data = (*self.data.get()).as_ptr();
        unsafe {
            core::slice::from_raw_parts(
                data.offset(range.start.try_into().unwrap()) as _,
                range.end - range.start
            )
        }
    }

    /// Get a mutable slice to the specified range.
    ///
    /// # SAFETY
    ///
    /// The caller must ensure that:
    /// * No overlapping slices must coexist on multiple threads.
    /// * Immutable slices to overlapping ranges may only coexist on a single thread.
    /// * Immutable and mutable slices must not coexist.
    #[inline]
    unsafe fn get_mut_slice(&self, range: &Range<usize>) -> &mut [T] {
        let data = (*self.data.get()).as_mut_ptr();
        unsafe {
            core::slice::from_raw_parts_mut(
                data.offset(range.start.try_into().unwrap()) as _,
                range.end - range.start
            )
        }
    }
}

/// Lock guard variable type for [VecRangeLock].
///
/// The [Deref] and [DerefMut] traits are implemented for this struct.
/// See the documentation of [VecRangeLock] for usage examples of [VecRangeLockGuard].
#[derive(Debug)]
pub struct VecRangeLockGuard<'a, T> {
    /// Reference to the underlying lock.
    lock: &'a VecRangeLock<T>,
    /// The locked range.
    range: Range<usize>,

    /// Suppresses Send and Sync autotraits for VecRangeLockGuard.
    _p: PhantomData<*mut T>,
}

impl<'a, T> VecRangeLockGuard<'a, T> {
    #[inline]
    fn new(lock: &'a VecRangeLock<T>, range: Range<usize>) -> VecRangeLockGuard<'a, T> {
        VecRangeLockGuard {
            lock,
            range,
            _p: PhantomData,
        }
    }
}

impl<'a, T> Drop for VecRangeLockGuard<'a, T> {
    #[inline]
    fn drop(&mut self) {
        self.lock.unlock(&self.range);
    }
}

impl<'a, T> Deref for VecRangeLockGuard<'a, T> {
    type Target = [T];

    #[inline]
    fn deref(&self) -> &Self::Target {
        // SAFETY: See deref_mut().
        unsafe { self.lock.get_slice(&self.range) }
    }
}

impl<'a, T> DerefMut for VecRangeLockGuard<'a, T> {
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
    use super::*;
    use std::cell::RefCell;
    use std::sync::{Arc, Barrier};
    use std::thread;

    #[test]
    fn test_base() {
        {
            // Range
            let a = VecRangeLock::new(vec![1_i32, 2, 3, 4, 5, 6]);
            {
                let mut g = a.try_lock(2..4).unwrap();
                assert!(!a.ranges.lock().unwrap().is_empty());
                assert_eq!(g[0..2], [3, 4]);
                g[1] = 10;
                assert_eq!(g[0..2], [3, 10]);
            }
            assert!(a.ranges.lock().unwrap().is_empty());
        }
        {
            // RangeInclusive
            let a = VecRangeLock::new(vec![1_i32, 2, 3, 4, 5, 6]);
            let g = a.try_lock(2..=4).unwrap();
            assert_eq!(g[0..3], [3, 4, 5]);
        }
        {
            // RangeTo
            let a = VecRangeLock::new(vec![1_i32, 2, 3, 4, 5, 6]);
            let g = a.try_lock(..4).unwrap();
            assert_eq!(g[0..4], [1, 2, 3, 4]);
        }
        {
            // RangeToInclusive
            let a = VecRangeLock::new(vec![1_i32, 2, 3, 4, 5, 6]);
            let g = a.try_lock(..=4).unwrap();
            assert_eq!(g[0..5], [1, 2, 3, 4, 5]);
        }
        {
            // RangeFrom
            let a = VecRangeLock::new(vec![1_i32, 2, 3, 4, 5, 6]);
            let g = a.try_lock(2..).unwrap();
            assert_eq!(g[0..4], [3, 4, 5, 6]);
        }
        {
            // RangeFull
            let a = VecRangeLock::new(vec![1_i32, 2, 3, 4, 5, 6]);
            let g = a.try_lock(..).unwrap();
            assert_eq!(g[0..6], [1, 2, 3, 4, 5, 6]);
        }
    }

    #[test]
    fn test_empty_range() {
        // Empty range doesn't cause conflicts.
        let a = VecRangeLock::new(vec![1_i32, 2, 3, 4, 5, 6]);
        let g0 = a.try_lock(2..2).unwrap();
        assert!(a.ranges.lock().unwrap().is_empty());
        assert_eq!(g0[0..0], []);
        let g1 = a.try_lock(2..2).unwrap();
        assert!(a.ranges.lock().unwrap().is_empty());
        assert_eq!(g1[0..0], []);
    }

    #[test]
    #[should_panic(expected = "index out of bounds")]
    fn test_base_oob_read() {
        let a = VecRangeLock::new(vec![1_i32, 2, 3, 4, 5, 6]);
        let g = a.try_lock(2..4).unwrap();
        let _ = g[2];
    }

    #[test]
    #[should_panic(expected = "index out of bounds")]
    fn test_base_oob_write() {
        let a = VecRangeLock::new(vec![1_i32, 2, 3, 4, 5, 6]);
        let mut g = a.try_lock(2..4).unwrap();
        g[2] = 10;
    }

    #[test]
    #[should_panic(expected = "guard 1 panicked")]
    fn test_overlap0() {
        let a = VecRangeLock::new(vec![1_i32, 2, 3, 4, 5, 6]);
        let _g0 = a.try_lock(2..4).expect("guard 0 panicked");
        let _g1 = a.try_lock(3..5).expect("guard 1 panicked");
    }

    #[test]
    #[should_panic(expected = "guard 0 panicked")]
    fn test_overlap1() {
        let a = VecRangeLock::new(vec![1_i32, 2, 3, 4, 5, 6]);
        let _g1 = a.try_lock(3..5).expect("guard 1 panicked");
        let _g0 = a.try_lock(2..4).expect("guard 0 panicked");
    }

    #[test]
    fn test_thread_no_overlap() {
        let a = Arc::new(VecRangeLock::new(vec![1_i32, 2, 3, 4, 5, 6]));
        let b = Arc::clone(&a);
        let c = Arc::clone(&a);
        let ba0 = Arc::new(Barrier::new(2));
        let ba1 = Arc::clone(&ba0);
        let j0 = thread::spawn(move || {
            {
                let mut g = b.try_lock(2..4).unwrap();
                assert!(!b.ranges.lock().unwrap().is_empty());
                assert_eq!(g[0..2], [3, 4]);
                g[1] = 10;
                assert_eq!(g[0..2], [3, 10]);
            }
            ba0.wait();
        });
        let j1 = thread::spawn(move || {
            {
                let g = c.try_lock(4..6).unwrap();
                assert!(!c.ranges.lock().unwrap().is_empty());
                assert_eq!(g[0..2], [5, 6]);
            }
            ba1.wait();
            let g = c.try_lock(3..5).unwrap();
            assert_eq!(g[0..2], [10, 5]);
        });
        j1.join().expect("Thread 1 panicked.");
        j0.join().expect("Thread 0 panicked.");
        assert!(a.ranges.lock().unwrap().is_empty());
    }

    #[allow(dead_code)]
    struct NoSyncStruct(RefCell<u32>); // No Sync auto-trait.

    #[test]
    fn test_nosync() {
        let a = Arc::new(VecRangeLock::new(vec![
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
            assert!(!b.ranges.lock().unwrap().is_empty());
            ba0.wait();
        });
        let j1 = thread::spawn(move || {
            let _g = c.try_lock(1..2).unwrap();
            assert!(!c.ranges.lock().unwrap().is_empty());
            ba1.wait();
        });
        j1.join().expect("Thread 1 panicked.");
        j0.join().expect("Thread 0 panicked.");
        assert!(a.ranges.lock().unwrap().is_empty());
    }
}

// vim: ts=4 sw=4 expandtab
